"""Evidence ledger — witness-graded output for all experiment results.

INTEGRATION.md §8. Every sensing claim carries exactly one witness grade:
- Gated: measured and passing a defined v1 bar; the only grade a headline
  claim may carry.
- Measured: computed from real, replayed or simulated input and recorded in
  the commit-keyed ledger.
- Claimed: asserted without a recorded measurement.
- Data-gated: blocked on missing input (fixture, labels, reference recording
  or paired samples); a blocked sub-grade of Claimed.
- Hardware-gated: blocked on missing hardware (reference device, belt,
  camera rig or the reference-scale node deployment); a blocked sub-grade of
  Claimed.

Australian English spelling throughout.
"""

from __future__ import annotations

import json
import os
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import List, Optional

LEDGER_DIR = Path("data/ledger")
LEDGER_DIR.mkdir(parents=True, exist_ok=True)

# ---------------------------------------------------------------------------
# Witness grades (strongest first)
# ---------------------------------------------------------------------------

WITNESS_GRADES = ["Gated", "Measured", "Claimed", "Data-gated", "Hardware-gated"]

HEADLINE_CLAIM_GRADES = ["Gated"]  # only Gated may carry a headline claim


@dataclass
class LedgerEntry:
    """One commit-keyed ledger row with witness grade."""
    session_id: str
    created_utc: str
    experiment_phase: str
    claim_id: str
    metric_name: str
    metric_value: float
    metric_threshold: Optional[float]
    witness: str
    passed: bool
    hardware_gated: bool = False
    data_gated: bool = False
    notes: Optional[str] = None

    def to_dict(self) -> dict:
        return {
            "session_id": self.session_id,
            "created_utc": self.created_utc,
            "experiment_phase": self.experiment_phase,
            "claim_id": self.claim_id,
            "metric_name": self.metric_name,
            "metric_value": self.metric_value,
            "metric_threshold": self.metric_threshold,
            "witness": self.witness,
            "passed": self.passed,
            "hardware_gated": self.hardware_gated,
            "data_gated": self.data_gated,
            "notes": self.notes,
        }


def assign_witness(passed: bool,
                   m2_band: str = "green",
                   data_available: bool = True,
                   hardware_available: bool = True,
                   is_headline: bool = False) -> str:
    """Assign the appropriate witness grade.

    Rules:
    - If not data_available -> Data-gated (blocked sub-grade of Claimed).
    - If not hardware_available -> Hardware-gated (blocked sub-grade of Claimed).
    - If passed and is_headline and m2_band != "red" -> Gated.
    - If passed and not is_headline -> Measured.
    - If not passed -> Claimed (asserted without meeting bar) — or
      Data-gated/Hardware-gated if blocked.
    """
    if not data_available:
        return "Data-gated"
    if not hardware_available:
        return "Hardware-gated"
    if passed and is_headline and m2_band != "red":
        return "Gated"
    if passed:
        return "Measured"
    # Not passed: Claimed unless blocked.
    if m2_band == "red" and hardware_available:
        return "Hardware-gated"
    return "Claimed"


def grade_experiment_result(passed: bool,
                            m2_band: str = "green",
                            is_headline: bool = False) -> str:
    """Convenience wrapper for assign_witness with defaults."""
    return assign_witness(
        passed=passed,
        m2_band=m2_band,
        data_available=True,
        hardware_available=True,
        is_headline=is_headline,
    )


def append_ledger_entry(entry: LedgerEntry,
                        session_id: str) -> None:
    """Append one ledger row to the session ledger JSONL."""
    LEDGER_DIR.mkdir(parents=True, exist_ok=True)
    path = LEDGER_DIR / f"{session_id}_ledger.jsonl"
    with open(path, "a") as f:
        f.write(json.dumps(entry.to_dict()) + "\n")


def summarise_ledger(session_id: str) -> dict:
    """Summarise all ledger entries for a session by witness grade."""
    LEDGER_DIR.mkdir(parents=True, exist_ok=True)
    path = LEDGER_DIR / f"{session_id}_ledger.jsonl"
    if not path.exists():
        return {"session_id": session_id, "entries": 0, "grades": {}}

    entries: List[dict] = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if line:
                entries.append(json.loads(line))

    grades: dict = {}
    for e in entries:
        g = e.get("witness", "unknown")
        grades[g] = grades.get(g, 0) + 1

    headline_passed = sum(
        1 for e in entries
        if e.get("witness") == "Gated" and e.get("passed")
    )

    return {
        "session_id": session_id,
        "entries": len(entries),
        "grades": grades,
        "headline_claims_passed": headline_passed,
        "entries_detail": entries,
    }


