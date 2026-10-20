"""How much should a person move the measured group delay?

The moving-scatterer experiment is built and its noise floor is measured, but
it has not been run with a person in the room yet. This script answers the
question that decides whether the experiment is worth running: using the link
model, how much does a person's presence move the group delay, and how does
that compare with the floor the hardware actually shows?

The model is the project's own (`csi_sim`), with the noise calibrated to what
the hardware shows: the per-frame spread of the measured phase slope is about
0.0006 rad per subcarrier, and the group delay of the node-to-node links is
about 0.93 m.

The comparison is the point. A predicted change well below the measured floor
would mean the experiment cannot work, however it is run; a predicted change
many times the floor means it should be obvious.

Australian English spelling throughout.
"""

from __future__ import annotations

import cmath
import math
import random
import statistics
import sys
from typing import Dict, List, Optional, Sequence, Tuple

sys.path.insert(0, ".")

from csi_sim import (
    LinkSpec,
    Node,
    Scatterer,
    measure_csi,
    path_length_m,
    subcarrier_frequencies,
)

CENTRE_HZ = 2_437_000_000.0
BANDWIDTH_HZ = 20_000_000.0
N_SUBCARRIERS = 64
SUBCARRIER_SPACING_HZ = 312_500.0
SPEED_OF_LIGHT_MPS = 299_792_458.0

# What the hardware shows, measured on the live nodes.
MEASURED_NOISE_FLOOR_CM = 3.0
MEASURED_PATH_M = 0.93
MEASURED_SLOPE_SPREAD = 0.0006


def unwrap(values: Sequence[float]) -> List[float]:
    """Unwrap a phase series that is smooth in the subcarrier index."""
    if not values:
        return []
    two_pi = 2.0 * math.pi
    out = [values[0]]
    for index in range(1, len(values)):
        rotations = round((out[-1] - values[index]) / two_pi)
        out.append(values[index] + rotations * two_pi)
    return out


def fit_slope(
    subcarriers: Sequence[int], phases: Sequence[float]
) -> Tuple[float, float]:
    """Fit phase = intercept + slope * subcarrier."""
    n = len(subcarriers)
    if n < 8:
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


def group_delay_m(
    nodes: Dict[str, Node],
    spec: LinkSpec,
    scatterers: List[Scatterer],
    noise_std: float = 0.0,
) -> float:
    """The group delay the model gives for one link, in metres."""
    frequencies = subcarrier_frequencies(
        spec.centre_freq_hz, spec.bandwidth_hz, spec.n_subcarriers
    )
    phases: List[float] = []
    for frequency in frequencies:
        value = measure_csi(
            nodes, spec, scatterers, frequency, 0.0,
            noise_std=noise_std, include_lo=False,
        )
        phases.append(cmath.phase(value))
    unwrapped = unwrap(phases)
    _intercept, slope = fit_slope(list(range(len(unwrapped))), unwrapped)
    return slope / (2.0 * math.pi * SUBCARRIER_SPACING_HZ) * SPEED_OF_LIGHT_MPS


def build_link(separation_m: float = MEASURED_PATH_M) -> Tuple[Dict[str, Node], LinkSpec]:
    """A two-node link with a line of sight and one static reflection.

    The separation is the one the hardware measures: the group delay of the
    node-to-node links is about 0.93 m, so that is the geometry the
    prediction has to be made for.
    """
    nodes: Dict[str, Node] = {
        "TX": Node(node_id="TX", position=(0.0, 0.0, 1.5), lo_freq_hz=2_437_000_000.0,
                   clock_drift_ppm=0.0, phase_noise_std_rad=0.0),
        "RX": Node(node_id="RX", position=(separation_m, 0.0, 1.5), lo_freq_hz=2_437_000_000.0,
                   clock_drift_ppm=0.0, phase_noise_std_rad=0.0),
    }
    direct = path_length_m(nodes["TX"].position, nodes["RX"].position)
    spec = LinkSpec(
        tx_node_id="TX", rx_node_id="RX",
        centre_freq_hz=CENTRE_HZ, bandwidth_hz=BANDWIDTH_HZ,
        n_subcarriers=N_SUBCARRIERS,
        static_paths=[(1.0, direct), (0.25, direct * 1.3)],
    )
    return nodes, spec


def main() -> int:
    """Predict the group-delay change a person causes."""
    random.seed(20261021)
    nodes, spec = build_link()
    baseline = group_delay_m(nodes, spec, [])

    print("=== the model's link ===")
    print(f"  separation {MEASURED_PATH_M:.2f} m — the geometry the hardware "
          f"measures — with a line of sight and one reflection")
    print(f"  modelled group delay without a person: {abs(baseline):.3f} m "
          f"(the hardware measures {MEASURED_PATH_M:.3f} m; the sign "
          f"conventions differ, the magnitudes are what is compared)")
    print(f"  the measured noise floor: {MEASURED_NOISE_FLOOR_CM:.1f} cm")
    print()

    print("=== the group-delay change a person causes ===")
    print("  the person stands on the link, at a distance from the direct path,")
    print("  with a radar-cross-section proxy as their strength")
    print()
    print("  offset (m)  strength   change (cm)   times the noise floor")
    for offset_m in (0.5, 1.0, 2.0, 3.0):
        for strength in (0.1, 0.25, 0.5, 1.0):
            # The person stands beside the direct path, so the two-bounce path
            # is longer than the line of sight.
            scatterer = Scatterer(
                scatterer_id="person",
                position=(1.5, offset_m, 1.2),
                axis=(0.0, 1.0, 0.0),
                amplitude_m=0.0,
                freq_hz=0.0,
                phase_rad=0.0,
                strength=strength,
            )
            with_person = group_delay_m(nodes, spec, [scatterer])
            change_cm = (with_person - baseline) * 100.0
            ratio = abs(change_cm) / MEASURED_NOISE_FLOOR_CM
            marker = "  <-" if ratio >= 3.0 else ""
            print(f"  {offset_m:>9.1f}  {strength:>8.2f}   {change_cm:>+10.1f}   "
                  f"{ratio:>18.1f}{marker}")

    print()
    print("=== with the hardware's own noise on the phase ===")
    print("  the same sweep, with the per-frame phase noise the hardware shows")
    print("  (the slope's spread of 0.0006 rad per subcarrier)")
    noise = MEASURED_SLOPE_SPREAD * 1.4826 / math.sqrt(2 * math.pi)
    for offset_m in (1.0, 2.0):
        for strength in (0.25, 0.5, 1.0):
            samples: List[float] = []
            for _ in range(24):
                scatterer = Scatterer(
                    scatterer_id="person",
                    position=(1.5, offset_m, 1.2),
                    axis=(0.0, 1.0, 0.0),
                    amplitude_m=0.0, freq_hz=0.0, phase_rad=0.0,
                    strength=strength,
                )
                samples.append(group_delay_m(nodes, spec, [scatterer], noise))
            samples.sort()
            median = samples[len(samples) // 2]
            spread = (samples[-1] - samples[0]) / 2 * 100.0
            change_cm = (median - baseline) * 100.0
            ratio = abs(change_cm) / MEASURED_NOISE_FLOOR_CM
            print(f"  offset {offset_m:.1f} m, strength {strength:.2f}: "
                  f"change {change_cm:+.1f} cm ({ratio:.0f} times the floor), "
                  f"frame-to-frame spread {spread:.1f} cm")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
