# 14 — Map/location-aided calibration and environment-as-antenna

Authored for the Aquila v2 20-agent deep-research brief. Australian English.
Reference hardware: ESP32-S3. Sources are primary (journals, conference
proceedings, preprints, upstream vendor docs) unless flagged otherwise.

## Question

How can a known map or known node positions aid calibration of CSI sensing, and
to what extent can the environment itself act as an antenna/aperture? Sub-asks:
(a) map- and location-aided calibration; (b) environment-as-antenna /
reflectarray analogues; (c) passive-reflector gain; (d) limits of uncontrolled
scatter.

Two distinct meanings of "calibration" matter here and are kept separate
throughout:

- **Bias calibration** — removing hardware/geometry-dependent offsets (per-boot
  phase, AGC, chip gain, room-specific noise floor) so a sensing model transfers.
- **Geometric calibration** — pinning down absolute coordinates/orientations of
  nodes, reflectors and scatterers so a physical (ray-tracing / scattering) model
  can be inverted.

A "map" supplies both: surveyed node positions give geometry, and a floorplan
supplies the large static scatterer set (walls, furniture) that dominates the
channel.

## Findings

### ESTABLISHED

**Map/floorplan-aided positioning is mature on COTS Wi-Fi, and the gain is
real but bounded.** A floor map used as a *constraint* (topology-constrained
KNN, map-aiding plus map-matching, fusion with pseudo-odometry) reduced mean
position error versus pure Wi-Fi: MSE improved by ~22.9% and maximum error by
~53.5% in a fused Wi-Fi/pseudo-odometry system, with a further ~1 m RMS
improvement from adding map-matching [S1, S2]. The mechanism is geometric
constraint — a floorplan eliminates physically impossible ("cross-wall")
solutions — not improved per-measurement precision. This is a *localisation*
result on Intel-NIC-class COTS, not a CSI-sensing calibration result.

**Map construction itself can be made uncalibrated.** An HMM over the channel
propagation matrix recovers the data-collection trajectory directly from the CSI
sequence, eliminating location-labelled calibration; simulated MIMO-OFDM reaches
~0.65 m average localisation error in mixed LoS/NLoS, beating supervised KNN/SVM/DNN
[S3]. Established in simulation, not on ESP32.

**A RIS/reflectarray operates as a controllable aperture and behaves, to first
order, as an "anomalous mirror."** The far-field SNR of an IRS-aided link grows
as N² in the number of elements N [S4]. This is the quantitative version of
"environment-as-antenna" when the environment is *controlled*. But [S4] also
proves the scaling law saturates once the array is large enough that the far-field
approximation breaks down, and — decisively for Aquila — an optimized IRS is a
**concave mirror that focuses**, not a flat specular mirror; a mirror-mimicking
surface wastes all area beyond a size set by the geometry. With a **fixed physical
aperture** there is no N² free lunch: gain is bounded by physical area.

**A completely passive conductor already gives usable, free gain — and this is
demonstrated on ESP32-S3.** A 123 × 123 mm, 0.2 mm copper plane reflector placed
1/8 λ (15 mm) behind the ESP32-S3 module's built-in PIFA improved measured RSSI in
both LOS and NLOS, and, combined with the PIFA, was one of two configurations that
sustained through-wall HAR across five rooms at 18 m (the other was an external
biquad antenna) [S5]. Notably the *plane* reflector outperformed a 90° corner
reflector, attributed to spacing/geometry-induced destructive interference. The
Wallhack1.8k dataset (1806 CSI amplitude spectrograms, ESP32-S3) was released with
this work [S5].

**A rich static multipath environment can act as a *coded aperture*.** In a
chaotic cavity, reverberation multiplexes deeply sub-wavelength detail into
intensity-only single-frequency single-pixel measurements, achieving λ/76
localisation with a small programmable metasurface and deep learning [S6]. This is
the strongest published evidence that uncontrolled scatter is an information
resource, not only a nuisance.