def record_fzog_result(session_id: str,
                       experiment_phase: str,
                       f1: float,
                       f1_threshold: float,
                       median_error_m: float,
                       error_threshold_m: float,
                       iou: float,
                       m2_band: str = "green") -> List[LedgerEntry]:
    """Record FZOG results as ledger entries with witness grades."""
    entries: List[LedgerEntry] = []

    # Presence F1 — headline claim if Gated.
    f1_passed = f1 >= f1_threshold
    f1_witness = grade_experiment_result(
        passed=f1_passed, m2_band=m2_band, is_headline=True)
    entries.append(LedgerEntry(
        session_id=session_id,
        created_utc=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        experiment_phase=experiment_phase,
        claim_id="CLM-presence-f1",
        metric_name="presence_f1",
        metric_value=f1,
        metric_threshold=f1_threshold,
        witness=f1_witness,
        passed=f1_passed,
        hardware_gated=(f1_witness == "Hardware-gated"),
        data_gated=(f1_witness == "Data-gated"),
        notes="FZOG amplitude-first 3D occupancy",
    ))

    # Median 3D error.
    err_passed = median_error_m <= error_threshold_m
    err_witness = grade_experiment_result(
        passed=err_passed, m2_band=m2_band, is_headline=False)
    entries.append(LedgerEntry(
        session_id=session_id,
        created_utc=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        experiment_phase=experiment_phase,
        claim_id="CLM-localisation-error",
        metric_name="median_3d_error_m",
        metric_value=median_error_m,
        metric_threshold=error_threshold_m,
        witness=err_witness,
        passed=err_passed,
        hardware_gated=(err_witness == "Hardware-gated"),
        data_gated=(err_witness == "Data-gated"),
        notes="FZOG median single-target 3D localisation error",
    ))

    # Occupied-voxel IoU.
    iou_passed = iou >= 0.5  # provisional threshold
    iou_witness = grade_experiment_result(
        passed=iou_passed, m2_band=m2_band, is_headline=False)
    entries.append(LedgerEntry(
        session_id=session_id,
        created_utc=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        experiment_phase=experiment_phase,
        claim_id="CLM-voxel-iou",
        metric_name="occupied_voxel_iou",
        metric_value=iou,
        metric_threshold=0.5,
        witness=iou_witness,
        passed=iou_passed,
        hardware_gated=(iou_witness == "Hardware-gated"),
        data_gated=(iou_witness == "Data-gated"),
        notes="FZOG occupied-voxel IoU",
    ))

    return entries


def record_m2_result(session_id: str,
                     experiment_phase: str,
                     m2_report: dict) -> List[LedgerEntry]:
    """Record M2 characterisation results as ledger entries."""
    entries: List[LedgerEntry] = []
    worst_band = m2_report.get("worst_band", "unknown")
    # Whether the residual came from the live nodes or from the model. The
    # witness grade cannot tell the two apart, and the distinction is the
    # whole point of the M2 question, so it is recorded explicitly.
    source = m2_report.get("source", "simulation")

    for result in m2_report.get("results", []):
        pair = result.get("pair", ["?", "?"])
        p95 = result.get("p95_rad", 0.0)
        band = result.get("band", "unknown")

        # M2 itself is a measurement, not a pass/fail claim.
        witness = "Measured"
        entries.append(LedgerEntry(
            session_id=session_id,
            created_utc=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            experiment_phase=experiment_phase,
            claim_id="CLM-m2-residual",
            metric_name=f"m2_p95_rad_{pair[0]}_{pair[1]}",
            metric_value=p95,
            metric_threshold=1.0,  # red band threshold
            witness=witness,
            passed=(band != "red"),
            hardware_gated=(band == "red"),
            data_gated=False,
            notes=(f"M2 cross-node phase residual, pair {pair[0]}->{pair[1]}, "
                   f"band={band}, source={source}"),
        ))

    # BCPT clearance claim (headline if proceeding to BCPT).
    bcpt_cleared = m2_report.get("bcpt_cleared", False)
    bcpt_witness = grade_experiment_result(
        passed=bcpt_cleared, m2_band=worst_band, is_headline=False)
    entries.append(LedgerEntry(
        session_id=session_id,
        created_utc=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        experiment_phase=experiment_phase,
        claim_id="CLM-bcpt-cleared",
        metric_name="bcpt_cleared",
        metric_value=1.0 if bcpt_cleared else 0.0,
        metric_threshold=1.0,
        witness=bcpt_witness,
        passed=bcpt_cleared,
        hardware_gated=(worst_band == "red"),
        data_gated=False,
        notes=f"BCPT clearance gate, M2 worst band={worst_band}",
    ))

    return entries


