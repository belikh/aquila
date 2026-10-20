# 22 — Invention S: The Reciprocal Gauge Mesh (RGM)

### A geometry-first 3D sensor: many cheap ESP32-S3 polyhedra that measure their own geometry with two-way carrier phase, manufacture their own phase coherence from that geometry — or fall back to a sharp, reference-free sensing mesh if coherence never comes

Status: invention proposal + boundary analysis. Australian English. Grades as in the v2 notes:
**ESTABLISHED / HYPOTHESIS / SPECULATION**; invented sub-systems are graded per sub-claim. Cross-
references are to notes `01`–`20` of this folder (`00-synthesis.md` integrates them). Sibling note
`22-invention-a.md` ("The Coherent Weave") develops the *coherence-first* route (wireless phase-locking
loop + disclosed 2026 platform claims); this note deliberately develops the **geometry-first** route and
positions the two as complementary (A's Fence is this note's optional Mode 2 upgrade). Where this note
relies on a 2026 result found by the sibling note and not re-verified here, it says so.

---

## 0. Mission and method

> Discover the new invention that makes a home full of cheap ESP32-S3 nodes behave as one sharp,
> self-calibrating, generalising 3D sensor — or prove that no such invention exists and state the exact
> physical boundary that blocks it.

Method: Parallel search/extraction sweeps over the six mission directions plus the round-trip scheme, from
primaries (arXiv/IEEE/Espressif/GitHub), building on notes `01`–`20` and the sibling invention note. Every
claim is graded and cited. The proposal below is organised so that **each mode of the sensor is decided by
a separately stated measurement, and the weakest link is stated first.**

**The answer in one paragraph.** The invention is the **Reciprocal Gauge Mesh**: a network of cheap nodes
in which (i) every pair is measured by a *scheduled ping + unique reply* exchange so that the **complex
product of the two directions of each link is a gauge-free, self-contained, phase-coherent observable** —
it needs no common clock, no shared LO, no reference emitter, and is immune to the PLL phase
re-randomisation that defeats one-way CSI; (ii) the mesh of these products, together with each node's
rigid geometry, is inverted to recover **its own mm-class geometry and the home's static scattering map**;
(iii) once a link's propagation phase is measured, **phase coherence across nodes is an algebraic
consequence**: any member (or an ambient AP) that transmits on that link lets every receiver recover its
own LO phase from the known channel — coherence is *manufactured from geometry*, not assumed from locking;
(iv) even if every coherence step fails, the mesh still senses: each link is a coherent bistatic pair in
the round-trip domain (double phase sensitivity, double Doppler, within-cell AoA at both ends), and a
volumetric polyhedron gives near-field curvature ranging (cm inside ~1 m) and polarimetric diversity. The
single decisive experiment is **E1 (§8): the reciprocal-gauge null test and geometry closure on four
ESP32-S3 nodes with tape/laser ground truth and a conducted phase reference** — one rig, two numbers, a
pass/fail tree that selects Mode 2 (coherent home aperture), Mode 1 (geometry-fused mesh) or Mode 0
(gauge-free mesh only), and states the physical quantity that closes each.

**What is genuinely new here** (detail in §5): the *geometry-first bootstrap* (measured channels →
recovered LO phases → coherence: the inverse of every published distributed-coherence scheme); the
**reciprocal-gauge survey** (rigid-body-aided ambiguity cascade + RF bundle adjustment over face-grid
products); the **virtual-monostatic round-trip aperture** for sensing; the **parallel 20-channel conducted
cell** with bidirectional reference (TX *and* RX chain calibration); and the **closed calibration↔sensing
loop** in which the static environment is simultaneously the map, the ambiguity resolver and the phase
reference, so cross-home generalisation is procedural (re-run the survey), not statistical.

---

## 1. The physics ledger

### 1.1 The gauge (ESTABLISHED — note `20`; primaries [1,2,3,8])

With independent LOs, a CSI observation is
`ĥ_ij(f_k) = g_ij·a_i·b_j·e^{j(θ_i−ψ_j)}·e^{−j2π f_k τ}`; only `θ_i − ψ_j` is observable, and across a
connected measurement graph node phases are fixed only up to one global additive constant (`U(1)`, plus
time/frequency gauges). Usable with no shared reference: amplitudes; per-link subcarrier phase slope
(delay); per-receiver inter-antenna differential phase (AoA *within* one coherent receiver); **two-way
products**; temporal phase differentials (Doppler). Not observable: absolute cross-node phase; absolute
phase-of-flight range; cross-node AoA. One constraint per connected component crosses the boundary; the
conducted references proven on the ESP32 family are the 40 MHz clock **plus a conducted 2.4 GHz frame
reference**, because the RF PLL's initial phase re-randomises per reset and per channel even with a shared
clock [11].

### 1.2 The exchange algebra (ESTABLISHED pieces; the composition is the invention's substrate)

Consider nodes A and B, single LO each, exchange (A→B at physical time `τ_a`; B→A at `τ_b`; channel
reciprocal between the two instants, i.e. `h_AB = h_BA = h`):

- **One-way**: `∠ĥ_AB = ∠h + (θ_A − θ_B) + hw` — contains the unknown LO difference.
- **Product** `ĥ_AB·ĥ_BA` (this is WiDRa's "sum-carrier-phase" [8]; Chronos' round-trip CSI [5]):
  `∠(ĥ_AB·ĥ_BA) = 2∠h + hw_P + Δω·(τ_a−τ_b)`, where `Δω` is the LO frequency difference. **The random
  initial PLL phases (θ0) cancel exactly**, so the product is invariant to per-boot and per-channel
  re-locking [8,11]; the channel phase is recovered at *doubled* rate (ambiguity `λ/2 = 6.25 cm` at
  2.4 GHz); the only debit is the `Δω·Δτ` term (below).
- **Ratio** `ĥ_AB/ĥ_BA`: `= hw_R + 2(θ_A−θ_B) + Δω·(τ_a+τ_b)` — the channel cancels; this **measures the
  relative LO phase** (at doubled sensitivity) including its random part — the quantity the product
  throws away. Product and ratio are complementary outputs of the same exchange.
