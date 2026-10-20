"""Per-link group delay from the ESP32-S3's CSI phase slope.

The ESP32-S3's absolute phase cannot be used across nodes: each frame's
channel vector is reported with an arbitrary sign, and the sign is an
independent coin flip at each receiver, so the cross-node phase difference for
the same packet is ambiguous modulo pi with no way to resolve it
(`m2_two_node_hardware.py` measures this).

The phase *slope* across subcarriers is a different matter, and it is the
quantity the project can still use. A whole-vector sign flip shifts every
subcarrier's phase by the same pi, so it leaves the phase differences between
subcarriers untouched. The slope of the phase against subcarrier index is
therefore exactly invariant to the flip, and it is the per-link group delay:

    delay = slope / (2 * pi * subcarrier_spacing)

The slope is also gauge-invariant in the sense that matters here: it does not
depend on the unknown phase offsets of either node, so it needs no common
reference and no cross-node coherence.

This module measures it, and checks the property that gives it its value: a
reciprocal pair of links must give the same path length. Node A measuring the
path from B and node B measuring the path from A are two independent
measurements of the same geometry, so their agreement is a real check on the
measurement, not a tautology.

Australian English spelling throughout.
"""

from __future__ import annotations

import cmath
import math
import statistics
import sys
import time
from collections import defaultdict
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Sequence, Tuple

sys.path.insert(0, ".")

from hardware import CsiCollector, CsiFrame, frames_by_node

# The 802.11 subcarrier spacing in hertz.
SUBCARRIER_SPACING_HZ = 312_500.0

# The speed of light in metres per second.
SPEED_OF_LIGHT_MPS = 299_792_458.0

# The subcarriers the ESP32-S3 leaves null: the carrier at the centre of the
# legacy long training field, and the null band in the middle of the payload.
NULL_SUBCARRIERS = {0, 28, 29, 30, 31, 32, 33}

# The smallest amplitude that counts as signal.
MIN_SIGNAL = 0.05

# The share of frames a subcarrier must carry signal on.
MIN_SIGNAL_SHARE = 0.9

# The 128-subcarrier layout is the legacy long training field followed by the
# high-throughput one. Their tone arrangements differ, so the slope is fitted
# over the long training field alone: that part is the same set of physical
# subcarriers as the 64-subcarrier layout, and the high-throughput half is not
# directly comparable with it.
LLTF_SUBCARRIERS = 64


@dataclass
class LinkDelay:
    """The measured phase slope of one link, and the path it implies."""

    node_id: int
    transmitter: str
    layout: int
    n_frames: int
    slope_rad_per_subcarrier: float
    slope_sd: float
    path_difference_m: float
    # The uncertainty of this link's path length, from the scatter of the
    # window's block means rather than from the per-frame spread. The frames
    # are temporally correlated — a slow drift in the channel moves them
    # together — so the per-frame spread divided by the square root of the
    # frame count is far too optimistic. Blocking the window and taking the
    # standard error of the block means prices in the drift.
    path_uncertainty_m: float = 0.0

    @property
    def path_spread_m(self) -> float:
        """The path-length uncertainty implied by the slope's per-frame spread."""
        return self.slope_sd / (2.0 * math.pi * SUBCARRIER_SPACING_HZ) * SPEED_OF_LIGHT_MPS


@dataclass
class DelayReport:
    """Every link measured, and the reciprocity check."""

    links: List[LinkDelay] = field(default_factory=list)
    notes: List[str] = field(default_factory=list)
    # The reciprocity checks, structured: for each pair of nodes and each
    # layout, the path length each node measured and how far apart they are.
    reciprocity: List[dict] = field(default_factory=list)


