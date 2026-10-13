//! Fixture loading: a typed, self-contained deterministic scene.
//!
//! A [`Fixture`] bundles everything a test or evaluation needs from one
//! deterministic scene: the synthesised session bytes, the decoded frames and
//! the ground-truth stream. [`Fixture::replay_source`] materialises the session
//! to a private temporary file and opens it through [`ReplaySource`], so replay
//! exercises the real `aquila-wire` codec and the file is removed immediately
//! after the reader has parsed it.
//!
//! Fixtures are produced, never edited: a change to the generator is a new
//! scene, not an in-place edit (`testing.md` §4).

use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use aquila_sources::{ReplayMode, ReplaySource, SourceError};
use aquila_types::{Clock, CsiEnvelope, GroundTruthRecord, Pose};

use crate::generator::DeterministicCsiGenerator;

/// The failure surface of fixture loading.
#[derive(Debug)]
pub enum FixtureError {
    /// The scene failed validation or synthesis in the simulator.
    Sim(String),
    /// The session could not be read back through the replay adapter.
    Source(SourceError),
    /// The temporary session file could not be written.
    Io(std::io::Error),
}

impl fmt::Display for FixtureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sim(reason) => write!(f, "simulator error: {reason}"),
            Self::Source(error) => write!(f, "replay source error: {error}"),
            Self::Io(error) => write!(f, "fixture I/O error: {error}"),
        }
    }
}

impl std::error::Error for FixtureError {}

impl From<SourceError> for FixtureError {
    fn from(error: SourceError) -> Self {
        Self::Source(error)
    }
}

impl From<std::io::Error> for FixtureError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// A loaded deterministic fixture.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// The scene name.
    pub name: String,
    /// The exact `.aqs` session bytes.
    pub session: Vec<u8>,
    /// The decoded CSI envelopes, in stream order.
    pub frames: Vec<CsiEnvelope>,
    /// The synthesised ground-truth records.
    pub ground_truth: Vec<GroundTruthRecord>,
    /// The synthesised coarse-pose ground truth, when the scene carries bodies.
    pub pose_truth: Vec<Pose>,
}

impl Fixture {
    /// Load a fixture from a generator.
    ///
    /// # Errors
    ///
    /// Returns [`FixtureError`] if synthesis or the direct frame drain fails.
    pub fn load(generator: &DeterministicCsiGenerator) -> Result<Self, FixtureError> {
        let session = generator
            .session_bytes()
            .map_err(|error| FixtureError::Sim(error.to_string()))?;
        let frames = generator.frames()?;
        let ground_truth = generator
            .ground_truth()
            .into_iter()
            .map(|(_, record)| record)
            .collect();
        Ok(Self {
            name: generator.script().name.clone(),
            session,
            frames,
            ground_truth,
            pose_truth: generator.pose_ground_truth(),
        })
    }

    /// Open a deterministic [`ReplaySource`] over this fixture's session.
    ///
    /// The session is written to a private temporary file, parsed, and the file
    /// removed before returning, so no path leaks past the call.
    ///
    /// # Errors
    ///
    /// Returns [`FixtureError`] if the temporary file cannot be written or the
    /// session cannot be parsed.
    pub fn replay_source(&self, clock: Arc<dyn Clock>) -> Result<ReplaySource, FixtureError> {
        let path = unique_temp_path();
        std::fs::write(&path, &self.session)?;
        let source = ReplaySource::open(&path, ReplayMode::Deterministic, clock);
        let _ = std::fs::remove_file(&path);
        Ok(source?)
    }
}

/// A process-unique temporary path for a session file.
fn unique_temp_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("aquila-fixture-{pid}-{n}.aqs"))
}
