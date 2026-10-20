# 22 — Invention: The Closure Lattice

**Prompt.** `22-invention-o`. Discover the new invention that makes a home full of
cheap ESP32-S3 nodes behave as one sharp, self-calibrating, generalising 3D
sensor — or prove that no such invention exists and state the exact physical
boundary that blocks it.

Status labels: **ESTABLISHED / HYPOTHESIS / SPECULATION**. Australian English.
Hardware tier stated per claim: **ESP32-S3 / family**, **other COTS Wi-Fi**,
**SDR-grade**, **radio astronomy**. Sources are primaries; DOIs/URLs at the end.

---

## 0. Answer in one paragraph

The invention is **the Closure Lattice**: run the mesh as a *radio
interferometer*, not as a crowd of radars. Give every node both TX and RX; the
hub correlates all receivers of each illumination frame into the cross-node
**visibility matrix** `V_ij(f)`. The gauge of note `20` is then not an obstacle
but a *nuisance with a known algebraic form*: every per-node LO and RF-chain
phase appears as a per-node factor (`V_ij = u_i · v_j* · g_ij`) and cancels
**exactly** in every **closure phase** `arg(V_ij V_jk V_ki)`. Closure phases
(Jennison 1958) are the best-tested calibration-free invariants in measurement
physics — sixty years of radio-astronomy self-calibration, redundant-baseline
calibration and VLBI imaging are built on them — and **no primary source applies
them to Wi-Fi CSI**. The polyhedron puck is the *coherent core*: a conducted
clock + 2.4 GHz phase reference (ESPARGOS principle) makes its many faces one
calibrated 3D aperture that (a) supplies the one absolute-phase constraint per
component that closure cannot give, (b) is the geometry anchor and commissioning
instrument for the whole home, and (c) can transmit *structured illumination*
that the incoherent mesh cannot. "Sharp" comes not from any one puck (physics
forbids that — see §7) but from the mesh's **baselines**: a home-scale,
calibration-free Fourier-synthesis aperture whose resolution grows with
distance between nodes, with motion (Doppler/ISAR) and with learned scene
priors. Self-calibrating is then *structural* (invariants + scene fitting), not
a calibration ritual; generalisation follows because closure/visibility features
are device- and home-independent by construction.

---

## 1. Directions searched (audit)

| # | Mission direction | Verdict |
|---|---|---|
| 1 | Wireless coherence on commodity silicon | Two-way phase ranging is **standardised on commodity BLE** (Channel Sounding PBR, LO cancels in the two-way exchange; ±20 cm early results). On Wi-Fi/ESP32-S3 two-way *phase* is undemonstrated (note `11`). Verdict: usable for a coarse *phase feed*; **not** needed for imaging once closure is used. |
| 2 | Self-calibrating learned array | The *physical* self-calibration is closure + self-cal + redundant baselines (radio astronomy). Learned pieces exist for *scene* models (NeRF2, WiNeRF, RFCanvas, WiFi-JEPA) but none learns arbitrary node geometry jointly and none is closure-based. Verdict: **this is where the invention lives**; learning is the prior layer, not the calibrator. |
| 3 | Environment as shared reference → distributed aperture | Illumination wavefront *is* the shared reference per frame; node-pair separations *are* the aperture; closure is the invariant that makes the environment usable without calibration. Virtual anchors (Witrisal; MSN 2025 at 0.384 m) are the multipath-turning-into-geometry mechanism. Verdict: incorporated. |
| 4 | Sharp 3D from coarse apertures | Diffraction closes the single-puck route (space–bandwidth product, §7). Crossings: baselines, motion, multi-band, priors. Round-trip hardware asymmetry becomes the **fingerprint ledger** (§6.3). Verdict: only via the lattice. |
| 5 | Ping + unique replies + two-way products | Recomposed as a TDM illumination schedule producing the visibility tensor; unique replies give identity/de-collision; two-way products give delay seeds and chain-asymmetry fingerprints. |
| 6 | Different excitation/composition | TDM illumination round-robin; coherent TX *within* the puck (conducted reference makes face switching phase-known — the **switched coherent aperture**); ambient AP traffic as free illumination; synthetic aperture by switching. |

---

## 2. ESTABLISHED foundations

**E1. The observation model and its gauge (note `20`).** For a frame from `k`,
receiver `i`: `H_ki(f) = g_ki(f) · a_k(f) · b_i(f) · e^{j(θ_k − ψ_i)}`. All
per-node terms factor as one complex scalar per node (times, at worst, a
per-node function of frequency). Only differences are observable; absolute
cross-node phase is gauge-blocked.

