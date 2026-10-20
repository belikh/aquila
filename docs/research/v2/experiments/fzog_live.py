"""FZOG on the live nodes: the calibration dictionary and the voxel score.

INTEGRATION.md §5. FZOG — the amplitude-first occupancy imaging — is the
project's primary result, and it is the one phase of the campaign that still
runs in simulation. It stays simulated for a reason that is not a code
problem: the calibration needs a person standing in each voxel position, and
the campaign has never had one.

This module is the hardware path, ready to run. It has two modes.

*Calibration* walks through the voxel list. For each one the operator puts a
person there, the nodes record the link amplitudes, and the result is
appended to a calibration dictionary keyed by *(transmitter, receiver,
subcarrier)*, exactly the shape the simulation's `score_voxel` consumes.

*Scoring* records the links once and scores every voxel against the
dictionary, reporting the best.

THE LINKS
---------
A hardware link is a *(receiver node, transmitting station, subcarrier)*. The
transmitting station is named by its MAC address, so the calibration and the
scoring see the same links: the other nodes, which stream their own CSI to
the core and are by far the strongest thing on the channel, and the access
point.

A link's amplitude is the log of the received power at one subcarrier,
averaged over the window's frames. Only the 64-subcarrier layout is used:
the 128-subcarrier layout is the same training field plus a second one, and
the two are not the same physical subcarriers, so mixing them would put a
calibrated index against a different frequency.

NOISE
-----
The floor is measured, not assumed: the spread of the per-frame amplitude
within the window, and the change between two windows with nothing moving.
A voxel score that is within the noise of the runner-up is not a detection,
and the module says so.

Australian English spelling throughout.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import statistics
import sys
import time
from collections import defaultdict
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Sequence, Tuple

sys.path.insert(0, ".")

from fzog import score_voxel, Voxel
from hardware import CsiCollector, CsiFrame, frames_by_node

# The subcarriers the ESP32-S3 leaves null.
NULL_SUBCARRIERS = {0, 28, 29, 30, 31, 32, 33}

# The smallest amplitude that counts as signal.
MIN_SIGNAL = 0.05

# The layout whose subcarrier indices the calibration is keyed on.
CALIBRATION_LAYOUT = 64


@dataclass
class LinkAmplitudes:
    """The link amplitudes of one window, and their spread."""

    # (transmitter MAC, receiver node id, subcarrier) -> log-amplitude
    values: Dict[Tuple[str, int, int], float] = field(default_factory=dict)
    # The per-frame spread of the amplitude, per link.
    spread: Dict[Tuple[str, int, int], float] = field(default_factory=dict)
    n_frames: Dict[Tuple[str, int], int] = field(default_factory=dict)


def measure_window(
    collector: CsiCollector, duration_s: float
) -> LinkAmplitudes:
    """Measure every link's amplitude over one window."""
    frames = collector.collect_for(duration_s)
    grouped = frames_by_node(frames)
    result = LinkAmplitudes()

    # Accumulate the per-frame amplitudes per link.
    samples: Dict[Tuple[str, int, int], List[float]] = defaultdict(list)
    counts: Dict[Tuple[str, int], int] = defaultdict(int)
    for receiver, node_frames in grouped.items():
        for frame in node_frames:
            if frame.source_mac is None or frame.n_subcarriers != CALIBRATION_LAYOUT:
                continue
            counts[(frame.source_mac, receiver)] += 1
            for k in range(min(CALIBRATION_LAYOUT, len(frame.csi))):
                if k in NULL_SUBCARRIERS:
                    continue
                value = frame.csi[k]
                if abs(value) < MIN_SIGNAL:
                    continue
                samples[(frame.source_mac, receiver, k)].append(
                    math.log(value.real * value.real + value.imag * value.imag + 1e-6)
                )

    for key, values in samples.items():
        if len(values) < 5:
            continue
        result.values[key] = statistics.mean(values)
        result.spread[key] = statistics.pstdev(values)
    result.n_frames = dict(counts)
    return result


