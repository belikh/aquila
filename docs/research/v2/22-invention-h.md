# 22 — The invention: the Closure Weave

**Agent 22 (inventor-researcher). Status: proposal. Australian English.**

**Verdict in one paragraph.** No new physics is needed, and none is available: the 20-note
synthesis already proves that cross-node carrier phase is unobservable from independent-LO
reception (a `U(1)` gauge per link), and it already names the three escapes (common
transmitter, OTA reciprocity, conducted reference). The invention is therefore not a new
escape. It is a **two-tier observable and instrument** that (A) *annihilates* the gauge
instead of crossing it — the **closure tier**, an always-on, synchronisation-free sensing
mode built from phase products around loops of links, which I derive here to be a per-node
range-change observable with a 2× sensitivity gain and a 4-node minimality result, and
(B) *crosses* the boundary once, cheaply, with the polyhedron as the **anchor** — the
**anchored tier**, which turns the whole home into one near-field coherent aperture whose
depth comes from wavefront curvature rather than bandwidth. Together they are **the
Closure Weave**. The decisive question is empirical and singular: whether the per-packet
element phase on ESP32-S3 is continuous (trackable) or random — the M2 residual the
synthesis already flagged. One six-node bench experiment decides it.

---

## 1. What this note takes as given (and what it updates)

From `00-synthesis.md` (not re-derived here):

- **The boundary (`20`).** `ĥ_ij = g_ij·a_i·b_j·e^{j(θ_i−ψ_j)}`; only `θ_i−ψ_j` is
  observable; on a connected graph the node phases are fixed only up to one global additive
  constant. Absolute cross-node phase/AoA is unobservable; per-receiver differential phase,
  delays, amplitudes, Doppler and two-way products are observable. **Minimal reference:**
  one common-transmitter broadcast, one reciprocal exchange per link (a star is
  CRB-optimal), or a shared clock+tone; cost O(N).
- **The numbers (`01`,`06`,`08`,`17`,`20`).** At 20 MHz, range resolution `c/2B` ≈ 7.5 m
  (round trip ≈ 3.75 m); a home is one delay bin. Turnaround vs coherence: raw CFO wraps
  relative phase in 5–10 µs; after CFO removal, ms–tens of ms (MegaMIMO: 100 Hz residual →
  π in 20 ms). ESP32 MAC exposes a nanosecond RX timestamp; no TX timestamp. The
  unmeasured **M2 residual** (post-CFO phase residual over the exchange interval) governs
  every wireless escape.
- **The polyhedron (`08`,`09`).** A soldered-PCB icosahedron is a plausible 20-face 3D
  aperture; established analogue: AFRL's geodesic-dome phased array (triangular planar
  subarrays in an icosahedral arrangement) [S20]. A shared clock removes cross-face PLL
  ambiguity but does not remove face-TDM; mutual coupling/shadowing unmeasured; no ESP32
  volumetric array exists.

Updates since the synthesis, verified here:

- **ESPARGOS is exactly the polyhedron's inner layer.** Eight ESP32-**S2** receivers on one
  four-layer FR4 board; daisy-chained 40 MHz reference + a conducted **WiFi-band phase
  reference** (2.4–2.5 GHz) frequency-multiplexed onto one coax, split to the PLLs via an
  RF switch; boards combine into a 4×8 coherent array; receive-only; calibration cancels
  PLL phase (re-randomised per reset/channel) and distribution-line delays [S1,S2,S3].
  ESP-SDR adds phase-coherent raw IQ and **supports ESP32-S3** [S4]. So the polyhedron
  premise is one design-generation away from a demonstrated part.
- **Wireless (cable-free) coherence on the ESP32 family is now demonstrated.** ESP-PPB is
  an open-source (GPL-3.0 / CERN-OHL-S-2.0) ESP32-**C3** platform: an FTM master plus a
  VCTCXO disciplined by dual DACs (sub-PPB clock alignment), and — crucially — **reference
  frames sent by the master 1 ms after each CSI frame**, so slaves track and remove their
  own receive phase. Reported single-frame phase accuracy <1° best, <10° typical; AoA
  functional; no cables [S5]. This is the common-transmitter escape, implemented on
  commodity silicon, with hobbyist tools, in 2026. It post-dates the synthesis.
- **ESP32 CSI phase is better behaved than the Intel-NIC folklore.** The commodity-NIC
  literature models carrier phase offset as random per packet and packet-detection delay as
  random per packet [S21]. On ESP32, however: the subcarrier phase *slope* (detection
  delay/timing) is "very consistent over packets", while the *intercept* drifts with
  residual CFO [S6]; Espressif's own C3+**S3** motion board shares a clock buffer between
  the two chips precisely to use phase [S7]. And a 2026 cross-platform study that includes
  the Espressif ESP32 finds receiver-induced nonlinear distortions "effectively static over
  days" and calibratable [S8]. The per-packet element phase question is therefore *open
  and favourable*, not closed — which is exactly what makes it the decisive experiment.

**Where the synthesis's verdict needs one amendment.** "Wireless-only escapes not proven on
ESP32-S3" should read: *not proven on S3 specifically; now demonstrated on C3 (ESP-PPB) and
S2 (ESPARGOS conducted); S3 has the same CSI, a supported raw-IQ path, and no known
blocker.* That reopens the coherent route at commodity cost.

