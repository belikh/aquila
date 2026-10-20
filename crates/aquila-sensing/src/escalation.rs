//! Zone → home escalation.
//!
//! Slice 1 emits zone-scope presence and count: a zone is the detection region
//! (for example "bed" inside the bedroom, or "shower" versus "toilet" inside
//! the bathroom). Slice 6 grows the hierarchy zone → home:
//! [`aggregate_home`] composes the zone summaries the same way, so the whole
//! home is presence, count and coarse location across zones with cross-zone
//! fusion intact. The hierarchy is data (from the topology), never hard-coded.
//!
//! The pooling rules are the documented ones (`fusion-sync.md` §2.5):
//!
//! - **Zone presence** is a **soft-OR** across the zone's covered nodes,
//!   `p_zone = 1 − Π_nodes (1 − p_node)`: nodes are independent presence
//!   witnesses, so presence is present if any node is.
//! - **Zone count** is the **convolution** of the zone's node count PMFs
//!   (`P_zone = P_n1 ⊛ P_n2 ⊛ …`), then MAP + confidence. Convolution is used
//!   only across a spatial boundary; nodes *within* a zone are mixed, not summed
//!   (§2.2). A node with no usable evidence contributes no count: the zone count
//!   is reported `Unknown`, never fabricated (§2.5, §2.6).
//! - **Zone vitals** are aggregated by **highest-confidence selection** per
//!   band across the zone's nodes — never a coherent phase combination
//!   (§2.2, §3). A band with no estimate above the confidence floor is absent.
//!
//! Slice 6 closes the hierarchy zone → home (`roadmap.md`, Slice 6):
//! [`aggregate_home`] composes the zone summaries the same way, so the whole
//! home is presence, count and coarse location across zones with cross-zone
//! fusion intact. The rules are the same shape, one tier up:
//!
//! - **Home presence** is a **soft-OR** over zones,
//!   `p_home = 1 − Π_zones (1 − p_zone)` (§2.5).
//! - **Home count** is the **convolution** of the zone count PMFs, then MAP +
//!   confidence. Any zone whose count is Unknown makes the home count Unknown,
//!   never fabricated (§2.5).
//! - **Whole-home location** is the attributed zone; it is withheld when two
//!   strongest zones are ambiguous ([`Conflict::AmbiguousZonesHome`]) while the
//!   home's presence and count still report.
//!
//! # Unknown is first-class
//!
//! A zone with no usable evidence is [`ZoneCoverage::Uncovered`]: its presence
//! and count are Unknown, never `Absent`/`0`, and an uncovered zone forces the
//! zone count to Unknown with a coverage note (`fusion-sync.md` §2.5). A zone
//! with no covered node forces the zone count to Unknown the same way.
//! Absence and unknown are never conflated.
//!
//! # Attribution is reported, not guessed
//!
//! When the zone is occupied and the two strongest nodes carry similar
//! presence probability (within [`Conflict::AmbiguousZones`]' `node_gap`), the
//! zone's node attribution is withheld and the conflict is surfaced with each
//! node's confidence — never a false confident node (`fusion-sync.md` §2.4.1,
//! `risks.md` R-01/Q-01). The zone's aggregate presence and convolved count are
//! unaffected, because they do not depend on attributing presence to a single
//! node: the ambiguity is about *which* node, not *whether* the zone is
//! occupied.
//!
//! The same rule holds at home scope: when two zones are near-tied, the
//! whole-home location is withheld ([`Conflict::AmbiguousZonesHome`]) while the
//! home's presence and count still report.

use std::cmp::Ordering;

use aquila_types::{Estimate, NodeId, ZoneId};

use crate::fusion::CountPmf;

/// Why a node contributes (or does not) to a zone's aggregate count.
///
/// Coverage is separate from the count value: an uncovered zone is Unknown,
/// never a fabricated `0` (`fusion-sync.md` §2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneCoverage {
    /// The zone had usable, calibrated evidence this window.
    Covered,
    /// The zone had no usable evidence; its count is Unknown.
    Uncovered,
}

/// The per-zone summary the escalation step consumes.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneSummary {
    /// The zone.
    pub zone: ZoneId,
    /// The pooled presence probability in `[0, 1]`.
    pub presence_probability: f64,
    /// The MAP people count.
    pub count: u8,
    /// The zone's count PMF, for the zone convolution.
    pub count_pmf: CountPmf,
    /// Whether the zone carried usable evidence this window.
    pub coverage: ZoneCoverage,
}

