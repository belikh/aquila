#![forbid(unsafe_code)]

//! `aquila-dashboard` — the Rust→WASM management UI (level 8,
//! `docs/spec/interface-register.md`; owning spec `docs/spec/dashboard.md`).
//!
//! A Leptos client-side-rendered single-page application, bundled with Trunk
//! and embedded in the add-on image, served over Home Assistant sidebar ingress
//! (`home-assistant.md` §3.1, §8). It is a **management** surface only: it
//! renders core truth from the canonical `/api/v1` HTTP control plane and sends
//! operator intent. It performs no DSP, inference, fusion, calibration
//! mathematics or sensing-state persistence, and keeps no sensing history
//! (`dashboard.md` §1 non-goals, ticket #10 boundary gate).
//!
//! # Layout
//!
//! - [`contract`] — serde views of the canonical `core-api.md` resources.
//! - [`api`] — the `CoreApi` seam and its error type; screen-portable.
//! - [`router`] — the pure hash-route parser (no `leptos_router` hash router
//!   exists in 0.8, so the app owns a small one).
//! - [`viewmodel`] — pure view-models and form validation; host-testable.
//! - [`fake`] — `FakeCoreApi`, the recorded-fixture client (`dashboard.md` §9).
//!
//! The Leptos components, the browser fetch client and the DOM hash glue live
//! under [`app`] and are compiled **only** for `wasm32`, so the host build is a
//! small dependency-light stub and `cargo test --workspace` never resolves the
//! browser crates (ticket #10 build gate).
//!
//! # Ingress base path
//!
//! Supervisor serves the panel under a token-based prefix and injects it as
//! `<base href>` (and `window.__AQUILA_BASE__`). Every request URL is built
//! from that one value; no absolute path is hard-coded (`dashboard.md` §4).
//!
//! # Prose
//!
//! Australian English throughout (behaviour, serialisation, normalise).

mod api;
mod contract;
mod router;
mod viewmodel;

#[cfg(not(target_arch = "wasm32"))]
mod fake;

#[cfg(target_arch = "wasm32")]
mod app;
#[cfg(target_arch = "wasm32")]
mod net;

pub use api::{ApiError, CoreApi};
pub use contract::*;
#[cfg(not(target_arch = "wasm32"))]
pub use fake::FakeCoreApi;
pub use router::{nav_entries, Route};
pub use viewmodel::*;

/// Re-exported so the dashboard can name the core's control-plane type without
/// a second dependency edge (`dashboard.md` §3; `tools/dag.toml`).
pub use aquila_api::Server as CoreServer;

/// The dashboard's display name.
pub const NAME: &str = "Aquila";

/// The API contract major the UI targets (`core-api.md` §4.2).
pub const API_MAJOR: u16 = aquila_types::CONTRACT_VERSION.major;

/// Derive the single `api_base()` path from the `X-Ingress-Path` request
/// header (`dashboard.md` §4).
///
/// The ingress prefix is token-based and changes per session, so all HTTP URLs
/// are built relative to it. When the header is absent or empty (local
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

/// Derive `api_base()` from the document `<base href>` (`dashboard.md` §4, §8).
///
/// The add-on injects the ingress prefix as `<base href>`; an absolute
/// `https://host/prefix/` value is reduced to its path, then normalised by
/// [`ingress_base_path`]. An absent or unusable value yields `""`, so the SPA
/// makes same-origin requests to `/api/v1/...`.
#[must_use]
pub fn api_base_from_base_href(href: Option<&str>) -> String {
    match href {
        Some(href) if !href.is_empty() => {
            let href = href.trim();
            let path = match href.find("://") {
                Some(index) => {
                    let after_authority = &href[index + 3..];
                    match after_authority.find('/') {
                        Some(slash) => &after_authority[slash..],
                        None => "/",
                    }
                }
                None => href,
            };
            ingress_base_path(Some(path))
        }
        _ => String::new(),
    }
}

/// Mount the Leptos SPA (WASM only; the host build has no DOM).
#[cfg(target_arch = "wasm32")]
pub fn start() {
    app::mount();
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
    fn the_base_href_may_be_a_path_or_an_absolute_url() {
        assert_eq!(
            api_base_from_base_href(Some("/api/hassio_ingress/tok/")),
            "/api/hassio_ingress/tok"
        );
        assert_eq!(
            api_base_from_base_href(Some("http://homeassistant:8123/api/hassio_ingress/tok/")),
            "/api/hassio_ingress/tok"
        );
        assert_eq!(api_base_from_base_href(Some("")), "");
        assert_eq!(api_base_from_base_href(None), "");
    }

    #[test]
    fn the_api_major_matches_the_contract_major() {
        assert_eq!(API_MAJOR, 1);
        assert_eq!(NAME, "Aquila");
    }
}
