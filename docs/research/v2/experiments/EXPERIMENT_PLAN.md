# Aquila v2 — Invention Test-and-Prove Plan

**Status:** ready to run in simulation; hardware-ready for 10× ESP32-S3.
**Location:** `docs/research/v2/experiments/`
**Language:** Australian English.

This plan defines how to test and prove the Jennison closure-phase invention
(and the related reciprocal-gauge and ambient-ledger inventions) first in
pure-Python simulation, then on 10 real ESP32-S3 nodes.

The experiments share one observability boundary (from `00-synthesis.md` §1):
with independent local oscillators, absolute cross-node phase is unobservable;
only gauge-invariant combinations survive. Closure phase, two-way products and
ratios, and ambient cross-receiver ratios are exactly those gauge-invariant
combinations. The experiments prove they work on commodity ESP32-S3 silicon.

---

## 1. Experiments

| ID | Invention | What it proves | Hardware needed? | Pass threshold |
|----|-----------|----------------|------------------|----------------|
| E1 | Jennison closure phase (patent Claim 1) | LO phase cancels around a directed loop; multiple loops localise a 3D scatterer | No (sim) / Yes (10 nodes) | closure residual < 0.3 rad; 3D error < 0.6 m |
| E2 | Reciprocal gauge (two-way product/ratio) | Two-way product gives LO-free channel; two-way ratio gives relative LO phase (mod π) | No (sim) / Yes (2+ nodes) | product CFO residual < 0.5 rad; ratio LO error < 0.5 rad |
| E3 | Ambient ledger (common-view phase) | Cross-node relative LO phase recovered from a shared ambient frame, no exchange | No (sim) / Yes (3+ nodes) | LO residual < 0.3 rad per frame |
| E4 | Still-person detection | Static fingerprint detects a person with zero Doppler (breathing-scale) | No (sim) / Yes (10 nodes) | variance ratio > 1.5× |
| E5 | Multi-loop localisation | N≥4 unsynchronised nodes track a scatterer in 3D via closure-only multilateration | No (sim) / Yes (10 nodes) | 3D error < 0.6 m |

E1 is the patent claim core. E2–E5 are supporting inventions from the
`/tmp/opencode/inv/` round-1 synthesis (inventions h, o, s, k, m, r).

---

## 2. Running in simulation (today)

No hardware required. Pure Python stdlib only — runs on any Python 3.

```bash
cd /home/matt/projects/aquila-v2-spec/docs/research/v2/experiments
python3 test_harness.py
```

Expected output: a JSON report with `all_pass: true` and per-experiment
pass/fail details. Each experiment seeds its RNG and runs 100–200 synthetic
frames at 100 Hz with 20 ppm independent LO drift, 100 Hz CFO, and thermal
noise.

### What each simulation proves

**E1 closure phase:**
- Builds 6 perimeter nodes + a central moving scatterer (4 mm breathing +
  gross motion).
- Forms directed triangular loops around the perimeter.
- Measures closure phase vs time and vs subcarrier frequency.
- Asserts LO cancellation residual < 0.3 rad (each node's LO appears once as
  TX and once as RX → telescopes to zero).
- Asserts frequency-diversity path-length estimate within 0.5 m.
- Asserts 3D localisation from multi-loop closure constraints within 0.6 m.
- Asserts still-person detection via static-fingerprint variance ratio.

**E2 reciprocal gauge:**
- Two nodes facing each other (~3 m LoS).
- Measures H_AB(t1) and H_BA(t2) across a 200 µs turnaround.
- Asserts two-way product CFO-turnaround residual < 0.5 rad.
- Asserts two-way ratio recovers relative LO phase within 0.5 rad.

**E3 ambient ledger:**
- Two receivers capture the same ambient frame from a third-party AP.
- Fits intercept (relative LO phase) and slope (differential ToF) of the
  cross-receiver CSI ratio vs subcarrier frequency.
- Asserts LO recovery residual < 0.3 rad per frame.

---

## 3. Running on real ESP32-S3 hardware (when the 10 devices arrive)

### 3.1 Firmware to flash

The existing firmware at `firmware/esp32-s3-csi-node/` streams raw CSI over the
AQCI UDP data plane (port 9100). For the experiments, each node needs to be
provisioned with:

| Kconfig option | Value for experiment |
|----------------|----------------------|
| `AQUILA_WIFI_SSID` / `AQUILA_WIFI_PASSWORD` | your test LAN |
| `AQUILA_CORE_HOST` | the core host IPv4 (the machine running `hardware.py`) |
| `AQUILA_CORE_DATA_PORT` | `9100` |
| `AQUILA_CORE_CONTROL_PORT` | `9101` |
| `AQUILA_NODE_ID` | dense id `0..9`, unique per node |
| `AQUILA_PAIRING_CODE` | `0` (skip join for the experiment) |

Build and flash (ESP-IDF v5.5.2, `nix develop .#esp`):

