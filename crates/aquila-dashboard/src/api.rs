//! The `CoreApi` seam and its error type (`docs/spec/dashboard.md` §3).
//!
//! Screens talk to the core through one trait so the UI can be exercised
//! headless against recorded fixtures (the project's fake-adapter pattern).
//! The trait has just two transport primitives — `get_json` and `send_json` —
//! plus typed convenience methods with default bodies, so a new client
//! (the real browser `HttpCoreApi`, or the test `FakeCoreApi`) implements two
//! methods rather than two dozen.
//!
//! v1 is HTTP polling only: there is no `subscribe` or `EventStream` here,
//! because `GET /api/v1/stream` returns a typed `501` (`core-api.md` §10).
//! The seam is a poll loop, not a socket.
//!
//! # Prose
//!
//! Australian English throughout (serialisation, behaviour, normalise).

use std::fmt;

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::contract::{
    CalibrationSession, Capabilities, Health, Home, HomePatch, List, MetricsSnapshot, NewFloor,
    NewNode, NewPair, NewRoom, Node, NodePair, Room, Schema, Snapshot,
};

/// A failure from the core's control plane (`core-api.md` §11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    /// The request never completed (network, abort, or a `fetch` rejection).
    Network(String),
    /// A response arrived but did not match the contract.
    Decode(String),
    /// A typed `application/problem+json` (or plain HTTP) error response.
    Http {
        /// The HTTP status.
        status: u16,
        /// The stable machine `code`, or `http_<status>` when absent.
        code: String,
        /// The human `detail`.
        detail: String,
    },
}

impl ApiError {
    /// A `404 not_found` (`core-api.md` §11).
    #[must_use]
    pub fn not_found() -> Self {
        Self::Http {
            status: 404,
            code: "not_found".to_string(),
            detail: "no such route in the v1 control plane".to_string(),
        }
    }

    /// A `501 websocket_unimplemented`, which the client treats as "poll
    /// instead", never as a retryable transport fault (`core-api.md` §10).
    #[must_use]
    pub fn not_implemented(what: &str) -> Self {
        Self::Http {
            status: 501,
            code: "not_implemented".to_string(),
            detail: format!("{what} is not implemented in this build"),
        }
    }

    /// A display string safe to show an operator (never a stack trace or a
    /// token, `core-api.md` §15).
    #[must_use]
    pub fn operator_message(&self) -> String {
        match self {
            Self::Network(message) => format!("core unreachable: {message}"),
            Self::Decode(message) => format!("unexpected response: {message}"),
            Self::Http {
                status,
                code,
                detail,
            } => format!("{code} ({status}): {detail}"),
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.operator_message())
    }
}

impl std::error::Error for ApiError {}