**E2. Closure phases are exact invariants.** For any transmitter `k` and three
receivers `i, j, l`:
`Φ_ijl(f) = arg[ V_ij V_jl V_li ]`, with `V_ij = H_ki H_kj*`.
Every per-receiver term — per-boot RF-chain offset, per-frame CFO intercept,
per-frame STO slope, AGC phase, antenna/board ripple, even nonlinearity that
depends on received power — enters with `+1` in one factor and `−1` in another
and cancels **at every subcarrier, for arbitrary per-receiver functions of
frequency**. The transmitter's own response appears squared in each `V_ij` and
is a common factor per frame, which also cancels. What remains is the *true
multipath closure* of the scene. This is the radio-astronomy closure theorem
(Jennison 1958; Cornwell & Wilkinson 1981; the H I 21-cm closure-diagnostics
review; VLA Electronics Memo 171) transposed to a mesh with one illuminator.
*[HYPOTHESIS at ESP32-S3: that the ESP32 CSI phase obeys E1 closely enough —
this is the decisive experiment, §8.]*

**E3. Closure phase measures scene structure, not the LoS piston.** For a
single dominant path (or a single point scatterer), closure is exactly zero:
all path delays cancel around the triad. Non-zero closure requires *structure*
(two or more paths that do not close) — which is exactly what people plus walls
provide indoors. Radio-astronomy analogue: closure probes structure; a point
source has zero closure phase. Consequence: closure alone does not give node
geometry; geometry comes from **relative delays** (phase slopes across
subcarriers, gauge-free up to per-node clock constants) and from round-trip
measurements.

**E4. Self-calibration and redundant baselines are solved algorithms.** Iterative
self-cal (model image → estimate antenna gains → re-image) is the standard
imaging loop (Cornwell & Wilkinson 1981; Bayesian VLBI self-cal 2024). It is
identifiable when the number of visibilities exceeds the number of gains
(`N(N−1)/2 > 2N−1`) and the source model is sparse — standard VLBI practice.
Redundant-baseline calibration needs no source model at all when baselines
repeat (Liu & Tegmark 2010; Dillon 2020; Adari & Slosar 2021). The independent
closure count is `(N−1)(N−2)/2` per frequency; with `N = 20` and 52–114
subcarriers this is thousands of invariants per frame.

**E5. The conducted coherent core is demonstrated on ESP32 family.** ESPARGOS
(S2 planar array) distributes a **40 MHz clock plus a 2.4 GHz phase-reference
signal** over microstrip, switched in to remove PLL/analog phase ambiguity
(arXiv:2502.09405; 2408.16377). PLL phase re-randomises per boot/channel even
with a shared clock, and the reference network is what fixes it. esp-crab
(ESP32-C5) shows the two-chip single-TX/dual-RX variant.

**E6. Two-way phase ranging on commodity silicon is standardised — in BLE.**
Bluetooth Channel Sounding phase-based ranging (72 channels, 2.4 GHz) cancels
the initiator/reflector LO because both directions' phases are combined; early
implementations are ±20 cm (Bluetooth SIG; TI SWRA791 Fig. 2-1: "No impact from
Initiator/Reflector LO as their relative phase is subtracted out"). It is the
existence proof that cheap 2.4 GHz silicon can do *two-way, LO-free* carrier
phase; the mechanism is not yet demonstrated over Wi-Fi frames on ESP32-S3
(note `11`).

**E7. Wi-Fi imaging is a crowded but phase-poor field.** Wision (SenSys 2014),
Holl & Reinhard (PRL 118, 183901, 2017; single scanning receiver, holographic),
"Imaging with WiFi" (Vakalis, Gong & Nanzer, IEEE Access 2019 — *incoherent*,
fourth-order statistics, no phase coherence needed), Wiffract (MobiCom 2022),
RISimg (2021), WiCaliper (JSAC 2026 — 1.5 cm 3D *size* of near-field objects),
Wi-Fi thermal tomography with ESP32 arrays (2026). Coherent imaging is done
with instrument-grade receivers, wired references or dedicated reference
antennas; LoSRef (arXiv:2602.05344) achieves phase-coherent *single-link*
bistatic radar on COTS Wi-Fi (802.11ax 160 MHz) by aligning successive CIRs to
the LoS path — temporal, per-link, not cross-node. Multistatic radar autofocus
(Joint Self-Calibration of Receiver Geometry…, Sensors 2026; MERL TR2026-128;
Mansour et al., IEEE TCI 2018) exists on radar data; incoherent imaging without
coherence is established (Nanzer group).

**E8. Fixed-path phase error hurts beamforming badly; closure is the way out.**
Distributed transmit beamforming gain degrades with combining phase error
variance (`σ_e² = (2π t_e)² σ_f² + σ_ph² + σ_fb²`; Quitin/Fusco-style models;
Nanzer group picosecond time-alignment work needing 12 ps RMS for full gain).
Coherent operation therefore has a high *accuracy* price; closure avoids the
price entirely by dropping the coherent-sum observable *for imaging*.

**E9. Radio astronomy never solves absolute phase either.** VLBI, LOFAR, HERA,
MeerKAT all image with independent clocks; phase is fixed by geometry + a
positioning/clock model, or by closure + self-cal. The gauge of note `20` is
the same gauge radio astronomers handle daily.

---

## 3. HYPOTHESIS (the lattice's operating premises)

- **H1 (ESP32 phase model).** ESP32-S3 CSI phase at a receiver equals
  (true channel) × (per-receiver frequency-dependent factor) × (per-frame scalar
  set by CFO/AGC), with residuals dominated by thermal + quantisation noise.
  OpenCSI's model (AGC scalar; linear phase trend `φ0 + kδ`; per-boot offset;
  per-frame CFO intercept) is a *strict subset* of what closure tolerates.
  Sources: arXiv:2607.26665 ("absolute ESP32 phase unusable" for *other*
  purposes); esp-csi issue #121 (intercept = residual CFO, slope = timing);
  Zubow et al. arXiv:2005.03755.