---

## 2. The physics that frames any invention

| Quantity | Value at 2.4 GHz (λ = 12.5 cm), 20/40 MHz | Consequence |
| --- | --- | --- |
| Range resolution (one-way `c/B`) | 15 m / 7.5 m | A home is a single delay bin: **no depth from delay, ever, on this spectrum.** |
| Round-trip range resolution `c/2B` | 7.5 m / 3.75 m | Same wall for monostatic schemes. |
| Polyhedron aperture `D` = 0.3 m | beamwidth ≈ λ/D ≈ 24°; Fraunhofer `2D²/λ` ≈ 1.4 m | Far-field angle sensor for a room at 2–5 m: coarse but full-sphere, no scan loss. |
| Home mesh baseline `D` = 5–10 m | beamwidth ≈ λ/D ≈ 0.7–1.4° | **The sharpness lives in the home, not the puck.** |
| Fraunhofer of the home mesh `2D²/λ` | 400–1600 m | A 4–8 m target is **deep in the near field**. |
| Depth from curvature (paraxial) | `δr = (4λr²/πD²)·σ_φ` → with D = 8 m, r = 4 m, σ_φ = 0.1 rad: **≈ 4 mm** | With phase coherence, depth comes from wavefront curvature, not bandwidth. |
| Doppler per Hz | `v = λ·f_d/2` ≈ 6.3 cm/s | Rich motion observables at packet rates of 100–1000 Hz. |
| Thermal phase floor (MCRB) | `σ_φ ≈ 1/√(2N_s ρ)` → 30 dB, 56 subcarriers: ≈ 3 mrad | Thermal noise is never the limit; **nuisance phase is**. |

The design consequence: a 30 cm polyhedron cannot be a "sharp 3D sensor" alone
(24° beam, 1.4 m near-field boundary, no range). **Sharpness is a property of the home-scale
coherent aperture**; the polyhedron is the phase reference, the coherent 3D transceiver and
the near-field anchor, not the imager. Any invention that must make the *house* sharp has
to obtain either (i) coherence across the house, or (ii) coherence-free observables with
wavelength-scale information. The Closure Weave does both.

---

## 3. The new mechanism: closure observables (Tier A)

### 3.1 Derivation

Write the measured phase of the directed link `i→j` at time `t` (single subcarrier, after
slope removal), collecting the static-channel phase `χ_ij`, the transmitter's LO phase
`θ_i(t)`, the receiver's LO phase `ψ_j(t)`, hardware chain terms and the propagation:

    φ_ij(t) = χ_ij + a_i + b_j + [θ_i(t) − ψ_j(t)] + (2π/λ)·(r_i,p(t) + r_p,j(t)) + n

with `p` a moving scatterer and `r_x,p` the distance from node `x` to it. Form the
**directed-triangle closure** from three links of a cycle `i→j`, `j→k`, `k→i`, measured in
three consecutive schedule slots:

    C_ijk = φ_ij + φ_jk + φ_ki

Every node contributes once as transmitter and once as receiver, so the per-node LO phases
appear with opposite signs. Using `θ_x(t) = 2π f_x t + θ_x,0` and that TX and RX share one LO
per node (TDD), the `θ_x,0` cancel exactly and the residual is the **relative-frequency
ledger**:

    C_ijk(t1,t2,t3) = [χ_ij+χ_jk+χ_ki] + 2π{ (f_j−f_r)(t2−t1) + (f_k−f_r)(t3−t2) }
                      + (4π/λ)·( s_i + s_j + s_k ) + n

where `s_x(t) = r_x,p(t) − r_x,p(t_ref)` is the path-length *change* of node `x` to the
scatterer, `f_r` any reference frequency (the common absolute frequency cancels), and the
ledger term vanishes for a simultaneous cycle or is removed with per-link relative-CFO
estimates — which **every 802.11 receiver already computes per packet**.

Three consequences, each checkable by hand:

1. **The gauge is annihilated, not estimated.** Per-node gains, phase offsets, sampling
   offsets, packet-detection delays and the PLL initial phase are *element-based*: they
   enter `C_ijk` as `(x at TX instant) + (x at RX instant)` pairs that cancel, up to the
   ledger. No common reference, no beacon, no reciprocity, no cable is required. This is
   the VLBI **closure phase** (`Jennison 1958` [S9]; modern statement: closure quantities
   are immune to any calibration that is separable into antenna-based gains [S10,S11]).
2. **Double sensitivity.** Each node appears in two of a triangle's three legs, giving the
   factor 2 in front of the path sum — the same 2× phase gain a round-trip radar gets, but
   *without any turnaround* and without any two-way exchange.
3. **Inversion.** With `N ≥ 4` nodes and all triple closures, the linear map
   `s ↦ {s_i+s_j+s_k}` is **injective** on `R^N`: for `N=4`, closures determine each
   `s_i` exactly (for `N=3`, only their sum). The recovered per-node range changes are then
   ordinary **true-range multilateration** [S22] — a solved problem — so **four
   unsynchronised nodes suffice to track a scatterer in 3D**, and `N > 4` over-determines
   it. This is the sharpest single result of this note: the gauge-free observable is not
   merely a detector, it is a *multilateration input*.

