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

use aquila_sensing::{FloorWindow, HomeWindow};
use aquila_types::{
    Event, EventKind, EventPayload, EventScope, FloorId, GroundTruthRecord, KeypointId, Pose,
    RoomId,
};

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
        if let (EventScope::Zone(room), EventPayload::Presence { occupied, .. }) =
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
        if let (EventScope::Zone(room), EventPayload::Count { people }) =
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

/// The room → floor grouping the floor scorers need: each floor and the rooms
/// it contains, in canonical order.
pub type FloorRooms = Vec<(FloorId, Vec<RoomId>)>;

/// The per-window room presence truth, aggregated from the ground-truth records.
fn room_presence_truth(
    ground_truth: &[GroundTruthRecord],
    hop_ms: i64,
) -> BTreeMap<i64, BTreeMap<RoomId, bool>> {
    let mut truth: BTreeMap<i64, BTreeMap<RoomId, bool>> = BTreeMap::new();
    for record in ground_truth {
        if let (EventScope::Zone(room), EventPayload::Presence { occupied, .. }) =
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
    truth
}

/// The per-window room count truth, aggregated from the ground-truth records.
fn room_count_truth(
    ground_truth: &[GroundTruthRecord],
    hop_ms: i64,
) -> BTreeMap<i64, BTreeMap<RoomId, u8>> {
    let mut truth: BTreeMap<i64, BTreeMap<RoomId, u8>> = BTreeMap::new();
    for record in ground_truth {
        if let (EventScope::Zone(room), EventPayload::Count { people }) =
            (&record.scope, &record.payload)
        {
            truth
                .entry(window_index(record.t_unix_ms, hop_ms))
                .or_default()
                .insert(room.clone(), *people);
        }
    }
    truth
}

/// The per-window floor presence truth: the union (logical OR) of the floor's
/// rooms' presence.
fn floor_presence_truth(
    ground_truth: &[GroundTruthRecord],
    floor_rooms: &FloorRooms,
    hop_ms: i64,
) -> BTreeMap<i64, BTreeMap<FloorId, bool>> {
    let room_truth = room_presence_truth(ground_truth, hop_ms);
    let mut truth: BTreeMap<i64, BTreeMap<FloorId, bool>> = BTreeMap::new();
    for (window, rooms) in &room_truth {
        let entry = truth.entry(*window).or_default();
        for (floor, ids) in floor_rooms {
            let occupied = ids
                .iter()
                .any(|room| rooms.get(room).copied().unwrap_or(false));
            entry.insert(floor.clone(), occupied);
        }
    }
    truth
}

/// The per-window floor count truth: the sum of the floor's rooms' counts.
fn floor_count_truth(
    ground_truth: &[GroundTruthRecord],
    floor_rooms: &FloorRooms,
    hop_ms: i64,
) -> BTreeMap<i64, BTreeMap<FloorId, u8>> {
    let room_truth = room_count_truth(ground_truth, hop_ms);
    let mut truth: BTreeMap<i64, BTreeMap<FloorId, u8>> = BTreeMap::new();
    for (window, rooms) in &room_truth {
        let entry = truth.entry(*window).or_default();
        for (floor, ids) in floor_rooms {
            let sum: u16 = ids
                .iter()
                .map(|room| u16::from(rooms.get(room).copied().unwrap_or(0)))
                .sum();
            entry.insert(floor.clone(), sum.min(u16::from(u8::MAX)) as u8);
        }
    }
    truth
}

/// Group floor presence-change events by window, in deterministic event order.
fn floor_presence_changes(events: &[Event], hop_ms: i64) -> BTreeMap<i64, Vec<(FloorId, bool)>> {
    let mut sorted: Vec<&Event> = events
        .iter()
        .filter(|e| e.kind == EventKind::PresenceChanged)
        .collect();
    sorted.sort_by_key(|e| (e.t_unix_ms, e.id.clone()));
    let mut changes: BTreeMap<i64, Vec<(FloorId, bool)>> = BTreeMap::new();
    for event in sorted {
        if let (EventScope::Floor(floor), EventPayload::Presence { occupied, .. }) =
            (&event.scope, &event.payload)
        {
            changes
                .entry(window_index(event.t_unix_ms, hop_ms))
                .or_default()
                .push((floor.clone(), *occupied));
        }
    }
    changes
}

/// Group floor count-change events by window, in deterministic event order.
fn floor_count_changes(events: &[Event], hop_ms: i64) -> BTreeMap<i64, Vec<(FloorId, u8)>> {
    let mut sorted: Vec<&Event> = events
        .iter()
        .filter(|e| e.kind == EventKind::CountChanged)
        .collect();
    sorted.sort_by_key(|e| (e.t_unix_ms, e.id.clone()));
    let mut changes: BTreeMap<i64, Vec<(FloorId, u8)>> = BTreeMap::new();
    for event in sorted {
        if let (EventScope::Floor(floor), EventPayload::Count { people }) =
            (&event.scope, &event.payload)
        {
            changes
                .entry(window_index(event.t_unix_ms, hop_ms))
                .or_default()
                .push((floor.clone(), *people));
        }
    }
    changes
}

/// Score floor-level presence against the union of the floor's room ground
/// truth (`roadmap.md`, Slice 5). Floor predictions come from the core's
/// floor-scoped presence events.
#[must_use]
pub fn floor_presence_f1(
    events: &[Event],
    ground_truth: &[GroundTruthRecord],
    floor_rooms: &FloorRooms,
    hop_ms: i64,
) -> PresenceScore {
    let truth = floor_presence_truth(ground_truth, floor_rooms, hop_ms);
    let changes = floor_presence_changes(events, hop_ms);
    let predicted = replay(&changes, truth.keys().copied());

    let (mut tp, mut fp, mut fn_, mut tn) = (0u64, 0u64, 0u64, 0u64);
    for (window, actual_floors) in &truth {
        let empty = BTreeMap::new();
        let predicted_floors = predicted.get(window).unwrap_or(&empty);
        for (floor, actual) in actual_floors {
            let guess = predicted_floors.get(floor).copied().unwrap_or(false);
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

/// The mean absolute error of the floor count timeline against the summed room
/// ground truth, with its paired sample count (`roadmap.md`, Slice 5).
#[must_use]
pub fn floor_count_mae(
    events: &[Event],
    ground_truth: &[GroundTruthRecord],
    floor_rooms: &FloorRooms,
    hop_ms: i64,
) -> (f64, u64) {
    let truth = floor_count_truth(ground_truth, floor_rooms, hop_ms);
    let changes = floor_count_changes(events, hop_ms);
    let predicted = replay(&changes, truth.keys().copied());

    let mut total = 0.0_f64;
    let mut n = 0u64;
    for (window, actual_floors) in &truth {
        let empty = BTreeMap::new();
        let predicted_floors = predicted.get(window).unwrap_or(&empty);
        for (floor, actual) in actual_floors {
            let guess = predicted_floors.get(floor).copied().unwrap_or(0);
            total += f64::from(actual.abs_diff(guess));
            n += 1;
        }
    }
    if n == 0 {
        (0.0, 0)
    } else {
        (total / n as f64, n)
    }
}

/// The accuracy of the core's floor room attribution, over the windows where
/// exactly one room on the floor is truly occupied (`roadmap.md`, Slice 5;
/// `attribution.accuracy.room`).
///
/// Windows with zero or more than one truly-occupied room are excluded: there
/// is no single correct room to attribute, so they are not scoreable and must
/// not inflate or deflate the number. Returns `(accuracy, paired_windows)`.
#[must_use]
pub fn room_attribution_accuracy(
    history: &[FloorWindow],
    ground_truth: &[GroundTruthRecord],
    floor_rooms: &FloorRooms,
    hop_ms: i64,
) -> (f64, u64) {
    let rooms_by_floor: BTreeMap<FloorId, &Vec<RoomId>> = floor_rooms
        .iter()
        .map(|(floor, rooms)| (floor.clone(), rooms))
        .collect();
    let room_truth = room_presence_truth(ground_truth, hop_ms);

    let mut correct = 0u64;
    let mut total = 0u64;
    for window in history {
        let window_index = window_index(window.t_ms, hop_ms);
        let Some(rooms) = room_truth.get(&window_index) else {
            continue;
        };
        for floor in &window.floors {
            let Some(floor_room_ids) = rooms_by_floor.get(&floor.floor) else {
                continue;
            };
            let occupied: Vec<&RoomId> = floor_room_ids
                .iter()
                .filter(|room| rooms.get(*room).copied().unwrap_or(false))
                .collect();
            if occupied.len() != 1 {
                continue;
            }
            total += 1;
            if floor.attribution.as_ref() == Some(occupied[0]) {
                correct += 1;
            }
        }
    }
    if total == 0 {
        (0.0, 0)
    } else {
        (correct as f64 / total as f64, total)
    }
}

/// The reference value for a vitals metric: the mean of its ground-truth
/// records. This is honest only for a reference that is constant over the
/// scored span (a steady-rate fixture); a varying reference needs per-window
/// pairing, which this helper does not pretend to do.
#[must_use]
pub fn vitals_reference(ground_truth: &[GroundTruthRecord], metric: &str) -> Option<f32> {
    let mut total = 0.0_f64;
    let mut n = 0u64;
    for record in ground_truth {
        if let EventPayload::Vitals {
            metric: name,
            value,
            ..
        } = &record.payload
        {
            if name == metric && value.is_finite() {
                total += f64::from(*value);
                n += 1;
            }
        }
    }
    if n == 0 {
        None
    } else {
        Some((total / n as f64) as f32)
    }
}

/// The mean absolute error of a vitals metric's emitted events against one
/// reference value, with the paired sample count.
///
/// Returns `(0.0, 0)` when no event carries the metric, so a caller can tell
/// "not measured" from "measured with no error".
#[must_use]
pub fn vitals_mae(events: &[Event], metric: &str, reference: f32) -> (f64, usize) {
    let mut total = 0.0_f64;
    let mut n = 0usize;
    for event in events {
        if let EventPayload::Vitals {
            metric: name,
            value,
            ..
        } = &event.payload
        {
            if name == metric && value.is_finite() {
                total += f64::from((*value - reference).abs());
                n += 1;
            }
        }
    }
    if n == 0 {
        (0.0, 0)
    } else {
        (total / n as f64, n)
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
        if let (EventScope::Zone(room), EventPayload::Presence { occupied, .. }) =
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
        if let (EventScope::Zone(room), EventPayload::Count { people }) =
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
/// forward so every requested window has a prediction. Generic over the unit
/// key (a room or a floor).
fn replay<K: Ord + Clone, T: Clone>(
    changes: &BTreeMap<i64, Vec<(K, T)>>,
    windows: impl Iterator<Item = i64>,
) -> BTreeMap<i64, BTreeMap<K, T>> {
    let change_windows: Vec<i64> = changes.keys().copied().collect();
    let mut cursor = 0usize;
    let mut state: BTreeMap<K, T> = BTreeMap::new();
    let mut out: BTreeMap<i64, BTreeMap<K, T>> = BTreeMap::new();
    for window in windows {
        while cursor < change_windows.len() && change_windows[cursor] <= window {
            for (unit, value) in &changes[&change_windows[cursor]] {
                state.insert(unit.clone(), value.clone());
            }
            cursor += 1;
        }
        out.insert(window, state.clone());
    }
    out
}

/// One of the six torso points torso-PCK@20 scores (`evaluation.md` §4.1): the
/// four COCO-17 shoulder and hip keypoints plus the derived neck (the shoulder
/// midpoint) and pelvis (the hip midpoint).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TorsoPoint {
    /// The left shoulder, COCO-17.
    LeftShoulder,
    /// The right shoulder, COCO-17.
    RightShoulder,
    /// The left hip, COCO-17.
    LeftHip,
    /// The right hip, COCO-17.
    RightHip,
    /// The neck, derived as the shoulder midpoint.
    Neck,
    /// The pelvis, derived as the hip midpoint.
    Pelvis,
}

impl TorsoPoint {
    /// The six torso points, in reporting order.
    pub const ALL: [TorsoPoint; 6] = [
        TorsoPoint::LeftShoulder,
        TorsoPoint::RightShoulder,
        TorsoPoint::LeftHip,
        TorsoPoint::RightHip,
        TorsoPoint::Neck,
        TorsoPoint::Pelvis,
    ];

    /// A stable label for per-keypoint reporting.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            TorsoPoint::LeftShoulder => "left_shoulder",
            TorsoPoint::RightShoulder => "right_shoulder",
            TorsoPoint::LeftHip => "left_hip",
            TorsoPoint::RightHip => "right_hip",
            TorsoPoint::Neck => "neck",
            TorsoPoint::Pelvis => "pelvis",
        }
    }

    /// The point's estimated planar position in `pose`, or `None` when the
    /// pose carries no usable estimate for it.
    #[must_use]
    pub fn position(self, pose: &Pose) -> Option<[f64; 2]> {
        match self {
            TorsoPoint::LeftShoulder => keypoint(pose, KeypointId::LeftShoulder),
            TorsoPoint::RightShoulder => keypoint(pose, KeypointId::RightShoulder),
            TorsoPoint::LeftHip => keypoint(pose, KeypointId::LeftHip),
            TorsoPoint::RightHip => keypoint(pose, KeypointId::RightHip),
            TorsoPoint::Neck => midpoint(
                keypoint(pose, KeypointId::LeftShoulder),
                keypoint(pose, KeypointId::RightShoulder),
            ),
            TorsoPoint::Pelvis => midpoint(
                keypoint(pose, KeypointId::LeftHip),
                keypoint(pose, KeypointId::RightHip),
            ),
        }
    }
}

/// A keypoint's position, or `None` when it is honestly absent.
fn keypoint(pose: &Pose, id: KeypointId) -> Option<[f64; 2]> {
    let key = &pose.keys[id as usize];
    if key.confidence > 0.0 && key.x_m.is_finite() && key.y_m.is_finite() {
        Some([f64::from(key.x_m), f64::from(key.y_m)])
    } else {
        None
    }
}

/// The midpoint of two optional points.
fn midpoint(a: Option<[f64; 2]>, b: Option<[f64; 2]>) -> Option<[f64; 2]> {
    match (a, b) {
        (Some(a), Some(b)) => Some([0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])]),
        _ => None,
    }
}

/// The torso span of a reference pose: the neck-to-pelvis distance, the
/// per-sample normaliser of torso-PCK@20 (`evaluation.md` §4.1). `None` when the
/// pose cannot yield a usable span.
#[must_use]
pub fn torso_span(pose: &Pose) -> Option<f64> {
    let neck = TorsoPoint::Neck.position(pose)?;
    let pelvis = TorsoPoint::Pelvis.position(pose)?;
    let dx = neck[0] - pelvis[0];
    let dy = neck[1] - pelvis[1];
    let span = (dx * dx + dy * dy).sqrt();
    (span.is_finite() && span > 1e-6).then_some(span)
}

/// The result of torso-PCK@20 scoring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PckScore {
    /// Correct torso keypoints.
    pub correct: u64,
    /// Scored torso keypoints (`6 × matched poses`).
    pub total: u64,
    /// `correct / total`, or `0.0` when nothing was scored.
    pub pck: f64,
    /// The mean reference torso span, in metres.
    pub torso_span_m: f64,
    /// Predictions paired with a reference pose.
    pub matched_poses: u64,
    /// Predictions with no reference within tolerance (never silently ignored).
    pub unmatched_predictions: u64,
}

