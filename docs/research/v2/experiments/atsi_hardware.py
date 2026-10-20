"""ATSI — Ambient-Traffic Shadow Imaging (hardware version).

INTEGRATION.md §5. Receive-only occupancy and coarse spatial inference from
ordinary 2.4 GHz traffic. Sensing nodes are receive-only; an external AP/router
is the traffic source. Runs Days 8–10, regardless of M2 outcome.

Pure Python analysis; hardware collection via hardware.CsiCollector over UDP
port 9100 when nodes are present. Runs in simulation today (csi_sim), on real
hardware later.

Critical constraint from the 10-node protocol: a complex ratio between two
receivers is exploratory secondary only. Common source phase may cancel in a
ratio, but receiver-specific phase terms remain unless a separately validated
reference is available. If M2 is amber/red, exclude phase ratios from claimed
results.

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
# ATSI pass/fail thresholds (campaign gates, INTEGRATION.md §5)
# ---------------------------------------------------------------------------

ATSI_ZONE_F1_THRESHOLD = 0.80
ATSI_PACKET_COVERAGE_THRESHOLD = 0.70


@dataclass
class ZoneResult:
    """Per-zone detection result."""
    zone_id: str
    ground_truth_occupied: bool
    predicted_occupied: bool
    occupancy_probability: float
    packet_count: int


@dataclass
class ATSIExperimentReport:
    n_nodes: int
    n_zones: int
    n_packets_total: int
    packet_coverage: float          # fraction of windows with usable packets
    zone_results: List[ZoneResult]
    presence_f1: float
    presence_precision: float
    presence_recall: float
    ambient_packet_rate_hz: float
    min_usable_rate_hz: float
    pass_receive_only_utility: bool
    gate_message: str


# ---------------------------------------------------------------------------
# Zone definitions (match NODE_PLACEMENTS in node_config.py)
# ---------------------------------------------------------------------------

ZONES: List[dict] = [
    {"zone_id": "zone-NW", "x_m": (0.0, 2.0), "y_m": (0.0, 2.0), "nodes": ["0", "4", "8"]},
    {"zone_id": "zone-NE", "x_m": (4.0, 6.0), "y_m": (0.0, 2.0), "nodes": ["1", "5", "8"]},
    {"zone_id": "zone-SW", "x_m": (0.0, 2.0), "y_m": (4.0, 6.0), "nodes": ["2", "6", "9"]},
    {"zone_id": "zone-SE", "x_m": (4.0, 6.0), "y_m": (4.0, 6.0), "nodes": ["3", "7", "9"]},
    {"zone_id": "zone-centre", "x_m": (2.0, 4.0), "y_m": (2.0, 4.0), "nodes": ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"]},
]


def build_mesh_nodes() -> Dict[str, Node]:
    """10-node geometry per INTEGRATION.md §3."""
    nodes: Dict[str, Node] = {}
    placements = [
        ("0", 0.30, 0.30, 0.30, 20.0),
        ("1", 5.70, 0.30, 0.30, 20.0),
        ("2", 0.30, 5.70, 0.30, 20.0),
        ("3", 5.70, 5.70, 0.30, 20.0),
        ("4", 0.30, 0.30, 2.35, 22.0),
        ("5", 5.70, 0.30, 2.35, 22.0),
        ("6", 0.30, 5.70, 2.35, 22.0),
        ("7", 5.70, 5.70, 2.35, 22.0),
        ("8", 3.00, 0.30, 1.30, 19.0),
        ("9", 3.00, 5.70, 1.30, 19.0),
    ]
    for nid, x, y, z, drift in placements:
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


def measure_node_local_power(nodes: Dict[str, Node],
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
    """Log-amplitude log(|H|^2 + delta) for one directed link at f_k, t_s.

    In ATSI the transmitter is an external AP; here we simulate by having one
    of the mesh nodes act as the ambient illuminator (receive-only at the
    other nodes).
    """
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


def classify_zone_occupancy(node_log_amps: Dict[str, List[float]],
                            zone_node_ids: List[str],
                            baseline_log_amps: Dict[str, List[float]],
                            threshold_db: float = 2.0) -> Tuple[bool, float]:
    """Classify one zone as occupied/not from node-local log-amplitude changes.

    Returns (predicted_occupied, occupancy_probability).
    """
    deltas: List[float] = []
    for nid in zone_node_ids:
        if nid not in node_log_amps or nid not in baseline_log_amps:
            continue
        cur = statistics.mean(node_log_amps[nid])
        base = statistics.mean(baseline_log_amps[nid])
        deltas.append(cur - base)

    if not deltas:
        return False, 0.0

    mean_delta = statistics.mean(deltas)
    # Simple logistic mapping from mean dB change to probability.
    prob = 1.0 / (1.0 + math.exp(-(mean_delta - threshold_db) / 1.0))
    predicted = mean_delta > threshold_db
    return predicted, prob


def run_atsi_simulation_demo(m2_worst_band: str = "green") -> ATSIExperimentReport:
    """Self-contained simulation demo of ATSI (no hardware)."""
    random.seed(20261019)

    nodes = build_mesh_nodes()
    node_ids = list(nodes.keys())
    static_paths = {}
    for tx_id, tx_node in nodes.items():
        for rx_id, rx_node in nodes.items():
            if tx_id == rx_id:
                continue
            d = path_length_m(tx_node.position, rx_node.position)
            static_paths[(tx_id, rx_id)] = [(1.0, d), (0.25, d * 1.3)]

    frequencies = subcarrier_frequencies(2_412_000_000.0, 20_000_000.0, 64)
    f_k = frequencies[32]

    # One node acts as ambient illuminator (TX); others receive.
    illuminator = "0"
    receivers = [nid for nid in node_ids if nid != illuminator]

    # Empty-room baseline: 30 measurements per receiver.
    baseline_log_amps: Dict[str, List[float]] = {nid: [] for nid in receivers}
    scatterers_empty: List[Scatterer] = []
    t_s = 0.0
    for _ in range(30):
        for nid in receivers:
            la = measure_node_local_power(
                nodes, illuminator, nid, f_k, t_s,
                static_paths, scatterers_empty, include_lo=False,
            )
            baseline_log_amps[nid].append(la)
        t_s += 0.01

    # Occupied trials: person at zone-centre positions.
    zone_positions = {
        "zone-NW": (1.0, 1.0, 1.2),
        "zone-NE": (5.0, 1.0, 1.2),
        "zone-SW": (1.0, 5.0, 1.2),
        "zone-SE": (5.0, 5.0, 1.2),
        "zone-centre": (3.0, 3.0, 1.2),
    }

    zone_results: List[ZoneResult] = []
    t_s = 1.0  # start after baseline
    for zone in ZONES:
        zone_id = zone["zone_id"]
        x_range = zone["x_m"]
        y_range = zone["y_m"]
        # pick centre of zone rectangle
        pos = ((x_range[0] + x_range[1]) / 2, (y_range[0] + y_range[1]) / 2, 1.2)
        scatterers = [
            Scatterer(
                scatterer_id="person",
                position=pos,
                axis=(1.0, 0.0, 0.0),
                amplitude_m=0.50,
                freq_hz=0.25,
                phase_rad=0.0,
                strength=20.0,
            )
        ]

        # Collect measurements during occupancy.
        current_log_amps: Dict[str, List[float]] = {nid: [] for nid in receivers}
        packet_count = 0
        for _ in range(20):
            for nid in receivers:
                la = measure_node_local_power(
                    nodes, illuminator, nid, f_k, t_s,
                    static_paths, scatterers, include_lo=False,
                )
                current_log_amps[nid].append(la)
                packet_count += 1
            t_s += 0.01

        predicted, prob = classify_zone_occupancy(
            current_log_amps, zone["nodes"], baseline_log_amps)

        zone_results.append(ZoneResult(
            zone_id=zone_id,
            ground_truth_occupied=True,
            predicted_occupied=predicted,
            occupancy_probability=prob,
            packet_count=packet_count,
        ))

    # Aggregate.
    tp = sum(1 for r in zone_results if r.predicted_occupied and r.ground_truth_occupied)
    fp = sum(1 for r in zone_results if r.predicted_occupied and not r.ground_truth_occupied)
    fn = sum(1 for r in zone_results if not r.predicted_occupied and r.ground_truth_occupied)
    tn = sum(1 for r in zone_results if not r.predicted_occupied and not r.ground_truth_occupied)

    precision = tp / max(tp + fp, 1)
    recall = tp / max(tp + fn, 1)
    f1 = 2 * precision * recall / max(precision + recall, 1e-9)

    n_packets_total = sum(r.packet_count for r in zone_results)
    n_windows = len(zone_results)
    windows_with_packets = sum(1 for r in zone_results if r.packet_count > 0)
    coverage = windows_with_packets / max(n_windows, 1)

    ambient_rate_hz = n_packets_total / 1.0  # rough: 1 second of collection
    min_usable_rate_hz = ambient_rate_hz * coverage

    pass_receive = (f1 >= ATSI_ZONE_F1_THRESHOLD and
                    coverage >= ATSI_PACKET_COVERAGE_THRESHOLD)

    if pass_receive:
        gate_message = ("ATSI pass: receive-only ambient occupancy feasible "
                        "within tested traffic conditions.")
    elif coverage < ATSI_PACKET_COVERAGE_THRESHOLD:
        gate_message = ("ATSI no-go: ambient packet coverage below 70%. "
                        "Retain as opportunistic fallback; use active FZOG.")
    else:
        gate_message = ("ATSI partial: coverage adequate but zone F1 below 0.80.")

    return ATSIExperimentReport(
        n_nodes=len(nodes),
        n_zones=len(ZONES),
        n_packets_total=n_packets_total,
        packet_coverage=coverage,
        zone_results=zone_results,
        presence_f1=f1,
        presence_precision=precision,
        presence_recall=recall,
        ambient_packet_rate_hz=ambient_rate_hz,
        min_usable_rate_hz=min_usable_rate_hz,
        pass_receive_only_utility=pass_receive,
        gate_message=gate_message,
    )


if __name__ == "__main__":
    import json
    report = run_atsi_simulation_demo()
    print(json.dumps({
        "presence_f1": report.presence_f1,
        "presence_precision": report.presence_precision,
        "presence_recall": report.presence_recall,
        "packet_coverage": report.packet_coverage,
        "ambient_packet_rate_hz": report.ambient_packet_rate_hz,
        "min_usable_rate_hz": report.min_usable_rate_hz,
        "pass_receive_only_utility": report.pass_receive_only_utility,
        "gate_message": report.gate_message,
        "zones": [
            {
                "zone_id": r.zone_id,
                "predicted": r.predicted_occupied,
                "probability": r.occupancy_probability,
                "packet_count": r.packet_count,
            }
            for r in report.zone_results
        ],
    }, indent=2))
