"""Two-node hardware M2: per-packet cross-node phase residual, on real hardware.

INTEGRATION.md §5, measured on the ESP32-S3 rather than simulated. Two nodes
sit on the same WiFi channel in promiscuous mode, so both hear the same ambient
802.11 frames. This module pairs the frames the two nodes captured of the *same
transmitted packet*, then measures how far the two nodes' phases for that
packet drift apart once the carrier-frequency offset and its drift are removed.

The residual is the decisive quantity: below a quarter of a radian the two
nodes' phases are coherent enough for closure-phase tomography, above one
radian they are not (`m2_characterization.py` defines the bands).

HOW THE PACKETS ARE PAIRED
--------------------------
Pairing is exact, not inferred. The node reports the MAC address of the
station that transmitted each captured frame and the 802.11 sequence number
of that frame, so two frames of the same transmission share both. The
remaining check is the arrival time: both nodes capture one transmission
within microseconds of each other and each forwards it to the core
immediately, so the two frames reach the core within a millisecond of each
other. That bound is kept even though the sequence number is the stronger
key, because the sequence number is twelve bits and wraps.

Earlier attempts to pair on arrival time and received power alone did not
discriminate: the access point's beacons from two access points interleave at
about half the beacon interval, and every beacon of one access point arrives
at nearly the same power, so both heuristics paired the wrong frames and the
residual came out uniformly random. The transmitter's address and the
sequence number are what make the pairing exact.

WHAT THE RESIDUAL IS
---------------------
For one link (one transmitter, one receiving node) and one subcarrier, the
measured phase is the static path phase plus the carrier-frequency drift plus
the phase noise. The residual is what is left after the drift *and* the
constant are removed: the constant is the static path phase, whose value
depends on the path length modulo a wavelength and is therefore arbitrary and
different for every link. Leaving it in would measure where the path length
happens to fall rather than how well the phase is held. `m2_metric_audit.py`
shows that mistake flipping the simulated verdict from green to red.

Frames are compared only within one CSI layout. The ESP32-S3 reports either
64 subcarriers (the legacy long training field alone) or 128 (that field
followed by the high-throughput one), and index *k* is a different physical
subcarrier in the two, so a phase must never be compared across layouts.

Australian English spelling throughout.
"""

from __future__ import annotations

import cmath
import math
import statistics
import time
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Sequence, Tuple

from hardware import CsiCollector, CsiFrame, frames_by_node

# The M2 operational bands (the same thresholds the simulation uses).
GREEN_MAX_RAD = 0.25
AMBER_MAX_RAD = 1.00

# The window, in microseconds, within which the two nodes' captures of one
# transmission may lie. The nodes capture within microseconds of each other
# and forward immediately, so a millisecond is generous; the transmitters
# involved send tens of times a second, so the window stays unambiguous.
MATCH_WINDOW_US = 4000

# The smallest number of paired packets that makes a residual meaningful.
MIN_MATCHED_PAIRS = 20

# The amplitude below which a subcarrier carries no phase. The ESP32-S3 leaves
# null subcarriers in the payload, and a null subcarrier's phase is a constant
# zero, which would flatter the residual.
MIN_SIGNAL = 0.05

# The share of pairs a subcarrier must carry signal on for it to be measured.
MIN_SIGNAL_SHARE = 0.9


@dataclass
class Pairing:
    """The frames two nodes captured of the same transmitted packets."""

    transmitter: str
    layout: int
    pairs: List[Tuple[CsiFrame, CsiFrame]] = field(default_factory=list)
    candidates: int = 0
    ambiguous: int = 0


@dataclass
class LinkResult:
    """The cross-node phase residual for one link and one subcarrier."""

    transmitter: str
    layout: int
    subcarrier: int
    n_pairs: int
    p95_rad: float
    median_rad: float
    offset_rad: float
    drift_rad_per_s: float
    concentration: float
    # The share of pairs whose phase difference sits near zero rather than
    # near pi, and the spread within each of the two groups.
    sign_agreement: float = 0.0
    conditioned_p95_rad: float = 0.0
    agree_centre_rad: float = 0.0
    disagree_centre_rad: float = 0.0
    ambiguity: bool = False


