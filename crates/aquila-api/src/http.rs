//! A minimal, dependency-light HTTP/1.1 request reader and response writer.
//!
//! This is deliberately **not** an async server and carries no framework: the
//! control plane is loopback-only, small, and speaks one request per
//! connection (`core-api.md` §2). It parses only the request line and whether an
//! upgrade was requested; headers and bodies are not modelled yet.

use std::io::{Read, Write};
use std::net::TcpStream;

use crate::error::ApiError;

/// The largest request head this reader will accept, in bytes.
const MAX_REQUEST_BYTES: usize = 64 * 1024;

/// A parsed request line plus the one header fact the router needs.
#[derive(Debug, Clone)]
pub(crate) struct Request {
    /// The HTTP method, upper-case.
    pub method: String,
    /// The request target, including any query string.
    pub path: String,
    /// Whether `Upgrade: websocket` was present.
    pub websocket: bool,
}

/// Read one request head from `stream`.
///
/// Returns [`ApiError::Http`] on an empty or malformed request or one larger
/// than [`MAX_REQUEST_BYTES`].
pub(crate) fn read_request(stream: &mut TcpStream) -> Result<Request, ApiError> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 2048];
    loop {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if find(&buffer, b"\r\n\r\n").is_some() {
            break;
        }
        if buffer.len() > MAX_REQUEST_BYTES {
            return Err(ApiError::Http("request head too large".to_string()));
        }
    }
    if buffer.is_empty() {
        return Err(ApiError::Http("empty request".to_string()));
    }

    let head = String::from_utf8_lossy(&buffer);
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();
    if method.is_empty() || path.is_empty() {
        return Err(ApiError::Http("malformed request line".to_string()));
    }
    let websocket = head.to_ascii_lowercase().contains("upgrade: websocket");
    Ok(Request {
        method,
        path,
        websocket,
    })
}

/// Write a complete HTTP/1.1 response and close the connection.
pub(crate) fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<(), ApiError> {
    let reason = reason_phrase(status);
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;
    Ok(())
}

/// The standard reason phrase for a status code (only those we emit).
fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        501 => "Not Implemented",
        _ => "Status",
    }
}

/// Find the first occurrence of `needle` in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
