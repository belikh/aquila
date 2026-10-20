"""Test protocol runner — orchestrates the 14-day ESP32-S3 campaign.

INTEGRATION.md §4. Runs experiments in sequence with pass/fail propagation:

  Days 1–3:  FZOG amplitude baseline (HT20 → HT40)
  Days 4–7:  M2 characterisation → BCPT (gate on M2 band)
  Days 8–10: ATSI receive-only ambient
  Days 11–14: Final synthesis + held-out evaluation

Each experiment writes to the evidence ledger with witness grades.
Runs in simulation today; hardware-ready when nodes are present.

Australian English spelling throughout.
"""

from __future__ import annotations

import json
import sys
import time
from dataclasses import dataclass, field
from typing import Dict, List, Optional

from m2_characterization import run_m2_simulation_demo as run_m2_sim
from m2_two_node_hardware import run_two_node_measurement
from fzog import (build_grid_positions, build_mesh_nodes, compute_fzog_report,
                  default_static_paths)
from bcpt_hardware import run_bcpt_simulation_demo as run_bcpt_sim
from atsi_hardware import run_atsi_simulation_demo as run_atsi_sim
from csi_sim import subcarrier_frequencies
from evidence_ledger import (
    record_fzog_result, record_m2_result, record_atsi_result,
    append_ledger_entry, summarise_ledger,
)


@dataclass
class PhaseResult:
    """Result of one campaign phase."""
    phase: str
    day: int
    passed: bool
    witness_summary: dict
    gate_message: str
    next_phase: Optional[str] = None


@dataclass
class CampaignReport:
    """Complete 14-day campaign report."""
    session_id: str
    started_utc: str
    completed_utc: Optional[str]
    phases: List[PhaseResult] = field(default_factory=list)
    overall_pass: bool = False
    final_ledger_summary: Optional[dict] = None


def run_phase_fzog(session_id: str,
                   m2_band: str = "green") -> PhaseResult:
    """Day 1–3: FZOG amplitude baseline."""
    from fzog import build_mesh_nodes, build_grid_positions, default_static_paths
    nodes = build_mesh_nodes()
    voxels = build_grid_positions()
    frequencies = subcarrier_frequencies(2_412_000_000.0, 20_000_000.0, 64)
    static_paths = default_static_paths(nodes)
    report = compute_fzog_report(nodes, voxels, frequencies, static_paths)
    entries = record_fzog_result(
        session_id, "fzog",
        f1=report.presence_f1,
        f1_threshold=0.90,
        median_error_m=report.median_3d_error_m,
        error_threshold_m=1.0,
        iou=report.occupied_voxel_iou,
        m2_band=m2_band,
    )
    for e in entries:
        append_ledger_entry(e, session_id)

    passed = report.pass_coarse_imaging
    return PhaseResult(
        phase="FZOG",
        day=3,
        passed=passed,
        witness_summary={
            "presence_f1": report.presence_f1,
            "median_error_m": report.median_3d_error_m,
            "iou": report.occupied_voxel_iou,
        },
        gate_message=report.gate_message,
        next_phase="M2" if passed else "STOP",
    )


def run_phase_m2(session_id: str,
                 hardware_duration_s: float = 120.0) -> PhaseResult:
    """Day 4: M2 characterisation (gate for BCPT).

    The campaign measures the *hardware* when live nodes are streaming, and
    falls back to the simulation only when they are not. The hardware measure
    is the one that decides the gate: the simulation's verdict depends on the
    modelled channel, and a simulated green says nothing about whether the
    ESP32-S3's phase is coherent across two nodes.
    """
    hardware = _try_hardware_m2(hardware_duration_s)
    if hardware is not None:
        worst_band = hardware.worst_band
        bcpt_cleared = hardware.bcpt_cleared
        source = "hardware"
        summary: dict = {
            "source": source,
            "worst_band": worst_band,
            "bcpt_cleared": bcpt_cleared,
            "node_ids": list(hardware.node_ids),
            "frames_per_node": hardware.frames_per_node,
            "results": [
                {
                    "pair": [r.node_a, r.node_b],
                    "p95_rad": r.p95_rad,
                    "band": r.band,
                    "control_p95_rad": r.control_p95_rad,
                    "n_pairs": r.n_pairs,
                }
                for r in hardware.results
            ],
        }
        gate_message = (
            f"M2 {worst_band} on hardware (nodes "
            f"{list(hardware.node_ids)}): "
            + (
                "BCPT cleared over the air"
                if bcpt_cleared
                else "cross-node coherent BCPT hardware-gated"
            )
        )
        entries = record_m2_result(session_id, "m2", summary)
        for e in entries:
            append_ledger_entry(e, session_id)
        return PhaseResult(
            phase="M2",
            day=4,
            passed=bcpt_cleared,
            witness_summary=summary,
            gate_message=gate_message,
            next_phase="BCPT" if bcpt_cleared else "BCPT_GATED",
        )

    report = run_m2_sim()
    entries = record_m2_result(session_id, "m2", {
        "source": "simulation",
        "worst_band": report.worst_band,
        "bcpt_cleared": report.bcpt_cleared,
        "results": [
            {
                "pair": list(r.pair),
                "p95_rad": r.p95_rad,
                "band": r.band,
            }
            for r in report.results
        ],
    })
    for e in entries:
        append_ledger_entry(e, session_id)

    passed = report.bcpt_cleared
    return PhaseResult(
        phase="M2",
        day=4,
        passed=passed,
        witness_summary={
            "source": "simulation",
            "worst_band": report.worst_band,
            "bcpt_cleared": report.bcpt_cleared,
        },
        gate_message=report.gate_message,
        next_phase="BCPT" if passed else "BCPT_GATED",
    )


