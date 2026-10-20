# 16 — RF-SLAM, occupancy grids, floorplans, loop closure

**Question.** How far can SLAM on RF measurements go — occupancy grids, floorplan
inference, loop closure — and what does it require? Sub-questions: (a) RF-SLAM
formulations; (b) occupancy-grid/floorplan inference from RF; (c) RF loop closure;
(d) sensor fusion and drift limits.

Australian English. Sources are primary: the papers named in the brief (waveSLAM,
MOSAIC, Thrun) plus the multipath-SLAM (MP-SLAM) literature (Witrisal/Leitinger/
Mendrzik), RFMap (IPSN/TOSN), UbiqMap, Wi-Closure, P²SLAM/ViWiD, and — the one
directly on our hardware — Wi-PRO on ESP32-S3. Claims are separated
ESTABLISHED / HYPOTHESIS / SPECULATION and the hardware tier is stated throughout.

> **Scope note.** This note is about *mapping* (building a geometric model of the
> environment) and *loop closure*, not about localising a single node. Node
> self-localisation is prompt 15; channel charting (relative geometry from
> unlabelled CSI, no map) is prompt 03; map-aided calibration is prompt 14. Those
> overlap and are cited only where RF-SLAM depends on them.

---

## 1. Findings

### 1.1 The two RF-SLAM families

**ESTABLISHED — there are two distinct, mature formulations, and they have
different inputs.**

1. **Multipath-based SLAM (MP-SLAM / Channel-SLAM)** treats each specular
   multipath component (MPC) as a *virtual anchor* (VA): the mirror image of a
   physical anchor/transmitter across a reflecting surface. Estimating the delay
   and angle-of-arrival (AoA) — and where available angle-of-departure (AoD) — of
   each MPC therefore yields both the agent pose *and* the surface geometry, so
   localisation and mapping are solved jointly. The canonical algorithms are
   **Channel-SLAM** (recursive Bayesian filter; Meissner & Witrisal 2012;
   Mendrzik et al.) and **BP-SLAM** (belief propagation / factor graph; Leitinger,
   Meyer, Hlawatsch, Witrisal, Tufvesson, Win, *IEEE TWC* 18(12):5613–5629, 2019).
   The critical input requirement is **joint delay–angle resolution**: MP-SLAM
   needs a channel estimator that resolves individual MPCs (their "standard"
   back-end is a UWB/massive-MIMO estimator). Geometry is expressed as virtual
   anchors whose positions are the mirror of the physical anchor; VAs stay static
   while the anchor stays static, so they are stable map landmarks.
   *(Leitinger 2019 DOI 10.1109/TWC.2019.2945525; Meissner & Witrisal 2012;
   "Data Fusion for Multipath-Based SLAM", 2023.)*

2. **RF-imaging / occupancy-grid SLAM** does not require explicit MPC-vs-surface
   association. It scans the environment (by moving a sensor, or by rotating a
   beam, or by a dense fixed sensor mesh) and back-projects measured reflections
   into a spatial occupancy grid.

**ESTABLISHED — both reduce to the same primitive: RF gives you *range to
reflectors* and/or *bearing to reflectors/anchors*; SLAM is then a standard
robot-mapping problem over those primitives.** Thrun's *Probabilistic Robotics*
occupancy-grid formalism (log-odds update, inverse sensor model) is used
unchanged underneath the radio front-end; RF contributes only the measurement
model. *(Thrun, Burgard, Fox, Probabilistic Robotics, MIT Press 2005.)*

### 1.2 Occupancy-grid and floorplan inference from RF

**ESTABLISHED — mmWave/multi-GHz systems do this today, with cm-to-decimetre
fidelity.**

- **RFMap** (Khan, Venkatnarayan, Shahzad; IPSN 2020; extended ACM TOSN 19(?,)
  2023) transmits RF, measures the **power delay profile (PDP)** at every
  look-angle as the antennas rotate, builds a polar grid `G(r,θ)`, extracts
  reflectors, classifies them as wall/corner/edge, removes "shadow reflectors"
  caused by second-order multipath, and uses a conditional **GAN** to fill the
  sparse gaps into a *fully connected* floorplan. Reported accuracy beats
  ThruMapper (0.14 m LoS, 0.25–0.57 m NLoS walls); it maps up to three levels of
  walls to ~35 ft depth and does **not** need to move along the walls. **Hardware
  tier: SDR-grade — two USRP N210s plus a 2×2 grid of 18° directional panel
  antennas on a 1° turntable.** It explicitly assumes a *static* channel (human
  motion degrades it) and near-perpendicular walls.
  *(DOI 10.1145/3534121; IPSN 2020 DOI 10.1109/IPSN48710.2020.00-40.)*

