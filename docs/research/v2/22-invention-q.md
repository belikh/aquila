# 22-q — The invention: ApertureLock — scene-anchored coherence for a home-scale 3D aperture

**Prompt.** Inventor-researcher. Discover the invention that makes a home full of
cheap ESP32-S3 nodes behave as one sharp, self-calibrating, generalising 3D
sensor — or prove no such invention exists and state the exact physical boundary
that blocks it.

**Hardware premise (assumed, not re-litigated).** A polyhedron of triangular
PCBs, ~30 cm diameter, one ESP32-S3 per face, soldered or flex edges; a hub face
carrying a shared 40 MHz clock fan-out, a conducted 2.4 GHz phase reference, and
data aggregation (Ethernet switch or FPGA). A compact coherent 3D aperture with
no house-wide cabling is therefore available.

Australian English. All claims labelled **ESTABLISHED / HYPOTHESIS /
SPECULATION**, with hardware tier: **ESP32-S3 / family**, **other COTS Wi-Fi**,
or **SDR-grade**. Sources are primaries (papers, standards, upstream code,
datasheets). Cross-references are to the v2 folder (`NN`).

---

## Question

Given (a) *puckets* — polyhedra whose ~20 faces are individually phase-coherent
to a shared on-puck reference, and (b) no cable between puckets, what single
mechanism can make the whole home behave as **one** coherent aperture — sharp
(decimetre-scale 3D), self-calibrating (no manual survey), and generalising
across homes — and if no such mechanism exists, which physical quantity closes
the space and what is the minimal reference that crosses it?

The v2 notes already establish the boundary (`20`) and the escapes (`10`, `11`).
This note asks the next question: **can the escapes be engineered into a sensor
with commodity silicon, and what is the *new object* that does it?**

---

## The proposal

### The object: the pucket as a one-scalar coherent 3D array

Within a pucket, the ESPARGOS principle applies at 3D scale. ESPARGOS showed on
ESP32 silicon that a shared 40 MHz clock alone does **not** give phase coherence
("reference clock distribution is insufficient for achieving phase coherence";
the PLL initial phase re-randomises at every reset and channel change), but a
shared clock **plus a conducted 2.4 GHz phase-reference signal** does, and that
multiple boards fed the same reference become mutually coherent (`arXiv:2502.09405`;
`arXiv:2408.16377`; espargos.net/docs/combined-arrays). A pucket therefore needs:
one reference generator (the hub), a distribution network, and per-face RF
switches that alternate each receiver between its antenna and the reference — the
ESPARGOS topology, bent into a sphere.

Two consequences matter:

1. **Per-face chain phases are measured, not assumed.** The reference signal
   arrives at each face through a known distribution network; the face's complex
   chain offset follows from the measured reference channel (ESPARGOS §III-B:
   `ĥ_CAL = ĥ_OTA / ĥ_REF · e^{−jφ_path}`).
2. **The pucket collapses to one unknown LO phase.** After that calibration, the
   only dynamic unknown shared by all 20 faces is the **hub reference's own
   phase** Θ_i(t). The pucket is thus a 20-port coherent receiver with **one
   scalar phase per node** — exactly the R-calibration model of distributed-MIMO
   theory, where each access point carries one phase parameter φ_n
   (Larsson & Vieira, `arXiv:2304.05144`; Rogalin et al., `arXiv:1310.7001`).
   A 20-antenna distributed array has been reduced to an N=1 calibration problem.

### The invention: ApertureLock

*Five mechanisms, one system.*

**(1) Gauge cancellation by common observation.** Any 802.11 frame that two or
more puckets receive is a phase probe. Puck i's raw per-face measurement on
packet p is, in the note-`20` model,

```
Φ_i,f(p) = [Θ_i(p) + β_i,f] − Ψ_tx(p) + Γ_i,f + ε_i,f(p)
```

where Θ_i is the hub LO phase, β the calibrated face chain, Ψ_tx the
transmitter's LO phase, Γ_i,f the static channel phase, ε the residual. The
transmitter's phase is **common** and cancels in any cross-pucket difference:

```
Φ_i,f(p) − Φ_j,g(p) = [Θ_i(p) − Θ_j(p)] + (Γ_i,f − Γ_j,g) + (ε_i,f − ε_j,g)
```

This is the standard uplink reciprocity-calibration measurement (the ambient
transmitter plays the role of the common UE, exactly as in
`arXiv:2509.03722`; Larsson, `arXiv:2401.11730`). No dedicated synchronisation
waveform, no tones, no cable: **ordinary frames suffice**.

**(2) Face averaging — the polyhedral phase meter.** ε_i,f(p) contains the
per-packet residual phase of an individual ESP32 radio. Because each face is a
*separate* oscillator and RF chain, its residual is largely independent across
faces (HYPOTHESIS; the decisive experiment tests this). Averaging over a
pucket's 20 faces, and over both puckets' faces pairwise, reduces the
cross-pucket phase estimator's variance by up to ~20–400× relative to a
single-antenna pair. This is the new reason a *polyhedral* node, specifically,
can plausibly carry a wireless phase loop on commodity silicon: it converts many
noisy, low-cost, single-antenna phase observations into one high-precision
per-pucket phase observation — and it is redundant to face shadowing, which a
single-antenna node cannot be.

