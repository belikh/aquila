"""Resolving the ESP32-S3's per-frame sign flip, and the cross-node residual
that follows.

The ESP32-S3 reports each frame's complex channel with an arbitrary sign: the
whole vector is negated from frame to frame, so a frame's absolute phase is
known only modulo pi. Two nodes measuring the same transmission do not agree
on the sign — the flips are independent at each receiver — so the raw
cross-node phase difference for the same packet is either about zero or about
pi, and which one it is looks random.

That ambiguity is a property of the receiver, not of the channel, and it is
resolvable. The channel between two fixed points does not change appreciably
between two frames from the same transmitter a few tens of milliseconds apart:
the phase at a given subcarrier moves by the carrier-frequency drift, which is
a few milliradians. A sign flip moves it by pi. The two are three orders of
magnitude apart, so the sign is recoverable from the data alone, by choosing,
for each frame, the sign that keeps the channel continuous with the frame
before it.

Resolving the sign at each node independently is what makes the cross-node
measurement meaningful: once both nodes' phases are referred to the same
continuous channel, their difference is the closure phase.

The assumption this rests on is stated plainly: the channel is quasi-static
over the interval between two frames of the same transmitter. That is the
same assumption every amplitude-based sensing method already rests on, and it
is checked afterwards by the spread of the resolved residual — a channel that
genuinely moved would show up as a large residual.

Australian English spelling throughout.
"""

from __future__ import annotations

import cmath
import math
import statistics
import sys
import time
from collections import defaultdict
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Sequence, Tuple

sys.path.insert(0, ".")

from hardware import CsiCollector, CsiFrame, frames_by_node
from m2_two_node_hardware import (
    AMBER_MAX_RAD,
    GREEN_MAX_RAD,
    MATCH_WINDOW_US,
    MIN_MATCHED_PAIRS,
    MIN_SIGNAL,
    MIN_SIGNAL_SHARE,
    group_by_transmitter,
    measure_link,
    usable_subcarriers,
)

# The subcarriers the ESP32-S3 leaves null: the carrier at the centre of the
# legacy long training field, and the null band in the middle of the payload.
NULL_SUBCARRIERS = {0, 28, 29, 30, 31, 32, 33}


@dataclass
class Resolution:
    """The outcome of resolving one node's per-frame signs."""

    transmitter: str
    layout: int
    n_frames: int
    n_flips: int
    # The correlation between consecutive frames, before resolution. A value
    # near -1 means the whole vector was negated.
    mean_signed_correlation: float = 0.0
    resolved: bool = False


def correlation(a: CsiFrame, b: CsiFrame, subcarriers: Sequence[int]) -> float:
    """The normalised complex correlation of two frames' channel vectors.

    The correlation is the sum over the subcarriers of the complex product,
    normalised by the amplitude. It is near +1 when two frames show the same
    channel and near -1 when the whole vector was negated.
    """
    total = 0j
    for k in subcarriers:
        va = a.csi[k]
        vb = b.csi[k]
        if abs(va) < MIN_SIGNAL or abs(vb) < MIN_SIGNAL:
            continue
        total += va.conjugate() * vb
    if abs(total) == 0.0:
        return 0.0
    return (total / abs(total)).real


def resolve_signs(
    frames: Sequence[CsiFrame], subcarriers: Sequence[int]
) -> Tuple[List[int], Resolution]:
    """Resolve the per-frame sign of one node's frames of one transmitter.

    Returns the sign of each frame (+1 or -1) and a summary. The sign of the
    first frame is arbitrary; only the differences matter.
    """
    signs: List[int] = []
    flips = 0
    correlations: List[float] = []
    previous_sign = 1
    for index, frame in enumerate(frames):
        if index == 0:
            signs.append(1)
            continue
        corr = correlation(frames[index - 1], frame, subcarriers)
        correlations.append(corr)
        sign = previous_sign
        if corr < 0.0:
            sign = -sign
            flips += 1
        signs.append(sign)
        previous_sign = sign

    resolution = Resolution(
        transmitter="",
        layout=0,
        n_frames=len(frames),
        n_flips=flips,
        mean_signed_correlation=(
            statistics.mean(correlations) if correlations else 0.0
        ),
        resolved=True,
    )
    return signs, resolution