Amplitude closure (`|h_ij|·|h_jk|·|h_ki|`-style products, Twiss closure [S10]) gives the
same gauge immunity without phase, at lower sensitivity: a built-in fallback channel.

### 3.2 What closure can and cannot see

- **Can:** any change that alters the sum of paths around loops — people walking, posture,
  doors, falling objects, pets; with enough subcarriers, per-subcarrier closure gives
  frequency diversity and multi-scatterer separation; temporal derivatives give Doppler.
- **Cannot:** the static room (closure of the static field is a constant of the home), and
  absolute phase. A static object that never changes is invisible to Tier A; a *new* static
  object is a *change* and is visible once (then absorbed into the new constant).
  This is not a defect: it is the price of never needing synchronisation, and it maps
  exactly onto the sensing jobs that matter (occupancy, activity, events, safety).
- **Noise floor (the empirical crux):** if ESP32 per-packet element phase is continuous and
  CFO-tracked, `σ_C ≈ √3·σ_link` (three links complete in 3–6 ms); with `σ_link ≈ 0.1 rad`
  the range-sum precision is `λσ_C/4π ≈ 1 mm`. If instead there is a genuinely random
  per-packet carrier phase offset `σ_CPO` on both TX and RX, then `σ_C ≈ √6·σ_CPO` and the
  closure floor saturates at ~1–2 rad — dead. **One measurement separates these worlds**
  (§7).

---

## 4. The invention: the Closure Weave

### 4.1 Tier A — the weave (always on, synchronisation-free)

**Mechanism.** A round-robin schedule of short OFDM frames (ESP-NOW/HT20, MCS0 — the
primitive esp-csi itself uses; raw `esp_wifi_80211_tx` 11b frames carry no CSI, per the
synthesis). Each node broadcasts once per cycle; every other node extracts CSI, so one slot
yields `N−1` link phasors sharing the transmitter's instantaneous phase. Full mesh at
1–2 ms/slot:
`N=6 → full cycle 6–12 ms; directed triangle = 3 slots`. Compute the closure tensor
`C_ijk(k_subcarrier, t)`, subtract its own static baseline (self-reference), apply the
relative-CFO ledger, and invert triple sums to per-node `s_i`; multilaterate movers.

**Genuinely new.** Closure-phase sensing for commodity wireless meshes: no prior art found
in this search square (the ancestors are VLBI closure [S9,S10,S11] and range-sum
multilateration [S22]; the port, the element-based-impairment argument, the triple-sum
inversion and the 4-node minimality are, to our search, new). It *dissolves* the boundary
of note `20` rather than crossing it.

**Must be true.** (T1) per-link phase slope stable and intercept trackable on S3 (evidence:
[S6], strong); (T2) per-link relative CFO estimated to ≲ few Hz and stable over a cycle
(standard 802.11 CFO tracking, plausible); (T3) closure floor below signal (open — M2);
(T4) no baseline-random impairment (e.g. per-packet amplitude/AGC resets couple into
phase; unlikely, checkable). Falsifier: closure floor grows like `√6·σ_CPO` with no
reduction under faster schedules/reference tracking.

**TX needed?** Yes, but never *coherent* TX — each node transmits alone in its slot. No
round trips, no turnaround constraint, no beacon duty.

**ESP32-S3 feasibility.** Cognitive: CSI in promiscuous mode (HT20/HT40) is the standard
esp-csi primitive; per-packet ns RX timestamps for slot alignment [S23]; all processing is
host-side (Rust/Python). Rate: `N=8` at 1 ms slots → 125 Hz/link; ESP-NOW frames are tiny
but the ESP32 airtime at MCS0/20 MHz for ~100-byte frames is ~100–200 µs, so 1 ms slots
are comfortable; a 1% duty per node. Cost: pucks at $3–5.

### 4.2 Tier B — the anchored weave (one polyhedron makes the home coherent)

**Mechanism.** The polyhedron is a *single coherent 3D node*: 20 ESP32-S3 bare chips on
triangular PCBs, hub face carrying a 40 MHz fan-out and a conducted 2.4 GHz phase reference
(ESPARGOS's exact scheme [S1,S2,S3], generalised from 8 planar to ~20 spherical; ESP-SDR
shows S3 raw-IQ support [S4]; AFRL's geodesic-dome array is the mature microwave analogue
[S20]). It is the **anchor**: it supplies the one extra constraint per connected component
that note `20` proves sufficient. Two implementations, in fallback order:

1. **Beacon-held phase (common-TX):** the anchor broadcasts reference frames at the weave
   cadence (≈100–1000 Hz); every puck tracks its own LO phase against the anchor with a
   Kalman filter (AirSync's mechanism [S12], proven at 2.37° std dev on WARP; now on
   commodity silicon by ESP-PPB at 1–10°/frame [S5]). The anchor's known geometry and
   multi-face coherence make the reference *spatially filtered* (multipath rejection, AoA),
   which a single-antenna beacon cannot do. Cost: ~1% airtime; needs TX at the anchor.
2. **Two-way carrier phase (TWCP):** each puck's slot is followed by a unique reply; the
   two-way phase products cancel LO terms and admit extrapolation to zero turnaround
   (Wi-Wi: 2.2 ps ≈ 1 mm on ZigBee/USRP [S13]; theory in [S14,S15]). This yields per-edge
   phase + cm range increments + the per-node hardware-asymmetry product
   `(a_i b_i)(a_j b_j)` (a stable fingerprint, calibratable once — reciprocity calibration
   is stable over hours [S16]). Constraint: the reply must land inside the post-CFO
   coherence window (M2 again).

