//! The browser transport: `HttpCoreApi`, the real `CoreApi` over `fetch`
//! (`docs/spec/dashboard.md` §3, §4, §10).
//!
//! Compiled only for `wasm32`. It reads the ingress prefix once from the
//! document `<base href>` (injected by the add-on, `dashboard.md` §4) and builds
//! every request URL from it. v1 polls; it never opens a WebSocket, because
//! `GET /api/v1/stream` is a typed `501` (`core-api.md` §10).
//!
//! # Prose
//!
//! Australian English throughout (serialisation, behaviour, normalise).

use serde_json::Value;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, Request, RequestInit, Response};

use crate::api::{ApiError, CoreApi};

/// The HTTP client of the core's private control plane (`dashboard.md` §3).
#[derive(Clone)]
pub struct HttpCoreApi {
    base: String,
}

impl HttpCoreApi {
    /// Build a client for an explicit base path (`""` at the origin root).
    #[must_use]
    #[allow(dead_code)]
    pub fn new(base: impl Into<String>) -> Self {
        Self { base: base.into() }
    }

    /// Build a client whose base path is derived from the document `<base
    /// href>` (`dashboard.md` §4).
    #[must_use]
    pub fn from_document() -> Self {
        Self {
            base: document_base_path(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/api/v1{}", self.base, path)
    }

    async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        revision: Option<i64>,
    ) -> Result<Value, ApiError> {
        let window = web_sys::window().ok_or_else(|| ApiError::Network("no window".to_string()))?;
        let init = RequestInit::new();
        init.set_method(method);
        if method != "GET" && method != "HEAD" {
            if let Some(value) = &body {
                let text = serde_json::to_string(value)
                    .map_err(|error| ApiError::Decode(error.to_string()))?;
                init.set_body(&JsValue::from_str(&text));
            }
        }
        let headers = Headers::new().map_err(js_error)?;
        headers
            .set("accept", "application/json")
            .map_err(js_error)?;
        if body.is_some() {
            headers
                .set("content-type", "application/json")
                .map_err(js_error)?;
        }
        if let Some(revision) = revision {
            headers
                .set("if-match", &format!("\"{revision}\""))
                .map_err(js_error)?;
        }
        init.set_headers(&headers);

        let request = Request::new_with_str_and_init(&self.url(path), &init).map_err(js_error)?;
        let response_value = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(js_error)?;
        let response: Response = response_value
            .dyn_into()
            .map_err(|_| ApiError::Network("malformed response object".to_string()))?;
        let status = response.status();
        let text = JsFuture::from(response.text().map_err(js_error)?)
            .await
            .map_err(js_error)?;
        let text = text.as_string().unwrap_or_default();

        if !(200..300).contains(&status) {
            return Err(http_error(status, &text));
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).map_err(|error| ApiError::Decode(error.to_string()))
    }
}

impl CoreApi for HttpCoreApi {
    fn base_path(&self) -> String {
        self.base.clone()
    }

    async fn get_json(&self, path: &str) -> Result<Value, ApiError> {
        self.request("GET", path, None, None).await
    }

    async fn send_json(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        revision: Option<i64>,
    ) -> Result<Value, ApiError> {
        self.request(method, path, body, revision).await
    }
}

/// Read the ingress prefix from the document `<base href>`.
fn document_base_path() -> String {
    let href = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.query_selector("base").ok().flatten())
        .and_then(|element| element.get_attribute("href"));
    crate::api_base_from_base_href(href.as_deref())
}

/// Turn a `fetch` rejection into a network error.
fn js_error(value: JsValue) -> ApiError {
    ApiError::Network(format!("{value:?}"))
}

/// Turn a non-2xx response into a typed [`ApiError::Http`], reading the
/// `application/problem+json` `code` and `detail` when present
/// (`core-api.md` §11).
fn http_error(status: u16, body: &str) -> ApiError {
    let parsed: Option<Value> = serde_json::from_str(body).ok();
    let code = parsed
        .as_ref()
        .and_then(|value| value.get("code"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("http_{status}"));
    let detail = parsed
        .as_ref()
        .and_then(|value| value.get("detail"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| "the core returned an error response".to_string());
    ApiError::Http {
        status,
        code,
        detail,
    }
}
