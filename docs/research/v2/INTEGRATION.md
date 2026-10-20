# Aquila v2 — Integrated Research & Test Plan

**Status:** ready for hardware arrival (10× ESP32-S3).  
**Language:** Australian English.  
**Location:** `docs/research/v2/INTEGRATION.md`

This document is the single integration point for all research produced in this session:
- v1 → v2 spec comparison and dataset deep research (`dataset-and-v2-spec-deep-research.md`)
- Freedom-to-operate / patent landscape review (`fto-review.md`)
- 3D imaging concepts — subagent report (`21-3d-imaging-concepts.md`)
- 3D imaging concepts — SciSpace report (supplied as text)
- Further invention ideas (inventions U–w, Z, {, |, })
- 10-node ESP32-S3 test protocol (supplied as text)
- Patent application (`docs/patent/jennison-closure-phase-wifi-mesh.*`)
- Test library (`docs/research/v2/experiments/`)

---

## 1. The Observability Boundary (Load-Bearing Constraint)

With independent LOs, CSI = ĥ_ij = g_ij·a_i·b_j·e^{j(θ_i−ψ_j)}·e^{−j2πf_kτ}.  
Only θ_i − ψ_j is observable. Absolute cross-node phase, absolute range, cross-receiver AoA are **unobservable** without an added reference.

**Three escapes:** (1) common-transmitter reference, (2) OTA reciprocity Ĥ_AB·Ĥ_BA⁻¹, (3) shared/conducted clock+tone. **None proven wireless-only on ESP32-S3.**

**M2** = per-packet cross-node phase residual after CFO removal. **Not measured on ESP32-S3 wireless-only.** Everything hinges on it.

**Correction from the 10-node protocol:** closure phase does NOT automatically cancel LO drift when edges are measured sequentially at different times. Residual drift remains. M2 must be characterised before any cross-node coherent claim.

---

## 2. Hardware Context (ESP32-S3)

| Parameter | Value |
|-----------|-------|
| Chip | ESP32-S3, 2.4 GHz only |
| CSI capture | esp-csi, signed int8 I/Q, imaginary byte first ([Im, Re]) |
| Subcarriers | ~64 @ 20 MHz HT, ~128 @ 40 MHz HT40 |
| Bandwidth | 20 MHz (HT20) or 40 MHz (HT40) |
| Range resolution | c/(2B) ≈ 7.5 m @ 20 MHz (coarse native bin) |
| RX timestamp | `rx_ctrl.timestamp` (µs, unreliable under power save); `WIFI_PKT_RX_TIMESTAMP_NSEC` (80 MHz / 12.5 ns grid) |
| TX timestamp | **None** for raw `esp_wifi_80211_tx` |
| Raw TX default | 1 Mbps 802.11b (no OFDM preamble, no CSI) |
| CSI excitation primitive | ESP-NOW HT40 MCS0 |
| FTM on S3 | HT20 only, metre-level (KPI 4 m @ 40 MHz; ESP32-S2 indoor mean 3.52 m) |

---

## 3. Hardware Configuration for 10 Nodes

**Geometry** (single room, ~4–6 m × 4–6 m, stable furniture):
| Nodes | Position | Height |
|-------|----------|--------|
| 1–4 | Four room perimeter positions | ~0.3 m |
| 5–8 | Corresponding perimeter positions | ~2.2–2.5 m |
| 9–10 | Opposite mid-wall positions | ~1.2–1.5 m |

- Non-coplanar, antenna orientation recorded, photographed.
- Antenna phase-centre positions surveyed to ~2 cm with tape/laser (deliberate survey input — NOT zero-survey commissioning).
- Same geometry through all three approach tests.
- One node is scheduled transmitter at a time during active tests; other nine receive. Up to 90 directed TX→RX observations per complete sounding round.
- Fixed 2.4 GHz channel. HT20 first; HT40 only after HT20 baseline stable. No channel hopping; no 5 GHz (S3 is 2.4 GHz only).