@dataclass
class PairResult:
    """The cross-node phase residual for one pair of nodes."""

    node_a: int
    node_b: int
    n_pairs: int
    links: List[LinkResult] = field(default_factory=list)
    p95_rad: float = 0.0
    band: str = "red"
    control_p95_rad: float = 0.0
    ambiguity: bool = False
    sign_agreement: float = 0.0
    conditioned_p95_rad: float = 0.0
    agree_centre_rad: float = 0.0
    disagree_centre_rad: float = 0.0
    notes: List[str] = field(default_factory=list)

    @property
    def passed(self) -> bool:
        """Whether the pair is below the red band."""
        return self.band != "red"


@dataclass
class TwoNodeReport:
    """The whole two-node measurement."""

    node_ids: Tuple[int, ...]
    duration_s: float
    frames_per_node: Dict[int, int] = field(default_factory=dict)
    transmitters: Dict[int, List[str]] = field(default_factory=dict)
    results: List[PairResult] = field(default_factory=list)
    worst_band: str = "red"
    notes: List[str] = field(default_factory=list)

    @property
    def bcpt_cleared(self) -> bool:
        """Whether every measured pair is below the red band."""
        return self.worst_band != "red"


# ---------------------------------------------------------------------------
# Pairing
# ---------------------------------------------------------------------------


def match_same_packet(
    frames_a: Sequence[CsiFrame], frames_b: Sequence[CsiFrame]
) -> Tuple[List[Tuple[CsiFrame, CsiFrame]], int, int]:
    """Pair the frames two nodes captured of the same transmission.

    The pairing is exact where it can be: the node reports the 802.11
    sequence number of each frame, so two frames of the same transmission
    share it, and the pair is accepted only when the two frames also arrived
    within the window. The time bound is kept even though the sequence number
    is the stronger key, because the sequence number is twelve bits and wraps.

    Frames whose sequence number the node did not report fall back to the
    arrival-time window alone, and are accepted only when exactly one
    candidate on each side falls in it: an ambiguous frame is dropped rather
    than paired with a guess.

    Returns the pairs, the number of candidates and the number of ambiguous
    frames.
    """
    pairs: List[Tuple[CsiFrame, CsiFrame]] = []
    used_b: set[int] = set()
    candidates = 0
    ambiguous = 0

    # Index node B's frames by sequence number for the exact pairing.
    by_sequence: Dict[int, List[int]] = {}
    for index, frame in enumerate(frames_b):
        if frame.rx_seq is not None:
            by_sequence.setdefault(frame.rx_seq, []).append(index)

    times_b = [frame.host_arrival_us for frame in frames_b]
    for frame_a in frames_a:
        if frame_a.rx_seq is not None:
            matches = [
                index
                for index in by_sequence.get(frame_a.rx_seq, [])
                if index not in used_b
                and abs(times_b[index] - frame_a.host_arrival_us)
                <= MATCH_WINDOW_US
            ]
        else:
            matches = []
        if not matches:
            # No sequence number: fall back to the arrival-time window.
            matches = [
                index
                for index, moment in enumerate(times_b)
                if index not in used_b
                and abs(moment - frame_a.host_arrival_us) <= MATCH_WINDOW_US
            ]
        if len(matches) != 1:
            if len(matches) > 1:
                ambiguous += 1
            continue
        index_b = matches[0]
        candidates += 1
        used_b.add(index_b)
        pairs.append((frame_a, frames_b[index_b]))

    return pairs, candidates, ambiguous


