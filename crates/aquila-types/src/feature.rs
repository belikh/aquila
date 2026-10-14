//! The canonical packed feature vector — the model-input carrier.
//!
//! `FeatureVector` is the one type `aquila-signal` produces and
//! `aquila-inference` consumes (`data-contracts.md` §1, `inference-models.md`
//! §1.2). It lives here, in the level-0 contract crate, so the producer and the
//! consumer share a single declaration instead of each owning a fork.
//!
//! # Layout
//!
//! The values are packed, node-relative and opaque to the model seam: the
//! producer owns the layout and the ONNX adapter maps the packed values onto a
//! tensor (`inference-models.md` §1.4, §8.3). For one node and one sensing
//! frame the layout is the usable-subcarrier amplitude RMS values followed by
//! the usable-subcarrier motion energies, both in ascending usable-subcarrier
//! order (`signal.md` §2.4). The seam therefore only requires a present,
//! non-empty vector; it never invents, resizes or pads a layout.

/// A packed feature vector for one node and one sensing frame.
///
/// Produced by `aquila-signal`; consumed by `aquila-inference`. The layout is
/// documented above and owned by the producer.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureVector {
    values: Vec<f32>,
}

impl FeatureVector {
    /// Wrap a packed feature vector.
    #[must_use]
    pub fn new(values: Vec<f32>) -> Self {
        Self { values }
    }

    /// The feature values, in the producer's layout.
    #[must_use]
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    /// The number of packed feature values.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether the vector carries no values.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl From<Vec<f32>> for FeatureVector {
    fn from(values: Vec<f32>) -> Self {
        Self::new(values)
    }
}
