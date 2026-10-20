"""Is the simulated M2 red band physics, or a metric artefact?

The simulated M2 characterisation reports a red band, which hardware-gates
cross-node coherent BCPT. This script asks whether that verdict survives a
corrected residual.

The question the residual must answer is: *how much does the link phase move
once the carrier-frequency offset and its drift are removed?* The static path
phase is a property of the geometry, not an error: its value depends on the
path length modulo a wavelength, so it is arbitrary and differs from one link
to the next. Subtracting only the drift leaves that arbitrary constant in the
residual, so the metric measures where the path length happens to fall rather
than how well the phase is held.

Two residuals are computed from the same simulated phases:

* the **as-implemented** residual, which removes the drift only;
* the **centred** residual, which removes the drift *and* the constant.

If the verdict flips, the red band was an artefact of the metric rather than a
finding about the channel.

Australian English spelling throughout.
"""

from __future__ import annotations

import math
import random
import statistics
from typing import List, Tuple

from m2_characterization import (
    AMBER_MAX_RAD,
    GREEN_MAX_RAD,
    build_star_mesh,
    classify_band,
    default_static_paths_star,
    fit_cfo_trend,
    measure_link_phase,
)
from csi_sim import Scatterer, wrap_to_pm_pi


def residuals_from_phases(
    times_s: List[float], phases_rad: List[float], centre: bool
) -> Tuple[List[float], float]:
    """The residual series for one link, with or without the constant removed.

    `centre` removes the intercept of the linear fit as well as the drift;
    without it the residual keeps the arbitrary static path phase.
    """
    intercept, cfo_rad_per_s = fit_cfo_trend(times_s, phases_rad)
    residuals: List[float] = []
    for moment, phase in zip(times_s, phases_rad):
        predicted = cfo_rad_per_s * moment
        if centre:
            predicted += intercept
        residuals.append(wrap_to_pm_pi(phase - predicted))
    return residuals, cfo_rad_per_s


def p95(values: List[float]) -> float:
    """The 95th percentile of a list of magnitudes."""
    ordered = sorted(abs(value) for value in values)
    index = int(math.ceil(0.95 * len(ordered))) - 1
    return ordered[max(0, min(index, len(ordered) - 1))]


def main() -> int:
    """Run the simulated M2 both ways and report the two verdicts."""
    random.seed(20261017)
    nodes = build_star_mesh("TX0", [f"RX{i}" for i in range(6)], separation_m=3.0)
    scatterers: List[Scatterer] = []
    rx_ids = [f"RX{i}" for i in range(6)]
    rx_positions = {rx_id: nodes[rx_id].position for rx_id in rx_ids}
    static_paths = default_static_paths_star("TX0", nodes["TX0"].position, rx_positions)
    f_k = 2_412_000_000.0 + 10_000_000.0
    times_s = [i * 0.01 for i in range(100)]

    print(f"{'link':>8} {'as-impl p95':>14} {'band':>6} "
          f"{'centred p95':>13} {'band':>6} {'intercept':>10}")
    print("-" * 64)

    bands_plain: List[str] = []
    bands_centred: List[str] = []
    for rx_id in rx_ids:
        phases = [
            measure_link_phase(
                nodes, "TX0", rx_id, f_k, moment, static_paths, scatterers, True
            )
            for moment in times_s
        ]
        plain, cfo = residuals_from_phases(times_s, phases, centre=False)
        centred, _ = residuals_from_phases(times_s, phases, centre=True)
        band_plain = classify_band(p95(plain))
        band_centred = classify_band(p95(centred))
        bands_plain.append(band_plain)
        bands_centred.append(band_centred)
        print(f"{rx_id:>8} {p95(plain):>14.4f} {band_plain:>6} "
              f"{p95(centred):>13.4f} {band_centred:>6} "
              f"{fit_cfo_trend(times_s, phases)[0]:>10.4f}")

    print()
    print(f"as-implemented worst band: "
          f"{'red' if 'red' in bands_plain else 'amber' if 'amber' in bands_plain else 'green'}")
    print(f"centred worst band:         "
          f"{'red' if 'red' in bands_centred else 'amber' if 'amber' in bands_centred else 'green'}")
    print()
    print(f"bands: green < {GREEN_MAX_RAD} rad, amber < {AMBER_MAX_RAD} rad")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