**Provisioning per node:**
| Kconfig option | Value |
|----------------|-------|
| `AQUILA_WIFI_SSID` | test LAN SSID |
| `AQUILA_WIFI_PASSWORD` | test LAN password |
| `AQUILA_CORE_HOST` | core host IPv4 |
| `AQUILA_CORE_DATA_PORT` | 9100 |
| `AQUILA_CORE_CONTROL_PORT` | 9101 |
| `AQUILA_NODE_ID` | 0–9, unique per node |
| `AQUILA_PAIRING_CODE` | 0 (skip join for experiment) |

Record per node: serial number, board revision, antenna, firmware hash, calibration date.

---

## 4. The 14-Day Campaign Sequence

| Day(s) | Experiment | Purpose | Go / No-Go Gate |
|--------|-----------|---------|-----------------|
| 1–3 | **FZOG** amplitude baseline | Coarse 3D occupancy from link amplitude changes | Presence F1 ≥ 0.90; median 3D target error ≤ 1.0 m (HT20) |
| 4–7 | **M2 characterisation** → **BCPT** | Cross-node phase residual; closure-phase tomography | M2 not red; closure SD ≤ 0.25 rad; target separation ≥ 3× pooled SD |
| 8–10 | **ATSI** receive-only ambient | Passive occupancy without sensing-node TX | Zone F1 ≥ 0.80; packet coverage ≥ 70% |
| 11–14 | **Final synthesis** | Held-out session, furniture rearrangement, evidence ledger | Publish witness-graded results |

**If M2 ≥ 1 rad (red):** cross-node coherent BCPT is hardware-gated for this wireless-only setup. The campaign still ships FZOG + ATSI + (possibly amber M2 with caveats).

---

## 5. Experiment Specifications (Integrated)

### FZOG — 3D Fresnel-Zone Occultation Grids
- **What:** Amplitude/power-first 3D occupancy; gauge-independent.
- **Feature:** `x_e(t,k) = log(|ĥ_e(t,k)|² + δ) − median_empty(log(|ĥ_e(t,k)|² + δ))`
- **Voxel score:** `S(v,t) = Σ_{e,k} w_{e,v,k} · x_e(t,k)` where `w` from bistatic path excess `ΔL_e(v) = ‖v−r_i‖ + ‖v−r_j‖ − ‖r_i−r_j‖`
- **Classifier:** logistic regression or regularised classifier, session-level train/test.
- **Ablations:** amplitude vs RSSI; HT20 vs HT40; all links vs one-link-at-a-time; active vs receive-only.
- **Pass:** held-out presence F1 ≥ 0.90; median single-target 3D error ≤ 1.0 m; occupied-voxel IoU with bootstrap CI.

### M2 Characterisation — Cross-Node Phase Residual Gate
- **Setup:** one fixed transmitter, ≥6 receivers, same packet burst, empty room.
- **Repeat:** across 10 minutes, after reboot, with temperature change if practical.
- **Metric:** `ε_{pq}[n] = wrap{(φ_p[n] − φ_p[0]) − (φ_q[n] − φ_q[0]) − ĈFO_{pq}[n]}`
- **Bands:** green < 0.25 rad; amber 0.25–1 rad; red ≥ 1 rad (95th percentile).
- **Gate:** BCPT only proceeds if not red. BCPT reports performance per band.

### BCPT — Bistatic Closure-Phase Tomography
- **What:** Directed-loop closure phase; highest-novelty phase hypothesis (patent Claim 1 core).
- **Measurement:** `Φ_{ijk} = arg(ĥ_{ij}(t₁) · ĥ_{jk}(t₂) · ĥ_{ki}(t₃))`; differential `ΔΦ = wrap(Φ_target − Φ_empty)`.
- **Variables:** loop duration (1, 2, 5, 10, 20 ms), edge order, reboot, temperature, target motion during loop, measured M2.
- **Pass:** empty-room closure SD ≤ 0.25 rad; stable after reboot; target separation ≥ 3× pooled SD; median 3D error ≤ 1.0 m (coarse imaging pass).
- **No-go:** M2 red or closure residual grows with loop duration → declare hardware-gated, stop expensive reconstruction.