/// Score predictions against reference poses with torso-PCK@20.
///
/// Each prediction is paired with the same room's nearest reference in time
/// within `max_match_ms`; pairing is by key and proximity, never by
/// interpolation. A torso keypoint is correct when its estimated position lies
/// within `tolerance_fraction` (0.20 for PCK@20) of the reference torso span of
/// its paired reference. An absent estimate is scored incorrect, never skipped,
/// so a missing keypoint cannot inflate the score.
#[must_use]
pub fn torso_pck20(
    predictions: &[Pose],
    references: &[Pose],
    tolerance_fraction: f64,
    max_match_ms: i64,
) -> PckScore {
    let (mut correct, mut total) = (0u64, 0u64);
    let (mut matched, mut unmatched) = (0u64, 0u64);
    let mut span_sum = 0.0_f64;

    for prediction in predictions {
        let Some(reference) = nearest_reference(prediction, references, max_match_ms) else {
            unmatched += 1;
            continue;
        };
        let Some(span) = torso_span(reference) else {
            unmatched += 1;
            continue;
        };
        matched += 1;
        span_sum += span;
        let threshold = tolerance_fraction * span;
        for point in TorsoPoint::ALL {
            total += 1;
            if let (Some(estimate), Some(actual)) =
                (point.position(prediction), point.position(reference))
            {
                let dx = estimate[0] - actual[0];
                let dy = estimate[1] - actual[1];
                let distance = (dx * dx + dy * dy).sqrt();
                if distance.is_finite() && distance <= threshold {
                    correct += 1;
                }
            }
        }
    }

    PckScore {
        correct,
        total,
        pck: if total == 0 {
            0.0
        } else {
            correct as f64 / total as f64
        },
        torso_span_m: if matched == 0 {
            0.0
        } else {
            span_sum / matched as f64
        },
        matched_poses: matched,
        unmatched_predictions: unmatched,
    }
}