/// The client seam for the core's private control plane (`dashboard.md` §3).
///
/// v1 exposes only the read/write routes the dashboard uses; the WebSocket is
/// absent by decision, so all reads are polls.
// The trait is used only in-tree and never as a `dyn` object, so the missing
// `Send` bound on its futures is deliberate; `spawn_local` needs no `Send`.
#[allow(async_fn_in_trait)]
pub trait CoreApi: Clone + 'static {
    /// The single ingress-derived base path (`""` at the origin root). Every
    /// request URL is built from this, never hard-coded (`dashboard.md` §4).
    fn base_path(&self) -> String;

    /// `GET path`, returning the JSON body.
    async fn get_json(&self, path: &str) -> Result<Value, ApiError>;

    /// `POST`/`PATCH`/`DELETE path` with an optional JSON body and an optional
    /// `If-Match` revision (`core-api.md` §5).
    async fn send_json(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        revision: Option<i64>,
    ) -> Result<Value, ApiError>;

    /// `GET /capabilities`.
    async fn capabilities(&self) -> Result<Capabilities, ApiError> {
        decode(self.get_json("/capabilities").await?)
    }

    /// `GET /schema` — the home → floor → room tree.
    async fn schema(&self) -> Result<Schema, ApiError> {
        decode(self.get_json("/schema").await?)
    }

    /// `GET /state` — the coordinator snapshot.
    async fn snapshot(&self) -> Result<Snapshot, ApiError> {
        decode(self.get_json("/state").await?)
    }

    /// `GET /health`.
    async fn health(&self) -> Result<Health, ApiError> {
        decode(self.get_json("/health").await?)
    }

    /// `GET /nodes`.
    async fn nodes(&self) -> Result<Vec<Node>, ApiError> {
        let list: List<Node> = decode(self.get_json("/nodes").await?)?;
        Ok(list.items)
    }

    /// `GET /node-pairs`.
    async fn node_pairs(&self) -> Result<Vec<NodePair>, ApiError> {
        let list: List<NodePair> = decode(self.get_json("/node-pairs").await?)?;
        Ok(list.items)
    }

    /// `GET /metrics/snapshot` — read-only; never triggers measurement.
    async fn metrics_snapshot(&self) -> Result<MetricsSnapshot, ApiError> {
        decode(self.get_json("/metrics/snapshot").await?)
    }

    /// `GET /calibration/current` — the active session, or `None`.
    async fn current_calibration(&self) -> Result<Option<CalibrationSession>, ApiError> {
        let value = self.get_json("/calibration/current").await?;
        decode_optional(value)
    }

    /// `GET /home`.
    async fn home(&self) -> Result<Home, ApiError> {
        decode(self.get_json("/home").await?)
    }

    /// `PATCH /home` with `If-Match`.
    async fn patch_home(&self, patch: &HomePatch, revision: i64) -> Result<Home, ApiError> {
        let body = to_value(patch)?;
        decode(
            self.send_json("PATCH", "/home", Some(body), Some(revision))
                .await?,
        )
    }

    /// `POST /floors` (idempotent by key, `core-api.md` §5).
    async fn create_floor(&self, floor: &NewFloor) -> Result<(), ApiError> {
        let body = to_value(floor)?;
        self.send_json("POST", "/floors", Some(body), None).await?;
        Ok(())
    }

    /// `POST /floors/{floorId}/rooms`.
    async fn create_room(&self, floor_id: &str, room: &NewRoom) -> Result<Room, ApiError> {
        let body = to_value(room)?;
        let path = format!("/floors/{floor_id}/rooms");
        decode(self.send_json("POST", &path, Some(body), None).await?)
    }

    /// `PATCH /rooms/{roomId}`.
    async fn patch_room(&self, id: &str, patch: Value, revision: i64) -> Result<Room, ApiError> {
        let path = format!("/rooms/{id}");
        decode(
            self.send_json("PATCH", &path, Some(patch), Some(revision))
                .await?,
        )
    }

    /// `DELETE /rooms/{roomId}`.
    async fn delete_room(&self, id: &str, revision: i64) -> Result<(), ApiError> {
        let path = format!("/rooms/{id}");
        self.send_json("DELETE", &path, None, Some(revision))
            .await?;
        Ok(())
    }

    /// `POST /nodes` — commission a node into a room.
    async fn register_node(&self, node: &NewNode) -> Result<Node, ApiError> {
        let body = to_value(node)?;
        decode(self.send_json("POST", "/nodes", Some(body), None).await?)
    }

    /// `PATCH /nodes/{nodeId}`.
    async fn patch_node(&self, id: &str, patch: Value, revision: i64) -> Result<Node, ApiError> {
        let path = format!("/nodes/{id}");
        decode(
            self.send_json("PATCH", &path, Some(patch), Some(revision))
                .await?,
        )
    }

    /// `DELETE /nodes/{nodeId}`.
    async fn delete_node(&self, id: &str, revision: i64) -> Result<(), ApiError> {
        let path = format!("/nodes/{id}");
        self.send_json("DELETE", &path, None, Some(revision))
            .await?;
        Ok(())
    }

    /// `POST /node-pairs`.
    async fn create_pair(&self, pair: &NewPair) -> Result<NodePair, ApiError> {
        let body = to_value(pair)?;
        decode(
            self.send_json("POST", "/node-pairs", Some(body), None)
                .await?,
        )
    }

    /// `DELETE /node-pairs/{pairId}`.
    async fn delete_pair(&self, id: &str, revision: i64) -> Result<(), ApiError> {
        let path = format!("/node-pairs/{id}");
        self.send_json("DELETE", &path, None, Some(revision))
            .await?;
        Ok(())
    }

    /// `POST /node-pairs/derive` — suggest pairs from calibration data.
    async fn derive_pairs(&self) -> Result<Vec<NodePair>, ApiError> {
        let value = self
            .send_json("POST", "/node-pairs/derive", None, None)
            .await?;
        let list: List<NodePair> = decode(value)?;
        Ok(list.items)
    }

    /// `POST /calibration` — start a global session (`core-api.md` §8).
    async fn start_calibration(&self) -> Result<CalibrationSession, ApiError> {
        let body = to_value(&serde_json::json!({ "scope": "global" }))?;
        decode(
            self.send_json("POST", "/calibration", Some(body), None)
                .await?,
        )
    }

    /// `POST /calibration/{id}/steps/{stepId}/complete`.
    async fn complete_step(
        &self,
        session_id: &str,
        step_id: &str,
    ) -> Result<CalibrationSession, ApiError> {
        let path = format!("/calibration/{session_id}/steps/{step_id}/complete");
        let body = to_value(&serde_json::json!({ "inputs": {} }))?;
        decode(self.send_json("POST", &path, Some(body), None).await?)
    }

    /// `POST /calibration/{id}/steps/{stepId}/skip`.
    async fn skip_step(
        &self,
        session_id: &str,
        step_id: &str,
    ) -> Result<CalibrationSession, ApiError> {
        let path = format!("/calibration/{session_id}/steps/{step_id}/skip");
        decode(self.send_json("POST", &path, None, None).await?)
    }

    /// `POST /calibration/{id}/commit` — finalise and persist.
    async fn commit_calibration(&self, session_id: &str) -> Result<CalibrationSession, ApiError> {
        let path = format!("/calibration/{session_id}/commit");
        decode(self.send_json("POST", &path, None, None).await?)
    }

    /// `POST /calibration/{id}/abort` — discard the session.
    async fn abort_calibration(&self, session_id: &str) -> Result<CalibrationSession, ApiError> {
        let path = format!("/calibration/{session_id}/abort");
        decode(self.send_json("POST", &path, None, None).await?)
    }
}