**What you get for it.** With cross-node phase known at each schedule instant, the home
becomes **one near-field aperture**: linear phase across 5–10 m baselines gives 0.7–1.4°
angles; quadratic phase gives depth from curvature (mm–cm at σ_φ ≈ 0.1 rad, §2); coherent
summing gives up to `N²` SNR; subtraction of the static field gives walls/floorplan and
objects; micro-motion (breathing) becomes visible on static links. The polyhedron
additionally provides (i) the gauge or validator for every wireless estimate, (ii) 3D
illumination/DoA for free-space *TX-side* calibration (faces are mutually visible;
intra-array OTA calibration fixes TX phases the way ESPARGOS fixed RX phases), and (iii)
the conducted truth channel for continuous self-check.

**Genuinely new.** Not the escape (established in SDR/metrology); yes the **integrated
commodity instrument**: polyhedron anchor + wifi weave + closure watchdog, and the
near-field "depth without bandwidth" use of a house-wide coherent aperture. No published
system does any of the three.

**Must be true.** (T5) M2 residual after tracking `≪ λ/10` on S3 (open, decisive);
(T6) range/phase ambiguity resolution: two-way carrier phase is mod λ/2 per frequency, but
multi-channel (2.412–2.462 GHz, 13 overlapping channels) plus coarse FTM seeds fix integers
(plausible; FTM 1–2 m on S3 [synthesis `15`]); (T7) near-field gain survives multipath and
a non-ideal array (test); (T8) intra-array coupling/shadowing on a conducting solid is
calibratable (unknown; theory exists [S24]).

**TX needed?** Yes for option 1 (anchor) and option 2 (both ways). TX is never *required to
be coherent* from the pucks — they pre-rotate in software using tracked phases.

### 4.3 Tier C — the learning layer (self-calibrating and generalising)

**Mechanism.** A complex-valued neural radiance field in the NeRF² sense [S17,S18] — a
continuous function of scene, transmitter position and direction, complex-valued to carry
phase — **extended with per-node complex-gain and phase-trajectory latents** (the gauge),
trained with three losses: (i) measurement likelihood; (ii) **closure consistency** (a
gauge-free loss: the predicted field must reproduce measured loop products even where
absolute phase is unknown — the unidentifiable directions are projected out exactly);
(iii) motion excitation (moving people make geometry observable — the RF analogue of
structure-from-motion; array self-calibration from sources of opportunity is classical
[?S19]). Geometry seeds from FTM + two-way; the instrument (geometry, gains, phases)
separates from the scene (scatterers), so the *scene* model transfers as few-shot
fine-tuning while the *instrument* self-calibrates per home. Synthetic ray-traced
pretraining has the synthesis's strongest transfer number (82.1% F1 with 20% real labels
[`19`]).

**Genuinely new.** Gauge-invariant supervision for self-calibrating RF neural fields on
commodity meshes (NeRF² assumes a fixed coherent RX array and known TX positions [S17]).

**Must be true.** (T9) closure loss is informative for the field (it is, by §3.1);
(T10) the latent instrument/scene separation transfers (test).

---

## 5. What is established, hypothesis and speculation

**ESTABLISHED** (peer-reviewed or datasheet-grade):

- VLBI closure phases cancel station-based (element-based) calibration errors exactly;
  closure amplitudes do the same for gains [S9,S10,S11]; imaging directly from closure
  quantities is standard practice (EHT-class) [S11].
- ESPARGOS: shared 40 MHz + conducted 2.4 GHz phase reference across 8 ESP32-S2 receivers
  gives phase coherence and AoA; boards cascade over coax; PLL phase re-randomises on
  reset/channel change, hence periodic calibration [S1,S2,S3].
- Common-transmitter/OTA-reciprocity phase calibration across independent LOs: AirSync
  (Kalman over the air, 2.37° std dev, WARP) [S12]; modern theory with graph conditions
  and Kalman tracking [S14]; BeamSync (beamed pilots, optimal estimators) [S15].
- Two-way carrier phase: mm-level distance-variation monitoring, mmWave-metrology lineage
  [S13]; carrier-phase ranging on commodity 802.11 (estimates/folklore of cm precision)
  [S25]; Chronos two-way multiplication [S26]; WiCapture sub-cm commodity WiFi tracking by
  exploiting multipath [S27].
- Near-field arrays: position/depth from wavefront curvature is established theory; CRBs
  derived; cm-level accuracy with practical surfaces in high bands [S28,S29].
- Geodesic/spherical arrays: full-sphere coverage, phase-mode processing, conformal-array
  coupling theory, AFRL geodesic-dome phased array hardware precedent [S20,S24].
- ESP32 platform facts: CSI phase slope stable / intercept CFO-driven [S6]; ns RX
  timestamp API `WIFI_PKT_RX_TIMESTAMP_NSEC` [S23]; ESP-SDR supports S3 raw IQ [S4];
  esp-crab common-crystal (ESP32-C5) with hardware files unpublished [S7].

**HYPOTHESIS** (supported but not yet proven in the exact form):

