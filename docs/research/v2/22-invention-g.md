# 22 — Invention: the Reciprocal Coherence Fabric (RCF)

Status: invention proposal (falsifiable; HYPOTHESIS-grade where noted). Written for the
Aquila v2 deep-research programme; Australian English. Internal cross-references are to
this folder's notes (`NN`). Grades: **ESTABLISHED / HYPOTHESIS / SPECULATION**.

Files read first, in full: `BRIEF.md`, `00-synthesis.md`.

---

## 0. Answer in one page

**The invention.** A home full of cheap ESP32-S3 nodes becomes **one sharp,
self-calibrating, generalising 3D sensor** when the nodes run a single scheduled,
compliant two-way *weave* — each node transmits exactly once per cycle, every node that
hears a transmission records its CSI — and the measurements are decomposed per link
into **two exact observables**:

1. the **product channel** `P_ij = Ĥ_ij · Ĥ_ji` — all local-oscillator (LO) phase and
   CFO cancel, leaving the *squared* propagation channel `H_ij²` times a static
   per-pair hardware factor; and
2. the **ratio channel** `R_ij = Ĥ_ij / Ĥ_ji` — the propagation channel cancels
   *exactly* (reciprocity), leaving the *relative LO phase* of the two nodes times a
   static hardware factor.

The product field is a **gauge-free, environment-referenced coherent aperture at an
effective wavelength λ/2**: each link is a virtual monostatic radar at its pair
midpoint, absolute in phase relative to its own static background ("clutter
heterodyne"). The ratio field is a **wireless phase bus**: it continuously tracks every
node's LO phase, which (a) removes the one irreducible error term in the product (the
CFO × exchange-latency phase), (b) converts every *one-way* channel into an absolutely
phased measurement — a coherent distributed receive array — and (c) runs the
Argos/distributed-MIMO calibration algebra *continuously, as sensing data*. The 30 cm
soldered polyhedron (one ESP32-S3 per face; hub face with a shared 40 MHz clock fan-out
and a conducted 2.4 GHz phase reference) is the **local** coherent camera; the weave is
the **global** stitcher. Home-scale baselines (metres) supply sharpness; the
environment's static field supplies the absolute phase reference; a self-calibrating
inverse problem (static field + targets estimated jointly) supplies generality.

**The decisive experiment (R1).** Two ESP32-S3 boards, a static room, a ping/reply loop
at 100–500 Hz, plus a conducted 2.4 GHz reference as ground truth: measure the
differential phase stability of `P_ij` and the LO-tracking residual of `R_ij` over
30 minutes with a temperature step. **Pass bar: σ(P phase) < 0.1 rad; σ(tracking) <
0.05 rad.** The mechanism predicts both from ns RX timestamps (note 17) plus a tracked
CFO; the closest published commodity result is 0.04 rad (ML-Track, Raspberry Pi CM4,
2025). If instead the product phase is random at ≫ 1 rad, the wireless-only fabric is
closed by physics-as-implemented and the boundary falls back to the conducted puck
(minimal crossing reference: one shared 2.4 GHz tone per coherent component).

**Cost.** R1: ~US$100 and about a week. A three-puck, twelve-node home: ~US$700–1500
BOM (pricing estimate, SPECULATION).

---

## 1. The boundary the invention must cross, stated exactly

**ESTABLISHED (note 20).** With independent LOs and receive-only operation, CSI is
`ĥ_ij = g_ij·a_i·b_j·e^{j(θ_i−ψ_j)}·e^{−j2πf_kτ}`; only `θ_i−ψ_j` is observable, and the
node phases are fixed only up to one global constant — a `U(1)` gauge (plus
time/frequency gauge freedom). Cross-node *absolute* phase, and therefore AoA/AoF
across independent receivers, are unobservable. Invariants that survive: amplitudes;
per-link subcarrier differential phase (delay); per-receiver inter-antenna differential
phase (AoA *within* one receiver); **two-way products**; temporal differentials.

**The quantity that would close the space for this invention.** Define the
*post-correction reciprocity residual*

```
σ_φ  =  std [ arg(P_ij(t)) − 2π·Δ̂f_ij(t)·δt_ij(t) − const_ij ]     (static channel)
```

i.e. the product-phase dispersion left after timestamp-based removal of the
CFO × exchange-latency term (`Δ̂f` = tracked relative CFO, `δt` = exchange latency from
ns timestamps). It combines the two published failure modes: residual CFO over the
exchange interval (Chronos/RTCSI: "cannot be fully cancelled; controlled to an
acceptable level"; ML-Track: 0.96 rad raw → **0.04 rad** after CFO-register + timestamp
correction) and any hidden per-packet phase randomness in the PHY's CSI estimator.
**If σ_φ is small (< 0.1 rad), the invention works; if it is ≳ 1 rad and random, no
wireless-only coherence exists on this hardware, and the minimal crossing reference is
one conducted 2.4 GHz tone (or one reciprocal exchange per spanning-tree edge, which is
the same thing done with radio) per coherent component.**

The mission's six search directions map onto this document: (1) phase bus → §3.5;
(2) self-calibrating array → §3.7; (3) environment as reference → §3.3; (4) sharp 3D /
round-trip asymmetry → §3.4, §3.6; (5) ping + unique replies + two-way products → §3.2;
(6) excitation/composition → §3.6.

---

## 2. Prior art, and what is genuinely new

### 2.1 What already exists