/// Decode a JSON body into `T`, mapping serde failures to [`ApiError::Decode`].
fn decode<T: DeserializeOwned>(value: Value) -> Result<T, ApiError> {
    serde_json::from_value(value).map_err(|error| ApiError::Decode(error.to_string()))
}

/// Decode an optional value: JSON `null` becomes `None`.
fn decode_optional<T: DeserializeOwned>(value: Value) -> Result<Option<T>, ApiError> {
    if value.is_null() {
        Ok(None)
    } else {
        decode(value).map(Some)
    }
}

/// Serialise a request body, mapping failures to [`ApiError::Decode`].
fn to_value<T: serde::Serialize>(value: &T) -> Result<Value, ApiError> {
    serde_json::to_value(value).map_err(|error| ApiError::Decode(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_http_error_has_an_operator_message_without_internals() {
        let error = ApiError::Http {
            status: 409,
            code: "node_offline".to_string(),
            detail: "nod_1 has not reported".to_string(),
        };
        let message = error.operator_message();
        assert!(message.contains("node_offline"));
        assert!(message.contains("409"));
    }

    #[test]
    fn null_calibration_decodes_to_none() {
        assert_eq!(
            decode_optional::<CalibrationSession>(Value::Null).expect("null"),
            None
        );
    }
}
