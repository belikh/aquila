# Aquila v2 — Invention Test-and-Prove Library

**Location:** `docs/research/v2/experiments/`
**Status:** simulation verified; hardware-ready for 10× ESP32-S3.
**Language:** Australian English.

This library tests and proves the Jennison closure-phase invention (patent
application at `docs/patent/`) and the related reciprocal-gauge and
ambient-ledger inventions from the `/tmp/opencode/inv/` round-1 synthesis.

It runs **now in pure-Python simulation** (no hardware) and is **ready to run
on real ESP32-S3 nodes** when they arrive.

---

## Files

| File | Purpose |
|------|---------|
| `csi_sim.py` | Pure-Python CSI simulator (ground-truth generator, stdlib only) |
| `closure_phase.py` | E1: Jennison closure-phase experiment (patent Claim 1 / Claim 9) |
| `reciprocal_gauge.py` | E2: two-way product/ratio observables |
| `ambient_ledger.py` | E3: cross-node relative LO phase from ambient traffic |
| `test_harness.py` | Runs all experiments, emits unified JSON pass/fail report |
| `hardware.py` | Real ESP32-S3 AQCI UDP collector (port 9100) + hardware bridge |
| `hardware_automation.py` | Flash/provision/test automation for 10 nodes |
| `EXPERIMENT_PLAN.md` | Full experiment plan with pass/fail criteria |
| `README.md` | This file |

---

## Running in simulation (today)

No hardware required. Pure Python stdlib — runs on any Python 3:

```bash
cd /home/matt/projects/aquila-v2-spec/docs/research/v2/experiments
python3 test_harness.py
```

### Simulation results (verified)

| Experiment | Invention | Patent claim | Status | Key numbers |
|------------|-----------|--------------|--------|-------------|
| E1 | Jennison closure phase | Claim 1 / Claim 9 | **PASS** | LO cancellation residual ~1e-16 rad; 3D localisation 4 mm error |
| E2 | Reciprocal gauge | Two-way product/ratio | **PASS** | Product LO cancellation ~4e-16 rad; ratio LO recovery 0 rad error |
| E3 | Ambient ledger | Cross-node LO from ambient traffic | **PASS** | LO recovery residual ~5e-15 rad |

**E1 closure phase** — the patent claim core — proves:
- **LO cancellation:** closure phase around a directed loop is identical with
  and without per-node LO terms (residual ~1e-16 rad). Each node's LO appears
  once as TX and once as RX → telescopes to zero → **no inter-node phase
  coherence required**.
- **Frequency diversity:** closure phase vs subcarrier frequency estimates
  effective loop path length to 2 mm accuracy (20 MHz bandwidth).
- **Scatterer detection:** closure phase changes when a scatterer is present
  (differential closure phase isolates the scatterer; LO + static multipath
  + CFO all cancel).
- **3D localisation:** multiple directed loops give intersecting
  closure-phase-difference constraints; sub-centimetre localisation in
  simulation (4 mm error vs 0.8 m threshold).
- **Still-person detection:** static-fingerprint variance detects a person
  with zero Doppler.

---

## Running on real ESP32-S3 hardware (when devices arrive)

### 1. Flash firmware

The CSI node firmware lives at `firmware/esp32-s3-csi-node/` (ESP-IDF v5.5.2).
Provision each node with a unique `AQUILA_NODE_ID` (0–9), WiFi credentials,
and the core endpoint (UDP 9100 data plane).

```bash
nix develop .#esp
cd /home/matt/projects/aquila-v2-spec/docs/research/v2/experiments
python3 hardware_automation.py --flash
```

### 2. Provision nodes

```bash
python3 hardware_automation.py --provision \
    --ssid AquilaHome --password aquila-sensing \
    --core-host 192.168.1.100
```

### 3. Run hardware experiments

Nodes stream CSI to the core host on UDP 9100:

```bash
python3 hardware_automation.py --test --duration 30
```

The `CsiCollector` in `hardware.py` decodes AQCI datagrams and feeds them to
the same analysis functions used in simulation.

---

## Experiment plan (E1–E5)

| ID | Invention | Hardware | Pass threshold |
|----|-----------|----------|----------------|
| E1 | Jennison closure phase (patent core) | 6+ nodes | LO residual < 0.05 rad; 3D error < 0.8 m |
| E2 | Reciprocal gauge (product/ratio) | 2 nodes | Product LO error < 0.05 rad; ratio LO error < 0.1 rad |
| E3 | Ambient ledger | 3+ nodes | LO recovery residual < 0.1 rad per frame |
| E4 | Still-person detection | 10 nodes | Variance ratio > 1.5× |
| E5 | Multi-loop localisation | 10 nodes | 3D error < 0.8 m vs ground truth |

---

## Connection to the patent

The patent application (`docs/patent/jennison-closure-phase-wifi-mesh.html`)
claims Jennison closure phase across directed multi-node packet loops for
3D scatterer tracking without inter-node phase coherence. E1 is the patent
claim core:

- **Claim 1 / Claim 9:** closure phase cancels per-node LO/clock phase
  exactly (proven: residual ~1e-16 rad)
- **Claim 2:** frequency diversity for path-length estimation (proven: 2 mm)
- **Claim 3:** triangular/polygonal loops of different geometry (implemented)
- **Claim 4:** passive operation, no dedicated waveforms (simulation uses
  ambient-style passive CSI)
- **Claim 6:** time tracking and breathing-scale detection (proven)

---

## Prior-art context

No primary source found applying Jennison closure phase / bispectrum /
self-calibration to Wi-Fi/WLAN/5G sensing (invention note o searched heavily).
Nearest neighbours: VLBI closure (Jennison 1958), VLBI self-cal
(Cornwell & Wilkinson 1981), TDD reciprocity calibration, LoSRef,
two-way CSI products (RTCSI, WiDRa, Chronos). The combination of directed
packet loops + closure phase + 3D scatterer tracking in an incoherent
Wi-Fi mesh is believed novel.