- **Mobile Radio Tomographic Imaging (UbiqMap**, Hunter/Shahzad et al., *IEEE
  TMC* 23(12):14031–14043, 2024) builds maps from RSS attenuation tomography along
  a short robot trajectory with minimal pre-deployed infrastructure; reports
  30–40 % map-quality improvement over prior RTI with equivalent resources, and
  advocates fusing RTI with SLAM. Evaluated in an 800 sq ft apartment and a
  3000 sq ft foyer. **Hardware tier: COTS Wi-Fi radios on a moving robot.**

- **Structure from WiFi (SfW**, Kim et al., arXiv 2403.02235, 2024) infers the
  *free space* of an unknown environment from RSSI via an "inverse k-visibility"
  formulation — coarse, robot-borne, but it demonstrates that even RSS alone
  carries geometric occupancy information.

- **waveSLAM** (Picazo et al., arXiv 2312.07192, 2023) is **not** Wi-Fi: it uses
  COTS **60 GHz mmWave** comms devices plus LiDAR on a robot, exchanging angle and
  distance estimates by multipath "self-sensing" to sharpen an occupancy grid.
  Error < 22 cm and 20° — "compatible with LiDAR". This is the closest analogue to
  the brief's MOSAIC idea in spirit; **MOSAIC** (Jog et al., 2018) does mmWave
  SLAM without a-priori knowledge using AoA/AoD from large arrays.

**ESTABLISHED — the raw material for RF mapping is *resolution*, and Wi-Fi at
2.4 GHz barely has it.** Delay resolution ≈ 1/bandwidth. A 20 MHz ESP32 channel
gives ≈ 50 ns ≈ **15 m** of path separation; 40 MHz gives ≈ 7.5 m. That is one to
two orders coarser than the 1.76 GHz systems (≈ 17 cm) that make clean reflector
mapping routine. CIRSense (arXiv 2510.11374, 2025) states this plainly: path
resolution and multipath separability "scale with channel bandwidth", and a
lower-bandwidth system convolving every path with a sinc makes one-to-one tap↔path
mapping impossible without sub-grid fractional-delay recovery. It achieves
**0.09 m** dynamic-target distance estimation only at **160 MHz** on a
Picoscenes mini-PC NIC.

### 1.3 RF loop closure

**ESTABLISHED — RF is fully capable of *loop closure* (drift correction), and
this is arguably its most robust contribution to SLAM.**

- **Wi-Closure** (arXiv 2210.01320) uses WiFi/UWB **AoA + range** to *prune* the
  inter-robot loop-closure search before the expensive place-recognition stage,
  resolving multipath by requiring AOA measurements to be mutually consistent in
  a loop (PCM). Result: computation down 54 % (sim) / 77 % (hardware) and
  absolute trajectory error from **66.1 m → 1.3 m** (sim) and **17.6 m → 1.9 m**
  (hardware); 99 %/79 % of false loop closures rejected, ≤ 15 % true ones missed.
  Hardware: two Locobots with Intel NUC + WiFi sensing toolbox (AoA from phase
  across antennas) + UWB.
- **Wi-Loop SLAM** (Gao, Fan, Zhai, Dai; *IEEE TWC* 23(10):15185–15197, 2024,
  DOI 10.1109/TWC.2024.3426544) performs loop closure *inside* MP-SLAM using the
  multipath parameters themselves as matching features — no extra sensor. Corrects
  drift "where propagation paths are often obscured"; **simulation only**.
- **Wi-Fi-augmented visual SLAM** (arXiv 1903.06687) uses **RSS similarity** to
  bound and verify loop closures in RTAB-Map on an Intel 7265; WiFi recovers
  correct loop closures and a much smaller trajectory drift where vision alone
  failed (glass, glare).
- Classic **WiFi GraphSLAM** (Huang, Millman, Quigley, Stavens, Thrun, Aggarwal,
  ICRA 2011) casts RSS SLAM as GraphSLAM in O(N²), removing the shape constraints
  GP-LVM needed.

### 1.4 Drift, fusion and accuracy limits

