# 22 — Invention: the Gauge Lattice

Inventor-researcher note for Aquila v2. Prompt suffix `<c>` = **n**.
Date: 2026-10-15. Australian English. Evidence graded **ESTABLISHED / HYPOTHESIS /
SPECULATION**. Sources are cited inline and listed in §12.

---

## 1. Mission and answer

**Mission.** Discover the invention that makes a home full of cheap ESP32-S3 nodes
behave as one sharp, self-calibrating, generalising 3D sensor — or prove that no
such invention exists and state the exact physical boundary that blocks it.

**Answer in one paragraph.** The invention exists, but not where the frame
"one coherent home-wide aperture" would put it. It is an **architecture**: a
constellation of **coherent 3D pucks** (each a ~30 cm polyhedron of ESP32-S3 faces
sharing a hub clock and a conducted 2.4 GHz phase reference, hence one
gauge-fixed 20-element 3D aperture), fused **only through gauge-invariant
observables** (3D bearings, differential delays, Dopplers, two-way products),
whose mutual geometry is solved **from shared environmental landmarks** — ambient
access points as physical anchors and wall reflections as virtual anchors —
rather than from a shared home-wide clock. Layered on top is an **opportunistic
coherence amplifier**: every Wi-Fi frame that several pucks can hear is a
common-view reference event, so each heard beacon is one constraint of the U(1)
gauge, and the constellation can become literally one coherent aperture for the
duration of that frame. I call the composite **the Gauge Lattice**. Its sharpness
is angular and Doppler, not range: the exact quantity that closes the
single-aperture *range* story on 2.4 GHz silicon is **coherent bandwidth**
(≤ ~83.5 MHz in-band ⇒ ≥ ~1.8 m resolution), crossed only by wider-band hardware.
Its static-single-puck phase story is closed by the **U(1) gauge** and crossed by
**one reference event per sensing frame** (or one round trip per link per
coherence interval).

The single decisive experiment is the **Two-Puck Room Test** (§7): two small
coherent pucks, zero transmitted illumination, bearings and shared landmarks
measured against ground truth. Everything load-bearing is falsified or
confirmed by it.

---

## 2. Method, and what changed since `00-synthesis.md`

Research used layered `parallel-search` → `parallel-fetch` waves on primaries:
project repositories, vendor documentation, arXiv/IEEE/journal papers, and
product pages. Six lines of enquiry (the six prompt directions) were each
grounded and then audited for novelty.

Four results **move the state of the art beyond the synthesis** and must be
folded into any v2 plan:

1. **The wireless escape now exists on ESP32-class silicon.** ESP-PPB
   (GitHub, Feb 2026) phase-locks *independent*, battery-powered ESP32-C3 nodes
   over the air using Wi-Fi FTM plus a DAC-disciplined VCTCXO: <0.1 ppb best /
   ~1 ppb typical clock alignment, <1° best / <10° typical single-frame phase
   accuracy, 10+ nodes tested, ~$50–80/node. It also documents the mechanism
   nobody had written down: when clocks differ, the ESP32's *internal* CFO
   correction differs per side and silently corrupts CSI phase; at ~1 ppb both
   corrections agree and CSI is clean. [ESP-PPB GitHub; `story.md`]
2. **The conducted architecture is now a shipping product.** ESPARGOS One
   (€350 for 8 dual-polarised faces at λ/2; coherent multi-array kits to 32
   faces) is a phase-coherent ESP32 array with a wired 40 MHz clock + Wi-Fi
   phase-reference distribution — the exact hub pattern the polyhedron premise
   needs, already reduced to practice. [espargos.net; buy.espargos.net]
3. **TX-free, phase-coherent CIR is demonstrated on commodity Wi-Fi.**
   LoSRef (arXiv 2602.05344, 2026) uses the transceiver's own LoS path as a
   per-frame delay and phase reference, producing phase-coherent delay–Doppler
   from *ambient* traffic on unmodified COTS devices, resolving target dynamics
   20 dB below static clutter. This is direction 3's "environment as shared
   reference" and direction 4's sharpness claim, already partially true.
4. **The two-way product is validated prior art, not a discovery.** RTCSI
   (WCNC 2023) and ML-Track (TMC 2025) cancel CFO with round-trip CSI and do
   bistatic Doppler tracking on commodity hardware; a 2025 arXiv result does
   simultaneous range–Doppler from a single Intel AX211 NIC with centimetre
   claims. Relative reciprocity calibration (Argos, Rogalin) and radio
   fingerprinting (IQ-imbalance/nonlinearity fingerprints) are established in
   adjacent communities. Direction 5 is therefore a **composition**, not a new
   physics.

Conversely, three things remain genuinely unbuilt: a **volumetric ESP32 sensing
array** (no polyhedron or spherical ESP32 aperture exists); **landmark-based
constellation self-calibration at 2.4 GHz** (multipath-SLAM is single-agent or
mmWave); and **cross-puck common-view referencing from ambient frames** (the
common-view principle is established in GNSS; nobody has run it as a Wi-Fi
coherence lattice).

---

## 3. The gauge, restated with today's numbers

### 3.1 The frame (ESTABLISHED, from synthesis `20`, re-verified)

With independent LOs, a measured CSI coefficient is

    ĥ_ij(f_k) = g_ij · a_i · b_j · e^{ j(θ_i − ψ_j) } · e^{ −j2π f_k τ_ij } + n,

where `a_i`, `b_j` are TX/RX chain coefficients, `θ_i − ψ_j` the instantaneous
cross-node LO phase, `τ_ij` the propagation delay. Only `θ_i − ψ_j` mixes the two
nodes' clocks into the observable; on a connected measurement graph the node
phases are fixed only up to one global additive constant (a U(1) gauge) plus a
time/frequency gauge.

**Gauge-invariant observables** (usable with no shared reference):

- amplitude and per-link path loss;
- **within-frame** subcarrier differential phase ⇒ delay/ToF *(per link)*;
- **within-receiver** inter-face differential phase ⇒ AoA *(per puck)*;
- temporal differentials ⇒ Doppler;
- **two-way products** `ĥ_AB·ĥ_BA` ⇒ cancels `θ/ψ` to first order, exposes chain
  asymmetry `a_A a_B b_A b_B` and round-trip delay `τ_AB+τ_BA`.

**Not observable without an added reference:** per-frame cross-node absolute
phase; absolute synchronisation; cross-receiver AoA. The minimal reference is
**one constraint per connected component per coherence interval** — a single
common-transmitter broadcast, one round trip per link, or a shared/conducted
tone (star topology is CRB-optimal).