def usable_subcarriers(frames: Sequence[CsiFrame]) -> List[int]:
    """The subcarriers carrying signal on almost every frame.

    Only the legacy long training field is used: it is the part whose physical
    subcarriers are the same in both layouts, so a slope measured on it means
    the same thing whichever layout the node happened to report.
    """
    if not frames:
        return []
    result: List[int] = []
    for k in range(min(LLTF_SUBCARRIERS, len(frames[0].csi))):
        if k in NULL_SUBCARRIERS:
            continue
        signalled = sum(1 for frame in frames if abs(frame.csi[k]) >= MIN_SIGNAL)
        if signalled >= MIN_SIGNAL_SHARE * len(frames):
            result.append(k)
    return result


def unwrap_increasing(values: Sequence[float]) -> List[float]:
    """Unwrap a phase series that is smooth within a frame."""
    if not values:
        return []
    two_pi = 2.0 * math.pi
    out = [values[0]]
    for index in range(1, len(values)):
        rotations = round((out[-1] - values[index]) / two_pi)
        out.append(values[index] + rotations * two_pi)
    return out


def fit_slope(
    subcarriers: Sequence[int], phases: Sequence[float]
) -> Tuple[float, float]:
    """Fit phase = intercept + slope * subcarrier."""
    n = len(subcarriers)
    if n < 8:
        return (0.0, 0.0)
    mean_k = sum(subcarriers) / n
    mean_p = sum(phases) / n
    denominator = sum((k - mean_k) ** 2 for k in subcarriers)
    if denominator == 0.0:
        return (mean_p, 0.0)
    slope = (
        sum((k - mean_k) * (p - mean_p) for k, p in zip(subcarriers, phases))
        / denominator
    )
    return mean_p - slope * mean_k, slope


