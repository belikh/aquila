"""M2 characterisation — cross-node phase residual gate.

INTEGRATION.md §5. Measures the per-packet cross-node phase residual after CFO
removal on wireless-only ESP32-S3. This is the decisive quantity: if M2 >= 1 rad
(red band), cross-node coherent BCPT is hardware-gated for this setup.

Pure Python (stdlib only) for the analysis functions; the hardware-facing
collection uses hardware.CsiCollector over UDP port 9100 when nodes are present.
Runs in simulation today (csi_sim.measure_csi), on real hardware later.

Australian English spelling throughout.
"""

from __future__ import annotations

import cmath
import math
import random
import statistics
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Tuple

from csi_sim import (
    LinkSpec,
    Node,
    Scatterer,
    SPEED_OF_LIGHT,
    link_phase_rad,
    measure_csi,
    path_length_m,
    subcarrier_frequencies,
    wrap_to_pm_pi,
)

# ---------------------------------------------------------------------------
# M2 operational bands (protocol gates for THIS campaign, not device specs)
# ---------------------------------------------------------------------------

GREEN_MAX_RAD = 0.25   # 95th percentile below this -> green
AMBER_MAX_RAD = 1.00   # below this -> amber; at or above -> red


@dataclass
class M2Result:
    pair: Tuple[str, str]
    packet_interval_s: float
    n_packets: int
    residuals_rad: List[float]          # per-packet wrapped residual
    median_rad: float
    p95_rad: float                      # 95th percentile of |residual|
    band: str                           # "green" / "amber" / "red"
    cfo_estimate_rad_per_s: float       # fitted CFO per link pair
    notes: List[str] = field(default_factory=list)


@dataclass
class M2ExperimentReport:
    transmitter: str
    receivers: List[str]
    n_links: int
    results: List[M2Result]
    worst_band: str                     # "green" / "amber" / "red"
    bcpt_cleared: bool                  # True if worst_band != "red"
    gate_message: str


def build_star_mesh(tx_id: str, rx_ids: List[str],
                    separation_m: float = 3.0) -> Dict[str, Node]:
    """Transmitter at origin; receivers on a circle of radius separation_m."""
    nodes: Dict[str, Node] = {}
    nodes[tx_id] = Node(
        node_id=tx_id,
        position=(0.0, 0.0, 1.5),
        lo_freq_hz=2_412_000_000.0,
        clock_drift_ppm=20.0,
        phase_noise_std_rad=0.05,
    )
    for i, rx_id in enumerate(rx_ids):
        ang = 2.0 * math.pi * i / len(rx_ids)
        nodes[rx_id] = Node(
            node_id=rx_id,
            position=(separation_m * math.cos(ang),
                      separation_m * math.sin(ang),
                      1.5),
            lo_freq_hz=2_412_000_000.0,
            clock_drift_ppm=20.0 * (1.0 + 0.1 * (i % 3 - 1)),
            phase_noise_std_rad=0.05,
        )
    return nodes


def default_static_paths_star(tx_id: str,
                              tx_pos: Tuple[float, float, float],
                              rx_positions: Dict[str, Tuple[float, float, float]]
                              ) -> Dict[Tuple[str, str], List[Tuple[float, float]]]:
    """LoS + one static reflection per link."""
    paths: Dict[Tuple[str, str], List[Tuple[float, float]]] = {}
    for rx_id, rx_pos in rx_positions.items():
        d = path_length_m(tx_pos, rx_pos)
        paths[(tx_id, rx_id)] = [(1.0, d), (0.25, d * 1.3)]
    return paths


def measure_link_phase(nodes: Dict[str, Node],
                       tx_id: str,
                       rx_id: str,
                       f_k: float,
                       t_s: float,
                       static_paths: Dict[Tuple[str, str], List[Tuple[float, float]]],
                       scatterers: List[Scatterer],
                       include_lo: bool = True) -> float:
    """Measured phase of one directed link at subcarrier f_k, time t_s."""
    spec = LinkSpec(
        tx_node_id=tx_id, rx_node_id=rx_id,
        centre_freq_hz=2_412_000_000.0, bandwidth_hz=20_000_000.0,
        n_subcarriers=64, static_paths=static_paths[(tx_id, rx_id)],
    )
    h = measure_csi(nodes, spec, scatterers, f_k, t_s,
                    noise_std=0.05, cfo_hz=100.0,
                    sampling_offset_s=1e-6, include_lo=include_lo)
    return link_phase_rad(h)


def fit_cfo_trend(times_s: List[float],
                  phases_rad: List[float]) -> Tuple[float, float]:
    """Linear fit of wrapped phase vs time: phase = intercept + cfo_rad_per_s * t.
    Returns (intercept_rad, cfo_rad_per_s). Unwraps phases first."""
    n = len(times_s)
    if n < 2:
        return phases_rad[0] if phases_rad else 0.0, 0.0
    two_pi = 2.0 * math.pi
    unwrapped = [phases_rad[0]]
    for k in range(1, n):
        y = phases_rad[k]
        prev = unwrapped[-1]
        n_rot = round((prev - y) / two_pi)
        unwrapped.append(y + n_rot * two_pi)
    t_mean = sum(times_s) / n
    y_mean = sum(unwrapped) / n
    num = sum((t - t_mean) * (y - y_mean)
              for t, y in zip(times_s, unwrapped))
    den = sum((t - t_mean) ** 2 for t in times_s)
    if den == 0.0:
        return unwrapped[0], 0.0
    slope = num / den  # rad/s
    intercept = y_mean - slope * t_mean
    return intercept, slope


