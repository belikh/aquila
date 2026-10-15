//! Static serving of the WASM dashboard bundle alongside the control plane.
//!
//! The add-on bakes the Trunk build of `aquila-dashboard` into
//! `AQUILA_DASHBOARD_DIR` (`/usr/share/aquila/dashboard/`) and serves it from
//! the same listener as `/api/v1` (`dashboard.md` §2, §4). Home Assistant
//! ingress proxies the panel under a per-session token
//! (`/api/hassio_ingress/<token>/`) and announces that prefix with the
//! `X-Ingress-Path` header, so:
//!
//! - every request path has the ingress prefix stripped before routing, and
//! - `index.html` is served with an injected `<base href="<prefix>/">`, which is
//!   exactly the base the SPA derives its `api_base()` from (`dashboard.md` §4).
//!
//! Trunk emits every asset reference relative (`./…`), so injecting the base is
//! sufficient for the bundle to load under any ingress token or at the origin
//! root. No absolute asset URL is rewritten.
//!
//! # Prose
//!
//! Australian English throughout (serialisation, behaviour, normalise).

use std::net::TcpStream;
use std::path::{Path, PathBuf};

use crate::error::ApiError;
use crate::http::write_response_with_headers;

/// The environment variable naming the baked dashboard bundle directory.
pub const DASHBOARD_DIR_ENV: &str = "AQUILA_DASHBOARD_DIR";

/// A directory of pre-built static dashboard assets.
#[derive(Debug, Clone)]
pub struct StaticAssets {
    root: PathBuf,
}

impl StaticAssets {
    /// Wrap an explicit asset directory.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Read the asset directory from [`DASHBOARD_DIR_ENV`], returning `None`
    /// when the variable is unset or names a directory that does not exist.
    ///
    /// A missing bundle is not an error: the control plane still serves
    /// `/api/v1`, and the absence is reported at startup by the composition
    /// root.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        let root = std::env::var_os(DASHBOARD_DIR_ENV)?;
        let root = PathBuf::from(root);
        root.is_dir().then_some(Self { root })
    }

    /// The asset root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve a request path (already stripped of any ingress prefix) to the
    /// file that serves it, or `None` when it is absent or escapes the root.
    ///
    /// A trailing `/` or the empty path maps to `index.html`. Path segments
    /// `.` and `..` are rejected outright rather than normalised, so a request
    /// can never address a file outside the bundle.
    fn resolve(&self, request_path: &str) -> Option<PathBuf> {
        let relative = request_path.trim_start_matches('/');
        if relative
            .split('/')
            .any(|segment| segment == "." || segment == "..")
        {
            return None;
        }
        let relative = if relative.is_empty() || relative.ends_with('/') {
            format!("{relative}index.html")
        } else {
            relative.to_string()
        };
        let candidate = self.root.join(&relative);
        if candidate.is_file() {
            Some(candidate)
        } else {
            None
        }
    }

    /// Serve one static request, honouring the ingress prefix.
    ///
    /// `ingress_prefix` is the raw `X-Ingress-Path` header value (without a
    /// trailing slash), used only to inject `<base href>` into `index.html`.
    ///
    /// # Errors
    ///
    /// Returns [`ApiError::Io`] if the file cannot be read or the response
    /// cannot be written.
    pub(crate) fn serve(
        &self,
        stream: &mut TcpStream,
        request_path: &str,
        ingress_prefix: Option<&str>,
    ) -> Result<(), ApiError> {
        let Some(file) = self.resolve(request_path) else {
            return not_found(stream);
        };
        let body = std::fs::read(&file)?;
        let content_type = content_type_for(&file);
        // The index document carries the injected ingress base; every other
        // asset is served verbatim. The bundle is content-hashed by Trunk, so it
        // is safe to cache aggressively except for the document itself.
        if file.file_name().is_some_and(|name| name == "index.html") {
            let body = inject_base_href(&body, ingress_prefix);
            write_response_with_headers(
                stream,
                200,
                content_type,
                &[("Cache-Control", "no-cache")],
                &body,
            )
        } else {
            write_response_with_headers(
                stream,
                200,
                content_type,
                &[("Cache-Control", "public, max-age=31536000, immutable")],
                &body,
            )
        }
    }
}