**Self-calibration across rooms, chips and Wi-Fi generations is achievable on
ESP32 with no map.** OpenCSI exposes each mesh link as a single dimensionless
Z-score (amplitude divided by its own quiet-period temporal standard deviation),
learned online from a short empty-room bootstrap; room geometry and chip gain enter
the denominator multiplicatively and cancel, transferring one empty-vs-occupied
threshold zero-shot across three rooms and ESP32-S3/C3/C6 at HT20 and HE20
[A1]. It publishes a reliability/maturity signal and detects drift. This is
*bias* calibration by statistics, complementary to map-aided *geometric*
calibration.

### HYPOTHESIS

**Known node positions buy geometric calibration of a physical CSI model.**
Prior RIS location/orientation is explicitly proposed as an **anchor** that can
substitute for CSI, and a priori map information can be exploited to decide which
surface to activate [S7]. This is reasoned/analysed in that paper with position-error
bounds, not experimentally demonstrated on commodity ESP32. On Aquila's hardware,
the plausible mechanism is: surveyed node coordinates + floorplan → Fresnel-zone
geometry (which volumes perturb each link and by how much) → physically grounded
feature selection and per-link sensitivity weighting, replacing learned
room-specific normalisation.

**Environment-as-antenna is a *siting* aid, not a general aperture for Aquila.**
Combining [S4] (fixed-aperture bound), [S5] (single-digit-dB reflector gain on
ESP32-S3) and [S6] (needs a controlled/reverberant structure plus a coherent
receiver) leads to the hypothesis that Aquila can deliberately place cheap passive
reflectors (or a small soldered-polyhedron aperture) to shape which Fresnel zones
are sampled, but cannot conjure aperture gain from arbitrary room scatter without
either physical area or control.

### SPECULATION

**A surveyed, reconfigurable reflector could act as a "poor man's RIS" anchor
for phase.** If a node could switch a small reflector's presence (e.g. a servo or
a PIN-diode panel), the *differential* channel with reflector on/off would isolate
the reflector path from static clutter, giving a known-geometry reference path —
analogous to the multi-receiver calibration trick used to cancel unknown
modulation in metasurface backscatter [S8]. Whether ESP32-grade phase stability
survives the on/off subtraction is unproven; this overlaps Prompt 13
(static-multipath self-referencing) and Prompt 10 (reciprocity). Flagged as
speculation.

**A soldered-PCB polyhedron could be passively driven by room scatter.** A 20-face
icosahedron acting as a fixed-geometry scatterer cluster could seed a known
"calibration constellation" in the channel, letting a map-aided solver jointly
estimate node pose and environment. No published demonstration; speculative.

## ESP32-S3 feasibility

- **Bias/self-calibration (map-free):** demonstrated on ESP32-S3 (and C3/C6)
  as a mesh, open-source Apache-2.0 [A1]. Feasible now.
- **Passive plane/corner reflector gain:** demonstrated on ESP32-S3 with the
  built-in PIFA [S5]. Feasible now, and cheap (copper sheet + printed frame).
  Gain is modest (directionality/nulling of backside noise; single-digit dB
  region), consistent with the ~11 dB aperture-gain ceiling for a 123 mm plate at
  2.4 GHz (G ≈ 4πAη/λ², A ≈ 0.015 m², λ ≈ 0.125 m).
- **Map-aided geometric calibration:** *not* demonstrated on ESP32-S3. The
  underlying biased-range/CSI geometry transfer from Intel-NIC work is plausible
  but untested at ESP32's coarser CSI (52 subcarriers HT20) and phase instability.
  HYPOTHESIS.
- **Environment-as-antenna (RIS / metasurface / coded aperture):** *not* on
  ESP32. Published work uses controllable metasurfaces plus coherent SDR receivers
  [S6, S8] or commodity Wi-Fi with an added IRS [S9]. The gap is a
  phase-controlled surface and a coherent multi-element receiver, neither of which
  ESP32-S3 provides.
