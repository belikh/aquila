"""Does the per-link group delay respond to a person moving between the nodes?

INTEGRATION.md §5, and the experiment the project now turns on. The
cross-node absolute phase is unusable on this hardware (§4 of the research
note: the ESP32-S3 negates the whole channel vector from frame to frame, and
the sign is noise in the estimator), but the phase *slope* — the per-link
group delay — is exactly invariant to that negation, and it is measurable.

This script measures the group delay of every link the live nodes can see,
twice: once as a baseline and once after something has moved. It reports the
change in the path length each link implies, against the uncertainty of the
measurement, and it separates the links that should have changed from the
links that serve as controls.

THE GEOMETRY
------------
A receiver's group delay for a transmitter is the weighted mean delay of the
paths between them. A person walking into that path adds a component whose
delay is longer, and the measured mean moves. So the test is: put a person on
one link, and watch that link's delay move while the others stay put.

The links are identified by the pair *(receiver, transmitter)*, and the
transmitter is named by its MAC address, so a link is unambiguous. The
dominant transmitter on each receiver is the other node: the nodes stream
their own CSI to the core, so each node hears the others far more strongly
than it hears the access point. A person walking between node 1 and node 3
therefore moves node 1's link to node 3, and node 2's link to node 3 is the
control: the person is not on that path.

NOISE
-----
The uncertainty is not assumed. Each link's per-frame slope spread gives the
uncertainty of a single frame's path length, and the window's uncertainty is
that divided by the square root of the number of frames. The significance of
a change is the change divided by the combined uncertainty of the two
windows, which is the honest test of whether anything moved.

USING IT
---------
    # a scripted run: baseline, then the operator presses enter, then after
    python3 scatterer_response.py --interactive

    # an unattended noise-floor run: two consecutive windows, nothing moving
    python3 scatterer_response.py --duration 120 --repeats 3

Australian English spelling throughout.
"""

from __future__ import annotations

import argparse
import cmath
import math
import statistics
import sys
import time
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Sequence, Tuple

sys.path.insert(0, ".")

from group_delay_hardware import (
    LinkDelay,
    measure_link,
)
from hardware import CsiCollector, CsiFrame, frames_by_node


@dataclass
class LinkChange:
    """The change in one link's path length between two windows."""

    receiver: int
    transmitter: str
    layout: int
    before_m: float
    after_m: float
    before_uncertainty_m: float
    after_uncertainty_m: float
    n_before: int
    n_after: int

    @property
    def change_m(self) -> float:
        """The change in the implied path length, in metres."""
        return self.after_m - self.before_m

    @property
    def uncertainty_m(self) -> float:
        """The combined uncertainty of the change, in metres."""
        return math.sqrt(
            self.before_uncertainty_m**2 + self.after_uncertainty_m**2
        )

    @property
    def significance(self) -> float:
        """The change in units of its own uncertainty."""
        if self.uncertainty_m == 0.0:
            return 0.0
        return self.change_m / self.uncertainty_m

    @property
    def moved(self) -> bool:
        """Whether the change is large enough to be worth looking at."""
        return abs(self.significance) >= 3.0


@dataclass
class Window:
    """One measurement window."""

    label: str
    links: Dict[Tuple[int, str, int], LinkDelay] = field(default_factory=dict)
    n_frames: Dict[int, int] = field(default_factory=dict)

    def uncertainty(self, key: Tuple[int, str, int]) -> float:
        """The uncertainty of a link's path length over this window.

        The window's block-mean uncertainty, which prices in the slow drift
        that the per-frame spread misses.
        """
        link = self.links.get(key)
        if link is None or link.n_frames < 2:
            return float("inf")
        return link.path_uncertainty_m


def measure_window(
    collector: CsiCollector,
    duration_s: float,
    label: str,
) -> Window:
    """Measure every link over one window."""
    frames = collector.collect_for(duration_s)
    grouped = frames_by_node(frames)
    window = Window(label=label)
    window.n_frames = {node: len(items) for node, items in grouped.items()}

    for receiver, node_frames in grouped.items():
        per_mac: Dict[str, List[CsiFrame]] = {}
        for frame in node_frames:
            if frame.source_mac is None:
                continue
            per_mac.setdefault(frame.source_mac, []).append(frame)
        for transmitter, subset in per_mac.items():
            for layout in (64, 128):
                frames_of_layout = [
                    frame for frame in subset if frame.n_subcarriers == layout
                ]
                link = measure_link(
                    receiver, frames_of_layout, transmitter, layout
                )
                if link is not None and link.n_frames >= 20:
                    window.links[(receiver, transmitter, layout)] = link
    return window