def compute_m2_residuals(nodes: Dict[str, Node],
                         tx_id: str,
                         rx_id: str,
                         f_k: float,
                         times_s: List[float],
                         static_paths: Dict[Tuple[str, str], List[Tuple[float, float]]],
                         scatterers: List[Scatterer],
                         include_lo: bool = True) -> List[float]:
    """Measure link phase at each time, fit CFO trend, return wrapped residuals.

    The residual removes the drift *and* the constant offset. The constant is
    the static path phase: its value depends on the path length modulo a
    wavelength, so it is arbitrary and differs from link to link. Leaving it in
    would make the metric measure where the path length happens to fall rather
    than how well the phase is held, and it turns a green link red.
    `m2_metric_audit.py` demonstrates the difference.
    """
    phases = [measure_link_phase(nodes, tx_id, rx_id, f_k, t,
                                 static_paths, scatterers, include_lo)
              for t in times_s]
    intercept, cfo_rad_per_s = fit_cfo_trend(times_s, phases)
    residuals = []
    for t, phi in zip(times_s, phases):
        predicted = intercept + cfo_rad_per_s * t
        residuals.append(wrap_to_pm_pi(phi - predicted))
    return residuals, cfo_rad_per_s


def classify_band(p95_rad: float) -> str:
    """Green / amber / red per campaign gates."""
    if p95_rad < GREEN_MAX_RAD:
        return "green"
    if p95_rad < AMBER_MAX_RAD:
        return "amber"
    return "red"


def run_m2_characterisation(nodes: Dict[str, Node],
                            tx_id: str,
                            rx_ids: List[str],
                            f_k: float,
                            times_s: List[float],
                            static_paths: Dict[Tuple[str, str], List[Tuple[float, float]]],
                            scatterers: List[Scatterer],
                            include_lo: bool = True) -> M2ExperimentReport:
    """Run M2 characterisation for one transmitter against multiple receivers."""
    results: List[M2Result] = []
    for rx_id in rx_ids:
        residuals, cfo_rad_per_s = compute_m2_residuals(
            nodes, tx_id, rx_id, f_k, times_s, static_paths,
            scatterers, include_lo,
        )
        abs_residuals = [abs(r) for r in residuals]
        median_rad = statistics.median(abs_residuals)
        sorted_res = sorted(abs_residuals)
        idx = int(math.ceil(0.95 * len(sorted_res))) - 1
        idx = max(0, min(idx, len(sorted_res) - 1))
        p95_rad = sorted_res[idx]
        band = classify_band(p95_rad)
        notes: List[str] = []
        if band == "red":
            notes.append("M2 red: cross-node coherent BCPT hardware-gated")
        elif band == "amber":
            notes.append("M2 amber: BCPT possible with caution, report per-band")
        results.append(M2Result(
            pair=(tx_id, rx_id),
            packet_interval_s=(times_s[1] - times_s[0]) if len(times_s) > 1 else 0.0,
            n_packets=len(times_s),
            residuals_rad=residuals,
            median_rad=median_rad,
            p95_rad=p95_rad,
            band=band,
            cfo_estimate_rad_per_s=cfo_rad_per_s,
            notes=notes,
        ))

    bands = [r.band for r in results]
    if "red" in bands:
        worst_band = "red"
        bcpt_cleared = False
    elif "amber" in bands:
        worst_band = "amber"
        bcpt_cleared = True  # proceed with caution
    else:
        worst_band = "green"
        bcpt_cleared = True

    if worst_band == "red":
        gate_message = ("M2 red: cross-node coherent BCPT hardware-gated for this "
                        "wireless-only setup. FZOG + ATSI still ship.")
    elif worst_band == "amber":
        gate_message = ("M2 amber: BCPT may proceed with per-band reporting; "
                        "residuals are elevated but below 1 rad.")
    else:
        gate_message = ("M2 green: cross-node coherent BCPT cleared for this "
                        "setup, within tested envelope.")

    return M2ExperimentReport(
        transmitter=tx_id,
        receivers=list(rx_ids),
        n_links=len(results),
        results=results,
        worst_band=worst_band,
        bcpt_cleared=bcpt_cleared,
        gate_message=gate_message,
    )


def run_m2_simulation_demo() -> M2ExperimentReport:
    """Self-contained simulation demo of M2 characterisation (no hardware)."""
    random.seed(20261017)
    nodes = build_star_mesh("TX0", [f"RX{i}" for i in range(6)],
                            separation_m=3.0)
    scatterers: List[Scatterer] = []  # empty room baseline
    rx_positions = {rx_id: nodes[rx_id].position for rx_id in
                    [f"RX{i}" for i in range(6)]}
    static_paths = default_static_paths_star("TX0", nodes["TX0"].position, rx_positions)
    f_k = 2_412_000_000.0 + 10_000_000.0  # centre subcarrier approx
    times_s = [i * 0.01 for i in range(100)]  # 100 Hz for 1 s
    return run_m2_characterisation(nodes, "TX0",
                                   [f"RX{i}" for i in range(6)],
                                   f_k, times_s, static_paths, scatterers,
                                   include_lo=True)


if __name__ == "__main__":
    import json
    report = run_m2_simulation_demo()
    print(json.dumps({
        "worst_band": report.worst_band,
        "bcpt_cleared": report.bcpt_cleared,
        "gate_message": report.gate_message,
        "results": [
            {
                "pair": r.pair,
                "median_rad": r.median_rad,
                "p95_rad": r.p95_rad,
                "band": r.band,
            }
            for r in report.results
        ],
    }, indent=2))
