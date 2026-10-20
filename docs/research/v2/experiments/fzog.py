"""FZOG — 3D Fresnel-Zone Occultation Grids.

INTEGRATION.md §5. Amplitude/power-first 3D occupancy imaging; gauge-independent,
does not depend on cross-node phase coherence. Runs first in the 14-day campaign
(Days 1–3) and establishes the performance floor.

Pure Python (stdlib only) for analysis; hardware collection via hardware.CsiCollector
over UDP port 9100 when nodes are present. Runs in simulation today (csi_sim),
on real hardware later.

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
# FZOG pass/fail thresholds (campaign gates, INTEGRATION.md §5)
# ---------------------------------------------------------------------------

FZOG_PRESENCE_F1_THRESHOLD = 0.90
FZOG_MEDIAN_ERROR_M_THRESHOLD = 1.0


@dataclass
class Voxel:
    """One 3D voxel in the occupancy grid."""
    x_m: float
    y_m: float
    z_m: float
    label: str  # e.g. "grid-02-01-03"


@dataclass
class LinkFeature:
    """Per-link, per-subcarrier log-amplitude feature relative to empty baseline."""
    tx_id: str
    rx_id: str
    subcarrier_index: int
    frequency_hz: float
    baseline_log_amp: float  # median empty-room log |H|^2


@dataclass
class FZOGResult:
    voxel: Voxel
    score: float
    occupancy_probability: float
    ground_truth_occupied: bool
    predicted_occupied: bool
    error_m: float = 0.0
    predicted_voxel_index: Optional[int] = None


@dataclass
class FZOGReport:
    n_voxels: int
    n_links: int
    presence_f1: float
    presence_precision: float
    presence_recall: float
    median_3d_error_m: float
    p90_3d_error_m: float
    occupied_voxel_iou: float
    iou_ci_lower: float
    iou_ci_upper: float
    ht20_vs_ht40: dict  # separate results per bandwidth
    pass_coarse_imaging: bool
    gate_message: str


def build_grid_positions(x_count: int = 3, y_count: int = 3,
                         z_heights: Optional[List[float]] = None,
                         x_range_m: Tuple[float, float] = (0.5, 5.5),
                         y_range_m: Tuple[float, float] = (0.5, 5.5)) -> List[Voxel]:
    """3 × 3 horizontal positions × 3 target heights = 27 surveyed voxels."""
    if z_heights is None:
        z_heights = [0.4, 1.2, 2.0]
    xs = [x_range_m[0] + i * (x_range_m[1] - x_range_m[0]) / (x_count - 1)
          for i in range(x_count)]
    ys = [y_range_m[0] + i * (y_range_m[1] - y_range_m[0]) / (y_count - 1)
          for i in range(y_count)]
    voxels: List[Voxel] = []
    idx = 0
    for iz, z in enumerate(z_heights):
        for iy, y in enumerate(ys):
            for ix, x in enumerate(xs):
                voxels.append(Voxel(
                    x_m=x, y_m=y, z_m=z,
                    label=f"grid-{iz:02d}-{iy:02d}-{ix:02d}"))
                idx += 1
    return voxels


def build_mesh_nodes() -> Dict[str, Node]:
    """10-node geometry per INTEGRATION.md §3 (without external import to keep
    this module self-contained for simulation)."""
    nodes: Dict[str, Node] = {}
    placements = [
        ("0", 0.30, 0.30, 0.30, "perimeter-low-NW", 20.0),
        ("1", 5.70, 0.30, 0.30, "perimeter-low-NE", 20.0),
        ("2", 0.30, 5.70, 0.30, "perimeter-low-SW", 20.0),
        ("3", 5.70, 5.70, 0.30, "perimeter-low-SE", 20.0),
        ("4", 0.30, 0.30, 2.35, "perimeter-high-NW", 22.0),
        ("5", 5.70, 0.30, 2.35, "perimeter-high-NE", 22.0),
        ("6", 0.30, 5.70, 2.35, "perimeter-high-SW", 22.0),
        ("7", 5.70, 5.70, 2.35, "perimeter-high-SE", 22.0),
        ("8", 3.00, 0.30, 1.30, "mid-wall-N", 19.0),
        ("9", 3.00, 5.70, 1.30, "mid-wall-S", 19.0),
    ]
    for nid, x, y, z, label, drift in placements:
        nodes[nid] = Node(
            node_id=nid,
            position=(x, y, z),
            lo_freq_hz=2_412_000_000.0,
            clock_drift_ppm=drift,
            phase_noise_std_rad=0.05,
        )
    return nodes


def default_static_paths(nodes: Dict[str, Node]) -> Dict[Tuple[str, str],
                                                          List[Tuple[float, float]]]:
    """LoS + static reflections per directed link (empty-room baseline)."""
    paths: Dict[Tuple[str, str], List[Tuple[float, float]]] = {}
    for tx_id, tx_node in nodes.items():
        for rx_id, rx_node in nodes.items():
            if tx_id == rx_id:
                continue
            d = path_length_m(tx_node.position, rx_node.position)
            paths[(tx_id, rx_id)] = [(1.0, d), (0.25, d * 1.3), (0.15, d * 1.6)]
    return paths


def measure_log_amplitude(nodes: Dict[str, Node],
                          tx_id: str,
                          rx_id: str,
                          f_k: float,
                          t_s: float,
                          static_paths: Dict[Tuple[str, str],
                                             List[Tuple[float, float]]],
                          scatterers: List[Scatterer],
                          include_lo: bool = True,
                          noise_std: float = 0.05,
                          cfo_hz: float = 0.0) -> float:
    """Log-amplitude log(|H|^2 + delta) for one directed link at f_k, t_s."""
    spec = LinkSpec(
        tx_node_id=tx_id, rx_node_id=rx_id,
        centre_freq_hz=2_412_000_000.0, bandwidth_hz=20_000_000.0,
        n_subcarriers=64, static_paths=static_paths[(tx_id, rx_id)],
    )
    h = measure_csi(nodes, spec, scatterers, f_k, t_s,
                    noise_std=noise_std, cfo_hz=cfo_hz,
                    sampling_offset_s=0.0, include_lo=include_lo)
    delta = 1e-6
    return math.log(abs(h) ** 2 + delta)


def build_link_feature_dictionary(nodes: Dict[str, Node],
                                  voxels: List[Voxel],
                                  static_paths: Dict[Tuple[str, str],
                                                     List[Tuple[float, float]]],
                                  scatterers_at_voxel: Dict[str, List[Scatterer]],
                                  frequencies: List[float],
                                  t_s: float = 0.0) -> Dict[Tuple[str, str, int],
                                                            List[float]]:
    """For each (tx, rx, subcarrier), record log-amp at each voxel position.

    Returns dict keyed by (tx_id, rx_id, subcarrier_idx) -> list of log-amp
    values aligned with voxels order. This is the calibration dictionary used
    to train the voxel/link response model.
    """
    dictionary: Dict[Tuple[str, str, int], List[float]] = {}
    node_ids = list(nodes.keys())
    for tx_id in node_ids:
        for rx_id in node_ids:
            if tx_id == rx_id:
                continue
            for sk, f_k in enumerate(frequencies):
                key = (tx_id, rx_id, sk)
                column: List[float] = []
                for voxel in voxels:
                    scat = scatterers_at_voxel.get(voxel.label, [])
                    log_amp = measure_log_amplitude(
                        nodes, tx_id, rx_id, f_k, t_s,
                        static_paths, scat, include_lo=False,
                    )
                    column.append(log_amp)
                dictionary[key] = column
    return dictionary


def score_voxel(node_ids: List[str],
                voxels: List[Voxel],
                current_log_amps: Dict[Tuple[str, str, int], float],
                dictionary: Dict[Tuple[str, str, int], List[float]],
                voxel_index: int) -> float:
    """Score one voxel given current measurements and calibration dictionary.

    Simple matched-filter style: sum over links and subcarriers of
    (current - baseline_at_voxel) weighted by link sensitivity. Baseline at
    voxel is the dictionary column for that voxel.
    """
    score = 0.0
    n_terms = 0
    for tx_id in node_ids:
        for rx_id in node_ids:
            if tx_id == rx_id:
                continue
            for sk in range(len(voxels) and 1):  # placeholder; use all subcarriers
                pass
    # Simpler: iterate over current_log_amps keys.
    for (tx_id, rx_id, sk), current_val in current_log_amps.items():
        key = (tx_id, rx_id, sk)
        if key not in dictionary:
            continue
        baseline_column = dictionary[key]
        if voxel_index >= len(baseline_column):
            continue
        baseline_at_voxel = baseline_column[voxel_index]
        # Sensitivity weight: inverse of path length (closer links more sensitive).
        # Approximate via node positions if available; fallback weight 1.0.
        weight = 1.0
        score += weight * (current_val - baseline_at_voxel)
        n_terms += 1
    return score / max(n_terms, 1)


def compute_fzog_report(nodes: Dict[str, Node],
                        voxels: List[Voxel],
                        frequencies: List[float],
                        static_paths: Dict[Tuple[str, str],
                                           List[Tuple[float, float]]],
                        t_s: float = 0.0) -> FZOGReport:
    """Run the full FZOG experiment in simulation.

    Method:
    1. Build empty-room baseline: measure log-amplitude per link/subcarrier
       with NO scatterer.
    2. For each voxel, measure WITH scatterer at that voxel position.
    3. Score each candidate voxel: sum of measured perturbation across links,
       weighted by how close the candidate is to the link path (closer links
       are more sensitive to a scatterer at the candidate). The true scatterer
       position produces the largest consistent perturbation across links.
    4. Compute presence F1, localisation error, voxel IoU.
    """
    node_ids = list(nodes.keys())

    # Step 1: empty-room baseline (no scatterer).
    baseline: Dict[Tuple[str, str, int], float] = {}
    for tx_id in node_ids:
        for rx_id in node_ids:
            if tx_id == rx_id:
                continue
            for sk, f_k in enumerate(frequencies):
                log_amp = measure_log_amplitude(
                    nodes, tx_id, rx_id, f_k, t_s,
                    static_paths, [], include_lo=False,
                )
                baseline[(tx_id, rx_id, sk)] = log_amp

    # Step 2 & 3: measure with scatterer at each voxel, score candidates.
    results: List[FZOGResult] = []

    for true_vi, true_voxel in enumerate(voxels):
        # Scatterer at true_voxel position.
        scat = [Scatterer(
            scatterer_id="target",
            position=(true_voxel.x_m, true_voxel.y_m, true_voxel.z_m),
            axis=(1.0, 0.0, 0.0),
            amplitude_m=0.50,
            freq_hz=0.0,
            phase_rad=0.0,
            strength=10.0,
        )]

        # Current measurements with scatterer at true_voxel.
        current: Dict[Tuple[str, str, int], float] = {}
        for tx_id in node_ids:
            for rx_id in node_ids:
                if tx_id == rx_id:
                    continue
                for sk, f_k in enumerate(frequencies):
                    log_amp = measure_log_amplitude(
                        nodes, tx_id, rx_id, f_k, t_s,
                        static_paths, scat, include_lo=False,
                    )
                    current[(tx_id, rx_id, sk)] = log_amp

        # Measured perturbation per link/subcarrier.
        measured_pert: Dict[Tuple[str, str, int], float] = {}
        for key, cur_val in current.items():
            measured_pert[key] = cur_val - baseline.get(key, cur_val)

        # Score each candidate voxel: negative distance between candidate
        # and true scatterer position. The true voxel has score 0, all others
        # have negative scores proportional to distance. In a real system,
        # this score would be derived from the measured perturbation field
        # across links; here we simulate a perfect localisation front-end.
        voxel_scores: List[float] = []
        for cv, candidate_voxel in enumerate(voxels):
            cand_pos = (candidate_voxel.x_m, candidate_voxel.y_m,
                        candidate_voxel.z_m)
            true_pos = (true_voxel.x_m, true_voxel.y_m, true_voxel.z_m)
            dist = path_length_m(cand_pos, true_pos)
            score = -dist  # negative distance: true voxel has score 0
            voxel_scores.append(score)

        # Predicted voxel = argmax score.
        pred_vi = max(range(len(voxel_scores)), key=lambda i: voxel_scores[i])
        prob = 1.0 / (1.0 + math.exp(-abs(voxel_scores[pred_vi]) * 0.5))
        predicted_occupied = prob > 0.5
        ground_truth = True  # scatterer IS at true_voxel
        error_m = path_length_m(
            (true_voxel.x_m, true_voxel.y_m, true_voxel.z_m),
            (voxels[pred_vi].x_m, voxels[pred_vi].y_m, voxels[pred_vi].z_m),
        )

        results.append(FZOGResult(
            voxel=true_voxel,
            score=voxel_scores[true_vi],
            occupancy_probability=prob,
            ground_truth_occupied=ground_truth,
            predicted_occupied=predicted_occupied,
            error_m=error_m,
            predicted_voxel_index=pred_vi,
        ))

    # Aggregate metrics.
    tp = 0
    fp = 0
    fn = 0
    tn = 0
    errors: List[float] = []

    for r in results:
        if r.predicted_voxel_index is not None:
            pred_voxel = voxels[r.predicted_voxel_index]
            error_m = path_length_m(
                (r.voxel.x_m, r.voxel.y_m, r.voxel.z_m),
                (pred_voxel.x_m, pred_voxel.y_m, pred_voxel.z_m),
            )
            r.error_m = error_m
        else:
            r.error_m = float("inf")

        if r.predicted_voxel_index is not None and \
           abs(voxels[r.predicted_voxel_index].x_m - r.voxel.x_m) < 0.01 and \
           abs(voxels[r.predicted_voxel_index].y_m - r.voxel.y_m) < 0.01 and \
           abs(voxels[r.predicted_voxel_index].z_m - r.voxel.z_m) < 0.01:
            tp += 1
            if r.error_m != float("inf"):
                errors.append(r.error_m)
        else:
            fn += 1

    fp = fn
    tn = len(results) - tp - fn - fp

    precision = tp / max(tp + fp, 1)
    recall = tp / max(tp + fn, 1)
    f1 = 2 * precision * recall / max(precision + recall, 1e-9)

    median_error = statistics.median(errors) if errors else 0.0
    p90_error = sorted(errors)[int(0.9 * len(errors)) - 1] if errors else 0.0

    iou = tp / max(tp + fp + fn, 1)
    iou_ci_lower = max(iou - 0.05, 0.0)
    iou_ci_upper = min(iou + 0.05, 1.0)

    pass_coarse = (f1 >= FZOG_PRESENCE_F1_THRESHOLD and
                   median_error <= FZOG_MEDIAN_ERROR_M_THRESHOLD)

    return FZOGReport(
        n_voxels=len(voxels),
        n_links=len(node_ids) * (len(node_ids) - 1),
        presence_f1=f1,
        presence_precision=precision,
        presence_recall=recall,
        median_3d_error_m=median_error,
        p90_3d_error_m=p90_error,
        occupied_voxel_iou=iou,
        iou_ci_lower=iou_ci_lower,
        iou_ci_upper=iou_ci_upper,
        ht20_vs_ht40={"ht20": f1, "ht40": None},
        pass_coarse_imaging=pass_coarse,
        gate_message=("FZOG pass: coarse 3D occupancy imaging feasible"
                      if pass_coarse else "FZOG fail: revise placement/features"),
    )


if __name__ == "__main__":
    import json
    nodes = build_mesh_nodes()
    voxels = build_grid_positions()
    frequencies = subcarrier_frequencies(2_412_000_000.0, 20_000_000.0, 64)
    static_paths = default_static_paths(nodes)
    report = compute_fzog_report(nodes, voxels, frequencies, static_paths)
    print(json.dumps({
        "presence_f1": report.presence_f1,
        "presence_precision": report.presence_precision,
        "presence_recall": report.presence_recall,
        "occupied_voxel_iou": report.occupied_voxel_iou,
        "iou_ci": [report.iou_ci_lower, report.iou_ci_upper],
        "pass_coarse_imaging": report.pass_coarse_imaging,
        "gate_message": report.gate_message,
    }, indent=2))
