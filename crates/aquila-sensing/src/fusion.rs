//! Feature-level fusion.
//!
//! Fusion combines per-node features and detections only; there is no cross-node
//! raw-phase combination (`fusion-sync.md` §2). Presence uses **log-odds
//! pooling** — a commutative, associative Bayesian product of independent
//! evidence — with each node's contribution clamped so no single node can force
//! occupancy. Count uses a **weighted arithmetic mixture** of node PMFs, which
//! avoids the over-confidence of pooling counts as independent decisions.
//!
//! Unknown is never conflated with absent: a prior log-odds of `0` yields
//! `p = 0.5`, and an empty evidence set yields the prior, never "absent".

use aquila_types::{NodeId, RoomId};

/// A discrete probability mass function over counts `0..=MAX_COUNT_V1`.
#[derive(Debug, Clone, PartialEq)]
pub struct CountPmf {
    /// The per-count probabilities; `probs[0]` is `P(0)`, and so on.
    pub probs: Vec<f64>,
}

impl CountPmf {
    /// A point mass at `count`, clamped into range.
    #[must_use]
    pub fn point(count: u8, max: usize) -> Self {
        let mut probs = vec![0.0; max + 1];
        let index = usize::from(count).min(max);
        probs[index] = 1.0;
        Self { probs }
    }

    /// The MAP count and its probability.
    #[must_use]
    pub fn map(&self) -> (u8, f64) {
        let mut best_index = 0usize;
        let mut best = f64::NEG_INFINITY;
        for (i, p) in self.probs.iter().enumerate() {
            if *p > best {
                best = *p;
                best_index = i;
            }
        }
        (best_index.min(u8::MAX as usize) as u8, best.max(0.0))
    }

    /// Renormalise the mass to sum to one, guarding against an all-zero PMF.
    fn normalised(mut self) -> Self {
        let sum: f64 = self.probs.iter().sum();
        if sum > 0.0 && sum.is_finite() {
            for p in &mut self.probs {
                *p /= sum;
            }
        }
        self
    }
}

/// One node's evidence for one window (the v1 subset of `fusion-sync.md` §2.1).
#[derive(Debug, Clone, PartialEq)]
pub struct NodeEvidence {
    /// The contributing node.
    pub node: NodeId,
    /// The room the node is mounted in.
    pub room: RoomId,
    /// The log-odds of presence in this node's volume.
    pub presence_logit: f64,
    /// The node's count PMF over its volume.
    pub count_pmf: CountPmf,
    /// The fusion weight, in `[0, 1]`.
    pub weight: f64,
    /// The number of frames behind the evidence.
    pub frames: usize,
}

/// The logistic function, evaluated without overflowing for large magnitudes.
#[must_use]
pub fn sigmoid(x: f64) -> f64 {
    if x >= 0.0 {
        let z = (-x).exp();
        1.0 / (1.0 + z)
    } else {
        let z = x.exp();
        z / (1.0 + z)
    }
}

/// Clamp a log-odds contribution to `[-l_max, l_max]`.
#[must_use]
pub fn clamp_logit(x: f64, l_max: f64) -> f64 {
    if !x.is_finite() {
        0.0
    } else {
        x.clamp(-l_max, l_max)
    }
}

/// Pool per-node presence evidence into a room log-odds.
///
/// `L = prior + Σ w_n · clamp(L_n)`. The sum is over a canonical node order so
/// the result is invariant to the order evidence is supplied in.
#[must_use]
pub fn pool_presence_logit(prior_logit: f64, evidence: &[NodeEvidence], l_max: f64) -> f64 {
    let mut ordered: Vec<&NodeEvidence> = evidence.iter().collect();
    ordered.sort_by_key(|e| e.node);
    let mut logit = prior_logit;
    for e in ordered {
        let weight = e.weight.clamp(0.0, 1.0);
        logit += weight * clamp_logit(e.presence_logit, l_max);
    }
    logit
}

/// The pooled presence probability of a room.
#[must_use]
pub fn pooled_presence_probability(prior_logit: f64, evidence: &[NodeEvidence], l_max: f64) -> f64 {
    sigmoid(pool_presence_logit(prior_logit, evidence, l_max))
}

/// A two-point PMF for one node: `P(0) = 1 - p`, `P(1) = p`.
///
/// A unary node cannot see two people; more than one occupant in a room is the
/// room mixture's job, not a single node's.
#[must_use]
pub fn node_count_pmf(presence_probability: f64, max: usize) -> CountPmf {
    let p = presence_probability.clamp(0.0, 1.0);
    let mut probs = vec![0.0; max + 1];
    probs[0] = 1.0 - p;
    if max >= 1 {
        probs[1] = p;
    } else {
        probs[0] = 1.0;
    }
    CountPmf { probs }
}

/// Mix node count PMFs by weight into one room PMF.
///
/// Nodes within a room are mixed, not summed, so two nodes seeing the same
/// person do not double-count (`fusion-sync.md` §2.2).
#[must_use]
pub fn mix_count_pmf(evidence: &[NodeEvidence], max: usize) -> CountPmf {
    let mut ordered: Vec<&NodeEvidence> = evidence.iter().collect();
    ordered.sort_by_key(|e| e.node);
    let total_weight: f64 = ordered.iter().map(|e| e.weight.clamp(0.0, 1.0)).sum();
    if ordered.is_empty() || total_weight <= 0.0 {
        // No usable evidence: a broad, uninformative PMF, never a fabricated 0.
        let uniform = 1.0 / (max + 1) as f64;
        return CountPmf {
            probs: vec![uniform; max + 1],
        };
    }
    let mut probs = vec![0.0_f64; max + 1];
    for e in ordered {
        let weight = e.weight.clamp(0.0, 1.0) / total_weight;
        for (slot, p) in probs.iter_mut().zip(e.count_pmf.probs.iter()) {
            *slot += weight * p;
        }
    }
    CountPmf { probs }.normalised()
}
