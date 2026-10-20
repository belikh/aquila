"""The structure of the ESP32-S3's per-packet phase ambiguity.

The cross-node residual came out bimodal: the phase difference between two
nodes for the same packet is either about zero or about pi, and which one it
is looks random. This script characterises the ambiguity on a single node,
where nothing is confounded by a second receiver.

It reports, per subcarrier and per CSI layout:

* the share of consecutive frames whose phase differs by about pi (a sign
  flip) rather than by a small amount (a genuine phase change);
* whether the flip is *coherent across subcarriers* — that is, whether the
  whole complex vector flips sign together, which is what a hardware sign
  flip looks like, as opposed to one subcarrier misbehaving;
* the phase series itself, so the structure can be seen rather than inferred.

A whole-vector sign flip is not fatal: it can be resolved, because the
amplitude does not flip. The received power and the amplitude both survive a
sign flip, so the sign can be recovered from any subcarrier with a stable
amplitude, and the phase becomes usable.

Australian English spelling throughout.
"""

from __future__ import annotations

import cmath
import math
import sys
import time
from collections import Counter, defaultdict
from typing import Dict, List, Sequence, Tuple

sys.path.insert(0, ".")

from hardware import CsiCollector, CsiFrame, frames_by_node


def flip_share(phases: Sequence[float]) -> Tuple[float, float]:
    """The share of consecutive phase steps near pi, and near zero."""
    if len(phases) < 2:
        return (0.0, 0.0)
    steps = [cmath.phase(phases[i + 1] - phases[i]) for i in range(len(phases) - 1)]
    flipped = sum(1 for s in steps if abs(abs(s) - math.pi) < 0.5)
    settled = sum(1 for s in steps if abs(s) < 0.5)
    total = len(steps)
    return (flipped / total, settled / total)


def main() -> int:
    """Collect and report the per-packet phase structure."""
    duration = float(sys.argv[1]) if len(sys.argv) > 1 else 60.0
    collector = CsiCollector(bind_port=9100)
    collector.start()
    time.sleep(2.0)
    frames = collector.collect_for(duration)
    collector.stop()

    grouped = frames_by_node(frames)
    node_ids = sorted(grouped)
    node_id = node_ids[0]
    print(f"node {node_id}, {duration:.0f}s")

    per_mac: Dict[str, List[CsiFrame]] = defaultdict(list)
    for frame in grouped[node_id]:
        if frame.source_mac is not None:
            per_mac[frame.source_mac].append(frame)

    counts = Counter({mac: len(fs) for mac, fs in per_mac.items()})
    mac = counts.most_common(1)[0][0]
    print(f"transmitter {mac} ({counts[mac]} frames)")

    # The phase series, split by CSI layout: index k is a different physical
    # subcarrier in the two layouts, so they are never mixed.
    for layout in (64, 128):
        subset = [f for f in per_mac[mac] if f.n_subcarriers == layout]
        if len(subset) < 10:
            continue
        print()
        print(f"=== layout {layout} subcarriers ({len(subset)} frames) ===")
        for subcarrier in (5, 10, 20, 30, 40, 50, 60):
            if subcarrier >= len(subset[0].csi):
                continue
            phases: List[float] = []
            amplitudes: List[float] = []
            for frame in subset:
                value = frame.csi[subcarrier]
                if abs(value) < 0.05:
                    continue
                phases.append(cmath.phase(value))
                amplitudes.append(abs(value))
            if len(phases) < 10:
                continue
            flipped, settled = flip_share(phases)
            # Two clusters?
            histogram = Counter(
                int((p + math.pi) / (2 * math.pi) * 12) for p in phases
            )
            occupied = sorted(histogram.items())
            print(
                f"  sc{subcarrier:<3} n={len(phases):<5} "
                f"flip={flipped:.3f} settle={settled:.3f} "
                f"amp[{min(amplitudes):.3f},{max(amplitudes):.3f}] "
                f"bins={occupied}"
            )

        # Is the flip coherent across subcarriers? Take the first pair of
        # frames and compare the sign of every subcarrier.
        print("  --- coherence across subcarriers (frame-to-frame sign) ---")
        signs: List[float] = []
        for index in range(len(subset) - 1):
            first = subset[index].csi
            second = subset[index + 1].csi
            agree = 0
            counted = 0
            for k in range(min(len(first), len(second))):
                if abs(first[k]) < 0.05 or abs(second[k]) < 0.05:
                    continue
                counted += 1
                # A whole-vector sign flip puts the two points on opposite
                # sides of the origin: their product is negative.
                if (first[k].real * second[k].real
                        + first[k].imag * second[k].imag) < 0:
                    agree += 1
            if counted:
                signs.append(agree / counted)
        if signs:
            histogram = Counter(int(s * 5) for s in signs)
            print(f"    share of subcarriers that flipped, per frame pair: "
                  f"{sorted(histogram.items())}")
            print(f"    mean share flipped: {sum(signs) / len(signs):.3f}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