- Closure-phase sensing on commodity WiFi: the element-based-impairment cancellation and
  the triple-sum inversion (this note's derivations; VLBI ancestry).
- Wireless phase coherence on ESP32 at useful cadence (ESP-PPB's numbers are an
  unaudited open-source claim; architecture matches established theory) [S5].
- The 4-node minimality + 2× gain + true-range multilateration chain as a working tracker.
- Near-field depth-from-curvature across a multipath-rich home at 2.4 GHz.
- Polyhedron mutual coupling/shadowing calibratability.

**SPECULATION** (flagged as such):

- Polarimetric closure (closure traces, immune to leakage as well as gains [S11]) on a
  polyhedron whose faces have varied polarisations — a "vector-field" sampler.
- The two-way hardware-asymmetry product as a per-home RF fingerprint for authentication.
- Time-reversal focusing from the polyhedron as a sharpening/SNR trick once TX phases are
  calibrated.

---

## 6. What must be true — falsifier table

| # | Condition | Falsifier |
| --- | --- | --- |
| T1 | S3 per-link phase slope stable; intercept trackable over seconds | Slopes jump per packet (PDD truly random) |
| T2 | Relative CFO per link to ≲ few Hz over a mesh cycle | CFO estimates noisier than the ledger tolerance |
| T3 | Closure floor ≪ signal (M2 small) | Closure floor ≈ √6·σ_CPO, π-ish, stubborn |
| T4 | No baseline-based impairment (AGC/phase coupling) | Closure floor tracks per-pair faults |
| T5 | Fabric residual ≪ λ/10 for ≫ MAC turnaround after tracking | Residual grows unbounded in the update interval |
| T6 | Integer ambiguity resolved by multi-channel + FTM + motion | Seconds-long range jumps that never converge |
| T7 | Near-field curvature gain survives multipath indoors | Depth error scales with 1/B, not with curvature |
| T8 | Coupling/shadowing calibratable on a conducting polyhedron | Array manifold irreproducible after any touch/temperature |
| T9 | Closure loss informative for learned field | Learned field fits measurements but closure residual random |
| T10 | Instrument/scene separation transfers across homes | Per-home retraining collapses to full supervision |

---

## 7. The decisive experiment (one rig, one figure)

**CLO-1 — "The weave bench".** Seven ESP32-S3 boards on one 20 MHz channel, round-robin
ESP-NOW at 1–2 ms slots, 60 s captures, host-side processing:

1. **Six pucks** (stock S3 modules, PCB antennas), spread over 6–8 m in a normal room,
   with a robot-mounted metal reflector and a walking human as targets.
2. **One reference harness** in ESPARGOS style: a seventh S3 on a small board with a
   conducted 40 MHz clock and 2.4 GHz tone distribution (or, initially, one ESP-PPB-style
   master) — the *conducted/known truth*, used only as ground truth, never as input to
   Tier A.

**Measure, in one capture campaign:**

- (a) per-link phase series: slope and intercept statistics vs packet index (T1, T2);
- (b) per-triple closure series, static and during motion: **the closure noise floor**
  (T3, T4) and closure-only multilateration of the robot reflector vs its commanded path
  (T6, inversion validity);
- (c) cross-node phase residual after each fabric variant (conducted truth; beacon-held;
  two-way) at update intervals from 1 ms to 1 s (T5 — the M2 number);
- (d) near-field curvature fit vs plane-wave fit on the coherent snapshots (T7);
- (e) a 10 °C temperature step over 30 min (the synthesis's residual unknown: hold time of
  a relative-CFO estimate within 10 Hz).

**Pre-registered success (the invention exists):** closure floor < 0.1 rad per link after
the relative-CFO ledger; closure multilateration < 10 cm RMS at robot speeds ≤ 2 m/s;
fabric residual < 0.2 rad held ≥ 100 ms without a conducted aid; and the whole-mesh
coherent fit beats plane-wave by ≥ 3× in depth error.

**Pre-registered failure (the boundary stands):** closure floor saturates at O(1) rad under
every schedule and reference scheme, or the fabric residual exceeds λ/10 within the MAC
turnaround. Then per-packet element phase is non-continuous and the only home-wide coherent
aperture is conducted — a cabled home.

**Why this is the single decisive experiment.** It measures M2, the closure floor and the
inversion validity on the same rig, in the same hours, with the same spend (~$500). Every
claim in this proposal is a function of those three numbers.

---

## 8. If the space is closed: the quantity and the minimal reference

- **The quantity that closes it:** the per-link cross-node LO phase `θ_i(t) − ψ_j(t)` —
  a `U(1)` gauge (plus time/frequency gauge) with no observable projection in
  independent-LO reception. Equivalently, if CLO-1 fails, the empirical form is the
  **per-packet element phase residual** (CPO/PDD-type): coherence is bounded by element
  phase *continuity*, not by geometry, antenna count or bandwidth.
- **The minimal reference that crosses it:** one constraint per connected component —
  a single common-transmitter broadcast heard by all, or one reciprocal exchange per edge,
  or a shared 40 MHz clock + 2.4 GHz tone. The polyhedron supplies all three in one object;
  **one coherent node suffices for the whole home** (that is note `20`'s minimal-reference
  result made physical). Cost O(N), never O(N²).
- **What remains impossible even then:** delay-based range at home scale (c/B ≈ 7.5 m) and
  absolute cross-node phase without the reference. Everything else in this proposal has a
  path around these two walls (triangulation/curvature; closure/anchoring).

---

## 9. ESP32-S3 feasibility (subsystem by subsystem)

| Subsystem | Status on S3 | Evidence / gap |
| --- | --- | --- |
| CSI, HT20/HT40, promiscuous | Demonstrated; S3 ranked ≈ C3, below C5/C6 | [S7]; esp-csi toolkit |
| CSI phase quality (slope/intercept) | Favourable but community-grade | [S6]; needs CLO-1 |
| Raw I/Q (ESP-SDR) | Supported on S3 | [S4] |
| TX (ESP-NOW / `esp_wifi_80211_tx`) | Demonstrated; no TX timestamp; raw 11b carries no CSI | [S23]; synthesis `07` |
| ns RX timestamp | `WIFI_PKT_RX_TIMESTAMP_NSEC` | [S23] |
| Shared 40 MHz + conducted tone | Shown on S2 (ESPARGOS), common crystal on C5 (esp-crab), root clock idea portable | [S1,S2,S3,S7]; S3 port untested |
| Wireless phase coherence | C3 (ESP-PPB), not S3 | [S5]; port + audit needed |
| FTM ranging | S3 supports FTM; 2.4 GHz HT20 only, metre-class | synthesis `15`,`17` |
| On-node processing | S3 has SIMD/vector ops and PSRAM; host does the heavy lifting | — |
| Net rate needed | ≤1% duty per node at 1 kHz slots | Airtime arithmetic, §4.1 |

**Feasibility verdict:** Tier A is buildable now on stock S3 modules with public APIs; Tier B
is buildable one hardware revision away, by porting two demonstrated designs (ESPARGOS-type
conducted reference; ESP-PPB-type wireless master); the polyhedron is a fab job, not a
research job.

---

## 10. Cost

| Item | Estimate |
| --- | --- |
| Tier-A puck (S3 module, PCB antenna, USB-C, case) | $3–8 |
| ESP-PPB-class wireless master (VCTCXO + DACs + PCB) | $50–80 (one per home is enough as a fallback anchor) |
| Polyhedron (20 × bare S3, 20 triangular PCBs, hub: 40 MHz fan-out + tone gen + splitter + Ethernet switch/FPGA, assembly) | $200–400 at small volume |
| Home deployment (1 polyhedron + 8–12 pucks + host) | $400–800 |
| CLO-1 bench (7 S3 + harness + robot/track) | ≈ $500 |

Compare: a single mmWave presence sensor is $50–200 with 1D/2D coverage and no
generalisation story. The Weave's cost is the same order for volumetric coverage.

---

## 11. Chaining, existing models/data, own data

**Chaining.** (1) ESP-PPB/ESPARGOS prove the two reference mechanisms on the family →
(2) Tier B anchor makes the home coherent → (3) coherence + near-field curvature gives
sharp 3D and static mapping → (4) closure (Tier A) makes sensing *independent of* step
2 succeeding, and acts as its continuous auditor → (5) two-way products supply geometry and
hardware fingerprints that (6) seed the learning layer's instrument latents → (7) the
learned field generalises across homes because instrument and scene are separated.
Highest-leverage enabler: the conducted/wireless reference; the item that unlocks a further
one: closure, because it removes the reference from the critical path of *sensing*.

**Existing models/data.** ESPARGOS datasets (DaRUS, DOI 10.18419/darus-4456) with robot +
total-station ground truth [S2] — directly reusable for coherent pipeline development;
DICHASUS channel-sounding datasets; NeRF²/RF-3DGS code and synthetic generation for the
field layer; SenseFi/MM-Fi/CSI-Bench/OpenCSI for cross-domain pretraining (all
non-ESP32 CSI — domain adaptation mandatory, per synthesis `19`); ESP-PPB firmware
(GPL-3.0) and hardware (CERN-OHL-S-2.0) as a starting point (copyleft: fine for research,
matters for product).

**Own data.** Nothing like the needed dataset exists: every public phase-coherent dataset
is *one* conducted array (ESPARGOS) or SDR. **Minimal new dataset:** "Weave-CLO-1" —
simultaneous (i) six-node unsynchronised mesh CSI, (ii) conducted-truth reference CSI,
(iii) robot/human ground-truth trajectories, (iv) closure tensor baselines, static and
dynamic, with temperature log. A few hours of capture; it would be the first public
resource for synchronisation-free distributed phase sensing and for M2 on commodity
silicon.

---

## 12. Residual unknowns

1. The closure noise floor on S3 — i.e. whether per-packet element phase is continuous
   (M2 in the loop domain). *The* unknown.
2. ESP32-S3 port of the conducted reference (S2 design + S3 CSI differences).
3. Mutual coupling and face shadowing on a soldered conducting polyhedron; effect on the
   array manifold and on calibratability.
4. Whether the 3D geometry adds measurable DoA benefit over a planar array of equal count
   in a reverberant room (no primary exists; synthesis `08`).
5. Integer-ambiguity resolution performance for two-way carrier phase on 20 MHz channels.
6. Legal/coexistence budget for a 1 kHz beacon from the anchor under ACMA LIPD/FCC 15.247
   (synthesis `07`) — expected fine at ~1% duty, but the beacon must share the home AP's
   channel.
7. Cross-home transfer of the learned field with instrument latents (T10).

---

## 13. Evidence strength

| Claim | Strength (1–10) | Justification |
| --- | --- | --- |
| Gauge/unobservability and escapes | 9 | Formal and peer-reviewed (synthesis `20`); VLBI-independent |
| Closure cancels element-based errors; inversion lemma | 8 | Direct mathematics on the standard model; VLBI established; port untested |
| Polyhedron hardware (conducted coherence at 2.4 GHz on ESP32 family) | 8 | ESPARGOS built and measured (S2); S3 port pending |
| Wireless coherence on ESP32 family | 6 | ESP-PPB open-source claims (C3); unaudited; architecture matches AirSync |
| Two-way carrier phase ranging | 8 | Metrology-grade on ZigBee/USRP; commodity extrapolation plausible |
| Home-scale near-field depth from curvature | 6 | Established theory and CRBs; no indoor 2.4 GHz demo |
| Closure-only 3D tracking (Tier A) | 4 (HYPOTHESIS) | New derivation; depends on T1–T4 |
| Learning layer with gauge-free supervision | 4 (HYPOTHESIS) | Ingredients established; combination new |
| **Overall "the invention exists"** | **6–7** | All components have independent strong evidence; the composition has one decisive unknown (M2) and one open hardware port |

---

## 14. Sources

Primary. DOIs/URLs as retrieved; brackets match in-text tags.

- [S1] F. Euchner, T. Schneider, M. Gauger, S. ten Brink, "ESPARGOS: An Ultra Low-Cost,
  Realtime-Capable Multi-Antenna WiFi Channel Sounder," arXiv:2502.09405,
  https://doi.org/10.48550/arXiv.2502.09405 (also ITG WSA 2023).
- [S2] F. Euchner, S. ten Brink, "ESPARGOS: Phase-Coherent WiFi CSI Datasets for Wireless
  Sensing Research," arXiv:2408.16377; dataset `espargos-0002`, DaRUS,
  https://doi.org/10.18419/darus-4456.
- [S3] ESPARGOS project pages: https://espargos.net/ ; pyespargos,
  https://github.com/ESPARGOS/pyespargos.
- [S4] ESP-SDR: raw IQ with ESP32 chips (incl. ESP32-S3), https://espargos.net/espsdr.
- [S5] ESP-PPB: wireless 1-PPB sync for phase-coherent ESP32 CSI,
  https://github.com/jonathanmuller/esp-ppb (GPL-3.0 firmware; CERN-OHL-S-2.0 hardware;
  accessed 2026-10).