| Prior art | What it establishes | What it does *not* do |
| --- | --- | --- |
| **Chronos** (NSDI 2016) | RTCSI: the two-way *product* of CSI cancels asynchrony; used for wideband round-trip *ranging* | no phase-coherent sensing, no network, no clock tracking |
| **RTCSI Doppler** (Shi et al., WCNC 2023) | the product `Ĥ_AP·Ĥ_STA = K·H²·e^{jΘ}`; CFO of the two directions is opposite; residual phase = CFO × (t₄−t₂), *stable if timing is stable*; commodity RPi CM4 | Doppler by STFT magnitude; constant phase discarded; single link |
| **ML-Track** (TMC 2025, DOI 10.1109/TMC.2025.3529897) | **multi-link** RTCSI (4 links, ring protocol, 250 req/s, 5.6 GHz); reads the chip's CFO register and timestamps; phase error 0.96 → **0.04 rad**; particle-filter tracking, median 0.23 m; 2.5 W on RPi CM4 | per-link bistatic Doppler spectra, *incoherent* fusion; no coherent aperture, no absolute phase, no LO tracking, no ESP32 |
| **LoSRef** (arXiv 2602.05344) | the Tx–Rx LoS path is a usable *over-the-air* delay+phase reference on COTS Wi-Fi (Intel AX210 + PicoScenes, single port): phase-coherent CIR across packets; sub-wavelength (respiration) phase observed; states the boundary: "with independent clocks … the lack of a common delay and phase reference fundamentally precludes phase-coherent radar-like delay–Doppler analysis" | single link; reference is the direct path only; no network coherence |
| **Argos** (MobiCom 2012) + Kaltenberger 2010 + **Rogalin 2014** (arXiv 1310.7001) | the **ratio** observable `Y_{1→j}/Y_{j→1}` cancels the channel and yields calibration coefficients `u_i = T_i/R_i` up to a common factor; star topology CRB-optimal; with independent clocks, "calibration needs to be performed at the same rate as synchronization" — the clock must be tracked, not calibrated once | for *beamforming* on base stations; calibration is periodic, not a sensing observable; no commodity CSI |
| **D-MIMO OTA phase calibration in the TDD flow** (arXiv 2509.03722, 2026) + Kolomvakis 2025 + BeamSync/AirSync | system model: bidirectional measurements give the ratio observable; a **Kalman filter tracks inter-AP LO phases**; graph scheduling; only *differences* estimable (U(1) gauge handled by an incidence matrix) | communications, custom hardware, for coherent downlink; not sensing, not ESP32, not CSI |
| **ESPARGOS** (arXiv 2502.09405, 2408.16377) | **the puck premise on ESP32**: daisy-chained 40 MHz clock ⇒ frequency sync; PLL initial phase re-randomises on reset/channel change, so a **conducted WiFi-frame phase-reference network** (resistive divider + microstrip, RF switch per antenna, known path delays) removes per-receiver phase ambiguity; multi-board combination via one coax (HP/LP split); long-term phase stability; AoA demonstrated. Chip: **ESP32-S2**, receive-only | no TX, not S3, no fabric, no sensing network |
| **esp-csi / esp-crab** (Espressif docs, 2026) | Espressif's own "WiFi-CSI RF phase synchronisation solution": two ESP32-**C5** chips share a crystal via a clock buffer (self-TX/self-RX and single-TX/dual-RX), millimetre-level disturbance sensing; the common-crystal board is C3-TX + **S3**-RX with a clock buffer; **CSI-quality ranking: C5 > C6 > C3 ≈ S3 > ESP32** | two nodes, no network, no weave |
| **DCAR ping-pong; two-way time transfer** (Prager et al.; IEEE TWC 2023 consensus/Kalman; MDPI RS 17/3/497; Chalmers 2025) | round-trip phase transfer is the standard way to cohere distributed radar apertures; residual MSD/degrees quantified | radar hardware, purpose-built clocks, not COTS CSI |
| **Wi-Fi holography** (PRL 118, 183901, 2017) | coherent 3D reconstruction from a *scanned* single receiver over a metre-scale aperture | mechanical scanning; no network |
| **Multipath-assisted localisation** (Meissner & Witrisal 2012) | static multipath carries geometric information (virtual anchors) | needs UWB resolution (2 GHz); at 20 MHz MPCs are unresolved |
| **Geodesic-dome / polyhedral arrays** (URSI 2002; MDPI Electronics 14/23/4618, 2025; arXiv 2604.02701) | the soldered-PCB polyhedron is established antenna art: full-sphere coverage, uniform 3D resolution, planar-subarray fabrication | no ESP32/CSI implementation |

### 2.2 Novelty claims of the RCF (each delimited honestly)

- **N1 — The weave protocol as an all-pairs decomposition machine (HYPOTHESIS).**
  One broadcast cycle in which every node transmits once and all receivers record CSI
  yields, for *every heard pair*, **both** the product and the ratio — the exact
  algebraic separation of "propagation" from "clock" for the whole network at O(N)
  frames per cycle. RTCSI/ML-Track compute products link-by-link (ML-Track's ring is
  O(N) but uses them only for per-link Doppler); the D-MIMO literature computes the
  ratio only for beamforming and only periodically. The *joint, continuous* use of both
  observables as *sensing data* is new.
- **N2 — The phase bus (HYPOTHESIS).** Using the ratio field to *continuously track*
  relative LO phase at sensing cadence, rather than to re-calibrate periodically, and
  using that tracked clock (i) to remove the product's CFO×latency term and (ii) to
  convert one-way channels into absolutely phased measurements. The algebra is Argos;
  the sensing use and commodity-CSI realisation are new.
