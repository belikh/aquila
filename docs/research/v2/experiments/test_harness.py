"""Aquila v2 invention test harness.

Runs every pure-Python experiment module and emits a unified JSON report with
pass/fail per test. Designed so the same harness runs:

  * NOW, in pure-Python simulation (no hardware).
  * LATER, on real ESP32-S3 nodes (the hardware backend substitutes the
    csi_sim.measure_csi calls with real UDP/ESP-NOW CSI captures).

Run:
    python3 test_harness.py
or
    ./sim-python test_harness.py   (inside nix develop for numpy if needed)
"""

from __future__ import annotations

import json
import sys
from typing import Dict, List

try:
    from closure_phase import run_closure_experiment
    from reciprocal_gauge import run_reciprocal_experiment
    from ambient_ledger import run_ambient_experiment
except ImportError:
    from experiments.closure_phase import run_closure_experiment  # type: ignore
    from experiments.reciprocal_gauge import run_reciprocal_experiment  # type: ignore
    from experiments.ambient_ledger import run_ambient_experiment  # type: ignore


class ExperimentConfig:
    """Container holding per-experiment config objects."""

    def __init__(self) -> None:
        # closure_phase config attributes
        self.closure_n_nodes = 6
        self.closure_centre_freq_hz = 2_412_000_000.0
        self.closure_bandwidth_hz = 20_000_000.0
        self.closure_n_subcarriers = 64
        self.closure_n_frames = 200
        self.closure_frame_interval_s = 0.01
        self.closure_lo_drift_ppm = 20.0
        self.closure_phase_noise_std_rad = 0.05
        self.closure_noise_std = 0.05
        self.closure_cfo_hz = 100.0
        self.closure_sampling_offset_s = 1e-6
        self.closure_seed = 42


def _make_closure_cfg(cfg: ExperimentConfig):
    """Build a config object compatible with closure_phase.run_closure_experiment."""
    C = type("C", (), {})()
    C.n_nodes = cfg.closure_n_nodes
    C.centre_freq_hz = cfg.closure_centre_freq_hz
    C.bandwidth_hz = cfg.closure_bandwidth_hz
    C.n_subcarriers = cfg.closure_n_subcarriers
    C.n_frames = cfg.closure_n_frames
    C.frame_interval_s = cfg.closure_frame_interval_s
    C.lo_drift_ppm = cfg.closure_lo_drift_ppm
    C.phase_noise_std_rad = cfg.closure_phase_noise_std_rad
    C.noise_std = cfg.closure_noise_std
    C.cfo_hz = cfg.closure_cfo_hz
    C.sampling_offset_s = cfg.closure_sampling_offset_s
    C.seed = cfg.closure_seed
    return C


def run_all() -> Dict[str, object]:
    """Run every experiment and return a combined report."""
    cfg = ExperimentConfig()
    closure_cfg = _make_closure_cfg(cfg)

    reports: List[Dict[str, object]] = []
    overall_pass = True

    # --- 1. Jennison closure phase (patent Claim 1 / Claim 9) ---
    cp = run_closure_experiment(closure_cfg)
    reports.append({
        "invention": "jennison_closure_phase",
        "patent_claim": "Claim 1 / Claim 9 (closure phase across directed loops)",
        "pass": bool(cp["all_pass"]),
        "details": {
            "lo_cancellation": cp["test_lo_cancel"],
            "frequency_diversity": cp["test_frequency_diversity"],
            "scatterer_detection": cp["test_scatterer_detection"],
            "localisation": cp["test_localisation"],
            "still_detection": cp["test_still_detection"],
        },
    })
    overall_pass = overall_pass and bool(cp["all_pass"])

    # --- 2. Reciprocal gauge (two-way product/ratio) ---
    rg = run_reciprocal_experiment(cfg)
    reports.append({
        "invention": "reciprocal_gauge",
        "patent_claim": "two-way product/ratio observables (gauge-free)",
        "pass": bool(rg["all_pass"]),
        "details": {
            "product": rg["test_product"],
            "ratio": rg["test_ratio"],
        },
    })
    overall_pass = overall_pass and bool(rg["all_pass"])

    # --- 3. Ambient ledger (common-view phase referencing) ---
    al = run_ambient_experiment(cfg)
    reports.append({
        "invention": "ambient_ledger",
        "patent_claim": "cross-node relative LO phase from ambient traffic",
        "pass": bool(al["all_pass"]),
        "details": {
            "ambient_ledger": al["test_ambient_ledger"],
        },
    })
    overall_pass = overall_pass and bool(al["all_pass"])

    return {
        "schema": "aquila.v2.experiment-report/v1",
        "all_pass": bool(overall_pass),
        "n_experiments": len(reports),
        "n_passed": sum(1 for r in reports if r["pass"]),
        "reports": reports,
    }


def main() -> int:
    report = run_all()
    print(json.dumps(report, indent=2))
    return 0 if report["all_pass"] else 1


if __name__ == "__main__":
    sys.exit(main())