**(3) Two-stage lock: steer frequency, then track phase.** With independent
references the relative phase wraps every T ≈ 1/(2Δf) ≈ 5–10 µs at 10–40 ppm
(24–96 kHz at 2.4 GHz) — four orders of magnitude below a ~millisecond MAC
event (`12`, `17`). Direct phase tracking is therefore impossible *unless
frequency is first made coherent*. ApertureLock makes the hub reference
**steerable**: the hub synthesises the 40 MHz + 2.4 GHz reference from a
controllable source (VCXO/DDS behind the hub FPGA or DAC), so a slow loop
(≲1 Hz bandwidth) disciplines each pucket's carrier to the network master to
≲1 Hz — after which the per-packet phase sequence is unambiguous and a Kalman
filter per node pair (the Ngo–Larsson construction, `arXiv:2509.03722`,
`arXiv:2504.11411`) predicts and corrects Θ_i − Θ_j between gauge updates.
This is AirSync's lesson ("predicts the instantaneous carrier phase correction
along the transmit slot", Balan et al., ToN 2013) applied at pucket scale.

**(4) The scene is the anchor; the two-way product is the self-test.**
The static channels Γ_i,f are learned in situ by long-term averaging in a quiet
house; their *constancy* is both the calibration observable and the divergence
test — exactly the LoSRef principle ("the temporal stability of the Tx–Rx LoS
path to provide a common delay and phase reference",
`arXiv:2602.05344`), generalised from one bilateral link to a network of
coherent arrays. Two-way exchanges (`Ĥ_AB·Ĥ_BA`) provide an independent,
LO-free consistency check (hardware asymmetry must stay constant; note `20` §4),
device fingerprints, and graph edges when no common source is audible
(star/broadcast topology is CRB-optimal; Zhu et al., `arXiv:1709.07206`).

**(5) Sensing: one aperture, not N sensors.** Once locked, the ensemble is a
single multistatic coherent aperture:

- every packet yields, per pucket, a **3D full-sphere angular spectrum** (the
  20-face sphere has no scan loss, unlike planar arrays), polarisation
  information (each face has a different orientation), and near-field wavefront
  curvature giving **range from a single snapshot** inside the Fresnel region
  (Fraunhofer distance 2D²/λ ≈ 1.4 m for D = 0.3 m; CRB theory in Wang et al.
  2024, Friedlander TSP 2019);
- cross-pucket phase coherence yields a **home-scale aperture** (λ/D ≈ 1° at a
  7 m span) and coherent multistatic backprojection over 3D voxels;
- the static scene is coherently removed, so moving targets (people) are imaged
  with gain and sharpness no single 20 MHz link can give;
- static multipath components are **exploited**, not discarded: each specular
  wall creates a virtual anchor that adds an effective "extra anchor" (MINT /
  SALMA; Meissner & Witrisal; `<0.20 m` at 95 % in-room with a 2 GHz UWB single
  anchor, PMC5713621) — with coherent 3D angle *and* delay per component, a
  2.4 GHz system can extract several virtual anchors per link even at 20 MHz
  bandwidth.
- **Passive mode** illuminates from the home's own APs/traffic (no pucket TX for
  sensing); **active mode** adds scheduled pings, and, because TX chains are
  calibrated against the same reference, phase-coherent transmit across the 20
  faces (a 20-element beam — a strict enhancement, not a requirement).

### What is genuinely new

| Element | Status |
| --- | --- |
| U(1) gauge, escapes, minimal reference, CRBs | **ESTABLISHED theory** (`20`; Larsson; Rogalin; Zhu) |
| Conducted-reference coherent ESP32 array (2D, 8–16 ports) | **ESTABLISHED hardware** (ESPARGOS; esp-crab; Espressif esp-csi clock buffer) |
| OTA phase sync on SDRs (AirSync 2.37° std; BeamSync) | **ESTABLISHED** (SDR-grade) |
| OTA calibration on COTS RRUs (non-Wi-Fi) | **ESTABLISHED** (Cao et al., JSAC 2023) |
| LoS-path phase referencing on COTS Wi-Fi, bilateral, single-antenna | **ESTABLISHED** (LoSRef 2026) |
| Scene-referenced coherent-on-receive distributed radar (military) | **ESTABLISHED** (dominant-scatterer CoRS, MDPI RS 2023) |
| **Polyhedral pucket as a one-scalar, face-averaged phase meter for a wireless network loop** | **NEW (this proposal)** |
| **Scene-anchored phase-lock for a *sensing* ensemble on commodity 802.11 CSI (no dedicated waveform, no cable)** | **NEW (this proposal)** |
| **Home-scale coherent aperture for 3D sensing from many 20 MHz pucket nodes, with self-calibration and graceful incoherent fallback** | **NEW (this proposal)** |