- **N3 — Clutter heterodyne: the static environment as the per-link reference
  (HYPOTHESIS).** Every product is normalised by its own *static mean*: the room's
  static field is the reference arm of a self-heterodyne interferometer, and the moving
  scene is coherently detected against it — per link, with no global phase datum. This
  is what makes the aperture *gauge-free*: no common clock, no common transmitter, no
  wired reference is needed *for the product field*. LoSRef uses the *LoS path* as
  reference on one link; generalising the reference to the whole static field and using
  it to knit a *network aperture* is new.
- **N4 — The λ/2 virtual-monostatic lattice (HYPOTHESIS).** Each link's product is
  exactly the monostatic channel of a virtual radar at the pair midpoint with effective
  wavelength λ/2: doubled Doppler and doubled phase-of-flight sensitivity; N(N−1)/2
  virtual radars at known midpoints form a self-referenced coherent aperture; targets
  are localised on intersecting ellipsoids. Two-way squaring is noted in the Bluetooth
  channel-sounding literature ("squares the one-way CFR"); its use as a *lattice of
  virtual monostatic radars stitched into a home aperture* is new.
- **N5 — Fabric + puck architecture (HYPOTHESIS).** A conducted-hub polyhedron as the
  local (per-face) coherent element and the wireless weave as the global (per-node)
  element, so that *sharpness = home baselines* and *calibration = the scene itself*.
  ESPARGOS provides the local element on S2; esp-crab provides Espressif's two-node
  phase sync on C5; no ESP32 volumetric sensing array exists (note 08).
- **N6 — Coherent transmit from the puck (HYPOTHESIS, stretch).** With the conducted
  reference, the 20 faces can pre-compensate and transmit coherently — a 13 dB,
  full-sphere, electronically steered illuminator on commodity silicon; across pucks
  the weave's bus extends this to a home-scale distributed phased array. D-MIMO shows
  coherent distributed transmit is possible with OTA phase calibration on custom
  hardware; doing it on ESP32-S3 with ESP-NOW frames is undemonstrated.

**Honest summary of novelty:** the *physics algebra* is largely known (reciprocity
calibration, two-way ranging, GPS-style clock transfer, DCAR phase transfer). The RCF's
new content is the **sensing-side formulation**: the product/ratio pair as a *sensing
instrument*, the environment as the per-link reference, the λ/2 lattice, the weave
protocol, the fabric+puck architecture, and the claim that these compose into one
home-scale coherent 3D sensor on commodity ESP32-S3. If a paper doing exactly this on
COTS CSI is found, the invention collapses to its engineering quantifications — the
residual budget (R1) and the aperture gain (R2) — which remain the falsifiable core.

---

## 3. Mechanism

### 3.1 The two observables (ESTABLISHED algebra)

Model (following RTCSI and ESPARGOS): node `i` has one LO shared by TX and RX (TDD),
phase `θ_i(t) = 2πf_i t + φ_i`; TX chain `T_i`, RX chain `R_i`, reciprocal channel
`H_ij = H_ji`. A frame from `i` received at `j` gives

```
Ĥ_ij(t) = T_i R_j · H_ij(t) · e^{ j(θ_i(t_tx) − θ_j(t_rx)) } .
```

Form the two quantities per exchange (each node hears the other once per cycle):

```
P_ij ≡ Ĥ_ij · Ĥ_ji = (T_iR_i)(T_jR_j) · H_ij² · e^{ j2π(f_i−f_j)(t_i−t_j) }   (product)
R_ij ≡ Ĥ_ij / Ĥ_ji = (T_i/R_i)/(T_j/R_j) · e^{ j2(θ_i−θ_j) }                  (ratio)
```

- `P_ij`: **all oscillator phases cancel exactly**; what remains is the square of the
  physical channel times a *static* per-pair hardware factor `v_i v_j`, times a CFO term
  `e^{j2πΔf_ij·δt_ij}` where `δt` is the exchange latency. `δt` is known from ns RX
  timestamps; `Δf_ij` is tracked from the ratio field (below). This term is the *only*
  dynamic error source, and it is observable and removable.
- `R_ij`: **the channel cancels exactly** (same channel both ways). What remains is
  `e^{j2(θ_i−θ_j)}` — twice the relative LO phase — times a static hardware ratio
  `u_i/u_j`, `u_i = T_i/R_i`. Tracked over time, its phase/2 is `θ_i−θ_j` mod π; the
  mod-π ambiguity is resolved once (coarse start) and maintained by continuity.

Two consequences that make the fabric work:

1. **The product is self-referencing.** No clock, no common transmitter, no conduit is
   needed to obtain an *absolute-phase* measurement of `H_ij²` per link, because the
   per-pair constants (hardware, static delays, per-channel chain phases) are constant
   and measurable.
2. **The ratio is the clock bus.** A spanning tree of tracked `R_ij` gives every node's
   `θ_i(t)` up to one global constant — a *wireless White-Rabbit* at CSI cadence.
   Combining both, the network also separates the hardware fingerprints:
   `log v_i v_j` (+) and `log u_i/u_j` (−) solve for all `T_i, R_i` (Argos algebra) up
   to global factors, which cancel in imaging.

### 3.2 The weave protocol (HYPOTHESIS; mission direction 5)