impl ZoneSummary {
    /// A covered zone summary from its pooled presence probability and count
    /// PMF. The MAP count is derived from the PMF, so the two cannot disagree.
    #[must_use]
    pub fn covered(zone: ZoneId, presence_probability: f64, count_pmf: CountPmf) -> Self {
        let (count, _) = count_pmf.map();
        Self {
            zone,
            presence_probability: presence_probability.clamp(0.0, 1.0),
            count,
            count_pmf,
            coverage: ZoneCoverage::Covered,
        }
    }

    /// An uncovered zone summary: presence and count are Unknown
    /// (`ZoneCoverage::Uncovered`, `fusion-sync.md` §2.5). The retained PMF is a
    /// broad, uninformative placeholder; [`ZoneCoverage::Uncovered`] is what
    /// forces the zone count to Unknown.
    #[must_use]
    pub fn uncovered(zone: ZoneId, max_count: usize) -> Self {
        let uniform = 1.0 / (max_count + 1) as f64;
        Self {
            zone,
            presence_probability: 0.0,
            count: 0,
            count_pmf: CountPmf {
                probs: vec![uniform; max_count + 1],
            },
            coverage: ZoneCoverage::Uncovered,
        }
    }
}

/// A conflict surfaced by escalation. Conflicts are **reported, not averaged
/// away** (`fusion-sync.md` §2.4).
#[derive(Debug, Clone, PartialEq)]
pub enum Conflict {
    /// Zone attribution is ambiguous: the two strongest zone presence
    /// probabilities lie within `node_gap`, so naming a single zone
    /// would be a false confident claim (`fusion-sync.md` §2.4.1).
    AmbiguousZones {
        /// The zone whose attribution is unresolved.
        zone: ZoneId,
        /// Each contending zone and its pooled presence probability, in
        /// canonical zone-id order.
        candidates: Vec<(ZoneId, f64)>,
    },
    /// Whole-home location is ambiguous: the two strongest zone presence
    /// probabilities lie within the home ambiguity margin, so naming a single
    /// zone would be a false confident claim (`fusion-sync.md` §2.4.1,
    /// `geometry-calibration.md` §4 staging). The home's aggregate presence and
    /// convolved count are unaffected.
    AmbiguousZonesHome {
        /// Each contending zone and its soft-OR presence probability, in
        /// canonical zone-id order.
        candidates: Vec<(ZoneId, f64)>,
    },
}

/// The aggregated state of one zone for one window.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneAggregate {
    /// The zone.
    pub zone: ZoneId,
    /// The soft-OR presence probability across the zone's covered nodes;
    /// `None` when no node in the zone has usable evidence
    /// (`Occupancy::Unknown`, never `Absent`).
    pub presence_probability: Option<f64>,
    /// The MAP zone count from the node-PMF convolution, or `None` when the
    /// zone count is Unknown (an uncovered node, or no covered nodes).
    pub count: Option<u8>,
    /// The convolved count PMF's MAP probability; `0.0` when the count is
    /// Unknown.
    pub count_confidence: f64,
    /// The convolved zone count PMF, or `None` when Unknown.
    pub count_pmf: Option<CountPmf>,
    /// The zones aggregated, canonical zone-id order.
    pub nodes: Vec<ZoneId>,
    /// The confidently attributed zone, or `None` when the zone is unoccupied
    /// or the attribution is ambiguous.
    pub attribution: Option<ZoneId>,
    /// Every conflict raised by this zone this window.
    pub conflicts: Vec<Conflict>,
}

/// Zone occupancy: the soft-OR `1 − Π(1 − p_node)` across the given nodes'
/// presence probabilities (`fusion-sync.md` §2.5).
///
/// Only [`ZoneCoverage::Covered`] nodes contribute: an uncovered node is
/// Unknown, not a `p = 0` witness. With no covered nodes this returns `0.0`; a
/// caller that needs to distinguish "unknown" from "absent" should use
/// [`aggregate_zone`], whose `presence_probability` is `Option`.
#[must_use]
pub fn zone_presence_probability(nodes: &[ZoneSummary]) -> f64 {
    let mut complement = 1.0_f64;
    for node in nodes
        .iter()
        .filter(|node| node.coverage == ZoneCoverage::Covered)
    {
        complement *= 1.0 - node.presence_probability.clamp(0.0, 1.0);
    }
    (1.0 - complement).clamp(0.0, 1.0)
}

/// Home occupancy: a soft-OR over zones, each already a soft-OR over nodes.
#[must_use]
pub fn home_presence_probability(zones: &[f64]) -> f64 {
    let mut complement = 1.0_f64;
    for p in zones {
        complement *= 1.0 - p.clamp(0.0, 1.0);
    }
    (1.0 - complement).clamp(0.0, 1.0)
}

