#![forbid(unsafe_code)]

//! `aquila-sensing` — the single stateful sensing core (level 5,
//! `docs/spec/interface-register.md`).
//!
//! Everything the pure crates cannot know lives here: a common time base and
//! per-node drift, whole-home calibration, per-node sliding windows and arrival
//! jitter, feature-level fusion across nodes and pairs, and the staged
//! room → floor → cross-floor escalation (`architecture.md` §4). The public
//! interface is deliberately tiny — construct with adapters,
//! [`Sensing::step`] one window, [`Sensing::calibrate`] from a sample set,
//! [`Sensing::sink`] the output.
//!
//! # Seams
//!
//! The core owns two seams:
//!
//! - [`Clock`](aquila_types::Clock) — the injectable source of "now". The trait
//!   lives in `aquila-types` (ADR-0003 §5); [`SystemClock`] and [`ManualClock`]
//!   are the adapters here.
//! - [`Sink`] — the one write path out of the core for
//!   [`Event`](aquila_types::Event)s.
//!
//! It consumes the `CsiSource` seam from `aquila-sources`, the
//! `PoseModel`/`VitalsModel` seams from `aquila-inference` (optional in slice 1)
//! and the `MetricSink` seam from `aquila-metrics`.
//!
//! # Internal modules
//!
//! `time`, `window`, `fusion`, `escalation`, `calibration`, `features` and
//! `bridge` are private and are exercised by the crate's own tests through this
//! interface. Slice 1 emits room-scope (zone) presence and count; floor and
//! cross-floor escalation are present but minimal.
//!
//! # Determinism
//!
//! Fusion and window assembly are deterministic functions of the observed
//! frames and the injected clock: same inputs imply identical events. No wall
//! clock is read except through the `Clock` seam.
//!
//! # Prose
//!
//! Australian English throughout (serialisation, behaviour, normalise).

mod bridge;
mod calibration;
mod clock;
mod config;
mod engine;
mod error;
mod escalation;
mod features;
mod fusion;
mod pose;
mod sink;
mod time;
mod window;

#[cfg(test)]
mod tests;

pub use calibration::{
    calibrate, topology_digest, CalibrationArtifact, CalibrationExpectation, CalibrationIndex,
    CalibrationIndexEntry, CalibrationInputs, CalibrationQuality, CalibrationReport,
    CalibrationRequest, CalibrationStore, Gate, QualityMetrics, QualityStatus, ZoneCalibration,
    ZoneDetection, CALIBRATION_SCHEMA_MAJOR, CALIBRATION_SCHEMA_VERSION,
    CALIBRATION_SEMANTICS_VERSION, DEFAULT_STALE_AFTER_DAYS,
};
pub use clock::{ManualClock, SystemClock};
pub use config::{MeasurementConfig, SensingConfig};
pub use engine::{FloorWindow, HomeWindow, Sensing, StepReport};
pub use error::{CalibrationError, InvalidationReason, SensingError, SinkError};
pub use escalation::{
    aggregate_floor, aggregate_floor_vitals, aggregate_home, convolve, floor_presence_probability,
    home_presence_probability, Conflict, FloorSummary, FloorVitals, HomeLocation, HomeSummary,
    RoomCoverage, RoomSummary, RoomVitals,
};
pub use fusion::{
    clamp_logit, fuse_torso_ranges, mix_count_pmf, node_count_pmf, node_fusion_weight,
    pool_presence_logit, pooled_presence_probability, sigmoid, CountPmf, NodeEvidence,
    NodeTorsoRanges, TorsoEstimate, TorsoGeometry, TORSO_KEYPOINT_SCATTERS,
};
pub use pose::{
    estimate_room_pose, extract_torso_ranges, DitherSearch, DspPoseModel, PoseNodeInput,
    DEFAULT_DITHER_SEARCH_STEP_HZ,
};
pub use sink::Sink;
pub use time::{NodeClockFit, TimeSync};
pub use window::{WindowAssembler, WindowBatch};

/// The indexed geometry topology the core queries for attribution.
pub use aquila_geometry::Topology;

// The model seam is re-exported so a test or composition root can build a fake
// against the exact trait the core consumes without a second dependency edge.
pub use aquila_inference::{
    FeatureVector, ModelError, ModelId, PoseModel, PoseOutcome, PoseRequest, VitalsModel,
    VitalsRequest,
};
