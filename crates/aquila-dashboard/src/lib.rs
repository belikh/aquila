#![forbid(unsafe_code)]

//! `aquila-dashboard` — the Rust→WASM management UI (level 8,
//! `docs/spec/interface-register.md`; owning spec `docs/spec/dashboard.md`).
//!
//! This is a **host-buildable stub**. The real Leptos single-page application,
//! the Trunk bundle and the HTTP/WS client are ticket #10
//! (`docs/spec/dashboard.md`); this crate exists so the interface register and
//! the dependency-DAG allow-list in `tools/dag.toml` match reality and
//! `cargo run -p xtask -- dag-check` can enforce the `dashboard → api → types`
//! edge (`sister-issues-specs.md` D42).
//!
//! The one genuinely useful, testable piece here is the ingress base-path
//! derivation the SPA needs in every build (`dashboard.md` §4): Supervisor
//! serves the panel under a token-based prefix, so every request URL is built
//! from that prefix rather than hard-coded.
//!
//! # Prose
//!
//! Australian English throughout (behaviour, serialisation, normalise).

use aquila_types::CONTRACT_VERSION;

/// Re-exported so the dashboard's transport adapter can name the core's
/// control-plane type without taking a second dependency (`dashboard.md` §3).
pub use aquila_api::Server as CoreServer;

/// The dashboard's display name.
pub const NAME: &str = "Aquila";

/// The API contract major the UI targets (`core-api.md` §4.2).
pub const API_MAJOR: u16 = CONTRACT_VERSION.major;

/// Derive the single `api_base()` prefix from the `X-Ingress-Path` request
/// header (`dashboard.md` §4).
///
/// The ingress prefix is token-based and changes per session, so all HTTP and
/// WS URLs are built relative to it. When the header is absent or empty (local
/// development, no ingress) the app uses the current origin, represented here
/// by the empty string. A well-formed prefix is always normalised to a single
/// leading slash and no trailing slash.
#[must_use]
pub fn ingress_base_path(header: Option<&str>) -> String {
    match header {
        Some(path) if !path.is_empty() => {
            let trimmed = path.trim_end_matches('/');
            if trimmed.starts_with('/') {
                trimmed.to_string()
            } else {
                format!("/{trimmed}")
            }
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_or_empty_header_yields_the_origin_root() {
        assert_eq!(ingress_base_path(None), "");
        assert_eq!(ingress_base_path(Some("")), "");
    }

    #[test]
    fn an_ingress_prefix_is_normalised() {
        assert_eq!(
            ingress_base_path(Some("/api/hassio_ingress/tok_123")),
            "/api/hassio_ingress/tok_123"
        );
        assert_eq!(
            ingress_base_path(Some("/api/hassio_ingress/tok_123/")),
            "/api/hassio_ingress/tok_123"
        );
        assert_eq!(
            ingress_base_path(Some("api/hassio_ingress/tok_123")),
            "/api/hassio_ingress/tok_123"
        );
    }

    #[test]
    fn the_api_major_matches_the_contract_major() {
        assert_eq!(API_MAJOR, 1);
        assert_eq!(NAME, "Aquila");
    }
}