- **H2 (visibility SNR).** With ESP-NOW HT20 (52 subcarriers; ht40 doubles) at
  ~100–1000 frames/s, per-frame per-link SNR ≫ 1 at home ranges for the direct
  path, and adequate (≥ a few dB after averaging) on the person-scattered
  component; closure dynamics of a walking person are detectable within 1 s of
  averaging. To measure.
- **H3 (identifiability).** Home mesh N ≥ 6 gives closures and delays enough to
  solve node geometry up to rotation/translation/global phase (the standard
  interferometric gauges), given: known puck intra-geometry, coarse ranges from
  FTM/two-way timing, and a sparse dynamic scene. Self-cal convergence expected
  from VLBI experience; local minima possible.
- **H4 (accuracy).** Realistic target with 8–12 nodes + 1 puck in a furnished
  home: **0.1–0.5 m 3D for a walking person at 3–6 m**, improving with tracking;
  centimetre-class *relative* interferometric precision for strong coherent
  scatterers (SNR-limited). Not centimetres for people, not optical.
- **H5 (portability).** Closure/visibility features are device-independent by
  construction, so models trained on them transfer across devices and homes far
  better than raw-CSI models (note `19`); hardware-asymmetry fingerprints
  (round-trip products) are environment-independent and therefore *transportable
  calibration*. Both testable.

---

## 4. SPECULATION (flagged)

- **S1.** Cross-home *closure atlases* (published, anonymised visibility
  statistics) become priors for zero-shot sensing in a new home.
- **S2.** The mesh doubles as a passive illuminator-of-opportunity interferometer
  using neighbours' traffic only, with Aquilanodes silent.
- **S3.** Learned "closure-to-scene" inversions (differentiable radio rendering,
  e.g. Sionna) replace explicit self-cal for real-time operation.

---

## 5. THE INVENTION — the Closure Lattice

**What it is.** A two-layer instrument. *Core*: one or more polyhedron pucks
with a conducted 40 MHz + 2.4 GHz phase reference (ESPARGOS principle, in 3D),
coherent RX *and* TX, face switching with reference re-measurement, and multiband
stitching across the 13 ISM channels. *Lattice*: the cheap nodes around the home,
independently clocked, alternately transmitting (TDM round-robin, legal duty
cycle), all others recording CSI on each frame; the hub (Ethernet/FPGA) forms
visibilities, closure phases/amplitudes, relative delays, and runs
interferometric self-calibration jointly for node geometry, per-node
gain/clock gauges, static environment map and moving targets.

**Physical mechanism.**
1. **Per-frame illuminations give the star visibility set.** One transmission,
   N−1 receivers → `(N−1)(N−2)/2` visibilities and `(N−1)(N−2)(N−3)/6`
   closure triads *at each subcarrier*, all exactly free of LO/chain phase.
2. **Round-robin gives the tensor.** Different transmitters add cross-links;
   the union over frames is a visibility tensor whose Fourier coverage equals
   the set of node-pair baselines (up to the illumination geometry).
3. **Closure = calibration-free scene data.** Non-zero closure is purely the
   multipath/scatter structure (§E3); moving people imprint time-varying
   structure; static walls imprint the static part.
4. **Self-cal fixes what closure cannot**: the per-node gains (using the sparse
   scene model) and thereby the *absolute* phases; one coherent element per
   connected component (the puck) removes the residual global d.o.f. and pins
   the image phase.
5. **Geometry from delays**: relative delays from the subcarrier phase slope of
   `V_ij` are gauge-free up to a per-node constant (clock/chain delay) and give
   the node survey; coarse ranges from FTM/two-way timing unwrap band edges;
   the puck's 3D AoA gives bearings in one shot (its faces are phase-calibrated
   by the conducted reference).