### 3.2 The three escapes, with measured residuals

| Escape | Best evidence | Residual today |
| --- | --- | --- |
| Conducted / shared reference | ESPARGOS (S2 planar array, Phase-ref distribution over one coax; product with multi-array kits); esp-crab (C5, common crystal via clock buffer); Espressif's own S3+C3 CSI demo uses a clock buffer | Per-face chain + distribution delays **constant** and calibratable via reference frames (ESPARGOS) — quality limited by array calibration, not drift |
| Two-way / injected reference exchange | ESP-PPB (FTM + VCTCXO + ping-pong anchoring + reference frame 1 ms after each sensed frame): **<1° best, <10° typical** single-frame; AirSync (WARP FPGA): few degrees; OTA D-MIMO calibration literature: 0.01 rad ≈ 0.5 ps **simulated** | 1–10° on commodity silicon; effective holdover set by discipline quality: 1 ppb = 2.45 Hz at 2.45 GHz ⇒ ~880°/s free, so a 20 ms anchor-to-sense gap is ~18° at 1 ppb, ~1.8° at 0.1 ppb |
| Ambient common-view | LoSRef (per-frame referencing on COTS); common-view time transfer (NIST/GNSS: sub-ns, ~10 ns/day dual-frequency over 5 314 km) | Cross-puck carrier phase from ambient frames is **not yet demonstrated**; theory is trivial, residuals are empirical |

### 3.3 A design invariant worth naming: geometry and phase are conjugate

Cross-node phase can be recovered from a common-view event only when the
**path-length difference** is known: 1 mm of geometry error = 3° of carrier
phase at 2.45 GHz. Conversely, fine delay from carrier phase *measures*
geometry. So "make the home coherent" and "self-survey the home" are the same
estimation problem — solved jointly, exactly as carrier-phase GNSS resolves
position and clock together. This is the mathematical backbone of the invention.

### 3.4 The two physical boundaries, named

**B1 — phase (closed at the per-frame level).** With no reference *simultaneous
with the sensing observation*, cross-node absolute phase is unobservable.
Crossed by: **one reference event per sensing frame** (a common transmitter the
nodes can hear at that instant, or an injected reference frame), or **one round
trip per link per coherence interval** plus holdover. On commodity silicon the
crossing buys 1–10° (measured, ESP-PPB), which is sufficient for coherent
combining (λ/20 at 2.45 GHz is 6 mm ≈ 177° — so even 10° is 18× inside the
tolerance needed for beamforming) but sets a floor on cross-puck *absolute*
phase for array processing.

**B2 — range (closed by bandwidth).** Coherent bandwidth in the 2.4 GHz ISM is
≤ ~83.5 MHz even if all thirteen channels are stitched with perfect phase
continuity ⇒ **ΔR = c/2B ≥ ~1.8 m**. Measured with one shot: 40 MHz ⇒ 3.75 m;
ESP-SDR raw IQ burst (13–54 MHz) ⇒ ~2.8 m. The minimal reference that crosses
B2 is **wider-band hardware**: 5/6 GHz VHT80 (0.94 m) or 802.11bk's 320 MHz
(0.47 m), or a **UWB transceiver per face** (500–900 MHz ⇒ 15–30 cm) at ~$3–8
per face. On S3 alone, sharp *range* is impossible; sharpness must come from
**angle × Doppler × home-scale geometry**.

**B3 — aperture (soft).** A 30 cm sphere at 2.45 GHz is a 2.45λ aperture:
Rayleigh ~28°, mitigated by 3D irregularity (no front–back ambiguity), dual
polarisation, TDM-MIMO (20×20 = 400 virtual elements spanning up to ~60 cm),
super-resolution at usable SNR, and — decisively — **home-scale baselines**
between pucks. The physical route to finer angles is more pucks or higher
frequency (C5 at 5 GHz halves λ).

---

## 4. The invention: the Gauge Lattice

```
      ambient APs (common-view beacons; physical anchors)
          \            |            /
           \           |           /
        ┌───┴───────────┴───────────┴───┐
        │   PUCK A  —  20 faces, hub:   │   each puck:  one coherent
        │   40 MHz clock + 2.4 GHz phase │   3D aperture (gauge fixed
        │   ref + aggregation; dual-pol  │   by construction)
        └───┬───────────────┬───────────┘
            │  two-way anchor lattice (ping + unique replies, FTM discipline)
        ┌───┴───────────────┴───────────┐
        │   PUCK B  —  same               │
        └────────────────────────────────┘
             walls = virtual anchors (shared landmarks)
```

### I-1 — The coherent 3D puck (gauge-fixing device)

- **Genuinely new**: the first *volumetric* ESP32 sensing aperture. Prior art is
  planar (ESPARGOS One 2×4 dual-pol), or non-ESP32 (geodesic-dome phased arrays
  for satellites; spherical-array DoA theory). A soldered/flex polyhedron of
  ESP32-S3 panels sharing one hub reference exists nowhere in the literature or
  the maker record.
- **Mechanism**: the hub distributes a 40 MHz reference (ESPARGOS pattern: one
  coax/microstrip network carrying clock + Wi-Fi phase-reference frames,
  separated by filters) and aggregates. The reference fixes the U(1) gauge
  across all 20 faces — the cheapest possible gauge fixing, because one
  constraint covers 20 phase degrees of freedom. Face geometry is known to PCB
  tolerances by *construction*; internal distribution delays are calibrated
  continuously by hub reference frames. Each face can be dual-polarised (as
  ESPARGOS One is). Face-TDM MIMO gives a 400-element virtual array.
- **Physical claims**: 3D AoA from within-receiver differential phase (an
  invariant — no cross-puck coherence needed); polarisation diversity separates
  some multipath classes; curvature removes front–back ambiguity.
- **What must be true**: (A1) conducted clock+phase-ref works on ESP32-S3 (shown
  on S2/C5, *not yet on S3*); (A2) mutual coupling and face shadowing on a solid
  body leave the array manifold invertible (unmeasured — the top hardware risk);
  (A3) distribution delays are constant (ESPARGOS: yes, on their board).
- **Falsifying experiment**: E1 (§7) — per-face phase stability and bearing
  accuracy under real multipath.
