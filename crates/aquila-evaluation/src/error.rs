//! The typed error surface of `aquila-evaluation`.

/// Everything that can go wrong scoring a run.
#[derive(Debug, thiserror::Error)]
pub enum EvaluationError {
    /// The simulator could not build the scene.
    #[error("simulator error: {0}")]
    Simulator(#[from] aquila_simulator::SimError),
    /// A reference record could not be aligned to the estimate.
    #[error("alignment failed: {0}")]
    Alignment(String),
    /// The run produced no comparable pairs, so no metric can be stated.
    #[error("no paired samples: {0}")]
    NoSamples(String),
}