- [S6] Espressif esp-csi issue #121, "Phase offsets in ESP32 CSI (AEGHB-250),"
  https://github.com/espressif/esp-csi/issues/121.
- [S7] Espressif ESP-Techpedia, "Introduction to ESP-CSI Solution" (common crystal,
  esp-crab, C3 + S3 board), https://docs.espressif.com/projects/esp-techpedia/... ;
  esp-crab hardware-files issue, https://github.com/espressif/esp-csi/issues/259.
- [S8] "Same Signal, Different Story: Demystifying Receiver Effects in Wi-Fi Channel State
  Information," arXiv:2605.26836 (includes Espressif ESP32; distortions static over days).
- [S9] R. C. Jennison, "A phase sensitive interferometer technique...," MNRAS 118, 276
  (1958), https://doi.org/10.1093/mnras/118.3.276.
- [S10] A. R. Thompson, J. M. Moran, G. W. Swenson, *Interferometry and Synthesis in Radio
  Astronomy*, 3rd ed. (2017).
- [S11] N. Thyagarajan et al., "Detection of cosmic structures using the bispectrum phase
  I," Phys. Rev. D 102, 022001 (2020), https://doi.org/10.1103/PhysRevD.102.022001;
  M. D. Johnson / A. Chael et al., "Interferometric Imaging Directly with Closure Phases
  and Closure Amplitudes," ApJ 857, 23 (2018),
  https://doi.org/10.3847/1538-4357/aab6a8; A. E. Broderick, D. W. Pesce, "Closure Traces,"
  arXiv:2010.00612.