No primary source found that demonstrates cross-node carrier-phase coherence
between independently-clocked Wi-Fi chip-based receivers using only ordinary
frames; ESPARGOS's multi-board coherence is wired. The novelty is a synthesis
whose every component is proven elsewhere but whose combination — and whose two
critical ESP32-specific premises (T1, T2 below) — are untested.

---

## Findings

### ESTABLISHED

**E1. Conducted reference on ESP32-family gives a coherent array; the clock
alone does not.** ESPARGOS distributes a frequency-multiplexed 40 MHz clock +
Wi-Fi phase-reference signal over one coax, switches it into each receiver's RF
path, and calibrates PLL and distribution-network offsets; the result is
long-term phase stability and AoA on 8-element boards, extensible to
master/slave multi-board coherent arrays. `arXiv:2502.09405`, `arXiv:2408.16377`,
espargos.net/docs/combined-arrays. *Tier: ESP32-family (S2 in papers; C61 in
ESPARGOS One).* Espressif's own esp-csi solution uses a clock buffer between two
chips "to eliminate the relative frequency offset… utilise the phase
information in the CSI" (mm-level disturbance sensing), and esp-crab shares a
crystal between two ESP32-C5s. *Tier: ESP32-family.*

**E2. The network phase problem reduces to one scalar per node, and its
identifiability is theory-complete.** R-calibration measures
(t_n + r_n) − (t_n' + r_n') from bidirectional measurements; with per-node
hardware symmetry one phase φ_n per access point suffices; the observable is
φ_n − φ_n' mod 2π; a connected graph fixes the solution up to one global
constant (Larsson & Vieira, IEEE Commun. Lett. 27(6):1619–1623, 2023;
`arXiv:2304.05144`; Rogalin et al., `arXiv:1310.7001`). Solving one global
calibration problem beats any local decomposition regardless of topology
(Larsson, IEEE TSP 72:855–866, 2024; `arXiv:2401.11730`); the star/broadcast
topology attains the minimum CRLB (Zhu et al., `arXiv:1709.07206`). *Tier:
theory.*

**E3. Over-the-air calibration and phase tracking are demonstrated on SDRs and
COTS cellular hardware.** AirSync achieves carrier-phase synchronisation within
a few degrees across distributed WARP radios (2.37° standard deviation; 95th
percentile ≤ 4.5°) by predicting phase correction along the slot (Balan et al.,
IEEE/ACM ToN 21(6):1681–1695, 2013; `arXiv:1205.6862`). An experimental cell-free
massive-MIMO prototype built from COTS RRUs demonstrates OTA reciprocity
calibration and phase synchronisation, characterising calibration error and
delay (Cao et al., IEEE JSAC 41(6):1620–1634, 2023,
DOI `10.1109/JSAC.2023.3276057`). Kalman-filter phase tracking integrated into a
TDD flow, with periodic re-estimation against LO drift, is analysed and
simulated (Ngo & Larsson, GLOBECOM 2025; `arXiv:2509.03722`, `arXiv:2504.11411`).
*Tier: SDR and COTS cellular — not ESP32.*

**E4. Scene-referenced phase alignment is demonstrated and standard in radar and
Wi-Fi sensing.** LoSRef uses the Tx–Rx line-of-sight path as a delay and phase
reference on commodity Wi-Fi with independent clocks, yielding phase-coherent
CIRs and phase-based Doppler/respiration across packets (`arXiv:2602.05344`).
Distributed coherent-aperture radar uses a dominant scatterer for
coherent-on-receive synthesis (MDPI Remote Sensing 15(6):1505, 2023), and
bistatic SAR synchronises time/phase via the direct path (Wang et al., IET RSN
2(1):1–11, 2008, DOI `10.1049/iet-rsn:20060097`). *Tier: COTS Wi-Fi / radar.*

**E5. Near-field wavefront curvature gives angle and range from one compact
array, with closed-form CRBs.** For a source in the Fresnel region of an array,
range is identifiable from curvature alone; the far-field "range CRB = ∞"
becomes finite, and CRB expressions exist for monostatic phased arrays
(Wang et al., IEEE 2024, document 10388218; `arXiv:2303.05736`; Friedlander,
IEEE TSP 67(15):3885–3893, 2019). *Tier: SDR/general theory.*

**E6. Multipath components act as virtual anchors, giving single-anchor indoor
positioning.** Mirrored images of the transmitter in walls become extra anchors;
with UWB, 95th-percentile errors below 0.20 m are reported in-room, and
centimetre accuracy in the multipath-assisted literature (Meissner & Witrisal,
MINT; SALMA; PMC5713621). *Tier: UWB (large bandwidth); mechanism general.*

**E7. The raw ESP32 CSI phase on commodity silicon is differential-grade but
characterised.** OpenCSI: "differential-phase formulation cancels per-boot phase
offsets and timing-offset trends that make absolute ESP32 phase unusable"
(`arXiv:2607.26665`). A 2026 cross-platform study finds ESP32 CSI phase noise
near-normal (skewness/kurtosis ≈ 0) versus outlier-heavy Broadcom
(`arXiv:2605.26836`). Wi-ESP reports ESP32 HT-LTF phase "usable" after linear
sanitisation (J. Comput. Des. Eng. 7(5):644, 2020). ESPARGOS's stability test
uses the argument of the subcarrier-summed channel — an intercept-like quantity
— and finds it stable long-term **with a shared clock**. *Tier: ESP32.*

