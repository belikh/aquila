//! A minimal, dependency-light HTTP/1.1 request reader and response writer.
//!
//! This is deliberately **not** an async server and carries no framework: the
//! control plane is loopback-only, small, and speaks one request per
//! connection (`core-api.md` §2). It parses the request line, the headers the
//! trust boundary needs, and whether an upgrade was requested; request bodies
//! are not modelled (no v1 route reads one).

use std::io::{Read, Write};
use std::net::TcpStream;

use crate::error::ApiError;

/// The largest request head this reader will accept, in bytes.
const MAX_REQUEST_BYTES: usize = 64 * 1024;

/// A parsed request line plus the header facts the router and the trust
/// boundary need.
#[derive(Debug, Clone, Default)]
pub(crate) struct Request {
    /// The HTTP method, upper-case.
    pub method: String,
    /// The request path, without any query string.
    pub path: String,
    /// The raw query string (no leading `?`), empty when absent.
    pub query: String,
    /// Header name/value pairs, names lower-cased and values trimmed.
    pub headers: Vec<(String, String)>,
    /// The request body, read up to `Content-Length` (empty when absent).
    pub body: Vec<u8>,
    /// Whether `Upgrade: websocket` was present.
    pub websocket: bool,
}

impl Request {
    /// The first value of a header, matched case-insensitively.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Whether the query string carries a token-shaped credential.
    ///
    /// Tokens in a query string leak into logs and referrers and are always
    /// rejected (`core-api.md` §3.2).
    #[must_use]
    pub fn query_has_credential(&self) -> bool {
        self.query.split('&').any(|pair| {
            let key = pair.split('=').next().unwrap_or("").to_ascii_lowercase();
            matches!(
                key.as_str(),
                "token" | "access_token" | "api_key" | "apikey"
            )
        })
    }
}

/// Read one request head from `stream`.
///
/// Returns [`ApiError::Http`] on an empty or malformed request or one larger
/// than [`MAX_REQUEST_BYTES`].
pub(crate) fn read_request(stream: &mut TcpStream) -> Result<Request, ApiError> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 2048];
    let head_end = loop {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break find(&buffer, b"\r\n\r\n");
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(index) = find(&buffer, b"\r\n\r\n") {
            break Some(index + 4);
        }
        if buffer.len() > MAX_REQUEST_BYTES {
            return Err(ApiError::Http("request head too large".to_string()));
        }
    };
    let head_end = match head_end {
        Some(index) => index,
        None if buffer.is_empty() => return Err(ApiError::Http("empty request".to_string())),
        None => return Err(ApiError::Http("malformed request head".to_string())),
    };

    let head = String::from_utf8_lossy(&buffer[..head_end]);
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or_default().to_string();
    if method.is_empty() || target.is_empty() {
        return Err(ApiError::Http("malformed request line".to_string()));
    }

    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (path.to_string(), query.to_string()),
        None => (target, String::new()),
    };

    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
        }
    }
    let websocket = headers
        .iter()
        .any(|(name, value)| name == "upgrade" && value.to_ascii_lowercase().contains("websocket"));

    // Read exactly `Content-Length` body bytes, using any already buffered past
    // the head and then the stream. A body larger than the head budget is
    // refused rather than read unbounded.
    let content_length = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    if content_length > MAX_REQUEST_BYTES {
        return Err(ApiError::Http("request body too large".to_string()));
    }
    let mut body = buffer[head_end..].to_vec();
    while body.len() < content_length {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(content_length);

    Ok(Request {
        method,
        path,
        query,
        headers,
        body,
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
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        409 => "Conflict",
        422 => "Unprocessable Entity",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        503 => "Service Unavailable",
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