**ESTABLISHED — RF never replaces inertial/odometric dead-reckoning; it corrects
it.** Every system above is a *sensor-fusion* system: Wi-Fi provides sparse,
absolute constraints (bearings to landmarks, loop closures, ranges) that pull a
drifting odometry backbone back. P²SLAM and ViWiD make this explicit — WiFi
bearings to ceiling APs are added as factors in GTSAM alongside wheel encoders/VIO,
and the map built is a map *of APs*, not of walls.

**ESTABLISHED — with pure RF and no motion, you get a sparse "map" of reflectors,
not a dense grid.** RFMap is the strongest counter-example (it generates a dense,
GAN-completed floorplan from a stationary SDR), but only with an SDR, a rotating
directional antenna, and strong (concrete/steel) reflectors.

### 1.5 ESP32-S3 specifics

**ESTABLISHED — RF-SLAM as such has *not* been demonstrated on ESP32 / ESP32-S3.**
No primary source found that builds an occupancy grid, floorplan or loop-closure
framework on ESP32 CSI. The ESP32 literature is **localisation**, not mapping:

- **Device-free localisation with ESP32 CSI** (preprints.org 202601.2378, 2026)
  does 2D DFL with **three ESP32 transceivers** over a 6 m × 4 m lab, 100 Hz CSI,
  35 grid points, >1.2 M snapshots, formulated as grid-cell classification +
  coordinate regression. That is a *fingerprint map over a known grid*, not SLAM.
- **ESP32-CSI-Tool** (MIT, StevenMHernandez) and **esp-csi** (Espressif,
  Apache-2.0) expose amplitude and phase on up to 64 subcarriers (20 MHz;
  40 MHz via 802.11n bonding) — the raw material for CSI fingerprints, but no
  mapping stack. Espressif rates S3 CSI below C5/C6.
- **Phase/AoA caveat.** Commodity ESP32 boards are single-antenna; AoA needs an
  antenna-switching array (Espressif's own motion board uses an ESP32-C3 with
  three switched directional antennas **phase-locked by a shared clock** to an
  ESP32-S3). Independent-LO cross-node phase is unobservable (see the brief's
  assumption), so *per-node AoA from one ESP32 is unavailable without an added
  reference*.

**ESTABLISHED (the one direct hit) — accurate ESP32-S3 *ranging* is now
demonstrated.** **Wi-PRO** (UCSD wcsng, 2026, wcsng.ucsd.edu/files/wi_pro.pdf)
runs FTM + CSI on **< $6 ESP32-S3** nodes, reconstructs a band-compensated CIR,
and runs leading-edge detection *on the ESP32-S3 in 24 ms*. It cuts FTM range
error from a stock **8.23 m median (LoS) / 18.52 m (NLoS)** to **1.51 m / 2.39 m**
(90th pct 6.65 m / 14.5 m); positioning median 3.17 m / 3.52 m. Crucially it
documents the ESP32 failure mode for mapping: **"multiple paths up to 6 m apart
can merge into a single peak, making it impossible to recover which one is the
shortest through peak detection alone."** That is the resolution wall, quantified
on our hardware.

### 1.6 HYPOTHESIS

- **H1.** ESP32-S3 can support *bearing-only* or *range-only* RF-SLAM of AP/VA
  landmarks (P²SLAM/ViWiD-style), because those need bearings/ranges, not wall
  imaging. Bearings require either a switched antenna array with a shared clock
  (esp-crab) or robot motion; ranges require Wi-PRO-quality FTM. Plausible but
  **not demonstrated on S3**.
- **H2.** A dense ESP32 fixed mesh (many TX/RX links) can substitute spatial
  resolution with **spatial diversity**: RTI/UbiqMap suggests a many-link mesh
  reconstructs an attenuation map without a moving aperture. Whether ESP32's link
  count and CSI quality give useful grid resolution is untested. *(Adopted from
  the RTI formulation of Wilson & Patwari and UbiqMap.)*
- **H3.** The brief's **round-trip ping-and-unique-reply** scheme is, in effect, a
  distributed range-measurement primitive: if node 0 pings and every other node
  replies with a unique packet, the round-trip times give *ranges from node 0 to
  every node* — i.e. a **range-only SLAM** edge set. This is the same measurement
  class as Wi-Closure's UWB ranging and could supply loop-closure constraints and
  inter-node geometry. It does **not** directly measure reflectors; it measures
  node-to-node ranges. Coarse (FTM-class) and multipath-biased unless
  Wi-PRO-corrected.

### 1.7 SPECULATION

