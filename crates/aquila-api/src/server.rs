//! The loopback HTTP server and its background guard.
//!
//! [`Server::bind`] refuses any non-loopback address (`core-api.md` §2, §15).
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
use serde_json::json;

use crate::error::ApiError;
use crate::handle::Handle;
use crate::http::{read_request, write_response};

/// How long the non-blocking accept loop pauses between polls.
const ACCEPT_POLL: Duration = Duration::from_millis(2);

/// The private loopback HTTP control plane (`core-api.md`).
pub struct Server {
    listener: TcpListener,
    handle: Handle,
}

impl Server {
    /// Bind a server to `addr` and initialise the projected entity model from
    /// `topology`.
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
        if !addr.ip().is_loopback() {
            return Err(ApiError::NonLoopback(addr));
        }
        let listener = TcpListener::bind(addr)?;
        let handle = Handle::new(EntityModel::from_topology(topology, instance_id));
        Ok(Self { listener, handle })
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
                    let _ = handle_client(&mut stream, &self.handle);
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
        let Self { listener, handle } = self;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        let join = thread::spawn(move || {
            while !stop_thread.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = handle_client(&mut stream, &handle);
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
fn handle_client(stream: &mut TcpStream, handle: &Handle) -> Result<(), ApiError> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let request = read_request(stream)?;
    let path = request.path.split('?').next().unwrap_or("");
    match (request.method.as_str(), path) {
        ("GET", "/api/v1/live") => write_response(
            stream,
            200,
            "application/json; charset=utf-8",
            br#"{"status":"ok"}"#,
        ),
        ("GET", "/api/v1/health") => {
            let body = serde_json::to_vec(&handle.health_json())
                .map_err(|e| ApiError::Other(e.to_string()))?;
            write_response(stream, 200, "application/json; charset=utf-8", &body)
        }
        ("GET", "/api/v1/state") => {
            let body = serde_json::to_vec(&handle.state_json())
                .map_err(|e| ApiError::Other(e.to_string()))?;
            write_response(stream, 200, "application/json; charset=utf-8", &body)
        }
        ("GET", "/api/v1/stream") if request.websocket => {
            let body = serde_json::to_vec(&json!({
                "type": "https://aquila.invalid/problems/websocket_unimplemented",
                "title": "WebSocket upgrade is not implemented",
                "status": 501,
                "code": "websocket_unimplemented",
                "retryable": false,
                "detail": "TODO(core-api): RFC 6455 upgrade and the aquila.v1 stream are pending",
            }))
            .map_err(|e| ApiError::Other(e.to_string()))?;
            write_response(stream, 501, "application/problem+json", &body)
        }
        ("GET", "/api/v1/stream") => {
            let body = serde_json::to_vec(&json!({
                "type": "https://aquila.invalid/problems/validation_failed",
                "title": "WebSocket upgrade required",
                "status": 400,
                "code": "validation_failed",
                "retryable": false,
            }))
            .map_err(|e| ApiError::Other(e.to_string()))?;
            write_response(stream, 400, "application/problem+json", &body)
        }
        _ => {
            let body = serde_json::to_vec(&json!({
                "type": "https://aquila.invalid/problems/not_found",
                "title": "Resource or route absent",
                "status": 404,
                "code": "not_found",
                "retryable": false,
            }))
            .map_err(|e| ApiError::Other(e.to_string()))?;
            write_response(stream, 404, "application/problem+json", &body)
        }
    }
}
