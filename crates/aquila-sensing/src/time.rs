//! Soft time synchronisation and per-node drift.
//!
//! Nodes are dumb: they stamp every frame with their own drifting device clock
//! and never adjust it (`fusion-sync.md` §1.1). The core owns correction. This
//! module holds a small, deterministic affine drift fit per node: a device-time
//! to core-time map fitted from the `(device, core)` pairs the core observes as
//! frames arrive.
//!
//! Slice 1 has no dedicated NTP-style control round trips, so the fit is not a
//! substitute for the four-timestamp exchange of `fusion-sync.md` §1.2: it is
//! seeded from frame arrivals and is honest about its sample count and residual.
//! A node with too few samples yields no constraint, and the window assembler
//! falls back to core arrival time.

use std::collections::BTreeMap;

use aquila_types::NodeId;

/// An affine map from device time to core time for one node.
///
/// `core_ns ≈ slope · device_ns + intercept_ns`. The slope is near `1` for a
/// well-behaved node; a slope below `1` means the device clock runs fast.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeClockFit {
    /// The node this fit describes.
    pub node: NodeId,
    /// The least-squares slope.
    pub slope: f64,
    /// The least-squares intercept, in nanoseconds.
    pub intercept_ns: f64,
    /// The number of accepted samples in the fit.
    pub samples: usize,
    /// The root-mean-square residual of the fit, in nanoseconds.
    pub residual_rms_ns: f64,
}

impl NodeClockFit {
    /// Correct a device timestamp to core time, saturating at zero.
    #[must_use]
    pub fn correct_core_ns(&self, device_ns: u64) -> u64 {
        let core = self.slope * device_ns as f64 + self.intercept_ns;
        if !core.is_finite() || core < 0.0 {
            0
        } else if core >= u64::MAX as f64 {
            u64::MAX
        } else {
            core.round() as u64
        }
    }

    /// A health weight in `(0, 1]` derived from the fit residual: a tighter fit
    /// weighs more heavily in fusion.
    #[must_use]
    pub fn health(&self) -> f64 {
        if !self.residual_rms_ns.is_finite() {
            return 0.0;
        }
        // A residual of 1 ms (a generous LAN-scale bound) halves the weight.
        1.0 / (1.0 + self.residual_rms_ns / 1_000_000.0)
    }
}

/// A bounded per-node collection of `(device, core)` observations and the fits
/// derived from them.
#[derive(Debug, Clone)]
pub struct TimeSync {
    window: usize,
    samples: BTreeMap<NodeId, Vec<(f64, f64)>>,
    fits: BTreeMap<NodeId, NodeClockFit>,
    min_samples: usize,
}

impl TimeSync {
    /// Construct a synchroniser keeping at most `window` samples per node.
    ///
    /// `min_samples` is the number of observations a node needs before a fit is
    /// published; below it, [`model`](TimeSync::model) returns `None`.
    #[must_use]
    pub fn new(window: usize, min_samples: usize) -> Self {
        Self {
            window: window.max(1),
            samples: BTreeMap::new(),
            fits: BTreeMap::new(),
            min_samples: min_samples.max(2),
        }
    }

    /// Record one observation: the node's device stamp and the core arrival of
    /// the same frame.
    pub fn observe(&mut self, node: NodeId, device_ns: u64, core_ns: u64) {
        let entry = self.samples.entry(node).or_default();
        entry.push((device_ns as f64, core_ns as f64));
        if entry.len() > self.window {
            let excess = entry.len() - self.window;
            entry.drain(0..excess);
        }
        if entry.len() >= self.min_samples {
            if let Some(fit) = fit_node(node, entry) {
                self.fits.insert(node, fit);
            }
        }
    }

    /// The current fit for a node, if it has enough accepted samples.
    #[must_use]
    pub fn model(&self, node: NodeId) -> Option<&NodeClockFit> {
        self.fits.get(&node)
    }

    /// The fusion weight for a node's clock: `1.0` when there is no fit
    /// (unconstrained, but not penalised), the fit's health otherwise.
    #[must_use]
    pub fn weight(&self, node: NodeId) -> f64 {
        match self.fits.get(&node) {
            Some(fit) => fit.health(),
            None => 1.0,
        }
    }

    /// The number of observations held for a node.
    #[must_use]
    pub fn sample_count(&self, node: NodeId) -> usize {
        self.samples.get(&node).map_or(0, Vec::len)
    }
}

/// Fit `core = slope · device + intercept` by ordinary least squares, returning
/// `None` when there are too few samples or the device axis has no spread.
fn fit_node(node: NodeId, samples: &[(f64, f64)]) -> Option<NodeClockFit> {
    let n = samples.len();
    if n < 2 {
        return None;
    }
    let nf = n as f64;
    let mut sum_x = 0.0_f64;
    let mut sum_y = 0.0_f64;
    for &(x, y) in samples {
        sum_x += x;
        sum_y += y;
    }
    let mean_x = sum_x / nf;
    let mean_y = sum_y / nf;

    let mut sxx = 0.0_f64;
    let mut sxy = 0.0_f64;
    for &(x, y) in samples {
        let dx = x - mean_x;
        sxx += dx * dx;
        sxy += dx * (y - mean_y);
    }
    if sxx <= f64::EPSILON {
        return None;
    }
    let slope = sxy / sxx;
    let intercept = mean_y - slope * mean_x;

    let mut sum_sq = 0.0_f64;
    for &(x, y) in samples {
        let residual = y - (slope * x + intercept);
        sum_sq += residual * residual;
    }
    let residual_rms = (sum_sq / nf).sqrt();

    Some(NodeClockFit {
        node,
        slope,
        intercept_ns: intercept,
        samples: n,
        residual_rms_ns: residual_rms,
    })
}
