# Aquila v2 — 20-agent deep-research synthesis

Status: working synthesis of the 20 notes in this folder. Cross-references are to
note numbers (`NN`). Australian English. Claims are graded as in the notes; this
document does not upgrade them.

## Method

Twenty parallel research agents, one per prompt in `BRIEF.md`, each searching
primary sources via Parallel. 20/20 notes exist (see index below). Each note holds
the full findings, ESP32-S3 feasibility, evidence strength and sources; this
document integrates them, states what it means for the v2 plan, and lists the
decisive experiments.

Note index: `01` single-node CSI · `02` multi-node localisation · `03` channel
charting · `04` through-wall/materials · `05` raw I/Q · `06` I/Q passive radar ·
`07` TX legality/primitives · `08` radar geometries · `09` MIMO/TDM/full-duplex ·
`10` OTA reciprocity · `11` common-ref/two-way · `12` consensus/oscillator ·
`13` static-multipath self-reference · `14` map/env-as-antenna ·
`15` node self-localisation · `16` RF-SLAM · `17` time/frequency precision ·
`18` environment inference · `19` model-vs-learning · `20` theoretical boundary.

## 1. The boundary — what is, and is not, autonomously determinable

`20` formalises it: with independent LOs, CSI takes the form
`ĥ_ij = g_ij·a_i·b_j·e^{j(θ_i−ψ_j)}·e^{−j2πf_kτ}`, and only `θ_i−ψ_j` is
observable. On a connected measurement graph the node phases are fixed **only up
to one global additive constant** — a `U(1)` gauge (plus a time/frequency gauge).

**Invariants that survive the gauge** (and are therefore usable with no shared
reference):
- amplitude;
- per-link subcarrier differential phase → delay/ToF;
- per-receiver inter-antenna differential phase → AoA **within one receiver**;
- two-way products `ĥ_AB·ĥ_BA` → cancels LO, exposes hardware asymmetry;
- temporal differentials → Doppler.

**Not observable without an added reference:** absolute cross-node phase, absolute
phase-of-flight range, absolute synchronisation. AoA **across** independent
receivers is unobservable; AoA **within** a receiver is observable.

**Minimal reference.** One extra constraint per connected component fixes the
gauge: a single **common-transmitter broadcast**, **one reciprocal round trip per
link** (a star is CRB-optimal), or a **shared/conducted clock+tone**. Cost O(N).
`σ_φ ≳ 1/√(2·N_s·ρ)` (Modified CRB), degraded by oscillator phase noise
(−120…−80 dBc/Hz).

## 2. The decisive numbers

- **Relative-LO phase, raw (untracked):** crystals ±10 ppm ⇒ 0–20 ppm ⇒ raw
  `τ_{π/2} = 1/(4Δf) ≈ 5–10 µs` (`17`). After CFO removal the residual is
  phase-noise-limited (track A), so the governing quantity is the **residual
  after CFO estimation over the exchange interval** — which is exactly the
  M2 residual. **M2 is now measured on S3 and is red**
  (`22-m2-hardware-measurement.md`): the ESP32-S3 negates the whole channel
  vector from frame to frame, independently at each receiver, so the
  cross-node phase of one packet is ambiguous modulo pi with no resolvable
  reference. The relative-LO reasoning above therefore does not apply on this
  hardware — the ambiguity is not a drift that estimation can track, it is a
  discrete per-frame sign. The **gauge-invariant phase slope** survives it and
  gives a per-link path length to about a decimetre, validated to a couple of
  centimetres by reciprocity. `10` notes hardware chain coefficients drift <2.6 % over 4 h, but
  inter-node **LO phase must be re-estimated at the synchronisation rate**, not
  the hardware-calibration rate; conflating the two is the classic error.