- **Timing**: RX timestamps (ESP32-S3: ns RX stamp, 12.5 ns latch grid [17,29]) plus reply discipline give
  range and clock offset at the usual FTM-class precision [15,30].
- **Across faces**: inter-face differentials of the two-way phase at each end give AoA at *both* ends
  (within-cell apertures), and the 400 face-pair ranges of a pair of 20-face cells are constrained by
  rigid-body geometry (below).

### 1.3 The two debits, stated exactly (HYPOTHESIS quantified from ESTABLISHED pieces)

1. **Turnaround × CFO**: `Δω·Δτ`. With `Δω = 20–40 ppm × 2.4 GHz ≈ 48–96 kHz`, a reply-interval
   uncertainty of 1 µs contributes 0.30–0.60 rad; 100 ns contributes 0.03–0.06 rad. Crucially, if the
   software loop keeps `Δτ` repeatable, the **mean** term is absorbed into the per-link calibration
   constant (sensing needs only *changes*); what remains is turn-around **jitter** and CFO drift. The
   chip-internal scheduling of a reply measured **std(t3−t2) < 1 µs** on a Raspberry Pi CM4's in-chip MCU
   [6]; the ESP32 has no TX hardware timestamp [17,29], so this number is **unmeasured on S3** and is E1's
   second output.
2. **Non-reciprocity over `Δτ`**: a target moving through the link between the two transmissions leaves
   `∠(h(τ_a)/h(τ_b))` residue. At 1 m/s radial and `Δτ = 1 ms`, the two-way phase error ≈ 0.2 rad
   (one-way Doppler 16 Hz × 2π × 1 ms × 2); reply intervals of tens of ms are only safe for *static*
   paths, which is why the survey uses short exchanges and the sensing uses temporal differencing.

### 1.4 The two theorems the mesh needs (HYPOTHESIS — the invention's load-bearing claims)

**T1 (gauge-free link phase).** The product phase recovers each link's propagation phase, hence (after
ambiguity resolution) its range, with *no cross-node reference of any kind*. Directly supported at the
component level: WiDRa attains **<1 mm differential-range RMSE on off-the-shelf Wi-Fi at 20 MHz** using
sum-carrier-phase [8]; Chronos pioneered round-trip phase ranging [5]; ML-Track runs multi-link round-trip
CSI Doppler on four Raspberry Pi CM4s for 0.23 m median passive tracking [7]. No prior work uses the *mesh*
of products to self-survey geometry, and none is on ESP32.

**T2 (manufactured coherence).** For any link whose complex channel `h` is known (T1 gives exactly this),
a single one-way frame on that link lets the receiver solve its own effective LO phase from the sender's,
because `∠ĥ = ∠h + (θ_A−θ_B)` and `∠h` is the quantity the mesh already measured:
`(θ_A−θ_B) = ∠ĥ − ∠h`. Coherence across nodes is therefore **an algebraic consequence of measured
geometry**, not a prerequisite for it. This inverts the published dependency: AirSync and the distributed-
MIMO calibration literature assume a reference must exist first and then calibrate [1,2,3,4,21]; LoSRef
uses the *one* Tx–Rx LoS path as an in-link reference [17]; T2 uses the *measured* channel of any link
(incl. a hub broadcast or an ambient AP's frame) to transfer phase across the whole mesh with no
locked reference at all. *Novel; needs E1.* A `π`-ambiguity on `h`'s phase (from doubling) is resolved by
(a) the delay/range estimate, (b) temporal continuity, (c) the ratio+one-way combination at exchange time.

Note the design consequence: if T2 is right, **the polyhedron's hub broadcast is a phase-transfer service
for the whole home**, and the conducted reference inside a cell is only needed *within* the cell (where it
is cheap and proven [11,12]).

---

## 2. Prior art map — what exists, what does not (ESTABLISHED, with primaries)

| Primitive | State of the art | Source |
| --- | --- | --- |
| Round-trip CSI cancels Tx/Rx asynchrony | Established (Chronos, NSDI 2016); COTS; ToF ranging | [5] |
| Two-way **sum-carrier-phase** ranging on COTS | **<1 mm differential** RMSE, 20 MHz, off-the-shelf HW; *direction-finding explicitly left to future work* | WiDRa, JSAC 2024 [8]; patent family [8b] |
| Multi-link round-trip CSI Doppler sensing | 4× Raspberry Pi CM4, passive tracking 0.23 m median; single antennas, no arrays, no geometry self-survey | ML-Track, TMC 2025 [7] |
| Round-trip phase for localisation | 0.06 m relative-displacement, 0.26 m 2-D (tracked phase, no absolute ranging) | JSAC 2024 [9] |
| Multi-channel coherent CFR stitching | Two-way CFR + timestamps; SDR testbed; 160 MHz stitched | PCMC, [9b]; two-way vs one-way [10]; splicing [31] |
| Distributed MIMO sync + reciprocity calibration | Master-pilot tracking + bidirectional calibration, repeated at sync rate; SDR/FPGA | AirSync [1]; Rogalin [2]; 2026 OTA-calibration [3,4] |
| Conducted coherent ESP32 arrays | 40 MHz + conducted frame reference; PLL phase re-randomises per reset/channel; phase stability restored; **planar 4×2 dual-pol**; multi-board kits; RX-only; phase-coherent TX stated possible, unreleased | ESPARGOS [11,12]; ESP-SDR IQ [13]; esp-crab C5 [14] |
| Wireless coherent ESP32 CSI | Claimed on ESP32-C3: FTM-disciplined VCTCXO + reference frame 1 ms after each external frame; unrefereed, no sensing results | ESP-PPB, GitHub, GPL-3.0 [16] *(grey literature; found by sibling note, verified here)* |
| COTS one-way phase-coherent CIR via a reference path | LoSRef: Tx–Rx LoS path as delay+phase reference; 5 GHz 160 MHz 802.11ax; sub-wavelength respiration | arXiv 2602.05344 [17] |
| Cell-free/distributed ISAC | Theory + some cabled measurement campaigns (1PPS + 10 MHz reference; PLL lock ambiguity documented; OTA calibration needs static scenes) | [20,21] |
| Multipath as information: virtual anchors, MP-SLAM | UWB/5G established; map/floorplan or self-built | [25] |
| Array self-calibration, unsynchronised arrays | Signal-processing literature (DOA+geometry joint estimation; TOA with unknown sources) | [26,27] |
| Volumetric coherent ESP32 aperture; mesh self-survey; manufactured coherence | **Does not exist** | [00 §7; 08,09] |

The gap this invention occupies: **nobody has used the gauge-free two-way product as the *coordinate
observable* of a mesh, and nobody has inverted measured channels back into cross-node LO phase to
manufacture coherence without a locked reference.**

---

## 3. THE INVENTION — the Reciprocal Gauge Mesh

One sentence: **measure every link redundantly and without a reference; let the redundant gauge-free
measurements define the mesh's own geometry and the home's static scattering map; then buy coherence
algebraically from that geometry, and sense with whatever survives.**

Nested operating modes (each independently falsifiable):

- **Mode 0 — Gauge-free mesh.** Per-link round-trip coherent bistatic sensing + within-cell AoA + near-
  field + polarimetry. Needs no cross-node coherence; survives every failure of T1/T2 except SNR.
- **Mode 1 — Geometry-fused mesh.** Add the reciprocal-gauge survey (T1): mm–cm geometry, static
  scattering map, virtual-anchor extension of the aperture; multistatic/monostatic-midpoint fusion.
- **Mode 2 — Manufactured-coherence aperture.** Add T2: hub broadcast (or ambient common-view) phase
  transfer; whole-home coherent aperture (metres of baseline → ~1–3° cross-node DoA); coherent
  subtraction/imaging.

### S1 — The cell: volumetric conducted-coherent transceiver (Mode 0–2 substrate)

- **New.** No ESP32-class *volumetric* coherent array exists; the conductor-states-of-the-art is planar
  [11,12]. This cell is a **parallel** 20-channel coherent receiver (one S3 per face; no RF switch), with a
  hub face distributing (a) the 40 MHz clock and (b) a conducted 2.4 GHz *frame* reference to every face,
  and back-hauling CSI to a hub MCU/FPGA (premise). The reference network is used **in both directions**:
  hub→faces (RX chain / PLL phase per boot per channel, as ESPARGOS [11,12]) and **faces→hub** (each
  face's TX chain phase, which ESPARGOS never needed because it is RX-only) — giving a self-calibrated
  coherent MIMO cell, not just a coherent receiver.
- **Mechanism.** Per face: `ĥ_REF = g_ref·g_face·e^{j(θ_hub−ψ_face)}·e^{−j2πfτ_path}`; with the
  distribution network's path phases `τ_path` known by construction (microstrip geometry; ESPARGOS
  assumes and measures this), division gives each face's chain state. Face orientations (icosahedral
  normals) give full-sphere coverage, 3-D polarisation projection diversity, and — inside
  `2D²/λ ≈ 1.4 m` — spherical wavefront curvature for ranging [22].