def _try_hardware_m2(duration_s: float):
    """Measure M2 on the live nodes, or return None when none are streaming.

    A node that has not been updated to report its transmitter's address
    cannot be paired, so the measurement is only attempted when at least two
    nodes report one.
    """
    try:
        report = run_two_node_measurement(duration_s=duration_s)
    except OSError:
        return None
    if len(report.node_ids) < 2:
        return None
    reporting = [
        node_id
        for node_id in report.node_ids
        if report.transmitters.get(node_id)
    ]
    if len(reporting) < 2:
        return None
    return report


def run_phase_bcpt(session_id: str,
                   m2_worst_band: str = "green") -> PhaseResult:
    """Days 5–7: BCPT closure-phase tomography (only if M2 not red)."""
    report = run_bcpt_sim(m2_worst_band=m2_worst_band)

    # Record BCPT results as ledger entries.
    from evidence_ledger import LedgerEntry
    entries: list = []
    for loop in report.loop_results:
        sep_passed = loop.pass_separation
        rep_passed = loop.pass_repeatability
        witness = "Measured" if (rep_passed and sep_passed) else "Claimed"
        entries.append(LedgerEntry(
            session_id=session_id,
            created_utc=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            experiment_phase="bcpt",
            claim_id="CLM-bcpt-loop",
            metric_name=f"closure_sd_{loop.loop_name}",
            metric_value=loop.sd_rad,
            metric_threshold=0.25,
            witness=witness,
            passed=rep_passed,
            hardware_gated=(m2_worst_band == "red"),
            data_gated=False,
            notes=f"Loop {loop.loop_name}, separation factor={loop.target_separation_factor:.2f}",
        ))
        append_ledger_entry(entries[-1], session_id)

    # Overall BCPT localisation claim.
    loc_passed = report.pass_coarse_imaging
    loc_witness = "Gated" if (loc_passed and m2_worst_band != "green") else (
        "Measured" if loc_passed else "Claimed")
    entries.append(LedgerEntry(
        session_id=session_id,
        created_utc=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        experiment_phase="bcpt",
        claim_id="CLM-bcpt-localisation",
        metric_name="median_3d_error_m",
        metric_value=report.median_3d_error_m,
        metric_threshold=1.0,
        witness=loc_witness,
        passed=loc_passed,
        hardware_gated=(m2_worst_band == "red"),
        data_gated=False,
        notes="BCPT median 3D localisation error",
    ))
    append_ledger_entry(entries[-1], session_id)

    passed = report.pass_coarse_imaging
    return PhaseResult(
        phase="BCPT",
        day=7,
        passed=passed,
        witness_summary={
            "median_error_m": report.median_3d_error_m,
            "bcpt_cleared": report.bcpt_cleared,
        },
        gate_message=report.gate_message,
        next_phase="ATSI",
    )


def run_phase_atsi(session_id: str,
                   m2_band: str = "green") -> PhaseResult:
    """Days 8–10: ATSI receive-only ambient."""
    report = run_atsi_sim(m2_worst_band=m2_band)
    entries = record_atsi_result(
        session_id, "atsi",
        f1=report.presence_f1,
        f1_threshold=0.80,
        coverage=report.packet_coverage,
        coverage_threshold=0.70,
        m2_band=m2_band,
    )
    for e in entries:
        append_ledger_entry(e, session_id)

    passed = report.pass_receive_only_utility
    return PhaseResult(
        phase="ATSI",
        day=10,
        passed=passed,
        witness_summary={
            "presence_f1": report.presence_f1,
            "packet_coverage": report.packet_coverage,
        },
        gate_message=report.gate_message,
        next_phase="SYNTHESIS",
    )