- [S12] H. V. Balan, R. Rogalin, A. Michaloliakos, K. Psounis, G. Caire, "AirSync...,"
  arXiv:1205.6862; IEEE/ACM ToN 21(6):1681–1695 (2013),
  https://doi.org/10.1109/TNET.2012.2230449.
- [S13] N. Shiga et al., "Range Variation Monitoring with Wireless Two-Way Interferometry
  (Wi-Wi)," arXiv:1806.02130; Sensors and Materials 31(7):2315 (2019); lineage: M. Fujieda
  et al., "Carrier-phase-based two-way satellite time and frequency transfer," IEEE UFFC 59
  (2012), https://doi.org/10.1109/TUFFC.2012.2503.
- [S14] "Distributed MIMO With Over-the-Air Phase Calibration Integrated Into the TDD
  Flow," arXiv:2509.03722.
- [S15] U. Kunnath Ganesan, R. Sarvendranath, E. G. Larsson, "BeamSync: Over-the-Air
  Synchronization for Distributed Massive MIMO," arXiv:2311.11070,
  https://doi.org/10.1109/TWC.2023.3335089.
- [S16] J. Vieira et al., "Reciprocity calibration for massive MIMO," IEEE TWC 16(5):3042
  (2017) (per-AP calibration stable over hours; quoted in [S15]).