def measure_link(
    node_id: int, frames: Sequence[CsiFrame], transmitter: str, layout: int
) -> Optional[LinkDelay]:
    """Measure the phase slope of one link."""
    if len(frames) < 20:
        return None
    usable = usable_subcarriers(frames)
    if len(usable) < 8:
        return None
    slopes: List[float] = []
    for frame in frames:
        present = [k for k in usable if abs(frame.csi[k]) >= MIN_SIGNAL]
        if len(present) < 8:
            continue
        phases = [cmath.phase(frame.csi[k]) for k in present]
        unwrapped = unwrap_increasing(phases)
        _intercept, slope = fit_slope(present, unwrapped)
        slopes.append(slope)
    if len(slopes) < 20:
        return None
    path_per_slope = (
        1.0 / (2.0 * math.pi * SUBCARRIER_SPACING_HZ) * SPEED_OF_LIGHT_MPS
    )

    # The median and the median absolute deviation, not the mean and the
    # standard deviation. A minority of frames carry a corrupted phase
    # profile — a sign flip that crosses the unwrap, or a null subcarrier
    # inside the fit — and those frames are worth tens of times the ordinary
    # spread. A mean lets them move the answer; the median does not.
    mean = statistics.median(slopes)
    spread = 1.4826 * statistics.median(
        [abs(slope - mean) for slope in slopes]
    )
    mean_path = mean * path_per_slope

    # The uncertainty of the window's mean, from the block medians. The
    # frames are temporally correlated, so the effective number of independent
    # samples is the number of blocks, not the number of frames.
    block = max(8, min(30, len(slopes) // 8))
    block_medians = [
        statistics.median(slopes[start : start + block])
        for start in range(0, len(slopes) - block + 1, block)
    ]
    if len(block_medians) >= 2:
        uncertainty = (
            statistics.pstdev(block_medians) / math.sqrt(len(block_medians))
            * path_per_slope
        )
    else:
        uncertainty = spread / math.sqrt(len(slopes)) * path_per_slope

    return LinkDelay(
        node_id=node_id,
        transmitter=transmitter,
        layout=layout,
        n_frames=len(slopes),
        slope_rad_per_subcarrier=mean,
        slope_sd=spread,
        path_difference_m=mean_path,
        path_uncertainty_m=uncertainty,
    )


def run_delay_measurement(
    duration_s: float = 90.0, collector: Optional[CsiCollector] = None
) -> DelayReport:
    """Collect from the live nodes and measure every link's phase slope."""
    owned = collector is None
    if collector is None:
        collector = CsiCollector(bind_port=9100)
    collector.start()
    try:
        time.sleep(2.0)
        frames = collector.collect_for(duration_s)
    finally:
        if owned:
            collector.stop()

    grouped = frames_by_node(frames)
    report = DelayReport()
    per_node: Dict[int, Dict[Tuple[str, int], List[CsiFrame]]] = {}

    for node_id, node_frames in grouped.items():
        table: Dict[Tuple[str, int], List[CsiFrame]] = defaultdict(list)
        for frame in node_frames:
            if frame.source_mac is not None:
                table[(frame.source_mac, frame.n_subcarriers)].append(frame)
        per_node[node_id] = table
        for (transmitter, layout), subset in sorted(table.items()):
            link = measure_link(node_id, subset, transmitter, layout)
            if link is not None:
                report.links.append(link)

    # The reciprocity check: node A's link to B and node B's link to A must
    # give the same path length. A node's own address is identified by the
    # fact that the other nodes hear it and it does not hear itself.
    node_ids = sorted(grouped)
    for first in range(len(node_ids)):
        for second in range(first + 1, len(node_ids)):
            node_a = node_ids[first]
            node_b = node_ids[second]
            heard_by_a = {
                frame.source_mac
                for frame in grouped[node_a]
                if frame.source_mac is not None
            }
            heard_by_b = {
                frame.source_mac
                for frame in grouped[node_b]
                if frame.source_mac is not None
            }
            # The address of A, as B hears it: B hears it and A does not.
            mac_a = heard_by_b - heard_by_a
            # The address of B, as A hears it.
            mac_b = heard_by_a - heard_by_b
            if not mac_a or not mac_b:
                continue
            for layout in (64, 128):
                forward = [
                    link
                    for link in report.links
                    if link.node_id == node_b
                    and link.layout == layout
                    and link.transmitter in mac_a
                ]
                reverse = [
                    link
                    for link in report.links
                    if link.node_id == node_a
                    and link.layout == layout
                    and link.transmitter in mac_b
                ]
                for link_b in forward:
                    for link_a in reverse:
                        difference = abs(
                            abs(link_a.path_difference_m)
                            - abs(link_b.path_difference_m)
                        )
                        report.reciprocity.append({
                            "node_a": node_a,
                            "node_b": node_b,
                            "layout": layout,
                            "path_a_m": link_a.path_difference_m,
                            "path_b_m": link_b.path_difference_m,
                            "difference_m": difference,
                            "uncertainty_m": max(
                                link_a.path_uncertainty_m,
                                link_b.path_uncertainty_m,
                            ),
                        })
                        report.notes.append(
                            f"reciprocity {node_a}<->{node_b} layout {layout}: "
                            f"{link_a.path_difference_m:+.3f} m and "
                            f"{link_b.path_difference_m:+.3f} m, "
                            f"differing by {difference * 100:.1f} cm"
                        )
    return report


def format_report(report: DelayReport) -> str:
    """Render the report for the operator."""
    lines = ["=== Aquila per-link group delay (phase slope) ==="]
    for link in sorted(report.links, key=lambda l: (l.node_id, l.transmitter, l.layout)):
        lines.append(
            f"  node {link.node_id} <- {link.transmitter} layout {link.layout}: "
            f"slope={link.slope_rad_per_subcarrier:+.6f} "
            f"+/-{link.slope_sd:.6f} rad/subcarrier, "
            f"path={link.path_difference_m:+.3f} m "
            f"+/-{link.path_spread_m * 100:.1f} cm per frame "
            f"(+/-{link.path_uncertainty_m * 100:.1f} cm over the window), "
            f"n={link.n_frames}"
        )
    if report.notes:
        lines.append("")
        lines.append("  reciprocity checks:")
        for note in report.notes:
            lines.append(f"    {note}")
    return "\n".join(lines)


if __name__ == "__main__":
    duration = float(sys.argv[1]) if len(sys.argv) > 1 else 90.0
    print(format_report(run_delay_measurement(duration_s=duration)))
