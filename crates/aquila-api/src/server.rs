//! The HTTP server and its background guard.
//!
//! [`Server::bind`] refuses any non-loopback address (`core-api.md` §2, §15);
//! [`Server::bind_with_config`] with [`ServerConfig::any_interface`] is the
//! add-on path for a container address behind Home Assistant ingress. It serves
//! the canonical v1 routes (`core-api.md` §6) and, when configured, the static
//! dashboard bundle. Every request is
//! authenticated by the private-network/ingress trust boundary (§3): a
//! loopback or private (RFC 1918 / link-local / IPv6 ULA) peer, the add-on
//! ingress identity header, or a valid bearer token; a token in a query string
//! is always rejected. The WebSocket stream is not implemented in v1:
//! `GET /api/v1/stream` returns a typed `501`, so clients poll
//! (`core-api.md` §10).
//!
//! [`Server::serve_in_background`] runs the accept loop on a thread and returns
//! a [`ServerGuard`] that stops it on drop, so a composition root can start the
//! control plane, wire it to the sensing core, and shut it down cleanly.

use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use aquila_geometry::Topology;
use aquila_ha::EntityModel;
use serde_json::{json, Value};

use crate::assets::{strip_ingress_prefix, StaticAssets};
use crate::error::ApiError;
use crate::handle::Handle;
use crate::http::{read_request, write_response, Request};
use crate::ota::InstallError;
use crate::projection;

/// How long the non-blocking accept loop pauses between polls.
const ACCEPT_POLL: Duration = Duration::from_millis(2);

/// The control plane's authentication policy (`core-api.md` §3).
///
/// The trust boundary is the add-on's private network seam: the port is not
/// published to the LAN, so only the HA core container (which shares the
/// `hassio` Docker network) and co-located processes can reach it. A request is
/// accepted when a loopback or private-network peer connects, when the add-on
/// ingress identity header is present, or when a valid bearer token is
/// presented. There is no anonymous mode on any other connection except the
/// `/live` probe, and a token in a query string is always rejected.
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

/// How a [`Server`] is bound and what it serves beyond `/api/v1`.
///
/// The default is a loopback-only control plane with no static assets, so
/// [`Server::bind`] keeps the v1 posture (`core-api.md` §2). The add-on builds a
/// config with [`any_interface`](ServerConfig::any_interface) and
/// [`assets`](ServerConfig::assets) so it is reachable through Home Assistant
/// ingress and also serves the dashboard bundle.
#[derive(Clone, Default)]
pub struct ServerConfig {
    /// The trust policy (`core-api.md` §3).
    pub auth: Auth,
    /// The optional dashboard bundle to serve from the same listener.
    pub assets: Option<StaticAssets>,
    /// Whether a non-loopback bind is permitted. `false` (the default) preserves
    /// the loopback-only posture.
    pub any_interface: bool,
}

impl ServerConfig {
    /// The default loopback-only config with no static assets.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the trust policy.
    #[must_use]
    pub fn with_auth(mut self, auth: Auth) -> Self {
        self.auth = auth;
        self
    }

    /// Set the optional static dashboard bundle.
    #[must_use]
    pub fn with_assets(mut self, assets: Option<StaticAssets>) -> Self {
        self.assets = assets;
        self
    }

    /// Permit binding a non-loopback address (the container address).
    #[must_use]
    pub fn any_interface(mut self) -> Self {
        self.any_interface = true;
        self
    }
}

