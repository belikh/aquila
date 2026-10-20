"""The phase slope across subcarriers: a gauge-invariant quantity that survives
the ESP32-S3's sign flip.

The ESP32-S3 reports each frame's complex channel with an arbitrary sign: the
whole vector is negated from frame to frame, so the *absolute* phase is
ambiguous modulo pi. A whole-vector negation shifts every subcarrier's phase
by the same pi, so the phase *differences* between subcarriers are untouched.

The phase slope across subcarriers is therefore the one phase-derived quantity
that is still meaningful on this hardware. It is the per-link group delay, and
it is what the concepts document's slope-based approaches need
(`21-3d-imaging-concepts.md`, approaches 4 and 7).

This script measures it: for each frame, the phase across the usable
subcarriers is fitted as a straight line, and the slope's stability over time
is reported. A slope that is stable to a small fraction of a radian per
subcarrier would mean the per-link delay is measurable and stable.

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

# The null band the ESP32-S3 leaves in the middle of the payload, and the DC
# carrier at the centre of the legacy long training field.
NULL_SUBCARRIERS = {0, 28, 29, 30, 31, 32, 33}


def usable_subcarriers(frame: CsiFrame) -> List[int]:
    """The subcarriers that carry signal in this frame."""
    return [
        k
        for k in range(len(frame.csi))
        if k not in NULL_SUBCARRIERS and abs(frame.csi[k]) > 0.05
    ]


def fit_slope(
    subcarriers: Sequence[int], phases: Sequence[float]
) -> Tuple[float, float]:
    """Fit phase = intercept + slope * subcarrier. Returns (intercept, slope)."""
    n = len(subcarriers)
    if n < 4:
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


def main() -> int:
    """Collect and report the phase-slope stability."""
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

        for layout in (64, 128):
            subset = [f for f in per_mac[mac] if f.n_subcarriers == layout]
            if len(subset) < 20:
                continue
            slopes: List[float] = []
            times: List[float] = []
            for frame in subset:
                usable = usable_subcarriers(frame)
                if len(usable) < 8:
                    continue
                phases = [cmath.phase(frame.csi[k]) for k in usable]
                # The phase has an arbitrary sign per frame; the *slope* is
                # invariant to it, but a whole-vector negation also shifts the
                # intercept by pi, so the fit is done on the unwrapped phase
                # across the subcarriers, which is smooth within a frame.
                unwrapped: List[float] = []
                for index, phase in enumerate(phases):
                    if index == 0:
                        unwrapped.append(phase)
                        continue
                    rotations = round(
                        (unwrapped[-1] - phase) / (2.0 * math.pi)
                    )
                    unwrapped.append(phase + rotations * 2.0 * math.pi)
                _intercept, slope = fit_slope(usable, unwrapped)
                slopes.append(slope)
                times.append(frame.host_arrival_us / 1e6)

            if len(slopes) < 20:
                continue
            spread = statistics.pstdev(slopes)
            mean = statistics.mean(slopes)
            ordered = sorted(abs(s - mean) for s in slopes)
            p95 = ordered[int(math.ceil(0.95 * len(ordered))) - 1]
            # The group delay implied by the slope, in nanoseconds.
            delay_ns = mean / (2.0 * math.pi * 312_500.0) * 1e9
            print()
            print(
                f"node {node_id} transmitter {mac} layout {layout}: "
                f"n={len(slopes)}"
            )
            print(
                f"  slope: mean={mean:+.6f} rad/subcarrier, "
                f"sd={spread:.6f}, p95|dev|={p95:.6f}"
            )
            print(
                f"  implied group delay: {delay_ns:+.2f} ns "
                f"(path difference {delay_ns * 0.299792458:.3f} m)"
            )
            print(f"  first 10 slopes: {[round(s, 5) for s in slopes[:10]]}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