- **Must be true.** (i) 40 MHz fan-out acceptable to S3 (the S3 design guidelines say firmware supports
  the 40 MHz crystal; ESPARGOS has proven the fan-out pattern on ESP32/S2/C61 [11,13], and esp-crab on C5
  [14] — S3-specific unverified); (ii) mutual coupling and body shadowing in a solid polyhedron are
  tolerable (unmeasured anywhere; sibling note E2 covers 3-D DoA); (iii) the face-to-hub reference path
  phases are stable with temperature.
- **Falsifier.** Face-pair differential phase noise (with conducted reference active) > 0.2 rad RMS in a
  20-minute run, or 3-D DoA RMSE > 20° (sibling note E2), or coupling-induced pattern nulls that move with
  the installation.
- **S3 feasibility.** CSI + ESP-NOW per S3: established [07,11]; 20 chips per cell is a BOM and thermal
  problem, not a physics problem. Data rate: 20 faces × ~30 Hz × ~200 B CSI ≈ 120 kB/s → one hub MCU.
- **Cost.** 20 S3 modules ≈ US$60–90; hub (MCU or small FPGA) + clock buffer fan-out + 2.4 GHz splitter
  and microstrip distribution + 20 triangular PCB faces + flex/rigid edges + power ≈ **US$250–400
  prototype per cell**, an estimated US$120–200 at 1k units. Satellites (single S3 + antenna): US$5–10.
- **TX needed?** Not for the reference (hub transmits internally); yes for any cooperative exchange and for
  Mode 2 transfer.

### S2 — PURR: the exchange protocol (ping, unique reply, rounds, ambient common-view)

- **New.** Scheduled TDM rounds exist for distributed-MIMO sync [1,2,3]; the *sensing* uses of round-trip
  CSI are pairwise [5,6,7,8]. PURR unifies them: one round produces, per ordered pair and per channel,
  the six observables of §1.2 (product phase, ratio phase, timestamps, amplitudes, per-face differentials,
  ASYM fingerprint), and is designed so **Mode 0 needs nothing else**.
- **Mechanism.** A round = hub-or-round-robin ping (ESP-NOW HT40 MCS0 frame — the CSI-capable primitive
  [00 §7]), every other node replying in an assigned unique slot (identity + slot index in the payload, so
  replies are unique and collision-diagnosable); all nodes promiscuous-log CSI for every frame with ns RX
  timestamps; nominally 20 ms cadence (note `12` optimum), reject exchanges whose measured reply intervals
  deviate from nominal (deferral/jitter gate). Channel plan: within-round single channel; rounds hop
  {1,6,11} for the survey and all 13 for the band-synthesis pass; the product is re-lock-immune, so
  hopping *between rounds* is free. **Ambient common-view** sub-mode: when the home's AP (or any
  transmitter) is heard on a surveyed link, the same T2 algebra runs receive-only — the mesh harvests
  coherence from the environment's own traffic at ambient frame rate.
- **Must be true.** ESP-NOW HT40 MCS0 yields CSI at both ends with the per-frame rate assumed (note `07`);
  MAC turnaround repeatable enough that the jitter gate passes at ≥ 10 Hz; more than 2 nodes can share the
  channel without violating the home's own duty cycle (ACMA LIPD envelope, note `07` [33]).
