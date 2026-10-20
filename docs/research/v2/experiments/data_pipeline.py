"""Data pipeline — ingestion, storage, and analysis for real ESP32-S3 data.

INTEGRATION.md §7. Handles AQCI datagrams from hardware.CsiCollector,
persists raw CSI + metadata, computes per-experiment features, and writes
session-level train/test separated results.

Pure Python (stdlib only) for analysis; wraps hardware.CsiCollector for
real-node collection.

Australian English spelling throughout.
"""

from __future__ import annotations

import cmath
import hashlib
import json
import math
import os
import statistics
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional, Tuple

from hardware import CsiFrame, CsiCollector, frames_by_node, phase_per_subcarrier

# ---------------------------------------------------------------------------
# Storage layout
# ---------------------------------------------------------------------------

RAW_DIR = Path("data/raw")
DERIVED_DIR = Path("data/derived")
LEDGER_DIR = Path("data/ledger")
MANIFEST_PATH = Path("data/manifest.json")

# Ensure directories exist.
for d in (RAW_DIR, DERIVED_DIR, LEDGER_DIR):
    d.mkdir(parents=True, exist_ok=True)


@dataclass
class SessionManifest:
    session_id: str
    created_utc: str
    experiment_phase: str  # "fzog", "m2", "bcpt", "atsi", "synthesis"
    hardware_config: dict  # node ids, firmware hashes, geometry
    n_nodes: int
    n_frames: int
    n_bytes_raw: int
    notes: List[str] = field(default_factory=list)


def sha256_file(path: Path) -> str:
    """SHA-256 of a file, for manifest integrity."""
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def save_raw_frame(frame: CsiFrame, session_dir: Path) -> None:
    """Append one CsiFrame to the session raw JSONL file."""
    session_dir.mkdir(parents=True, exist_ok=True)
    path = session_dir / "csi_raw.jsonl"
    record = {
        "node_id": frame.node_id,
        "sequence": frame.sequence,
        "device_time_us": frame.device_time_us,
        "boot_epoch": frame.boot_epoch,
        "subcarrier_offset": frame.subcarrier_offset,
        "n_subcarriers": frame.n_subcarriers,
        "centre_freq_hz": frame.centre_freq_hz,
        "bandwidth_mhz": frame.bandwidth_mhz,
        "rssi_dbm": frame.rssi_dbm,
        "noise_floor_dbm": frame.noise_floor_dbm,
        "first_word_invalid": frame.first_word_invalid,
        "host_arrival_us": frame.host_arrival_us,
        "csi": [[c.real, c.imag] for c in frame.csi],  # [re, im] pairs
    }
    with open(path, "a") as f:
        f.write(json.dumps(record) + "\n")


def collect_session(collector: CsiCollector, duration_s: float,
                    session_dir: Path) -> List[CsiFrame]:
    """Collect frames for duration_s and save raw JSONL."""
    session_dir.mkdir(parents=True, exist_ok=True)
    frames = collector.collect_for(duration_s)
    for frame in frames:
        save_raw_frame(frame, session_dir)
    return frames


def frames_to_log_amplitudes(frames: List[CsiFrame],
                             subcarrier_index: int = 32,
                             delta: float = 1e-6) -> Dict[int, List[float]]:
    """Convert frames to per-node log-amplitude time series at one subcarrier.

    Returns {node_id: [log_amp, ...]} aligned by frame order within node.
    """
    grouped = frames_by_node(frames)
    result: Dict[int, List[float]] = {}
    for node_id, node_frames in grouped.items():
        amps: List[float] = []
        for frame in node_frames:
            if subcarrier_index >= len(frame.csi):
                continue
            c = frame.csi[subcarrier_index]
            amps.append(math.log(abs(c) ** 2 + delta))
        result[node_id] = amps
    return result


def empty_baseline_log_amp(frames: List[CsiFrame],
                           subcarrier_index: int = 32,
                           delta: float = 1e-6) -> Dict[int, float]:
    """Median log-amplitude per node from an empty-room baseline session."""
    grouped = frames_by_node(frames)
    result: Dict[int, float] = {}
    for node_id, node_frames in grouped.items():
        amps: List[float] = []
        for frame in node_frames:
            if subcarrier_index >= len(frame.csi):
                continue
            c = frame.csi[subcarrier_index]
            amps.append(math.log(abs(c) ** 2 + delta))
        result[node_id] = statistics.median(amps) if amps else 0.0
    return result


def fzog_amplitude_feature(current_amps: Dict[int, List[float]],
                           baseline_amps: Dict[int, float],
                           node_ids: List[int]) -> Dict[int, float]:
    """Per-node FZOG feature: mean(log_amp - baseline) over the window.

    Returns {node_id: feature_value}.
    """
    features: Dict[int, float] = {}
    for nid in node_ids:
        if nid not in current_amps or nid not in baseline_amps:
            continue
        cur = statistics.mean(current_amps[nid])
        base = baseline_amps[nid]
        features[nid] = cur - base
    return features


def save_manifest(session_id: str, experiment_phase: str,
                  hardware_config: dict, frames: List[CsiFrame],
                  notes: Optional[List[str]] = None) -> SessionManifest:
    """Write session manifest JSON."""
    n_bytes = 0
    for d in (RAW_DIR, DERIVED_DIR, LEDGER_DIR):
        for p in d.rglob("*"):
            if p.is_file():
                n_bytes += p.stat().st_size

    manifest = SessionManifest(
        session_id=session_id,
        created_utc=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        experiment_phase=experiment_phase,
        hardware_config=hardware_config,
        n_nodes=len(set(f.node_id for f in frames)),
        n_frames=len(frames),
        n_bytes_raw=n_bytes,
        notes=notes or [],
    )
    path = LEDGER_DIR / f"{session_id}_manifest.json"
    with open(path, "w") as f:
        json.dump(manifest.__dict__, f, indent=2)
    return manifest


def save_ledger_entry(session_id: str, experiment_phase: str,
                      claim_id: str, metric_name: str, metric_value: float,
                      witness: str, passed: bool,
                      notes: Optional[str] = None) -> None:
    """Append one commit-keyed ledger row (witness-graded).

    Witness grades: Gated, Measured, Claimed, Data-gated, Hardware-gated.
    Only Gated may be a headline claim.
    """
    LEDGER_DIR.mkdir(parents=True, exist_ok=True)
    path = LEDGER_DIR / f"{session_id}_ledger.jsonl"
    row = {
        "session_id": session_id,
        "created_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "experiment_phase": experiment_phase,
        "claim_id": claim_id,
        "metric_name": metric_name,
        "metric_value": metric_value,
        "witness": witness,
        "passed": passed,
        "notes": notes,
    }
    with open(path, "a") as f:
        f.write(json.dumps(row) + "\n")


def train_test_split_by_session(sessions: List[str],
                                test_session_ids: List[str]) -> Tuple[List[str],
                                                                      List[str]]:
    """Split sessions into train/test by session ID (never split within a
    session — adjacent packets from one session must not be split across
    train and test, per INTEGRATION.md §9).
    """
    train = [s for s in sessions if s not in test_session_ids]
    test = [s for s in sessions if s in test_session_ids]
    return train, test


if __name__ == "__main__":
    print("Data pipeline module ready.")
    print(f"  Raw dir: {RAW_DIR}")
    print(f"  Derived dir: {DERIVED_DIR}")
    print(f"  Ledger dir: {LEDGER_DIR}")
