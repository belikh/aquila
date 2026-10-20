"""Two-node hardware M2: cross-node phase residual on real ESP32-S3 CSI.

INTEGRATION.md §5, measured on hardware rather than simulated. Two nodes sit
on the same WiFi channel in promiscuous mode, so both hear the same ambient
802.11 packets. This module matches the packets the two nodes saw and
measures the per-packet cross-node phase residual after CFO removal — the
decisive quantity for whether coherent BCPT is available (`m2_characterization.py`
defines the bands: green < 0.25 rad, amber < 1.0 rad, red above).

The hardware path differs from the simulation in one essential way: the
simulation knows which link produced which sample, whereas on hardware the
same ambient packet must be identified in each node's own stream. The
identification used here is the received-power signature: a packet's RSSI is
a property of the transmitter-receiver path, so the two nodes see nearly the
same value for the same packet, and the pairing is unambiguous when the
values are distinct.

Australian English spelling throughout.
"""

from __future__ import annotations

import cmath
import math
import statistics
import time
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Tuple

from hardware import CsiFrame, CsiCollector, frames_by_node

# The M2 operational bands (the same thresholds the simulation uses).
GREEN_MAX_RAD = 0.25
AMBER_MAX_RAD = 1.00

# The RSSI tolerance for calling two observations the same packet, in dB.
# The two nodes differ in position, so the same packet is not received at
# exactly the same power; a couple of dB covers that.
RSSI_MATCH_TOL_DB = 3.0

# The smallest amplitude-signature correlation for two observations to be
# accepted as the same packet. The same packet seen by two nodes shares its
# multipath amplitude pattern, so the normalised signature correlates even
# though the absolute power and the phase differ.
MIN_SIGNATURE_CORRELATION = 0.80

# The smallest complex-CSI correlation for two observations to be accepted as
# the same packet. The same packet seen by two nodes gives the same channel
# response up to a complex scalar (the power and the phase differ), so the
# normalised complex vectors correlate strongly even where the amplitude
# alone does not distinguish one packet from another.
MIN_COMPLEX_CORRELATION = 0.90

# The window, in microseconds, within which the host must have received the
# two nodes' observations of one packet. The host arrival time is the only
# clock the nodes share: each node stamps its own boot-relative
# `device_time_us`, but the collector stamps `host_arrival_us` on arrival,
# so the same ambient packet reaches the host from the two nodes within a
# few hundred microseconds of each other.
HOST_MATCH_WINDOW_US = 2000

# The subcarrier the residual is measured on. The ESP32-S3's long training
# field leaves a block of null subcarriers in the middle of the payload, and
# the first four bytes are nulled when the driver reports the first word
# invalid, so a subcarrier inside either region measures nothing at all.
# Subcarrier 10 sits in the middle of the first populated block, where the
# CSI is strong and varies from packet to packet.
DEFAULT_SUBCARRIER = 10

# The smallest number of matched packets that makes a residual meaningful.
MIN_MATCHED_PAIRS = 20


@dataclass
class PacketPair:
    """One ambient packet seen by both nodes."""

    rssi_a: int
    rssi_b: int
    index_a: int
    index_b: int
    device_time_a_us: int
    device_time_b_us: int


@dataclass
class ResidualResult:
    """The cross-node phase residual for one node pair."""

    node_a: int
    node_b: int
    n_matched: int
    n_candidates: int
    residuals_rad: List[float] = field(default_factory=list)
    median_rad: float = 0.0
    p95_rad: float = 0.0
    band: str = "red"
    cfo_rad_per_s: float = 0.0
    offset_rad: float = 0.0
    conjugate: bool = False
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
    frames_a: int
    frames_b: int
    results: List[ResidualResult] = field(default_factory=list)
    worst_band: str = "red"
    notes: List[str] = field(default_factory=list)

    @property
    def bcpt_cleared(self) -> bool:
        """Whether every measured pair is below the red band."""
        return self.worst_band != "red"


def _rssi_signature(frame: CsiFrame) -> int:
    """The received power of one frame, in dBm."""
    return frame.rssi_dbm