/// Strip `prefix` (the `X-Ingress-Path`) from `path`, yielding a root-relative
/// request path.
///
/// Handles both ingress shapes: a proxy that forwards the full prefixed path
/// (stripped here) and one that has already removed the prefix (left as-is). A
/// prefix that does not match is a no-op.
#[must_use]
pub(crate) fn strip_ingress_prefix(path: &str, prefix: Option<&str>) -> String {
    let Some(prefix) = prefix else {
        return path.to_string();
    };
    let prefix = prefix.trim_end_matches('/');
    if prefix.is_empty() {
        return path.to_string();
    }
    if path == prefix {
        return "/".to_string();
    }
    match path.strip_prefix(prefix) {
        Some(rest) if rest.starts_with('/') => rest.to_string(),
        _ => path.to_string(),
    }
}

/// Inject `<base href="<prefix>/">` immediately after the opening `<head>` of a
/// dashboard document, or return the bytes unchanged when there is no prefix or
/// no `<head>`.
///
/// The injected base is what makes Trunk's relative `./…` asset references
/// resolve under the per-session ingress token, and it is the value the SPA
/// reads back for `api_base()` (`dashboard.md` §4).
fn inject_base_href(body: &[u8], ingress_prefix: Option<&str>) -> Vec<u8> {
    let Some(prefix) = ingress_prefix.map(|prefix| prefix.trim_end_matches('/')) else {
        return body.to_vec();
    };
    if prefix.is_empty() {
        return body.to_vec();
    }
    let Ok(text) = std::str::from_utf8(body) else {
        return body.to_vec();
    };
    let Some(head) = text.find("<head>") else {
        return body.to_vec();
    };
    let insert_at = head + "<head>".len();
    let tag = format!("\n    <base href=\"{prefix}/\" />");
    let mut injected = String::with_capacity(text.len() + tag.len());
    injected.push_str(&text[..insert_at]);
    injected.push_str(&tag);
    injected.push_str(&text[insert_at..]);
    injected.into_bytes()
}

/// The response content type for an asset, by lowercase extension.
fn content_type_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("json" | "map") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Write the standard `404 not_found` problem for a missing asset.
fn not_found(stream: &mut TcpStream) -> Result<(), ApiError> {
    let body = r#"{"type":"https://aquila.invalid/problems/not_found","title":"Resource or route absent","status":404,"code":"not_found","retryable":false,"detail":"no such static asset (dashboard.md section 4)"}"#;
    write_response_with_headers(
        stream,
        404,
        "application/problem+json",
        &[],
        body.as_bytes(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_matching_prefix_is_stripped() {
        assert_eq!(
            strip_ingress_prefix("/api/hassio_ingress/tok/api/v1/state", Some("/api/hassio_ingress/tok")),
            "/api/v1/state"
        );
        assert_eq!(
            strip_ingress_prefix("/api/hassio_ingress/tok/", Some("/api/hassio_ingress/tok/")),
            "/"
        );
    }

    #[test]
    fn an_absent_or_non_matching_prefix_is_a_no_op() {
        assert_eq!(strip_ingress_prefix("/api/v1/state", None), "/api/v1/state");
        assert_eq!(
            strip_ingress_prefix("/api/v1/state", Some("/other")),
            "/api/v1/state"
        );
    }

    #[test]
    fn the_base_href_is_injected_after_head() {
        let html = b"<!DOCTYPE html><html><head><title>x</title></head><body></body></html>";
        let out = String::from_utf8(inject_base_href(html, Some("/api/hassio_ingress/tok"))).unwrap();
        assert!(out.contains("<head>\n    <base href=\"/api/hassio_ingress/tok/\" />"));
        // No prefix: unchanged.
        assert_eq!(inject_base_href(html, None), html);
    }

    #[test]
    fn path_traversal_is_refused() {
        let assets = StaticAssets::new(".");
        assert!(assets.resolve("/../Cargo.toml").is_none());
        assert!(assets.resolve("/./Cargo.toml").is_none());
    }
}
