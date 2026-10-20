"""The per-frame phase rotation, and whether two nodes share it.

The raw phase series of one transmitter at one node wanders freely, but the
phase *difference* between two subcarriers is nearly constant from frame to
frame. That says each frame's complex vector is rotated by an arbitrary angle
while its internal phase structure survives.

This script measures the rotation directly, and then asks the question the
cross-node measurement depends on: when two nodes capture the same
transmission, do they see the *same* rotation?

The rotation is estimated as the phase at a reference subcarrier, minus that
subcarrier's stable median phase. Subtracting the median removes the channel's
own phase and leaves the per-frame rotation.

If the two nodes' rotations agree for the same packet, the cross-node phase
difference is well defined and the only remaining error is a sign. If they
disagree, the cross-node absolute phase is hopeless on this hardware and the
project must use the amplitude or the within-packet phase slope instead.

Australian English spelling throughout.
"""

from __future__ import annotations

import cmath
import math
import statistics
import sys
import time
from collections import Counter, defaultdict
from typing import Dict, List, Sequence, Tuple

sys.path.insert(0, ".")

from hardware import CsiCollector, CsiFrame, frames_by_node

REFERENCE_SUBCARRIER = 50


def histogram(values: Sequence[float], bins: int = 12) -> List[int]:
    """A histogram of phases over (-pi, pi]."""
    counts = [0] * bins
    for value in values:
        index = int((value + math.pi) / (2 * math.pi) * bins)
        counts[min(bins - 1, max(0, index))] += 1
    return counts


def relative_stability(
    frames: Sequence[CsiFrame], first: int, second: int
) -> Tuple[float, float]:
    """The mean and spread of the phase difference between two subcarriers."""
    differences: List[float] = []
    for frame in frames:
        if first >= len(frame.csi) or second >= len(frame.csi):
            continue
        va = frame.csi[first]
        vb = frame.csi[second]
        if abs(va) < 0.05 or abs(vb) < 0.05:
            continue
        differences.append(cmath.phase(va) - cmath.phase(vb))
    if not differences:
        return (0.0, 0.0)
    return (
        statistics.mean(differences),
        statistics.pstdev(differences),
    )


def main() -> int:
    """Collect and report the rotation structure."""
    duration = float(sys.argv[1]) if len(sys.argv) > 1 else 60.0
    collector = CsiCollector(bind_port=9100)
    collector.start()
    time.sleep(2.0)
    frames = collector.collect_for(duration)
    collector.stop()

    grouped = frames_by_node(frames)
    node_ids = sorted(grouped)
    print(f"nodes {node_ids}, {duration:.0f}s")

    per_mac: Dict[int, Dict[str, List[CsiFrame]]] = {}
    for node_id in node_ids:
        table: Dict[str, List[CsiFrame]] = defaultdict(list)
        for frame in grouped[node_id]:
            if frame.source_mac is not None and frame.n_subcarriers == 64:
                table[frame.source_mac].append(frame)
        per_mac[node_id] = table

    common = set(per_mac[node_ids[0]]) & set(per_mac[node_ids[1]])
    counts = {
        mac: min(len(per_mac[node_ids[0]][mac]), len(per_mac[node_ids[1]][mac]))
        for mac in common
    }
    mac = max(counts, key=lambda m: counts[m])
    node_a, node_b = node_ids[0], node_ids[1]
    print(f"transmitter {mac}: node {node_a} {len(per_mac[node_a][mac])} frames, "
          f"node {node_b} {len(per_mac[node_b][mac])} frames")

    print()
    print("--- within-frame relative phase across subcarriers ---")
    for node_id in (node_a, node_b):
        subset = per_mac[node_id][mac]
        pairs = [(10, 50), (20, 50), (30, 50), (50, 60)]
        detail = []
        for first, second in pairs:
            mean, spread = relative_stability(subset, first, second)
            detail.append(f"sc{first}-sc{second}={mean:+.3f}±{spread:.3f}")
        print(f"  node {node_id}: " + "  ".join(detail))

    # The per-frame rotation at each node: the phase at the reference
    # subcarrier, minus that subcarrier's median over the run.
    print()
    print("--- per-frame rotation ---")
    rotations: Dict[int, List[Tuple[float, float]]] = {}
    for node_id in (node_a, node_b):
        subset = per_mac[node_id][mac]
        phases: List[float] = []
        times: List[float] = []
        for frame in subset:
            if REFERENCE_SUBCARRIER >= len(frame.csi):
                continue
            value = frame.csi[REFERENCE_SUBCARRIER]
            if abs(value) < 0.05:
                continue
            phases.append(cmath.phase(value))
            times.append(frame.host_arrival_us / 1e6)
        median = statistics.median(phases)
        rotations[node_id] = [
            (moment, cmath.phase(p - median)) for moment, p in zip(times, phases)
        ]
        print(f"  node {node_id}: rotation histogram (12 bins): "
              f"{histogram([r for _, r in rotations[node_id]])}")
        print(f"    first 12 rotations: "
              f"{[round(r, 3) for _, r in rotations[node_id][:12]]}")

    # Do the two nodes share the rotation for the same packet?
    print()
    print("--- cross-node rotation difference, same packet ---")
    times_b = [t for t, _ in rotations[node_b]]
    rot_b = dict(rotations[node_b])
    differences: List[float] = []
    for moment, rotation in rotations[node_a]:
        best = min(range(len(times_b)), key=lambda j: abs(times_b[j] - moment))
        if abs(times_b[best] - moment) > 0.004:
            continue
        differences.append(cmath.phase(rotation - rot_b[best]))
    print(f"  n={len(differences)}")
    print(f"  histogram (12 bins over -pi..pi): {histogram(differences)}")
    if differences:
        print(f"  |difference| median: "
              f"{statistics.median(abs(d) for d in differences):.4f} rad")
        near_zero = sum(1 for d in differences if abs(d) < 0.5)
        near_pi = sum(1 for d in differences if abs(abs(d) - math.pi) < 0.5)
        print(f"  within 0.5 rad of zero: {near_zero}; "
              f"within 0.5 rad of pi: {near_pi}; "
              f"of {len(differences)}")

    # And the same, with the rotation computed relative to each node's own
    # median, but with the cross-node difference taken on the raw phases.
    print()
    print("--- raw cross-node phase difference, same packet ---")
    frames_a = per_mac[node_a][mac]
    frames_b = per_mac[node_b][mac]
    times_b_raw = [f.host_arrival_us / 1e6 for f in frames_b]
    raw: List[float] = []
    for frame_a in frames_a:
        best = min(
            range(len(times_b_raw)),
            key=lambda j: abs(times_b_raw[j] - frame_a.host_arrival_us / 1e6),
        )
        if abs(times_b_raw[best] - frame_a.host_arrival_us / 1e6) > 0.004:
            continue
        va = frame_a.csi[REFERENCE_SUBCARRIER]
        vb = frames_b[best].csi[REFERENCE_SUBCARRIER]
        if abs(va) < 0.05 or abs(vb) < 0.05:
            continue
        raw.append(cmath.phase(va) - cmath.phase(vb))
    print(f"  n={len(raw)}  histogram: {histogram(raw)}")
    print(f"  first 12: {[round(r, 3) for r in raw[:12]]}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