- **On-site calibration precedent from the vendor:** Espressif's esp-csi solution
  ships an on-site calibration workflow (learn site-specific wander/jitter
  thresholds) and a co-crystal dual-board (ESP32-C3 + ESP32-S3 sharing a clock
  buffer to remove relative CFO), which is a *hardware* route to a shared
  reference [A2].

## TX needed?

- Map-aided *bias/self-calibration* (OpenCSI): passive; no Aquila-controlled TX
  required beyond ordinary frames (it is receiver-side statistics).
- Passive-reflector gain [S5]: uses a normal TX/RX pair; no special frames.
- Geometric map-aided calibration via ranging: benefits from Aquila's
  `esp_wifi_80211_tx` and the ping-and-unique-reply round-trip scheme (ranges →
  node self-localisation → map registration). TX required, but only compliant
  ordinary frames (see Prompt 07 for the legal envelope).
- RIS/metasurface/coded-aperture environment-as-antenna: would require a
  *separately controlled* surface (extra hardware), plus a coherent receiver; not
  a TX-primitive question.

## Round-trip scheme (asked of every prompt)

The ping-and-unique-reply scheme — one node pings, every other replies with a
unique packet, round trips timed — is, for this prompt, a **map-registration
primitive**. Each node's reply time gives a round-trip range; several
simultaneous ranges from surveyed anchors enable multilateration and, conversely,
let a node self-localise inside a known floorplan. Two-way exchange cancels the
clock bias that one-way time-of-flight cannot. What it measures: coarse ranges
(dm–m, FTM-class [S5][A2]), node clock-offset/identity (unique packets), Doppler
only via drift, and a per-link channel. Where it breaks: multipath range bias,
no common LO across nodes (per-node phase unobservable), and collisions when many
nodes reply concurrently (TDM required — see Prompt 09). It cannot, by itself,
give the sub-wavelength geometric precision that an RIS/coded aperture exploits.

## Evidence strength

**6 / 10.**

Justification: the *component* claims are individually strong and primary —
map-aided localisation gain on COTS (multiple independent Sensors papers),
fixed-aperture RIS scaling proved analytically [S4], passive-reflector gain on
ESP32-S3 measured [S5], self-calibration on ESP32-S3 measured and open-sourced
[A1], coded-aperture subwavelength coding proved in a cavity [S6]. What holds the
score down is the **synthesis gap**: no single primary source demonstrates
map-aided calibration *of CSI sensing* on ESP32-S3, and no source demonstrates
environment-as-antenna on ESP32-class hardware. The chain is inference across
heterogeneous hardware, and the c) passive-reflector gain is measured only
qualitatively (RSSI trends, HAR range) in [S5], not as a controlled dB figure.

## Primary sources

- **[S1]** "A Floor-Map-Aided WiFi/Pseudo-Odometry Integration Algorithm for an
  Indoor Positioning System", Sensors 15(3):6496–6524 (2015).
  https://pmc.ncbi.nlm.nih.gov/articles/PMC4431230/
- **[S2]** "A Map/INS/Wi-Fi Integrated System for Indoor Location-Based Service
  Applications", Sensors 17(6):1272 (2017). https://doi.org/10.3390/s17061272
- **[S3]** "Unsupervised Radio Map Construction in Mixed LoS/NLoS Indoor
  Environments", arXiv:2510.08015. https://arxiv.org/abs/2510.08015
- **[S4]** E. Björnson, L. Sanguinetti, "Power Scaling Laws and Near-Field
  Behaviors of Massive MIMO and Intelligent Reflecting Surfaces", IEEE Open
  Journal of the Communications Society 1:1306–1324 (2020); arXiv:2002.04960.
  https://arxiv.org/abs/2002.04960
- **[S5]** "Directional Antenna Systems for Long-Range Through-Wall Human Activity
  Recognition", arXiv:2401.01388.
  https://arxiv.org/abs/2401.01388 (ships the Wallhack1.8k dataset; ESP32-S3
  plane/corner reflector).
