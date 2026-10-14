//! The loopback HTTP server and its background guard.
//!
//! [`Server::bind`] refuses any non-loopback address (`core-api.md` §2, §15)
//! and serves the canonical v1 routes (`core-api.md` §6). Every request is
//! authenticated by the loopback/ingress trust boundary (§3): a loopback peer,
//! the add-on ingress identity header, or a valid bearer token; a token in a
//! query string is always rejected. The WebSocket stream is not implemented in
//! v1: `GET /api/v1/stream` returns a typed `501`, so clients poll
//! (`core-api.md` §10).
//!
//! [`Server::serve_in_background`] runs the accept loop on a thread and returns
//! a [`ServerGuard`] that stops it on drop, so a composition root can start the
//! control plane, wire it to the sensing core, and shut it down cleanly.

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use aquila_geometry::Topology;
use aquila_ha::EntityModel;
use serde_json::{json, Value};

use crate::error::ApiError;
use crate::handle::Handle;
use crate::http::{read_request, write_response, Request};
use crate::projection;

/// How long the non-blocking accept loop pauses between polls.
const ACCEPT_POLL: Duration = Duration::from_millis(2);

/// The control plane's authentication policy (`core-api.md` §3).
///
/// The trust boundary is the loopback/ingress seam. A request is accepted when
/// a loopback peer connects, when the add-on ingress identity header is
/// present, or when a valid bearer token is presented. There is no anonymous
/// mode on a non-loopback connection except the `/live` probe, and a token in a
/// query string is always rejected.
#[derive(Debug, Clone, Default)]
pub struct Auth {
    token: Option<String>,
}

impl Auth {
    /// A policy with an optional local bearer token. `None` means no bearer
    /// token is configured (the loopback/ingress boundary is the credential).
    #[must_use]
    pub fn new(token: Option<String>) -> Self {
        Self { token }
    }

    /// The default policy: no bearer token configured.
    #[must_use]
    pub fn open() -> Self {
        Self { token: None }
    }

    /// The configured token, if any.
    #[must_use]
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }
}

/// The private loopback HTTP control plane (`core-api.md`).
pub struct Server {
    listener: TcpListener,
    handle: Handle,
    auth: Auth,
}

impl Server {
    /// Bind a server to `addr` with the default [`Auth`] policy and initialise
    /// the projected model and read-only documents from `topology`.
    ///
    /// # Errors
    ///
    /// Returns [`ApiError::NonLoopback`] if `addr` is not a loopback address, or
    /// [`ApiError::Io`] if the socket cannot be bound.
    pub fn bind(
        addr: SocketAddr,
        topology: &Topology,
        instance_id: impl Into<String>,
    ) -> Result<Self, ApiError> {
        Self::bind_with_auth(addr, topology, instance_id, Auth::open())
    }

    /// Bind a server to `addr` with an explicit [`Auth`] policy.
    ///
    /// # Errors
    ///
    /// Returns [`ApiError::NonLoopback`] if `addr` is not loopback, or
    /// [`ApiError::Io`] if the socket cannot be bound.
    pub fn bind_with_auth(
        addr: SocketAddr,
        topology: &Topology,
        instance_id: impl Into<String>,
        auth: Auth,
    ) -> Result<Self, ApiError> {
        if !addr.ip().is_loopback() {
            return Err(ApiError::NonLoopback(addr));
        }
        let listener = TcpListener::bind(addr)?;
        let instance_id = instance_id.into();
        let projections = projection::build(topology, &instance_id);
        let handle = Handle::new(
            EntityModel::from_topology(topology, instance_id),
            projections,
        );
        Ok(Self {
            listener,
            handle,
            auth,
        })
    }

    /// The bound address (`127.0.0.1:0` resolves to the chosen ephemeral port).
    ///
    /// # Errors
    ///
    /// Returns [`ApiError::Io`] if the local address cannot be read.
    pub fn local_addr(&self) -> Result<SocketAddr, ApiError> {
        Ok(self.listener.local_addr()?)
    }

    /// A handle to publish events and serve state.
    #[must_use]
    pub fn handle(&self) -> Handle {
        self.handle.clone()
    }

    /// Serve until the process stops (a blocking, never-returning loop).
    ///
    /// # Errors
    ///
    /// Returns [`ApiError::Io`] if the accept loop fails.
    pub fn run(self) -> Result<(), ApiError> {
        for incoming in self.listener.incoming() {
            match incoming {
                Ok(mut stream) => {
                    let _ = handle_client(&mut stream, &self.handle, &self.auth);
                }
                Err(error) => return Err(ApiError::Io(error)),
            }
        }
        Ok(())
    }