def group_by_transmitter(
    frames: Sequence[CsiFrame],
) -> Dict[str, List[CsiFrame]]:
    """Group one node's frames by the station that transmitted them."""
    grouped: Dict[str, List[CsiFrame]] = {}
    for frame in frames:
        if frame.source_mac is None:
            continue
        grouped.setdefault(frame.source_mac, []).append(frame)
    return grouped


def group_by_layout(frames: Sequence[CsiFrame]) -> Dict[int, List[CsiFrame]]:
    """Group one node's frames by their CSI layout (the subcarrier count).

    The ESP32-S3 reports either 64 subcarriers or 128, and index *k* is a
    different physical subcarrier in the two, so a phase is never compared
    across layouts.
    """
    grouped: Dict[int, List[CsiFrame]] = {}
    for frame in frames:
        grouped.setdefault(frame.n_subcarriers, []).append(frame)
    return grouped


# ---------------------------------------------------------------------------
# Residual
# ---------------------------------------------------------------------------


def _unwrap(values: Sequence[float]) -> List[float]:
    """Unwrap a phase series so it can be fitted as a straight line."""
    if not values:
        return []
    two_pi = 2.0 * math.pi
    unwrapped = [values[0]]
    for index in range(1, len(values)):
        current = values[index]
        rotations = round((unwrapped[-1] - current) / two_pi)
        unwrapped.append(current + rotations * two_pi)
    return unwrapped


def _fit_linear(
    times_s: Sequence[float], values: Sequence[float]
) -> Tuple[float, float]:
    """Fit value = intercept + slope * time."""
    count = len(times_s)
    if count < 2:
        return (values[0] if values else 0.0, 0.0)
    mean_t = sum(times_s) / count
    mean_y = sum(values) / count
    denominator = sum((t - mean_t) ** 2 for t in times_s)
    if denominator == 0.0:
        return mean_y, 0.0
    slope = (
        sum((t - mean_t) * (y - mean_y) for t, y in zip(times_s, values))
        / denominator
    )
    return mean_y - slope * mean_t, slope


def measure_link(
    transmitter: str,
    layout: int,
    pairs: Sequence[Tuple[CsiFrame, CsiFrame]],
    subcarrier: int,
) -> Optional[LinkResult]:
    """Measure the cross-node phase residual for one link and subcarrier.

    The residual is the difference of the two nodes' measured phases for the
    same packet, with the carrier-frequency drift and the constant offset
    removed.
    """
    differences: List[float] = []
    times_s: List[float] = []
    for frame_a, frame_b in pairs:
        if subcarrier >= len(frame_a.csi) or subcarrier >= len(frame_b.csi):
            continue
        value_a = frame_a.csi[subcarrier]
        value_b = frame_b.csi[subcarrier]
        if abs(value_a) < MIN_SIGNAL or abs(value_b) < MIN_SIGNAL:
            continue
        differences.append(cmath.phase(value_a) - cmath.phase(value_b))
        times_s.append(frame_a.host_arrival_us / 1_000_000.0)

    if len(differences) < MIN_MATCHED_PAIRS:
        return None

    unwrapped = _unwrap(differences)
    intercept, slope = _fit_linear(times_s, unwrapped)
    residuals = [
        cmath.phase(value - (intercept + slope * moment))
        for moment, value in zip(times_s, unwrapped)
    ]

    magnitudes = sorted(abs(value) for value in residuals)
    index = int(math.ceil(0.95 * len(magnitudes))) - 1
    index = max(0, min(index, len(magnitudes) - 1))
    vector = sum(cmath.exp(1j * value) for value in residuals)

    # The ESP32-S3 negates the whole channel vector from frame to frame, so
    # the absolute phase of a frame is only known modulo pi. When that is what
    # is happening, the residual is not a spread but two groups: pairs whose
    # signs agree and pairs whose signs do not. The group centres say how far
    # the two nodes agree once the sign is known, and the spread within each
    # group says how tight that agreement is.
    near_zero = [r for r in residuals if abs(r) < 0.5]
    near_pi = [r for r in residuals if abs(abs(r) - math.pi) < 0.5]
    sign_agreement = len(near_zero) / len(residuals) if residuals else 0.0
    ambiguity = len(near_zero) + len(near_pi) >= 0.9 * len(residuals)

    def _centre(group: Sequence[float]) -> float:
        if not group:
            return 0.0
        return cmath.phase(
            sum(cmath.exp(1j * value) for value in group) / len(group)
        )

    def _spread(group: Sequence[float]) -> float:
        if not group:
            return 0.0
        centre = _centre(group)
        magnitudes = sorted(abs(cmath.phase(value - centre)) for value in group)
        index = int(math.ceil(0.95 * len(magnitudes))) - 1
        return magnitudes[max(0, min(index, len(magnitudes) - 1))]

    agree_centre = _centre(near_zero)
    disagree_centre = _centre(near_pi)
    conditioned = max(_spread(near_zero), _spread(near_pi))

    return LinkResult(
        transmitter=transmitter,
        layout=layout,
        subcarrier=subcarrier,
        n_pairs=len(differences),
        p95_rad=magnitudes[index],
        median_rad=statistics.median(magnitudes),
        offset_rad=cmath.phase(intercept),
        drift_rad_per_s=slope,
        concentration=abs(vector) / len(residuals),
        sign_agreement=sign_agreement,
        conditioned_p95_rad=conditioned,
        agree_centre_rad=agree_centre,
        disagree_centre_rad=disagree_centre,
        ambiguity=ambiguity,
    )