def run_phase_synthesis(session_id: str,
                        phases: List[PhaseResult]) -> PhaseResult:
    """Days 11–14: Final synthesis — held-out session, furniture rearrangement."""
    # Summarise all ledger entries.
    summary = summarise_ledger(session_id)

    # Overall pass: at least FZOG passes (performance floor).
    fzog_passed = any(p.phase == "FZOG" and p.passed for p in phases)
    atsi_passed = any(p.phase == "ATSI" and p.passed for p in phases)
    bcpt_passed = any(p.phase == "BCPT" and p.passed for p in phases)

    # At least one of FZOG or ATSI must pass for a usable system.
    overall_pass = fzog_passed or atsi_passed

    gate_parts = []
    if fzog_passed:
        gate_parts.append("FZOG: pass")
    else:
        gate_parts.append("FZOG: fail")
    if bcpt_passed:
        gate_parts.append("BCPT: pass")
    else:
        gate_parts.append("BCPT: no-go (M2 or closure insufficient)")
    if atsi_passed:
        gate_parts.append("ATSI: pass")
    else:
        gate_parts.append("ATSI: fail")

    gate_message = "; ".join(gate_parts)

    return PhaseResult(
        phase="SYNTHESIS",
        day=14,
        passed=overall_pass,
        witness_summary={
            "fzog_passed": fzog_passed,
            "bcpt_passed": bcpt_passed,
            "atsi_passed": atsi_passed,
            "ledger_entries": summary.get("entries", 0),
            "headline_claims_passed": summary.get("headline_claims_passed", 0),
        },
        gate_message=gate_message,
        next_phase=None,
    )


def run_campaign(session_id: Optional[str] = None) -> CampaignReport:
    """Run the full 14-day campaign in sequence with pass/fail propagation."""
    if session_id is None:
        session_id = f"campaign_{time.strftime('%Y%m%d_%H%M%S')}"

    started_utc = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    phases: List[PhaseResult] = []

    print(f"=== Aquila 10-node campaign: {session_id} ===")
    print(f"Started: {started_utc}\n")

    # ---- Phase 1: FZOG ----
    print("--- Phase 1: FZOG amplitude baseline (Days 1-3) ---")
    p1 = run_phase_fzog(session_id)
    phases.append(p1)
    print(f"Result: {'PASS' if p1.passed else 'FAIL'} — {p1.gate_message}\n")

    if not p1.passed:
        print("FZOG failed. Revise placement/features before phase processing.")
        # Still continue to M2/ATSI for data, but flag.
    
    # ---- Phase 2: M2 characterisation ----
    print("--- Phase 2: M2 characterisation (Day 4) ---")
    p2 = run_phase_m2(session_id)
    phases.append(p2)
    print(f"Result: {'PASS' if p2.passed else 'FAIL'} — {p2.gate_message}\n")

    m2_worst_band = p2.witness_summary.get("worst_band", "unknown")

    # ---- Phase 3: BCPT (only if M2 not red) ----
    print("--- Phase 3: BCPT closure-phase tomography (Days 5-7) ---")
    if m2_worst_band == "red":
        p3 = PhaseResult(
            phase="BCPT", day=7, passed=False,
            witness_summary={"reason": "M2 red"},
            gate_message="BCPT skipped: M2 red band — hardware-gated.",
            next_phase="ATSI",
        )
        phases.append(p3)
        print(f"Result: SKIPPED — {p3.gate_message}\n")
    else:
        p3 = run_phase_bcpt(session_id, m2_worst_band=m2_worst_band)
        phases.append(p3)
        print(f"Result: {'PASS' if p3.passed else 'FAIL'} — {p3.gate_message}\n")

    # ---- Phase 4: ATSI ----
    print("--- Phase 4: ATSI receive-only ambient (Days 8-10) ---")
    p4 = run_phase_atsi(session_id, m2_band=m2_worst_band)
    phases.append(p4)
    print(f"Result: {'PASS' if p4.passed else 'FAIL'} — {p4.gate_message}\n")

    # ---- Phase 5: Synthesis ----
    print("--- Phase 5: Final synthesis (Days 11-14) ---")
    p5 = run_phase_synthesis(session_id, phases)
    phases.append(p5)
    print(f"Result: {'PASS' if p5.passed else 'FAIL'} — {p5.gate_message}\n")

    completed_utc = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    ledger_summary = summarise_ledger(session_id)

    return CampaignReport(
        session_id=session_id,
        started_utc=started_utc,
        completed_utc=completed_utc,
        phases=phases,
        overall_pass=p5.passed,
        final_ledger_summary=ledger_summary,
    )


def print_campaign_report(report: CampaignReport) -> None:
    """Print a human-readable campaign report."""
    print("=" * 60)
    print("AQUILA 10-NODE CAMPAIGN REPORT")
    print("=" * 60)
    print(f"Session: {report.session_id}")
    print(f"Started: {report.started_utc}")
    print(f"Completed: {report.completed_utc}")
    print(f"Overall pass: {'YES' if report.overall_pass else 'NO'}")
    print()
    for p in report.phases:
        status = "PASS" if p.passed else "FAIL/SKIP"
        print(f"  {p.phase:12s} (day {p.day}): {status:10s} — {p.gate_message}")
    print()
    if report.final_ledger_summary:
        s = report.final_ledger_summary
        print(f"Ledger entries: {s.get('entries', 0)}")
        print(f"Grades: {s.get('grades', {})}")
        print(f"Headline claims passed: {s.get('headline_claims_passed', 0)}")
    print("=" * 60)


if __name__ == "__main__":
    report = run_campaign()
    print_campaign_report(report)