### ATSI — Ambient-Traffic Shadow Imaging
- **What:** Receive-only occupancy from ordinary 2.4 GHz traffic (AP as illuminator).
- **Feature:** node-local power `x_j(n,k) = log(|ĥ_j(n,k)|² + δ)`, fuse calibrated node vectors.
- **Constraint:** sensing nodes are receive-only; no TX from them. Complex receiver ratio is exploratory secondary only.
- **Pass:** zone F1 ≥ 0.80; ≥ 70% observation windows with usable packets; compare vs active FZOG on same trials.

---

## 6. FTO / Patent Position

**Closure phase:** no WiFi closure-phase patent found — open novelty niche supporting the drafted patent application at `docs/patent/`.

**Highest FTO risk:** occupancy (Aerial US12044789B2, expires 2038), multi-node fusion (nami.ai mesh patent).

**Moderate risk:** count, pose, vitals, presence.

**Low risk:** DSP pipeline, time sync, calibration, deliberate "no cross-node phase coherence" design.

**Strategy:** file defensive patent on closure-phase/gauge-invariant multi-node sensing **before** publishing 10-device validation results; defensively publish the rest; keep code GPLv3-only (§11 implicit patent license + retaliation). **Not legal advice — obtain counsel review before filing.**

---

## 7. Code Modules (Ready for Hardware)

| Module | Path | Purpose |
|--------|------|---------|
| M2 characterisation | `docs/research/v2/experiments/m2_characterization.py` | Cross-node phase residual gate |
| FZOG experiment | `docs/research/v2/experiments/fzog.py` | Amplitude-first 3D grid |
| BCPT hardware | `docs/research/v2/experiments/bcpt_hardware.py` | Sequential closure-phase loops |
| ATSI hardware | `docs/research/v2/experiments/atsi_hardware.py` | Receive-only ambient traffic |
| Protocol runner | `docs/research/v2/experiments/test_protocol_runner.py` | 14-day sequence orchestrator |
| Node config | `docs/research/v2/experiments/node_config.py` | 10-node geometry + provisioning |
| Data pipeline | `docs/research/v2/experiments/data_pipeline.py` | Ingestion, storage, analysis |
| Evidence ledger | `docs/research/v2/experiments/evidence_ledger.py` | Witness-graded output |

Existing modules remain: `csi_sim.py`, `closure_phase.py`, `reciprocal_gauge.py`, `ambient_ledger.py`, `test_harness.py`, `hardware.py`, `hardware_automation.py`.

---

## 8. Evidence Ledger Vocabulary (Witness Grades)

| Grade | Meaning | Use in this campaign |
|-------|---------|---------------------|
| **Gated** | measured and passing a defined bar | headline claim only (presence F1 ≥ 0.90) |
| **Measured** | computed from real/replayed/simulated input, recorded in ledger | all experiment results |
| **Claimed** | asserted without recorded measurement | avoid |
| **Data-gated** | blocked on missing input | avoid |
| **Hardware-gated** | blocked on missing hardware | BCPT if M2 red |

Every result from the 10-node campaign carries exactly one grade.

---

## 9. Immediate Actions When Devices Arrive

1. **Flash** all 10 nodes with `idf.py -p /dev/ttyUSB<N> flash` (firmware at `firmware/esp32-s3-csi-node/`).
2. **Provision** each node with unique `AQUILA_NODE_ID` 0–9, WiFi credentials, core endpoint UDP 9100.
3. **Discover** nodes: `python3 hardware.py` (2 s listen on UDP 9100).
4. **Run M2 characterisation first** — gate everything phase-related.
5. **Run FZOG** — establish the performance floor.
6. **Run BCPT** — only if M2 gate passes.
7. **Run ATSI** — passive fallback regardless.
8. **Produce evidence ledger** — witness-graded results, session-level train/test separation, no random splits of adjacent packets.

---

**End of integration document.** All code modules below are written to be dropped into `docs/research/v2/experiments/` and run with `python3` or `./sim-python` inside `nix develop`.