**E8. Phase-coherent raw I/Q is now available on ESP32 without hardware mods.**
ESP-SDR exposes an undocumented raw-I/Q capture path (up to 80 MSa/s; low duty
cycle); on an ESPARGOS One board it yields eight **phase-coherent** IQ streams;
on ESP32-S3 a firmware variant provides continuous decimated IQ streaming over
USB, and an on-chip FFT mode exists. `https://github.com/ESPARGOS/esp-sdr`;
espargos.net/espsdr. Phase-coherent transmit is stated possible on ESPARGOS One
but not released. *Tier: ESP32 / S3.*

**E9. Cheap oscillators + sufficiently frequent re-calibration are sufficient
for coherent operation at 1 ms-scale intervals.** Phase-noise analyses of
cell-free mMIMO conclude low-cost LOs suffice within a standard 5G TTI
(`arXiv:2405.04099`); the governing design rule is that the re-calibration
cadence must beat the phase-noise/drift accumulation (`arXiv:2509.03722`;
`12`). *Tier: theory/COTS cellular.*

### HYPOTHESIS

**H1 (the critical one). Per-packet cross-pucket phase is an unbiased,
averageable estimator on ESP32-S3.** The face-averaging gain (mechanism 2) is
real only if ε_i,f(p) is zero-mean across faces and packets, with the LO term
common to a pucket's faces within a packet. The opposite is equally plausible:
the PHY's per-packet CFO correction leaves a residual phase that may be
deterministic-but-unknown per packet (so averaging over packets, not faces, is
needed) or large enough that loop SNR is insufficient at a useful cadence.
This is the unmeasured M2 residual of the synthesis (`00` §2, §9); no primary
was found that measures it on S3. **The decisive experiment (below) tests
exactly this.**

**H2. Frequency steering gives ≲1 Hz relative stability, and the Kalman loop
holds ≲0.2 rad between gauge updates.** A steerable hub reference
(VCXO/DDS) is ordinary engineering at pucket cost, but no demonstration exists
on an ESP32 reference distribution. The alternative — free-running references
with a loop at the packet cadence — gives short coherence windows; whether they
are long enough for coherent aperture processing (≥~10 ms) is open.

**H3. The pucket's 20-face geometry gives usable 3D performance per snapshot.**
At λ/D ≈ 24° nominal beamwidth, super-resolution at the achievable SNR pushes
single-pucket bearing precision to a few degrees in a multipath-rich room;
near-field depth is decimetre-class inside ~1.5–2 m and degrades as R² beyond
(σ_R ≈ 1.27·λ·R²/D² · σ_φ, SPECULATION on the constant). The thermal CRB at
20 dB per-element SNR is far tighter (≈0.1°), so calibration and multipath — not
noise — are the real limits.