- **[S6]** P. del Hougne et al., "Deeply Subwavelength Localization with
  Reverberation-Coded Aperture", Phys. Rev. Lett. 127, 043903 (2021);
  arXiv:2102.05642. https://doi.org/10.1103/PhysRevLett.127.043903
- **[S7]** "Radio Localization and Mapping with Reconfigurable Intelligent
  Surfaces", arXiv:1912.09401 (RIS anchor; prior location substitutes for CSI).
  https://arxiv.org/abs/1912.09401
- **[S8]** "Metasurface-assisted massive backscatter wireless communication with
  commodity Wi-Fi signals", Nature Communications 11:4171 (2020).
  https://doi.org/10.1038/s41467-020-17808-y
- **[S9]** "Multi-Person Passive WiFi Indoor Localization with Intelligent
  Reflecting Surface", arXiv:2201.01463 (commodity WiFi + IRS; 9×9 elements;
  without-IRS RMSE ≈ 2.8 m, optimized-IRS cm-level, simulation).
  https://arxiv.org/abs/2201.01463
- **[S10]** P. Holl, F. Reinhard, "Holography of Wi-Fi Radiation", Phys. Rev.
  Lett. 118, 183901 (2017); arXiv:1612.03194 (3 × 2 m synthetic aperture,
  cm-scale 3D localisation, dark-field multipath suppression).
  https://doi.org/10.1103/PhysRevLett.118.183901
- **[S11]** "Wireless Communication Using Metal Reflectors: Reflection Modelling
  and Experimental Verification", arXiv:2211.08626 (RCS-based metal-plate model;
  5λ × 5λ copper plate, 8 m links). https://arxiv.org/abs/2211.08626
- **[S12]** "Metasurface-Enabled Multi-Target WiFi Sensing" (SlingShot), IEEE
  Trans. Mobile Computing 25(3):3985–3997 (2026) (commodity Wi-Fi + beam-scanning
  metasurface; passive clock synchronisation).
- **[A1]** "OpenCSI: Self-Calibration Layer for Heterogeneous Mesh Wireless Sensor
  Networks", arXiv:2607.26665 (Apache-2.0; ESP32-S3/C3/C6 mesh; per-link Z-score;
  cross-room/cross-chip/cross-generation transfer).
  https://arxiv.org/abs/2607.26665
- **[A2]** Espressif, "Introduction to ESP-CSI Solution" (on-site calibration;
  ESP32-C3 + ESP32-S3 co-crystal dual-board sharing a clock buffer).
  https://docs.espressif.com/projects/esp-techpedia/en/latest/esp-friends/solution-introduction/esp-csi/esp-csi-solution.html
- **[A3]** Espressif component `esp_wifi_sensing` v0.1.1 (built-in on-site
  training workflow). https://components.espressif.com/components/espressif/esp_wifi_sensing