```bash
cd firmware/esp32-s3-csi-node
idf.py set-target esp32s3
idf.py menuconfig    # set the options above
idf.py build
idf.py -p /dev/ttyUSB<N> flash monitor
```

Repeat for all 10 nodes, giving each a distinct `AQUILA_NODE_ID`.

### 3.2 Core host setup

The core host runs `hardware.py`'s `CsiCollector` on UDP port 9100, which
decodes AQCI datagrams from all nodes and feeds them to the analysis modules.

```bash
cd /home/matt/projects/aquila-v2-spec/docs/research/v2/experiments
./sim-python hardware.py        # discover nodes (2 s listen)
```

### 3.3 Experiment procedure on hardware

**E1 (closure phase, 10 nodes):**
1. Place 6+ nodes on a perimeter (~4 m radius), maximally spaced (v2 placement).
2. Place a person/scatterer at the centre.
3. Each node transmits ESP-NOW HT40 MCS0 frames to the next node in the loop
   (directed round-robin). The receiver captures CSI on each incoming frame.
4. Core collects all CSI streams, aligns by `device_time_us`, computes closure
   phase per loop per subcarrier.
5. Assert closure residual < 0.3 rad and 3D localisation error < 0.6 m vs a
   tape-measure/laser ground truth.

**E2 (reciprocal gauge, 2 nodes):**
1. Two nodes facing each other ~3 m apart.
2. Node A transmits; node B captures CSI (H_AB). Then node B transmits; node A
   captures CSI (H_BA). Turnaround ~200 µs (SIFS-class).
3. Compute two-way product and ratio. Assert residuals within thresholds.

**E3 (ambient ledger, 3+ nodes):**
1. Two receiver nodes + one ambient transmitter (a third ESP32-S3 or the home AP).
2. Both receivers capture the same ambient frame.
3. Compute cross-receiver CSI ratio vs frequency; fit intercept/slope.
4. Assert LO recovery residual < 0.3 rad per frame.

**E4 (still-person):**
1. Compare closure-phase variance with person present vs absent in the same
   loop. Assert variance ratio > 1.5×.

**E5 (multi-loop localisation):**
1. Use all 10 nodes to form many directed loops.
2. Localise a walked target; compare against laser/tape ground truth.

---

## 4. Pass/fail criteria (the "gate")

An experiment **passes** only if every sub-test meets its threshold. The
harness exits 0 if all pass, 1 otherwise. For a hardware run, the gate is:

```
E1: closure residual < 0.3 rad AND 3D error < 0.6 m
E2: product CFO residual < 0.5 rad AND ratio LO error < 0.5 rad
E3: ambient LO residual < 0.3 rad per frame (95% of frames)
E4: still-person variance ratio > 1.5×
E5: 3D localisation error < 0.6 m (walked target vs ground truth)
```

**Falsification:** if any test fails consistently (residual ≥ 1 rad, or
localisation error ≥ 1 m), the wireless-only boundary stands and the system
falls back to a conducted reference (one coherent node per home) — this is the
expected Track-3 fallback per `00-synthesis.md` §8.

---

## 5. Automation for 10 devices

When the hardware arrives, flashing and testing 10 nodes can be automated:

```bash
# Flash all 10 nodes (example; adjust /dev/ttyUSB* per board)
for i in $(seq 0 9); do
  idf.py -p /dev/ttyUSB$i flash
done

# Run discovery
./sim-python hardware.py   # confirm all 10 nodes stream CSI to port 9100

# Run the full experiment suite on hardware
./sim-python test_harness.py   # hardware backend substitutes real CSI
```

The `hardware.py` `CsiCollector` is the production path: swap
`csi_sim.measure_csi` for real AQCI decode, and the same `closure_phase`,
`reciprocal_gauge`, and `ambient_ledger` analysis runs unchanged.

A future `tools/flash_test.py` can iterate `/dev/ttyUSB*`, flash a given
binary, and report per-node pass/fail.

---

## 6. Files

| File | Purpose |
|------|---------|
| `csi_sim.py` | Pure-Python CSI simulator (ground truth generator) |
| `closure_phase.py` | E1: Jennison closure-phase experiment |
| `reciprocal_gauge.py` | E2: two-way product/ratio experiment |
| `ambient_ledger.py` | E3: common-view ambient phase experiment |
| `test_harness.py` | Orchestrates all experiments, emits JSON report |
| `hardware.py` | Real ESP32-S3 AQCI UDP collector + hardware analysis bridge |
| `EXPERIMENT_PLAN.md` | This plan |

---

## 7. Notes

- All simulation code is stdlib-only (no numpy), so it runs immediately.
- When hardware arrives, only the CSI source changes: `hardware.py` decodes
  real AQCI datagrams; the analysis functions are identical.
- The 10-node count supports E1 (6 perimeter + centre), E5 (all 10 for dense
  loops), plus spares for replacement/re-badging.
- Australian English spelling throughout (behaviour, centre, licence, etc.).
