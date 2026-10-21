"""Does the receiver's own per-frame metadata predict the sign flip?

The ESP32-S3 negates the whole channel vector from frame to frame, and the
negation is independent at each receiver. The node now reports the receiver's
per-frame link parameters — the antenna, the rate, the modulation and coding
scheme, the aggregation flag, the channel-estimate smoothing flag and the
sounding flag — and its own receive timestamp.

The antenna is the interesting one: it is the only per-frame binary choice
the receiver makes about *which* channel response it is reporting, and a
receiver that switched between two paths differing by a half wavelength would
negate the whole vector, coherently across every subcarrier, which is exactly
what the measured flip looks like.

This script tests each reported field against the flip:

* does the antenna predict it?
* does any other field?
* does the receive timestamp — that is, the frame's arrival time — predict it?

A field that predicts the flip would let the core recover the sign and so
recover the absolute cross-node phase.

Australian English spelling throughout.
"""

from __future__ import annotations

import cmath
import math
import statistics
import sys
import time
from collections import Counter, defaultdict
from typing import Dict, List, Optional, Sequence, Tuple

sys.path.insert(0, ".")

from hardware import CsiCollector, CsiFrame, frames_by_node

NULL_SUBCARRIERS = {0, 28, 29, 30, 31, 32, 33}

# The fields the node packs into `rx_flags`, and the bits each takes.
RX_FLAG_FIELDS = {
    "antenna": (0, 1),
    "rate": (1, 5),
    "mcs": (6, 7),
    "aggregation": (13, 1),
    "smoothing": (14, 1),
    "sounding": (15, 1),
}


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


def field_value(flags: int, name: str) -> int:
    """One packed field of the node's `rx_flags`."""
    offset, width = RX_FLAG_FIELDS[name]
    return (flags >> offset) & ((1 << width) - 1)


def main() -> int:
    """Collect and test the reported metadata against the flip."""
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
        if len(subset) < 40:
            continue

        print()
        print(f"=== node {node_id}, transmitter {mac}, {len(subset)} frames ===")

        # The flip between consecutive frames, and the metadata of the second.
        flips: List[int] = []
        meta: List[Dict[str, int]] = []
        stamps: List[int] = []
        previous: Optional[CsiFrame] = None
        for frame in subset:
            if previous is not None:
                sign = flip_sign(previous, frame)
                if sign is not None:
                    flips.append(sign)
                    meta.append({name: field_value(frame.rx_flags, name)
                                 for name in RX_FLAG_FIELDS})
                    stamps.append(frame.rx_timestamp)
            previous = frame
        if len(flips) < 30:
            print("  too few clean transitions")
            continue

        negated = sum(1 for sign in flips if sign < 0)
        print(f"  transitions: {len(flips)}, negated {negated} "
              f"({negated / len(flips):.0%})")

        # What values do the fields take?
        for name in RX_FLAG_FIELDS:
            values = Counter(m[name] for m in meta)
            distinct = len(values)
            summary = ", ".join(
                f"{value}:{count}" for value, count in values.most_common(4)
            )
            print(f"    {name:<12} {distinct} distinct value(s): {summary}")

            # Does the field predict the flip? A predictor would make one
            # value's flips overwhelmingly one sign.
            if distinct < 2:
                continue
            groups: Dict[int, List[int]] = defaultdict(list)
            for sign, record in zip(flips, meta):
                groups[record[name]].append(sign)
            best = 0.0
            for value, signs in groups.items():
                if len(signs) < 10:
                    continue
                share = max(
                    sum(1 for s in signs if s > 0), sum(1 for s in signs if s < 0)
                ) / len(signs)
                best = max(best, share)
            # The null: the expected largest share over that many groups.
            groups_used = max(1, len(groups))
            average_n = len(flips) / groups_used
            null = 0.5 + 1.9 * 0.5 / math.sqrt(average_n) * math.sqrt(
                2 * math.log(groups_used)
            )
            verdict = "PREDICTS" if best > null + 0.05 else "no"
            print(f"                  best group {best:.3f} against a null of "
                  f"{null:.3f}: {verdict}")

        # The receive timestamp: does the flip follow the clock?
        gaps = [
            stamps[index + 1] - stamps[index] for index in range(len(stamps) - 1)
        ]
        if gaps:
            print(f"    receive timestamp: {len(set(stamps))} distinct values; "
                  f"gaps: median {statistics.median(gaps)} us, "
                  f"min {min(gaps)}, max {max(gaps)}")
            halves: Dict[int, List[int]] = defaultdict(list)
            for sign, stamp in zip(flips, stamps):
                halves[stamp & 1].append(sign)
            for parity, signs in sorted(halves.items()):
                if len(signs) < 20:
                    continue
                share = max(
                    sum(1 for s in signs if s > 0), sum(1 for s in signs if s < 0)
                ) / len(signs)
                print(f"                  timestamp parity {parity}: "
                      f"flip share {share:.3f} over {len(signs)} frames")

    _cross_node_antenna_test(grouped, node_ids)
    return 0