- **Falsifier.** E1's jitter/phase-floor number (below) fails; or CSI from relayed slots proves too sparse.
- **S3 feasibility.** All APIs are S3-native (ESP-NOW, promiscuous CSI, ns RX stamp via the known IDF
  patch [17,29]); processing host-side.
- **Cost.** Firmware only.
- **TX needed?** Yes for rounds; no for ambient common-view.

**The round-trip scheme, adjudicated** (mission item 5): it measures *offset* (clock, from timestamps and
the ratio's CFO term), *range* (delay + carrier phase), *Doppler* (temporal phase derivative, doubled),
*identity* (unique replies + per-link ASYM fingerprint, stable per boot/channel), and *channel* (one-way
CSI + gauge-free product). It breaks at: SNR margin in deep NLoS; non-reciprocity over the reply interval
for moving scatterers (§1.3 debit 2); and any unresolvable TX-instant ambiguity when CFO × jitter grows
(below).

### S3 — The reciprocal-gauge survey: RF bundle adjustment + the ambiguity cascade

- **New.** Pairwise round-trip phase ranging exists [5,8,9b]; TOA-only unsynchronised array localisation
  exists in theory [26]; array self-calibration exists [27]; multipath SLAM exists [25]. The invention is
  the *joint* estimator over a **rigid volumetric cell grid**: unknown node poses (6-DoF per cell, 0-DoF
  per face — faces are rigid!), unknown integer ambiguities per link per channel, unknown per-channel
  chain-phase constants, unknown clock states, and the static scatterer list — solved from the product
  phases, the within-cell AoA differentials, the FTM-class timings and the amplitudes, with the
  **ambiguity cascade**:
  `FTM (~1 m) → synthetic-band delay over ~80 MHz (ambiguity 1.9 m, resolution ~cm at SNR [9b,31]) →
  carrier phase (ambiguity 6.25 cm, mm sensitivity [8])`,
  resolved **jointly across the 20×20 face-pair grid** of each node pair by the rigid-body constraint
  `r(A_i,B_j) = |x_i − y_j| + b` (a carrier-phase ambiguity problem with known antenna geometry, in the
  GNSS style — the geometry itself is the ambiguity resolver, and 400 redundant measurements pin ~40
  integers).
- **Mechanism.** Because the product phase is reference-free, the survey is just a big least-squares over
  self-contained observables; the static multipath is estimated as part of the same map (it *is* the
  nuisance parameter) and then reused as the virtual-anchor set (§ S6). The estimator is literally a
  bundle adjustment: poses + calibration + map, iterated.
- **Must be true.** Direct-path (or at least *stable-dominant-path*) phase extractable on each link in
  realistic rooms; the cascade's first two stages accurate enough to hand over to the carrier phase;
  temperature stability of chain constants between survey epochs (note `10`: <2.6 % over 4 h is the
  adjacent number).
- **Falsifier.** E1(c): all-pairs absolute range RMSE > 3 cm, or pose RMSE > 10 cm, with no prior
  positions. A *closed* space would then exist at this layer and it is named in §7.
- **S3 feasibility.** Host-side estimation; S3 supplies CSI + timestamps. The pieces have COTS precedent
  [5–10]; the mesh version is new.
- **Cost.** Nil (compute).
- **TX needed?** Yes (rounds).

### S4 — Manufactured coherence (T2): the hub broadcast as a phase-transfer service

- **New.** All published cross-node phase acquisition assumes a locked/locking reference (master pilot
  [1], TDD reciprocity rounds [2,3,4], FTM-disciplined oscillators [16]) or a stable in-link path [17].
  T2 instead *computes* the reference from the mesh's own measured channels: after S3, each surveyed link
  `h` is known to a fraction of a wavelength; one broadcast on that link yields the receiver's LO phase to
  `σ ≈ σ_φ(ĥ) + σ_φ(carrier-tracking gap)`; the mesh is coherent for the following interval, at broadcast
  cadence (hub round or ambient AP frames).
- **Mechanism.** `(θ_A−θ_B) = ∠ĥ_AB − ∠h_AB` per measurement; tracked with a Kalman filter on wrapped
  phase (the sibling note's M2′ machinery [22a]) with the *advantage that the channel term is measured,
  not assumed static to λ*. Multipath contamination of `∠h` is the residual: sub-band delay gating and the
  face-level field map refine it. Failure mode is graceful: coherence degrades where a surveyed link is
  disturbed (large moving object on it), which the ASYM/phase-innovation monitor flags.
- **Must be true.** `σ_φ(h)` from S3 ≲ 0.5 rad per link; LO phase evolution between exchange and use is
  smooth (free-running PLL; quantified by E1(b)); hub broadcast receivable on surveyed links (or ambient
  common-view available).
- **Falsifier.** E1(d): geometry-derived phase prediction residual > 0.5 rad on independent frames.
- **S3 feasibility.** A hub face broadcasting ESP-NOW frames is trivial hardware; the estimation is
  host-side; **wireless phase transfer on S3 remains undemonstrated** — this is the invention's boldest
  step and the one the decisive experiment targets directly.
- **Cost.** Nil beyond S1/S2.
- **TX needed?** Yes, unless ambient common-view suffices (then receive-only).

### S5 — Gauge-free sensing algebra (Mode 0's sharpness; what survives every failure)

1. **Virtual-monostatic round-trip aperture (new framing).** Each surveyed link is a *self-contained*
   coherent radar whose effective monostatic element sits at the pair's midpoint with doubled path
   sensitivity; the mesh's link midpoints form a sparse volumetric monostatic array *distributed through
   the home*, usable for coherent back-projection with **zero cross-node phase** (each element's phase is
   internally referenced). This is the multistatic-to-monostatic synthesis nobody has exploited for
   Wi-Fi sensing (ML-Track used round-trip Doppler only [7]).
2. **Doubled sensitivity.** The product's target term is `2·h_static·h_target` — the dynamic-to-static
   ratio and the Doppler are both **doubled** relative to one-way CSI (cross-term algebra of §1.2;
   experimentally the reason round-trip CSI improves sensing [6,7,8]). LoSRef shows dynamics 20 dB below
   static multipath are extractable with careful referencing [17].
3. **Within-cell AoA at both ends.** Inter-face differentials of the two-way phase give each cell a
   24°(λ/D; ~12° with doubled sensitivity and close to the sibling note's target of ≤15° [22a]) *angle
   to the perturbation*; two cells triangulate.
4. **Near-field curvature ranging.** Inside `2D²/λ ≈ 1.4 m` of a cell, the wavefront curvature across the
   20 faces encodes range directly: ~0.6 rad of curvature phase at R = 1 m (one-way; doubled in the
   product), ~0.57 rad/m range slope at 1 m falling as `1/R²` — with 0.05 rad phase noise, ~5–10 cm range
   precision at 1 m, decimetres at 2 m [22]. This makes a *single cell* a sharp short-range 3-D sensor with
   no network at all.
5. **Polarimetric diversity.** The 20 face normals sample distinct polarisation projections (design the
   patches as mixed linear/circular, as ESPARGOS One does on its 8 faces [12]); material and orientation
   discrimination (metal/water/person/fabric) from the polarisation response matrix — previously only on
   SDR/RFID-class hardware per note `04`.
6. **Temporal CSI ratios** (note `01`) remain usable as a zero-configuration sanity channel.
- **Falsifier.** E2–E4 (§8): near-field ranging > 20 cm at 1 m; polarisation classes inseparable; link
  Dopplers not extractable at ≥10 Hz cadence on S3.
- **TX needed?** Yes for products; items 3–5 also work on *one-way* coherent light inside a cell
  (ambient illumination) because the cell is conducted-coherent.

### S6 — The environment as calibrator (map = ambiguity resolver = phase reference = virtual aperture)

- **New in role, established in pieces.** Virtual anchors / MP-SLAM [25], LoSRef's in-link reference [17],
  and the sibling note's "Loom" [22a] all exploit static multipath. Here it is one object with four jobs:
  (i) the survey's nuisance parameter; (ii) the ambiguity cascade's stabiliser (stable specular paths give
  redundant coarse range equations); (iii) the phase reference for T2 on disturbed links (a gathered
  virtual anchor can substitute for a vanished direct path); (iv) extra coherent aperture elements
  (reflectors with known positions focus energy that a free-space model would discard). The loop is
  closed: **sensing improves the map, the map re-calibrates the sensor.**
- **Must be true.** Specular components at 2.4 GHz in furnished rooms are stable over minutes-to-hours and
  associable across links (the sibling note's E3 tests VA jitter; LoSRef shows the direct path is stable
  [17]).
- **Falsifier.** VA position jitter > 30 cm over 30 min; association failure > 20 % of rounds.
- **S3 feasibility.** CSI-only; host-side.
- **Cost.** Nil. **TX needed?** No (works with ambient illumination once surveyed).

### S7 — The learning layer: calibrate, don't generalise

- **New as a system stance.** The mesh does not learn geometry, or per-home phase, or fingerprints: those
  are *measured* each installation by S2–S3 (minutes-to-hours of automatic protocol). Learning is confined
  to (i) residual models (per-chain temperature drift, ASYM drift — note `10`), (ii) scattering/material
  priors, (iii) target signatures (pose/identity) on top of physical coordinates. Cross-home transfer is
  therefore *procedural*: the same protocol re-derives a new home's tensor; the residual models are the
  only transferable parameters and are few.
- **Must be true.** The survey converges in new homes without human metrology (E1 in ≥2 distinct rooms).
- **Falsifier.** Survey RMSE degrades > 3× in a second, structurally different room, or requires manual
  seed points, at which point "self-calibrating" is overstated.
- **S3 feasibility.** ESP-DL/ESP-NN INT8 for residual/target models on-cell (note `19`); heavy imaging
  host-side.
- **Cost.** Nil. **TX needed?** No additional.

---

## 4. What is genuinely new — the precise claim list

**New (no precedent found; each graded by its falsifier):**
- **N1 — Geometry-first bootstrap / manufactured coherence (T2).** Coherence derived algebraically from
  *measured* channels, no locked reference assumed. (Closest: [17] uses one link's LoS as an in-link
  reference; [1–4] assume a reference; none inverts measured geometry into cross-node LO phase.)
- **N2 — Reciprocal-gauge survey.** Rigid-body-aided ambiguity cascade + bundle adjustment over face-grid
  products for mm–cm self-survey and joint static-map estimation on commodity Wi-Fi. (Closest: [8]
  differential-only; [9b] SDR stitching; [26,27] theory; [25] UWB.)
- **N3 — Parallel volumetric conducted cell with bidirectional calibration.** 20-face 3-D aperture, TX and
  RX chain phases calibrated through the reference network; polarisation and near-field capability.
  (Closest: planar RX-only ESPARGOS [11,12].)
- **N4 — Virtual-monostatic round-trip aperture and the nested-mode sensing algebra** (M0–M2 with
  explicit degradation).
- **N5 — Environment-as-calibrator closed loop** (map ↔ ambiguity ↔ reference ↔ aperture) as the
  generalisation mechanism. (Sibling note's Loom is the nearest claim; the roles here differ —
  geometry/ambiguity vs phase gauge — the two compose.)

**Assembled, not new (honesty list):** the U(1) gauge (note `20`); air-sync master-pilot tracking [1];
TDD reciprocity calibration [2,3,4]; sum-carrier-phase ranging [8]; round-trip CSI sensing [5,6,7];
multi-channel stitching [9b,10,31]; conducted ESP32 coherence [11,12]; FTM [15,30]; virtual anchors and
MP-SLAM [25]; array self-calibration [27]; near-field curvature bounds [22]; holographic imaging [23,24];
cell-free ISAC framing [20,21]; CSI sanitisation [28].

---

## 5. Assumption ledger

| # | Assumption | Grade | Decided by |
| --- | --- | --- | --- |
| A1 | Product phase is extractable from S3 CSI, re-lock-immune, with jitter-limited noise floor ≤ 0.3 rad at the working cadence | HYPOTHESIS (COTS: [8]; S3: none) | E1(a,b) |
| A2 | Reply-interval jitter on S3 ≲ 0.3 µs *after deferral gating*, or CFO tracking absorbs it | HYPOTHESIS | E1(b) |
| A3 | Ambiguity cascade closes (FTM→80 MHz→carrier) over face grids | HYPOTHESIS | E1(c) |
| A4 | Geometry-derived phase prediction ≤ 0.5 rad (T2) | HYPOTHESIS (boldest) | E1(d) |
| A5 | Direct/dominant-path phase stable enough per link in furnished rooms | HYPOTHESIS (LoSRef supports [17]) | E1 + E6 |
| A6 | Cell internal coherence on S3 via 40 MHz + conducted frame reference | ESTABLISHED pattern on S2/C61 [11]; unverified S3 | S1 bench test (sibling E2) |
| A7 | Near-field curvature ranging ≤ 10 cm at 1 m | HYPOTHESIS (bounds established [22]) | E2 |
| A8 | Polarisation classes separable from face diversity | SPECULATION | E3 |
| A9 | Static map stable/associable enough to anchor ambiguity + T2 | HYPOTHESIS | E6 (sibling E3) |
| A10 | UK/AU duty-cycle and frame lawfulness of PURR rounds | ESTABLISHED per note `07` (class licence [33]); schedule must be shaped to it | legal check |

---

## 6. The boundary: if it fails, exactly which quantity closes it, and the minimal reference that crosses it

The space is **not** closed by physics — every layer has a known physical bound, a measurable quantity, and
a named, minimal reference that crosses it. This is the honest form of the "no such invention" answer:

| Layer | What closes it if the bound is hit | The quantity | Minimal reference that crosses it |
| --- | --- | --- | --- |
| M0 link sensing | Nothing; SNR/dynamic-range bound only | Dynamic-to-static ratio (LoSRef handles −20 dB [17]) | A longer coherent dwell (integration) |
| M0 near-field | Fraunhofer boundary of the cell | `2D²/λ ≈ 1.4 m`; slope ∝ 1/R² | A larger cell (D), or a 5 GHz carrier (shorter λ) |
| M1 survey | Ambiguity cascade breaks | Coarse-range error vs synthetic-delay ambiguity (≈0.9 m at 80 MHz); direct-path phase bias vs λ/4 | **One tape-measured pair** (one known distance), or one fixed corner reflector |
| M1 range sharpness | Synthetic bandwidth | `c/2B_eff` ≈ 1.9 m at 80 MHz stitched | **One 5 GHz-capable chip** (ESP32-C5/C6: 160 MHz contiguous, ~500 MHz of UNII) or a UWB beacon |
| M2 manufacture coherence | Channel phase knowledge insufficient | `σ_φ(h)` = 2π·σ_d/λ ⊕ multipath phase error | **One coax between cells** (a conducted phase bus), or a corner reflector anchoring one LoS link |
| M2 two-way transfer (if used instead of T2) | Turnaround jitter × CFO | `Δω·σ_τ` (0.3 rad at 48 kHz × 1 µs) | An in-chip timed reply (absent on S3 [17]) or a disciplined TCXO (ESP-PPB pattern [16]) |
| Cross-home generality | Data association under change | MAP drift on moved furniture | One human "what changed" label, or drift-tolerant mapping |

Two honest boundary observations. (1) **The M2 boundary is the one the whole field is stuck on**, and this
invention's distinctive contribution is that M2 is *optional*: unlike the Coherent Weave (sibling note) or
cell-free ISAC [20], RGM's sensing claims do not depend on it. (2) **The sharpness that is *guaranteed*
without any coherence is modest** — cell-local 3-D within ~1–1.5 m (near-field, ~10° angle), link-local
Doppler and displacement elsewhere. The *sharp* whole-home image (1–3° cross-baseline) exists only in M2,
and M2 stands or falls on E1(d).

---

## 7. The decisive experiment

**E1 — The reciprocal-gauge null test and geometry closure (single decisive experiment; one rig, two
numbers, a decision tree).**

*Rig.* 4× ESP32-S3 (single antennas) at 2–5 m separations in a furnished room; one host aggregator; ground
truth: laser/tape distances (mm–cm) plus a **conducted reference pair** (coax + attenuators between two
nodes' RF ports) that carries the same frames for phase truth; optional fifth node as hub broadcaster.
Temperature step (≥10 °C) and scripted human motion during the run. PURR: ESP-NOW HT40 MCS0, 20 ms
nominal cadence, channels {1,6,11} then all 13, 30+ minutes.

*Measurements and thresholds:*
- (a) **Product-phase floor:** residual σ of `∠(ĥ_AB·ĥ_BA)` after CFO derotation and deferral gating, per
  link/channel, vs the conducted truth. **PASS ≤ 0.3 rad; FAIL > 1.0 rad.**
- (b) **Jitter budget:** measured distribution of reply intervals (ns RX stamps, software TX times) —
  the S3 answer to the Pi's <1 µs datum [6].
- (c) **Geometry closure:** all-pairs absolute ranges via the ambiguity cascade and the 4-node pose
  solution vs tape. **PASS ≤ 3 cm range RMSE and ≤ 10 cm pose RMSE with zero prior positions; FAIL
  otherwise** (→ M1 closed).
- (d) **Manufactured coherence (T2):** using only the (c) channels, predict each receiver's LO phase for
  *independent, later* one-way frames (from a hub node, and from an ambient AP); residual σ.
  **PASS ≤ 0.5 rad** (→ M2 unlocked); FAIL > 1.0 rad (→ M2 closed; M0/M1 stand).

*Decision tree.* Pass (a)+(c) → Mode 1 is real: a home mesh that surveys itself. Pass (d) as well → Mode 2
is real: a home-scale manufactured-coherence aperture. Fail (a) only → Mode 0 only (still a novel sensing
mesh: round-trip virtual-monostatic aperture + near-field cells). Fail (c) → M1 closed at the ambiguity
cascade; the minimal reference is one tape-measured pair (§6). Fail (d) but pass (c) → the fascinating
outcome "geometry without coherence": sharp local sensing, coarse global fusion. E1 is decisive because
**σ_φ(product) and σ_φ(predicted channel) are the two quantities that gate every claim above, and both are
one cable-truth measurement away.**

*Secondary, after E1:* **E2** single-cell near-field ranging on a rail (target ±0.5–3 m), **E3**
polarisation class separation, **E4** 6-node two-room survey repeatability (cross-home test), **E5**
Mode 2 localisation gain over Mode 1 (coherent vs incoherent image of a walker), **E6** static-map
stability/association (shared with sibling E3).

---

## 8. Direction audit (what the sweep found for each mission direction)

| Direction | Finding | RGM's use |
| --- | --- | --- |
| 1. Wireless coherence on commodity silicon | Locking loops exist as claims/FPGA ([1,16]); two-way phase products on COTS are proven for *ranging* [8] and *Doppler* [6,7] | T2: coherence *derived*, not locked — deliberately avoids this dependency |
| 2. Self-calibrating learned array | Array self-calibration [27], unsynchronised-array localisation [26], channel charting (note `03`) | S3/S7: measure geometry physically; learn only residuals |
| 3. Environment as shared reference | LoSRef [17], virtual anchors [25], sibling "Loom" | S6: map = ambiguity resolver + phase reference + virtual aperture |
| 4. Sharp 3-D from coarse apertures | Near-field bounds [22], holography [23,24], geodesic arrays (note `08`) | S1/S5: curvature + face diversity + virtual-monostatic synthesis |
| 5. Ping + unique replies + two-way products | Pieces exist pairwise [5–10]; fleet-scale *survey* does not | S2/S3: the protocol is the coordinate instrument |
| 6. New excitation/composition | ESPARGOS coherent TX "possible" [13]; cell-free ISAC theory [20,21] | S1: parallel conducted cell; no transmit beamsteering claimed — calibrated fixed phases only |

---

## 9. Chaining, data, and the minimal own-dataset (BRIEF §A/B/C)

**(A) Chaining.** `PURR (S2)` → *gauge-free products* → `scan (S3)` → *geometry + map* → `T2 (S4)` →
*coherence* → `M2 aperture` → *cross-baseline sharpness*; and independently `S2` → `M0 sensing` → `S5`
(near-field/Doppler/polarimetry) → `S6` map refinement → back into `S3`. The highest-leverage edge is
**S3→S4 (measured channel ⇒ coherence)** because it makes every coherence-dependent capability optional;
the second is **S2→M0**, which needs nothing else. Blocked-without edges: M1 needs the ambiguity cascade;
M2 needs S3 precision at the λ-fraction level or the §6 minimal references.

**(B) Existing models/data.** SenseFi/MM-Fi/CSI-Bench/OpenCSI and ESPARGOS datasets (coherent CSI) are
reusable for the learning layer with licence/hardware caveats as in note `19`; ESPARGOS One's specs and
open tooling [12,35] are the cell's closest engineering reference; WiDRa [8], Chronos [5], RTCSI/ML-Track
[6,7], PCMC [9b] and the distributed-MIMO calibration papers [1–4] are the algorithm references.

**(C) Own data — the minimal experiment/dataset.** E1 as specified *is* the dataset: 4-node, multi-channel,
PURR traffic with conducted phase truth and scripted motion — **no such dataset exists** (ESPARGOS has
conducted-coherent CSI but no two-way products or air/conducted pairs; ESP-PPB is unrefereed and
unsensored). Minimal viable version: 2 nodes + 1 coax truth link, 60 minutes, ≥3 channels — enough to
measure A1/A2 and publish the first ESP32-S3 reciprocal-phase characterisation. What would falsify the
whole programme: A1 and A3 both fail (product phase unusable *and* ambiguity cascade open) — then no
commodity-Wi-Fi mesh survey exists, and the honest product is note `02`-class RTI plus cell-local
near-field sensing, with the boundary table of §6 as the result.

---

## 10. Evidence strength and residual unknowns

**Strength: 6/10** for the invention as a whole — *pieces* are strong (round-trip phase on COTS <1 mm
differential [8]; multi-link round-trip sensing on Pi [7]; conducted ESP32 coherence [11,12]), but the
three load-bearing steps (S3 mesh survey, T2 manufactured coherence, volumetric S3 cell) are unproven, and
the two gating numbers are unmeasured. Layer strengths: M0 7/10; M1 5/10; M2 4/10; cell 5/10.

**Residual unknowns (ranked):** the product-phase noise floor and reply jitter on S3 (E1 a,b); ambient
direct-path phase stability per link in furnished rooms; mutual coupling/shadowing inside a solid
polyhedron; 40 MHz fan-out acceptance by S3 silicon; S3 CSI-quality differences vs S2/C61 for phase work
(an S3-specific gap flagged in notes `17`,`20`); polarisation separability; legality/duty shaping of
PURR (note `07`); whether ambient common-view has enough usable AP frames in a quiet home.

---

## 11. Sources

[1] H. V. Balan et al., "AirSync: Enabling Distributed Multiuser MIMO with Full Spatial Multiplexing",
arXiv:1205.6862; IEEE/ACM ToN 21(6), 2013. DOI 10.1109/TNET.2012.2230449.
[2] R. Rogalin et al., "Scalable Synchronization and Reciprocity Calibration for Distributed Multiuser
MIMO", arXiv:1310.7001.
[3] "Distributed MIMO With Over-the-Air Phase Calibration Integrated Into the TDD Flow", arXiv:2509.03722.
[4] "Over-the-Air Amplitude and Phase Reciprocity Calibration for Distributed MIMO", 2025 (ResearchGate
393642973).
[5] D. Vasisht, S. Kumar, D. Katabi, "Decimeter-Level Localization with a Single WiFi Access Point"
(Chronos), USENIX NSDI 2016.
[6] F. Shi, W. Li, C. Tang, P. Brennan, K. Chetty, "Doppler Sensing Using WiFi Round-Trip Channel State
Information", IEEE WCNC 2023; open access: discovery.ucl.ac.uk/id/eprint/10161602 (reply-interval
std < 1 µs; phase error 0.96→0.04 rad).
[7] F. Shi et al., "ML-Track: Passive Human Tracking Using WiFi Multi-Link Round-Trip CSI and Particle
Filter", IEEE TMC, 2025. DOI 10.1109/TMC.2025.3529897.
[8] V. V. Ratnam et al., "WiDRa — Enabling Millimeter-Level Differential Ranging Accuracy in Wi-Fi Using
Carrier Phase", IEEE JSAC 42(9), 2024, pp. 2410–2423; arXiv:2405.12168. [8b] related patent:
US 2025/0076484 (negotiating carrier-phase measurement interval in FTM).
[9] F. Shi et al., "Decimeter-Level Indoor Localization Using WiFi Round-Trip Phase and Factor Graph
Optimization", IEEE JSAC 42(1), 2024. DOI 10.1109/JSAC.2023.3322812.
[9b] J. P. Van Marter et al., "A Multichannel Approach and Testbed for Centimeter-Level WiFi Ranging"
(PCMC two-way CFR stitching), IEEE, 2024, doc. 10438856.
[10] "Bridging the Performance Gap Between Two-Way and One-Way CSI-Based 5 GHz WiFi Ranging", 2023
(ResearchGate 371751578).
[11] F. Euchner et al., "ESPARGOS: An Ultra Low-Cost, Realtime-Capable Multi-Antenna WiFi Channel
Sounder", arXiv:2502.09405 (PLL initial phase changes per reset/channel even with shared 40 MHz; conducted
frame reference; division calibration).
[12] ESPARGOS One specifications, espargos.net/espargos-one (8 dual-polarised antennas, λ/2, reference
in/out, multi-array aperture kits).
[13] ESP-SDR, espargos.net/espsdr and github.com/ESPARGOS/esp-sdr (undocumented raw IQ; ESP32-S3 streaming
mode; "phase-coherent transmission is possible" on ESPARGOS One, unreleased).
[14] ESP-Techpedia esp-crab (common-crystal C5 phase-sync; self-TX/dual-RX modes); espressif/esp-csi
issue #259 (missing HW files; 8-node C5 sounder).
[15] "Indoor Positioning with Wi-Fi Location: A Survey of IEEE 802.11mc/az/bk FTM Research",
arXiv:2509.03901.
[16] J. Muller, "ESP-PPB: wireless, battery-powered phase-coherent CSI synchronisation platform",
github.com/jonathanmuller/esp-ppb (ESP32-C3, FTM-disciplined VCTCXO, 1 ms reference frame; GPL-3.0;
unrefereed).
[17] "Wi-Fi Radar via Over-the-Air Referencing: Bridging Wi-Fi Sensing and Bistatic Radar" (LoSRef),
arXiv:2602.05344.
[18] NIST, "Two-Way Satellite Time and Frequency Transfer (TWSTFT)", nist.gov (mechanism and residual
asymmetries).
[19] M. Fujieda et al., "Carrier-phase Two-Way Satellite Frequency Transfer over a Very Long Baseline",
arXiv:1403.3193 (short-term instability 2×10⁻¹³ at 1 s).
[20] "Cell-Free Integrated Sensing and Communication: Principles, Advances, and Future Directions",
arXiv:2502.20345.
[21] "Distributed MIMO Measurements for Integrated Communication and Sensing in an Industrial
Environment", Sensors 24(5), 2024 (PMC10935044) — 1PPS + 10 MHz cabling; documented PLL lock-phase
ambiguity; OTA calibration requires static scene portions.
[22] "Near-field Tracking with Large Antenna Arrays: Fundamental Limits and Practical Algorithms",
arXiv:2102.05890 (Fraunhofer boundary, curvature-of-arrival ranging).
[22a] `22-invention-a.md`, this folder (sibling note; M2′, ESP-PPB, WiCAL, LoSRef, DCAR — cited there in
full and used here where marked).
[23] P. Holl and F. Reinhard, "Holography of Wi-Fi Radiation", Phys. Rev. Lett. 118, 183901, 2017.
DOI 10.1103/PhysRevLett.118.183901.
[24] "Through-the-Wall Imaging Exploiting 2.4 GHz Commodity Wi-Fi", arXiv:1903.03895.
[25] P. Meissner et al., "Multipath-assisted single-anchor indoor localization in an office environment",
IEEE, 2012; "Channel-SLAM", 2016; map-free multipath spatial consistency, IEEE 2026 (doc. 11429614).
[26] "Localizing Unsynchronized Sensors with Unknown Sources", arXiv:2102.03565.
[27] B. P. Flanagan and K. L. Bell, "Array self-calibration with large sensor position errors", Signal
Processing 81(10), 2001. DOI 10.1016/S0165-1684(01)00121-9.
[28] H. Zhu, Y. Zhuo et al., "π-Splicer: Perceiving accurate CSI phases with commodity WiFi devices",
IEEE TMC 17(9), 2018; X. Wang et al., "FarSense … CSI ratio of two antennas", IMWUT 3(3), 2019,
DOI 10.1145/3351279; "Wi-ESP … CSI-based device-free Wi-Fi sensing", JCDE 7(5), 2020 (ESP32 phase usable).
[29] ESP-IDF: Wi-Fi Vendor Features (esp_wifi_80211_tx frame types, ESP32-S3), docs.espressif.com;
esp-idf issue #9843 (WIFI_PKT_RX_TIMESTAMP_NSEC exposure); ESP32-S3 Clock Tree / Hardware Design
Guidelines (40 MHz crystal, XTAL fan-out notes).
[30] "Indoor Carrier Phase Positioning Technology Based on OFDM System", Sensors 21(20), 2021
(ambiguity resolution in OFDM carrier-phase ranging).
[31] "WiFi-Based Channel Impulse Response Estimation and Localization via Multi-Band Splicing",
arXiv:2011.10402 (per-channel PLL phase randomness in splicing).
[32] "Bistatic OFDM-based ISAC with Over-the-Air Synchronization", arXiv:2405.04962.
[33] ACMA, Radiocommunications (Low Interference Potential Devices) Class Licence 2015 (LIPD), and
note `07` of this folder (2.4 GHz EIRP/duty envelope).
[35] F. Euchner et al., "Phase-Coherent WiFi CSI Datasets for Wireless Sensing", IEEE, 2024
(doc. 10739065).

---

*Australian English throughout; grades are the author's calibration, not the cited works'.*
