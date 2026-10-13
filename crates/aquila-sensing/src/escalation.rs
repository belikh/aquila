//! Room → floor → home escalation.
//!
//! Slice 1 emits room-scope (zone) presence and count: a room *is* a zone in v1
//! (`CONTEXT.md`). Slice 5 grows the hierarchy room → floor
//! (`roadmap.md`, Slice 5): room-level outputs compose into per-floor presence,
//! count and vitals, with room attribution recorded, and the hierarchy is data
//! (from the topology), never hard-coded.
//!
//! The pooling rules are the documented ones (`fusion-sync.md` §2.5):
//!
//! - **Floor presence** is a **soft-OR** across the floor's rooms,
//!   `p_floor = 1 − Π_rooms (1 − p_room)`: rooms are independent presence
//!   witnesses, so presence is present if any room is.
//! - **Floor count** is the **convolution** of the floor's room count PMFs
//!   (`P_floor = P_r1 ⊛ P_r2 ⊛ …`), then MAP + confidence. Convolution is used
//!   only across a spatial boundary; nodes *within* a room are mixed, not summed
//!   (§2.2). A room with no usable evidence contributes no count: the floor count
//!   is reported `Unknown`, never fabricated (§2.5, §2.6).
//! - **Floor vitals** are aggregated by **highest-confidence selection** per
//!   band across the floor's rooms — never a coherent phase combination
//!   (§2.2, §3). A band with no estimate above the confidence floor is absent.
//!
//! # Unknown is first-class
//!
//! A room with no usable evidence is [`RoomCoverage::Uncovered`]: its presence
//! and count are Unknown, never `Absent`/`0`, and an uncovered room forces the
//! floor count to Unknown with a coverage note (`fusion-sync.md` §2.5). Absence
//! and unknown are never conflated.
//!
//! # Attribution is reported, not guessed
//!
//! When the floor is occupied and the two strongest rooms carry similar
//! presence probability (within [`Conflict::AmbiguousRooms`]' `room_gap`), the
//! floor's room attribution is withheld and the conflict is surfaced with each
//! room's confidence — never a false confident room (`fusion-sync.md` §2.4.1,
//! `risks.md` R-01/Q-01). The floor's aggregate presence and convolved count are
//! unaffected, because they do not depend on attributing presence to a single
//! room: the ambiguity is about *which* room, not *whether* the floor is
//! occupied.

use std::cmp::Ordering;

use aquila_types::{Estimate, FloorId, RoomId};

use crate::fusion::CountPmf;

/// Why a room contributes (or does not) to a floor's aggregate count.
///
/// Coverage is separate from the count value: an uncovered room is Unknown,
/// never a fabricated `0` (`fusion-sync.md` §2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomCoverage {
    /// The room had usable, calibrated evidence this window.
    Covered,
    /// The room had no usable evidence; its count is Unknown.
    Uncovered,
}

/// The per-room summary the escalation step consumes.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomSummary {
    /// The room.
    pub room: RoomId,
    /// The pooled presence probability in `[0, 1]`.
    pub presence_probability: f64,
    /// The MAP people count.
    pub count: u8,
    /// The room's count PMF, for the floor convolution.
    pub count_pmf: CountPmf,
    /// Whether the room carried usable evidence this window.
    pub coverage: RoomCoverage,
}

impl RoomSummary {
    /// A covered room summary from its pooled presence probability and count
    /// PMF. The MAP count is derived from the PMF, so the two cannot disagree.
    #[must_use]
    pub fn covered(room: RoomId, presence_probability: f64, count_pmf: CountPmf) -> Self {
        let (count, _) = count_pmf.map();
        Self {
            room,
            presence_probability: presence_probability.clamp(0.0, 1.0),
            count,
            count_pmf,
            coverage: RoomCoverage::Covered,
        }
    }

    /// An uncovered room summary: presence and count are Unknown
    /// (`Coverage::Uncalibrated`, `fusion-sync.md` §2.5). The retained PMF is a
    /// broad, uninformative placeholder; [`RoomCoverage::Uncovered`] is what
    /// forces the floor count to Unknown.
    #[must_use]
    pub fn uncovered(room: RoomId, max_count: usize) -> Self {
        let uniform = 1.0 / (max_count + 1) as f64;
        Self {
            room,
            presence_probability: 0.0,
            count: 0,
            count_pmf: CountPmf {
                probs: vec![uniform; max_count + 1],
            },
            coverage: RoomCoverage::Uncovered,
        }
    }
}