**H4. Home-scale coherent 3D sensing is decimetre-class.** With 4–8 puckets
spanning ~7 m: λ/D ≈ 1° angular; coherent multistatic backprojection cross-range
≈ λR/D ≈ 7 cm at 4 m; range from multistatic geometry with 20 MHz delay
super-resolution ≈ 0.1–0.3 m single-link, better fused; Doppler per-face for
motion. "Sharp" = 0.1–0.3 m 3D at room scale, which is sharp *for human
sensing*. If upgraded to ESP32-C5 faces (2.4 + 5 GHz), the multi-band span
(≈2.6 GHz) gives ≈5.7 cm Rayleigh range resolution (SPECULATION, configuration
limited by the premise's S3).

**H5. Intra-pucket calibration is self-contained and stable.** Face chains
calibrate from the conducted reference (E1); TX chains from mutual coupling
between neighbouring faces (mutual-coupling calibration is established in
massive MIMO: Wei et al., IEEE TWC 15(1):606–619, 2016, DOI
`10.1109/TWC.2015.2476467`); face-to-face coupling and shadowing on a solid
polyhedron are unmeasured (`08`/`09`) and could degrade the aperture.

### SPECULATION

**S1. The whole premise can run ambiently.** If ambient AP traffic is the
illuminator and the scene is the anchor, the system can learn and sense with
**no pucket transmissions at all** beyond an initial self-survey. This is the
cleanest legal and coexistence story; it depends on traffic density (beacons
~10 Hz are marginal for the loop; data traffic in occupied homes is plentiful).

**S2. Cross-home generalisation comes from the geometry-first design.** The
estimators (angle, delay, Doppler, gauge tracking) are physical and portable;
the home-specific content (static channels, floorplan, occupancy map) is
self-supervised and re-learned per home. Learned refinement can borrow the
channel-charting and Sim2Real results of `03`/`19`; an ESPARGOS-based passive
channel-charting study exists on the dataset line, showing the field is moving
toward distributed learned geometry.

**S3. A "sync tree" of one coax is the cheap escape if the wireless loop
fails.** One 2.4 GHz phase reference shared by all puckets (ESPARGOS
master/slave) removes the gauge entirely. This contradicts the "no house-wide
cabling" premise but is the known-good fallback and should be built as a
reference arm in the experiment, not as a failure.

---

## The boundary — the quantity that closes the space, and the minimal reference

If H1 fails (ε not unbiased/averageable) or H2 fails (frequency cannot be held),
then **wireless-only cross-pucket carrier coherence is closed on commodity
Wi-Fi silicon**, and the closing quantity is specific:

> **the network phase-coherence time deliverable by commodity CSI**, set by the
> ratio between the raw phase-wrap time (T ≈ 1/(2Δf) ≈ 5–10 µs at 10–40 ppm)
> and the achievable gauge cadence (≈ ms MAC events), discounted by the
> per-packet cross-node phase estimator variance after all available averaging
> (faces × packets). Coherence exists only where the frequency loop has
> collapsed the wrap rate below the cadence — i.e., only if the estimator is
> good enough to estimate relative frequency to ≲1 Hz before phase can be
> tracked at all.

This is a field-strength statement, not a theorem: the physics of H1 is
*measurable*, and the same quantity was never the blocker on SDRs (AirSync:
2.37°). The minimal references that cross it, ranked:

1. **One conducted 2.4 GHz phase reference per connected component** — the
   ESPARGOS escape; proven on ESP32-family; removes the gauge rather than
   estimating it. Cost: one sync cable tree. *Crosses the boundary absolutely.*
2. **A common emitter of sufficient rate/SNR** (an AP, a phone, or a
   round-robin of puckets) — free if H1 holds; the entire invention.
3. **A two-tone wireless clock (AirShare)** — sub-Hz frequency transfer is
   demonstrated (Abari et al., INFOCOM 2015), but it needs raw waveform
   transmission that stock `esp_wifi_80211_tx` cannot produce; only relevant if
   ESPARGOS-style raw TX is enabled on the pucket.

If all fail, the home sensor does not vanish — it degrades to the **incoherent
mode**: per-pucket coherent 3D bearings (few degrees), incoherent multistatic
fusion and tracking across known geometry, giving ~0.1–0.3 m 3D for people and
decimetre-to-metre for static structure. The v2 commodity route (`00` §8),
sharpened by the pucket.

---

## The decisive experiment (single, cheapest-decisive)

**"Two independent puckets, one common transmitter, one coax switch."**

- **Hardware.** Two ESPARGOS One boards (or two minimal 4-face pucket
  prototypes) on **independent** hub references; one common transmitter
  (ESP32-S3 or the home AP) transmitting ≥100 frames/s; one coax run that can
  optionally feed the *same* reference to both hub inputs (the ground-truth
  arm).
- **Arm A (ground truth).** Coax connected: measure the cross-board phase
  difference and its drift — the truth.
- **Arm B (wireless gauge).** Coax removed: estimate the same phase difference
  per packet from cross-board differential CSI with and without per-board face
  averaging; run a Kalman/PLL loop using only wireless observations; compare
  continuously against Arm A for 10 minutes, with a temperature step included.
- **Measurements.** (i) per-packet σ_φ versus number of faces and packets
  (the face-averaging law); (ii) zero-mean-ness of ε (byte-block vs
  time-block variance); (iii) loop residual RMS and the **holdover coherence
  time** T_c (time the loop holds |error| < 0.2 rad without a gauge update);
  (iv) cycle-slip rate; (v) relative-frequency estimation floor; (vi) the same
  metrics using raw IQ (ESP-SDR) instead of CSI.
- **Success thresholds.** Face-averaged per-packet σ_φ ≲ 0.3 rad (≈17°) and a
  loop residual ≲ 0.2 rad with T_c ≥ 20 ms using ambient-rate frames. That is
  enough for coherent aperture gain and multistatic imaging (H4).
- **Falsification.** The wireless estimate is uncorrelated with Arm A, or the
  residual grows without bound, or averaging fails to converge (H1 false) at
  any cadence the MAC permits. Then the boundary above stands, and the program
  falls back to the sync tree (escape 1) or to the incoherent mode.
- **Sub-experiment (do it in parallel).** Six puckets (or six ESPARGOS boards),
  one walking person, ground truth from a few marked positions: measure the
  incoherent-mode 3D accuracy. This bounds the fallback and is needed either
  way.

This experiment is the synthesis's Experiment #1 (`00` §6) specialised to the
pucket: the new content is the **face-averaging law** and the **holdover
coherence time**, which are the two numbers that decide whether the home
aperture is one sensor or N sensors.

---

## What must be true (the whole proposal rests on these)

| # | Statement | Status | Test |
| --- | --- | --- | --- |
| T1 | Per-face chains calibrate from the conducted reference on S3 | HYPOTHESIS (proven on S2/C61 family) | ESPARGOS method ported to S3 |
| T2 | Per-packet cross-pucket phase is unbiased and face-averageable | **HYPOTHESIS (critical)** | decisive experiment (i)–(ii) |
| T3 | Relative frequency to ≲1 Hz by steering or estimation | HYPOTHESIS | decisive experiment (v) |
| T4 | Scene constants stable; motion separable; loop does not poison itself | HYPOTHESIS (LoSRef precedent) | 10-minute run with motion |
| T5 | Mutual coupling/shadowing does not wreck per-face calibration | UNKNOWN (`08`/`09`) | prototype measurement |
| T6 | Holdover T_c ≥ ~20 ms | HYPOTHESIS | decisive experiment (iii) |
| T7 | Incoherent fallback gives 0.1–0.3 m 3D | HYPOTHESIS | sub-experiment |

---

## The six search directions, scored against the proposal

1. **Wireless coherence on commodity silicon (no cable).** *Answered:* the
   scene-anchored loop (mechanisms 1–4). The protocol that tracks relative
   carrier phase fast enough is *ordinary frames + face averaging + a steerable
   hub reference + Kalman phase tracking*. H1/T2 is the whole game.
2. **Self-calibrating learned array; joint discovery of geometry and targets.**
   *Answered:* the factor graph estimates, jointly and mostly label-free:
   pucket geometry (FTM + MDS; `15`), per-face chains (E1/H5), per-pucket phases
   (mechanisms 1–4), static scene (long-term averaging), targets (tracking).
   Precedents exist for joint estimation with unknown sources — array-shape
   calibration from sources in unknown locations (Rockah & Schultheiss 1987),
   joint attitude + gain/phase + mutual-coupling calibration from GNSS
   (Zorn et al., DLR), and joint device-localisation + target-tracking on Wi-Fi
   (WiSLAT, `arXiv:2603.17283`, 0.68 m median) — the pucket simply supplies far
   richer measurements to the same estimator family.
3. **The environment as a shared reference, turned into aperture.**
   *Answered:* it **is** the reference (mechanism 4) and it **is** extra
   aperture: virtual anchors from specular walls (E6) multiply effective
   observation points. Established mechanism; novel at 2.4 GHz/20 MHz scale
   with coherent 3D arrays.
4. **Sharp 3D from many coarse apertures.** *Answered by four levers:* (a) the
   home-scale coherent aperture (λ/D ≈ 1°), exploiting the 30 cm 3D face
   geometry for full-sphere coverage; (b) per-face polarisation diversity
   (ESPARGOS One is dual-polarised; Jones-calculus polarimetry is demonstrated
   in its AR demo, `arXiv:2608.25996`) to separate direct/multipath;
   (c) multi-channel stitching within 2.4 GHz (S3) and multi-band on C5;
   (d) the round-trip product's hardware-asymmetry signature as a stability
   check and device fingerprint. Round-trip *radar* gains 2× motion
   sensitivity; it is a bonus, not the backbone.
5. **A protocol: scheduled ping + unique replies + two-way products.**
   *Answered:* the loop's **gauge edges** (pairwise α measurements; star/broadcast
   optimal), plus by-products: hardware fingerprints (stable asymmetry), a
   divergence alarm (LO-free quantity must stay constant), obstruction maps
   (two-way amplitude/phase visibility), and relative geometry (FTM/MDS
   refinement). Where it breaks: turnaround drift, 2π ambiguity, motion
   non-reciprocity (`20`, round-trip section).