def record_atsi_result(session_id: str,
                       experiment_phase: str,
                       f1: float,
                       f1_threshold: float,
                       coverage: float,
                       coverage_threshold: float,
                       m2_band: str = "green") -> List[LedgerEntry]:
    """Record ATSI results as ledger entries."""
    entries: List[LedgerEntry] = []

    # Zone F1 — headline claim.
    f1_passed = f1 >= f1_threshold
    f1_witness = grade_experiment_result(
        passed=f1_passed, m2_band=m2_band, is_headline=True)
    entries.append(LedgerEntry(
        session_id=session_id,
        created_utc=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        experiment_phase=experiment_phase,
        claim_id="CLM-atsi-zone-f1",
        metric_name="zone_f1",
        metric_value=f1,
        metric_threshold=f1_threshold,
        witness=f1_witness,
        passed=f1_passed,
        hardware_gated=(f1_witness == "Hardware-gated"),
        data_gated=(f1_witness == "Data-gated"),
        notes="ATSI receive-only ambient zone occupancy F1",
    ))

    # Packet coverage.
    cov_passed = coverage >= coverage_threshold
    cov_witness = grade_experiment_result(
        passed=cov_passed, m2_band=m2_band, is_headline=False)
    entries.append(LedgerEntry(
        session_id=session_id,
        created_utc=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        experiment_phase=experiment_phase,
        claim_id="CLM-atsi-coverage",
        metric_name="packet_coverage",
        metric_value=coverage,
        metric_threshold=coverage_threshold,
        witness=cov_witness,
        passed=cov_passed,
        hardware_gated=(cov_witness == "Hardware-gated"),
        data_gated=(cov_witness == "Data-gated"),
        notes="ATSI packet coverage fraction",
    ))

    return entries


def record_hardware_characterisation(
    session_id: str,
    experiment_phase: str,
    claims: List[dict],
) -> List[LedgerEntry]:
    """Record the hardware characterisation claims as ledger entries.

    These are the measurements that characterise what the ESP32-S3 can and
    cannot do, rather than the outcome of a protocol phase: the per-link
    group delay and its reciprocity, the amplitude's noise floor, and the
    per-frame sign ambiguity that closes the cross-node phase route.

    Each claim is a dictionary with a `claim_id`, a `metric_name`, a
    `metric_value`, a `metric_threshold`, a `passed` flag and a `note`.
    """
    entries: List[LedgerEntry] = []
    for claim in claims:
        entries.append(LedgerEntry(
            session_id=session_id,
            created_utc=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            experiment_phase=experiment_phase,
            claim_id=claim["claim_id"],
            metric_name=claim["metric_name"],
            metric_value=float(claim["metric_value"]),
            metric_threshold=float(claim["metric_threshold"]),
            witness=claim.get("witness", "Measured"),
            passed=bool(claim.get("passed", True)),
            hardware_gated=bool(claim.get("hardware_gated", False)),
            data_gated=bool(claim.get("data_gated", False)),
            notes=claim.get("note", ""),
        ))
    return entries


if __name__ == "__main__":
    # Demo: record a sample FZOG + M2 + ATSI ledger.
    session_id = "demo_session_20261017"

    fog_entries = record_fzog_result(
        session_id, "fzog",
        f1=0.92, f1_threshold=0.90,
        median_error_m=0.8, error_threshold_m=1.0,
        iou=0.72,
        m2_band="green",
    )
    for e in fof_entries if False else f1_entries if False else fog_entries:
        append_ledger_entry(e, session_id)

    m2_entries = record_m2_result(
        session_id, "m2",
        {"worst_band": "green", "bcpt_cleared": True,
         "results": [{"pair": ["TX0", "RX1"], "p95_rad": 0.18, "band": "green"}]})
    for e in m2_entries:
        append_ledger_entry(e, session_id)

    atsi_entries = record_atsi_result(
        session_id, "atsi",
        f1=0.85, f1_threshold=0.80,
        coverage=0.82, coverage_threshold=0.70,
        m2_band="green",
    )
    for e in atsi_entries:
        append_ledger_entry(e, session_id)

    print(json.dumps(summarise_ledger(session_id), indent=2))