/// The reference pose of the same room nearest in time to `prediction`, within
/// `max_match_ms`; ties break by earliest reference, so pairing is deterministic.
fn nearest_reference<'a>(
    prediction: &Pose,
    references: &'a [Pose],
    max_match_ms: i64,
) -> Option<&'a Pose> {
    let mut best: Option<&Pose> = None;
    let mut best_delta = i64::MAX;
    for reference in references {
        if reference.room != prediction.room {
            continue;
        }
        let delta = (reference.t_unix_ms - prediction.t_unix_ms).abs();
        if delta <= max_match_ms && delta < best_delta {
            best_delta = delta;
            best = Some(reference);
        }
    }
    best
}

// ---------------------------------------------------------------------------
// Whole-home scoring (`roadmap.md`, Slice 6)
// ---------------------------------------------------------------------------

/// The per-window whole-home presence truth: the home is occupied when any room
/// is.
fn home_presence_truth(ground_truth: &[GroundTruthRecord], hop_ms: i64) -> BTreeMap<i64, bool> {
    room_presence_truth(ground_truth, hop_ms)
        .into_iter()
        .map(|(window, rooms)| (window, rooms.values().any(|occupied| *occupied)))
        .collect()
}

/// The per-window whole-home count truth: the sum of the home's room counts.
fn home_count_truth(ground_truth: &[GroundTruthRecord], hop_ms: i64) -> BTreeMap<i64, u8> {
    room_count_truth(ground_truth, hop_ms)
        .into_iter()
        .map(|(window, rooms)| {
            let sum: u16 = rooms.values().map(|count| u16::from(*count)).sum();
            (window, sum.min(u16::from(u8::MAX)) as u8)
        })
        .collect()
}