    /// Serve on a background thread, returning a guard that stops it on drop.
    ///
    /// # Errors
    ///
    /// Returns [`ApiError::Io`] if the listener cannot be put into non-blocking
    /// mode.
    pub fn serve_in_background(self) -> Result<ServerGuard, ApiError> {
        let Self {
            listener,
            handle,
            auth,
        } = self;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        let join = thread::spawn(move || {
            while !stop_thread.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = handle_client(&mut stream, &handle, &auth);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(ACCEPT_POLL);
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(ServerGuard {
            addr,
            stop,
            join: Some(join),
        })
    }
}

/// A running background server. Stopping is idempotent; the thread is joined on
/// [`shutdown`](ServerGuard::shutdown) and on `Drop`.
pub struct ServerGuard {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl ServerGuard {
    /// The address the server is listening on.
    #[must_use]
    pub const fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Stop the server and wait for its thread to finish.
    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Serve one request on an accepted connection.
fn handle_client(stream: &mut TcpStream, handle: &Handle, auth: &Auth) -> Result<(), ApiError> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let request = read_request(stream)?;
    let peer_loopback = stream
        .peer_addr()
        .map(|addr| addr.ip().is_loopback())
        .unwrap_or(false);
    let path = request.path.clone();
    let segments = segments(&path);

    // The liveness probe is the one anonymous route (`core-api.md` §4).
    if request.method == "GET" && segments == ["api", "v1", "live"] {
        return write_json(stream, 200, &json!({ "status": "ok" }));
    }

    if !authorised(&request, peer_loopback, auth) {
        return problem(
            stream,
            401,
            "unauthenticated",
            "Missing or invalid credentials",
            false,
            "the control plane requires a loopback peer, an ingress identity header or a bearer token",
        );
    }

    route(stream, &request, segments.as_slice(), handle)
}

/// The canonical v1 router (`core-api.md` §6).
fn route(
    stream: &mut TcpStream,
    request: &Request,
    segments: &[&str],
    handle: &Handle,
) -> Result<(), ApiError> {
    match (request.method.as_str(), segments) {
        ("GET", ["api", "v1", "health"]) => write_json(stream, 200, &handle.health_json()),
        ("GET", ["api", "v1", "state"]) => write_json(stream, 200, &handle.state_json()),
        ("GET", ["api", "v1", "schema"]) => write_json(stream, 200, &handle.schema_json()),
        ("GET", ["api", "v1", "capabilities"]) => write_json(stream, 200, &capabilities()),
        ("GET", ["api", "v1", "nodes"]) => write_json(stream, 200, &handle.nodes_json()),
        ("GET", ["api", "v1", "firmware"]) => write_json(stream, 200, &handle.firmware_json()),
        ("GET", ["api", "v1", "metrics", "snapshot"]) => {
            write_json(stream, 200, &handle.metrics_json())
        }
        ("GET", ["api", "v1", "nodes", node, "firmware"]) => {
            match handle.node_firmware_json(node) {
                Some(body) => write_json(stream, 200, &body),
                None => not_found(stream),
            }
        }
        ("GET", ["api", "v1", "nodes", node, "firmware", "job"]) => {
            if handle.node_firmware_json(node).is_some() {
                write_json(
                    stream,
                    200,
                    &json!({ "node": node, "state": "idle", "progress": Value::Null,
                             "result": Value::Null }),
                )
            } else {
                not_found(stream)
            }
        }
        // OTA is specified (`firmware-ota.md`) but not implemented in v1: the
        // endpoints exist only to fail closed with a typed 503, and the node
        // control opcodes 0x0E–0x14 are unimplemented (F2/F28/F29/F36/F38).
        ("POST", ["api", "v1", "nodes", node, "firmware", "install"])
        | ("POST", ["api", "v1", "nodes", node, "firmware", "abort"]) => {
            if handle.node_firmware_json(node).is_some() {
                problem(
                    stream,
                    503,
                    "ota_unavailable",
                    "Firmware OTA is not implemented in this build",
                    true,
                    "the OTA manager is ticketed (#13–#15); the endpoint exists and fails closed",
                )
            } else {
                not_found(stream)
            }
        }
        ("GET", ["api", "v1", "stream"]) if request.websocket => problem(
            stream,
            501,
            "websocket_unimplemented",
            "WebSocket upgrade is not implemented in v1",
            true,
            "v1 is HTTP polling only; clients poll /api/v1/state (core-api.md §10)",
        ),
        ("GET", ["api", "v1", "stream"]) => problem(
            stream,
            400,
            "validation_failed",
            "WebSocket upgrade required",
            false,
            "GET /api/v1/stream is a WebSocket route in v1; it is not implemented",
        ),
        _ => not_found(stream),
    }
}

/// The capability document (`core-api.md` §4.2). WebSocket is honestly absent.
fn capabilities() -> Value {
    json!({
        "api": { "major": 1, "minor": 0 },
        "transports": { "http": true, "websocket": false },
        "features": {
            "calibrationWizard": false,
            "nodeControl": false,
            "firmwareOta": false,
            "polling": true
        },
        "minComponentVersion": "0.1.0"
    })
}

/// Evaluate the trust policy (`core-api.md` §3).
///
/// Order matters: a credential-shaped query parameter is rejected before any
/// other mode so it can never be smuggled in alongside one.
fn authorised(request: &Request, peer_loopback: bool, auth: &Auth) -> bool {
    if request.query_has_credential() {
        return false;
    }
    if let Some(header) = request.header("authorization") {
        let presented = header
            .strip_prefix("Bearer ")
            .or_else(|| header.strip_prefix("bearer "));
        return match (presented, auth.token()) {
            (Some(presented), Some(expected)) => constant_eq(presented, expected),
            _ => false,
        };
    }
    if request.header("x-aquila-ingress").is_some() || request.header("x-remote-user-id").is_some()
    {
        return true;
    }
    peer_loopback
}

/// Compare two secrets without an early exit on the first differing byte.
///
/// The length comparison leaks only the secret's length, which is fixed by the
/// token format (`core-api.md` §15).
fn constant_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Split a request path into non-empty segments.
fn segments(path: &str) -> Vec<&str> {
    path.trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

/// Serialise `value` and write a `200` JSON response.
fn write_json(stream: &mut TcpStream, status: u16, value: &Value) -> Result<(), ApiError> {
    let body = serde_json::to_vec(value).map_err(|error| ApiError::Other(error.to_string()))?;
    write_response(stream, status, "application/json; charset=utf-8", &body)
}

/// Write a typed `application/problem+json` response (`core-api.md` §11).
fn problem(
    stream: &mut TcpStream,
    status: u16,
    code: &str,
    title: &str,
    retryable: bool,
    detail: &str,
) -> Result<(), ApiError> {
    let body = serde_json::to_vec(&json!({
        "type": format!("https://aquila.invalid/problems/{code}"),
        "title": title,
        "status": status,
        "code": code,
        "retryable": retryable,
        "detail": detail,
    }))
    .map_err(|error| ApiError::Other(error.to_string()))?;
    write_response(stream, status, "application/problem+json", &body)
}

/// Write the standard `404 not_found` response.
fn not_found(stream: &mut TcpStream) -> Result<(), ApiError> {
    problem(
        stream,
        404,
        "not_found",
        "Resource or route absent",
        false,
        "no such route in the v1 control plane (core-api.md §6)",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(method: &str, query: &str, headers: &[(&str, &str)]) -> Request {
        Request {
            method: method.to_string(),
            path: "/api/v1/state".to_string(),
            query: query.to_string(),
            headers: headers
                .iter()
                .map(|(name, value)| (name.to_ascii_lowercase(), (*value).to_string()))
                .collect(),
            websocket: false,
        }
    }

    #[test]
    fn a_loopback_peer_needs_no_header() {
        let req = request("GET", "", &[]);
        assert!(authorised(&req, true, &Auth::open()));
        assert!(!authorised(&req, false, &Auth::open()));
    }

    #[test]
    fn an_ingress_identity_header_is_accepted_off_loopback() {
        let req = request("GET", "", &[("X-Aquila-Ingress", "hmac")]);
        assert!(authorised(&req, false, &Auth::open()));
        let req = request("GET", "", &[("X-Remote-User-Id", "user")]);
        assert!(authorised(&req, false, &Auth::open()));
    }

    #[test]
    fn a_query_string_token_is_always_rejected() {
        let req = request("GET", "token=leaked", &[]);
        assert!(!authorised(&req, true, &Auth::open()));
        let req = request("GET", "access_token=leaked", &[]);
        assert!(!authorised(&req, true, &Auth::open()));
    }

    #[test]
    fn a_bearer_token_is_checked_against_the_configured_secret() {
        let auth = Auth::new(Some("secret".to_string()));
        let valid = request("GET", "", &[("Authorization", "Bearer secret")]);
        assert!(authorised(&valid, false, &auth));
        let invalid = request("GET", "", &[("Authorization", "Bearer nope")]);
        assert!(!authorised(&invalid, false, &auth));
        // No token configured: a presented bearer is refused rather than trusted.
        assert!(!authorised(&valid, false, &Auth::open()));
    }
}
