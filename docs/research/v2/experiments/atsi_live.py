"""ATSI on the live nodes: node-local power and the occupancy detector.

INTEGRATION.md §5. ATSI — ambient sensing of occupancy — is receive-only and
uses no phase, so it is unaffected by the ESP32-S3's per-frame sign flip
(§4 of the research note). The classifier consumes a per-node series of
log-amplitudes and a baseline, and decides whether a zone is occupied from
the change.

This module produces that series from the live nodes and runs the same
classifier over it.

THE MEASUREMENT
---------------
A frame's contribution is the received power over the usable subcarriers:
the mean of |CSI|² across the subcarriers that carry signal, in the log
domain, exactly the quantity the simulation's `measure_node_local_power`
produces. The null band and the centre carrier are skipped, because a null
subcarrier's zero would drag the mean down for a reason that has nothing to
do with the channel.

THE NOISE FLOOR
----------------
The question the detector turns on is how large a change in the node-local
power has to be to be distinguishable from the room's own drift. That is
measured here, over consecutive windows with nothing moving, from the
scatter of the window's block means — the per-frame spread is far too
optimistic, because a slow drift moves the frames together.

A person entering a zone perturbs the received power by several decibels.
The floor this measures is what that has to beat.

USING IT
---------
    # the noise floor, unattended
    python3 atsi_live.py --duration 45 --repeats 3

    # baseline, then a person enters, then measure again
    python3 atsi_live.py --interactive

Australian English spelling throughout.
"""

from __future__ import annotations

import argparse
import cmath
import math
import statistics
import sys
import time
from collections import defaultdict
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Sequence, Tuple

sys.path.insert(0, ".")

from atsi_hardware import classify_zone_occupancy
from hardware import CsiCollector, CsiFrame, frames_by_node

NULL_SUBCARRIERS = {0, 28, 29, 30, 31, 32, 33}

# The smallest power that counts as signal.
MIN_SIGNAL = 0.05

# The occupancy threshold in the log-amplitude domain, as the simulation uses.
OCCUPANCY_THRESHOLD_DB = 2.0


def frame_log_amplitude(frame: CsiFrame) -> Optional[float]:
    """The log of the received power over the frame's usable subcarriers."""
    total = 0.0
    counted = 0
    for k in range(len(frame.csi)):
        if k in NULL_SUBCARRIERS:
            continue
        value = frame.csi[k]
        if abs(value) < MIN_SIGNAL:
            continue
        total += value.real * value.real + value.imag * value.imag
        counted += 1
    if counted < 8:
        return None
    return math.log(total / counted + 1e-6)


@dataclass
class NodeSeries:
    """A node's log-amplitude series over one window."""

    node_id: int
    values: List[float] = field(default_factory=list)

    @property
    def mean(self) -> float:
        """The mean log-amplitude."""
        return statistics.mean(self.values) if self.values else 0.0

    def block_uncertainty(self, block: int = 30) -> float:
        """The uncertainty of the window's mean, from the block means."""
        if len(self.values) < 2 * block:
            return float("inf")
        means = [
            statistics.mean(self.values[start : start + block])
            for start in range(0, len(self.values) - block + 1, block)
        ]
        if len(means) < 2:
            return float("inf")
        return statistics.pstdev(means) / math.sqrt(len(means))


def collect_window(
    collector: CsiCollector, duration_s: float
) -> Dict[int, NodeSeries]:
    """Collect one window and reduce it to a per-node log-amplitude series."""
    frames = collector.collect_for(duration_s)
    grouped = frames_by_node(frames)
    series: Dict[int, NodeSeries] = {}
    for node_id, node_frames in grouped.items():
        node_series = NodeSeries(node_id=node_id)
        for frame in node_frames:
            value = frame_log_amplitude(frame)
            if value is not None:
                node_series.values.append(value)
        if node_series.values:
            series[node_id] = node_series
    return series


def summarise(series: Dict[int, NodeSeries]) -> str:
    """Render a window's per-node statistics."""
    lines = []
    for node_id in sorted(series):
        node_series = series[node_id]
        lines.append(
            f"  node {node_id}: n={len(node_series.values)} "
            f"mean={node_series.mean:+.4f} "
            f"sd={statistics.pstdev(node_series.values):.4f} "
            f"block uncertainty={node_series.block_uncertainty():.4f}"
        )
    return "\n".join(lines)


def run_repeats(duration_s: float, repeats: int) -> int:
    """Measure consecutive windows with nothing moving: the noise floor."""
    collector = CsiCollector(bind_port=9100)
    collector.start()
    try:
        time.sleep(2.0)
        windows = [
            collect_window(collector, duration_s) for _ in range(repeats)
        ]
    finally:
        collector.stop()

    print("=== ATSI noise floor: consecutive windows, nothing moving ===")
    for index, window in enumerate(windows):
        print(f"  window {index + 1}")
        print(summarise(window))

    print()
    print("  change between consecutive windows:")
    changes: Dict[int, List[float]] = defaultdict(list)
    for index in range(len(windows) - 1):
        for node_id in windows[index]:
            if node_id not in windows[index + 1]:
                continue
            before = windows[index][node_id].mean
            after = windows[index + 1][node_id].mean
            changes[node_id].append(after - before)
    for node_id in sorted(changes):
        values = changes[node_id]
        magnitude = max(abs(v) for v in values)
        print(f"    node {node_id}: largest |change| {magnitude:.4f} "
              f"({magnitude * 4.343:.2f} dB)")
    if changes:
        worst = max(max(abs(v) for v in values) for values in changes.values())
        print(f"  a person entering a zone has to move the node-local power by "
              f"more than {worst * 4.343:.2f} dB")
    return 0


def run_interactive(duration_s: float) -> int:
    """Measure a baseline, wait for the operator, then measure again."""
    collector = CsiCollector(bind_port=9100)
    collector.start()
    try:
        time.sleep(2.0)
        print(f"Baseline for {duration_s:.0f} s, with the zone empty.",
              flush=True)
        before = collect_window(collector, duration_s)
        print(summarise(before))
        input(
            "\nPress enter once the person has entered the zone, "
            "then press enter again when they have left: "
        )
        after = collect_window(collector, duration_s)
    finally:
        collector.stop()

    print("  after")
    print(summarise(after))
    print()
    node_ids = sorted(set(before) & set(after))
    if not node_ids:
        print("  no node was heard in both windows")
        return 1
    before_map = {str(n): before[n].values for n in node_ids}
    after_map = {str(n): after[n].values for n in node_ids}
    occupied, probability = classify_zone_occupancy(
        after_map, [str(n) for n in node_ids], before_map,
        threshold_db=OCCUPANCY_THRESHOLD_DB,
    )
    print(f"  classifier: occupied={occupied}, probability={probability:.3f}")
    for node_id in node_ids:
        delta = after[node_id].mean - before[node_id].mean
        print(f"    node {node_id}: {delta:+.4f} "
              f"({delta * 4.343:+.2f} dB)")
    return 0


def main() -> int:
    """Run the live ATSI measurement."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--duration", type=float, default=45.0,
        help="the length of each window, in seconds",
    )
    parser.add_argument(
        "--repeats", type=int, default=3,
        help="how many consecutive windows to measure",
    )
    parser.add_argument(
        "--interactive", action="store_true",
        help="measure a baseline, wait for the operator, then measure again",
    )
    options = parser.parse_args()

    if options.interactive:
        return run_interactive(options.duration)
    return run_repeats(options.duration, options.repeats)


if __name__ == "__main__":
    raise SystemExit(main())