/// Aggregate one zone from its node summaries (`fusion-sync.md` §2.5).
///
/// `node_gap` is the attribution ambiguity margin; `presence_threshold` is the
/// presence probability above which a node counts as occupied for attribution
/// (`p_hi`); `max_count` bounds the convolved PMF.
///
/// Determinism: nodes are processed in canonical node-id order, so the result
/// is independent of the order `nodes` is supplied in.
#[must_use]
pub fn aggregate_zone(
    zone: ZoneId,
    nodes: &[ZoneSummary],
    node_gap: f64,
    presence_threshold: f64,
    max_count: usize,
) -> ZoneAggregate {
    let mut ordered: Vec<ZoneSummary> = nodes.to_vec();
    ordered.sort_by(|a, b| a.zone.cmp(&b.zone));

    let covered: Vec<&ZoneSummary> = ordered
        .iter()
        .filter(|node| node.coverage == ZoneCoverage::Covered)
        .collect();
    let all_covered = covered.len() == ordered.len() && !ordered.is_empty();

    // Presence: soft-OR over the covered nodes only.
    let presence_probability = if covered.is_empty() {
        None
    } else {
        Some(zone_presence_probability(&ordered))
    };

    // Count: convolution over the covered nodes. Any uncovered node makes the
    // zone count Unknown rather than treating a broad, uninformative factor as
    // evidence (`fusion-sync.md` §2.5).
    let count_pmf = if !all_covered {
        None
    } else {
        let mut acc: Option<CountPmf> = None;
        for node in &covered {
            acc = Some(match acc {
                None => node.count_pmf.clone(),
                Some(previous) => convolve(&previous, &node.count_pmf, max_count),
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

    // Attribution: the strongest occupied node, or an ambiguity conflict.
    let mut present: Vec<&ZoneSummary> = covered
        .iter()
        .copied()
        .filter(|node| node.presence_probability >= presence_threshold)
        .collect();
    present.sort_by(|a, b| {
        b.presence_probability
            .partial_cmp(&a.presence_probability)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.zone.cmp(&b.zone))
    });

    let mut attribution = None;
    let mut conflicts = Vec::new();
    if let Some(top) = present.first() {
        let mut candidates: Vec<(ZoneId, f64)> = present
            .iter()
            .filter(|node| top.presence_probability - node.presence_probability < node_gap)
            .map(|node| (node.zone.clone(), node.presence_probability))
            .collect();
        candidates.sort_by(|a, b| a.0.cmp(&b.0));
        if candidates.len() >= 2 {
            conflicts.push(Conflict::AmbiguousZones {
                zone: zone.clone(),
                candidates,
            });
        } else {
            attribution = Some(top.zone.clone());
        }
    }

    ZoneAggregate {
        zone,
        presence_probability,
        count,
        count_confidence,
        count_pmf,
        nodes: ordered.iter().map(|n| n.zone.clone()).collect(),
        attribution,
        conflicts,
    }
}

/// A confidently attributed whole-home coarse location: the zone, when the
/// zone's own attribution is confident (`fusion-sync.md` §2.5, home).
#[derive(Debug, Clone, PartialEq)]
pub struct HomeLocation {
    /// The attributed zone, or `None` when the zone attribution is ambiguous
    /// (zone-level accuracy only) — never a false confident zone
    /// (`fusion-sync.md` §2.4.1).
    pub zone: Option<ZoneId>,
}

/// The aggregated state of the whole home for one window.
#[derive(Debug, Clone, PartialEq)]
pub struct HomeSummary {
    /// The soft-OR presence probability across the zones' soft-OR
    /// probabilities; `None` when no zone has usable evidence
    /// (`Occupancy::Unknown`, never `Absent`).
    pub presence_probability: Option<f64>,
    /// The MAP home count from the zone-PMF convolution, or `None` when the
    /// home count is Unknown (any zone unknown, or no zones).
    pub count: Option<u8>,
    /// The convolved home count PMF's MAP probability; `0.0` when Unknown.
    pub count_confidence: f64,
    /// The convolved home count PMF, or `None` when Unknown.
    pub count_pmf: Option<CountPmf>,
    /// The zone summaries, canonical zone-id order.
    pub zones: Vec<ZoneAggregate>,
    /// The confidently attributed whole-home location, or `None` when the home
    /// is unoccupied or the attribution is ambiguous.
    pub attribution: Option<HomeLocation>,
    /// Every conflict raised at home scope this window.
    pub conflicts: Vec<Conflict>,
}

/// Aggregate the whole home from its zone summaries (`fusion-sync.md` §2.5).
///
/// Home presence soft-ORs the zones; home count convolves the zone count
/// PMFs; home location attributes the strongest occupied zone. `zone_gap` is
/// the cross-zone attribution ambiguity margin; `presence_threshold` is the
/// zone presence probability above which a zone counts as occupied for
/// attribution (`p_hi`); `max_count` bounds the convolved PMF.
///
/// Determinism: zones are processed in canonical zone-id order, so the result
/// is independent of the order `zones` is supplied in. An unknown zone count
/// makes the home count Unknown, never a fabricated `0` (§2.5, §2.6).
#[must_use]
pub fn aggregate_home(
    zones: &[ZoneAggregate],
    zone_gap: f64,
    presence_threshold: f64,
    max_count: usize,
) -> HomeSummary {
    let mut ordered: Vec<ZoneAggregate> = zones.to_vec();
    ordered.sort_by(|a, b| a.zone.cmp(&b.zone));

    // Presence: soft-OR over the zones that carry a known probability.
    let probabilities: Vec<f64> = ordered
        .iter()
        .filter_map(|zone| zone.presence_probability)
        .collect();
    let presence_probability = if probabilities.is_empty() {
        None
    } else {
        Some(home_presence_probability(&probabilities))
    };

    // Count: convolution over the zones. Any zone whose count is Unknown
    // forces the home count to Unknown rather than treating a broad,
    // uninformative factor as evidence (`fusion-sync.md` §2.5).
    let mut acc: Option<CountPmf> = None;
    let mut all_known = !ordered.is_empty();
    for zone in &ordered {
        match &zone.count_pmf {
            Some(pmf) => {
                acc = Some(match acc {
                    None => pmf.clone(),
                    Some(previous) => convolve(&previous, pmf, max_count),
                });
            }
            None => {
                all_known = false;
                break;
            }
        }
    }
    let count_pmf = if all_known { acc } else { None };
    let (count, count_confidence) = match &count_pmf {
        Some(pmf) => {
            let (count, confidence) = pmf.map();
            (Some(count), confidence)
        }
        None => (None, 0.0),
    };

    // Attribution: the strongest occupied zone.
    let mut present: Vec<(ZoneId, f64)> = ordered
        .iter()
        .filter_map(|zone| {
            zone.presence_probability
                .filter(|p| *p >= presence_threshold)
                .map(|p| (zone.zone.clone(), p))
        })
        .collect();
    present.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });

    let mut attribution = None;
    let mut conflicts = Vec::new();
    if let Some((top_zone, top_probability)) = present.first() {
        let mut candidates: Vec<(ZoneId, f64)> = present
            .iter()
            .filter(|(_, p)| top_probability - p < zone_gap)
            .map(|(zone, p)| (zone.clone(), *p))
            .collect();
        candidates.sort_by(|a, b| a.0.cmp(&b.0));
        if candidates.len() >= 2 {
            conflicts.push(Conflict::AmbiguousZonesHome { candidates });
        } else {
            attribution = Some(HomeLocation {
                zone: Some(top_zone.clone()),
            });
        }
    }

    HomeSummary {
        presence_probability,
        count,
        count_confidence,
        count_pmf,
        zones: ordered,
        attribution,
        conflicts,
    }
}

/// Convolve two bounded count PMFs, truncating mass beyond `max`
/// (`fusion-sync.md` §2.5, zone count).
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

/// A per-zone vitals estimate for home aggregation.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneVitals {
    /// The zone.
    pub zone: ZoneId,
    /// The zone's breathing estimate, when one was produced.
    pub breathing: Option<Estimate>,
    /// The zone's heart-rate estimate, when one was produced.
    pub heart_rate: Option<Estimate>,
}