- **S1.** A **soldered-PCB polyhedron** (icosahedron = 20 nodes) acting as a
  compact, *phase-coherent-aperture* array could supply the AoA/AoD resolution
  (spatial, not bandwidth) that single-antenna ESP32s lack, turning each
  reflection into an estimable angle. If the 20 faces share a clock, this is a
  volumetric aperture that could give 3-D DoA without relying on delay resolution.
  **No source found demonstrating this on ESP32**; it is a synthesis of the array
  literature and the geometry questions in prompts 08/09.
- **S2.** Because ESP32 delay resolution is ~7–15 m, wall-level floorplanning is
  out of reach, but *room-level* topology (which rooms are adjacent) from
  fingerprint/loop-closure graphs — the SmartSLAM/SfW line — is plausible.

---

## 2. ESP32-S3 feasibility

| RF-SLAM ingredient | Demonstrated on | Nearest ESP32-S3 evidence | Verdict on S3 |
|---|---|---|---|
| Multipath SLAM (MP-SLAM/BP-SLAM) | UWB / massive-MIMO / mmWave | none | **Blocked**: needs joint delay–angle resolution S3 lacks |
| Dense floorplan (RFMap) | USRP N210 SDR + rotating 18° antennas | none | **Blocked**: no phase-coherent array, ~7–15 m delay bins |
| Occupancy/attenuation mapping (RTI/UbiqMap) | COTS Wi-Fi mesh/mobile robot | ESP32 DFL grid (localisation only) | **Plausible (H2)**: many-link mesh, room-scale |
| Bearing-based SLAM (P²SLAM/ViWiD) | Atheros/Intel CSI NICs | esp-crab shared-clock array exists | **Possible (H1)**: needs switched array + shared clock |
| RF loop closure (Wi-Closure) | Intel NUC + AoA/UWB | none | **Partial**: range-only via FTM/round-trip; AoA blocked |
| Range primitive for SLAM | UWB; ESP32-S3 via Wi-PRO | **Wi-PRO: 1.5–2.4 m median** | **Demonstrated (coarse)** |
| Phase coherence across nodes | — | not demonstrated | **Blocked** (brief's assumption) |

**Bottom line.** On ESP32-S3, dense RF floorplanning is not feasible with the
platform as it stands; coarse range-based geometry and possibly mesh
attenuation-mapping are. Wi-PRO is the single most important ESP32-S3 result
and it delivers *metre-class* ranges, which is enough for room-level topology and
loop closure but not for wall geometry.

## 3. TX needed?

**Yes, for most of it.** Mapping via reflections (RFMap, MP-SLAM, waveSLAM,
MOSAIC) needs a **known transmitted waveform** to measure delay/Doppler/AoA of
echoes — receive-only passive CSI provides no reference for absolute delay.
Options: (a) a dedicated ESP32 TX node (`esp_wifi_80211_tx` compliant frames, or
ESP-NOW); (b) opportunistic APs (Wi-Fi-fingerprint/LoS only, as in SfW/ViWiD).
RTI/DFL uses existing links or a mesh. The **round-trip ping-and-reply** scheme
provides ranges without an external illuminator, since the reply timing is the
measurement.

---

## 4. Evidence strength

**RF-SLAM as a field: 8/10.** Multiple peer-reviewed formulations (BP-SLAM,
Channel-SLAM, RFMap, UbiqMap, Wi-Closure, P²SLAM) with real measurements; RFMap
and Wi-Closure have hardware deployments and quantitative errors.

**RF-SLAM on ESP32-S3: 2/10.** No mapping/occupancy/loop-closure demonstration
found on the platform. The one strong ESP32-S3 primary source (Wi-PRO) is
*localisation+ranging*, and it documents the very resolution limitation
(paths ≤ 6 m merge) that blocks floorplanning. This is honest, not dismissive:
the gap is *resolution and phase coherence*, both structural.

---

## 5. Primary sources

- Leitinger, Meyer, Hlawatsch, Witrisal, Tufvesson, Win. "A belief propagation
  algorithm for multipath-based SLAM." *IEEE TWC* 18(12):5613–5629, 2019.
  DOI 10.1109/TWC.2019.2945525.
- Meissner, Witrisal. "Multipath-assisted single-anchor indoor localization in an
  office environment." 2012.
- "Data Fusion for Multipath-Based SLAM." 2023 (TU Graz, Elsevier Pure).
- Khan, Venkatnarayan, Shahzad. "RFMap: Generating Indoor Maps using RF Signals."
  IPSN 2020, DOI 10.1109/IPSN48710.2020.00-40; extended: "Using RF Signals to
  Generate Indoor Maps," *ACM TOSN*, DOI 10.1145/3534121 (2023).
- "Ubiquitous Indoor Mapping Using Mobile Radio Tomography (UbiqMap)." *IEEE TMC*
  23(12):14031–14043, Dec 2024. (computer.org/csdl/journal/tm/2024/12/10634790.)
- Picazo, Groshev, Blanco, Fiandrino, de la Oliva, Widmer. "waveSLAM…"
  arXiv:2312.07192 (2023).
- Jog et al. "MOSAIC: SLAM and Environment Mapping Using mmWave Without A-Priori
  Knowledge." 2018.
- Thrun, Burgard, Fox. *Probabilistic Robotics.* MIT Press, 2005.
- Gao, Fan, Zhai, Dai. "Wi-Loop SLAM: Loop Closures With Wireless Sensing in
  Multipath SLAM." *IEEE TWC* 23(10):15185–15197, 2024.
  DOI 10.1109/TWC.2024.3426544.
- "Wi-Closure: Reliable and Efficient Search of Inter-robot Loop Closures Using
  Wireless Sensing." arXiv:2210.01320.
- Arun, Ayyalasomayajula, Hunter, Bharadia. "P²SLAM: Bearing based WiFi SLAM for
  Indoor Robots." *IEEE RA-L* 7(2):3326–3333, 2022. (wcsng.ucsd.edu/p2slam)
- "ViWiD: Leveraging WiFi for Robust and Resource-Efficient SLAM." arXiv:2209.08091.
- "Augmenting Visual SLAM with Wi-Fi Sensing For Indoor Applications."
  arXiv:1903.06687.
- Huang, Millman, Quigley, Stavens, Thrun, Aggarwal. "Efficient, Generalized Indoor
  WiFi GraphSLAM." ICRA 2011.
- "Structure from WiFi (SfW): RSSI-based Geometric Mapping of Indoor
  Environments." arXiv:2403.02235 (2024).
- Kim et al. "Probabilistic Occupancy Grid for Radio-Based SLAM." arXiv:2603.03559
  (2026) — occupancy grid + reflection coefficients in MP-SLAM; **simulation only**.
- "Soft-Coherent Direct Multipath SLAM." arXiv:2604.19723 (2026) — phase-coherent
  D-MIMO MP-SLAM; shows noncoherent processing loses aperture.
- **Wi-PRO: Accurate Indoor PNT using FTM with Multipath Suppression.** wcsng.ucsd.edu/files/wi_pro.pdf
  (2026) — ESP32-S3 FTM+CSI ranging.
- "CIRSense: Rethinking WiFi Sensing with Channel Impulse Response."
  arXiv:2510.11374 (2025).
- "Device-Free Indoor Localization with ESP32 Wi-Fi CSI Fingerprints."
  preprints.org 202601.2378 (2026).
- ESP32-CSI-Tool (MIT) github.com/StevenMHernandez/ESP32-CSI-Tool; esp-csi
  (Apache-2.0) github.com/espressif/esp-csi; Espressif ESP-Techpedia ESP-CSI
  solution page (esp-crab shared-clock board).

---

## 6. Residual unknowns

1. Is there **any** ESP32 CSI mapping/occupancy demonstration not surfaced here?
   (Searches found only localisation/DFL/fingerprinting.)
2. What is the **actual ESP32-S3 CSI subcarrier count and phase usability** under
   40 MHz bonding, and does the driver's phase carry the per-subcarrier
   consistency CIR recovery needs (Wi-PRO patched firmware suggests raw phase is
   not clean)?
3. Can a **shared-clock polyhedron array** (S1) deliver usable 3-D DoA on ESP32
   silicon — and does any Espressif/esp-crab variant already do multi-element
   coherent capture?
4. Does a **many-link ESP32 mesh** (H2) reach RTI-quality attenuation maps at
   room scale? Untested.
5. **Round-trip ping-and-reply** timing precision on S3 (tied to the microsecond
   RX timestamp, prompt 17) — what range resolution results, and is it
   multipath-suppressible?

---

## 7. Strategic framing

### (A) Chaining

- **TX capability enables RF-SLAM.** Mapping by reflection needs a known waveform:
  `esp_wifi_80211_tx` (or ESP-NOW) → measurable delay/AoA → RFMap/MP-SLAM-style
  mapping. **Blocked without TX** (or opportunistic APs).
- **Common-clock / shared-reference capability enables bearing-SLAM.** AoA needs
  phase across elements; esp-crab-style shared-clock arrays are the enabler
  (depends on prompts 10/11/12). Independent-LO cross-node phase is unobservable,
  so **per-node AoA is blocked** until a shared/conducted reference exists.
- **Phase-coherent *volumetric* aperture (polyhedron) enables 3-D DoA and dense
  mapping** — the highest-leverage unlock, but undemonstrated (S1). It is the one
  capability that would move ESP32 from range-only to angle-resolved mapping.
- **Range primitive (Wi-PRO / round-trip) enables loop closure and node geometry
  now**, independent of AoA. Loop closure is the cheapest RF-SLAM win.
- **RTI/mesh capability enables infrastructure-free attenuation mapping** without
  a moving aperture (H2), and unlocks material inference (prompt 04/18).
- **Highest-leverage enabling capability overall:** a *shared-clock multi-element
  ESP32 aperture* (esp-crab scaled to the polyhedron). It chains: reference →
  phase → AoA/AoD → MP-SLAM → floorplans → loop closure.

### (B) Existing models/data — can Aquila use them?

| Asset | Type | Usable by Aquila? |
|---|---|---|
| BP-SLAM / Channel-SLAM (Leitinger/Mendrzik) | algorithm | Concepts yes; **needs delay–angle MPC estimator S3 lacks.** Code not found open. |
| RFMap (Khan/Shahzad) | system | **No** — SDR + rotating array; assumptions differ. GAN-completion ideas reusable. |
| UbiqMap / RTI (Wilson & Patwari) | algorithm | **Yes, in principle** — RSS-based, COTS-friendly; needs multi-link mesh. |
| Wi-Closure | algorithm | **Partly** — range factor usable with FTM/round-trip; AoA factor blocked on S3. |
| P²SLAM / ViWiD | system | **Partly** — bearing-based; needs coherent array or motion; code partly open (GTSAM). |
| Wi-PRO (ESP32-S3) | algorithm | **Yes — directly relevant.** FTM+CSI ranging on S3, planned open-source. |
| Thrun *Probabilistic Robotics* | textbook | **Yes** — occupancy-grid/GraphSLAM back-end, platform-agnostic. |
| ESP32-CSI-Tool / esp-csi | tooling | **Yes** — MIT / Apache-2.0, capture layer. |
| Espressif ESP-CSI (esp-crab) | hardware/tooling | **Yes** — shared-clock phase solution; C5-centric. |

### (C) Own-data / new technology

**Could Aquila unlock something not demonstrated?** Possibly, but only with a
purpose-built aperture/experiment — pure single-antenna ESP32 CSI will not.

- **Minimal experiment that would prove the key claim (S1/H1):** a
  **shared-clock multi-element ESP32-S3 aperture** (start with esp-crab-style
  2–3 switched antennas; escalate to a small polyhedron) capturing AoA of a known
  ESP32 TX against a flat reflector, validated against measured ground truth.
  **Falsified if** estimated AoA error is not far below the geometry's angular
  ambiguity — i.e. if shared-clock phase does not yield usable per-element phase.
- **Minimal dataset that would test H2 (mesh mapping):** a fixed ESP32 mesh
  (≥ 6–8 nodes) in a mapped room, RSS+CSI across all links, ground-truth
  occupancy from LiDAR. **Prove** if RTI's log-odds grid recovers walls to
  room-scale accuracy. **Falsify if** link count/diversity is insufficient for any
  better-than-chance grid.
- **Minimal dataset to test range-based loop closure (H3):** log Wi-PRO-style FTM
  ranges + round-trip times during a walk that revisits a location; check whether
  range constraints close the loop and reduce drift. Falsified if range bias is
  too large/nonstationary to constrain.
- **What would be genuinely new:** *phase-coherent 3-D RF mapping on sub-$10
  MCUs* — no published system does this. Even a negative result (shared-clock S3
  phase insufficient) is a valuable, acid-clearing finding.

**Round-trip ping-and-reply, specifically.** It measures **node-to-node ranges**
(and, via timing, coarse offset and identity) — the same measurement class as
UWB ranging in Wi-Closure — so it could supply **range-only SLAM / inter-node loop
closure**. It **breaks** where multipath biases the round-trip (FTM's documented
"+2–3× overestimate", Wi-PRO), where replies collide (needs the unique-packet/TDM
discipline of prompt 09), and where clock drift over the round trip exceeds the
range resolution. It cannot measure reflectors, so it maps *nodes*, not walls.
