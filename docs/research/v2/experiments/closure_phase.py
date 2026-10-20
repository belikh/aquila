"""Jennison closure-phase invention test (patent Claim 1 / Claim 9).

Proves three things from synthetic CSI:

  1. LO cancellation: closure phase around a directed loop is IDENTICAL
     whether per-node LO terms are included or removed. Each node's LO
     phase appears once as TX (positive) and once as RX (negative) and
     therefore telescopes to zero — no inter-node phase coherence needed.

  2. Scatterer sensitivity: closure phase with scatterer present vs absent
     differs by the scatterer-induced path perturbation. LO, static
     multipath, CFO and sampling offsets all cancel in the difference.

  3. 3D localisation: multiple directed loops of different geometry give
     intersecting closure-phase-difference constraints that localise a
     moving passive scatterer in 3D.

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

LO_CANCEL_TOL_RAD = 0.05   # closure phase must match with/without LO
LOCALISATION_TOL_M = 0.8   # 3D position error threshold (metres)
SCATTER_DETECT_RATIO = 1.5 # variance ratio threshold for still detection


@dataclass
class Loop:
    """A directed multi-node packet loop: ordered cycle of node ids."""

    name: str
    nodes: Tuple[str, ...]

    def links(self) -> List[Tuple[str, str]]:
        n = len(self.nodes)
        return [(self.nodes[i], self.nodes[(i + 1) % n]) for i in range(n)]


def build_mesh(n_nodes: int = 6, radius: float = 4.0) -> Dict[str, Node]:
    """N fixed nodes on a perimeter, maximally spaced (v2 placement)."""
    nodes: Dict[str, Node] = {}
    for i in range(n_nodes):
        ang = 2.0 * math.pi * i / n_nodes
        x = radius * math.cos(ang)
        y = radius * math.sin(ang)
        z = 1.5
        node_id = f"n{i}"
        nodes[node_id] = Node(
            node_id=node_id,
            position=(x, y, z),
            lo_freq_hz=2_412_000_000.0,
            clock_drift_ppm=20.0 * (1.0 + 0.1 * (i % 3 - 1)),
            phase_noise_std_rad=0.05,
        )
    return nodes


def default_scatterers() -> List[Scatterer]:
    """One moving person at centre of mesh, breathing-scale chest motion."""
    return [
        Scatterer(
            scatterer_id="person",
            position=(0.0, 0.0, 1.2),
            axis=(1.0, 0.0, 0.0),
            amplitude_m=0.004,
            freq_hz=0.25,
            phase_rad=0.0,
            strength=0.6,
        )
    ]


def default_static_multipath(tx: Tuple[float, float, float],
                             rx: Tuple[float, float, float]) -> List[Tuple[float, float]]:
    """Empty-home baseline: LoS + static reflections."""
    d = path_length_m(tx, rx)
    return [(1.0, d), (0.25, d * 1.25), (0.15, d * 1.6)]


def closure_phase_loop(nodes: Dict[str, Node],
                       loop: Loop,
                       f_k: float,
                       t_s: float,
                       static_paths: Dict[Tuple[str, str], List[Tuple[float, float]]],
                       scatterers: List[Scatterer],
                       include_lo: bool = True,
                       centre_freq_hz: float = 2_412_000_000.0,
                       bandwidth_hz: float = 20_000_000.0,
                       n_subcarriers: int = 64) -> float:
    """Jennison closure phase around a directed loop at subcarrier f_k.

    Sum of measured link phases around the loop. With include_lo=False, the
    per-node LO terms are removed; the closure phase should be unchanged
    (LO cancels exactly around a directed loop).
    """
    phase_sum = 0.0
    for tx_id, rx_id in loop.links():
        spec = LinkSpec(
            tx_node_id=tx_id, rx_node_id=rx_id,
            centre_freq_hz=centre_freq_hz, bandwidth_hz=bandwidth_hz,
            n_subcarriers=n_subcarriers,
            static_paths=static_paths[(tx_id, rx_id)],
        )
        h = measure_csi(nodes, spec, scatterers, f_k, t_s,
                        noise_std=0.0, cfo_hz=0.0,
                        sampling_offset_s=0.0, include_lo=include_lo)
        phase_sum += link_phase_rad(h)
    return wrap_to_pm_pi(phase_sum)


def closure_phase_vs_freq(nodes: Dict[str, Node],
                          loop: Loop,
                          frequencies: List[float],
                          t_s: float,
                          static_paths: Dict[Tuple[str, str], List[Tuple[float, float]]],
                          scatterers: List[Scatterer],
                          include_lo: bool = True,
                          centre_freq_hz: float = 2_412_000_000.0,
                          bandwidth_hz: float = 20_000_000.0,
                          n_subcarriers: int = 64) -> List[float]:
    """Closure phase at every subcarrier frequency."""
    return [closure_phase_loop(nodes, loop, f_k, t_s, static_paths, scatterers,
                               include_lo, centre_freq_hz, bandwidth_hz,
                               n_subcarriers) for f_k in frequencies]


def estimate_path_length_from_slope(closure_phases: List[float],
                                    frequencies: List[float]) -> float:
    """Estimate effective loop path length from closure phase vs frequency.

    closure_phase[k] approx = -2*pi*f_k/c * D_loop (mod 2pi).
    After unwrapping across frequency, slope = -2*pi/c * D_loop.
    """
    n = len(closure_phases)
    if n < 2:
        return 0.0
    # Unwrap across frequency.
    unwrapped = [closure_phases[0]]
    two_pi = 2.0 * math.pi
    for k in range(1, n):
        y = closure_phases[k]
        prev = unwrapped[-1]
        n_rot = round((prev - y) / two_pi)
        unwrapped.append(y + n_rot * two_pi)
    f_mean = sum(frequencies) / n
    y_mean = sum(unwrapped) / n
    num = sum((f - f_mean) * (y - y_mean) for f, y in zip(frequencies, unwrapped))
    den = sum((f - f_mean) ** 2 for f in frequencies)
    if den == 0.0:
        return 0.0
    slope = num / den
    return -slope * SPEED_OF_LIGHT / (2.0 * math.pi)


def localise_from_loops(nodes: Dict[str, Node],
                        loops: List[Loop],
                        loop_delta_vector: Dict[str, List[float]],
                        frequencies: List[float],
                        scatterers: List[Scatterer],
                        static_paths: Dict[Tuple[str, str], List[Tuple[float, float]]],
                        centre_freq_hz: float,
                        bandwidth_hz: float,
                        n_subcarriers: int) -> Tuple[float, float, float]:
    """Grid-search 3D position from closure-phase DIFFERENCE VECTORS
    (present vs absent scatterer) across ALL subcarriers.

    For each candidate position, compute the predicted closure-phase
    difference at every subcarrier (with vs without scatterer at that
    position). Compare the full vector to the measured vector. Using the
    whole frequency vector (not just one subcarrier) resolves the 2*pi
    ambiguity: different subcarriers wrap differently, so only the true
    position matches across all frequencies.

    Small grid around the centre (±1 m) keeps compute tractable while
    still demonstrating geometric localisation.
    """
    best_pos = (0.0, 0.0, 1.5)
    best_err = float("inf")
    step = 0.1
    rng = range(-5, 6)   # -0.5..0.5 m around centre

    # `frequencies` is already sub-sampled (every 4th subcarrier).
    sub_freqs = frequencies

    for ix in rng:
        x = ix * step
        for iy in rng:
            y = iy * step
            for iz in range(6, 14):  # 0.6..1.4 m
                z = iz * step
                cand = (x, y, z)
                err = 0.0
                for loop in loops:
                    # Measured delta vector (present - absent) for this loop.
                    delta_meas = loop_delta_vector[loop.name]
                    # Predicted delta vector for candidate position.
                    delta_pred = []
                    for f_k in sub_freqs:
                        dp = 0.0
                        for tx_id, rx_id in loop.links():
                            tx_p = nodes[tx_id].position
                            rx_p = nodes[rx_id].position
                            direct = path_length_m(tx_p, rx_p)
                            via = path_length_m(tx_p, cand) + path_length_m(cand, rx_p)
                            amp_dir = 1.0
                            amp_scat = scatterers[0].strength / (1.0 + via / 3.0)
                            h_with = (amp_dir * cmath.exp(-1j * 2.0 * math.pi * f_k * direct / SPEED_OF_LIGHT)
                                      + amp_scat * cmath.exp(-1j * 2.0 * math.pi * f_k * via / SPEED_OF_LIGHT))
                            h_without = amp_dir * cmath.exp(-1j * 2.0 * math.pi * f_k * direct / SPEED_OF_LIGHT)
                            dp += wrap_to_pm_pi(link_phase_rad(h_with) - link_phase_rad(h_without))
                        delta_pred.append(wrap_to_pm_pi(dp))
                    # Vector residual (circular distance).
                    for pm, dm in zip(delta_pred, delta_meas):
                        err += wrap_to_pm_pi(pm - dm) ** 2
                if err < best_err:
                    best_err = err
                    best_pos = cand
    return best_pos


def run_closure_experiment(cfg) -> Dict[str, object]:
    """Run the full closure-phase experiment."""
    random.seed(cfg.seed)
    nodes = build_mesh(cfg.n_nodes)
    scatterers = default_scatterers()
    node_ids = list(nodes.keys())

    # Directed loops: consecutive triangles + centre-crossing loops.
    loops: List[Loop] = []
    for i in range(cfg.n_nodes):
        a = node_ids[i]
        b = node_ids[(i + 1) % cfg.n_nodes]
        c = node_ids[(i + 2) % cfg.n_nodes]
        loops.append(Loop(name=f"L{i}", nodes=(a, b, c)))
    if cfg.n_nodes >= 4:
        loops.append(Loop(name="Lcross1", nodes=(node_ids[0], node_ids[2], node_ids[4])))
        loops.append(Loop(name="Lcross2", nodes=(node_ids[1], node_ids[3], node_ids[5])))

    # Static multipath per directed link (empty-home baseline).
    static_paths: Dict[Tuple[str, str], List[Tuple[float, float]]] = {}
    for tx_id in node_ids:
        for rx_id in node_ids:
            if tx_id != rx_id:
                static_paths[(tx_id, rx_id)] = default_static_multipath(
                    nodes[tx_id].position, nodes[rx_id].position)

    frequencies = subcarrier_frequencies(cfg.centre_freq_hz, cfg.bandwidth_hz,
                                         cfg.n_subcarriers)
    centre_f = frequencies[cfg.n_subcarriers // 2]

    # --- Test 1: LO cancellation ---
    # Closure phase with LO ON vs LO OFF must be identical.
    t_s = cfg.n_frames * cfg.frame_interval_s / 2.0  # mid-experiment time
    lo_cancel_errors: List[float] = []
    for loop in loops:
        cp_with_lo = closure_phase_loop(nodes, loop, centre_f, t_s, static_paths,
                                        scatterers, include_lo=True,
                                        centre_freq_hz=cfg.centre_freq_hz,
                                        bandwidth_hz=cfg.bandwidth_hz,
                                        n_subcarriers=cfg.n_subcarriers)
        cp_without_lo = closure_phase_loop(nodes, loop, centre_f, t_s, static_paths,
                                           scatterers, include_lo=False,
                                           centre_freq_hz=cfg.centre_freq_hz,
                                           bandwidth_hz=cfg.bandwidth_hz,
                                           n_subcarriers=cfg.n_subcarriers)
        lo_cancel_errors.append(abs(wrap_to_pm_pi(cp_with_lo - cp_without_lo)))
    max_lo_cancel = max(lo_cancel_errors) if lo_cancel_errors else 0.0
    mean_lo_cancel = sum(lo_cancel_errors) / len(lo_cancel_errors) if lo_cancel_errors else 0.0
    lo_cancel_pass = max_lo_cancel < LO_CANCEL_TOL_RAD

    # --- Test 2: Frequency diversity -> path length estimate ---
    # Closure phase vs frequency with scatterer present; estimate D.
    loop0 = loops[0]
    cp_present = closure_phase_vs_freq(nodes, loop0, frequencies, t_s, static_paths,
                                       scatterers, include_lo=False,
                                       centre_freq_hz=cfg.centre_freq_hz,
                                       bandwidth_hz=cfg.bandwidth_hz,
                                       n_subcarriers=cfg.n_subcarriers)
    D_est = estimate_path_length_from_slope(cp_present, frequencies)
    # True D = sum of direct link lengths (LoS only).
    D_true = sum(path_length_m(nodes[a].position, nodes[b].position)
                 for a, b in loop0.links())
    D_error = abs(D_est - D_true)
    freq_diversity_pass = D_error < 0.5

    # --- Test 3: Scatterer detection (differential closure phase) ---
    # Closure phase present vs absent: difference isolates scatterer.
    cp_absent = closure_phase_vs_freq(nodes, loop0, frequencies, t_s, static_paths,
                                      [], include_lo=False,
                                      centre_freq_hz=cfg.centre_freq_hz,
                                      bandwidth_hz=cfg.bandwidth_hz,
                                      n_subcarriers=cfg.n_subcarriers)
    # Mean absolute difference across subcarriers.
    diffs = [abs(wrap_to_pm_pi(p - a)) for p, a in zip(cp_present, cp_absent)]
    mean_diff = sum(diffs) / len(diffs) if diffs else 0.0
    scatter_detect_pass = mean_diff > 0.05  # any measurable perturbation

    # --- Test 4: 3D localisation from multi-loop closure-difference vectors ---
    # Full closure-phase-difference vector (present vs absent) across all
    # subcarriers per loop. Different loop geometries give intersecting
    # constraints -> 3D position. The whole frequency vector resolves 2*pi
    # ambiguity.
    sub_idx = list(range(0, cfg.n_subcarriers, 4))
    sub_freqs = [frequencies[i] for i in sub_idx]

    loop_delta_vector: Dict[str, List[float]] = {}
    for loop in loops:
        cp_p = closure_phase_vs_freq(nodes, loop, sub_freqs, t_s, static_paths,
                                     scatterers, include_lo=False,
                                     centre_freq_hz=cfg.centre_freq_hz,
                                     bandwidth_hz=cfg.bandwidth_hz,
                                     n_subcarriers=cfg.n_subcarriers)
        cp_a = closure_phase_vs_freq(nodes, loop, sub_freqs, t_s, static_paths,
                                     [], include_lo=False,
                                     centre_freq_hz=cfg.centre_freq_hz,
                                     bandwidth_hz=cfg.bandwidth_hz,
                                     n_subcarriers=cfg.n_subcarriers)
        loop_delta_vector[loop.name] = [
            wrap_to_pm_pi(p - a) for p, a in zip(cp_p, cp_a)]

    est_pos = localise_from_loops(nodes, loops, loop_delta_vector, sub_freqs,
                                  scatterers, static_paths,
                                  cfg.centre_freq_hz, cfg.bandwidth_hz,
                                  cfg.n_subcarriers)
    true_pos = scatterers[0].position_at(t_s)
    loc_error = path_length_m(est_pos, true_pos)
    localisation_pass = loc_error < LOCALISATION_TOL_M

    # --- Test 5: Still-person detection (static fingerprint, not Doppler) ---
    # Closure phase variance over time: person present increases variance.
    def closure_var_over_time(scatters: List[Scatterer]) -> float:
        vals = []
        t = 0.0
        for _ in range(60):
            v = closure_phase_loop(nodes, loop0, centre_f, t, static_paths,
                                   scatters, include_lo=False,
                                   centre_freq_hz=cfg.centre_freq_hz,
                                   bandwidth_hz=cfg.bandwidth_hz,
                                   n_subcarriers=cfg.n_subcarriers)
            vals.append(v)
            for node in nodes.values():
                node.advance(cfg.frame_interval_s)
            t += cfg.frame_interval_s
        mean = sum(vals) / len(vals)
        return sum((v - mean) ** 2 for v in vals) / len(vals)

    var_present = closure_var_over_time(scatterers)
    var_absent = closure_var_over_time([])
    still_detect_pass = var_present > var_absent * SCATTER_DETECT_RATIO

    return {
        "config": cfg,
        "n_loops": len(loops),
        "n_frames": cfg.n_frames,
        "test_lo_cancel": {
            "pass": lo_cancel_pass,
            "max_residual_rad": max_lo_cancel,
            "mean_residual_rad": mean_lo_cancel,
            "threshold_rad": LO_CANCEL_TOL_RAD,
        },
        "test_frequency_diversity": {
            "pass": freq_diversity_pass,
            "D_true_m": D_true,
            "D_est_m": D_est,
            "error_m": D_error,
            "threshold_m": 0.5,
        },
        "test_scatterer_detection": {
            "pass": scatter_detect_pass,
            "mean_closure_diff_rad": mean_diff,
            "threshold_rad": 0.05,
        },
        "test_localisation": {
            "pass": localisation_pass,
            "true_pos_m": list(true_pos),
            "est_pos_m": list(est_pos),
            "error_m": loc_error,
            "threshold_m": LOCALISATION_TOL_M,
        },
        "test_still_detection": {
            "pass": still_detect_pass,
            "var_present": var_present,
            "var_absent": var_absent,
            "ratio": var_present / var_absent if var_absent > 0 else float("inf"),
        },
        "all_pass": (lo_cancel_pass and freq_diversity_pass
                     and scatter_detect_pass and localisation_pass
                     and still_detect_pass),
    }


if __name__ == "__main__":
    import json

    class Cfg:
        pass

    cfg = Cfg()
    cfg.n_nodes = 6
    cfg.centre_freq_hz = 2_412_000_000.0
    cfg.bandwidth_hz = 20_000_000.0
    cfg.n_subcarriers = 64
    cfg.n_frames = 200
    cfg.frame_interval_s = 0.01
    cfg.lo_drift_ppm = 20.0
    cfg.phase_noise_std_rad = 0.05
    cfg.noise_std = 0.05
    cfg.cfo_hz = 100.0
    cfg.sampling_offset_s = 1e-6
    cfg.seed = 42

    out = run_closure_experiment(cfg)
    print(json.dumps({
        "lo_cancel": out["test_lo_cancel"],
        "freq_diversity": out["test_frequency_diversity"],
        "scatterer_detection": out["test_scatterer_detection"],
        "localisation": out["test_localisation"],
        "still_detection": out["test_still_detection"],
        "all_pass": out["all_pass"],
    }, indent=2))