def usable_subcarriers(
    pairs: Sequence[Tuple[CsiFrame, CsiFrame]],
) -> List[int]:
    """The subcarriers that carried signal on almost every paired frame.

    A null subcarrier's phase is a constant, which would make the residual
    look perfect without measuring anything.
    """
    if not pairs:
        return []
    first_a, first_b = pairs[0]
    count = min(len(first_a.csi), len(first_b.csi))
    usable: List[int] = []
    for subcarrier in range(count):
        signalled = 0
        for frame_a, frame_b in pairs:
            value_a = frame_a.csi[subcarrier]
            value_b = frame_b.csi[subcarrier]
            if abs(value_a) >= MIN_SIGNAL and abs(value_b) >= MIN_SIGNAL:
                signalled += 1
        if signalled >= MIN_SIGNAL_SHARE * len(pairs):
            usable.append(subcarrier)
    return usable


def measure_pair(
    node_a: int,
    node_b: int,
    frames_a: Sequence[CsiFrame],
    frames_b: Sequence[CsiFrame],
) -> PairResult:
    """Measure the cross-node phase residual between two nodes."""
    result = PairResult(node_a=node_a, node_b=node_b, n_pairs=0)

    transmitters_a = group_by_transmitter(frames_a)
    transmitters_b = group_by_transmitter(frames_b)
    common = sorted(set(transmitters_a) & set(transmitters_b))

    if not common:
        result.notes.append(
            "the two nodes share no transmitter: the node reports no source "
            "address, so the frames cannot be paired"
        )
        return result

    best_p95 = 0.0
    total_pairs = 0
    for transmitter in common:
        layouts_a = group_by_layout(transmitters_a[transmitter])
        layouts_b = group_by_layout(transmitters_b[transmitter])
        for layout in sorted(set(layouts_a) & set(layouts_b)):
            pairs, candidates, ambiguous = match_same_packet(
                layouts_a[layout], layouts_b[layout]
            )
            total_pairs += len(pairs)
            if len(pairs) < MIN_MATCHED_PAIRS:
                continue
            for subcarrier in usable_subcarriers(pairs):
                link = measure_link(transmitter, layout, pairs, subcarrier)
                if link is not None:
                    result.links.append(link)
                    best_p95 = max(best_p95, link.p95_rad)
            result.notes.append(
                f"transmitter {transmitter}, {layout} subcarriers: "
                f"{len(pairs)} packets paired ({candidates} candidates, "
                f"{ambiguous} ambiguous dropped)"
            )

    result.n_pairs = total_pairs
    result.p95_rad = best_p95
    if not result.links:
        result.notes.append("no subcarrier carried enough signal to measure")
        return result

    if result.p95_rad < GREEN_MAX_RAD:
        result.band = "green"
    elif result.p95_rad < AMBER_MAX_RAD:
        result.band = "amber"
    else:
        result.band = "red"

    # Is the residual a sign ambiguity rather than a spread? If it is, the
    # measurement is limited by the ESP32-S3's per-frame sign flip, and the
    # residual conditioned on the sign is the real noise.
    ambiguous_links = [link for link in result.links if link.ambiguity]
    if ambiguous_links:
        result.ambiguity = True
        result.sign_agreement = statistics.mean(
            link.sign_agreement for link in ambiguous_links
        )
        result.conditioned_p95_rad = max(
            link.conditioned_p95_rad for link in ambiguous_links
        )
        result.agree_centre_rad = statistics.mean(
            link.agree_centre_rad for link in ambiguous_links
        )
        result.disagree_centre_rad = statistics.mean(
            link.disagree_centre_rad for link in ambiguous_links
        )
        result.notes.append(
            f"the residual is a sign ambiguity, not a spread: "
            f"{len(ambiguous_links)} of {len(result.links)} subcarriers sit "
            f"at zero or pi, with the signs agreeing on "
            f"{result.sign_agreement:.0%} of pairs; the agreeing group is "
            f"centred at {result.agree_centre_rad:+.4f} rad and the "
            f"disagreeing group at {result.disagree_centre_rad:+.4f} rad, "
            f"each with a p95 spread of {result.conditioned_p95_rad:.4f} rad"
        )

    # The control: pair the two nodes' frames across *different* transmitters.
    # The residual must be much larger, or the pairing is not what the result
    # depends on.
    control = _cross_transmitter_control(transmitters_a, transmitters_b, common)
    if control is not None:
        result.control_p95_rad = control
    return result