- **Cycle.** A coordinator (any node; the puck hub is natural indoors) emits a schedule.
  In slot `k`, node `k` transmits one **unique, ESP-NOW-compliant frame** (identity in
  the payload/MAC; ACK enabled; HT20/HT40 MCS0 so CSI exists — note 07: raw
  `esp_wifi_80211_tx` defaults to 1 Mbps 11b with no CSI, so the weave uses ESP-NOW
  HT frames, as esp-csi itself does). Every node records CSI for every frame it hears;
  the hub aggregates (premise: Ethernet switch or FPGA in the hub; across pucks, a home
  server or the pucks' Ethernet backhaul).
- **What one cycle yields.** With `N` nodes and `M` heard pairs: `M` **products**
  (LO-free squared channels), `M` **ratios** (channel-free clocks), `2M` **one-way
  channels** (absolutely phased once the bus and hardware fingerprints are solved), `M`
  **round-trip ranges** from the same timestamps, and the **hardware fingerprints**
  from the static parts. All from O(N) frames.
- **Rates.** A ~250-byte ESP-NOW frame at MCS0 lasts ≈ 200–300 µs; MAC turnaround tens
  of µs; ESP-class hardware has demonstrated ~1250 frames/s end-to-end in an office
  (esp-csi issue #201). A 20-slot cycle at ~1 ms/slot ≈ 20 ms → **50 product
  measurements per link per second** (Nyquist 25 Hz: walking and running covered;
  respiration uses sub-carrier phase over longer windows or a slow sub-schedule).
  HYPOTHESIS pending R1; office throughput/loss is the risk.
- **Failure handling.** `P_ij` from the *previous* cycle substitutes for a lost frame
  (one-cycle staleness at 20 ms is benign); ESP-NOW ACK/retry is the first line; the
  schedule is adaptive (more slots to weak links).
- **CFO bookkeeping.** The ratio slope gives `Δf_ij`; the product phase is corrected by
  `2πΔ̂f_ij·(t_i−t_j)` with `t` from ns timestamps (note 17; 12.5 ns grid ⇒ quantisation
  `2πΔf·12.5 ns` is negligible: at 24 kHz, 0.002 rad). Where S3 exposes no CFO register
  (no documented equivalent of the Broadcom register ML-Track reads), the CFO is
  estimated from the CSI phase history and the ratio field itself — a closed loop:
  **the ratio tracks the clock, the clock corrects the product, the product senses the
  scene.**

### 3.3 Clutter heterodyne — the environment as the per-link reference (HYPOTHESIS; mission direction 3)

For each link, keep a running *static mean* `⟨P_ij⟩` (EWMA over seconds, or a stored
baseline from a quiet period) and form

```
P̃_ij(t) = P_ij(t) / ⟨P_ij⟩ ≈ 1 + 2·δH_ij(t)/H_ij^static + O(δ²) .
```

`P̃_ij` is dimensionless and **free of every per-pair constant** (hardware, delays,
static multipath); its phase and amplitude are a *relative* measurement of the scene's
change, with the static field as the reference arm of a heterodyne interferometer. The
static clutter is not noise to be subtracted; it is the **local oscillator**, and the
moving target beats against it — per link, at whatever phase the room provides. This is
LoSRef's insight (the LoS path as reference) applied to the *whole* static field and
lifted to the *network*: different links beat at different static phases, and the
ensemble is coherently solvable because all links see the same scene.

Consequences:

- Static *change* detection (a person standing still, a moved chair, an opened door) is
  a change of `⟨P_ij⟩` itself against the stored baseline — no motion required, and the
  phase makes it geometric rather than a fingerprint (notes 01/13/18 upgrade to
  coherent detection).
- The squared multipath (cross terms `a_j a_k e^{−j2πf(τ_j+τ_k)}`, the "squaring the
  one-way CFR" effect) enriches the static field but does not damage the *differential*
  measurement: to first order `P̃−1 = 2(δH/H^static)` — twice the *relative* change of
  the one-way channel (doubled sensitivity; same information, phase-fixed).
- Slow drifts (thermal, furniture) are tracked by adapting the baseline over minutes,
  with the stored baseline retained for long-horizon change maps.

### 3.4 The λ/2 virtual-monostatic lattice (HYPOTHESIS; mission direction 4)

`P_ij = v_i v_j H_ij²` has the phase `−4πf·(|x_i−p|+|p−x_j|)/c` for a target at `p`
(two-way, doubled phase slope) — **exactly the monostatic channel of a virtual radar at
the pair midpoint `m_ij`, with effective wavelength λ/2 (6.22 cm at 2.412 GHz)** and
monostatic Doppler. The network owns `N(N−1)/2` coherent virtual monostatic radars at
known midpoints.

- **Localisation.** Each link constrains the target to an *ellipsoid* with foci
  `x_i, x_j`; the phase gives the ellipsoid index to `Δr = (λ/2)·σ_φ/(2π)` — **1 mm per
  link at σ_φ = 0.1 rad** — modulo the λ/2 ambiguity, resolved by multi-channel
  (82 MHz across 2.4 GHz channels → 1.8 m index scale) and FTM/RSSI coarse geometry
  (note 15). 190 links in a puck and hundreds across the fabric over-determine the
  target; accuracy is then limited not by noise but by the static-field model, which
  the network measures.
- **Doppler.** Doubled (λ/2). A person walking at 1 m/s gives `f_D = 2v/λ ≈ 16 Hz`,
  inside a 50 Hz cycle. Sign is preserved (complex phase, unlike classic CSI Doppler).
- **Range and geometry.** The same frames' timestamps give round-trip ranges
  (FTM-class, note 15) for a node self-survey; the static field gives the rest.
- **Polarisation (extension).** A face with two switchable, differently oriented
  antennas yields polarimetric products; the polyhedron's face orientations already
  diversify polarisation. Adds material/orientation discrimination.

### 3.5 The phase bus and the gauge-free home aperture (HYPOTHESIS; mission direction 1)

- **Gauge.** Ratio tracking fixes all `θ_i` up to one global constant and one global
  sign — the discrete remnant of the U(1) gauge. Neither matters for the fabric's
  observables: the product is gauge-invariant by construction; the one-way channels are
  coherent up to a global phase (harmless in imaging) and a per-node sign (resolved
  once by a triangle of ratios — `φ_AB + φ_BC + φ_CA ≡ 0` — or by continuity).
- **Distributed receive aperture.** With `θ_i(t)` tracked and hardware fingerprints
  solved, each one-way `Ĥ_ij` is corrected to `H_ij`: an *absolutely phased* channel
  from a known node position. The 30 cm puck adds its calibrated per-face channels; the
  fabric adds home-scale baselines. Composite aperture = the home (up to ~10 m):
  transverse resolution ~ `λ·R/D` ≈ 0.125·4/8 ≈ 6 cm at 4 m. At 2.4 GHz a 5 m
  aperture's Fraunhofer distance is ~200 m, so every home is deep in the *near field*:
  imaging is exact spherical-phase holography, not far-field beamforming.
- **Coherence vs cadence.** The bus must update faster than phase decorrelation. The
  governing quantity is the *tracked* phase variance: post-CFO residuals of 10–100 Hz
  (ms–tens of ms coherence, note 12); the 20 ms weave cadence sits at the optimum
  suggested there, and R1 measures the actual S3 number.
- **Time, not just phase.** The bus is carrier phase, not symbol timing: for *receive*
  coherent processing only the (known) geometry enters, so no sub-µs time sync is
  required — the fabric's structural advantage over distributed-MIMO *transmit*, which
  additionally needs timing.

### 3.6 The puck: local sharpness, coherent transmit (HYPOTHESIS; mission direction 6)

- **Conducted hub (ESTABLISHED on S2, HYPOTHESIS on S3).** ESPARGOS's topology —
  daisy-chained 40 MHz clock + a WiFi-frame reference distributed over microstrip/coax
  to an RF switch per face, with known path delays — is the premise's hub. It removes
  the PLL initial-phase ambiguity per face and per channel, making the 20 faces
  *phase-coherent receivers* with no over-the-air calibration. On S3: same chip family
  and radio generation as S2; Espressif's own C3-TX/S3-RX common-crystal board shows
  the clock-conduction step already exists in S3 products; the 2.4 GHz reference
  network needs re-laying on S3 boards (HYPOTHESIS, ~1 hardware iteration).
- **TDM-MIMO inside the puck.** One face transmits at a time; all faces receive on the
  shared reference ⇒ the full 20×20 coherent bistatic matrix per frame-set. Note 09's
  result stands (a face switch samples sequentially; only 20 parallel RX chains would
  remove TDM). At 20 slots ≈ 20 ms, puck Doppler matches the fabric.
- **Coherent transmit (stretch, HYPOTHESIS).** With per-face phase known, the puck can
  pre-compensate and transmit coherently: `10·log10(20) = 13 dB` gain, full-sphere
  steering, as an *electronically steered illuminator*. Because OFDM carrier phase is
  what matters (sample-timing errors ≪ symbol time only add a linear across-subcarrier
  phase), µs-level TX scheduling suffices for a ~20 MHz waveform; scheduling jitter is
  *measured* (ns RX timestamps) and fed forward. Across pucks, the weave's bus extends
  coherent transmit to a home-scale distributed phased array — it can *illuminate* a
  chosen 3D region (direction-selective interrogation, suppressing multipath by
  geometry) and receive coherently.

### 3.7 Self-calibrating inverse problem and cross-home generalisation (HYPOTHESIS; mission direction 2)

The fabric's unknowns are hierarchical and identifiable:

1. **Clocks** `θ_i(t)` — from the ratio field (linear, exact).
2. **Hardware** `v_i, u_i` — from the static parts of products/ratios (bilinear, exact
   up to global constants), stable for hours (note 10: chain drift < 2.6 % over 4 h).
3. **Node geometry** — by construction (fab), FTM self-survey, and the fabric's own
   AoA/range field (notes 15, 16).
4. **The static field** `H^static_ij` — measured (baseline) and *modelled* as a sparse
   set of specular planes + diffuse clutter with few parameters; the network fits it
   from the baseline (a small inverse-scattering problem per room, not a per-home
   neural model).
5. **Targets** — dynamic localisation/imaging against the fitted field.

Because (4) is *measured per home* and (3) is known, nothing in the inversion is
home-specific except the fitted static parameters; the *algorithm* and the physics
transfer. Learned components (a generative static-field prior, or Sim2Real-pretrained
detectors — note 19's 82.1 % F1 with 20 % real labels) act as priors for (4)/(5), not
as the transfer mechanism. One pipeline yields: decimetre-class 3D localisation with
mm-class phase precision, doubled-Doppler vitals/gait, static-change maps (doors,
furniture, occupants at rest), material/polarisation cues, and an obstruction map (the
walls are literally the reference field).

---

## 4. What must be true (preconditions), and the exact falsifiers

| # | Precondition | Falsified if |
| --- | --- | --- |
| P1 | `P_ij` phase, after timestamp+CFO correction, is stable on a static channel at ≪ 0.1 rad at weave cadence | R1: σ_φ ≳ 1 rad random, or grows without bound |
| P2 | `R_ij` channel cancellation is exact enough to track ground-truth relative LO phase to < 0.05 rad over 10 min | R1: ratio phase does not track the conducted reference, or multipath echoes leak |
| P3 | TDD channel reciprocity holds over the exchange; motion-induced non-reciprocity is correctable as a known TDM-Doppler term | R2: the product depends on which node initiates (non-separable hardware), or motion biases uncontrolled |
| P4 | The S3's per-packet CSI phase obeys the CFO/SFO model (no hidden random component) | R1: raw-phase fit residual random at ≫ 0.1 rad |
| P5 | Weave cadence ≥ 20–50 Hz per link with acceptable loss at home distances and through 1–2 walls (note 04) | throughput/loss prevents useful cadence |
| P6 | Environment-baseline normalisation keeps phase meaning (static field stable over the horizon) | R3: baseline drift ≫ target signal, uncorrectable by EWMA adaptation |
| P7 | The 20-face puck can be built without fatal mutual coupling/shadowing (note 08: unmeasured on a conducting solid) | R4: face patterns blind the array or calibration degrades |
| P8 | Coherent action (imaging/AoA) beats incoherent fusion by the predicted aperture gain | R2: coherent combination gain absent |

---

## 5. Experiments (cheapest-decisive first)

**R1 — Reciprocal Coherence Test (THE decisive experiment).**
2–3× ESP32-S3 devkits on a bench; one 2.4 GHz conducted reference (splitter +
attenuators/coax, ESPARGOS-style) as ground truth; optional RPi/Intel sniffer.
(i) Nodes alternate ESP-NOW frames at 100–500 Hz in a static room; log CSI, ns RX
timestamps, RSSI. (ii) 30 min run including a temperature step (hair dryer on one
crystal). (iii) Compute `P_ij`, `R_ij`; correct `P` by `2πΔ̂f·δt` (Δ̂f from the ratio
slope or phase history; δt from timestamps); measure residual phase σ per 10 s window.
(iv) Compare ratio-derived relative LO phase with the conducted reference. (v) Moving-
reflector test: verify product phase = 2 × one-way phase (ground truth from the
conducted reference) and doubled Doppler.
**Pass:** σ(P) < 0.1 rad; tracking < 0.05 rad; doubling confirmed. **Fail:** σ(P) ≳ 1 rad
random → the wireless-only boundary closes; fall back to conducted-coherent pucks with
incoherent fusion (still useful, but not the invention).

**R2 — Triangle closure and coherent gain.** 3–4 nodes; verify the ratio triangle
closes mod π; then (a) a common transmitter illuminates all nodes; after bus-based phase
correction, sum the corrected one-way channels and compare SNR with the incoherent sum
(predicted ~N improvement; 4 nodes ≈ 6 dB) — the first wireless distributed coherent
receive on ESP32-S3; (b) repeat with the fabric rather than the common TX supplying the
reference.

**R3 — Environment-referenced imaging.** 6–8 nodes enclosing a room (2×2 m test field);
move a reflector or a person; back-project the differential product field onto a grid
(`I(p) = |Σ_links w_l (P̃_l−1) e^{j4πf(|x_a−p|+|p−x_b|)/c}|²`); compare with (i) ground
truth and (ii) a magnitude-only fusion baseline. Falsified if the coherent image does
not beat incoherent beyond the predicted resolution.

**R4 — Puck prototype.** One 20-face S3 puck (or a 5-face partial) with the conducted
hub; verify per-face phase calibration and AoA against a turntable; measure mutual
coupling and shadowing; stretch: coherent transmit (13 dB gain check).

**R5 — Learned static-field prior.** Ray-traced generative prior for static fields
(note 19's route); test cross-room fitting with few measurements.

---

## 6. ESP32-S3 feasibility, element by element

| Element | S3 status | Basis |
| --- | --- | --- |
| CSI per frame (L-LTF + HT-LTF; int8 I/Q per subcarrier) | **ESTABLISHED** | esp-csi; ESP-IDF vendor docs |
| CSI quality vs peers | C5 > C6 > C3 ≈ **S3** > ESP32 | Espressif ESP-CSI docs |
| ESP-NOW HT frames excite CSI; ACK; identity in frame | **ESTABLISHED** | esp-csi; ESP-IDF ESP-NOW docs |
| Raw `esp_wifi_80211_tx` for the weave | works but defaults to 1 Mbps 11b (no OFDM/CSI) — use ESP-NOW MCS0 | note 07 |
| ns RX timestamp | exists (`WIFI_PKT_RX_TIMESTAMP_NSEC`, ~12.5 ns grid, latched ~20.8 µs after sampling) | note 17 |
| CFO register readout | **not documented** on S3 (ML-Track's trick is Broadcom-specific) → estimate from CSI phase history / ratio field | this proposal |
| Frame throughput | ~1250 pps demonstrated in an office (lossy); 20-slot cycle ≈ 20 ms plausible | esp-csi issue #201 |
| Two-way product phase stability | **HYPOTHESIS — the whole invention turns on R1**; 0.04 rad achieved on RPi CM4 | ML-Track |
| Ratio-based LO tracking | algebra ESTABLISHED (Argos/D-MIMO); commodity-CSI realisation HYPOTHESIS | Argos; arXiv 2509.03722 |
| Conducted 40 MHz + 2.4 GHz hub | ESTABLISHED on S2 (ESPARGOS); HYPOTHESIS on S3 (~1 board iteration); Espressif's C5 esp-crab and C3/S3 common-crystal board show the family supports it | ESPARGOS; Espressif |
| Coherent transmit from puck | HYPOTHESIS, undemonstrated on any ESP32 | D-MIMO literature for the method |
| TDM turnaround | MAC turnaround ms-class; sub-ms scheduling not guaranteed (note 09) — absorbed by the 20 ms cycle | note 09 |
| FTM ranging | S3 2.4 GHz HT20 only, metre-class (note 15) | note 15 |

**Fallbacks.** If R1 fails on S3 but succeeds on a laptop-driven Nexmon/Intel capture of
the same link, the *protocol* is fine and the deficit is ESP32 CSI firmware (revisit
later, possibly C5/C6). If R1 fails everywhere, the fabric concept reduces to its
conducted form (wired hub inside each puck; pucks fused incoherently) — the sharpness
then comes only from each puck.

---

## 7. Cost (estimates; SPECULATION on pricing)

- **R1 kit:** 2–3× S3 devkits (~US$30), splitter/attenuators/coax (~US$50), laptop.
  ~US$100 and about a week.
- **20-face puck (unit, low volume):** S3 modules US$80–120; 20 triangles + hub PCB
  (4-layer, ~0.05–0.1 m²) US$30–60 + NRE US$500–1500; clock buffer/splitter/RF-switch
  network US$15–30; hub (S3 + 100M switch) US$15–40; assembly US$50–150 →
  **≈ US$200–400**; ~US$120–200 at 1k units.
- **Fabric node:** bare S3 module + PSU, **US$5–10**.
- **Home (3 pucks + 12 nodes): ≈ US$700–1500.** For comparison, one SDR coherent array
  costs US$5–20k.

---

## 8. Transmit needed?

**Yes, for the invention as stated.** The weave is two-way by construction: no frame,
no product; no reply, no ratio. The boundary result (note 20) says receive-only
independent-LO operation cannot supply the cross-node phase *at all*, so there is no
receive-only version of the fabric. However, the amount of TX is small (one compliant
ESP-NOW frame per node per 20 ms cycle; ~1 % duty) and legal (note 07: ESP-NOW frames
under ACMA LIPD/FCC Part 15; ACMA class licence EIRP/duty respected). The puck's
coherent-transmit mode is optional (an enhancement); the sensing fabric only needs
ordinary two-way exchanges. A passive-only Aquila variant remains possible: it forfeits
cross-node coherence and keeps only per-puck coherence and fingerprints.

---

## 9. Evidence strength

| Claim | Strength (1–10) | Justification |
| --- | --- | --- |
| Product cancels LO/CFO to a static factor | **8** (on RPi/Intel), **2** (on S3) | Chronos, RTCSI, ML-Track; no S3 demonstration |
| Ratio cancels the channel; tracks LO phase | **7** (algebra + custom hw), **3** (commodity CSI) | Argos/Rogalin/D-MIMO OTA calibration; not yet a sensing observable |
| Timestamp+CFO correction residual ≈ 0.04 rad | **8** (RPi CM4) | ML-Track, measured |
| Conducted hub on ESP32-family | **8** (S2), **5** (S3) | ESPARGOS; esp-crab (C5); C3/S3 board |
| λ/2 doubled-phase physics | **7** | monostatic equivalence of the product; BLE MCS squaring note |
| Environment as per-link reference | **6** (single link), **2** (network) | LoSRef; Witrisal MPC theory needs UWB |
| Home-scale coherent aperture performance | **1–2** | SPECULATION until R2/R3 |
| Overall invention | **HYPOTHESIS (4)** | cheap decisive test specified |

---

## 10. Chaining, existing models/data, own-data

**Chaining (BRIEF §A).** The weave's phase bus *enables* coherent cross-node
processing (AoA/imaging/mapping) that is blocked without a common reference (note 20);
the conducted hub *enables* the puck's 20-face coherence and coherent transmit; the
puck *enables* local geometry/static-field estimation that the fabric *requires* for
absolute calibration; FTM self-survey *enables* the node geometry; the static field
*enables* absolute phase without a global datum; learned static priors *enable*
few-shot home fitting. Dependency chain: **weave ⇒ phase bus ⇒ coherent aperture ⇒
3D sensing ⇒ home self-model**; and **hub ⇒ local camera ⇒ mapping ⇒ better
calibration**. The highest-leverage enabler is the woven phase bus: it converts a
collection of cheap radios into an interferometer.

**Existing models/data (BRIEF §B).** SenseFi (benchmark library), MM-Fi, ESPARGOS
datasets (DaRUS; phase-coherent, ESP32-based — the best match for calibration/imaging
studies), CSI-Bench, OpenCSI (cross-room bias); Pi-RTCSI (Nexmon SD-card image) and
ML-Track's code/method for the product; pyespargos for the puck; PicoScenes/LoSRef for
the one-link reference method; ray-traced Sim2Real pipelines (note 19) for static
priors. Licences: ESPARGOS data (DaRUS, check terms), OpenCSI Apache-2.0, esp-csi
(Espressif, permissive), pyespargos (see repo). None of these is an ESP32-S3 *fabric*
dataset; domain adaptation applies as in note 19.

**Own-data / new technology (BRIEF §C).** The minimal purpose-built dataset: the R1
capture itself (two S3s, 30 min, static + moving reflector + temperature step,
conducted reference ground truth) — publishable and decisive; followed by R3's six-node
product-field capture (one room, annotated trajectories) — the first phase-coherent
COTS-CSI aperture dataset. The falsifier for the whole programme is R1's σ_φ against the
0.1 rad bar.

---

## 11. Residual unknowns

- The S3 per-packet CSI phase law (is the CFO/SFO model complete?) — R1 decides.
- Whether ML-Track's CFO-register trick has an S3 equivalent or whether CSI-history
  tracking suffices.
- ESP-NOW two-way latency distribution and jitter on S3 under the weave schedule
  (`δt` and its stability).
- Conducted hub on S3: board-level feasibility and per-channel calibration stability.
- Mutual coupling/shadowing on a solid 20-face puck (note 08).
- The static-field inverse problem's conditioning with only a few pucks after
  per-link constants are removed (how much of the room must be baselined?).
- Multipath-bias limits on absolute ellipsoid index (cross-channel phase continuity
  through 13 channel hops).
- Cross-home transfer of the fitted static prior (note 19 route).

---

## Sources (primary; DOIs/URLs as verified)

1. Shepard et al., *Argos: practical many-antenna base stations*, MobiCom 2012. DOI 10.1145/2348543.2348553.
2. Kaltenberger, Jiang, Guillaud, Knopp, *Relative channel reciprocity calibration in MIMO/TDD systems*, Future Network & Mobile Summit 2010. https://www.eurecom.fr/en/publication/4862
3. Rogalin et al., *Scalable synchronization and reciprocity calibration for distributed multiuser MIMO*, arXiv:1310.7001 (IEEE TWC 13(4), 2014).
4. Kolomvakis et al., *Over-the-air amplitude and phase reciprocity calibration for distributed MIMO*, IEEE, 2025. https://ieeexplore.ieee.org/document/11274876/
5. *Distributed MIMO with over-the-air phase calibration integrated into the TDD flow*, arXiv:2509.03722.
6. Vasisht, Kumar, Katabi, *Decimeter-level localization with a single WiFi access point* (Chronos), NSDI 2016.
7. Shi, Li, Tang, Fang, Brennan, Chetty, *Doppler sensing using WiFi round-trip channel state information*, IEEE WCNC 2023. https://discovery.ucl.ac.uk/10161602/ (IEEE 10119022).
8. Shi et al., *ML-Track: passive human tracking using WiFi multi-link round-trip CSI and particle filter*, IEEE TMC 24(6), 2025. DOI 10.1109/TMC.2025.3529897.
9. *Wi-Fi radar via over-the-air referencing: bridging Wi-Fi sensing and bistatic radar* (LoSRef), arXiv:2602.05344.
10. *ESPARGOS: an ultra low-cost, realtime-capable multi-antenna WiFi channel sounder*, arXiv:2502.09405; *ESPARGOS: phase-coherent WiFi CSI datasets*, arXiv:2408.16377; https://espargos.net/ ; https://github.com/ESPARGOS/pyespargos
11. Espressif, *Introduction to ESP-CSI Solution* (esp-crab, chip ranking, common-crystal boards). https://docs.espressif.com/projects/esp-techpedia/en/latest/esp-friends/solution-introduction/esp-csi/esp-csi-solution.html
12. ESP-IDF, *Wi-Fi vendor features* (CSI internals; `esp_wifi_80211_tx`), ESP32-S3. https://docs.espressif.com/projects/esp-idf/en/release-v6.0/esp32s3/api-guides/wifi-driver/wifi-vendor-features.html
13. Holl & Reinhard, *Holography of Wi-Fi radiation*, PRL 118, 183901 (2017). DOI 10.1103/PhysRevLett.118.183901.
14. Meissner & Witrisal, *Multipath-assisted single-anchor indoor localization in an office environment*, IWSSIP 2012. https://www2.spsc.tugraz.at/www-archive/downloads/MeissnerIWSSIP2012.pdf
15. *Mitigation of CSI temporal phase rotation with B2B calibration method*, Sensors 18(11), 2018. https://pmc.ncbi.nlm.nih.gov/articles/PMC6263436/
16. Zeng et al., *FarSense: pushing the range limit of Wi-Fi-based respiration sensing with CSI ratio of two antennas*, IMWUT 3(3), 2019. DOI 10.1145/3351279.
17. *Time and phase synchronization for distributed aperture coherent radar*, 2013 (round-trip phase transfer); Prager et al., two-way time transfer for distributed arrays; *An optimized diffuse Kalman filter for frequency and phase synchronization in distributed radar networks*, Remote Sensing 17(3), 497, 2025. DOI 10.3390/rs17030497.
18. *Spherical antenna arrays: principles, applications, and research directions*, arXiv:2604.02701; *A geodesic sphere phased array antenna*, URSI 2002, https://www.ursi.org/proceedings/procGA02/papers/p1274.pdf; *Hexagonal subarrays over Goldberg polyhedra*, Electronics 14(23), 4618, 2025. DOI 10.3390/electronics14234618.
19. Espressif esp-csi throughput issue #201 (throughput/loss data). https://github.com/espressif/esp-csi/issues/201
20. Internal: `00-synthesis.md` and notes 01–20 in this folder (esp. 07, 08, 09, 10, 12, 15, 17, 20).

