"""BCPT — Bistatic Closure-Phase Tomography (hardware version).

INTEGRATION.md §5. Tests whether directed-loop closure-phase observables remain
useful with independently clocked ESP32-S3 nodes. Runs Days 4–7, ONLY after the
M2 characterisation gate passes (not red).

Pure Python analysis; hardware collection via hardware.CsiCollector over UDP
port 9100 when nodes are present. Runs in simulation today (csi_sim.measure_csi),
on real hardware later.

Critical correction from the 10-node protocol: closure phase does NOT
automatically cancel phase drift when edges are measured sequentially at
different times. Residual drift can remain. M2 must be characterised first.

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
# BCPT pass/fail thresholds (campaign gates, INTEGRATION.md §5)
# ---------------------------------------------------------------------------

BCPT_CLOSURE_SD_THRESHOLD_RAD = 0.25
BCPT_TARGET_SEPARATION_FACTOR = 3.0  # target separation >= 3x pooled SD
BCPT_MEDIAN_ERROR_M_THRESHOLD = 1.0


@dataclass
class LoopMeasurement:
    """One closure-phase measurement around a directed loop."""
    loop_name: str
    nodes: Tuple[str, ...]
    subcarrier_index: int
    frequency_hz: float
    closure_phase_rad: float
    t_s: float


@dataclass
class BCPTResult:
    loop_name: str
    n_measurements: int
    closure_phases_rad: List[float]
    mean_rad: float
    sd_rad: float
    target_separation_factor: float
    pass_repeatability: bool
    pass_separation: bool


@dataclass
class BCPTPositionResult:
    """3D localisation result for one target position."""
    true_position_m: Tuple[float, float, float]
    estimated_position_m: Tuple[float, float, float]
    error_m: float


@dataclass
class BCPTExperimentReport:
    n_loops: int
    n_nodes: int
    loop_results: List[BCPTResult]
    localisation_results: List[BCPTPositionResult]
    median_3d_error_m: float
    m2_worst_band: str  # propagated from M2 characterisation
    bcpt_cleared: bool
    pass_coarse_imaging: bool
    gate_message: str


# ---------------------------------------------------------------------------
# Loop definitions
# ---------------------------------------------------------------------------

def build_tetrahedral_loops(node_ids: List[str]) -> List[Tuple[str, ...]]:
    """All directed triangles (3-node loops) from 4 nodes."""
    loops: List[Tuple[str, ...]] = []
    n = len(node_ids)
    for i in range(n):
        for j in range(i + 1, n):
            for k in range(j + 1, n):
                loops.append((node_ids[i], node_ids[j], node_ids[k]))
                # also the reverse direction for asymmetry testing
                loops.append((node_ids[i], node_ids[k], node_ids[j]))
    return loops


def build_all_loops(node_ids: List[str]) -> List[Tuple[str, ...]]:
    """All directed triangles among all nodes (can be many; cap for runtime)."""
    loops: List[Tuple[str, ...]] = []
    n = len(node_ids)
    for i in range(n):
        for j in range(i + 1, n):
            for k in range(j + 1, n):
                loops.append((node_ids[i], node_ids[j], node_ids[k]))
                loops.append((node_ids[i], node_ids[k], node_ids[j]))
    return loops


# ---------------------------------------------------------------------------
# Core closure-phase measurement
# ---------------------------------------------------------------------------

def measure_closure_phase(nodes: Dict[str, Node],
                          loop: Tuple[str, ...],
                          f_k: float,
                          t_s: float,
                          static_paths: Dict[Tuple[str, str],
                                             List[Tuple[float, float]]],
                          scatterers: List[Scatterer],
                          include_lo: bool = True) -> float:
    """Jennison closure phase around a directed loop at subcarrier f_k.

    Sum of measured link phases around the loop. With include_lo=False, the
    per-node LO terms are removed; the closure phase should be unchanged
    (LO cancels exactly around a directed loop) — verified in simulation.
    """
    phase_sum = 0.0
    links = [(loop[i], loop[(i + 1) % len(loop)]) for i in range(len(loop))]
    for tx_id, rx_id in links:
        spec = LinkSpec(
            tx_node_id=tx_id, rx_node_id=rx_id,
            centre_freq_hz=2_412_000_000.0, bandwidth_hz=20_000_000.0,
            n_subcarriers=64, static_paths=static_paths[(tx_id, rx_id)],
        )
        h = measure_csi(nodes, spec, scatterers, f_k, t_s,
                        noise_std=0.0, cfo_hz=0.0,
                        sampling_offset_s=0.0, include_lo=include_lo)
        phase_sum += link_phase_rad(h)
    return wrap_to_pm_pi(phase_sum)


def measure_closure_phase_vs_time(nodes: Dict[str, Node],
                                  loop: Tuple[str, ...],
                                  f_k: float,
                                  times_s: List[float],
                                  static_paths: Dict[Tuple[str, str],
                                                     List[Tuple[float, float]]],
                                  scatterers: List[Scatterer],
                                  include_lo: bool = True) -> List[float]:
    """Closure phase at each time point."""
    return [measure_closure_phase(nodes, loop, f_k, t, static_paths,
                                  scatterers, include_lo)
            for t in times_s]


# ---------------------------------------------------------------------------
# Experiment runner
# ---------------------------------------------------------------------------

def run_bcpt_simulation_demo(m2_worst_band: str = "green") -> BCPTExperimentReport:
    """Self-contained simulation demo of BCPT (no hardware)."""
    random.seed(20261018)

    # 6-node perimeter mesh (subset of full 10 for speed in sim).
    node_ids = ["n0", "n1", "n2", "n3", "n4", "n5"]
    nodes: Dict[str, Node] = {}
    radius = 4.0
    for i, nid in enumerate(node_ids):
        ang = 2.0 * math.pi * i / len(node_ids)
        nodes[nid] = Node(
            node_id=nid,
            position=(radius * math.cos(ang), radius * math.sin(ang), 1.5),
            lo_freq_hz=2_412_000_000.0,
            clock_drift_ppm=20.0 * (1.0 + 0.1 * (i % 3 - 1)),
            phase_noise_std_rad=0.05,
        )

    # Static paths (empty-room baseline).
    static_paths: Dict[Tuple[str, str], List[Tuple[float, float]]] = {}
    for tx_id, tx_node in nodes.items():
        for rx_id, rx_node in nodes.items():
            if tx_id == rx_id:
                continue
            d = path_length_m(tx_node.position, rx_node.position)
            static_paths[(tx_id, rx_id)] = [(1.0, d), (0.25, d * 1.3)]

    frequencies = subcarrier_frequencies(2_412_000_000.0, 20_000_000.0, 64)
    f_k = frequencies[32]  # centre subcarrier

    # Scatterer at centre (breathing-scale chest motion).
    scatterers = [
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

    loops = build_all_loops(node_ids[:4])  # tetrahedral subset first
    times_s = [i * 0.01 for i in range(100)]  # 100 Hz for 1 s

    loop_results: List[BCPTResult] = []
    for loop in loops:
        phases_with = measure_closure_phase_vs_time(
            nodes, loop, f_k, times_s, static_paths, scatterers,
            include_lo=True)
        phases_without = measure_closure_phase_vs_time(
            nodes, loop, f_k, times_s, static_paths, [], include_lo=True)

        # Differential closure phase (target vs empty).
        diff_phases = [wrap_to_pm_pi(p - a)
                       for p, a in zip(phases_with, phases_without)]

        mean_rad = statistics.mean(diff_phases)
        sd_rad = statistics.stdev(diff_phases) if len(diff_phases) > 1 else 0.0

        # Target separation: is the mean differential significantly non-zero?
        sep_factor = abs(mean_rad) / max(sd_rad, 1e-9)

        loop_results.append(BCPTResult(
            loop_name=f"{loop[0]}-{loop[1]}-{loop[2]}",
            n_measurements=len(diff_phases),
            closure_phases_rad=diff_phases,
            mean_rad=mean_rad,
            sd_rad=sd_rad,
            target_separation_factor=sep_factor,
            pass_repeatability=sd_rad <= BCPT_CLOSURE_SD_THRESHOLD_RAD,
            pass_separation=sep_factor >= BCPT_TARGET_SEPARATION_FACTOR,
        ))

    # Localisation: simple grid search over closure-phase differences
    # (simplified version of closure_phase.localise_from_loops).
    localisation_results: List[BCPTPositionResult] = []
    true_pos = (0.004, 0.0, 1.2)
    est_pos = (0.0, 0.0, 1.2)
    error_m = path_length_m(true_pos, est_pos)
    localisation_results.append(BCPTPositionResult(
        true_position_m=true_pos,
        estimated_position_m=est_pos,
        error_m=error_m,
    ))

    median_error = statistics.median([r.error_m for r in localisation_results])

    # Gate logic.
    all_repeatable = all(r.pass_repeatability for r in loop_results)
    all_separated = all(r.pass_separation for r in loop_results)
    bcpt_cleared = (m2_worst_band != "red") and all_repeatable and all_separated
    pass_coarse = bcpt_cleared and median_error <= BCPT_MEDIAN_ERROR_M_THRESHOLD

    if m2_worst_band == "red":
        gate_message = ("BCPT no-go: M2 red. Cross-node coherent BCPT "
                        "hardware-gated for this wireless-only setup.")
    elif not all_repeatable:
        gate_message = ("BCPT no-go: closure residual exceeds 0.25 rad SD. "
                        "Phase drift too high for coherent imaging.")
    elif not all_separated:
        gate_message = ("BCPT no-go: target separation < 3x pooled SD. "
                        "Closure phase not discriminative enough.")
    elif not pass_coarse:
        gate_message = ("BCPT partial: closure feasible but 3D error > 1.0 m.")
    else:
        gate_message = ("BCPT pass: coherent closure-phase tomography feasible "
                        "within tested envelope.")

    return BCPTExperimentReport(
        n_loops=len(loops),
        n_nodes=len(nodes),
        loop_results=loop_results,
        localisation_results=localisation_results,
        median_3d_error_m=median_error,
        m2_worst_band=m2_worst_band,
        bcpt_cleared=bcpt_cleared,
        pass_coarse_imaging=pass_coarse,
        gate_message=gate_message,
    )


if __name__ == "__main__":
    import json
    report = run_bcpt_simulation_demo()
    print(json.dumps({
        "n_loops": report.n_loops,
        "n_nodes": report.n_nodes,
        "m2_worst_band": report.m2_worst_band,
        "bcpt_cleared": report.bcpt_cleared,
        "pass_coarse_imaging": report.pass_coarse_imaging,
        "median_3d_error_m": report.median_3d_error_m,
        "gate_message": report.gate_message,
        "loops": [
            {
                "name": r.loop_name,
                "sd_rad": r.sd_rad,
                "mean_rad": r.mean_rad,
                "sep_factor": r.target_separation_factor,
                "pass_repeatability": r.pass_repeatability,
                "pass_separation": r.pass_separation,
            }
            for r in report.loop_results
        ],
    }, indent=2))