def _amplitude_signature(frame: CsiFrame) -> List[float]:
    """The frame's per-subcarrier amplitude, normalised to unit norm.

    The same ambient packet produces the same multipath pattern for every
    receiver, so the normalised pattern is a fingerprint of the packet that
    survives the difference in the receivers' positions. The absolute
    amplitude is discarded, because that is exactly what differs.
    """
    magnitudes = [abs(value) for value in frame.csi]
    norm = math.sqrt(sum(value * value for value in magnitudes))
    if norm == 0.0:
        return magnitudes
    return [value / norm for value in magnitudes]


def _signature_correlation(
    signature_a: List[float], signature_b: List[float]
) -> float:
    """The Pearson correlation of two amplitude signatures.

    Both signatures are already unit-norm, so this is their dot product.
    """
    length = min(len(signature_a), len(signature_b))
    if length == 0:
        return 0.0
    return sum(signature_a[index] * signature_b[index] for index in range(length))


def _complex_signature(frame: CsiFrame) -> List[complex]:
    """The frame's per-subcarrier complex CSI, normalised to unit norm.

    The same ambient packet produces the same channel response for every
    receiver, so the normalised complex vector correlates between the nodes
    even though the absolute amplitude and the phase differ. The amplitude
    alone is not enough: a long training field's spectrum is nearly flat, so
    its normalised magnitude is nearly the same for every packet.
    """
    values = [complex(value) for value in frame.csi]
    norm = math.sqrt(sum(abs(value) ** 2 for value in values))
    if norm == 0.0:
        return values
    return [value / norm for value in values]


def _complex_correlation(
    signature_a: List[complex], signature_b: List[complex]
) -> float:
    """The magnitude of the complex inner product of two signatures.

    Both signatures are unit-norm, so this is the correlation coefficient's
    magnitude: 1.0 means the two observations differ by a complex scalar
    alone, which is what the same packet looks like at two receivers.
    """
    length = min(len(signature_a), len(signature_b))
    if length == 0:
        return 0.0
    total = sum(
        signature_a[index].conjugate() * signature_b[index] for index in range(length)
    )
    return abs(total)


def match_packets(
    frames_a: List[CsiFrame], frames_b: List[CsiFrame]
) -> Tuple[List[PacketPair], int]:
    """Pair the frames the two nodes saw by their complex CSI response.

    Returns the matched pairs and the number of candidate pairs considered.
    Each frame is used at most once, and a frame is only paired when the
    other node's stream holds exactly one frame that correlates strongly: an
    ambiguous match is dropped rather than guessed, because a wrong pairing
    would corrupt the phase residual.
    """
    pairs: List[PacketPair] = []
    used_a: set[int] = set()
    used_b: set[int] = set()
    candidates = 0

    signatures_a = [_complex_signature(frame) for frame in frames_a]
    signatures_b = [_complex_signature(frame) for frame in frames_b]

    for index_a, frame_a in enumerate(frames_a):
        if index_a in used_a:
            continue
        rssi_a = _rssi_signature(frame_a)
        arrival_a = frame_a.host_arrival_us

        # Candidates: the frames the other node received at about the same
        # moment, whose CSI response agrees, and whose long training field
        # layout matches. A frame carrying only the legacy field has 64
        # subcarriers and a different offset from one carrying the legacy
        # and high-throughput fields merged, so their subcarrier indices do
        # not name the same physical subcarriers: pairing across the two
        # layouts compares different points of the channel.
        matches: List[Tuple[int, float]] = []
        for index_b, frame_b in enumerate(frames_b):
            if index_b in used_b:
                continue
            if frame_b.subcarrier_offset != frame_a.subcarrier_offset:
                continue
            if frame_b.n_subcarriers != frame_a.n_subcarriers:
                continue
            if abs(frame_b.host_arrival_us - arrival_a) > HOST_MATCH_WINDOW_US:
                continue
            correlation = _complex_correlation(
                signatures_a[index_a], signatures_b[index_b]
            )
            if correlation >= MIN_COMPLEX_CORRELATION:
                matches.append((index_b, correlation))
        if len(matches) != 1:
            # No match, or an ambiguous one: skip it rather than guess.
            continue
        index_b, correlation = matches[0]
        candidates += 1
        used_a.add(index_a)
        used_b.add(index_b)
        pairs.append(
            PacketPair(
                rssi_a=rssi_a,
                rssi_b=frames_b[index_b].rssi_dbm,
                index_a=index_a,
                index_b=index_b,
                device_time_a_us=frame_a.device_time_us,
                device_time_b_us=frames_b[index_b].device_time_us,
            )
        )

    return pairs, candidates