Standard radar-support primaries for the passive-gain arithmetic: flat-plate
broadside RCS σ = 4πA²/λ², corner reflectors high RCS over ±60° (Knott, Shaeffer
& Tuley, *Radar Cross Section*; RF Café EW handbook
https://www.rfcafe.com/references/electrical/ew-radar-handbook/radar-cross-section.htm).

## Residual unknowns

1. No measured dB figure for ESP32-S3 passive plane/corner reflector gain
   (directionally positive only, [S5]).
2. Whether map-aided *geometric* calibration improves CSI-sensing metrics
   (as opposed to position error) on ESP32-S3 — untested.
3. Whether an on/off switching reflector can isolate a stable calibration path at
   ESP32 phase-noise levels.
4. How much of the coded-aperture subwavelength benefit survives without a
   reverberant cavity and a phase-controlled surface (open-plan room, ESP32).
5. Espressif co-crystal clock-sharing: how it scales beyond two chips on one PCB
   (relevant to a polyhedron with a shared clock).
6. Licence/format fit of the Intel-NIC map-aided datasets for ESP32 (different CSI
   shape: 30/90 vs 52 subcarriers).

## (A) Chaining

- **Map / known node positions → geometric calibration → model-based CSI
  interpretation.** Known coordinates make Fresnel-zone reasoning possible;
  without them, features must be learned per-room (Prompt 19).
- **Round-trip ranging (Prompts 11, 15, 17) → node self-localisation → map
  registration → map-aided calibration.** This is the concrete chaining that
  turns Aquila's own TX into a map: ranges from anchors give positions, positions
  give geometry, geometry gives calibration. Blocked without a shared timing
  reference or a common-transmitter escape (Prompt 11).
- **Environment-as-antenna (reflector/RIS) requires known geometry to exploit.**
  Chaining: map → correct reflector siting → passive gain. Conversely,
  *uncontrolled* scatter without a map is an uncontrolled aperture — the same
  physics that helps [S6] hurts [S3]-style calibration.
- **Bias self-calibration [A1] is orthogonal and always available**, so it is the
  floor on which map-aided geometric calibration must add value.
- **Highest-leverage enabling capability:** a surveyed mesh geometry
  (node positions + floorplan), because it unlocks both map-aided calibration and
  intelligent passive-reflector siting. It in turn is unlocked by the round-trip
  scheme / common reference.

## (B) Existing models and data

- **OpenCSI** [A1] — Apache-2.0 library + ESP32-S3/C3/C6 traces; directly usable.
- **Wallhack1.8k** [S5] — ESP32-S3 CSI amplitude spectrograms, through-wall HAR;
  directly usable; check dataset licence before redistribution.
- **CSI-fingerprinting models on Intel 5300** (DeepFi [see S1-era refs],
  MobLoc, SpotFi) — architecture reusable, input format differs (30/90 vs 52
  subcarriers); phase-not-interchangeable.
- **RIS channel/geometry tooling:** SimRIS-style simulators and codebooks
  (surveyed in [S4], [S9]); usable for design, not for ESP32 measurement.
- **Metal-reflector RCS model** [S11] — closed-form, usable to predict passive
  gain for a chosen plate size/orientation without measurement.
- **del Hougne coded-aperture method** [S6] — algorithm reusable; requires a
  coherent receiver and a cavity/controlled scatterer Aquila lacks.
- **Espressif esp-csi / esp_wifi_sensing** [A2, A3] — vendor on-site calibration,
  directly usable.

## (C) Own-data / new technology

A purpose-built Aquila dataset could unlock something not currently
demonstrated: **map-aided calibration of CSI sensing on ESP32-S3**, and a clean
measurement of **passive-reflector gain on ESP32-S3** as a function of
reflector size, spacing and orientation.

Minimal experiment (tracer bullet):

1. Survey an 8-node ESP32-S3 mesh (perimeter topology, as [A1]) in one room with
   a measured floorplan; record node coordinates to ±2 cm.
2. Conditions: (i) bare PIFA; (ii) plane reflector at 1/8 λ and at swept offsets;
   (iii) corner reflector; (iv) reflector removed. Log per-link RSSI and CSI.
3. Protocol: OpenCSI empty-room bootstrap per condition, then occupied/moving
   trials; compute per-link gain (dB) and cross-condition sensing transfer.
4. Cross-room transfer to a second surveyed room to test whether *geometry*
   (map) explains residual variance that statistics [A1] cannot.

What it would prove: a measured dB gain for passive reflectors on ESP32-S3, and
whether adding known geometry/map to OpenCSI improves cross-room transfer beyond
the zero-shot statistics-only baseline.

What falsifies it: if plane/corner reflector gain is below the per-link noise
floor (no measurable RSSI/CSI change), or if map/geometry adds no transfer
improvement over OpenCSI alone, then map-aided geometric calibration is not
worth pursuing on this hardware and effort should stay on bias self-calibration.