def _cross_node_antenna_test(
    grouped: Dict[int, List[CsiFrame]], node_ids: Sequence[int]
) -> None:
    """Do the two nodes' antenna bits differ exactly when their signs differ?

    This is the sharp version of the test. If the antenna is what decides the
    sign, then for the *same* packet the two nodes should report different
    antennas about as often as they report different signs, and the two should
    agree with each other frame by frame.
    """
    for first in range(len(node_ids)):
        for second in range(first + 1, len(node_ids)):
            node_a, node_b = node_ids[first], node_ids[second]
            by_key_a: Dict[Tuple[str, int], CsiFrame] = {}
            by_key_b: Dict[Tuple[str, int], CsiFrame] = {}
            for frame in grouped[node_a]:
                if frame.source_mac is None or frame.rx_seq is None:
                    continue
                by_key_a[(frame.source_mac, frame.rx_seq)] = frame
            for frame in grouped[node_b]:
                if frame.source_mac is None or frame.rx_seq is None:
                    continue
                by_key_b[(frame.source_mac, frame.rx_seq)] = frame
            shared = sorted(set(by_key_a) & set(by_key_b))
            if len(shared) < 30:
                continue

            ant_differs = 0
            sign_agrees = 0
            ant_and_sign_agree = 0
            for key in shared:
                a = by_key_a[key]
                b = by_key_b[key]
                ant_a = field_value(a.rx_flags, "antenna")
                ant_b = field_value(b.rx_flags, "antenna")
                differs = ant_a != ant_b
                ant_differs += 1 if differs else 0
                # The sign agreement, from the raw vectors.
                counted = 0
                flipped = 0
                for k in range(min(len(a.csi), len(b.csi))):
                    if k in NULL_SUBCARRIERS:
                        continue
                    va = a.csi[k]
                    vb = b.csi[k]
                    if abs(va) < 0.05 or abs(vb) < 0.05:
                        continue
                    counted += 1
                    if (va.real * vb.real + va.imag * vb.imag) < 0:
                        flipped += 1
                if counted == 0:
                    continue
                same_sign = flipped * 2 < counted
                sign_agrees += 1 if same_sign else 0
                if same_sign == (not differs):
                    ant_and_sign_agree += 1

            total = len(shared)
            print()
            print(f"=== node {node_a} <-> node {node_b}, the same packets ===")
            print(f"  {total} packets seen by both")
            print(f"  the antenna bits differ on {ant_differs} of them "
                  f"({ant_differs / total:.0%})")
            print(f"  the signs agree on {sign_agrees} of them "
                  f"({sign_agrees / total:.0%})")
            print(f"  the antenna difference and the sign difference agree on "
                  f"{ant_and_sign_agree} of them "
                  f"({ant_and_sign_agree / total:.0%}; chance is 50%)")


if __name__ == "__main__":
    raise SystemExit(main())