/// A conflict surfaced by escalation. Conflicts are **reported, not averaged
/// away** (`fusion-sync.md` §2.4).
#[derive(Debug, Clone, PartialEq)]
pub enum Conflict {
    /// Room attribution is ambiguous: the two strongest room presence
    /// probabilities on a floor lie within `room_gap`, so naming a single room
    /// would be a false confident claim (`fusion-sync.md` §2.4.1).
    AmbiguousRooms {
        /// The floor whose attribution is unresolved.
        floor: FloorId,
        /// Each contending room and its pooled presence probability, in
        /// canonical room-id order.
        candidates: Vec<(RoomId, f64)>,
    },
}

/// The aggregated state of one floor for one window.
#[derive(Debug, Clone, PartialEq)]
pub struct FloorSummary {
    /// The floor.
    pub floor: FloorId,
    /// The soft-OR presence probability across the floor's covered rooms;
    /// `None` when no room on the floor has usable evidence
    /// (`Occupancy::Unknown`, never `Absent`).
    pub presence_probability: Option<f64>,
    /// The MAP floor count from the room-PMF convolution, or `None` when the
    /// floor count is Unknown (an uncovered room, or no covered rooms).
    pub count: Option<u8>,
    /// The convolved count PMF's MAP probability; `0.0` when the count is
    /// Unknown.
    pub count_confidence: f64,
    /// The convolved floor count PMF, or `None` when Unknown.
    pub count_pmf: Option<CountPmf>,
    /// The floor's rooms, canonical room-id order.
    pub rooms: Vec<RoomSummary>,
    /// The confidently attributed room, or `None` when the floor is unoccupied
    /// or the attribution is ambiguous.
    pub attribution: Option<RoomId>,
    /// Every conflict raised by this floor this window.
    pub conflicts: Vec<Conflict>,
}

/// Floor occupancy: the soft-OR `1 − Π(1 − p_room)` across the given rooms'
/// presence probabilities (`fusion-sync.md` §2.5).
///
/// Only [`RoomCoverage::Covered`] rooms contribute: an uncovered room is
/// Unknown, not a `p = 0` witness. With no covered rooms this returns `0.0`; a
/// caller that needs to distinguish "unknown" from "absent" should use
/// [`aggregate_floor`], whose `presence_probability` is `Option`.
#[must_use]
pub fn floor_presence_probability(rooms: &[RoomSummary]) -> f64 {
    let mut complement = 1.0_f64;
    for room in rooms
        .iter()
        .filter(|room| room.coverage == RoomCoverage::Covered)
    {
        complement *= 1.0 - room.presence_probability.clamp(0.0, 1.0);
    }
    (1.0 - complement).clamp(0.0, 1.0)
}

/// Home occupancy: a soft-OR over floors, each already a soft-OR over rooms.
#[must_use]
pub fn home_presence_probability(floors: &[f64]) -> f64 {
    let mut complement = 1.0_f64;
    for p in floors {
        complement *= 1.0 - p.clamp(0.0, 1.0);
    }
    (1.0 - complement).clamp(0.0, 1.0)
}