- [S17] X. Zhao, Z. An, Q. Pan, L. Yang, "NeRF²: Neural Radio-Frequency Radiance Fields,"
  MobiCom 2023, https://doi.org/10.1145/3570361.3592527; arXiv:2305.06118.
- [S18] URF-GS / unified radiation field, npj Wireless Technology (2026),
  https://doi.org/10.1038/s44459-026-00067-6.
- [S19] Y. Rockah, P. M. Schultheiss, "Array shape calibration using sources in unknown
  locations," IEEE TASSP (1986–87); B. P. Flanagan, K. L. Bell, "Array self-calibration
  with large sensor position errors," Signal Processing 81(10):2201 (2001),
  https://doi.org/10.1016/S0165-1684(01)00121-9.
- [S20] S. Liu et al., "A geodesic sphere phased array antenna for satellite control and
  communication," URSI GA 2002, https://www.ursi.org/proceedings/procGA02/papers/p1274.pdf
  (triangular planar subarrays, icosahedral arrangement).
- [S21] H. Wang, X. Li, Y. He, J. Luo, "Path to Diversity: A Primer on ISAC-izing Commodity
  Wi-Fi," arXiv:2601.12980 (CPO/PDD random per packet; two-way multiplication; CSI ratio).
- [S22] H. B. Lee, "Accuracy of range-range and range-sum multilateration systems," IEEE
  Trans. AES 11(6):1346 (1975); modern treatments: "A Review of Linear Multilateration
  Techniques and Applications" (2022).
- [S23] ESP-IDF Wi-Fi driver and API references (`esp_wifi_80211_tx`), ESP-IDF v5.5
  https://docs.espressif.com/projects/esp-idf/en/v5.5/esp32/api-guides/wifi.html ; ns RX
  timestamp `WIFI_PKT_RX_TIMESTAMP_NSEC`, ESP-IDF issue #9843.
- [S24] P.-S. Kildal / P. Persson, L. Josefsson, *Conformal Array Antenna Theory and
  Design* (Wiley, 2006); "Mutual Coupling in Phased Arrays: A Review," Int. J. Antennas
  Propag. (2013), https://doi.org/10.1155/2013/348123.
- [S25] R. Exel, "Carrier-based ranging in IEEE 802.11 wireless local area networks,"
  IEEE WCNC 2013.
- [S26] D. Vasisht, S. Kumar, D. Katabi, "Decimeter-Level Localization with a Single WiFi
  Access Point (Chronos)," MobiCom 2015, https://doi.org/10.1145/2789168.2790124.
- [S27] M. Kotaru, S. Katti, "Position Tracking for Virtual Reality Using Commodity WiFi,"
  CVPR 2017, https://doi.org/10.1109/CVPR.2017.286; arXiv:1703.03468.
- [S28] "Beam Focusing for Near-Field Multi-User Localization," arXiv:2407.20264;
  "Cramér-Rao Bounds for Near-Field Localization," arXiv:2104.14825 (cm-level with
  practical surfaces in mmWave bands); "Performance Bounds of Near-Field Sensing with
  Circular Arrays," arXiv:2404.17318.
- [S29] Zubow et al., "On Phase Offsets of 802.11ac Commodity WiFi," WONS 2021
  (per-chain offsets semi-time-invariant; calibratable).
- [S30] Aquila v2 synthesis: `docs/research/v2/00-synthesis.md` (boundary, M2, minimal
  reference, capability map).

Secondary/contextual: ESP-IDF FTM docs; DICHASUS (https://dichasus.inue.uni-stuttgart.de/);
Eurotech note: every quoted ESP-PPB figure is the project's own claim and is treated as
HYPOTHESIS until CLO-1 audits it.

---

## 15. One-paragraph statement of the invention

**The Closure Weave** is a home sensing instrument with two stitched tiers and one anchor.
**Tier A (the weave)** makes every loop of links in an ordinary ESP32 mesh a
synchronisation-free observable: phases around directed triangles cancel every element-based
impairment (LO phase, gains, sampling and detection delays, CFO, up to a measurable
relative-frequency ledger), leave a physical sum of path changes with 2× sensitivity, and —
with four or more nodes — invert exactly to per-node range changes, i.e. a
gauge-free, true-range multilateration of everything that moves. **Tier B (the anchored
weave)** spends the one reference the theory says is necessary — supplied by a conducted-
coherent 3D polyhedron acting as common transmitter, two-way partner and near-field
anchor — to make the whole house one coherent aperture: 1° angles from home baselines, depth
from wavefront curvature instead of bandwidth, coherent gain, static mapping. The closure
tier runs continuously as both a sensing mode and the coherence auditor of the anchored
tier; a gauge-invariant learned field joins them. Its decisive test is a seven-node bench
measuring three numbers: the closure floor, the M2 residual, and near-field depth gain. If
those pass, a home full of cheap ESP32-S3 nodes is one sharp, self-calibrating, generalising
3D sensor; if they fail, the closing quantity is the per-packet element-phase continuity and
the minimal crossing reference is one coherent, conducted node.
