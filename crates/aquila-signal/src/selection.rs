//! Subcarrier selection.
//!
//! `select_subcarriers` chooses the subcarriers that actually carry
//! motion/vitals signal rather than noise (`signal.md` §4.11). Selection runs
//! during calibration and is frozen into [`Config`](crate::Config); the runtime
//! pipeline consumes the frozen mask.

/// A per-subcarrier feature matrix: band power against noise power.
///
/// `indices`, `band_power` and `noise_power` are parallel; the score of a
/// candidate is `band_power / noise_power` (or `band_power` when the noise
/// power is zero).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SubcarrierFeatureMatrix {
    /// The candidate subcarrier indices.
    pub indices: Vec<u16>,
    /// The signal-band power per candidate.
    pub band_power: Vec<f32>,
    /// The noise power per candidate.
    pub noise_power: Vec<f32>,
}

impl SubcarrierFeatureMatrix {
    /// Construct a matrix from parallel vectors.
    #[must_use]
    pub fn new(indices: Vec<u16>, band_power: Vec<f32>, noise_power: Vec<f32>) -> Self {
        SubcarrierFeatureMatrix {
            indices,
            band_power,
            noise_power,
        }
    }

    /// The score of candidate `i`, or `None` if any input is non-finite.
    #[must_use]
    pub fn score(&self, i: usize) -> Option<f32> {
        let band = *self.band_power.get(i)?;
        let noise = *self.noise_power.get(i)?;
        if !band.is_finite() || !noise.is_finite() || band < 0.0 {
            return None;
        }
        let score = if noise > 0.0 { band / noise } else { band };
        if score.is_finite() {
            Some(score)
        } else {
            None
        }
    }
}

/// The frozen selection: the chosen subcarrier indices, ascending.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubcarrierMask {
    /// The selected indices, strictly ascending (`K2`).
    pub selected: Vec<u16>,
}

impl SubcarrierMask {
    /// Whether `index` is selected.
    #[must_use]
    pub fn contains(&self, index: u16) -> bool {
        self.selected.binary_search(&index).is_ok()
    }

    /// The number of selected subcarriers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.selected.len()
    }

    /// Whether no subcarrier was selected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }
}

/// Select the top-`k` subcarriers by score.
///
/// Candidates are ordered by score descending, index ascending (`K4`); the top
/// `k` are taken and returned ascending (`K2`). If fewer than `k` valid
/// candidates exist, all are returned (`K3`). Non-finite scores are excluded,
/// never treated as `+∞`. The result depends only on `(index, score)` pairs, so
/// it is permutation-invariant (`K6`).
#[must_use]
pub fn select_subcarriers(features: &SubcarrierFeatureMatrix, k: usize) -> SubcarrierMask {
    let mut pairs: Vec<(u16, f32)> = Vec::new();
    for i in 0..features.indices.len() {
        if let Some(score) = features.score(i) {
            pairs.push((features.indices[i], score));
        }
    }
    // Score descending, then index ascending. `total_cmp` gives a total order
    // even for values a plain `PartialOrd` would leave unordered, but scores
    // are finite here, so the comparison is exact and deterministic.
    pairs.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut selected: Vec<u16> = pairs.into_iter().take(k).map(|(index, _)| index).collect();
    selected.sort_unstable();
    selected.dedup();
    SubcarrierMask { selected }
}