def apply_signs(frames: Sequence[CsiFrame], signs: Sequence[int]) -> List[CsiFrame]:
    """Return the frames with their sign applied, as new frames."""
    corrected: List[CsiFrame] = []
    for frame, sign in zip(frames, signs):
        if sign > 0:
            corrected.append(frame)
            continue
        corrected.append(
            CsiFrame(
                node_id=frame.node_id,
                sequence=frame.sequence,
                device_time_us=frame.device_time_us,
                boot_epoch=frame.boot_epoch,
                subcarrier_offset=frame.subcarrier_offset,
                n_subcarriers=frame.n_subcarriers,
                centre_freq_hz=frame.centre_freq_hz,
                bandwidth_mhz=frame.bandwidth_mhz,
                rssi_dbm=frame.rssi_dbm,
                noise_floor_dbm=frame.noise_floor_dbm,
                first_word_invalid=frame.first_word_invalid,
                source_mac=frame.source_mac,
                csi=[-value for value in frame.csi],
                host_arrival_us=frame.host_arrival_us,
            )
        )
    return corrected


def measure_resolved_pair(
    node_a: int,
    node_b: int,
    frames_a: Sequence[CsiFrame],
    frames_b: Sequence[CsiFrame],
) -> Dict[str, object]:
    """Measure the cross-node residual with the signs resolved.

    Both nodes' frames are resolved independently, and the frames of the same
    transmission are then paired on the transmitter address and the arrival
    time.
    """
    per_mac_a: Dict[str, List[CsiFrame]] = defaultdict(list)
    per_mac_b: Dict[str, List[CsiFrame]] = defaultdict(list)
    for frame in frames_a:
        if frame.source_mac is not None:
            per_mac_a[frame.source_mac].append(frame)
    for frame in frames_b:
        if frame.source_mac is not None:
            per_mac_b[frame.source_mac].append(frame)

    common = sorted(set(per_mac_a) & set(per_mac_b))
    best_p95 = 0.0
    best_conditioned = 0.0
    total_pairs = 0
    worst_links: List[Tuple[float, str, int, int]] = []
    resolutions: List[Resolution] = []
    per_link: List[Tuple[str, int, int, float]] = []

    for transmitter in common:
        for layout in (64, 128):
            subset_a = [f for f in per_mac_a[transmitter] if f.n_subcarriers == layout]
            subset_b = [f for f in per_mac_b[transmitter] if f.n_subcarriers == layout]
            if len(subset_a) < MIN_MATCHED_PAIRS or len(subset_b) < MIN_MATCHED_PAIRS:
                continue

            usable_a = [
                k
                for k in range(len(subset_a[0].csi))
                if k not in NULL_SUBCARRIERS and abs(subset_a[0].csi[k]) >= MIN_SIGNAL
            ]
            if len(usable_a) < 8:
                continue

            signs_a, resolution_a = resolve_signs(subset_a, usable_a)
            signs_b, resolution_b = resolve_signs(subset_b, usable_a)
            for resolution in (resolution_a, resolution_b):
                resolution.transmitter = transmitter
                resolution.layout = layout
            resolutions.extend((resolution_a, resolution_b))

            corrected_a = apply_signs(subset_a, signs_a)
            corrected_b = apply_signs(subset_b, signs_b)

            times_b = [frame.host_arrival_us for frame in corrected_b]
            pairs: List[Tuple[CsiFrame, CsiFrame]] = []
            used: set[int] = set()
            for frame_a in corrected_a:
                matches = [
                    index
                    for index, moment in enumerate(times_b)
                    if index not in used
                    and abs(moment - frame_a.host_arrival_us) <= MATCH_WINDOW_US
                ]
                if len(matches) != 1:
                    continue
                used.add(matches[0])
                pairs.append((frame_a, corrected_b[matches[0]]))

            total_pairs += len(pairs)
            if len(pairs) < MIN_MATCHED_PAIRS:
                continue

            usable = [
                subcarrier
                for subcarrier in usable_a
                if subcarrier in usable_subcarriers(pairs)
            ]
            link_p95 = 0.0
            for subcarrier in usable:
                link = measure_link(transmitter, layout, pairs, subcarrier)
                if link is None:
                    continue
                worst_links.append((link.p95_rad, transmitter, layout, subcarrier))
                best_p95 = max(best_p95, link.p95_rad)
                best_conditioned = max(best_conditioned, link.conditioned_p95_rad)
                link_p95 = max(link_p95, link.p95_rad)
            per_link.append((transmitter, layout, len(pairs), link_p95))

    worst_links.sort(reverse=True)
    return {
        "n_pairs": total_pairs,
        "p95_rad": best_p95,
        "conditioned_p95_rad": best_conditioned,
        "worst": worst_links[:5],
        "per_link": per_link,
        "resolutions": resolutions,
    }