6. **Different excitation/composition.** *Answered:* TDM across puckets
   (scheduled pings; half-duplex is fine because the aperture is receive-side);
   coherent transmit *within* a pucket (20-element beam using the shared
   reference — enhancement only); passive illumination by ambient APs
   (S1); synthetic aperture by face diversity rather than mechanical switching
   (the faces are simultaneous, thanks to one radio per face — no TDM inside
   the pucket).

---

## ESP32-S3 feasibility

- **Chips.** ESP32-S3 is 2.4 GHz-only, HT20, with the weakest CSI class in
  Espressif's own ranking (C5 > C6 > C3 ≈ S3 > ESP32). It is nonetheless
  ESP-SDR-supported for raw IQ (continuous decimated streaming variant), and
  the ns RX timestamp exists (`17`). CSI phase is differential-grade (E7).
- **Realistic ranking for the invention:** ESP32-C6/C5 are better CSI
  citizens; C5 adds 5 GHz (multi-band range resolution, H4) and is the
  esp-crab target. **Recommendation: prototype the loop on C5/C6 if S3's
  per-packet phase residual (T2) proves the binding constraint; keep S3 as the
  cost/ubiquity floor.**
- **The pucket build.** 20× S3 (≈ $2–3 each at volume), 20 antennas, 20 RF
  switches, a triangle PCB set with flex/soldered edges, a hub (FPGA or MCU +
  Ethernet switch) generating and distributing the 40 MHz + 2.4 GHz reference,
  thermally ~5–10 W. Everything but the 3D reference distribution is proven
  ESPARGOS engineering; the 3D distribution and mutual coupling are the new
  hardware risks (T5).
- **Compute.** Per-face estimation (sanitise, cross-correlate, phase) fits an
  S3; the network loop and imaging live on the host/fusion node. ESP-DL/ESP-NN
  exist if on-pucket learning is ever needed (`19`).