- **ESP32-S3 feasibility**: high. CSI HT40 = 114 subcarriers; external 40 MHz
  XTAL input documented; ESP-SDR gives raw IQ bursts (80 MSa/s, 13–54 MHz);
  RF switch per face exactly as ESPARGOS does. Caveat: esp-crab's shared-crystal
  topology needs a **clock buffer**; direct crystal sharing has been reported to
  fail to boot slave chips (community, GitHub issue espressif/esp-csi#259).
- **Cost**: DIY $250–400/puck (20× S3 modules ≈ $80, PCB/assembly $100–150,
  clock fan-out $10, RF switching $30, PoE $20–50); buy-don't-build today:
  3× ESPARGOS One (24 dual-pol faces) = €1 050; 32-face kit €1 735.
- **Transmit needed?** No (passive mode works from ambient AP traffic).

### I-2 — Per-frame over-the-air LoS referencing inside a coherent array

- **Genuinely new**: LoSRef is single-port COTS; nothing combines per-face
  self-referencing with a *shared hub reference* so that the 20 referenced CIRs
  of one heard beacon form one phase-coherent 3D snapshot, with *zero* own
  transmissions.
- **Mechanism**: for each face, the dominant arriving path (the direct path from
  an ambient AP, normally early and stable) is aligned in delay and phase per
  frame (§LoSRef). Across faces, this referencing is *consistent* because the
  hub reference already ties the faces' LOs; the geometric part of the direct-path
  phase difference across faces is exactly the known array response — so the
  referenced per-face residual phases are the *target* echo phases, coherently
  comparable. The same machinery supports clutter removal (subtract the static
  average), giving moving-target isolation 20 dB below clutter (demonstrated
  on COTS by LoSRef).
- **What must be true**: (A4) a dominant, resolvable direct path in a 40 MHz CIR
  at room scale (usually true; walls and NLoS rooms are the failure mode);
  AGC/CFO compensation behaves as ESP-PPB reports (per-side CFO correction
  consistent when clocks are disciplined).
- **Falsifying experiment**: E1 — referenced-phase variance across frames for a
  static scene; if > ~20° RMS in a furnished room, TX-free coherent 3D dies.
- **ESP32-S3 feasibility**: moderate–high; ESP32 CSI phase is known noisy but
  showable with the clock-buffer discipline (Espressif's own demo) — S3-specific
  measurement needed.
- **Cost**: nil beyond I-1. **Transmit needed?** No.

### I-3 — Invariants-only fusion + landmark bundle adjustment (the self-calibration)

- **Genuinely new**: static infrastructure nodes solving their **own** relative
  SE(3)+clock pose from **shared landmarks they observe**, not from known anchor
  positions and not from a shared clock. Channel-SLAM treats multipath as
  virtual transmitters for a *mobile* agent; channel charting with real-world
  coordinates (Taner et al. 2025) needs *known* O-RU positions for alignment;
  cooperative virtual-transmitter maps (2018) still live in per-user coordinate
  frames. The missing step — using shared landmarks (physical APs + wall
  reflections) to joint-register *static* coherent arrays — is what I propose.
- **Mechanism**: a pose graph. Nodes: pucks (position, rotation, clock offset).
  Landmarks: ambient APs (physical anchors, positions unknown but joint-solved)
  and wall reflections (virtual anchors; their positions are mirror images).
  Factors: 3D bearing vectors per landmark per puck (from I-1); coarse ranges
  from FTM; hybrid TDOA/range factors from I-4/I-5; integer-resolved carrier
  phases when I-5 locks. Initialisation: FTM all-pairs + MDS + Procrustes
  (synthesis §6.5); then bundle adjustment with robust association.
- **What must be true**: (A5) landmark association across pucks is solvable from
  bearings + coarse range + persistence (walls do not move); (A6) homes supply
  ≥3 associable landmarks per room (2+ APs or 1 AP + 2 wall facets — likely);
  (A7) FTM+bearing initialisation is inside the basin of the joint optimum.
- **Falsifying experiment**: E1 arm 2 — two pucks, recover relative pose and
  landmark constellations; fail if pose error >0.5 m / >15° or association
  fails.
- **Feasibility**: host/hub compute; ESP32 only measures. **Cost**: nil.
  **Transmit needed?** No (ambient), low-duty (self-survey).

### I-4 — The anchor-lattice protocol (scheduled ping + unique replies + two-way products)

- **Genuinely new (carefully)**: each primitive already exists —
  round-trip products cancel CFO and yield Doppler (RTCSI WCNC'23; ML-Track
  TMC'25); reciprocity calibration from two-way products is Massive-MIMO
  orthodoxy (Argos 2012; Rogalin 2014; BeamSync 2024; 2026 OTA-calibration
  papers simulate 0.5 ps); hardware asymmetry as identity is RF fingerprinting
  (USENIX'09; IQ-imbalance fingerprints arXiv 2104.10397). What is new is the
  **lattice pass**: one scheduled round over all node pairs producing a *matrix*
  of observables — round-trip delay (phase slope), asymmetry product
  (fingerprint/calibration), two-way Doppler, path loss and variance — used
  jointly as range graph, calibration graph, identity graph, landmark spectra
  and obstruction map (RTI), on ESP32, with frequency discipline (FTM+VCTCXO)
  so the product phases are usable.
- **Mechanism**: A pings; every peer replies with a *unique* frame within the
  same coherence interval. Per pair:
  `ĥ_AB·ĥ_BA = (a_A a_B b_A b_B) · g_AB g_BA · e^{−jω(τ_AB+τ_BA)} · e^{jΔψ_drift}`.
  Frequency slope over the 114 subcarriers ⇒ round-trip delay (cm-class at high
  SNR; coarse at low). The zero-frequency value ⇒ asymmetry vector across pairs
  — a rank-1 matrix in `a_i b_j`, solvable per node up to one scalar (relative
  calibration). Its slow drift over hours is a *node identity* (fingerprint);
  its stability also lets one-way CSI be chain-corrected. Time series ⇒ two-way
  Doppler (bistatic sensitivity, ×2 on monostatic paths). Unique replies remove
  ambiguity; the MAC's lack of a TX timestamp is irrelevant because the phase
  slope, not the timestamp, carries the delay (RX nanosecond timestamps exist
  for coarse alignment: `WIFI_PKT_RX_TIMESTAMP_NSEC`, used by ESP-PPB via an
  IDF patch).
- **What must be true**: (A8) the exchange fits a phase-coherence interval at
  the achieved discipline (ESP-PPB holds ~1 ppb; ms-scale turns are fine);
  (A9) MAC queue latency does not exceed the anchor-to-sense budget; (A10)
  phase-slope ranging accuracy < ~0.5–1 m at room range after averaging.
- **Falsifying experiment**: E2 — two nodes, tape-measured separations; if
  product-phase-slope range error exceeds ~20% of range, or product phase is
  unusable at 20 ms cadence, the self-survey degrades to FTM+RTI only.
- **ESP32-S3 feasibility**: demonstrated primitives (`esp_wifi_80211_tx`,
  ESP-NOW, CSI callbacks; ESP-PPB ping-pong). **Cost**: nil. **TX needed?** Yes,
  low duty (short frames; sensing itself can stay passive).

### I-5 — The RTK layer: ambient common-view referencing as a coherence amplifier

- **Genuinely new in Wi-Fi form**: every beacon frame that two or more pucks
  receive **simultaneously** is a common-view reference event (NIST/GNSS
  common-view: sub-ns time transfer; carrier-phase common-view is the backbone
  of international time comparison). On Wi-Fi CSI this has not been done; the
  closest art injects its own reference frames (ESP-PPB) rather than harvesting
  ambient ones. Think of it as **carrier-phase differential positioning
  ("RTK") against the home's access points**.
- **Mechanism**: from one heard frame, each puck estimates per-subcarrier
  frequency slope ⇒ one-way delay `d_i/c + δ_i` where `δ_i` is its own sampling
  /chain offset; the *slope difference* across pucks cancels per-frame LO phase
  entirely (a pure invariant) and yields **differential path length**
  `d_i − d_j + (δ_i−δ_j)`. `δ_i` is a stable per-puck constant (calibrated
  jointly with geometry). FTM supplies the metre-scale integer; carrier phase
  supplies the centimetre part; the constellation solve (I-3) supplies the
  geometry that converts path differences into **cross-puck absolute phase for
  that frame** — i.e. one gauge constraint per connected component per frame,
  from which coherent combining across pucks is possible *for that frame and
  the objects it illuminates*.
- **What must be true**: (A11) simultaneously-received frames are identifiable
  across pucks (shared clock or ns timestamps + MAC/subcarrier fingerprinting —
  ESPARGOS clusters frames exactly this way); (A12) per-puck `δ_i` is stable to
  ≲1 ns over minutes to hours (measurable; LoSRef-style first-path alignment
  removes the per-frame packet-detection delay component); (A13) geometry
  knowledge of a few millimetres for few-degree cross-puck phase (from I-3+I-5
  fixed-point iterations) — the conjugate-variable property of §3.3.
- **Falsifying experiment**: E3 — two disciplined nodes, one ambient AP;
  differential-delay variance across a 10-minute static run. If > ~2 ns RMS,
  the lattice never reaches coherent combining and stays an invariant-fusion
  system (still the core invention).
- **ESP32-S3 feasibility**: measurement side yes (CSI or raw IQ; ns timestamps);
  the limiting question is empirical stability. **Cost**: nil. **TX needed?**
  No.

### I-6 — Stretch: puck-internal coherent channel stitching, and the wireless ladder

- **Puck-internal stitching** (plausible new use of the hub): the PLL phase
  re-randomises per channel even with a shared clock (ESPARGOS), but the hub's
  conducted phase reference resolves that re-lock at every hop, so a puck can
  hop across the thirteen 2.4 GHz channels and stitch an **~83.5 MHz coherent
  aperture** (§B2), roughly halving range bins versus one channel and enabling
  super-resolved delay via phase slope. Not demonstrated; falls out of the
  ESPARGOS calibration loop.
- **Wireless ladder** (amplifier for coherent *transmit* only): FTM+VCTCXO
  discipline (ESP-PPB) + per-frame anchoring + LoSRef referencing. With this,
  a home could in principle transmit coherently from several pucks at once.
  Because sensing does not need it (invariants suffice), it is deliberately
  **not** load-bearing in this proposal. Falsifier: M2 residual measurement
  (synthesis §6.1).
- **TX needed?** Stitching: no (receive-only hopping). Ladder: yes.

### I-7 — Optional: coherent-TX structured illumination

A puck with calibrated faces can transmit a *steerable 3D beam* (ESPARGOS
reports coherent TX is possible on its hardware but is not released because of
misuse potential). A sweeping searchlight from one puck, received passively by
the others, gives illumination diversity: targets are switched on and off by
beam position, materially improving clutter rejection in the angle–Doppler
domain where range is hopeless (B2). This is engineering, not new physics;
mention it because it is the natural upgrade if the ambient-illumination path
is starved of traffic in a quiet home.

### Novelty ledger (to keep claims honest)

| Primitive | Nearest prior art | What the Lattice adds |
| --- | --- | --- |
| Coherent multi-face ESP32 array | ESPARGOS (planar, S2), esp-crab (C5 pair) | 3D polyhedron, 3D bearings + polarisation + 400-virtual-element MIMO |
| Per-frame LoS referencing | LoSRef (single-port COTS) | Per-face referencing under a hub reference ⇒ coherent 3D snapshots, TX-free |
| Multipath virtual anchors | Channel-SLAM, DC-SLAM, mmWave SLAM | Landmark-based **joint registration of static arrays**; environment as shared fiducials |
| Two-way products | RTCSI, ML-Track, reciprocity calibration, RFF | One lattice pass extracting ranges + fingerprints + landmark spectra + RTI jointly on ESP32 |
| Common-view referencing | GNSS common-view time transfer | Ambient Wi-Fi PPDUs as the reference events; carrier-phase differential ("Wi-Fi RTK") |
| Wireless clock discipline | ESP-PPB (FTM+VCTCXO) | Used as an accessory, not a prerequisite; separate anchors from illumination |

---

## 5. Sharpness budget (what "sharp" actually means here)

Numbers are engineering estimates from cited primaries; those marked ~ are
design targets to be measured (hypotheses), not demonstrated results.

| Dimension | Physics | Achievable on S3 pucks | Notes / evidence |
| --- | --- | --- | --- |
| 3D bearing (per puck) | Rayleigh ~λ/D ≈ 28° at 2.45λ; irregular 3D geometry, dual-pol, 400-element virtual array, super-resolution | **~1–10° RMS target** | Limited by calibration residual (ESP-PPB: <1° best, <10° typical phase) and multipath, not by CRB; multipath is the binding term in rooms (HYPOTHESIS; E1 decides) |
| Cross-puck phase | Common-view + geometry (conjugate variables, §3.3) | **1–10° typical, few-degree best** | ESP-PPB measured single-frame phase; holdover at 1 ppb is ~18°/20 ms; E3 decides whether ambient frames improve it |
| Range (delay) | c/2B | 3.75 m (40 MHz CSI), ~2.8 m (54 MHz IQ burst), ~1.8 m (stitched ISM), ~15–30 cm with UWB face | B2; 802.11bk 320 MHz ⇒ 0.47 m is the standard-track answer at 6 GHz |
| Doppler | λ/(2T) unambiguous; 1/T resolution | ±6 m/s unambiguous at 10 ms; sub-Hz (cm/s) resolution in seconds | Established; two-way products double sensitivity on reciprocal paths (prior art) |
| Fused 3D target position | Bearing triangulation across home-scale baselines | **tens of centimetres at room range with 3+ pucks** (e.g. 5° at 4 m ⇒ 35 cm lateral, less in depth) | Geometry-dependent; the point of home-scale baselines is to buy what a 30 cm aperture cannot give |
| Clutter rejection | Angle + Doppler (+ polarisation) instead of range | moving-vs-static isolation ≥20 dB **if** referencing works | LoSRef shows 20 dB below clutter on COTS with 80 MHz; lower bandwidth on S3 ⇒ rely on angle/Doppler more |

**What is not achievable on 2.4 GHz S3 silicon**: millimetre/centimetre range
imaging; penetrating range-gated "radar video" of a room; monostatic on-chip
self-sensing at long range (self-interference ≈110 dB problem, unchanged).

---

## 6. Audit of the six search directions

**(1) Wireless coherence on commodity silicon.** Now real: ESP-PPB (FTM +
VCTCXO + two-way anchoring + reference frames) achieves ~1 ppb frequency and
<1–10° single-frame phase across 10+ ESP32-C3 nodes, wirelessly, battery-powered.
ESPARGOS proves the conducted variant. AirSync/OTA-calibration literature bounds
what is possible (few-degree phase on FPGA; 0.5 ps *simulated* for 2026 D-MIMO
protocols). Residuals at the sensing instant remain the open empirical question
(E3), but the direction is no longer blue-sky. **This is the single most
important capability update for v2.**

**(2) Self-calibrating learned array; joint discovery; transfer.** Channel
charting is mature (Studer group; ESPARGOS datasets include charting demos;
2026 work adds position *and orientation*), but alignment to physical space
needs known anchor positions or labels. The Lattice's route is the complement:
solve the *physical* frame from shared landmarks (I-3), and keep learning on top
of physical invariants (bearings/delays/Dopplers), which is what makes
cross-home transfer plausible — the estimator, not the weights, is the reusable
object. Supervision ladder from synthesis §5 remains valid.

**(3) Environment as shared reference → distributed aperture.** Two established
halves: multipath virtual anchors (Channel-SLAM/DC-SLAM/mmWave SLAM) and
per-frame LoS referencing (LoSRef). The lattice's addition is using the same
landmarks as a *shared fiducial system across static nodes*, and using ambient
APs as common-view events. Walls become the calibration standard every home
supplies for free.

**(4) Sharp 3D from many coarse apertures.** The honest accounting: a 30 cm
aperture cannot beat the CRB, but it changes the *kind* of measurement —
3D bearings instead of 1D range; the volumetric geometry removes front–back
ambiguity and supports polarimetric separation; face-TDM MIMO builds a 400-
element virtual aperture; home-scale baselines carry depth. Range, however, is
bandwidth-closed (B2). The round-trip hardware-asymmetry signature is real but
is calibration/identity (prior art), not a sharpness mechanism.

**(5) The protocol.** Ping + unique replies + two-way products recovers:
round-trip ranges (phase slope), chain asymmetry per pair (rank-1 system ⇒
per-node calibration + identity), two-way Doppler, path loss/variance
(obstruction maps via RTI, established), and — with frequency discipline — a
per-exchange LO-phase anchor. Failure modes: multipath (all pairs), exchange
latency vs coherence, and ambiguity without unique replies. The composition is
new; the parts are not.

**(6) Excitation/composition.** TDM MIMO across faces is mandatory and cheap
(face switching); coherent TX within a puck is optional and gated by
self-interference/legal constraints (ESPARGOS withholds it); passive
illumination by ambient APs is the default and is the reason this design can be
TX-free; "synthetic aperture by switching" across *pucks* fails without
cross-puck phase, and with it is exactly I-5.

**Beyond the six.** Two additions survived the audit: (a) the
geometry↔phase conjugacy as a *solver structure* (joint carrier-phase/geometry
bundle adjustment, GNSS-style); (b) the identity/security corollary — a node
cannot spoof its two-way asymmetry + range signature, so the lattice doubles as
a physical-layer trust anchor (small, real, secondary).

---

## 7. Falsification programme

### Assumption ledger

| # | Assumption | Killed by |
| --- | --- | --- |
| A1 | Conducted clock + phase ref works on S3 | E1 part 1 |
| A2 | Solid-body coupling/shadowing tolerable | E1 part 1 |
| A3 | Distribution delays constant | E1 part 1 |
| A4 | Dominant-path referencing usable in rooms | E1 part 2 |
| A5 | Landmarks associable across pucks | E1 part 3 |
| A7 | FTM+bearing init inside basin | E1 part 3 |
| A8/A9/A10 | Two-way exchange fits coherence; slope-ranging works | E2 |
| A11/A12/A13 | Ambient common-view stable at ≲2 ns; conjugacy converges | E3 |
| A6 | Homes have enough landmarks | Home survey (desk study) |

### The decisive experiment — **E1: the Two-Puck Room Test**

Build **two minimal coherent pucks**. Cheapest faithful version: two ESPARGOS
One boards per puck, mounted orthogonally (3D coverage), sharing one reference
chain; or DIY: 8–16 ESP32-S3 faces on a plastic/printed frame with a hub face
carrying the 40 MHz buffer fan-out, RF phase-ref splitter and a small aggregator.
Place them at tape-measured positions in a furnished 6×4 m room with a normal
Wi-Fi AP running (ambient illumination only — **no puck transmits, except the
hub's conducted reference frames**).

Measure, in one data set:

1. **Face coherence**: residual phase across faces after hub-reference and
   per-frame referencing, static scene (target: <10° best, <20° usable).
2. **3D bearing**: azimuth/elevation to a person walking a scripted grid
   (ground truth: floor markers + laser range); median bearing RMSE per puck.
3. **Landmarks and pose**: per-puck bearing/(coarse) delay sets for wall
   reflections and APs; solve the two-puck relative pose and landmark map;
   compare with tape (target: translation ≤0.5 m, rotation ≤15°, ≥3 associated
   landmarks).
4. (Optional) **Product sanity**: one two-way exchange round; product-phase
   slope vs tape distance.

**Pass criterion**: (2) ≤5° azimuth / ≤8° elevation median; (1) <20°; (3) pose
within target with ≥3 associated landmarks. **Fail** ⇒ the "sharp 3D,
self-calibrating" claim collapses to per-link Doppler/RTI (still useful; the
commodity route of the v2 map survives). Cost ≈ €1 400 (three ESPARGOS One
boards) or ≈ $300 DIY; effort: weeks. Falsification is cheap, local and total.

### Secondary decisive experiments

- **E2 (protocol)**: two S3 nodes with VCTCXO discipline (ESP-PPB BOM);
  measure two-way product phase-noise vs exchange interval and phase-slope
  range vs tape. Gates I-4's ranging claims.
- **E3 (RTK layer)**: two disciplined nodes listening to one AP; differential
  delay variance over 10 minutes. Gates I-5's coherence amplifier.
- **E4 (stitching)**: single puck, hop thirteen channels with hub reference;
  stitched impulse response vs single-channel. Gates I-6.
- **E5 (coupling)**: measure the 20×20 mutual-coupling matrix of a built
  polyhedron; compare plane-wave manifold to ideal. Gates I-1's linearity.

---

## 8. ESP32-S3 feasibility and integration blueprint

| Item | Status on ESP32-S3 | Evidence |
| --- | --- | --- |
| CSI, HT40, 114 subcarriers | Yes (LLTF/HT-LTF; 20/40 MHz; default HT40) | ESP-IDF Wi-Fi docs |
| External 40 MHz reference input | Yes (XTAL_P/XTAL_N accept an external clock/oscillator) | ESP32-S3 datasheet; clock-tree docs |
| Clock fan-out to many chips | Yes via clock buffer (esp-crab pattern); direct crystal sharing unreliable | Espressif ESP-Techpedia; esp-csi#259 |
| Conducted phase reference | Proven on ESP32-S2 (ESPARGOS); S3 not yet | ESPARGOS papers; espargos.net |
| Nanosecond RX timestamp | Yes (`WIFI_PKT_RX_TIMESTAMP_NSEC`; ESP-PPB patches IDF for ns promiscuous timestamps) | synthesis `17`; ESP-PPB |
| FTM | S3: 2.4 GHz HT20 only; metre-level accuracy | synthesis `15`; ESP-IDF FTM docs |
| Raw IQ (esp-sdr) | S3 supported; 80 MSa/s bursts; ~13–54 MHz analogue BW; gapped, uncalibrated | espargos.net/espsdr; esp-sdr repo |
| Two-way ping/unique reply | Yes (`esp_wifi_80211_tx`, ESP-NOW; ESP-PPB ping-pong runs on C3) | ESP-IDF docs; ESP-PPB |
| CSI excitation | ESP-NOW HT40 MCS0 (esp-csi pattern) | synthesis `07`; esp-csi |
| On-puck inference | ESP-DL INT8 SIMD exists (vision-demonstrated); host-side preferred for array processing | synthesis `19` |
| Chip ranking note | C5 > C6 > C3 ≈ S3 > ESP32 for CSI quality; C5 adds 5 GHz (λ halves) | Espressif ESP-Techpedia |

**Hub face blueprint** (per the premise, and consistent with ESPARGOS): 40 MHz
oscillator + buffer fan-out; Wi-Fi phase-reference generator multiplexed onto
the same distribution network (high-pass/low-pass split at each face, as
ESPARGOS does); per-face RF switch between antenna and reference; PoE
Ethernet/switch or an S3/FPGA aggregator; ns-timestamp patch; TDM schedule
generator. Faces: S3 module + dual-pol patch (or patch pair) + RF switch +
per-face microstrip reference tap.

**Compute placement**: per-face feature extraction (phase slope, amplitude) on
the S3; MUSIC/atomic-norm and the pose graph on the hub/host; the bus is
CSI-sized (~kB/frame/face in 40 MHz), fine for PoE.

---

## 9. Cost

| Item | Cost (indicative) |
| --- | --- |
| DIY puck, 20 faces | $250–400 (S3 modules $80; PCB/assembly $100–150; fan-out $10; RF switching/antennas $30; PoE/aggregation $30–100) |
| Off-the-shelf puck path | ESPARGOS One €350 per 8 dual-pol faces ⇒ €1 050 for a 24-face 3D rig; €1 735 for 32 faces |
| Home (4 pucks) | $1.0–1.6 k DIY; €4.2 k off-the-shelf |
| Wireless discipline node (ladder) | $50–80 (ESP-PPB-class BOM: C3 + VCTCXO + dual DAC) |
| Minimal decisive experiment (E1) | €1 400 purchased, ~$300 DIY |
| Ground truth (E1) | total-station hire or laser + floor grid (~$100–300) |
| UWB face upgrade (crossing B2) | +$3–8 per face (DW3000-class) |

---

## 10. Evidence strength and residual unknowns

| Claim | Grade (1–10) | Why |
| --- | --- | --- |
| U(1) gauge / minimal-reference result | 9 | Multiple formal treatments; unchanged |
| Conducted ESP32 coherence (planar) | 9 | ESPARGOS: papers, product, datasets, multi-array |
| *Wireless* ESP32 coherence (FTM+VCTCXO+anchoring) | 6 | Open-source, detailed, self-reported, 10+ nodes; single antenna; no independent replication yet |
| Per-frame LoS referencing (COTS) | 8 | arXiv 2026, several devices, quantified clutter margin |
| Two-way product Doppler/ranging on COTS | 8 | IEEE WCNC/TMC; single-NIC range-Doppler (arXiv, 6/10 for the cm-level claim) |
| Multipath virtual anchors / multipath SLAM | 9 | Channel-SLAM lineage; Wi-Fi and mmWave implementations |
| Volumetric ESP32 sensing array | 2 | Nothing exists; established theory only |
| Landmark-based static-array self-registration at 2.4 GHz | 2 | Proposed here; components established |
| Ambient common-view carrier-phase lattice on Wi-Fi | 2 | Common-view established in GNSS; Wi-Fi form proposed here |
| Lattice as a whole | 3 | Coherent design hypothesis; buildable with known primitives |

**Residual unknowns.** (i) Mutual coupling/shadowing of a soldered solid
polyhedron — the top hardware risk; (ii) per-frame phase variance on S3 CSI
under real AGC/multipath; (iii) stability of per-puck sampling/chain offsets
(the number that decides the RTK layer); (iv) landmark associability at
2.4 GHz with 30 cm arrays; (v) MAC queue latency vs coherence cadence for the
two-way lattice; (vi) ambient traffic density in quiet homes (drives the need
for I-7); (vii) whether the S3-specific phase-reference calibration behaves as
ESPARGOS's S2 does; (viii) legal duty/airtime for continuous two-way survey
frames (ACMA LIPD/FCC 15.247 envelopes); (ix) thermal drift of VCTCXO loops in
closed enclosures; (x) compute cost of joint polarimetric 3D super-resolution
per frame at home node count.

---

## 11. Chaining, existing models/data, own data

**Chaining.** Highest-leverage enablers, ranked: (1) **hub reference inside the
puck** — fixes 20 phase DOF with one constraint, and unlocks I-1/I-2 (3D
bearings) and hence I-3; (2) **per-frame LoS referencing** — makes every
ambient frame usable and is the difference between "CSI phase is unusable" and
"TX-free coherent 3D"; (3) **FTM+VCTCXO discipline** — unlocks I-4's phase
products and I-5's holdover; (4) **two-way lattice** — self-survey ranges,
fingerprints, identity; (5) **ambient common-view (RTK layer)** — upgrades
invariant fusion to opportunistic home-wide coherence. Capability that unlocks
a further one: the landmark self-registration (I-3) converts a collection of
sensors into a *coordinate system*, which is what every downstream mapping and
learning task needs.

**Existing models and data.** ESPARGOS datasets (phase-coherent CSI, multi-array
rooms, channel-charting demos, DOI datasets on DaRUS); SenseFi, MM-Fi,
CSI-Bench, OpenCSI (all non-ESP32 — domain adaptation mandatory); DiChasus
(timestamp/triplet channel charting tutorials); LoSRef and RTCSI/ML-Track
processing pipelines as reference implementations; esp-csi, esp-sdr (GPL-3),
pyESPARgos (open). Licences: esp-sdr GPL-3; ESP-PPB GPL-3; ESPARGOS firmware
open (see repo); datasets generally permissive with attribution — verify before
vendoring.

**Own data (the missing dataset).** *Lattice Benchmark v0*: two minimal
coherent pucks in a furnished room, ambient illumination only, scripted
trajectories with total-station ground truth, per-frame hub-reference frames
included, plus one two-way survey round and raw IQ bursts on a subset. Minimal
version: 2 pucks, 1 AP, 20 trajectories, 1 hour. This dataset does not exist
publicly and it is exactly what E1 produces as a by-product. It would prove or
falsify: per-face phase stability, 3D bearing accuracy, landmark assoc­iability,
and product-slope ranging — the four unknowns that decide the invention.

---

## 12. References

Primaries were fetched or directly excerpted during research; where only a
secondary citation was seen, it is marked "(as cited in)".

**Coherent ESP32 arrays and hardware**

1. F. Euchner et al., “ESPARGOS: An Ultra Low-Cost, Realtime-Capable
   Multi-Antenna WiFi Channel Sounder,” arXiv:2502.09405.
   https://arxiv.org/abs/2502.09405
2. F. Euchner and S. ten Brink, “ESPARGOS: Phase-Coherent WiFi CSI Datasets,”
   arXiv:2408.16377. https://arxiv.org/abs/2408.16377
3. ESPARGOS project pages: https://espargos.net/ · https://espargos.net/espargos-one/
   · https://espargos.net/espsdr/ · https://espargos.net/docs/combined-arrays.html
   · pricing: https://buy.espargos.net/
4. ESP-SDR firmware: https://github.com/ESPARGOS/esp-sdr (GPL-3.0).
5. J. Muller, “ESP-PPB: Wireless phase-coherent Wi-Fi CSI,”
   https://github.com/jonathanmuller/esp-ppb and `story.md` (GPL-3.0).
6. Espressif, ESP32-S3 Series Datasheet v2.2 (external clock input; HT40).
   https://documentation.espressif.com/esp32-s3_datasheet_en.pdf
7. Espressif, ESP-IDF Wi-Fi Driver (CSI, HT20/40, subcarrier tables).
   https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/api-guides/wifi.html
8. Espressif, Clock Tree (ESP32-S3). https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/api-reference/peripherals/clk_tree.html
9. Espressif ESP-Techpedia, “Introduction to ESP-CSI Solution” (esp-crab common
   crystal; S3+C3 clock buffer demo). https://docs.espressif.com/projects/esp-techpedia/en/latest/esp-friends/solution-introduction/esp-csi/esp-csi-solution.html
10. esp-crab example and issues: https://github.com/espressif/esp-csi/blob/master/examples/esp-crab/README.md ·
    https://github.com/espressif/esp-csi/issues/259
11. esp-csi (CSI acquisition, ESP-NOW excitation). https://github.com/espressif/esp-csi

**Phase referencing, two-way products, over-the-air synchronisation**

12. “Wi-Fi Radar via Over-the-Air Referencing: Bridging Wi-Fi Sensing and
    Bistatic Radar” (LoSRef), arXiv:2602.05344. https://arxiv.org/abs/2602.05344
13. F. Shi, W. Li, C. Tang, P. Brennan, K. Chetty, “Doppler Sensing Using WiFi
    Round-Trip Channel State Information,” IEEE WCNC 2023.
    https://discovery.ucl.ac.uk/id/eprint/10161602/
14. “ML-Track: Passive Human Tracking Using WiFi Multi-Link Round-Trip CSI,”
    IEEE TMC 2025. https://www.computer.org/csdl/journal/tm/2025/06/10842509/23wAPQchLjO
15. “Extracting Range–Doppler Information of Moving Targets from Wi-Fi CSI,”
    arXiv:2508.02799. https://arxiv.org/abs/2508.02799
16. “Path to Diversity: A Primer on ISAC-izing Commodity Wi-Fi for Practical
    Deployments,” arXiv:2601.12980. https://arxiv.org/abs/2601.12980
17. “CSI-Assisted Time Synchronization via Two-Way Time Transfer for Wireless
    Telemetry,” Proc. CIBDA 2026. https://dl.acm.org/doi/10.1145/3813822.3813976
18. “Repeater-Aided Over-the-Air Phase Synchronization in Distributed MIMO,”
    arXiv:2603.19903. https://arxiv.org/abs/2603.19903
19. “Distributed MIMO With Over-the-Air Phase Calibration Integrated Into the
    TDD Flow,” arXiv:2509.03722. https://arxiv.org/abs/2509.03722
20. “BeamSync: Over-the-Air Synchronization for Distributed Massive MIMO,”
    IEEE TWC 2024. doi:10.1109/TWC.2023.3335089
21. H. V. Balan et al., “AirSync,” IEEE/ACM ToN 21(6), 2013;
    arXiv:1205.6862. doi:10.1109/TNET.2012.2230449
22. Q. Yang et al., “Argos: practical many-antenna base stations,” MobiCom
    2012. doi:10.1145/2348543.2348553
23. R. Rogalin et al., “Scalable synchronization and reciprocity calibration
    for distributed multiuser MIMO,” IEEE TWC 13(4), 2014 (as cited in
    arXiv:1912.01503, arXiv:1708.07811).
24. J. Vieira et al., “Reciprocity calibration / self-calibration” family
    (arXiv:1912.01503; arXiv:1708.07811; arXiv:2206.14752).

**Multipath landmarks, SLAM, channel charting**

25. C. Gentner et al., “Multipath Assisted Positioning with Simultaneous
    Localization and Mapping (Channel-SLAM),” IEEE TWC 15(9), 2016.
    doi:10.1109/TWC.2016.2578336
26. “Multipath-assisted positioning enhancement via DC-SLAM with MAP-PF,”
    Digital Signal Processing, 2024.
    https://www.sciencedirect.com/science/article/abs/pii/S1051200423004670
27. “Angle-based SLAM on 5G mmWave Systems,” 2023.
    https://www.researchgate.net/publication/370949479
28. S. Taner, V. Palhares, C. Studer, “Channel Charting in Real-World
    Coordinates with Distributed MIMO,” arXiv:2406.13722 (IEEE TWC 2025).
    https://arxiv.org/abs/2406.13722
29. “Channel Charting for Position and Orientation,” arXiv:2606.18151.
    https://arxiv.org/abs/2606.18151
30. DiChasus channel-charting tutorials.
    https://dichasus.inue.uni-stuttgart.de/tutorials/tutorial/channelcharting/
31. M. A. Skoglund et al., “Improving Maps of Physical and Virtual Radio
    Transmitters,” ION GNSS 2018. https://www.ion.org/publications/abstract.cfm?articleID=16072

**Tomography, TDOA, common-view, ranging**

32. J. Wilson, N. Patwari, “Radio Tomographic Imaging with Wireless Networks,”
    arXiv:0909.5417 and IEEE TMC; “Through-Wall Tracking … Variance-Based RTI,”
    arXiv:0909.5417. https://arxiv.org/abs/0909.5417 ·
    https://span.ece.utah.edu/uploads/RTI_version_3.pdf
33. O. Kaltiokallio et al., “MA-RTI,” Sensors 2023.
    https://pmc.ncbi.nlm.nih.gov/articles/PMC9962819/
34. “A CFO-Assisted Algorithm for Wireless TDOA Localization Networks,”
    Sensors 2024. https://pmc.ncbi.nlm.nih.gov/articles/PMC10857262
35. “Receiver-Centric TDOA Localization Framework …,” arXiv:2605.26545.
    https://arxiv.org/abs/2605.26545
36. NIST, “Common-View GNSS Time Transfer.”
    https://www.nist.gov/pml/time-and-frequency-division/time-services/common-view-gnss-time-transfer
37. “Indoor Carrier Phase Positioning Technology Based on OFDM System,”
    Sensors 21(20):6731, 2021. doi:10.3390/s21206731
38. J. H. Cho et al., “OFDM-Based Carrier Phase Localization With Integer
    Ambiguity Resolution,” IEEE 2025 (URL).
    https://ieeexplore.ieee.org/document/11069288
39. “Secure Wi-Fi Ranging Today: Security and Adoption of IEEE 802.11az/bk,”
    arXiv:2603.18687. https://arxiv.org/abs/2603.18687

**Arrays, coupling, fingerprints**

40. “A Geodesic Sphere Phased Array Antenna for Satellite Control and
    Communication,” URSI GA 2002.
    https://www.ursi.org/proceedings/procGA02/papers/p1274.pdf
41. “Spherical Atomic Norm-Inspired Approach for DoA … with Undefined Mutual
    Coupling,” Appl. Sci. 13(5):3067, 2023. doi:10.3390/app13053067
42. “Subspace Pseudointensity Vectors … Spherical Antenna Array … Unknown
    Mutual Coupling,” Appl. Sci. 12(19):10099, 2022. doi:10.3390/app121910099
43. Pralon et al., “Suitability of compact antenna arrays for DoA estimation,”
    IEEE TAP 65, 2017 (as cited in 41).
44. “A novel data transformation approach for DoA estimation with 3-D antenna
    arrays in the presence of mutual coupling,” IEEE AWPL 2017.
    doi:10.1109/LAWP.2017.2699292
45. “Stable Nonlinear and IQ Imbalance RF Fingerprint for Wireless OFDM
    Devices,” arXiv:2104.10397. https://arxiv.org/abs/2104.10397
46. “Physical-layer Identification of RFID Devices,” USENIX Security 2009.
    https://www.usenix.org/event/sec09/tech/full_papers/sec09_radio.pdf
47. “On Phase Offsets of 802.11ac Commodity WiFi,” arXiv:2005.03755.
    https://arxiv.org/abs/2005.03755

**ESP32 sensing practice**

48. Espressif ESP-Techpedia CSI performance ranking (C5 > C6 > C3 ≈ S3 > ESP32)
    — see ref. 9.
49. “Device-Free Indoor Localization with ESP32 Wi-Fi CSI Fingerprints,”
    Preprints 2026. https://www.preprints.org/manuscript/202601.2378
50. “Design and Experimental Evaluation of CSI and RSSI-Based Indoor Wi-Fi
    Ranging on ESP32-S3,” 2026 (15 m range bin at 20 MHz).
    https://science.lpnu.ua/ictee/all-volumes-and-issues/volume-6-number-1-2026/design-and-experimental-evaluation-csi-and-rssi

---

*Note ends. Corrections welcome; the claims most in need of adversarial
measurement are A2 (solid-body coupling), A4 (per-frame referencing on S3) and
A12 (per-puck sampling-offset stability).*