- **Timekeeping (`17`):** the MAC exposes a **nanosecond RX timestamp**
  (`WIFI_PKT_RX_TIMESTAMP_NSEC`), latched ~20.8 µs after sampling start on an
  80 MHz (12.5 ns) grid; the µs `rx_ctrl.timestamp` is unreliable under power
  save. TSF is 64-bit / 1 MHz, ~3.96 ppm skew. FTM carries **picosecond** t1–t4
  but accuracy is **metre-level** (802.11mc KPI 4 m at 40 MHz; ESP32-S2 indoor
  mean 3.52 m). There is **no TX hardware timestamp** for `esp_wifi_80211_tx`.
- **Range/velocity at 20 MHz (`01`,`06`,`08`):** range resolution `c/2B` ≈ 7.5 m
  (15 m path bin, some notes); `v_max = λ/2T` ≈ 1.2 m/s at 20 Hz, ~6 m/s at
  100 Hz. Doppler is the only useful dimension at commodity bandwidth.
- **Turnaround vs coherence:** `09` — TDM is **mandatory** on S3, and the MAC
  turnaround is milliseconds; OTA reciprocity requires a bidirectional exchange
  **inside** a phase-coherence interval. `12` reconciles the raw-vs-residual gap:
  untracked CFO wraps relative phase at `t_π = 1/(2Δf)` → **5.1 µs at 40 ppm,
  10.3 µs at 20 ppm**; after CFO removal the residual (~10–100 Hz) gives
  **ms–tens-of-ms** coherence (MegaMIMO: 100 Hz → π in 20 ms). The raw wrap
  governs un-removed absolute phase/AoA/ToF; the **post-removal residual** governs
  per-frame CFO-sanitised CSI and tracking. Update-rate/holdover optimum ≈ **20 ms**;
  ±10 ppm holdover = 10 µs/s. Two further facts: the PLL initial phase
  **re-randomises per boot/channel even with a shared clock** (ESPARGOS), so a
  frequency ledger alone cannot give coherence; and consensus clock sync
  (`12`) pins **time** (µs-class) but never carrier phase. So this is a go/no-go
  *measurement* (M2 residual), not a settled negative.

## 3. Capability map (ESP32-S3 feasibility)

| Capability | S3 feasibility | Needs phase coherence? | Needs TX? | Evidence |
| --- | --- | --- | --- | --- |
| Single-node motion/presence | **Demonstrated** (ESPectre ~97 %, `esp-radar`) | No | No | `01` 7/10 |
| Amplitude/CSI-power Doppler | Feasible, uncharacterised on S3 | No | No | `01` |
| Multi-node fingerprinting / RTI | **Receive-only, unblocked**; COTS sub-metre; S3 analogue 0.45 m median | No | No | `02` 8/10 |
| Channel charting | Relative geometry label-free; Doppler CC is the independent-node hope | No (affine-ambiguous) / partly | Optional | `03` 7/10 |
| Through-wall presence/HAR | **Demonstrated** (Strohmayer: 18.5 m, 5 rooms) | No | No | `04` 7/10 |
| Material / liquid ID | COTS/SDR/RFID only; not S3 | Partly | — | `04`,`18` |
| Raw I/Q capture | **Demonstrated** (80 MSa/s burst; gapped, uncalibrated) | n/a | No | `05` 8/10 |
| Passive/bistatic radar on I/Q | **Hypothesis on S3**; SDR + recent COTS (LoSRef) | Delay/phase ref | Yes | `06` 7/10 |
| Monostatic radar | **Blocked** on-chip (~110 dB SIC needed) | — | Yes | `08`,`09` |
| OTA reciprocity | **Undemonstrated on S3**; residual unmeasured | Yes | Yes (bidirectional) | `10` 6/10 |
| Common-TX / two-way transfer | Conducted proven on S3-family (ESPARGOS S2, esp-crab C5); wireless-only undemonstrated | Yes | Yes | `11` 7/10 |
| Static-multipath / LoS self-reference | Established on COTS; per-link gauge only; not cross-node | No | No | `13` 6/10 |
| Map-aided / env-as-antenna | Map-aided bounded; passive reflector gain real & cheap on S3 | No | No | `14` 6/10 |
| Node self-localisation (FTM) | **FTM 1–2 m**, but S3 is 2.4 GHz HT20 only; AoA blocked | No (ranging) | Yes (FTM) | `15` 8/10 |
| RF-SLAM / floorplans | **No ESP32 demo**; blocked by delay resolution; Wi-PRO (2026) FTM+CSI 8.2→1.5 m | Partly | Likely | `16` 2/10 |
| Doors/windows; object change | Established on commodity Wi-Fi; S3 weakest class | No | No | `18` 7/10 |
| Volumetric/polyhedron coherent AoA | **No ESP32 volumetric array exists**; coupling/shadowing unmeasured | Yes | Optional | `08`,`09`,`16` |

