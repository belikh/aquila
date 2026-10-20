"""Is the per-frame sign flip really a whole-vector negation, and does
resolving it make the cross-node phase continuous?

This script tests the sign-resolution model end to end, at one node first and
then across two:

1. the correlation between *consecutive* frames of one transmitter, before
   resolution. If the flip model holds, the correlation is near +1 or near -1
   and never in between;
2. the same correlation after the sign has been resolved. If the resolution is
   right, it is near +1 for every transition;
3. the cross-node phase difference after both nodes have resolved their
   signs: is it continuous from packet to packet, or does it still jump by
   pi?

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
from m2_sign_resolved import apply_signs, correlation, resolve_signs

NULL_SUBCARRIERS = {0, 28, 29, 30, 31, 32, 33}


def histogram(values: Sequence[float], bins: int = 12) -> List[int]:
    counts = [0] * bins
    for value in values:
        index = int((value + math.pi) / (2 * math.pi) * bins)
        counts[min(bins - 1, max(0, index))] += 1
    return counts


def usable_for(frames: Sequence[CsiFrame]) -> List[int]:
    """The subcarriers carrying signal in at least nine frames in ten."""
    if not frames:
        return []
    count = len(frames[0].csi)
    result: List[int] = []
    for k in range(count):
        if k in NULL_SUBCARRIERS:
            continue
        signalled = sum(1 for f in frames if abs(f.csi[k]) > 0.05)
        if signalled >= 0.9 * len(frames):
            result.append(k)
    return result


def main() -> int:
    """Collect and report the sign-resolution diagnostics."""
    duration = float(sys.argv[1]) if len(sys.argv) > 1 else 60.0
    collector = CsiCollector(bind_port=9100)
    collector.start()
    time.sleep(2.0)
    frames = collector.collect_for(duration)
    collector.stop()

    grouped = frames_by_node(frames)
    node_ids = sorted(grouped)
    node_a, node_b = node_ids[0], node_ids[1]
    print(f"nodes {node_ids}, {duration:.0f}s")

    per_mac: Dict[int, Dict[str, List[CsiFrame]]] = {}
    for node_id in node_ids:
        table: Dict[str, List[CsiFrame]] = defaultdict(list)
        for frame in grouped[node_id]:
            if frame.source_mac is not None:
                table[frame.source_mac].append(frame)
        per_mac[node_id] = table

    common = set(per_mac[node_a]) & set(per_mac[node_b])
    counts = {
        mac: min(len(per_mac[node_a][mac]), len(per_mac[node_b][mac]))
        for mac in common
    }
    mac = max(counts, key=lambda m: counts[m])
    print(f"transmitter {mac}: "
          f"node {node_a} {len(per_mac[node_a][mac])} frames, "
          f"node {node_b} {len(per_mac[node_b][mac])} frames")

    for layout in (64, 128):
        subset = {
            node_id: [f for f in per_mac[node_id][mac] if f.n_subcarriers == layout]
            for node_id in (node_a, node_b)
        }
        if len(subset[node_a]) < 20 or len(subset[node_b]) < 20:
            continue
        print()
        print(f"=== layout {layout} ===")
        usable = usable_for(subset[node_a])
        print(f"  usable subcarriers: {len(usable)} of {len(subset[node_a][0].csi)}")

        for node_id in (node_a, node_b):
            frames_n = subset[node_id]
            before = [
                correlation(frames_n[i - 1], frames_n[i], usable)
                for i in range(1, len(frames_n))
            ]
            signs, resolution = resolve_signs(frames_n, usable)
            corrected = apply_signs(frames_n, signs)
            after = [
                correlation(corrected[i - 1], corrected[i], usable)
                for i in range(1, len(corrected))
            ]
            print(
                f"  node {node_id}: transitions={len(before)} "
                f"flips={resolution.n_flips}"
            )
            print(f"    correlation before: mean={statistics.mean(before):+.4f} "
                  f"sd={statistics.pstdev(before):.4f} "
                  f"min={min(before):+.4f} max={max(before):+.4f}")
            print(f"    correlation after:  mean={statistics.mean(after):+.4f} "
                  f"sd={statistics.pstdev(after):.4f} "
                  f"min={min(after):+.4f} max={max(after):+.4f}")
            negative_after = sum(1 for value in after if value < 0)
            print(f"    transitions still negative after resolution: "
                  f"{negative_after} of {len(after)}")

        # The cross-node difference, after both nodes have resolved.
        corrected_a = apply_signs(subset[node_a], resolve_signs(subset[node_a], usable)[0])
        corrected_b = apply_signs(subset[node_b], resolve_signs(subset[node_b], usable)[0])
        times_b = [f.host_arrival_us for f in corrected_b]
        differences: List[float] = []
        matched_times: List[float] = []
        for frame_a in corrected_a:
            matches = [
                index for index, moment in enumerate(times_b)
                if abs(moment - frame_a.host_arrival_us) <= 4000
            ]
            if len(matches) != 1:
                continue
            frame_b = corrected_b[matches[0]]
            va = frame_a.csi[50]
            vb = frame_b.csi[50]
            if abs(va) < 0.05 or abs(vb) < 0.05:
                continue
            differences.append(cmath.phase(va) - cmath.phase(vb))
            matched_times.append(frame_a.host_arrival_us / 1e6)
        print(f"  cross-node pairs: {len(differences)}")
        if differences:
            print(f"    difference histogram (12 bins): {histogram(differences)}")
            steps = [
                cmath.phase(differences[i + 1] - differences[i])
                for i in range(len(differences) - 1)
            ]
            near_pi = sum(1 for s in steps if abs(abs(s) - math.pi) < 0.5)
            print(f"    successive steps near pi: {near_pi} of {len(steps)}")
            print(f"    first 12 differences: "
                  f"{[round(d, 3) for d in differences[:12]]}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
