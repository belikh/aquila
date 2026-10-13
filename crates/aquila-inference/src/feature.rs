//! The feature carrier consumed by the model seam.
//!
//! `inference-models.md` §1.2 and §1.3 show the seam consuming a
//! `FeatureVector` produced by `aquila-signal`, with the `FeatureVector`→tensor
//! projection owned by this crate and frozen with `aquila-signal` before the
//! ONNX adapter is implemented (§8.3). That layout is **not pinned yet**, and
//! the L0 contract crate does not yet declare `FeatureVector`, so this crate
//! carries a provisional, layout-agnostic wrapper for the seam.
//!
//! This type is deliberately opaque: it holds the packed feature values and
//! exposes them by slice, without asserting an internal layout. When
//! `aquila-signal` freezes the layout, the carrier moves to the L0 contract
//! crate and the ONNX adapter maps it to a tensor here. Until then, no code
//! fabricates a layout.

/// A packed feature vector for one `SensingFrame`, produced by `aquila-signal`.
///
/// The feature values are opaque to this crate: their layout is owned by
/// `aquila-signal` and is not pinned yet (`inference-models.md` §8.3). The seam
/// only checks that a vector is present and non-empty; it never invents a
/// layout, resizes or pads.
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