## 4. Chaining — which capabilities enable which

The user's hypothesis (one technology feeding the next) is borne out. Ranked by
leverage:

1. **Common reference ⇒ coherence ⇒ AoA/DoA ⇒ 3D localisation ⇒ mapping/SLAM.**
   The single highest-leverage enabler. On S3 it is the **conducted** reference
   (clock + 2.4 GHz tone), proven on ESP32-family, not S3 specifically (`08`,`11`,`20`).
2. **Synthetic simulator + weak supervision ⇒ cross-room transfer**, with **no**
   coherence. Ray-traced Sim2Real reached 82.1 % F1 with 20 % real labels =
   98.6 % of full supervision (`19`). This is the learned route's enabler.
3. **FTM ranging ⇒ node auto-survey ⇒ anchors ⇒ geometric constraint** for
   localisation and **map-aided calibration** (`15` ⇒ `14`,`02`).
4. **ESP-NOW OFDM excitation ⇒ controlled CSI** ⇒ better Doppler/HAR (see §7
   conflict).
5. **LoS-path self-reference ⇒ CFO/delay calibration ⇒ bistatic radar phase**
   (`13` ⇒ `06`).
6. **Channel charting (unlabelled) ⇒ relative geometry ⇒ few-shot zone labels**
   (`03` ⇒ `19`).

## 5. Reusable models, data and tools

- **SenseFi** (MIT) — benchmark library; **MM-Fi**, **ESPARGOS datasets** (DaRUS,
  DOI), **CSI-Bench** (CC BY-NC-ND), **OpenCSI** (Apache-2.0) for cross-room bias
  self-calibration (`14`,`19`).
- Caveat: these are **L-CSI from Intel/Atheros NICs** — domain adaptation is
  mandatory; none is ESP32-S3 CSI (`19`).
- **Supervision ladder:** none (AutoFi geometric self-supervision) · weak
  (channel charting: only AP positions + LoS boxes) · synthetic (ray-traced
  Sim2Real) · full (`19`).
- **Route split:** physical quantities (presence, respiration, Doppler, range,
  count) → model-based, label-free; high-dimensional labels (identity, pose,
  gesture) → learning. **Cross-modal (vision/radar) distillation is the strongest
  generalisation lever** (`19`).
- **ESP-DL / ESP-NN** INT8 SIMD (~18×) exists but is demonstrated on vision, not
  CSI (`19`).

## 6. Decisive experiments (cheapest-decisive first)

1. **Relative-LO coherence & reciprocity residual** on two S3s, against a
   conducted-reference ground truth (`10`,`11`,`17`,`20`). Gates every coherent
   method. *Falsified if residual grows unbounded / is statistically random.*
2. **Nanosecond RX-timestamp characterisation** against a known coax delay: latch
   latency, 12.5 ns grid, jitter, forced re-lock phase RMSE (`17`).
3. **ESP-NOW vs raw-TX CSI yield** — prove which primitive excites CSI (`07`,`01`).
4. **Multi-node fingerprinting/RTI accuracy vs node count vs geometry** on S3 —
   the curve no public primary covers; include a volumetric sub-array (`02`).