/// Aggregate one floor from its room summaries (`fusion-sync.md` §2.5).
///
/// `room_gap` is the attribution ambiguity margin; `presence_threshold` is the
/// presence probability above which a room counts as occupied for attribution
/// (`p_hi`); `max_count` bounds the convolved PMF.
///
/// Determinism: rooms are processed in canonical room-id order, so the result
/// is independent of the order `rooms` is supplied in.
#[must_use]
pub fn aggregate_floor(
    floor: FloorId,
    rooms: &[RoomSummary],
    room_gap: f64,
    presence_threshold: f64,
    max_count: usize,
) -> FloorSummary {
    let mut ordered: Vec<RoomSummary> = rooms.to_vec();
    ordered.sort_by(|a, b| a.room.cmp(&b.room));

    let covered: Vec<&RoomSummary> = ordered
        .iter()
        .filter(|room| room.coverage == RoomCoverage::Covered)
        .collect();
    let all_covered = covered.len() == ordered.len() && !ordered.is_empty();

    // Presence: soft-OR over the covered rooms only.
    let presence_probability = if covered.is_empty() {
        None
    } else {
        Some(floor_presence_probability(&ordered))
    };

    // Count: convolution over the covered rooms. Any uncovered room makes the
    // floor count Unknown rather than treating a broad, uninformative factor as
    // evidence (`fusion-sync.md` §2.5).
    let count_pmf = if !all_covered {
        None
    } else {
        let mut acc: Option<CountPmf> = None;
        for room in &covered {
            acc = Some(match acc {
                None => room.count_pmf.clone(),
                Some(previous) => convolve(&previous, &room.count_pmf, max_count),
            });
        }
        acc
    };
    let (count, count_confidence) = match &count_pmf {
        Some(pmf) => {
            let (count, confidence) = pmf.map();
            (Some(count), confidence)
        }
        None => (None, 0.0),
    };

    // Attribution: the strongest occupied room, or an ambiguity conflict.
    let mut present: Vec<&RoomSummary> = covered
        .iter()
        .copied()
        .filter(|room| room.presence_probability >= presence_threshold)
        .collect();
    present.sort_by(|a, b| {
        b.presence_probability
            .partial_cmp(&a.presence_probability)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.room.cmp(&b.room))
    });

    let mut attribution = None;
    let mut conflicts = Vec::new();
    if let Some(top) = present.first() {
        let mut candidates: Vec<(RoomId, f64)> = present
            .iter()
            .filter(|room| top.presence_probability - room.presence_probability < room_gap)
            .map(|room| (room.room.clone(), room.presence_probability))
            .collect();
        candidates.sort_by(|a, b| a.0.cmp(&b.0));
        if candidates.len() >= 2 {
            conflicts.push(Conflict::AmbiguousRooms {
                floor: floor.clone(),
                candidates,
            });
        } else {
            attribution = Some(top.room.clone());
        }
    }

    FloorSummary {
        floor,
        presence_probability,
        count,
        count_confidence,
        count_pmf,
        rooms: ordered,
        attribution,
        conflicts,
    }
}

/// Convolve two bounded count PMFs, truncating mass beyond `max`
/// (`fusion-sync.md` §2.5, floor count).
#[must_use]
pub fn convolve(a: &CountPmf, b: &CountPmf, max: usize) -> CountPmf {
    let mut probs = vec![0.0_f64; max + 1];
    for (i, pa) in a.probs.iter().enumerate() {
        if i > max {
            break;
        }
        for (j, pb) in b.probs.iter().enumerate() {
            let k = i + j;
            if k > max {
                break;
            }
            probs[k] += pa * pb;
        }
    }
    CountPmf { probs }.normalised()
}

/// A per-room vitals estimate for floor aggregation.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomVitals {
    /// The room.
    pub room: RoomId,
    /// The room's breathing estimate, when one was produced.
    pub breathing: Option<Estimate>,
    /// The room's heart-rate estimate, when one was produced.
    pub heart_rate: Option<Estimate>,
}

/// The floor's aggregated vitals: the highest-confidence room estimate per
/// band, with the room it came from (`fusion-sync.md` §2.2).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FloorVitals {
    /// The selected breathing estimate and its source room.
    pub breathing: Option<(RoomId, Estimate)>,
    /// The selected heart-rate estimate and its source room.
    pub heart_rate: Option<(RoomId, Estimate)>,
}

/// Aggregate floor vitals by **highest-confidence selection** per band.
///
/// Each band's estimate is the most confident room estimate whose confidence
/// is at least `min_confidence`; ties break on the lower room id, so the result
/// is order-independent and reproducible. A band with no estimate clears the
/// floor is absent — vitals never fabricate a rate from absent evidence
/// (`fusion-sync.md` §2.2).
#[must_use]
pub fn aggregate_floor_vitals(rooms: &[RoomVitals], min_confidence: f64) -> FloorVitals {
    let best = |select: fn(&RoomVitals) -> Option<Estimate>| -> Option<(RoomId, Estimate)> {
        let mut valid: Vec<(RoomId, Estimate)> = rooms
            .iter()
            .filter_map(|room| select(room).map(|estimate| (room.room.clone(), estimate)))
            .filter(|(_, estimate)| {
                estimate.confidence.is_finite() && f64::from(estimate.confidence) >= min_confidence
            })
            .collect();
        valid.sort_by(|a, b| {
            b.1.confidence
                .partial_cmp(&a.1.confidence)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });
        valid.into_iter().next()
    };
    FloorVitals {
        breathing: best(|room| room.breathing),
        heart_rate: best(|room| room.heart_rate),
    }
}
