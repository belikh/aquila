"""Why is the cross-node phase residual uniform?

The paired packets are unambiguous, yet the cross-node phase difference comes
out uniformly distributed. This script separates the possible causes by
decomposing the measurement:

1. the **single-node** phase of one transmitter over time — is it stable at
   all?
2. the **cross-node** difference of the same transmitter, formed two ways:
   the raw difference of the two wrapped phases, and the difference after each
   node's phase has been unwrapped and drift-removed on its own.

If (1) is stable but (2) is uniform, the fault is in the differencing. If (1)
is uniform too, the node's phase for that transmitter is not stable and the
fault is in the premise.

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


def unwrap(values: Sequence[float]) -> List[float]:
    """Unwrap a phase series already wrapped to (-pi, pi]."""
    if not values:
        return []
    two_pi = 2.0 * math.pi
    out = [values[0]]
    for index in range(1, len(values)):
        current = values[index]
        rotations = round((out[-1] - current) / two_pi)
        out.append(current + rotations * two_pi)
    return out


def fit(times: Sequence[float], values: Sequence[float]) -> Tuple[float, float]:
    """Fit value = intercept + slope * time."""
    n = len(times)
    if n < 2:
        return (values[0] if values else 0.0, 0.0)
    mt = sum(times) / n
    my = sum(values) / n
    den = sum((t - mt) ** 2 for t in times)
    if den == 0.0:
        return my, 0.0
    slope = sum((t - mt) * (y - my) for t, y in zip(times, values)) / den
    return my - slope * mt, slope


def concentration(values: Sequence[float]) -> float:
    """The circular concentration of a set of phases."""
    if not values:
        return 0.0
    vector = sum(cmath.exp(1j * v) for v in values)
    return abs(vector) / len(values)


def histogram(values: Sequence[float], bins: int = 12) -> List[int]:
    """A histogram of phases over (-pi, pi]."""
    counts = [0] * bins
    for value in values:
        index = int((value + math.pi) / (2 * math.pi) * bins)
        counts[min(bins - 1, max(0, index))] += 1
    return counts


def main() -> int:
    """Collect and report the decomposition."""
    duration = float(sys.argv[1]) if len(sys.argv) > 1 else 90.0
    collector = CsiCollector(bind_port=9100)
    collector.start()
    time.sleep(2.0)
    frames = collector.collect_for(duration)
    collector.stop()

    grouped = frames_by_node(frames)
    node_ids = sorted(grouped)
    print(f"nodes {node_ids}, {duration:.0f}s")

    by_transmitter: Dict[int, Dict[str, List[CsiFrame]]] = {}
    for node_id in node_ids:
        per_mac: Dict[str, List[CsiFrame]] = defaultdict(list)
        for frame in grouped[node_id]:
            if frame.source_mac is not None and frame.n_subcarriers == 64:
                per_mac[frame.source_mac].append(frame)
        by_transmitter[node_id] = per_mac

    # The transmitter both nodes hear most: the strongest common link.
    common = set(by_transmitter[node_ids[0]]) & set(by_transmitter[node_ids[1]])
    counts = {
        mac: min(
            len(by_transmitter[node_ids[0]][mac]),
            len(by_transmitter[node_ids[1]][mac]),
        )
        for mac in common
    }
    mac = max(counts, key=lambda m: counts[m])
    print(f"common transmitter: {mac} (frames {counts[mac]})")

    node_a, node_b = node_ids[0], node_ids[1]
    frames_a = by_transmitter[node_a][mac]
    frames_b = by_transmitter[node_b][mac]

    for subcarrier in (10, 30, 50):
        print()
        print(f"--- subcarrier {subcarrier} ---")

        # (1) Single-node phase stability, per node.
        for node_id, node_frames in ((node_a, frames_a), (node_b, frames_b)):
            times: List[float] = []
            phases: List[float] = []
            for frame in node_frames:
                if subcarrier >= len(frame.csi):
                    continue
                value = frame.csi[subcarrier]
                if abs(value) < 0.05:
                    continue
                times.append(frame.host_arrival_us / 1e6)
                phases.append(cmath.phase(value))
            unwrapped = unwrap(phases)
            intercept, slope = fit(times, unwrapped)
            residual = [
                cmath.phase(v - (intercept + slope * t))
                for t, v in zip(times, unwrapped)
            ]
            magnitudes = sorted(abs(r) for r in residual)
            p95 = magnitudes[int(math.ceil(0.95 * len(magnitudes))) - 1]
            print(
                f"  node {node_id} alone: n={len(phases)} "
                f"concentration={concentration(residual):.4f} "
                f"p95={p95:.4f} rad drift={slope:+.4f} rad/s "
                f"offset={cmath.phase(intercept):+.4f} rad"
            )

        # (2) Cross-node difference, two ways.
        times_b = [f.host_arrival_us / 1e6 for f in frames_b]
        pairs: List[Tuple[CsiFrame, CsiFrame]] = []
        used = set()
        for frame_a in frames_a:
            matches = [
                index
                for index, moment in enumerate(times_b)
                if index not in used
                and abs(moment - frame_a.host_arrival_us / 1e6) <= 0.004
            ]
            if len(matches) == 1:
                used.add(matches[0])
                pairs.append((frame_a, frames_b[matches[0]]))

        raw: List[float] = []
        times: List[float] = []
        for frame_a, frame_b in pairs:
            va = frame_a.csi[subcarrier]
            vb = frame_b.csi[subcarrier]
            if abs(va) < 0.05 or abs(vb) < 0.05:
                continue
            raw.append(cmath.phase(va) - cmath.phase(vb))
            times.append(frame_a.host_arrival_us / 1e6)

        # (2a) The raw difference, wrapped first.
        wrapped = [cmath.phase(value) for value in raw]
        unwrapped = unwrap(wrapped)
        intercept, slope = fit(times, unwrapped)
        residual_a = [
            cmath.phase(v - (intercept + slope * t))
            for t, v in zip(times, unwrapped)
        ]
        magnitudes = sorted(abs(r) for r in residual_a)
        p95_a = magnitudes[int(math.ceil(0.95 * len(magnitudes))) - 1]
        print(
            f"  cross-node (wrap then unwrap): n={len(raw)} "
            f"concentration={concentration(residual_a):.4f} p95={p95_a:.4f} rad "
            f"drift={slope:+.4f} rad/s offset={cmath.phase(intercept):+.4f} rad"
        )
        print(f"    residual histogram (12 bins over -pi..pi): {histogram(residual_a)}")
        print(f"    raw difference range: "
              f"[{min(raw):+.4f}, {max(raw):+.4f}] rad")

        # (2b) Each node unwrapped and drift-removed on its own, then
        #      differenced.
        def node_series(node_frames: List[CsiFrame]) -> Tuple[List[float], List[float]]:
            t_list: List[float] = []
            p_list: List[float] = []
            for frame in node_frames:
                if subcarrier >= len(frame.csi):
                    continue
                value = frame.csi[subcarrier]
                if abs(value) < 0.05:
                    continue
                t_list.append(frame.host_arrival_us / 1e6)
                p_list.append(cmath.phase(value))
            return t_list, p_list

        ta, pa = node_series(frames_a)
        tb, pb = node_series(frames_b)
        ua = unwrap(pa)
        ub = unwrap(pb)
        ia, sa = fit(ta, ua)
        ib, sb = fit(tb, ub)
        # Interpolate node B's detrended phase at node A's sample times.
        det_a = [v - (ia + sa * t) for t, v in zip(ta, ua)]
        det_b = [v - (ib + sb * t) for t, v in zip(tb, ub)]
        times_b_only = [t for t in tb]
        combined: List[float] = []
        combined_times: List[float] = []
        for index, frame_a in enumerate(frames_a):
            if index >= len(ta) or subcarrier >= len(frame_a.csi):
                continue
            value_a = frame_a.csi[subcarrier]
            if abs(value_a) < 0.05:
                continue
            moment = ta[index]
            # Nearest node B sample in time.
            best = min(range(len(times_b_only)), key=lambda j: abs(times_b_only[j] - moment))
            if abs(times_b_only[best] - moment) > 0.004:
                continue
            combined.append(cmath.phase(det_a[index] - det_b[best]))
            combined_times.append(moment)
        if len(combined) > 10:
            print(
                f"  cross-node (each detrended first): n={len(combined)} "
                f"concentration={concentration(combined):.4f}"
            )
            print(f"    histogram: {histogram(combined)}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