5. **FTM self-survey**, 6–8 nodes, all-pairs, MDS + Procrustes without anchors
   (`15`).
6. **Volumetric vs planar aperture AoA** off a known reflector — the single
   unlock that would move S3 past range-only into mapping (`08`,`09`,`16`).
7. **Cross-room transfer**: synthetic-pretrained + few-shot vs pure supervised vs
   analytic (all target labels withheld) (`19`).

## 7. The volumetric / polyhedron question

`08`/`09` verdict: a soldered-PCB polyhedron is a **plausible 20-face 3D
aperture** (geodesic/spherical arrays give full-sphere DoA, no scan loss; the
geometry is established for CubeSat arrays). A **shared clock removes cross-face
phase calibration and the shared-LO drift**, and a hub face (Ethernet-switch data
plane + separate conducted sync plane; an FPGA hub could add ns triggering and
coherent TX) is the sensible topology. **But it does not remove TDM**: a face
switch samples faces sequentially, and avoiding face-TDM needs 20 coherent RX
chains from one LO, not a switch. Mutual coupling and face shadowing on a
conducting solid are **unmeasured**, and **no ESP32 volumetric sensing array
exists**. It remains a Track 3 structure.

## 8. What this does to the v2 plan and the map

**Confirms the commodity learned route** (`docs/plan/v2/map.md`): presence,
multi-node fingerprinting/RTI, channel charting, FTM self-survey, doors/windows,
and environment change detection are all **receive-only and require no cross-node
phase**. Nothing here overturns it.

**Confirms coherent apertures, coherent ToA, RF-SLAM geometry and volumetric
arrays are Track 3** — they need a conducted reference (proven on ESP32-family,
not S3) or an unproven wireless escape.

**Changes / new facts to fold into the specs:**
- **The nanosecond RX timestamp exists** (`17`) — this supersedes the v1
  "no PHY timestamp" stance in `fusion-sync.md` §1.1 for S3, and upgrades the
  trigger/epoch design.
- **FTM on S3 is 2.4 GHz HT20 only** (`15`) — coarse; set expectations.
- **Raw `esp_wifi_80211_tx` defaults to 1 Mbps 11b (no OFDM preamble, no CSI)**
  (`07`) — so the CSI "ping" must be an **ESP-NOW HT40 MCS0** frame, as esp-csi
  itself uses. This matters for any active/round-trip scheme.
- **Potential glossary conflict:** `CONTEXT.md` bans **ESP-NOW**, yet ESP-NOW is
  the de-facto ESP32 CSI excitation primitive. Needs an ADR or glossary change.
- Reusable assets registered in §5 (SenseFi, MM-Fi, ESPARGOS, CSI-Bench, OpenCSI).

## 9. Residual unknowns

- The reciprocity residual after CFO correction (M2) — **measured, and red**
  (`22-m2-hardware-measurement.md`). The cross-node absolute phase is
  ambiguous modulo pi on ESP32-S3, with no resolvable reference, so every
  coherent route is closed on this hardware. The gauge-invariant phase slope
  (per-link group delay) survives the ambiguity and is validated to about a
  decimetre; its sensitivity to a person moving between nodes is not yet
  measured.
- S3-specific multi-node accuracy and volumetric gain (no primary exists).
- I/Q **path** coherence (ESPARGOS coherence is on the CSI path, not I/Q).
- Mutual coupling / shadowing in a solid array.
- Cross-configuration transfer on S3 specifically.
- Whether the conducted reference can be realised on S3 (not just S2/C5).
- The **ESP32-S3 CFO-vs-temperature curve**: over ≥30 min with a temperature
  step, how long a relative-CFO estimate stays within 10 Hz (`12`). That one
  number sets every tracking rate and decides "calibrate once" vs continuous.