## TX needed?

**For the gauge: no, in principle** — ambient frames suffice (S1). **For
self-survey and for active illumination: yes** — scheduled compliant frames
(`esp_wifi_80211_tx`/ESP-NOW, legal under ACMA LIPD/FCC 15.247 per `07`), FTM
for the first-pass geometry (`15`), and two-way products for the fingerprint
and divergence test. **For coherent transmit: not required** (it is an
enhancement; transmit-chain calibration rides on the same reference).

## Cost

Prototype pucket ≈ **A$150–300** (20 S3 modules + PCB + hub + reference
distribution); home with 6 puckets + fusion host ≈ **A$1.0–2.5k**. Volume
pucket target ≈ **A$40–80**. Compare: one ESPARGOS One array is sold as a
research instrument; the pucket is that principle manufactured as a solid.

---

## Evidence strength

**Headline: 5 / 10 for the wireless realisation; 9 / 10 for the underlying
identities; 8 / 10 for the hardware premise; 7 / 10 for the fallback.**

Justification: the gauge-cancellation identity, the one-scalar-per-node
reduction, the topology/CRB theory, and the scene-referencing precedent are all
solid primaries (E2–E4). The conducted-reference ESP32 array is demonstrated
hardware (E1). What keeps the headline at 5 is a single unmeasured quantity —
the ESP32 per-packet cross-node phase residual (H1/T2), which no primary
reports and which the decisive experiment is built to settle; plus two
hardware uncertainties (steerable reference distribution; coupling/shadowing,
T5). This mirrors note `20`'s conclusion: the theory is complete; the
commodity realisation is one experiment away from being known.

## Primary sources

- **ESPARGOS**: Euchner et al., "ESPARGOS: An Ultra Low-Cost, Realtime-Capable
  Multi-Antenna WiFi Channel Sounder", `arXiv:2502.09405` (PLL phase
  uncertainty; phase-reference distribution; calibration; stability).
  Datasets: `arXiv:2408.16377`. AR/polarisation: `arXiv:2608.25996`.
  Multi-array coherence: https://espargos.net/docs/combined-arrays.html
- **ESP-SDR / phase-coherent IQ**: https://github.com/ESPARGOS/esp-sdr ;
  https://espargos.net/espsdr
- **esp-crab / Espressif**: https://docs.espressif.com/projects/esp-techpedia/en/latest/esp-friends/solution-introduction/esp-csi/esp-csi-solution.html ;
  https://github.com/espressif/esp-csi/issues/259
- **Larsson & Vieira**, "Phase Calibration of Distributed Antenna Arrays",
  IEEE Commun. Lett. 27(6):1619–1623, 2023; `arXiv:2304.05144`
- **Larsson**, "Massive Synchrony in Distributed Antenna Systems",
  IEEE TSP 72:855–866, 2024; `arXiv:2401.11730`
- **Ngo & Larsson**, "Breaking the TDD Flow for Over-the-Air Phase
  Synchronization…", GLOBECOM 2025; `arXiv:2504.11411`. "Distributed MIMO With
  Over-the-Air Phase Calibration Integrated Into the TDD Flow",
  `arXiv:2509.03722`
- **Rogalin et al.**, "Scalable Synchronization and Reciprocity Calibration for
  Distributed Multiuser MIMO", `arXiv:1310.7001`
- **Zhu et al.**, "A Framework for Over-the-Air Reciprocity Calibration…",
  `arXiv:1709.07206` (star topology optimal)
- **Balan et al.**, "AirSync…", IEEE/ACM ToN 21(6):1681–1695, 2013;
  `arXiv:1205.6862` (2.37° std)
- **Kunnath Ganesan et al.**, "BeamSync", IEEE TWC 23(7):6824–6837, 2024,
  DOI `10.1109/TWC.2023.3335089`
- **Cao et al.**, JSAC 41(6):1620–1634, 2023, DOI `10.1109/JSAC.2023.3276057`
  (COTS RRU OTA calibration + phase sync; `arXiv:2208.14048`)
- **Abari et al.**, "AirShare: Distributed coherent transmission made
  seamless", IEEE INFOCOM 2015 (two-tone wireless clock; sub-Hz frequency
  accuracy); robust clocking demo: `arXiv:2509.25277`
- **Yamamoto**, "Wi-Fi Radar via Over-the-Air Referencing" (LoSRef),
  `arXiv:2602.05344`
- **Wang et al.**, "Time and phase synchronisation via direct-path signal for
  bistatic SAR", IET RSN 2(1):1–11, 2008, DOI `10.1049/iet-rsn:20060097`
- **Coherent-on-receive synthesis using a dominant scatterer**, MDPI Remote
  Sensing 15(6):1505, 2023
- **Wang et al.**, "CRBs for Near-Field Sensing with XL-MIMO", IEEE 2024,
  document 10388218; `arXiv:2303.05736`. **Friedlander**, IEEE TSP
  67(15):3885–3893, 2019