/// The whole-home presence changes, keyed by window, in deterministic event
/// order. The last change in a window wins, so replay is order-canonical.
fn home_presence_changes(events: &[Event], hop_ms: i64) -> BTreeMap<i64, bool> {
    let mut sorted: Vec<&Event> = events
        .iter()
        .filter(|e| e.kind == EventKind::PresenceChanged)
        .collect();
    sorted.sort_by_key(|e| (e.t_unix_ms, e.id.clone()));
    let mut changes: BTreeMap<i64, bool> = BTreeMap::new();
    for event in sorted {
        if let (EventScope::Home, EventPayload::Presence { occupied, .. }) =
            (&event.scope, &event.payload)
        {
            changes.insert(window_index(event.t_unix_ms, hop_ms), *occupied);
        }
    }
    changes
}

/// The whole-home count changes, keyed by window, in deterministic event order.
fn home_count_changes(events: &[Event], hop_ms: i64) -> BTreeMap<i64, u8> {
    let mut sorted: Vec<&Event> = events
        .iter()
        .filter(|e| e.kind == EventKind::CountChanged)
        .collect();
    sorted.sort_by_key(|e| (e.t_unix_ms, e.id.clone()));
    let mut changes: BTreeMap<i64, u8> = BTreeMap::new();
    for event in sorted {
        if let (EventScope::Home, EventPayload::Count { people }) = (&event.scope, &event.payload) {
            changes.insert(window_index(event.t_unix_ms, hop_ms), *people);
        }
    }
    changes
}