/// The private HTTP control plane (`core-api.md`).
pub struct Server {
    listener: TcpListener,
    handle: Handle,
    auth: Auth,
    assets: Option<StaticAssets>,
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
        Self::bind_with_config(
            addr,
            topology,
            instance_id,
            ServerConfig::new().with_auth(auth),
        )
    }

    /// Bind a server with a full [`ServerConfig`].
    ///
    /// Unlike [`bind`](Server::bind), this permits a non-loopback address when
    /// [`ServerConfig::any_interface`] is set, which is how the add-on serves the
    /// container address behind Home Assistant ingress.
    ///
    /// # Errors
    ///
    /// Returns [`ApiError::NonLoopback`] if `addr` is not loopback and the config
    /// does not permit any interface, or [`ApiError::Io`] if the socket cannot be
    /// bound.
    pub fn bind_with_config(
        addr: SocketAddr,
        topology: &Topology,
        instance_id: impl Into<String>,
        config: ServerConfig,
    ) -> Result<Self, ApiError> {
        if !config.any_interface && !addr.ip().is_loopback() {
            return Err(ApiError::NonLoopback(addr));
        }
        let listener = TcpListener::bind(addr)?;
        let instance_id = instance_id.into();
        let projections = projection::build(topology, &instance_id);
        let handle = Handle::new(
            instance_id.clone(),
            EntityModel::from_topology(topology, instance_id),
            projections,
        );
        Ok(Self {
            listener,
            handle,
            auth: config.auth,
            assets: config.assets,
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
                    let _ =
                        handle_client(&mut stream, &self.handle, &self.auth, self.assets.as_ref());
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
            assets,
        } = self;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        let join = thread::spawn(move || {
            while !stop_thread.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = handle_client(&mut stream, &handle, &auth, assets.as_ref());
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
///
/// The request path has any Home Assistant ingress prefix (`X-Ingress-Path`)
/// stripped before routing, so the canonical `/api/v1/...` routes and the
/// dashboard bundle both resolve under the per-session ingress token
/// (`dashboard.md` §4). Static assets are served for any non-`/api` GET;
/// `/api` routes keep the [`Auth`] trust boundary.
fn handle_client(
    stream: &mut TcpStream,
    handle: &Handle,
    auth: &Auth,
    assets: Option<&StaticAssets>,
) -> Result<(), ApiError> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let request = read_request(stream)?;
    let ingress_prefix = request.header("x-ingress-path").map(str::to_string);
    let path = strip_ingress_prefix(&request.path, ingress_prefix.as_deref());
    let segments = segments(&path);
    let is_api = segments.first() == Some(&"api");

    // The liveness probe is the one anonymous route (`core-api.md` §4).
    if request.method == "GET" && segments == ["api", "v1", "live"] {
        return write_json(stream, 200, &json!({ "status": "ok" }));
    }

    if !is_api {
        // The dashboard bundle is public static content; the sensitive surface
        // (`/api/v1`) remains behind the trust boundary below.
        if matches!(request.method.as_str(), "GET" | "HEAD") {
            if let Some(assets) = assets {
                return assets.serve(stream, &path, ingress_prefix.as_deref());
            }
        }
        return not_found(stream);
    }

    let peer = stream.peer_addr().ok().map(|addr| addr.ip());
    if !authorised(&request, peer, auth) {
        return problem(
            stream,
            401,
            "unauthenticated",
            "Missing or invalid credentials",
            false,
            "the control plane requires a trusted (loopback or private) peer, an ingress identity header or a bearer token",
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
            match handle.node_firmware_job_json(node) {
                Some(body) => write_json(stream, 200, &body),
                None => not_found(stream),
            }
        }
        ("POST", ["api", "v1", "nodes", node, "firmware", "install"]) => {
            install_firmware(stream, request, node, handle)
        }
        ("POST", ["api", "v1", "nodes", node, "firmware", "abort"]) => {
            if handle.node_firmware_json(node).is_none() {
                return not_found(stream);
            }
            let aborted = handle.ota().abort(node);
            write_json(
                stream,
                200,
                &json!({
                    "node": node.to_ascii_lowercase(),
                    "state": if aborted { "aborted" } else { "idle" },
                }),
            )
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

/// Begin a firmware OTA (`core-api.md` §6, `firmware-ota.md` §7).
fn install_firmware(
    stream: &mut TcpStream,
    request: &Request,
    node: &str,
    handle: &Handle,
) -> Result<(), ApiError> {
    let target = match handle.node_target(node) {
        Some(target) => target,
        None => return not_found(stream),
    };

    // Capability first: an unconfigured core cannot run OTA at all, whatever
    // body arrives, and says so honestly (`core-api.md` §6).
    if !handle.ota().has_store() {
        return problem(
            stream,
            503,
            "ota_unavailable",
            "Firmware OTA is not configured in this build",
            true,
            "no signed image store or node transport is wired to the control plane",
        );
    }

    let body: Value = if request.body.is_empty() {
        json!({})
    } else {
        match serde_json::from_slice(&request.body) {
            Ok(value) => value,
            Err(_) => {
                return problem(
                    stream,
                    422,
                    "validation_failed",
                    "Request body is not valid JSON",
                    false,
                    "POST .../firmware/install expects { image_id, force? }",
                )
            }
        }
    };
    let image_id = body
        .get("image_id")
        .or_else(|| body.get("imageId"))
        .and_then(Value::as_str);
    let image_id = match image_id {
        Some(id) if !id.is_empty() => id,
        _ => {
            return problem(
                stream,
                422,
                "validation_failed",
                "image_id is required",
                false,
                "POST .../firmware/install expects { image_id, force? }",
            )
        }
    };
    let force = body.get("force").and_then(Value::as_bool).unwrap_or(false);

    match handle.ota().install(node, &target, image_id, force) {
        Ok(job) => write_json(
            stream,
            202,
            &json!({
                "jobId": job.job_id,
                "node": job.node,
                "imageId": job.image_id,
                "state": job.state.as_str(),
            }),
        ),
        Err(InstallError::Unavailable) => problem(
            stream,
            503,
            "ota_unavailable",
            "Firmware OTA is not configured in this build",
            true,
            "no signed image store or node transport is wired to the control plane",
        ),
        Err(InstallError::ImageNotFound(id)) => problem(
            stream,
            404,
            "not_found",
            "Firmware image is not in the store",
            false,
            &format!("no stored image with id {id}"),
        ),
        Err(InstallError::NodeOffline(node)) => problem(
            stream,
            409,
            "node_offline",
            "Command cannot reach an offline node",
            true,
            &format!("no control transport is registered for node {node}"),
        ),
        Err(InstallError::Store(error)) => problem(
            stream,
            422,
            "validation_failed",
            "Firmware image was rejected",
            false,
            &error.to_string(),
        ),
        Err(InstallError::Transfer(error)) => problem(
            stream,
            503,
            "degraded",
            "Firmware transfer did not complete",
            true,
            &error.to_string(),
        ),
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
            "firmwareOta": true,
            "polling": true
        },
        "minComponentVersion": crate::handle::MIN_COMPONENT_VERSION
    })
}

/// Evaluate the trust policy (`core-api.md` §3).
///
/// Order matters: a credential-shaped query parameter is rejected before any
/// other mode so it can never be smuggled in alongside one.
fn authorised(request: &Request, peer: Option<IpAddr>, auth: &Auth) -> bool {
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
    if request.header("x-aquila-ingress").is_some()
        || request.header("x-remote-user-id").is_some()
        // Home Assistant ingress sets `X-Ingress-Path` on every proxied request;
        // its presence identifies the supervisor's ingress gateway, so the
        // dashboard's same-origin `/api/v1` calls are trusted without a bearer
        // token (`home-assistant.md` §5.2).
        || request.header("x-ingress-path").is_some()
    {
        return true;
    }
    peer.map(peer_is_trusted).unwrap_or(false)
}

/// Whether a peer address lies inside the control plane's trust boundary
/// (`core-api.md` §3.1).
///
/// The boundary is the add-on's **private network plus the unpublished port**,
/// not loopback alone: the Home Assistant core container reaches `8099` over the
/// `hassio` Docker network, whose addresses are RFC 1918 private, and neither it
/// nor any other process can arrive from the LAN because the port is not
/// published. A peer is therefore trusted when it is loopback, RFC 1918 private
/// (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`), link-local
/// (`169.254.0.0/16`), IPv6 unique-local (`fc00::/7`) or IPv6 link-local
/// (`fe80::/10`). A public peer is not trusted and must present the ingress
/// identity header or a bearer token.
///
/// This is a plain classification of the peer address, not a confidentiality
/// claim: the private network is not encrypted, and the bearer token is the only
/// mode that authenticates past the network placement itself.
fn peer_is_trusted(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local(),
    }
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
            body: Vec::new(),
            websocket: false,
        }
    }

    #[test]
    fn a_loopback_peer_needs_no_header() {
        let req = request("GET", "", &[]);
        assert!(authorised(
            &req,
            Some("127.0.0.1".parse().unwrap()),
            &Auth::open()
        ));
        assert!(authorised(
            &req,
            Some("::1".parse().unwrap()),
            &Auth::open()
        ));
    }

    #[test]
    fn a_private_network_peer_needs_no_header() {
        // The Home Assistant core container reaches the add-on at
        // `http://aquila-core:8099` over the `hassio` Docker network, so its peer
        // address is private, not loopback.
        let req = request("GET", "", &[]);
        for ip in ["10.0.0.5", "172.17.0.2", "192.168.1.20", "169.254.10.10"] {
            let peer: IpAddr = ip.parse().unwrap();
            assert!(authorised(&req, Some(peer), &Auth::open()), "{ip}");
        }
    }

    #[test]
    fn a_public_peer_without_a_credential_is_rejected() {
        let req = request("GET", "", &[]);
        for ip in ["8.8.8.8", "1.1.1.1", "2606:4700:4700::1111"] {
            let peer: IpAddr = ip.parse().unwrap();
            assert!(!authorised(&req, Some(peer), &Auth::open()), "{ip}");
        }
        // No peer address at all (a failed `peer_addr`) is not trusted.
        assert!(!authorised(&req, None, &Auth::open()));
    }

    #[test]
    fn peer_trust_covers_every_private_and_loopback_range() {
        let trusted = [
            // Loopback.
            "127.0.0.1",
            "127.8.8.8",
            "::1",
            // RFC 1918.
            "10.0.0.1",
            "10.255.255.254",
            "172.16.0.1",
            "172.31.255.254",
            "192.168.0.1",
            "192.168.255.254",
            // Link-local (IPv4 and IPv6).
            "169.254.10.10",
            "fe80::1",
            // IPv6 unique-local.
            "fc00::1",
            "fd12:3456:789a::1",
        ];
        for ip in trusted {
            let ip: IpAddr = ip.parse().unwrap();
            assert!(peer_is_trusted(ip), "{ip} must be trusted");
        }

        let rejected = [
            // Public IPv4.
            "8.8.8.8",
            "1.1.1.1",
            "203.0.113.1",
            // Just outside each private range.
            "172.15.0.1",
            "172.32.0.1",
            "192.169.0.1",
            "169.255.0.1",
            // Carrier-grade NAT is not RFC 1918.
            "100.64.0.1",
            // Public IPv6 (documentation prefix and a global anycast address).
            "2001:db8::1",
            "2606:4700:4700::1111",
            // Just below the IPv6 link-local prefix.
            "fe00::1",
        ];
        for ip in rejected {
            let ip: IpAddr = ip.parse().unwrap();
            assert!(!peer_is_trusted(ip), "{ip} must not be trusted");
        }
    }

    #[test]
    fn an_ingress_identity_header_is_accepted_off_loopback() {
        let public: IpAddr = "8.8.8.8".parse().unwrap();
        let req = request("GET", "", &[("X-Aquila-Ingress", "hmac")]);
        assert!(authorised(&req, Some(public), &Auth::open()));
        let req = request("GET", "", &[("X-Remote-User-Id", "user")]);
        assert!(authorised(&req, Some(public), &Auth::open()));
    }

    #[test]
    fn a_query_string_token_is_always_rejected() {
        let peer: IpAddr = "127.0.0.1".parse().unwrap();
        let req = request("GET", "token=leaked", &[]);
        assert!(!authorised(&req, Some(peer), &Auth::open()));
        let req = request("GET", "access_token=leaked", &[]);
        assert!(!authorised(&req, Some(peer), &Auth::open()));
    }

    #[test]
    fn a_bearer_token_is_checked_against_the_configured_secret() {
        let auth = Auth::new(Some("secret".to_string()));
        let public: IpAddr = "8.8.8.8".parse().unwrap();
        let valid = request("GET", "", &[("Authorization", "Bearer secret")]);
        assert!(authorised(&valid, Some(public), &auth));
        let invalid = request("GET", "", &[("Authorization", "Bearer nope")]);
        assert!(!authorised(&invalid, Some(public), &auth));
        // No token configured: a presented bearer is refused rather than trusted.
        assert!(!authorised(&valid, Some(public), &Auth::open()));
    }
}