def _cross_transmitter_control(
    transmitters_a: Dict[str, List[CsiFrame]],
    transmitters_b: Dict[str, List[CsiFrame]],
    common: Sequence[str],
) -> Optional[float]:
    """The residual when the two nodes' frames come from different senders.

    This is the check that the result depends on pairing the same packet:
    mixing transmitters must destroy the residual.
    """
    if len(common) < 2:
        return None
    mac_a, mac_b = common[0], common[1]
    layout_a = group_by_layout(transmitters_a[mac_a])
    layout_b = group_by_layout(transmitters_b[mac_b])
    for layout in sorted(set(layout_a) & set(layout_b)):
        pairs, _candidates, _ambiguous = match_same_packet(
            layout_a[layout], layout_b[layout]
        )
        if len(pairs) < MIN_MATCHED_PAIRS:
            continue
        worst = 0.0
        for subcarrier in usable_subcarriers(pairs):
            link = measure_link("control", layout, pairs, subcarrier)
            if link is not None:
                worst = max(worst, link.p95_rad)
        return worst
    return None


# ---------------------------------------------------------------------------
# The run
# ---------------------------------------------------------------------------


def run_two_node_measurement(
    duration_s: float = 120.0,
    collector: Optional[CsiCollector] = None,
) -> TwoNodeReport:
    """Collect from the live nodes and measure the cross-node residual.

    Needs at least two nodes streaming to the control host; with fewer the
    report is red, because the cross-node residual is not measurable at all.
    """
    owned = collector is None
    if collector is None:
        collector = CsiCollector(bind_port=9100)
    collector.start()
    try:
        time.sleep(2.0)  # discovery window
        frames = collector.collect_for(duration_s)
    finally:
        if owned:
            collector.stop()

    grouped = frames_by_node(frames)
    node_ids = tuple(sorted(grouped))
    report = TwoNodeReport(
        node_ids=node_ids,
        duration_s=duration_s,
        frames_per_node={nid: len(grouped[nid]) for nid in node_ids},
    )

    for node_id in node_ids:
        transmitters = group_by_transmitter(grouped[node_id])
        report.transmitters[node_id] = sorted(
            transmitters, key=lambda mac: -len(transmitters[mac])
        )[:6]

    if len(node_ids) < 2:
        report.notes.append(
            f"only {len(node_ids)} node(s) streaming; a cross-node "
            "residual needs two"
        )
        report.worst_band = "red"
        return report

    for node_id in node_ids:
        without = sum(
            1 for frame in grouped[node_id] if frame.source_mac is None
        )
        if without:
            report.notes.append(
                f"node {node_id}: {without} frames report no transmitter "
                "address; the node is running firmware older than wire minor 1"
            )

    bands: List[str] = []
    for first in range(len(node_ids)):
        for second in range(first + 1, len(node_ids)):
            node_a = node_ids[first]
            node_b = node_ids[second]
            result = measure_pair(
                node_a, node_b, grouped[node_a], grouped[node_b]
            )
            report.results.append(result)
            bands.append(result.band)

    if "red" in bands:
        report.worst_band = "red"
    elif "amber" in bands:
        report.worst_band = "amber"
    else:
        report.worst_band = "green"
    return report