6. **Sharpness from baselines + motion + priors**: home-scale baselines set the
   main lobe; target motion synthesises ISAR-like extra aperture and separates
   by Doppler; the learned prior does sparse inversion (CLEAN/sparse-RI
   analogues).

**What is genuinely new.**
- **The transposition itself**: cross-node visibilities + closure phases as the
  *primitive observable* of a commodity Wi-Fi sensing mesh, and radio-astronomy
  self-cal/redundancy as its calibration stack. No primary source found applying
  closure/bispectrum/self-cal to Wi-Fi CSI, WLAN, or 5G localisation/imaging;
  nearest neighbours are (i) radio-astronomy closure/self-cal, (ii) incoherent
  Wi-Fi imaging (Nanzer), (iii) TDD reciprocity calibration (Argos-class), (iv)
  LoSRef's single-link temporal referencing, (v) multistatic radar autofocus
  (radar hardware, coherent).
- **The coherent core + incoherent mesh hybrid**: a conducted-coherent 3D puck
  providing the *minimal* absolute reference, geometry anchor and structured
  illumination, glued to a calibration-free sparse mesh by invariants.
- **The "invariants-first" reframing of self-calibration**: the mesh is
  self-calibrating *because its observables are invariants*, not because a
  calibration procedure succeeded.
- **Volumetric switched coherent aperture**: the conducted reference makes face
  switching phase-known, so a *single* radio can synthesise a 20-element
  coherent 3D aperture by TDM switching; multiband stitching makes the switch
  phase traceable across channels. (Mechanism established on ESPARGOS planar;
  new in 3D, multiband and mesh-integrated.)

**What must be true.**
- H1 (per-receiver phase structure), H2 (SNR), H3 (identifiability), H4
  (accuracy target), plus:
- **W1.** Frame association across receivers is reliable (ESP-NOW sequence +
  ns RX timestamp; note `17`).
- **W2.** Enough illuminations per second: with 8 nodes round-robin at 50
  frames/s each node transmits ~6/s; closure datasets at ~2–60 Hz per triad
  after scheduling. Legality: LIPD/FCC duty cycle for the schedule (note `07`).
- **W3.** Static scene changes (doors, furniture) are part of the model, not
  calibration error — the self-cal scene model must be adaptive (it is: motion
  is the signal).

**Falsifying experiment (the single decisive one).** The **Closure Gate**:
four ESP32-S3 nodes (A transmits ESP-NOW HT20 frames at ~200/s on a fixed
channel; B, C, D receive the same frames; a laptop computes `V_ij` and
`Φ_BCD(k,t)` for 10 minutes). Tests, in order: (1) invariance — closure is
stable across AGC settings, receiver reboots and channel changes while raw
phases jump; (2) physics — moving a known metal plate produces the
geometrically predicted closure shift; (3) signal — a walking person produces a
reproducible closure signature and closure residuals remain at the noise floor
for a static room. **Falsified if** closure is not stable/invariant, or tracks
device state rather than geometry, or carries no motion signature. Cost < $40,
one day. Every claim in §5 inherits its truth value from this gate.

**ESP32-S3 feasibility.** TX: `esp_wifi_80211_tx`/ESP-NOW frames (note `07`);
CSI on all receivers (esp-csi; HT20 52 subcarriers, HT40 114); ns RX timestamp
for association; hub computes (RPi 4/5, mini-PC or small FPGA). No firmware
exotica. The *core* needs the S3 clock/phase-reference injection verified
per-board (S2 documented; S3 boards unaudited — known risk, note `11`).

**Cost.** Node $3–5; 6–12-node mesh $30–60; hub $50–150; one prototype puck
$150–300 (20 modules + RF switch + reference network + PCB); production puck
target $100. Software: ports of open CSI stacks + new estimator.