def noise_floor(
    first: LinkAmplitudes, second: LinkAmplitudes
) -> Tuple[float, float]:
    """The noise in the amplitudes, per subcarrier and per link.

    The first is the largest change in any single subcarrier's amplitude
    between two windows; the second is the largest change in a *link's* mean
    amplitude. The second is what sets the uncertainty of a voxel score,
    because the score is a mean over links and the per-subcarrier
    fluctuations are largely common to a link — they are one fading event,
    not independent noise.
    """
    worst_subcarrier = 0.0
    for key, value in first.values.items():
        other = second.values.get(key)
        if other is None:
            continue
        worst_subcarrier = max(worst_subcarrier, abs(other - value))

    def link_means(amplitudes: LinkAmplitudes) -> Dict[Tuple[str, int], float]:
        grouped: Dict[Tuple[str, int], List[float]] = defaultdict(list)
        for (transmitter, receiver, _subcarrier), value in amplitudes.values.items():
            grouped[(transmitter, receiver)].append(value)
        return {key: statistics.mean(values) for key, values in grouped.items()}

    first_means = link_means(first)
    second_means = link_means(second)
    worst_link = 0.0
    for key, value in first_means.items():
        other = second_means.get(key)
        if other is None:
            continue
        worst_link = max(worst_link, abs(other - value))
    return (worst_subcarrier, worst_link)


def to_feature_map(
    amplitudes: LinkAmplitudes,
) -> Dict[Tuple[str, str, int], float]:
    """The amplitudes in the shape `score_voxel` consumes.

    The simulation's links are named by node; the hardware's are named by the
    transmitting station's MAC address and the receiving node's id, so the
    receiver is carried as a string to keep the key shape.
    """
    return {
        (transmitter, str(receiver), subcarrier): value
        for (transmitter, receiver, subcarrier), value in
        amplitudes.values.items()
    }


def load_voxels(path: str) -> List[Voxel]:
    """Read the voxel list from a JSON file."""
    with open(path) as handle:
        raw = json.load(handle)
    voxels: List[Voxel] = []
    for entry in raw:
        if isinstance(entry, str):
            voxels.append(Voxel(x_m=0.0, y_m=0.0, z_m=0.0, label=entry))
            continue
        position = entry.get("position", [0.0, 0.0, 0.0])
        voxels.append(
            Voxel(
                x_m=float(position[0]),
                y_m=float(position[1]),
                z_m=float(position[2]) if len(position) > 2 else 0.0,
                label=entry["label"],
            )
        )
    return voxels


def save_calibration(path: str, dictionary: Dict[str, List[float]]) -> None:
    """Write the calibration dictionary out."""
    serialised = {
        "|".join([key[0], key[1], str(key[2])]): values
        for key, values in dictionary.items()
    }
    with open(path, "w") as handle:
        json.dump(serialised, handle, indent=1, sort_keys=True)


def load_calibration(path: str) -> Dict[Tuple[str, str, int], List[float]]:
    """Read the calibration dictionary back."""
    with open(path) as handle:
        raw = json.load(handle)
    dictionary: Dict[Tuple[str, str, int], List[float]] = {}
    for key, values in raw.items():
        transmitter, receiver, subcarrier = key.split("|")
        dictionary[(transmitter, receiver, int(subcarrier))] = values
    return dictionary


def run_calibration(
    voxels: Sequence[Voxel],
    calibration_path: str,
    duration_s: float,
) -> int:
    """Walk through the voxels, recording the calibration."""
    collector = CsiCollector(bind_port=9100)
    collector.start()
    dictionary: Dict[Tuple[str, str, int], List[float]] = {}
    try:
        time.sleep(2.0)
        for index, voxel in enumerate(voxels):
            print(
                f"\n[{index + 1}/{len(voxels)}] voxel {voxel.label}: "
                f"put the person there, then press enter "
                f"(press 's' to skip): ",
                end="",
                flush=True,
            )
            try:
                answer = input().strip().lower()
            except EOFError:
                # The operator's input ended: stop the walk rather than
                # failing, and keep whatever has been recorded.
                print("\n  input ended; stopping the walk")
                break
            if answer == "s":
                print("  skipped")
                continue
            amplitudes = measure_window(collector, duration_s)
            features = to_feature_map(amplitudes)
            for key, value in features.items():
                column = dictionary.setdefault(key, [])
                # Keep the column aligned with the voxel list.
                while len(column) < index:
                    column.append(float("nan"))
                column.append(value)
            print(
                f"  recorded {len(features)} link amplitudes across "
                f"{sum(amplitudes.n_frames.values())} frames on "
                f"{len(amplitudes.n_frames)} links"
            )
    finally:
        collector.stop()

    save_calibration(calibration_path, dictionary)
    print(f"\ncalibration written to {calibration_path}: "
          f"{len(dictionary)} links")
    return 0


