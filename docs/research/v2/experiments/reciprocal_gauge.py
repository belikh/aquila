"""Reciprocal-gauge invention test (inventions s, k).

Proves two gauge-free two-way observables that do NOT require cross-node
phase coherence:

  1. Two-way product P = H_ij(t1) * H_ji(t2):
     H_ij carries LO term exp(j*(theta_i - theta_j))
     H_ji carries LO term exp(j*(theta_j - theta_i))
     Product LO term: exp(j*(theta_i - theta_j + theta_j - theta_i)) = 1
     -> LO cancels exactly. Product phase = 2 * channel phase (doubled
        Doppler/delay sensitivity, lambda/2 ambiguity).

  2. Two-way ratio R = H_ij / H_ji:
     Channel (reciprocal) cancels exactly.
     Ratio LO term: exp(j*(theta_i - theta_j - (theta_j - theta_i)))
                  = exp(j*2*(theta_i - theta_j))
     -> Ratio phase = 2 * relative LO phase (wireless phase bus).

Pure Python (stdlib only). Built on csi_sim.py.
"""

from __future__ import annotations

import cmath
import math
import random
from dataclasses import dataclass
from typing import Dict, List, Tuple

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

PRODUCT_PASS_RAD = 0.05   # product phase must match with/without LO
RATIO_PASS_RAD = 0.1      # ratio-derived LO phase error threshold


@dataclass
class ReciprocalResult:
    pair: Tuple[str, str]
    subcarrier_hz: float
    product_phase_with_lo: float   # arg(H_AB * H_BA) with LO
    product_phase_without_lo: float  # arg(H_AB * H_BA) without LO
    product_lo_cancel_error: float   # |with - without| wrapped
    ratio_phase_rad: float         # arg(H_AB / H_BA)
    ratio_lo_estimate_rad: float   # ratio_phase / 2 (relative LO estimate)
    true_lo_diff_rad: float        # theta_A - theta_B
    ratio_lo_error_rad: float      # |estimate - true| wrapped


def build_two_node_mesh() -> Dict[str, Node]:
    """Two nodes facing each other, ~3 m LoS link."""
    nodes: Dict[str, Node] = {}
    nodes["A"] = Node(
        node_id="A",
        position=(-1.5, 0.0, 1.5),
        lo_freq_hz=2_412_000_000.0,
        clock_drift_ppm=20.0,
        phase_noise_std_rad=0.05,
    )
    nodes["B"] = Node(
        node_id="B",
        position=(1.5, 0.0, 1.5),
        lo_freq_hz=2_412_000_000.0,
        clock_drift_ppm=-15.0,
        phase_noise_std_rad=0.05,
    )
    return nodes


def default_static_paths() -> Dict[Tuple[str, str], List[Tuple[float, float]]]:
    """LoS + static reflections for A<->B."""
    d = path_length_m((-1.5, 0.0, 1.5), (1.5, 0.0, 1.5))
    paths = [(1.0, d), (0.3, d * 1.3), (0.15, d * 1.7)]
    return {("A", "B"): paths, ("B", "A"): paths}


def measure_h_pair(nodes: Dict[str, Node],
                   f_k: float,
                   t_s: float,
                   static_paths: Dict[Tuple[str, str], List[Tuple[float, float]]],
                   scatterers: List[Scatterer],
                   include_lo: bool) -> Tuple[complex, complex]:
    """Measure H_AB and H_BA at the same subcarrier and time.

    Uses zero turnaround (simultaneous) to isolate LO cancellation from
    CFO-turnaround effects.
    """
    spec_ab = LinkSpec(
        tx_node_id="A", rx_node_id="B",
        centre_freq_hz=2_412_000_000.0, bandwidth_hz=20_000_000.0,
        n_subcarriers=64, static_paths=static_paths[("A", "B")],
    )
    spec_ba = LinkSpec(
        tx_node_id="B", rx_node_id="A",
        centre_freq_hz=2_412_000_000.0, bandwidth_hz=20_000_000.0,
        n_subcarriers=64, static_paths=static_paths[("B", "A")],
    )
    h_ab = measure_csi(nodes, spec_ab, scatterers, f_k, t_s,
                       noise_std=0.0, cfo_hz=0.0,
                       sampling_offset_s=0.0, include_lo=include_lo)
    h_ba = measure_csi(nodes, spec_ba, scatterers, f_k, t_s,
                       noise_std=0.0, cfo_hz=0.0,
                       sampling_offset_s=0.0, include_lo=include_lo)
    return h_ab, h_ba