- **Meissner & Witrisal** (MINT), multipath-assisted single-anchor
  localisation; SALMA (ACM SenSys 2018); PMC5713621 (95th pct < 0.20 m at
  2 GHz bandwidth)
- **Wei et al.**, "Mutual Coupling Calibration for Multiuser Massive MIMO",
  IEEE TWC 15(1):606–619, 2016, DOI `10.1109/TWC.2015.2476467`
- **Rockah & Schultheiss**, "Array shape calibration using sources in unknown
  locations", IEEE TASSP 35(3):286–299, 1987
- **Zorn et al.**, "Self-contained Antenna Crosstalk and Phase Offset
  Calibration…", DLR/ION (joint attitude + array calibration)
- **WiSLAT**, `arXiv:2603.17283` (joint device localisation + target tracking,
  0.68 m median)
- **OpenCSI**, `arXiv:2607.26665`; **"Same Signal, Different Story…"**,
  `arXiv:2605.26836`; **Wi-ESP**, J. Comput. Des. Eng. 7(5):644, 2020
- **IEEE 802.11bf-2025**, IEEE SA, Amendment 4: Enhancements for WLAN Sensing
  (procedure-level support for bistatic/multistatic sensing below 7 GHz);
  overview: `arXiv:2310.17661`; Ropitault et al., IEEE Commun. Stand. Mag.
  8(1):58–64, 2024

## Residual unknowns

1. **The M2 residual on ESP32-S3** — per-packet cross-node phase
   bias/variance; the decisive experiment. Not published.
2. **Face-averaging covariance** — are per-face ε independent, or is a
   PLL/AGC state common to the hub? (If common, the √N gain collapses; packets,
   not faces, must average.)
3. **Steerability of the ESP32 reference path** — does the PLL track a slowly
   steered 40 MHz/VCXO without re-locking/re-phasing? Untested.
4. **Mutual coupling and shadowing** in a solid 30 cm polyhedron (`08`/`09`).
5. **Holdover coherence time** T_c after the loop behaves — the number that
   sets every processing interval.
6. **Ambient traffic statistics** in real homes for S1 (beacon-only rooms).
7. **Incoherent-mode accuracy** from pucket-scale bearings — the fallback floor.

## Chaining / Existing models and data / Own data

**(A) Chaining.** The highest-leverage enabling capability is **cross-pucket
phase coherence** (this note's mechanisms 1–4): it unlocks coherent aperture
gain, cross-node AoA/interferometry, multistatic 3D imaging, and absolute
geometry — all of which are today blocked by the
U(1) gauge (`20`). Upstream dependencies: per-face calibration (E1) ⇒ one
scalar per pucket (E2) ⇒ common-observation gauge (mechanism 1) ⇒ frequency
steering (mechanism 3) ⇒ phase loop (mechanism 4) ⇒ coherent home aperture
(mechanism 5) ⇒ RF-SLAM/geometry (`16`) and environment inference (`18`).
Parallel enabling capability: **FTM/MDS self-survey** (`15`) gives the geometry
that every coherent estimator consumes. Learned capability chains from `19`:
channel charting (`03`) and Sim2Real give the home-specific layer without
labels.

**(B) Existing models and data.** Directly usable: the distributed-MIMO
calibration theory and estimators (Larsson/Rogalin/Zhu/Ngo) as the loop's
mathematical skeleton; ESPARGOS hardware designs, firmware and datasets for the
front end (GPL/open, DaRUS DOI); ESP-SDR for raw IQ; the multipath-assisted
localisation model (MINT/SALMA) for virtual anchors; standard array processing
(MUSIC/ESPRIT, polarimetric Jones calculus as in ESPARGOS's AR demo). Licence
notes: ESPARGOS/esp-sdr is GPL-3.0 (firmware), datasets as published; the
ESP32 code paths are ESP-IDF (Apache-2.0). None of these is a complete
home-aperture sensing stack — the models are per-link or communications-first.

**(C) Own data / new technology.** The minimal new datasets this program must
produce: (i) the **face-averaging law** and phase residual distribution
(decisive experiment); (ii) **holdover coherence time vs temperature** on S3
and C5 (extends `12`'s open item); (iii) the **incoherent-vs-coherent 3D
accuracy curve vs pucket count** in a real home; (iv) a **farm-house-to-house
transfer set** (same scene, different homes) to test S2. Falsifiers are stated
with the experiment.

## One-line answer

**The invention is ApertureLock: turn each 30 cm polyhedron into a
one-scalar, face-averaged phase meter (conducted-reference coherent 3D array),
then lock all puckets together with a scene-anchored phase loop built from
ordinary 802.11 frames — gauge cancellation by common observation, face
averaging, a steerable hub reference for frequency, and a Kalman phase tracker
— so the home becomes one coherent ≈1° aperture with decimetre-scale 3D
sensing and an incoherent fallback; if the ESP32 per-packet cross-node phase
residual cannot be averaged and tracked (the one decisive measurement), the
space is closed by the packet-cadence-vs-wrap-time ratio and the minimal
reference that crosses it is a single conducted 2.4 GHz sync tree — or one
cable-free common emitter if the loop closes.**