def compare(
    before: Window, after: Window
) -> List[LinkChange]:
    """Compare the links the two windows share."""
    changes: List[LinkChange] = []
    for key in sorted(set(before.links) & set(after.links)):
        receiver, transmitter, layout = key
        first = before.links[key]
        second = after.links[key]
        changes.append(
            LinkChange(
                receiver=receiver,
                transmitter=transmitter,
                layout=layout,
                before_m=first.path_difference_m,
                after_m=second.path_difference_m,
                before_uncertainty_m=before.uncertainty(key),
                after_uncertainty_m=after.uncertainty(key),
                n_before=first.n_frames,
                n_after=second.n_frames,
            )
        )
    return changes


def format_change(change: LinkChange) -> str:
    """Render one link's change."""
    moved = "MOVED" if change.moved else "     "
    return (
        f"  {moved} node {change.receiver} <- {change.transmitter} "
        f"({change.layout} sub): "
        f"{change.before_m:+.3f} m -> {change.after_m:+.3f} m, "
        f"change {change.change_m * 100:+.1f} cm "
        f"+/-{change.uncertainty_m * 100:.1f} cm "
        f"({change.significance:+.1f} sigma, "
        f"n {change.n_before}->{change.n_after})"
    )


def run_interactive(duration_s: float, repeats: int) -> int:
    """Measure a baseline, wait for the operator, then measure again."""
    collector = CsiCollector(bind_port=9100)
    collector.start()
    try:
        time.sleep(2.0)
        print(
            f"Baseline for {duration_s:.0f} s. Stand well clear of the links.",
            flush=True,
        )
        before = measure_window(collector, duration_s, "before")
        print(
            f"\nBaseline done ({len(before.links)} links).",
            flush=True,
        )
        input(
            "Press enter once the person is walking the link, "
            "then press enter again when they have finished: "
        )
        after = measure_window(collector, duration_s, "after")
        changes = compare(before, after)
    finally:
        collector.stop()

    print()
    print("=== link changes, largest first ===")
    for change in sorted(changes, key=lambda c: -abs(c.significance)):
        print(format_change(change))
    moved = [c for c in changes if c.moved]
    print()
    if moved:
        print(f"{len(moved)} of {len(changes)} links moved by three sigma or more.")
        for change in moved:
            print(f"  node {change.receiver} <- {change.transmitter} "
                  f"({change.layout} sub): {change.change_m * 100:+.1f} cm")
    else:
        print("No link moved by three sigma or more.")
    return 0


def run_repeats(duration_s: float, repeats: int) -> int:
    """Measure consecutive windows with nothing moving: the noise floor."""
    collector = CsiCollector(bind_port=9100)
    collector.start()
    try:
        time.sleep(2.0)
        windows = [
            measure_window(collector, duration_s, f"window {index + 1}")
            for index in range(repeats)
        ]
    finally:
        collector.stop()

    print("=== noise floor: consecutive windows, nothing moving ===")
    # Only the strong links count: a link heard a handful of times has an
    # uncertainty of metres and its change is meaningless.
    strong: List[LinkChange] = []
    for index in range(len(windows) - 1):
        changes = compare(windows[index], windows[index + 1])
        usable = [c for c in changes if c.n_before >= 60 and c.n_after >= 60]
        print(f"  {windows[index].label} -> {windows[index + 1].label} "
              f"({len(usable)} strong links)")
        for change in sorted(usable, key=lambda c: -abs(c.significance))[:6]:
            print(format_change(change))
        strong.extend(usable)
        print()

    if strong:
        # A link with a large uncertainty is not usable: its change is
        # swamped by the drift, whatever actually happened.
        usable = [c for c in strong if c.uncertainty_m < 0.05]
        magnitudes = sorted(abs(c.change_m) for c in usable or strong)
        median = magnitudes[len(magnitudes) // 2]
        p90 = magnitudes[min(len(magnitudes) - 1, int(0.9 * len(magnitudes)))]
        print(f"strong links across consecutive windows: {len(strong)} "
              f"({len(usable)} of them precise enough to use)")
        print(f"  median |change| {median * 100:.1f} cm, "
              f"90th percentile {p90 * 100:.1f} cm")
        significances = sorted(abs(c.significance) for c in usable or strong)
        print(f"  median |significance| "
              f"{significances[len(significances) // 2]:.1f} sigma")
        print(f"  a moving person has to beat about {p90 * 100:.0f} cm")
    else:
        print("no strong links: the nodes need more frames per window")
    return 0


def main() -> int:
    """Run the scatterer-response experiment."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--duration", type=float, default=60.0,
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
        return run_interactive(options.duration, options.repeats)
    return run_repeats(options.duration, options.repeats)


if __name__ == "__main__":
    raise SystemExit(main())