def format_report(report: TwoNodeReport) -> str:
    """Render the report for the operator."""
    lines = [
        "=== Aquila two-node hardware M2 ===",
        f"nodes {report.node_ids} over {report.duration_s:.0f}s",
        f"frames: {report.frames_per_node}",
        "",
    ]
    for node_id, transmitters in report.transmitters.items():
        lines.append(f"  node {node_id} hears: {', '.join(transmitters)}")
    lines.append("")
    for result in report.results:
        lines.append(
            f"  node {result.node_a} <-> node {result.node_b}: "
            f"band={result.band} p95={result.p95_rad:.4f} rad "
            f"(cross-transmitter control {result.control_p95_rad:.4f} rad) "
            f"pairs={result.n_pairs} links={len(result.links)}"
        )
        if result.ambiguity:
            lines.append(
                f"      SIGN AMBIGUITY: signs agree on "
                f"{result.sign_agreement:.0%} of pairs; conditioned on the "
                f"sign the p95 is {result.conditioned_p95_rad:.4f} rad"
            )
        for note in result.notes:
            lines.append(f"      {note}")
        if result.links:
            worst = max(result.links, key=lambda link: link.p95_rad)
            best = min(result.links, key=lambda link: link.p95_rad)
            lines.append(
                f"      worst subcarrier sc{worst.subcarrier} "
                f"(layout {worst.layout}): p95={worst.p95_rad:.4f} rad, "
                f"offset={worst.offset_rad:+.4f} rad, "
                f"drift={worst.drift_rad_per_s:+.4f} rad/s, "
                f"concentration={worst.concentration:.4f}"
            )
            lines.append(
                f"      best subcarrier sc{best.subcarrier}: "
                f"p95={best.p95_rad:.4f} rad"
            )
    lines.append("")
    lines.append(f"worst band: {report.worst_band}")
    lines.append(
        "BCPT cleared over the air"
        if report.bcpt_cleared
        else "BCPT hardware-gated: the cross-node phase residual is in the red band"
    )
    for note in report.notes:
        lines.append(note)
    return "\n".join(lines)


if __name__ == "__main__":
    import sys

    duration = float(sys.argv[1]) if len(sys.argv) > 1 else 120.0
    measurement = run_two_node_measurement(duration_s=duration)
    print(format_report(measurement))
    sys.exit(0 if measurement.bcpt_cleared else 1)