def _fit_cfo_trend(
    times_s: List[float], phases_rad: List[float]
) -> Tuple[float, float]:
    """Fit phase = intercept + cfo * t, unwrapping the phases first.

    Returns (intercept_rad, cfo_rad_per_s).
    """
    if len(times_s) < 2:
        return (phases_rad[0] if phases_rad else 0.0, 0.0)
    two_pi = 2.0 * math.pi
    unwrapped = [phases_rad[0]]
    for index in range(1, len(phases_rad)):
        current = phases_rad[index]
        previous = unwrapped[-1]
        rotations = round((previous - current) / two_pi)
        unwrapped.append(current + rotations * two_pi)

    mean_t = sum(times_s) / len(times_s)
    mean_y = sum(unwrapped) / len(unwrapped)
    numerator = sum(
        (t - mean_t) * (y - mean_y) for t, y in zip(times_s, unwrapped)
    )
    denominator = sum((t - mean_t) ** 2 for t in times_s)
    if denominator == 0.0:
        return mean_y, 0.0
    slope = numerator / denominator
    return mean_y - slope * mean_t, slope


def _is_conjugate(
    frames_a: List[CsiFrame],
    frames_b: List[CsiFrame],
    pairs: List[PacketPair],
    subcarrier: int,
) -> bool:
    """Whether the two nodes report the channel response conjugated.

    The two readings are compared as they are and conjugated; the reading
    with the tighter spread is the one the nodes agree on. The conjugation is
    detected rather than assumed, so a pair that genuinely agrees directly
    is not reported as conjugated.
    """
    direct: List[float] = []
    conjugated: List[float] = []
    for pair in pairs:
        frame_a = frames_a[pair.index_a]
        frame_b = frames_b[pair.index_b]
        if subcarrier >= len(frame_a.csi) or subcarrier >= len(frame_b.csi):
            continue
        value_a = frame_a.csi[subcarrier]
        value_b = frame_b.csi[subcarrier]
        if abs(value_a) < 0.05 or abs(value_b) < 0.05:
            continue
        direct.append(cmath.phase(value_a) - cmath.phase(value_b))
        conjugated.append(cmath.phase(value_a) - cmath.phase(value_b.conjugate()))
    if len(direct) < 2:
        return False
    spread_direct = statistics.pstdev(direct)
    spread_conjugated = statistics.pstdev(conjugated)
    return spread_conjugated < spread_direct