/// Replay a scalar edge-triggered signal across the given window axis.
fn replay_scalar<T: Clone>(
    changes: &BTreeMap<i64, T>,
    windows: impl Iterator<Item = i64>,
    initial: T,
) -> BTreeMap<i64, T> {
    let change_windows: Vec<i64> = changes.keys().copied().collect();
    let mut cursor = 0usize;
    let mut state = initial;
    let mut out: BTreeMap<i64, T> = BTreeMap::new();
    for window in windows {
        while cursor < change_windows.len() && change_windows[cursor] <= window {
            state = changes[&change_windows[cursor]].clone();
            cursor += 1;
        }
        out.insert(window, state.clone());
    }
    out
}

/// Score whole-home presence against the union of the home's room ground truth
/// (`roadmap.md`, Slice 6; `presence.f1.home`). Home predictions come from the
/// core's home-scoped presence events.
#[must_use]
pub fn home_presence_f1(
    events: &[Event],
    ground_truth: &[GroundTruthRecord],
    hop_ms: i64,
) -> PresenceScore {
    let truth = home_presence_truth(ground_truth, hop_ms);
    let changes = home_presence_changes(events, hop_ms);
    let predicted = replay_scalar(&changes, truth.keys().copied(), false);

    let (mut tp, mut fp, mut fn_, mut tn) = (0u64, 0u64, 0u64, 0u64);
    for (window, actual) in &truth {
        let guess = predicted.get(window).copied().unwrap_or(false);
        match (guess, *actual) {
            (true, true) => tp += 1,
            (true, false) => fp += 1,
            (false, true) => fn_ += 1,
            (false, false) => tn += 1,
        }
    }
    PresenceScore::from_counts(tp, fp, fn_, tn)
}

