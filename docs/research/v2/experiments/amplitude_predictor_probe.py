"""Can the sign flip be predicted from the amplitude?

The flip is not a counter-driven pattern and not a post-processing negation,
so the last possibility is that the estimator's sign decision depends on
something visible in the *amplitude* of the frame — for instance if the
estimate is a difference of two nearly equal quantities, the sign of that
difference would be visible in the amplitude.

This script tests, per node and transmitter:

* the mean amplitude of the frame against the sign of the flip;
* the change of amplitude from the previous frame, against the sign;
* the received power, grouped by its exact value, against the sign;
* the same tests on the *amplitude profile's slope* across the subcarriers,
  which is the quantity the group-delay measurement uses.

Each test is reported against a null that accounts for the number of groups
and their sizes, because a bare threshold over many groups calls noise a
signal.

Australian English spelling throughout.
"""

from __future__ import annotations

import math
import statistics
import sys
import time
from collections import defaultdict
from typing import Dict, List, Optional, Sequence, Tuple

sys.path.insert(0, ".")

from hardware import CsiCollector, CsiFrame, frames_by_node

NULL_SUBCARRIERS = {0, 28, 29, 30, 31, 32, 33}


def flip_sign(first: CsiFrame, second: CsiFrame) -> Optional[int]:
    """+1 when the whole vector kept its sign, -1 when it was negated."""
    counted = 0
    flipped = 0
    for k in range(min(len(first.csi), len(second.csi))):
        if k in NULL_SUBCARRIERS:
            continue
        a = first.csi[k]
        b = second.csi[k]
        if abs(a) < 0.05 or abs(b) < 0.05:
            continue
        counted += 1
        if (a.real * b.real + a.imag * b.imag) < 0:
            flipped += 1
    if counted == 0:
        return None
    if flipped * 2 < counted:
        return 1
    if flipped * 2 > counted:
        return -1
    return None


def mean_amplitude(frame: CsiFrame) -> float:
    """The mean amplitude over the usable subcarriers."""
    values = [
        abs(frame.csi[k])
        for k in range(len(frame.csi))
        if k not in NULL_SUBCARRIERS and abs(frame.csi[k]) > 0.05
    ]
    return statistics.mean(values) if values else 0.0


def amplitude_slope(frame: CsiFrame) -> float:
    """The slope of the amplitude across the usable subcarriers."""
    usable = [
        k
        for k in range(len(frame.csi))
        if k not in NULL_SUBCARRIERS and abs(frame.csi[k]) > 0.05
    ]
    if len(usable) < 8:
        return 0.0
    amplitudes = [abs(frame.csi[k]) for k in usable]
    mean_k = sum(usable) / len(usable)
    mean_a = sum(amplitudes) / len(amplitudes)
    denominator = sum((k - mean_k) ** 2 for k in usable)
    if denominator == 0.0:
        return 0.0
    return sum(
        (k - mean_k) * (a - mean_a) for k, a in zip(usable, amplitudes)
    ) / denominator


def correlation(xs: Sequence[float], ys: Sequence[float]) -> float:
    """The Pearson correlation of two series."""
    n = len(xs)
    if n < 10:
        return 0.0
    mean_x = sum(xs) / n
    mean_y = sum(ys) / n
    numerator = sum((x - mean_x) * (y - mean_y) for x, y in zip(xs, ys))
    denominator = math.sqrt(
        sum((x - mean_x) ** 2 for x in xs) * sum((y - mean_y) ** 2 for y in ys)
    )
    if denominator == 0.0:
        return 0.0
    return numerator / denominator


def group_excess(groups: Dict[int, List[int]]) -> Tuple[float, float]:
    """The strongest group's share and the null expectation for it."""
    usable = [signs for signs in groups.values() if len(signs) >= 5]
    if len(usable) < 2:
        return (0.5, 0.5)
    best = 0.0
    for signs in usable:
        share = max(
            sum(1 for s in signs if s > 0), sum(1 for s in signs if s < 0)
        ) / len(signs)
        best = max(best, share)
    average_n = sum(len(s) for s in usable) / len(usable)
    sd = 0.5 / math.sqrt(average_n)
    null = 0.5 + 1.9 * sd * math.sqrt(2 * math.log(len(usable)))
    return (best, null)


def main() -> int:
    """Collect and test the amplitude hypotheses."""
    duration = float(sys.argv[1]) if len(sys.argv) > 1 else 120.0
    collector = CsiCollector(bind_port=9100)
    collector.start()
    time.sleep(2.0)
    frames = collector.collect_for(duration)
    collector.stop()

    grouped = frames_by_node(frames)
    node_ids = sorted(grouped)
    print(f"nodes {node_ids}, {duration:.0f}s")

    for node_id in node_ids:
        per_mac: Dict[str, List[CsiFrame]] = defaultdict(list)
        for frame in grouped[node_id]:
            if frame.source_mac is not None:
                per_mac[frame.source_mac].append(frame)
        if not per_mac:
            continue
        mac = max(per_mac, key=lambda m: len(per_mac[m]))
        subset = [f for f in per_mac[mac] if f.n_subcarriers == 64]
        if len(subset) < 60:
            continue

        signs: List[int] = []
        amplitudes: List[float] = []
        deltas: List[float] = []
        slopes: List[float] = []
        rssis: List[int] = []
        previous: Optional[CsiFrame] = None
        for frame in subset:
            if previous is not None:
                sign = flip_sign(previous, frame)
                if sign is not None:
                    signs.append(sign)
                    amplitudes.append(mean_amplitude(frame))
                    deltas.append(mean_amplitude(frame) - mean_amplitude(previous))
                    slopes.append(amplitude_slope(frame))
                    rssis.append(frame.rssi_dbm)
            previous = frame

        if len(signs) < 40:
            continue
        print()
        print(f"=== node {node_id}, transmitter {mac}, {len(signs)} transitions ===")
        print(f"  negated {sum(1 for s in signs if s < 0)} "
              f"({sum(1 for s in signs if s < 0) / len(signs):.1%})")
        print(f"  correlation of the sign with the mean amplitude: "
              f"r = {correlation(amplitudes, signs):+.4f}")
        print(f"  correlation of the sign with the amplitude change: "
              f"r = {correlation(deltas, signs):+.4f}")
        print(f"  correlation of the sign with the amplitude slope: "
              f"r = {correlation(slopes, signs):+.4f}")
        print(f"  correlation of the sign with the received power: "
              f"r = {correlation([float(v) for v in rssis], signs):+.4f}")
        groups: Dict[int, List[int]] = defaultdict(list)
        for sign, rssi in zip(signs, rssis):
            groups[rssi].append(sign)
        share, null = group_excess(groups)
        print(f"  received power grouped by exact value: strongest group "
              f"{share:.3f} against a null of {null:.3f} "
              f"({len(groups)} groups)")
        # Also: the mean amplitude of the negated frames against the others.
        negated = [a for s, a in zip(signs, amplitudes) if s < 0]
        kept = [a for s, a in zip(signs, amplitudes) if s > 0]
        if negated and kept:
            print(f"  mean amplitude: negated {statistics.mean(negated):.4f}, "
                  f"kept {statistics.mean(kept):.4f}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
