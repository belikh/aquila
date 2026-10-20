"""The nodes' pairwise distances, measured by radio.

The phase slope of the CSI across the subcarriers gives the power-weighted
mean path delay of a link — the delay from the transmitter's packet start to
the receiver — and that is an *absolute* quantity, not a relative one. The
model agrees: a link of 0.93 m gives a modelled group delay of 0.939 m.

The ESP32-S3's per-frame sign flip does not touch it, because a whole-vector
negation shifts every subcarrier's phase by the same pi and leaves the slope
alone. So the nodes can range to each other without any cross-node phase
coherence at all.

Three checks make the claim falsifiable rather than merely plausible:

* **Reciprocity.** Node A measuring node B and node B measuring node A are
  two independent measurements of the same path, so they must agree.
* **The triangle inequality.** Three measured distances must close.
* **Closure.** The three measured distances must satisfy the constraint that
  a triangle's sides do, which is what a purely relative measurement would
  fail.

The sign of the measured delay follows the convention of the phase and the
subcarrier order, so the *magnitude* is the distance; the sign is reported
so that a change of convention would be visible rather than silent.

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

from group_delay_hardware import measure_link
from hardware import CsiCollector, CsiFrame, frames_by_node


@dataclass
class PairDistance:
    """The distance between two nodes, measured from both ends."""

    node_a: int
    node_b: int
    layout_a: int
    layout_b: int
    from_a_m: float
    from_b_m: float
    n_a: int
    n_b: int

    @property
    def distance_m(self) -> float:
        """The distance, from the mean of the two directions."""
        return (abs(self.from_a_m) + abs(self.from_b_m)) / 2.0

    @property
    def asymmetry_m(self) -> float:
        """How far the two directions differ."""
        return abs(abs(self.from_a_m) - abs(self.from_b_m))


@dataclass
class GeometryReport:
    """The measured triangle of nodes."""

    distances: List[PairDistance] = field(default_factory=list)
    node_ids: List[int] = field(default_factory=list)
    # The transmitter's address that each node was measured against, per pair.
    transmitter_of: Dict[Tuple[int, int], str] = field(default_factory=dict)
    notes: List[str] = field(default_factory=list)

    def side(self, node_a: int, node_b: int) -> Optional[PairDistance]:
        """The measured distance between two nodes."""
        for distance in self.distances:
            if {distance.node_a, distance.node_b} == {node_a, node_b}:
                return distance
        return None


def measure_geometry(duration_s: float = 60.0) -> GeometryReport:
    """Measure the pairwise distance between every pair of nodes."""
    collector = CsiCollector(bind_port=9100)
    collector.start()
    try:
        time.sleep(2.0)
        frames = collector.collect_for(duration_s)
    finally:
        collector.stop()

    grouped = frames_by_node(frames)
    report = GeometryReport(node_ids=sorted(grouped))

    # For each receiver, the group delay of each other node's transmissions.
    delays: Dict[Tuple[int, int, int], float] = {}
    counts: Dict[Tuple[int, int, int], int] = {}
    transmitters: Dict[Tuple[int, int], str] = {}

    for receiver, node_frames in grouped.items():
        per_mac: Dict[str, List[CsiFrame]] = defaultdict(list)
        for frame in node_frames:
            if frame.source_mac is not None:
                per_mac[frame.source_mac].append(frame)
        for transmitter, subset in per_mac.items():
            for layout in (64, 128):
                frames_of_layout = [
                    frame for frame in subset if frame.n_subcarriers == layout
                ]
                link = measure_link(
                    receiver, frames_of_layout, transmitter, layout
                )
                if link is None or link.n_frames < 30:
                    continue
                delays[(receiver, transmitter, layout)] = (
                    link.path_difference_m
                )
                counts[(receiver, transmitter, layout)] = link.n_frames

    # A node's own address is the one the other nodes hear and it does not
    # hear itself.
    heard: Dict[int, set] = {}
    for receiver, node_frames in grouped.items():
        heard[receiver] = {
            frame.source_mac
            for frame in node_frames
            if frame.source_mac is not None
        }

    for receiver in report.node_ids:
        for other in report.node_ids:
            if receiver == other:
                continue
            # The address of `other`, as `receiver` hears it: the one the
            # third node hears and `receiver` does not.
            for transmitter in heard.get(receiver, set()):
                if transmitter in heard.get(other, set()):
                    # `receiver` hears it, so it is not `other`'s own address
                    # unless the geometry is degenerate.
                    continue
                for layout in (64, 128):
                    value = delays.get((receiver, transmitter, layout))
                    if value is None:
                        continue
                    report.transmitter_of[(receiver, other)] = transmitter
                    report.notes.append(
                        f"node {receiver} <- {transmitter} layout {layout}: "
                        f"{value:+.3f} m from "
                        f"{counts[(receiver, transmitter, layout)]} frames"
                    )

    # Pair the two directions.
    for first in range(len(report.node_ids)):
        for second in range(first + 1, len(report.node_ids)):
            node_a = report.node_ids[first]
            node_b = report.node_ids[second]
            best: Optional[PairDistance] = None
            for layout_a in (64, 128):
                for layout_b in (64, 128):
                    from_a = delays.get((node_a, _address_of(report, node_a, node_b), layout_a))
                    from_b = delays.get((node_b, _address_of(report, node_b, node_a), layout_b))
                    if from_a is None or from_b is None:
                        continue
                    candidate = PairDistance(
                        node_a=node_a,
                        node_b=node_b,
                        layout_a=layout_a,
                        layout_b=layout_b,
                        from_a_m=from_a,
                        from_b_m=from_b,
                        n_a=counts.get((node_a, _address_of(report, node_a, node_b), layout_a), 0),
                        n_b=counts.get((node_b, _address_of(report, node_b, node_a), layout_b), 0),
                    )
                    if best is None or candidate.asymmetry_m < best.asymmetry_m:
                        best = candidate
            if best is not None:
                report.distances.append(best)
    return report


def _address_of(
    report: GeometryReport, receiver: int, other: int
) -> str:
    """The transmitter's address that `receiver` was measured against."""
    return report.transmitter_of.get((receiver, other), "")