def run_scoring(
    voxels: Sequence[Voxel],
    calibration_path: str,
    duration_s: float,
) -> int:
    """Record the links once and score every voxel."""
    if not os.path.exists(calibration_path):
        print(f"no calibration at {calibration_path}; run --calibrate first")
        return 1
    dictionary = load_calibration(calibration_path)

    collector = CsiCollector(bind_port=9100)
    collector.start()
    try:
        time.sleep(2.0)
        first = measure_window(collector, duration_s)
        second = measure_window(collector, duration_s)
    finally:
        collector.stop()

    subcarrier_floor, link_floor = noise_floor(first, second)
    features = to_feature_map(first)
    print(f"recorded {len(features)} link amplitudes")
    print(f"  the per-subcarrier floor is {subcarrier_floor:.4f} and the "
          f"per-link floor is {link_floor:.4f} in the log-amplitude domain")

    scores: List[Tuple[float, str]] = []
    uncalibrated: List[str] = []
    for index, voxel in enumerate(voxels):
        # Only the links that were actually calibrated at this voxel can
        # contribute: the calibration file is padded with NaN for the voxels
        # the operator skipped, and a NaN would poison the whole score.
        usable_features: Dict[Tuple[str, str, int], float] = {}
        usable_dictionary: Dict[Tuple[str, str, int], List[float]] = {}
        for key, value in features.items():
            column = dictionary.get(key)
            if column is None or index >= len(column):
                continue
            baseline = column[index]
            if baseline is None or math.isnan(baseline):
                continue
            usable_features[key] = value
            usable_dictionary[key] = column
        if not usable_features:
            uncalibrated.append(voxel.label)
            continue
        score = score_voxel(
            [str(node) for node in range(1, 32)],
            list(voxels),
            usable_features,
            usable_dictionary,
            index,
        )
        scores.append((score, voxel.label))
    scores.sort(reverse=True)
    if uncalibrated:
        print(f"  {len(uncalibrated)} voxels were not calibrated and are "
              f"not scored: {', '.join(uncalibrated)}")
    for score, label in scores:
        print(f"  voxel {label:<12} score {score:+.4f}")
    if len(scores) >= 2:
        margin = scores[0][0] - scores[1][0]
        # The score is a mean over the links, so its uncertainty is the
        # *per-link* floor divided by the square root of the number of links
        # that contributed. The per-subcarrier floor is larger and largely
        # common to a link — one fading event, not independent noise — so it
        # must not be used here.
        links_used = max(1, len(usable_features))
        score_floor = link_floor / math.sqrt(links_used)
        print()
        print(f"  best: {scores[0][1]}, ahead of {scores[1][1]} by "
              f"{margin:.4f}")
        print(f"  the score's uncertainty is {score_floor:.4f} "
              f"(the per-link floor of {link_floor:.4f} over {links_used} links)")
        if margin < score_floor:
            print("  that margin is inside the noise floor: this is not a "
                  "detection")
        else:
            print(f"  the margin is {margin / score_floor:.1f} times the "
                  f"uncertainty: a detection")
    elif scores:
        print()
        print(f"  only one voxel was calibrated: {scores[0][1]}")
    return 0


def main() -> int:
    """Run the live FZOG calibration or scoring."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--voxels", required=True,
        help="a JSON file listing the voxel labels, with optional positions",
    )
    parser.add_argument(
        "--calibration", default="fzog_calibration.json",
        help="where the calibration dictionary is kept",
    )
    parser.add_argument(
        "--duration", type=float, default=20.0,
        help="the length of each window, in seconds",
    )
    parser.add_argument(
        "--calibrate", action="store_true",
        help="walk through the voxels and record the calibration",
    )
    options = parser.parse_args()

    voxels = load_voxels(options.voxels)
    if options.calibrate:
        return run_calibration(voxels, options.calibration, options.duration)
    return run_scoring(voxels, options.calibration, options.duration)


if __name__ == "__main__":
    raise SystemExit(main())
