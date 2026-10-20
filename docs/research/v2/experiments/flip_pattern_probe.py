"""Is the per-frame sign flip a deterministic pattern after all?

The flip is not a post-processing negation of the CSI buffer — there is no
sign-mask xor anywhere in the ROM — so it is intrinsic to the estimator's
arithmetic. If the estimator's decision is driven by a counter or a state
machine rather than by noise, it leaves a pattern in the data.

This script looks for one:

* the **autocorrelation** of the sign sequence, at lags 1 to 64. A flip
  driven by a counter with a short period shows up as a peak.
* the sign against the low bits of the node's **capture sequence number** and
  of the node's **own microsecond clock**, and of the **802.11 sequence
  number**. A flip driven by a counter's parity shows up as a dependence.
* the sign against the **received power** and the **frame layout**, again as
  a dependence.

A null result here is what closes the question: the flip would then be noise
in the estimator rather than a state machine, and there is nothing to decode.

Australian English spelling throughout.
"""

from __future__ import annotations

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


def autocorrelation(series: Sequence[int], lag: int) -> float:
    """The correlation of a ±1 series with itself at a lag."""
    n = len(series)
    if n <= lag + 2:
        return 0.0
    a = series[: n - lag]
    b = series[lag:]
    mean_a = sum(a) / len(a)
    mean_b = sum(b) / len(b)
    numerator = sum((x - mean_a) * (y - mean_b) for x, y in zip(a, b))
    denominator = math.sqrt(
        sum((x - mean_a) ** 2 for x in a) * sum((y - mean_b) ** 2 for y in b)
    )
    if denominator == 0.0:
        return 0.0
    return numerator / denominator


def dependence(groups: Dict[int, List[int]]) -> Tuple[float, float, float]:
    """The strongest group's sign share, the null expectation and the excess.

    A predictor that carries information gives a share well above the null.
    The null matters: with `G` groups of about `n` samples each and a fair
    coin, the *largest* group's share is already well above one half, so a
    bare threshold would call noise a signal.
    """
    if len(groups) < 2:
        return (0.5, 0.5, 0.0)
    best = 0.0
    bias = 0.0
    sizes: List[int] = []
    for signs in groups.values():
        if len(signs) < 5:
            continue
        sizes.append(len(signs))
        share = max(
            sum(1 for s in signs if s > 0), sum(1 for s in signs if s < 0)
        ) / len(signs)
        best = max(best, share)
        bias = max(bias, abs(sum(signs) / len(signs)))
    if not sizes:
        return (0.5, 0.5, 0.0)
    # The expected largest share of `G` independent fair groups of the
    # observed sizes: the 95th percentile of the maximum of G normals with
    # the group's own standard deviation.
    average_n = sum(sizes) / len(sizes)
    sd = 0.5 / math.sqrt(average_n)
    null = 0.5 + 1.9 * sd * math.sqrt(2 * math.log(len(sizes)))
    return (best, null, best - null)


def main() -> int:
    """Collect and look for a pattern in the flip."""
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
        sequences: List[int] = []
        clocks: List[int] = []
        rx_seqs: List[int] = []
        rssis: List[int] = []
        previous: Optional[CsiFrame] = None
        for frame in subset:
            if previous is not None:
                sign = flip_sign(previous, frame)
                if sign is not None:
                    signs.append(sign)
                    sequences.append(frame.sequence & 0xFFFF)
                    clocks.append(frame.device_time_us & 0xFFFF)
                    rx_seqs.append(
                        (frame.rx_seq if frame.rx_seq is not None else 0) & 0xFFFF
                    )
                    rssis.append(frame.rssi_dbm)
            previous = frame

        if len(signs) < 60:
            continue
        print()
        print(f"=== node {node_id}, transmitter {mac}, {len(signs)} transitions ===")
        print(f"  negated: {sum(1 for s in signs if s < 0)} "
              f"({sum(1 for s in signs if s < 0) / len(signs):.1%})")

        print("  autocorrelation of the sign sequence:")
        peaks = []
        for lag in range(1, 65):
            value = autocorrelation(signs, lag)
            peaks.append((value, lag))
        peaks.sort(reverse=True)
        for value, lag in peaks[:4]:
            print(f"    lag {lag:>3}: r = {value:+.4f}")
        # The null: a fair sequence of this length has a correlation of about
        # 1/sqrt(n) at each lag, and the largest of 64 such is a little more.
        null = 1.9 / math.sqrt(len(signs)) * math.sqrt(2 * math.log(64))
        print(f"    largest |r| = {peaks[0][0]:+.4f} against a null of "
              f"{null:.4f} (64 lags, n = {len(signs)})")

        print("  dependence of the sign on:")
        for name, values in (
            ("the capture sequence number's low bits", sequences),
            ("the node clock's low bits", clocks),
            ("the 802.11 sequence number's low bits", rx_seqs),
            ("the received power", rssis),
        ):
            for width in (1, 2, 4, 8, 16):
                groups: Dict[int, List[int]] = defaultdict(list)
                for sign, value in zip(signs, values):
                    groups[value & (width - 1)].append(sign)
                share, null_share, excess = dependence(groups)
                if excess > 0.04:
                    print(f"    {name} mod {width:>2}: "
                          f"best group {share:.3f}, null {null_share:.3f}, "
                          f"excess {excess:+.3f}")
        print("    (only dependences above the null are listed)")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