def measure_pair(
    node_a: int,
    node_b: int,
    frames_a: List[CsiFrame],
    frames_b: List[CsiFrame],
    subcarrier: int = DEFAULT_SUBCARRIER,
) -> ResidualResult:
    """Measure the cross-node phase residual for one node pair.

    The residual is the phase difference between the two nodes for the same
    ambient packet, after the linear phase trend (the carrier-frequency
    offset between the nodes and their clock drift) is removed.
    """
    pairs, candidates = match_packets(frames_a, frames_b)
    result = ResidualResult(
        node_a=node_a,
        node_b=node_b,
        n_matched=len(pairs),
        n_candidates=candidates,
    )

    if len(pairs) < MIN_MATCHED_PAIRS:
        result.band = "red"
        result.notes.append(
            f"only {len(pairs)} matched packets; at least "
            f"{MIN_MATCHED_PAIRS} are needed for a residual"
        )
        return result

    phases: List[float] = []
    times_s: List[float] = []
    for pair in pairs:
        frame_a = frames_a[pair.index_a]
        frame_b = frames_b[pair.index_b]
        # The subcarrier is named relative to the frame's own long training
        # field layout: the frame's `subcarrier_offset` says which physical
        # subcarrier its first entry is, so the same index in two frames with
        # the same layout is the same physical subcarrier. The matcher only
        # pairs frames with equal layouts, so the index is comparable.
        if subcarrier >= len(frame_a.csi) or subcarrier >= len(frame_b.csi):
            continue
        phase_a = cmath.phase(frame_a.csi[subcarrier])
        phase_b = cmath.phase(frame_b.csi[subcarrier])
        # The residual is the difference of the two measured phases.
        phases.append(cmath.phase(phase_a - phase_b))
        # The pair's time is the host arrival: the only clock both nodes
        # share, so the trend fitted to it is a real drift of the pair
        # rather than the difference of two unrelated clocks.
        times_s.append(frame_a.host_arrival_us / 1e6)

    if len(phases) < MIN_MATCHED_PAIRS:
        result.band = "red"
        result.notes.append("too few usable subcarrier samples")
        return result

    # The two nodes may report the same channel response conjugated — the
    # imaginary and the real parts exchanged — which is a fixed half-turn of
    # the phase plus a reversal of the subcarrier order. The conjugation is
    # detected, not assumed: the pair whose residual is the tighter of the
    # two readings is the one reported.
    result.conjugate = _is_conjugate(
        frames_a, frames_b, pairs, subcarrier
    )
    if result.conjugate:
        phases = [cmath.phase(-value) for value in phases]
        result.notes.append(
            "the two nodes report the channel response conjugated; the "
            "residual is reported against the conjugated reading"
        )

    _intercept, cfo = _fit_cfo_trend(times_s, phases)
    residuals = []
    for time_s, phase in zip(times_s, phases):
        # The constant term is the pair's fixed phase offset (the two nodes'
        # independent carrier-frequency offsets and the fixed path delay), and
        # the slope is the drift of that offset over the window. Both are
        # removed, so what remains is the per-packet residual: the part of
        # the cross-node phase that the linear model cannot explain.
        predicted = _intercept + cfo * time_s
        residuals.append(cmath.phase(phase - predicted))

    absolute = [abs(value) for value in residuals]
    result.residuals_rad = residuals
    result.median_rad = statistics.median(absolute)
    ordered = sorted(absolute)
    index = int(math.ceil(0.95 * len(ordered))) - 1
    index = max(0, min(index, len(ordered) - 1))
    result.p95_rad = ordered[index]
    result.cfo_rad_per_s = cfo
    result.offset_rad = cmath.phase(_intercept)
    if result.p95_rad < GREEN_MAX_RAD:
        result.band = "green"
    elif result.p95_rad < AMBER_MAX_RAD:
        result.band = "amber"
    else:
        result.band = "red"
    result.notes.append(
        f"{len(pairs)} packets matched by host arrival (within "
        f"{HOST_MATCH_WINDOW_US} us) and complex CSI correlation "
        f"(>= {MIN_COMPLEX_CORRELATION}), subcarrier {subcarrier}"
    )
    return result


def classify_band(p95_rad: float) -> str:
    """Green / amber / red per the campaign gates."""
    if p95_rad < GREEN_MAX_RAD:
        return "green"
    if p95_rad < AMBER_MAX_RAD:
        return "amber"
    return "red"


def run_two_node_measurement(
    duration_s: float = 120.0,
    subcarrier: int = DEFAULT_SUBCARRIER,
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
        frames_a=len(grouped.get(node_ids[0], [])) if node_ids else 0,
        frames_b=len(grouped.get(node_ids[1], [])) if len(node_ids) > 1 else 0,
    )

    if len(node_ids) < 2:
        report.notes.append(
            f"only {len(node_ids)} node(s) streaming; a cross-node "
            "residual needs two"
        )
        report.worst_band = "red"
        return report

    # Measure every pair and keep the worst band.
    bands: List[str] = []
    for first in range(len(node_ids)):
        for second in range(first + 1, len(node_ids)):
            node_a = node_ids[first]
            node_b = node_ids[second]
            result = measure_pair(
                node_a,
                node_b,
                grouped[node_a],
                grouped[node_b],
                subcarrier=subcarrier,
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
        f"frames: {report.frames_a} / {report.frames_b}",
        "",
    ]
    for result in report.results:
        lines.append(
            f"  node {result.node_a} <-> node {result.node_b}: "
            f"band={result.band} p95={result.p95_rad:.4f} rad "
            f"median={result.median_rad:.4f} rad "
            f"matched={result.n_matched} "
            f"offset={result.offset_rad:.4f} rad "
            f"cfo={result.cfo_rad_per_s:.4f} rad/s"
            + (" [conjugated]" if result.conjugate else "")
        )
        for note in result.notes:
            lines.append(f"      {note}")
    lines.append("")
    lines.append(f"worst band: {report.worst_band}")
    lines.append(
        "BCPT cleared over the air" if report.bcpt_cleared else
        "BCPT hardware-gated: the cross-node phase residual is in the red band"
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