/// The mean absolute error of the whole-home count timeline against the summed
/// room ground truth, with its paired sample count (`roadmap.md`, Slice 6;
/// `count.mae.home`).
#[must_use]
pub fn home_count_mae(
    events: &[Event],
    ground_truth: &[GroundTruthRecord],
    hop_ms: i64,
) -> (f64, u64) {
    let truth = home_count_truth(ground_truth, hop_ms);
    let changes = home_count_changes(events, hop_ms);
    let predicted = replay_scalar(&changes, truth.keys().copied(), 0u8);

    let mut total = 0.0_f64;
    let mut n = 0u64;
    for (window, actual) in &truth {
        let guess = predicted.get(window).copied().unwrap_or(0);
        total += f64::from(actual.abs_diff(guess));
        n += 1;
    }
    if n == 0 {
        (0.0, 0)
    } else {
        (total / n as f64, n)
    }
}

/// The accuracy of the core's whole-home floor attribution, over the windows
/// where exactly one floor is truly occupied (`roadmap.md`, Slice 6).
///
/// Windows with zero or more than one truly-occupied floor are excluded: there
/// is no single correct floor to attribute, so they are not scoreable and must
/// not inflate or deflate the number. Returns `(accuracy, paired_windows)`.
#[must_use]
pub fn home_floor_attribution_accuracy(
    history: &[HomeWindow],
    ground_truth: &[GroundTruthRecord],
    floor_rooms: &FloorRooms,
    hop_ms: i64,
) -> (f64, u64) {
    let room_truth = room_presence_truth(ground_truth, hop_ms);

    let mut correct = 0u64;
    let mut total = 0u64;
    for window in history {
        let window_index = window_index(window.t_ms, hop_ms);
        let Some(rooms) = room_truth.get(&window_index) else {
            continue;
        };
        let mut occupied_floors: Vec<&FloorId> = floor_rooms
            .iter()
            .filter(|(_, ids)| {
                ids.iter()
                    .any(|room| rooms.get(room).copied().unwrap_or(false))
            })
            .map(|(floor, _)| floor)
            .collect();
        if occupied_floors.len() != 1 {
            continue;
        }
        total += 1;
        let attributed = window
            .home
            .attribution
            .as_ref()
            .map(|location| &location.floor);
        if attributed == Some(occupied_floors.remove(0)) {
            correct += 1;
        }
    }
    if total == 0 {
        (0.0, 0)
    } else {
        (correct as f64 / total as f64, total)
    }
}
