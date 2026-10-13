#![forbid(unsafe_code)]

//! `aquila-simulator` — deterministic, hardware-free CSI synthesis.
//!
//! This crate is level 4 in the workspace dependency graph
//! (`docs/spec/interface-register.md`) and depends on `aquila-types`,
//! `aquila-wire`, `aquila-session` and `aquila-sources`. It owns the
//! deterministic simulator of `docs/spec/capture-and-firmware.md` §7.
//!
//! [`SimulatorSource`] implements the same [`CsiSource`](aquila_sources::CsiSource)
//! seam as replay and live, so the whole pipeline is exercised without hardware.
//! It is a pure function of `(scene script, seed, virtual start time, tick
//! schedule)`: it reads no wall clock, spawns no threads and uses only a seeded
//! PRNG, so same inputs imply byte-identical frames on every run (§7.1). Each
//! frame is encoded through `aquila-wire` and decoded again, so the simulator
//! exercises the real wire codec (§3.2).
//!
//! [`synthesise_session`] drains a simulator into a `.aqs` session and appends a
//! synthesised [`GroundTruthRecord`](aquila_types::GroundTruthRecord) stream
//! (§7.3), so a synthesised Session is self-labelling.
//!
//! # Example
//!
//! ```
//! use aquila_simulator::{synthesise_session, SceneScript, SimConfig};
//! use aquila_types::Slug;
//!
//! let script = SceneScript::demo();
//! let config = SimConfig::new(1, Slug("demo".into()), 0, 0);
//! let session = synthesise_session(&script, &config).unwrap();
//! assert!(session.starts_with(b"AQS"));
//! ```
//!
//! Australian English throughout (serialisation, behaviour, normalise).

mod error;
mod rng;
mod scene;
mod source;

#[cfg(test)]
mod tests;

pub use error::SimError;
pub use scene::{
    Actor, Impairments, Motion, NodeSpec, Rhythm, SceneScript, SPEED_OF_LIGHT_MPS,
};
pub use source::{synthesise_ground_truth, synthesise_session, SimConfig, SimulatorSource};
