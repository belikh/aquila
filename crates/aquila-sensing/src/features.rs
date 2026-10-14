//! Per-node feature extraction at the contract ↔ signal boundary.
//!
//! The presence detector consumes a per-node **presence energy**: the mean over
//! subcarriers of the window's mean bridged amplitude. A person scatters the
//! signal, raising the received amplitude over an empty room, so the energy is
//! the node's presence evidence. It is a pure function of the window's frames,
//! needs no calibration and is deterministic. The mean temporal standard
//! deviation and coefficient of variation are computed alongside as motion
//! diagnostics and recorded as measurements.
//!
//! The same window is also bridged into `aquila-signal`'s representation and run
//! through its reference pipeline, so the signal crate's `motion_energy` feature
//! is computed at exactly the same boundary (`signal.md` §2, `architecture.md`
//! §4); the bridge is the only place the two representations meet.

use aquila_signal::{Calibration, Config as SignalConfig};
use aquila_types::{CsiEnvelope, NodeId};

use crate::bridge;

/// The motion features of one node over one window.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeMotionFeature {
    /// The node the features belong to.
    pub node: NodeId,
    /// The number of contributing frames.
    pub frames: usize,
    /// The mean per-subcarrier temporal coefficient of variation of amplitude.
    pub amplitude_cv: f64,
    /// The mean per-subcarrier temporal standard deviation of amplitude.
    pub amplitude_std: f64,
    /// The mean per-subcarrier mean amplitude (the node's presence energy).
    pub presence_energy: f64,
    /// The summed `aquila-signal` motion energy, or `0.0` when unavailable.
    pub signal_motion_energy: f64,
    /// Whether `aquila-signal` produced a feature for this node.
    pub used_signal: bool,
}

/// Extract the motion features of one node's window.
///
/// `frames` all belong to `node`. The direct amplitude-variability scores are
/// always computed; the signal-pipeline energy is best-effort and never fails
/// the caller.
#[must_use]
pub fn extract_motion(
    node: NodeId,
    frames: &[CsiEnvelope],
    signal_config: &SignalConfig,
    calibration: &Calibration,
) -> NodeMotionFeature {
    let variability = amplitude_variability(frames);

    let (signal_motion_energy, used_signal) =
        match signal_motion_energy(frames, signal_config, calibration) {
            Some((energy, found)) => (energy, found),
            None => (0.0, false),
        };

    NodeMotionFeature {
        node,
        frames: frames.len(),
        amplitude_cv: variability.cv,
        amplitude_std: variability.std,
        presence_energy: variability.mean,
        signal_motion_energy,
        used_signal,
    }
}

/// The per-subcarrier amplitude statistics of a window.
pub(crate) struct AmplitudeVariability {
    /// Mean over subcarriers of the temporal coefficient of variation.
    pub(crate) cv: f64,
    /// Mean over subcarriers of the temporal standard deviation.
    pub(crate) std: f64,
    /// Mean over subcarriers of the mean amplitude.
    pub(crate) mean: f64,
}

/// The mean over subcarriers of the temporal coefficient of variation, the
/// absolute temporal standard deviation and the mean amplitude, all accumulated
/// in `f64`.
///
/// This is the same variability the presence detector consumes, exposed to the
/// calibration module so a learned threshold is expressed in exactly the units
/// the detector measures (`geometry-calibration.md` §7, L3).
pub(crate) fn amplitude_variability(frames: &[CsiEnvelope]) -> AmplitudeVariability {
    if frames.is_empty() {
        return AmplitudeVariability {
            cv: 0.0,
            std: 0.0,
            mean: 0.0,
        };
    }
    let bins = frames
        .iter()
        .map(|env| env.frame.n_subcarriers())
        .min()
        .unwrap_or(0);
    if bins == 0 {
        return AmplitudeVariability {
            cv: 0.0,
            std: 0.0,
            mean: 0.0,
        };
    }

    let n = frames.len() as f64;
    let mut total_cv = 0.0_f64;
    let mut total_std = 0.0_f64;
    let mut total_mean = 0.0_f64;
    for k in 0..bins {
        let mut sum = 0.0_f64;
        for env in frames {
            let a = env.frame.amplitude_at(k);
            if a.is_finite() {
                sum += a as f64;
            }
        }
        let mean = sum / n;
        let mut var = 0.0_f64;
        for env in frames {
            let a = env.frame.amplitude_at(k);
            if a.is_finite() {
                let d = a as f64 - mean;
                var += d * d;
            }
        }
        let std = (var / n).sqrt();
        total_std += std;
        total_mean += mean;
        total_cv += std / (mean + 1e-6);
    }
    AmplitudeVariability {
        cv: total_cv / bins as f64,
        std: total_std / bins as f64,
        mean: total_mean / bins as f64,
    }
}

/// Run the window through `aquila-signal`'s reference pipeline and return the
/// summed motion energy for the node, or `None` when it cannot be computed.
fn signal_motion_energy(
    frames: &[CsiEnvelope],
    config: &SignalConfig,
    calibration: &Calibration,
) -> Option<(f64, bool)> {
    if frames.is_empty() {
        return None;
    }
    let chunk = bridge::to_signal_chunk(frames).ok()?;
    let features = aquila_signal::process(&chunk, calibration, config).ok()?;
    let node = frames[0].frame.node_id;
    let node_features = features.nodes.iter().find(|n| n.node == node)?;
    let energy: f64 = node_features
        .motion_energy
        .iter()
        .map(|e| f64::from(*e))
        .sum();
    Some((energy, true))
}
