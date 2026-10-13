//! Scoring: presence F1 and count MAE against simulator ground truth.
//!
//! The core emits *edge-triggered* [`Event`]s, so a scorer first reconstructs a
//! per-window presence and count timeline from those edges, then compares it
//! with the per-window ground truth. Alignment is by window index on the shared
//! core-time axis (`t_unix_ms / hop_ms`), which is why the tracer test sets the
//! simulator's `start_unix_ms` to zero.
//!
//! Ground truth is research-only and never a runtime input
//! (`data-contracts.md` §7); this module lives in the dev-only testkit.

use std::collections::BTreeMap;

use aquila_types::{Event, EventKind, EventPayload, EventScope, GroundTruthRecord, RoomId};

/// A binary-classification confusion matrix and its derived scores.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PresenceScore {
    /// Windows and rooms both predicted and truly occupied.
    pub true_positive: u64,
    /// Windows and rooms predicted occupied but truly absent.
    pub false_positive: u64,
    /// Windows and rooms predicted absent but truly occupied.
    pub false_negative: u64,
    /// Windows and rooms both predicted and truly absent.
    pub true_negative: u64,
    /// `TP / (TP + FP)`, or `0.0` when the denominator is zero.
    pub precision: f64,
    /// `TP / (TP + FN)`, or `0.0` when the denominator is zero.
    pub recall: f64,
    /// The harmonic mean of precision and recall.
    pub f1: f64,
}

