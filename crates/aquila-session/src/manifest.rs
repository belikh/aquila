//! The `.aqs` header metadata: a typed manifest and its canonical-JSON parse.
//!
//! `capture-and-firmware.md` §8.1 places a JSON metadata block after the fixed
//! container header. That metadata records the session's identity and
//! provenance. The canonical *domain* metadata is
//! [`Session`](aquila_types::Session) (`data-contracts.md` §7.1), which is a
//! frozen contract type; this module owns the container-level wrapper that adds
//! the capture-tool, core-version, elicitation and licence facts the spec lists.
//!
//! The JSON is written **canonical**: `serde_json`'s object map is ordered, so
//! serialising through [`serde_json::Value`] yields sorted keys. That keeps a
//! synthesised session byte-identical for identical inputs.

use aquila_types::Session;
use serde::{Deserialize, Serialize};

use crate::error::SessionError;

/// The container schema label written into a manifest.
pub const SCHEMA: &str = "aquila-session/1";

/// How the CSI in the session was elicited.
///
/// v1 nodes are passive observers: a CSI frame exists only when the radio
/// receives ambient 802.11 traffic (`capture-and-firmware.md` §5.3), so the only
/// production value is [`Elicitation::Ambient`]. Dedicated elicitation is
/// research-only, and `Unknown` is the forward-compatible fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Elicitation {
    /// Ambient home traffic; no elicitation waveform.
    Ambient,
    /// An unrecognised or unset elicitation context.
    #[serde(other)]
    Unknown,
}

/// The typed metadata block of a `.aqs` container.
///
/// It wraps the frozen [`Session`] contract with the container-level facts the
/// capture spec requires. The wrapped session carries the identity, node
/// inventory, contract version, seed, config hash and any ground-truth
/// reference; the wrapper carries capture provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// The container schema label, [`SCHEMA`].
    pub schema: String,
    /// Unix wall time the container was created, in milliseconds.
    pub created_unix_ms: i64,
    /// The tool that produced the container, for example `"aquila-simulator"`.
    pub capture_tool: String,
    /// The core version string active at capture time.
    pub core_version: String,
    /// The elicitation context of the recorded CSI.
    pub elicitation: Elicitation,
    /// The licence of the recorded clip, for example `"GPL-3.0-only"`.
    pub licence: String,
    /// The canonical session contract metadata.
    pub session: Session,
}

impl Manifest {
    /// Build a manifest for `session` with the v1 defaults.
    ///
    /// `created_unix_ms` is taken from the session's start, so a caller that
    /// pins the session's times gets a deterministic manifest. The capture tool
    /// defaults to `"aquila-simulator"`; `aquila-capture record` would override
    /// it. The licence is `GPL-3.0-only` and the elicitation is ambient.
    #[must_use]
    pub fn new(session: Session) -> Self {
        Self {
            schema: SCHEMA.to_string(),
            created_unix_ms: session.started_unix_ms,
            capture_tool: "aquila-simulator".to_string(),
            core_version: env!("CARGO_PKG_VERSION").to_string(),
            elicitation: Elicitation::Ambient,
            licence: "GPL-3.0-only".to_string(),
            session,
        }
    }

    /// Override the capture tool recorded in the manifest.
    #[must_use]
    pub fn with_capture_tool(mut self, tool: impl Into<String>) -> Self {
        self.capture_tool = tool.into();
        self
    }

    /// Validate the container-level fields and the wrapped session.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Manifest`] if a text field is empty or the
    /// wrapped [`Session`] fails its contract invariants.
    pub fn validate(&self) -> Result<(), SessionError> {
        if self.schema.is_empty() {
            return Err(SessionError::Manifest("schema must be non-empty".into()));
        }
        if self.capture_tool.is_empty() {
            return Err(SessionError::Manifest(
                "capture_tool must be non-empty".into(),
            ));
        }
        if self.core_version.is_empty() {
            return Err(SessionError::Manifest(
                "core_version must be non-empty".into(),
            ));
        }
        if self.licence.is_empty() {
            return Err(SessionError::Manifest("licence must be non-empty".into()));
        }
        self.session
            .validate()
            .map_err(|err| SessionError::Manifest(err.to_string()))
    }

    /// Serialise the manifest as canonical JSON (sorted keys).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Json`] if the manifest cannot be serialised.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, SessionError> {
        let value = serde_json::to_value(self)?;
        Ok(serde_json::to_vec(&value)?)
    }
}

/// Parse a manifest from canonical JSON bytes.
///
/// # Errors
///
/// Returns [`SessionError::Json`] if the bytes are not a valid manifest.
pub fn parse_manifest(json: &[u8]) -> Result<Manifest, SessionError> {
    Ok(serde_json::from_slice(json)?)
}