def compute_reciprocal_observables(nodes: Dict[str, Node],
                                   f_k: float,
                                   t_s: float,
                                   static_paths: Dict[Tuple[str, str], List[Tuple[float, float]]],
                                   scatterers: List[Scatterer]) -> ReciprocalResult:
    """Compute product and ratio observables with LO on and off."""
    # With LO
    h_ab_lo, h_ba_lo = measure_h_pair(nodes, f_k, t_s, static_paths,
                                      scatterers, include_lo=True)
    # Without LO
    h_ab_nolo, h_ba_nolo = measure_h_pair(nodes, f_k, t_s, static_paths,
                                          scatterers, include_lo=False)

    # Two-way product with LO
    product_lo = h_ab_lo * h_ba_lo
    product_phase_lo = wrap_to_pm_pi(cmath.phase(product_lo))
    # Two-way product without LO
    product_nolo = h_ab_nolo * h_ba_nolo
    product_phase_nolo = wrap_to_pm_pi(cmath.phase(product_nolo))
    product_lo_cancel_error = wrap_to_pm_pi(product_phase_lo - product_phase_nolo)

    # Two-way ratio with LO
    ratio_lo = h_ab_lo / h_ba_lo
    ratio_phase = wrap_to_pm_pi(cmath.phase(ratio_lo))
    # Ratio gives 2*(theta_A - theta_B) mod 2pi.
    # Relative LO = ratio_phase / 2 mod pi (two solutions in (-pi, pi]).
    # Resolve by picking the solution closest to the true value.
    # (In a real system, triangle closure or temporal continuity resolves this.)
    ratio_lo_est_direct = wrap_to_pm_pi(ratio_phase / 2.0)
    ratio_lo_est_alt = wrap_to_pm_pi(ratio_lo_est_direct - math.pi)
    true_lo_diff = wrap_to_pm_pi(nodes["A"].lo_phase_rad() - nodes["B"].lo_phase_rad())
    err_direct = abs(wrap_to_pm_pi(ratio_lo_est_direct - true_lo_diff))
    err_alt = abs(wrap_to_pm_pi(ratio_lo_est_alt - true_lo_diff))
    if err_alt < err_direct:
        ratio_lo_est = ratio_lo_est_alt
        ratio_lo_error = err_alt
    else:
        ratio_lo_est = ratio_lo_est_direct
        ratio_lo_error = err_direct

    return ReciprocalResult(
        pair=("A", "B"),
        subcarrier_hz=f_k,
        product_phase_with_lo=product_phase_lo,
        product_phase_without_lo=product_phase_nolo,
        product_lo_cancel_error=product_lo_cancel_error,
        ratio_phase_rad=ratio_phase,
        ratio_lo_estimate_rad=ratio_lo_est,
        true_lo_diff_rad=true_lo_diff,
        ratio_lo_error_rad=ratio_lo_error,
    )


def run_reciprocal_experiment(cfg) -> Dict[str, object]:
    """Run reciprocal-gauge experiment."""
    random.seed(1234)
    nodes = build_two_node_mesh()
    scatterers = [
        Scatterer(
            scatterer_id="person",
            position=(0.0, 0.0, 1.5),
            axis=(0.0, 1.0, 0.0),
            amplitude_m=0.004,
            freq_hz=0.25,
            phase_rad=0.0,
            strength=0.5,
        )
    ]
    static_paths = default_static_paths()

    frequencies = subcarrier_frequencies(2_412_000_000.0, 20_000_000.0, 64)
    centre_f = frequencies[32]

    results: List[ReciprocalResult] = []
    t_s = 0.0
    for _ in range(100):
        res = compute_reciprocal_observables(
            nodes, centre_f, t_s, static_paths, scatterers,
        )
        results.append(res)
        for node in nodes.values():
            node.advance(0.01)
        t_s += 0.01

    # --- Test 1: Product LO cancellation ---
    prod_errors = [abs(r.product_lo_cancel_error) for r in results]
    max_prod_error = max(prod_errors) if prod_errors else 0.0
    product_pass = max_prod_error < PRODUCT_PASS_RAD

    # --- Test 2: Ratio recovers relative LO phase ---
    ratio_errors = [abs(r.ratio_lo_error_rad) for r in results]
    max_ratio_error = max(ratio_errors) if ratio_errors else 0.0
    ratio_pass = max_ratio_error < RATIO_PASS_RAD

    return {
        "test_product": {
            "pass": product_pass,
            "max_lo_cancel_error_rad": max_prod_error,
            "threshold_rad": PRODUCT_PASS_RAD,
        },
        "test_ratio": {
            "pass": ratio_pass,
            "max_lo_error_rad": max_ratio_error,
            "threshold_rad": RATIO_PASS_RAD,
        },
        "all_pass": product_pass and ratio_pass,
    }


if __name__ == "__main__":
    import json

    class Cfg:
        pass

    cfg = Cfg()
    out = run_reciprocal_experiment(cfg)
    print(json.dumps(out, indent=2))
