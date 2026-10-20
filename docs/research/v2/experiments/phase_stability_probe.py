"""Per-subcarrier phase behaviour of the ESP32-S3 CSI, measured on hardware.

INTEGRATION.md §5. This probes a single question that decides whether the
cross-node phase measurement is possible at all: is the phase the ESP32-S3
reports a physical quantity, or is it ambiguous?

The probe is deliberately node-local, because a node-local answer is
decisive on its own. The ambient traffic on the channel is dominated by
one access point's beacons, which arrive at a fixed interval from a fixed
place. The path delay from that access point to the node is therefore
constant, so the phase the node reports for successive beacons at a given
subcarrier should be constant too, up to a slow carrier-frequency drift.

Two behaviours would break that expectation, and each is diagnosable:

* a phase that wanders at random — the reported phase carries no path
  information, and no amount of packet pairing will make the cross-node
  residual meaningful;
* a phase that jumps by exactly pi from one beacon to the next — the
  reported phase is ambiguous modulo pi, and the sign has to be resolved
  before the phase can be used.

Australian English spelling throughout.
"""

from __future__ import annotations

import cmath
import math
import statistics
import time
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Sequence, Tuple

from hardware import CsiCollector, CsiFrame, frames_by_node
from m2_two_node_hardware import build_train, dominant_period_ms

# A successive-difference is "settled" when it is within this many radians
# of zero, and "flipped" when it is within this many radians of plus or
# minus pi.
SETTLED_TOL_RAD = 0.35
FLIPPED_TOL_RAD = 0.35

# The circular concentration above which a subcarrier's phase is treated
# as stable across the beacon train.
STABLE_CONCENTRATION = 0.90


@dataclass
class SubcarrierBehaviour:
    """How one subcarrier's phase behaves across a node's beacon train."""

    subcarrier: int
    n_samples: int
    concentration: float  # circular concentration of the phase about its mean
    settled_fraction: float  # share of successive differences near zero
    flipped_fraction: float  # share of successive differences near +/- pi
    mean_phase_rad: float
    phase_spread_rad: float

    @property
    def verdict(self) -> str:
        """A short word for the behaviour."""
        if self.concentration >= STABLE_CONCENTRATION:
            return "stable"
        if self.flipped_fraction > 0.5:
            return "pi-flipping"
        return "wandering"


@dataclass
class NodeProbe:
    """The probe result for one node."""

    node_id: int
    n_frames: int
    train_period_ms: float
    train_size: int
    behaviours: List[SubcarrierBehaviour] = field(default_factory=list)

    @property
    def stable(self) -> List[int]:
        """The subcarriers whose phase is stable across the train."""
        return [b.subcarrier for b in self.behaviours if b.verdict == "stable"]

    @property
    def flipping(self) -> List[int]:
        """The subcarriers whose phase jumps by pi from beacon to beacon."""
        return [b.subcarrier for b in self.behaviours if b.verdict == "pi-flipping"]


def probe_node(node_id: int, frames: Sequence[CsiFrame]) -> NodeProbe:
    """Measure the phase behaviour of every subcarrier for one node."""
    arrivals = [f.host_arrival_us / 1000.0 for f in frames]
    period, _ = dominant_period_ms(arrivals)
    train = build_train(list(frames), period)
    probe = NodeProbe(
        node_id=node_id,
        n_frames=len(frames),
        train_period_ms=period,
        train_size=len(train),
    )
    if len(train) < 8:
        return probe

    n_subcarriers = len(train[0].frame.csi)
    for subcarrier in range(n_subcarriers):
        phases: List[float] = []
        for item in train:
            if subcarrier >= len(item.frame.csi):
                continue
            value = item.frame.csi[subcarrier]
            if abs(value) < 0.05:
                continue
            phases.append(cmath.phase(value))
        if len(phases) < 8:
            continue

        # The concentration of the phase about its circular mean.
        vector = sum(cmath.exp(1j * phase) for phase in phases)
        concentration = abs(vector) / len(phases)
        mean_phase = cmath.phase(vector)

        # The successive differences, already wrapped to (-pi, pi].
        differences = [
            cmath.phase(phases[index + 1] - phases[index])
            for index in range(len(phases) - 1)
        ]
        settled = sum(
            1 for value in differences if abs(value) < SETTLED_TOL_RAD
        )
        flipped = sum(
            1
            for value in differences
            if abs(abs(value) - math.pi) < FLIPPED_TOL_RAD
        )
        divisor = max(1, len(differences))
        spread = statistics.median(abs(value) for value in differences)

        probe.behaviours.append(
            SubcarrierBehaviour(
                subcarrier=subcarrier,
                n_samples=len(phases),
                concentration=concentration,
                settled_fraction=settled / divisor,
                flipped_fraction=flipped / divisor,
                mean_phase_rad=mean_phase,
                phase_spread_rad=spread,
            )
        )
    return probe


def format_probe(probe: NodeProbe) -> str:
    """Render a probe for the operator."""
    lines = [
        f"node {probe.node_id}: {probe.n_frames} frames, beacon period "
        f"{probe.train_period_ms:.0f} ms, train of {probe.train_size} packets",
    ]
    if not probe.behaviours:
        lines.append("  no subcarrier carried enough signal to probe")
        return "\n".join(lines)

    lines.append(
        f"  {'sc':>4} {'n':>5} {'concen':>7} {'settled':>8} {'flipped':>8} "
        f"{'spread':>7}  verdict"
    )
    for behaviour in probe.behaviours:
        lines.append(
            f"  {behaviour.subcarrier:>4} {behaviour.n_samples:>5} "
            f"{behaviour.concentration:>7.3f} {behaviour.settled_fraction:>8.3f} "
            f"{behaviour.flipped_fraction:>8.3f} {behaviour.phase_spread_rad:>7.3f}  "
            f"{behaviour.verdict}"
        )
    lines.append(
        f"  stable subcarriers: {probe.stable if probe.stable else 'none'}"
    )
    lines.append(
        f"  pi-flipping subcarriers: {probe.flipping if probe.flipping else 'none'}"
    )
    return "\n".join(lines)


def main() -> int:
    """Collect from the live nodes and report the phase behaviour."""
    collector = CsiCollector(bind_port=9100)
    collector.start()
    try:
        time.sleep(2.0)
        frames = collector.collect_for(90.0)
    finally:
        collector.stop()

    grouped = frames_by_node(frames)
    for node_id in sorted(grouped):
        probe = probe_node(node_id, grouped[node_id])
        print(format_probe(probe))
        print()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