/// The home's aggregated vitals: the highest-confidence zone estimate per
/// band, with the zone it came from (`fusion-sync.md` §2.2).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HomeVitals {
    /// The selected breathing estimate and its source zone.
    pub breathing: Option<(ZoneId, Estimate)>,
    /// The selected heart-rate estimate and its source zone.
    pub heart_rate: Option<(ZoneId, Estimate)>,
}

/// Aggregate home vitals by **highest-confidence selection** per band.
///
/// Each band's estimate is the most confident zone estimate whose confidence
/// is at least `min_confidence`; ties break on the lower zone id, so the result
/// is order-independent and reproducible. A band with no estimate clears the
/// home band — vitals never fabricate a rate from absent evidence
/// (`fusion-sync.md` §2.2).
#[must_use]
pub fn aggregate_home_vitals(zones: &[ZoneVitals], min_confidence: f64) -> HomeVitals {
    let best = |select: fn(&ZoneVitals) -> Option<Estimate>| -> Option<(ZoneId, Estimate)> {
        let mut valid: Vec<(ZoneId, Estimate)> = zones
            .iter()
            .filter_map(|zone| select(zone).map(|estimate| (zone.zone.clone(), estimate)))
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
    HomeVitals {
        breathing: best(|zone| zone.breathing),
        heart_rate: best(|zone| zone.heart_rate),
    }
}