impl PresenceScore {
    /// Compute the derived scores from the counts.
    #[must_use]
    pub fn from_counts(tp: u64, fp: u64, fn_: u64, tn: u64) -> Self {
        let precision = ratio(tp, tp + fp);
        let recall = ratio(tp, tp + fn_);
        let f1 = if precision + recall > 0.0 {
            2.0 * precision * recall / (precision + recall)
        } else {
            0.0
        };
        Self {
            true_positive: tp,
            false_positive: fp,
            false_negative: fn_,
            true_negative: tn,
            precision,
            recall,
            f1,
        }
    }
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

/// The window index of a wall-clock millisecond, on a `hop_ms` grid.
#[must_use]
pub fn window_index(t_unix_ms: i64, hop_ms: i64) -> i64 {
    if hop_ms <= 0 {
        return 0;
    }
    t_unix_ms.div_euclid(hop_ms)
}

/// Reconstruct the per-room presence state for every window named by the ground
/// truth, then score it.
#[must_use]
pub fn presence_f1(
    events: &[Event],
    ground_truth: &[GroundTruthRecord],
    hop_ms: i64,
) -> PresenceScore {
    // Ground truth: window → room → occupied-if-any.
    let mut truth: BTreeMap<i64, BTreeMap<RoomId, bool>> = BTreeMap::new();
    for record in ground_truth {
        if let (EventScope::Room(room), EventPayload::Presence { occupied, .. }) =
            (&record.scope, &record.payload)
        {
            truth
                .entry(window_index(record.t_unix_ms, hop_ms))
                .or_default()
                .entry(room.clone())
                .and_modify(|state| *state |= *occupied)
                .or_insert(*occupied);
        }
    }

    let changes = presence_changes(events, hop_ms);
    let predicted = replay(&changes, truth.keys().copied());

    let (mut tp, mut fp, mut fn_, mut tn) = (0u64, 0u64, 0u64, 0u64);
    for (window, actual_rooms) in &truth {
        let empty = BTreeMap::new();
        let predicted_rooms = predicted.get(window).unwrap_or(&empty);
        for (room, actual) in actual_rooms {
            let guess = predicted_rooms.get(room).copied().unwrap_or(false);
            match (guess, *actual) {
                (true, true) => tp += 1,
                (true, false) => fp += 1,
                (false, true) => fn_ += 1,
                (false, false) => tn += 1,
            }
        }
    }
    PresenceScore::from_counts(tp, fp, fn_, tn)
}

/// The mean absolute error of the reconstructed count timeline against the
/// ground truth.
#[must_use]
pub fn count_mae(events: &[Event], ground_truth: &[GroundTruthRecord], hop_ms: i64) -> f64 {
    let mut truth: BTreeMap<i64, BTreeMap<RoomId, u8>> = BTreeMap::new();
    for record in ground_truth {
        if let (EventScope::Room(room), EventPayload::Count { people }) =
            (&record.scope, &record.payload)
        {
            // The last count within a window describes that window's tally.
            truth
                .entry(window_index(record.t_unix_ms, hop_ms))
                .or_default()
                .insert(room.clone(), *people);
        }
    }

    let changes = count_changes(events, hop_ms);
    let predicted = replay(&changes, truth.keys().copied());

    let mut total = 0.0_f64;
    let mut n = 0u64;
    for (window, actual_rooms) in &truth {
        let empty = BTreeMap::new();
        let predicted_rooms = predicted.get(window).unwrap_or(&empty);
        for (room, actual) in actual_rooms {
            let guess = predicted_rooms.get(room).copied().unwrap_or(0);
            total += f64::from(actual.abs_diff(guess));
            n += 1;
        }
    }
    if n == 0 {
        0.0
    } else {
        total / n as f64
    }
}

/// Group presence-change events by window, in deterministic event order.
fn presence_changes(events: &[Event], hop_ms: i64) -> BTreeMap<i64, Vec<(RoomId, bool)>> {
    let mut sorted: Vec<&Event> = events
        .iter()
        .filter(|e| e.kind == EventKind::PresenceChanged)
        .collect();
    sorted.sort_by_key(|e| (e.t_unix_ms, e.id.clone()));
    let mut changes: BTreeMap<i64, Vec<(RoomId, bool)>> = BTreeMap::new();
    for event in sorted {
        if let (EventScope::Room(room), EventPayload::Presence { occupied, .. }) =
            (&event.scope, &event.payload)
        {
            changes
                .entry(window_index(event.t_unix_ms, hop_ms))
                .or_default()
                .push((room.clone(), *occupied));
        }
    }
    changes
}

/// Group count-change events by window, in deterministic event order.
fn count_changes(events: &[Event], hop_ms: i64) -> BTreeMap<i64, Vec<(RoomId, u8)>> {
    let mut sorted: Vec<&Event> = events
        .iter()
        .filter(|e| e.kind == EventKind::CountChanged)
        .collect();
    sorted.sort_by_key(|e| (e.t_unix_ms, e.id.clone()));
    let mut changes: BTreeMap<i64, Vec<(RoomId, u8)>> = BTreeMap::new();
    for event in sorted {
        if let (EventScope::Room(room), EventPayload::Count { people }) =
            (&event.scope, &event.payload)
        {
            changes
                .entry(window_index(event.t_unix_ms, hop_ms))
                .or_default()
                .push((room.clone(), *people));
        }
    }
    changes
}

/// Replay edge-triggered changes across the given window axis, carrying state
/// forward so every requested window has a prediction.
fn replay<T: Clone>(
    changes: &BTreeMap<i64, Vec<(RoomId, T)>>,
    windows: impl Iterator<Item = i64>,
) -> BTreeMap<i64, BTreeMap<RoomId, T>> {
    let change_windows: Vec<i64> = changes.keys().copied().collect();
    let mut cursor = 0usize;
    let mut state: BTreeMap<RoomId, T> = BTreeMap::new();
    let mut out: BTreeMap<i64, BTreeMap<RoomId, T>> = BTreeMap::new();
    for window in windows {
        while cursor < change_windows.len() && change_windows[cursor] <= window {
            for (room, value) in &changes[&change_windows[cursor]] {
                state.insert(room.clone(), value.clone());
            }
            cursor += 1;
        }
        out.insert(window, state.clone());
    }
    out
}