**TX needed?** For the *full lattice*, yes (round-robin illumination, two-way
calibration, puck coherent TX). For the *minimum viable closure instrument*, no:
**three receivers + any transmitter** (even a neighbour's AP) already produce
visibilities and closures.

---

## 6. Supporting inventions (each with the required fields)

### 6.1 The Switched Coherent Polyhedron (core)

- **New**: volumetric (20-face) coherent aperture on ESP32-class silicon with a
  conducted clock + phase reference and reference-calibrated face switching;
  multiband stitching across ISM channels with switch-phase correction.
- **Mechanism**: PLL phase re-randomises per boot/channel; the reference tone is
  switched in beside each antenna to measure the chain/switch phase *when it
  matters*; TDM across faces (and TDM MIMO across TX faces) synthesises a
  20-element 3D aperture plus a 20×20 virtual MIMO co-array inside 30 cm
  (face spacing ≈ 16 cm ≈ 1.3λ).
- **Must be true**: reference distribution stable across the soldered 3D
  structure; channel-change phase jumps measured and corrected; mutual coupling
  calibratable (spherical arrays and coupling corrections are established
  theory: URSI geodesic GDPAA; spherical-array reviews 2026; VSWF coupling
  models 2025).
- **Falsifier**: phase repeatability of the switched aperture < ~0.2 rad over
  30 min and across channels, measured against a cabled reference; else the
  core degrades to a non-coherent 3D amplitude array.
- **S3 feasibility**: same as ESPARGOS + board audit; **TX needed**: yes for
  coherent illumination/MIMO; RX-only mode possible with reference only.
- **Cost**: $150–300 prototype.

### 6.2 Round-robin visibility protocol with unique replies

- **New**: the *scheduling+association* design that turns the BRIEF's ping +
  unique reply into a visibility tensor with per-frame closure availability;
  identity via unique frames; two-way products folded in.
- **Mechanism**: rotate transmit grants; every grant yields a star visibility
  set; alternate grants with paired two-way exchanges (A→B, B→A) to harvest
  delay and asymmetry products.
- **Must be true**: W1, W2; de-collision.
- **Falsifier**: frame association failure rate or scheduling overhead makes
  > 20 % of closures unusable.
- **S3 feasibility**: trivial firmware; **TX needed**: yes (or ambient).
- **Cost**: firmware only.

### 6.3 The Reciprocal Fingerprint Ledger

- **New**: treat two-way products `ĥ_ij ĥ_ji` as a *transportable hardware
  ledger*: chain asymmetry + round-trip delay, environment-independent,
  measured in milliseconds on demand, stored across reboots, bands and homes;
  it converts one-way CSI into reciprocity-calibrated CSI and seeds geometry
  (delays) and coherent TX (TDD pre-compensation).
- **Mechanism**: in the two-way product the LO terms reduce to
  `e^{j2π Δf_ij Δt}` with `Δf_ij` estimated per exchange, hence removable;
  the residual is hardware + `2τ_ij` (note `20`, §round-trip; Argos/reciprocity
  calibration; hardware drift < 2.6 % over 4 h per note `10`).
- **Must be true**: CFO estimate accuracy × turnaround jitter small (jitter
  tolerance scales inversely with CFO error; with `Δf` known to ~1 Hz the
  tolerance is milliseconds — the MAC's ms turnaround is acceptable); ESP32 TX
  and RX share their LO within a node.
- **Falsifier**: ledger coefficients drift faster than a home season or fail to
  reproduce after reboot/channel change.
- **S3 feasibility**: HYPOTHESIS (undemonstrated, note `11`); **TX needed**:
  yes; **cost**: firmware.

### 6.4 Scene-Sharpness Autofocus (fallback refinement)

- **New**: transplant of radar autofocus (PGA, min-entropy, sparsity-assisted,
  deep-unfolded; Wahl 1994; MERL TR2026-128; Mansour TCI 2018; Sensors 2026
  multistatic joint self-cal) to CSI visibilities, to refine per-frame phase
  terms where closure-only imaging underperforms and to enable coherent TX
  phase pre-compensation.
- **Mechanism**: maximise image sharpness / minimise entropy over per-node phase
  corrections; sparse scenes give a usable basin.
- **Must be true**: enough SNR and scene sparsity for a sharpness maximum at
  zero error; initialisation from closure/self-cal.
- **Falsifier**: sharpness objective not correlated with true phase error at
  ESP32 CSI SNR.
- **S3 feasibility**: host-side; **TX needed**: no (works on existing
  visibilities); **cost**: compute only.

### 6.5 Virtual-Anchor Environment Mesh

- **New**: use the coherent core's 3D AoA to *associate* multipath components to
  physical surfaces (the hard part of virtual-anchor mapping), converting walls
  into calibrated reference points for the lattice; MSN 2025 shows 0.384 m
  virtual-anchor mapping even at limited bandwidth and small arrays.
- **Mechanism**: geometric delays of specular paths → mirrored anchors; the core
  resolves association ambiguity in 3D; lattice closures track them over time.
- **Must be true**: specular dominance on walls at 2.4 GHz indoors (partially
  true; diffuse component is the limiting factor — SALOS/UWB literature).
- **Falsifier**: anchor map unstable or uninformative vs diffuse multipath
  indoors at 20 MHz.
- **S3 feasibility**: host-side; **TX needed**: no; **cost**: compute only.

---

## 7. The physical boundary and the minimal reference

**B1 — What closes absolute cross-node phase.** The per-node complex gain
`g_n = a_n b_n e^{j(θ_n − ψ_n)}` (the `U(1)` + time/frequency gauge of note
`20`) closes: absolute cross-node phase, phase-of-flight ranging, and coherent
transmit beamforming *gain*. **This boundary does not close sensing.** The
invention routes around it: closure phases are functions of the *data* that are
provably insensitive to `g_n`, and self-cal recovers the scene and the gains
*up to the gauge* with no reference. **Minimal reference that crosses it** (for
coherent TX and absolute phase): one coherent element per connected component —
the conducted puck (`O(1)` conductor per component; ESPARGOS-demonstrated on the
family) or one common-TX observation per boot (star-topology CRB-optimal, but
drift-limited; notes `10`, `11`, `20`). Cost `O(N)` at most.

**B2 — What closes "sharp 3D" from one 30 cm puck at 2.4 GHz.** The
**space–bandwidth product of the wavefield**: `SB = (D/λ)² · (B · T_int)`.
With `D = 0.3 m`, `λ = 0.125 m` (`D/λ = 2.4`), `B ≤ 83.5 MHz` (the entire ISM
band, requiring coherent multiband stitching) and `T_int` bounded by target
motion, one puck supports of order `10³` independent scene samples at 3 m —
angular Airy width `≈ 1.22 λ/D ≈ 29°`, range resolution `c/2B ≥ 1.8 m` with
every 2.4 GHz channel stitched. **No algorithm stably beats this**; sparse
super-resolution buys a factor ~5–10 only with high SNR and scene sparsity
(established in array processing/CS). The blunter versions: one ESP32-S3 HT20
alone gives 7.5 m range bins and ~29° angles. **Minimal crossings, in
increasing cost:** (i) *baselines* — the lattice's node-pair separations extend
`D` to home scale (10 m ⇒ main lobe ≈ 0.7°); (ii) *motion* — Doppler/ISAR
synthesis; (iii) *priors* — learned sparse inversion on invariant features;
(iv) *hardware* — dual-band C5/C6 at 5–6 GHz with 160 MHz (range ≈ 0.9 m,
`λ ≈ 5–6 cm`) or wider-band silicon. Nothing crosses B2 for an *instantaneous*,
*single-aperture*, *prior-free* centimetre 3D: that space is closed, exactly
as optics would be with a 2.4λ lens.

**B3 — The honest claim.** A home full of $5 nodes becomes *sharp relative to
its class*: decimetre-scale 3D tracking of people, sub-degree angular precision
on strong coherent scatterers, and device-independent operation — sharp, not
optical.

---

## 8. Decisive experiment programme (cheapest-decisive first)

1. **The Closure Gate** (§5) — gates the entire invention. 4×S3, one day.
2. **Switched-aperture repeatability** — 1× S3 + 20-way RF switch + cabled
   2.4 GHz reference tone; measure aperture phase repeatability across sweeps,
   reboots, channels. Falsifies the core if > ~0.3 rad RMS.
3. **Self-cal simulation** — Sionna ray-traced home + synthetic ESP32 phase
   model (per-receiver nuisance functions); test geometry+scene recovery and
   the H4 accuracy target before hardware. Output: identifiability/threshold
   curves.
4. **Mesh field trial** — 6–8 nodes + 1 puck bench prototype in a real home;
   moving-person localisation vs ground truth; report closure residuals,
   latency, duty cycle.
5. **Ledger characterisation** — two-way asymmetry stability over days, reboots,
   channels; CFO-residual-limited phase precision of the two-way feed.

---

## 9. ESP32-S3 feasibility summary

| Element | Status | Evidence |
|---|---|---|
| Per-frame CSI at many receivers (52/114 subcarriers) | Demonstrated (esp-csi) | notes `01`,`05`,`07` |
| TX of compliant frames / ESP-NOW excitation | Demonstrated | note `07`; synthesis §8 |
| ns RX timestamp, frame association | Demonstrated | note `17` |
| Per-receiver phase model (H1) | **Hypothesis — the gate** | arXiv:2607.26665; esp-csi #121 |
| Conducted clock + 2.4 GHz reference on family | Demonstrated (S2 planar = ESPARGOS; C5 = esp-crab); S3 boards unaudited | arXiv:2502.09405, 2408.16377 |
| Volumetric switched aperture | No ESP32 volumetric array exists | synthesis §7; URSI geodesic arrays (theory) |
| Wireless two-way phase on S3 | Undemonstrated | note `11` |
| Hub compute (self-cal, imaging) | Trivial for RPi/mini-PC on this data volume | — |

---

## 10. Cost

Nodes $3–5 each; a 6–12-node lattice $30–60; hub $50–150; prototype 20-face
puck $150–300; software mostly adaptation of existing open stacks. A
single-puck, closure-free "presence + motion" product exists at <$10; the
lattice is deliberately a $200-class whole-home instrument.

---

## 11. TX needed?

- **Closure imaging**: no — 3 receivers + any transmitter (including third-party
  AP traffic).
- **Full lattice** (geometry, scheduling, coherent transmit, best SNR): yes —
  round-robin ESP-NOW/`esp_wifi_80211_tx` frames.
- **Core puck**: yes for coherent illumination/MIMO TX; the reference network
  itself is conducted and non-radiating.

---

## 12. Evidence strength

- Closure/self-cal physics and practice: **10/10** (60+ years, operational).
- Transposition to Wi-Fi CSI: **no prior found; derivation exact** — call the
  *idea* 7/10 evidence (theory exact, ESP32 phase model unproven).
- ESP32-S3 realisation: **HYPOTHESIS, 4–5/10** until the Closure Gate passes.
- Overall invention: **6/10 pending the gate; 8–9/10 if the gate passes.**

---

## 13. (A) Chaining — dependencies

1. **Closure Gate (E2/H1) ⇒ everything.** No lattice without it.
2. **Conducted coherent core ⇒ absolute phase + coherent TX + structured
   illumination ⇒ (with closure) full lattice.** The highest-leverage enabling
   capability, because it is the *cheapest* path to the one gauge-fixing
   constraint per component and the only demonstrated ESP32-family route.
3. **Round-trip two-way products ⇒ geometry seeds + transportable ledger ⇒
   faster self-cal convergence.**
4. **Baselines + self-cal ⇒ sharpness**; **motion ⇒ ISAR/Doppler diversity ⇒
   robustness in multipath**; **learned priors ⇒ generalisation across homes.**
5. **Ambient-traffic operation ⇒ zero-downtime sensing**; **own TX ⇒ full
   coverage and geometry.**

## 13. (B) Existing models, data and tools

- **Radio-astronomy stack**: closure/self-cal literature, CLEAN/sparse-RI,
  redundant calibration (Liu & Tegmark 2010; Dillon 2020; Adari & Slosar 2021;
  VLBI Bayesian self-cal 2024) — directly portable mathematics.
- **ESPARGOS datasets/code** (espargos.net; arXiv:2408.16377) — phase-coherent
  CSI for validating invariants (phase-synchronised, so they do *not* exercise
  the gauge; still the best method reference).
- **esp-csi / OpenCSI** — acquisition, sanitisation, per-boot phase model
  (github.com/espressif/esp-csi; arXiv:2607.26665).
- **BLE Channel Sounding** — the two-way LO-cancellation precedent (Bluetooth
  Core 6.1 Part H; TI SWRA791).
- **Learned wireless fields / transfer**: NeRF2 (MobiCom 2023), RFCanvas
  (SenSys 2024), WiNeRF (EWSN 2026), WiFi-JEPA (2026), AutoFi, RePos
  (arXiv:2607.02986), MM-Fi/CSI-Bench/SenseFi — the prior layer *on top of*
  invariant features.
- **Incoherent imaging** (Nanzer IEEE Access 2019) and **multistatic radar
  autofocus** (Sensors 2026; MERL TR2026-128) — contrast and fallback methods.
- Licences: esp-ppb GPL-3.0 (careful); ESPARGOS code/datasets per repo;
  esp-csi Apache-2.0; MM-Fi etc. per note `19`.

## 13. (C) Own-data / new technology

- **New data**: a **closure-phase corpus** on ESP32-S3 (static and dynamic
  scenes, reboots, channel changes, AGC sweeps) — nobody has published one; it
  directly tests H1/H2 and is publishable regardless of outcome.
- **New experiment**: the Closure Gate (§8.1) and the switched-aperture
  repeatability test (§8.2).
- **New artefact**: the visibility-tensor + self-cal reference implementation
  for Wi-Fi meshes (the "Aquila interferometer" software).
- **Falsification path**: if the gate fails, the fallback is the tolerance-
  bounded common-TX/autofocus route (notes `10`, `11`, `20`), i.e. a
  coherence-hardware instrument with a measured error budget — still a valid
  v2 outcome, but a different product.

---

## 14. Residual unknowns

1. ESP32-S3 closure residual (the gate) — magnitude and spectrum.
2. ESP32-S3 AGC/phase interaction *between* frames and channels under
   round-robin scheduling.
3. Bistatic scattering of people at 2.4 GHz, and how much closure SNR survives
   diffuse (non-specular) scattering.
4. Self-cal convergence and local minima at home-scale SNR and geometry.
5. Mutual coupling and near-field effects in a soldered 30 cm 20-face puck.
6. S3 board clock/phase-reference injection feasibility (board audit).
7. Legal duty-cycle ceilings for the round-robin at desired rates (note `07`).
8. Whether the ledger's two-way phase precision survives the Wi-Fi MAC's
   turnaround on S3.

---

## 15. Primary sources

- Jennison, R.C. (1958), closure phase — via Cornwell & Wilkinson (1981),
  "A new method for making maps with unstable radio interferometers", MNRAS.
- H I 21-cm Cosmology and the Bispectrum: Closure Diagnostics in Massively
  Redundant Interferometric Arrays (2018), Radio Science,
  doi:10.1029/2018RS006537.
- Clark, B.G., "Closure — some examples", VLA Electronics Memo 171 (1978).
- Liu, A., Tegmark, M. et al., "Precision calibration of radio interferometers
  using redundant baselines", arXiv:1001.5268; Dillon et al. 2020 (HERA);
  Adari & Slosar, "Generalized redundant calibration", arXiv:2107.10186.
- "Bayesian self-calibration and imaging in VLBI", A&A 2024,
  aanda.org 10.1051/0004-6361/202449663 (self-cal d.o.f. condition).
- Bluetooth SIG, Channel Sounding — bluetooth.com feature page; Core 6.1 Part H;
  TI SWRA791 ("no impact from Initiator/Reflector LO…").
- ESPARGOS — arXiv:2502.09405; arXiv:2408.16377; espargos.net.
- Wi-Fi Radar via Over-the-Air Referencing (LoSRef) — arXiv:2602.05344.
- Joint Self-Calibration of Receiver Geometry, Timing, and Target Positions for
  Multistatic Radar Autofocus — Sensors 26(15):4954 (2026).
- MERL, "Deep-Unfolded Autofocus Imaging for Distributed MIMO Radar",
  TR2026-128; Mansour et al., IEEE TCI 4(4), 2018.
- Wahl et al., "Phase gradient autofocus", IEEE TAES 30(3), 1994.
- Vakalis, Gong, Nanzer, "Imaging With WiFi", IEEE Access 7 (2019),
  doi:10.1109/ACCESS.2019.2902315; Holl & Reinhard, PRL 118, 183901 (2017);
  Huang et al., "Feasibility and limits of Wi-Fi imaging" (Wision), SenSys 2014.
- Wiffract, MobiCom 2022, doi:10.1145/3495243.3514261; WiCaliper, IEEE JSAC
  2026, doi:10.1109/JSAC.2025.3609312.
- Meissner & Witrisal, multipath-assisted single-anchor localisation (2012);
  SALOS, Sensors 24(8):2428 (2024); "Robust Mapping of Virtual Anchors…",
  IEEE MSN 2025, doi:10.1109/MSN69125.2025.00070.
- Rogalin et al., arXiv:1310.7001; Zhu/Jiang CRLB arXiv:1709.07206;
  arXiv:2606.14419; arXiv:2509.03722; Brown & Poor, IEEE TSP 56(11), 2008,
  doi:10.1109/TSP.2008.927073.
- Zubow et al., "On Phase Offsets of 802.11ac Commodity WiFi", arXiv:2005.03755.
- OpenCSI, arXiv:2607.26665; esp-csi github.com/espressif/esp-csi (issue #121);
  ESP-CSI techpedia (esp-crab), Espressif.
- Nanzer group, "Decentralized Picosecond Synchronization…", arXiv:2405.18384;
  "Cooperative Coherent Multistatic Imaging and Phase Synchronization in
  Networked Sensing", arXiv:2311.07212.
- URSI, "A geodesic sphere phased array antenna…" (2002); Frontiers in Antennas
  and Propagation, "Spherical Antenna Arrays: Principles…" (arXiv:2604.02701);
  VSWF sparse-array optimisation, arXiv:2509.15636.
- IEEE 802.11bf — "Toward ISAC in IEEE 802.11bf Wi-Fi Networks",
  arXiv:2212.13930.
- NeRF2 (MobiCom 2023); RFCanvas (SenSys 2024); WiNeRF arXiv:2609.22132;
  WiFi-JEPA arXiv:2607.11064; RePos arXiv:2607.02986.
- Cross-references to this project: notes `01`–`20` in this folder (esp. `07`,
  `10`, `11`, `12`, `17`, `19`, `20`) and `00-synthesis.md`.

---

## 16. One-line answer

**The invention is the Closure Lattice: give every cheap ESP32-S3 TX and RX,
let the hub correlate each illumination into cross-node visibilities, and read
the scene from *closure-phase invariants* — which cancel every LO and RF-chain
phase exactly — with the conducted-coherent polyhedron providing the single
reference, the 3D anchor and coherent illumination; the physics that closes
naive "sharp 3D" from one puck is the 2.4 GHz space–bandwidth product
(`(D/λ)²·B·T`), crossed only by home-scale baselines, motion and priors; and the
single decisive experiment is a four-node ESP32-S3 closure test.**