def main() -> int:
    """Collect from the live nodes and measure the sign-resolved residual."""
    duration = float(sys.argv[1]) if len(sys.argv) > 1 else 150.0
    collector = CsiCollector(bind_port=9100)
    collector.start()
    time.sleep(2.0)
    frames = collector.collect_for(duration)
    collector.stop()

    grouped = frames_by_node(frames)
    node_ids = sorted(grouped)
    print(f"=== Aquila two-node M2, signs resolved ===")
    print(f"nodes {node_ids} over {duration:.0f}s")
    print(f"frames: { {n: len(grouped[n]) for n in node_ids} }")

    worst_band = "green"
    for first in range(len(node_ids)):
        for second in range(first + 1, len(node_ids)):
            node_a = node_ids[first]
            node_b = node_ids[second]
            outcome = measure_resolved_pair(
                node_a, node_b, grouped[node_a], grouped[node_b]
            )
            p95 = float(outcome["p95_rad"])
            band = (
                "green"
                if p95 < GREEN_MAX_RAD
                else "amber"
                if p95 < AMBER_MAX_RAD
                else "red"
            )
            if band == "red":
                worst_band = "red"
            elif band == "amber" and worst_band == "green":
                worst_band = "amber"
            print()
            print(
                f"  node {node_a} <-> node {node_b}: band={band} "
                f"p95={p95:.4f} rad, pairs={outcome['n_pairs']}"
            )
            flips = sum(r.n_flips for r in outcome["resolutions"])  # type: ignore[union-attr]
            frames_seen = sum(r.n_frames for r in outcome["resolutions"])  # type: ignore[union-attr]
            if frames_seen:
                print(
                    f"      sign flips: {flips} of {frames_seen - len(outcome['resolutions'])} "
                    f"frame transitions ({flips / max(1, frames_seen - len(outcome['resolutions'])):.0%})"
                )
            for worst in outcome["worst"]:  # type: ignore[union-attr]
                print(
                    f"      worst subcarrier sc{worst[3]} (tx {worst[1]}, "
                    f"layout {worst[2]}): p95={worst[0]:.4f} rad"
                )
            print("      per transmitter and layout:")
            for transmitter, layout, pairs, link_p95 in sorted(
                outcome["per_link"],  # type: ignore[union-attr]
                key=lambda row: -row[3],
            ):
                print(
                    f"        tx {transmitter} layout {layout}: "
                    f"{pairs} pairs, worst subcarrier p95={link_p95:.4f} rad"
                )

    print()
    print(f"worst band: {worst_band}")
    print(
        "BCPT cleared over the air"
        if worst_band != "red"
        else "BCPT hardware-gated"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