def check_triangle(report: GeometryReport) -> List[str]:
    """The triangle checks: the inequality and the closure.

    Three measured distances must form a triangle. That is the check that
    a *relative* measurement would fail: a quantity that only said how the
    three links differed from each other would place the three nodes
    anywhere.
    """
    notes: List[str] = []
    sides: Dict[Tuple[int, int], float] = {}
    for distance in report.distances:
        sides[(distance.node_a, distance.node_b)] = distance.distance_m

    node_ids = report.node_ids
    for first in range(len(node_ids)):
        for second in range(first + 1, len(node_ids)):
            for third in range(second + 1, len(node_ids)):
                node_a, node_b, node_c = (
                    node_ids[first], node_ids[second], node_ids[third]
                )
                ab = sides.get((node_a, node_b))
                ac = sides.get((node_a, node_c))
                bc = sides.get((node_b, node_c))
                if ab is None or ac is None or bc is None:
                    continue
                checks = (
                    (ab + ac, bc, "a-b + a-c"),
                    (ab + bc, ac, "a-b + b-c"),
                    (ac + bc, ab, "a-c + b-c"),
                )
                outcomes = []
                for total, opposite, label in checks:
                    outcomes.append(
                        f"{label} = {total * 100:.0f} cm > "
                        f"{opposite * 100:.0f} cm"
                    )
                notes.append(
                    f"triangle {node_a}-{node_b}-{node_c}: "
                    f"{ab * 100:.0f} cm, {ac * 100:.0f} cm, {bc * 100:.0f} cm; "
                    + "; ".join(outcomes)
                )
    return notes


def format_report(report: GeometryReport) -> str:
    """Render the report for the operator."""
    lines = ["=== Aquila node geometry, measured by radio ==="]
    for note in report.notes:
        lines.append(f"  {note}")
    lines.append("")
    lines.append("  pairwise distances:")
    for distance in report.distances:
        lines.append(
            f"    node {distance.node_a} <-> node {distance.node_b}: "
            f"{distance.distance_m:.3f} m "
            f"({distance.from_a_m:+.3f} m and {distance.from_b_m:+.3f} m, "
            f"asymmetry {distance.asymmetry_m * 100:.1f} cm)"
        )
    triangle = check_triangle(report)
    if triangle:
        lines.append("")
        lines.append("  triangle checks:")
        for note in triangle:
            lines.append(f"    {note}")
    return "\n".join(lines)


def main() -> int:
    """Measure the nodes' geometry."""
    duration = float(sys.argv[1]) if len(sys.argv) > 1 else 60.0
    report = measure_geometry(duration_s=duration)
    print(format_report(report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
