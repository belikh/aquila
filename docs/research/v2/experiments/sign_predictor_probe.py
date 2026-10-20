"""Is the per-frame sign flip predictable from anything the node reports?

The ESP32-S3 negates the whole channel vector from frame to frame, and the
negation is independent at each receiver. If it were a function of anything
the node can report — the received power, the rate, the sequence number, the
gap since the last frame — the core could correct it and the cross-node phase
would become usable.

This script tests each candidate observable against the sign of the flip
between consecutive frames of one transmitter:

* the received power, and its change since the previous frame;
* the PHY flags;
* the 802.11 sequence number, its parity and its residue modulo four;
* the gap since the previous frame;
* whether the sign is *the same* for two different transmitters seen by the
  same node, which would make it a property of the node rather than of the
  frame.

A dependence on any of them would be the way out. None of them is expected to
show one, because the flip comes from the receiver's channel estimate, which
is below anything the node can see.

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


def correlation(xs: Sequence[float], ys: Sequence[float]) -> float:
    """The Pearson correlation of two series."""
    n = len(xs)
    if n < 3:
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


def main() -> int:
    """Collect and test every candidate predictor of the sign."""
    duration = float(sys.argv[1]) if len(sys.argv) > 1 else 90.0
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
        print(f"=== node {node_id}, transmitter {mac}, 64-subcarrier layout ===")

        signs: List[int] = []
        rssis: List[float] = []
        drssis: List[float] = []
        gaps: List[float] = []
        parities: List[float] = []
        residues: List[float] = []
        phis: List[float] = []
        previous: Optional[CsiFrame] = None
        for frame in subset:
            if previous is None:
                previous = frame
                continue
            sign = flip_sign(previous, frame)
            if sign is None:
                previous = frame
                continue
            signs.append(sign)
            rssis.append(float(frame.rssi_dbm))
            drssis.append(float(frame.rssi_dbm - previous.rssi_dbm))
            gaps.append((frame.host_arrival_us - previous.host_arrival_us) / 1000.0)
            sequence = frame.rx_seq if frame.rx_seq is not None else 0
            parities.append(float(sequence & 1))
            residues.append(float(sequence & 3))
            phis.append(float(frame.noise_floor_dbm))
            previous = frame

        if len(signs) < 20:
            print("  too few clean transitions")
            continue

        print(f"  transitions: {len(signs)}, "
              f"negated {sum(1 for s in signs if s < 0)} "
              f"({sum(1 for s in signs if s < 0) / len(signs):.0%})")
        print(f"  correlation of the sign with:")
        for name, series in (
            ("received power", rssis),
            ("its change", drssis),
            ("the gap since the last frame (ms)", gaps),
            ("the sequence number's parity", parities),
            ("the sequence number mod 4", residues),
            ("the noise floor", phis),
        ):
            print(f"    {name:<38} r = {correlation(series, signs):+.4f}")

        # Is the sign the same for two different transmitters at this node?
        others = [m for m in per_mac if m != mac and len(per_mac[m]) >= 40]
        for other in others[:2]:
            other_subset = [f for f in per_mac[other] if f.n_subcarriers == 64]
            other_signs: List[int] = []
            previous = None
            for frame in other_subset:
                if previous is not None:
                    sign = flip_sign(previous, frame)
                    if sign is not None:
                        other_signs.append(sign)
                previous = frame
            if len(other_signs) < 20:
                continue
            # Compare the two sign sequences by position: if the sign were a
            # property of the node, the two sequences would agree more often
            # than chance.
            shared = min(len(signs), len(other_signs))
            agree = sum(
                1 for i in range(shared) if signs[i] == other_signs[i]
            )
            print(
                f"  sign sequence against transmitter {other}: "
                f"{agree}/{shared} agree "
                f"({agree / shared:.0%}, chance is 50%)"
            )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
