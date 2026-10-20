"""Is the per-frame sign flip present in both CSI layouts, and does the
LLTF/HT-LTF merge explain it?

The ESP-IDF header describes `ltf_merge_en` as "generate htlft data by
averaging lltf and ht_ltf data when receiving HT packet. Otherwise, use ht_ltf
data directly". The two training fields give two independently computed
channel estimates, and averaging them is only meaningful if they agree.

If the flip comes from the merge, it must appear in the 128-subcarrier layout
(the one with a high-throughput field to merge) and *not* in the
64-subcarrier layout (the legacy field alone, which the merge cannot touch).
If it appears in both, the merge is not the cause.

The script also reports, for each layout, the share of consecutive frames
whose whole channel vector is negated — measured as the share of subcarriers
whose sign flipped between the two frames, which is one when the vector was
negated as a whole and zero when it was not.

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

NULL_SUBCARRIERS = {0, 28, 29, 30, 31, 32, 33}


def flip_share(first: CsiFrame, second: CsiFrame) -> Optional[float]:
    """The share of subcarriers whose sign differs between two frames."""
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
    if not counted:
        return None
    return flipped / counted


def main() -> int:
    """Collect and report the flip structure per layout."""
    duration = float(sys.argv[1]) if len(sys.argv) > 1 else 60.0
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
        print()
        print(f"=== node {node_id}, transmitter {mac} ===")
        for layout in (64, 128):
            subset = [f for f in per_mac[mac] if f.n_subcarriers == layout]
            if len(subset) < 20:
                continue
            shares = []
            for index in range(len(subset) - 1):
                share = flip_share(subset[index], subset[index + 1])
                if share is not None:
                    shares.append(share)
            whole = sum(1 for s in shares if s > 0.9)
            none = sum(1 for s in shares if s < 0.1)
            mixed = len(shares) - whole - none
            amplitudes = [
                abs(frame.csi[k])
                for frame in subset
                for k in range(min(64, len(frame.csi)))
                if k not in NULL_SUBCARRIERS and abs(frame.csi[k]) > 0.05
            ]
            print(
                f"  layout {layout}: n={len(subset)} transitions={len(shares)} "
                f"whole-vector negated={whole} unchanged={none} mixed={mixed}"
            )
            if amplitudes:
                print(
                    f"    amplitude: mean={statistics.mean(amplitudes):.3f} "
                    f"sd={statistics.pstdev(amplitudes):.3f} "
                    f"min={min(amplitudes):.3f} max={max(amplitudes):.3f}"
                )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
