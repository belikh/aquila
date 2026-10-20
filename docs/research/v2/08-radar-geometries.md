# 08 — Monostatic / bistatic / multistatic radar geometries

Agent 08 of 20. Prompt: `08-radar-geometries`. Australian English.

## Question

With compliant 802.11 TX (including `esp_wifi_80211_tx`), which mono-, bi- and
multistatic geometries are achievable — including **volumetric arrays
(soldered-PCB polyhedra, geodesic/spherical arrays)** — and how do
synchronisation and self-interference constrain them? Required: (a) geometry
taxonomy and coverage; (b) required sync per mode; (c) monostatic
self-interference; (d) waveform/duty design; (e) cluster spectral sharing;
(f) spherical/volumetric vs planar array benefits for 3D DoA.

The three Aquila radio capabilities in scope throughout:
1. `esp-csi` CSI mode (per-frame CSI, whole ESP32 family).
2. Experimental raw-I/Q receive mode (ESPARGOS / esp-sdr) — receive-only, gapped, uncalibrated.
3. Compliant 802.11 TX (`esp_wifi_80211_tx`), single or clustered nodes.

---

## 1. Geometry taxonomy and coverage

**ESTABLISHED.** Radar geometry is classified by the spatial relation of the
transmitter(s) and receiver(s) to one another and to the target.

| Mode | TX/RX relation | Observable primitive | Coverage/measurement |
|---|---|---|---|
| Monostatic | co-located, one RF chain (or full-duplex) | round-trip delay `τ = 2r_t/c` | range, radial Doppler; range–Doppler map |
| Bistatic | TX and RX separated by a baseline `L` | `τ = (r_t + r_r)/c` | bistatic range defines an **ellipsoid with TX and RX as foci**; angle-sensitive RCS |
| Multistatic | ≥1 TX, ≥2 RX (or vice versa) | set of bistatic ranges/Dopplers | intersects ellipsoids → **3D position**; velocity ambiguity resolution that a single monostatic pair cannot achieve |
| Passive / bistatic-passive | receiver uses an illuminator of opportunity, no cooperative TX | cross-ambiguity with a **reference channel** | covert; needs a direct-path copy |

Source: IEEE Technology Navigator, *Multistatic radar*
(https://technav.ieee.org/topic/multistatic-radar/); the same page states the
target's liability is that geometry *directly* determines the CRLB on position
error — obtuse arrangements near the baseline are poor, geometries that
**surround** the target are good. This is the single most important geometric
design rule and it is exactly what a **volumetric array** maximises.

**ESTABLISHED.** Coherent vs incoherent fusion is a *sync* consequence, not a
geometry one: "When the receivers share a common clock or can synchronize phase
coherently, the network can coherently combine waveforms... Non-coherent
combination, which uses amplitude or energy information rather than phase, is
more practical when precise synchronization is unavailable" (same source).

For WiFi-of-opportunity the illuminator is OFDM; the matched-filter/ambiguity
behaviour is documented for 802.11 specifically (Colone et al., *WiFi-based
passive bistatic radar: data processing schemes and experimental results*, IEEE
TAES 48(2):1061–1079, 2012, DOI 10.1109/TAES.2012.6178049; Falcone et al.,
*Experimental results for OFDM WiFi-based passive bistatic radar*, IEEE Radar
Conf. 2010).

---

## 2. Required synchronisation per mode

**ESTABLISHED — this is the binding constraint, and it maps cleanly onto the
BRIEF's three "escapes".**

- **Monostatic**: TX and RX share one LO and one clock, so phase coherence is
  *free* by construction. The only problem is self-interference (§3).
- **Bistatic/multistatic**: each node has an independent LO. Time, frequency
  **and** phase must be aligned across nodes to combine coherently. This is the
  independent-LO / no-common-reference boundary.
- **Non-coherent multistatic**: no cross-node phase needed; fuse amplitude /
  energy / per-link Doppler only. Diversity gain survives; spatial (coherent)
  gain and angle resolution from cross-node phase do not.

Concretely demonstrated sync mechanisms (all outside the ESP32-S3 world unless
noted):

- **Conducted shared reference** — ESPARGOS distributes a frequency-multiplexed
  **40 MHz clock + 2.4 GHz phase reference** down one coax per board, through a
  cascade of power splitters, with a master returning its own reference to
  SYNC IN (`https://espargos.net/docs/combined-arrays.html`;
  ESPARGOS One manual/datasheet). This is a **shared/conducted reference**, one
  of the three escapes. It is *receive-side only* and phase-coherent across
  combined boards.
- **OTA / wireless two-way** — a leader broadcasts a **two-tone waveform (TTW)**
  for frequency sync and performs a **bi-directional waveform (BDW) exchange**
  for time sync; simulated timing accuracies of 5–20 ns (GPSDO-class) are the
  target (arXiv 2512.22686, *Multistatic Radar Performance in the Presence of
  Distributed Wireless Synchronization*). Picosecond wireless time sync across
  distributed arrays has been demonstrated on SDR (Merlo, Mghabghab & Nanzer,
  IEEE TMTT 71(4):1720–1731, 2023, DOI 10.1109/TMTT.2022.3227878).
- **Common 10 MHz + PPS + FPGA** — USRP-based distributed sounders
  (arXiv 2403.11856) and the 256-antenna D-MIMO testbed (arXiv 2605.02388)
  distribute a 10 MHz reference through an octoclock tree and a timing pulse
  over a daisy-chain; the FPGA trims per-node cable delay in software.
- **Over-the-air reciprocity** (`Ĥ_AB·Ĥ_BA⁻¹`) — per BRIEF, assumed not yet
  proven on ESP32-S3.

**HYPOTHESIS.** On ESP32-S3, the achievable OTA sync is limited by the
microsecond RX timestamp and coarse FTM; the *conducted* escape is the only one
that plausibly reaches the `<1 ns` timing and carrier-phase coherence a coherent
multistatic aperture needs. This must be measured, not assumed.

---

## 3. Monostatic self-interference (SI)

**ESTABLISHED.** Monostatic OFDM radar on commodity WiFi faces SI from Tx→Rx
within the same chain. The literature gives clean numbers:

- ISAC-Fi (arXiv 2408.09851, *Enabling Full-fledged Monostatic Sensing over
  Wi-Fi Communication*): a Tx–Rx separator (circulator/hybrid coupler) gives
  **~12 dB**, an analog cancellator a further **~40 dB**, a digital cancellator
  **~25 dB**, for **~77 dB total**, bringing residual SI "very close to the noise
  floor". Practically, ISAC-Fi pairs an **ESP32 as the Wi-Fi NIC with a LimeSDR
  as the sensing module**, sharing an external 40 MHz TCXO — i.e. the ESP32 is
  *not* doing the monostatic cancellation itself.
- An SDR-based monostatic Wi-Fi system achieves **~40 dB analog SIC** using an
  auxiliary Tx RF chain, stable for 30 minutes without retuning
  (arXiv 2412.08612, *An SDR-Based Monostatic Wi-Fi System with Analog
  Self-Interference Cancellation for Sensing*, Dec 2024).
- A waveform-only escape: **zero-padded (ZP-)OFDM** avoids SI during the guard
  interval without full duplex, at the cost of range-dependent processing
  (Self-Interference-Resistant IEEE 802.11ad-Based Joint Communication and
  Automotive Radar Design, 2021).

**ESP32-S3 reality.** The ESP32 family is a single-radio, **half-duplex** part:
it cannot transmit and receive simultaneously on the same channel. There is no
on-chip circulator, no analog SIC, and the CSI receiver is designed for
reception, not for capturing a self-reflection while transmitting. Therefore:

- A **true monostatic** ESP32-S3 node is **blocked** without external RF
  hardware (circulator + analog cancellation), and even with it the bursty,
  half-duplex MAC makes continuous full-duplex operation impossible.
- A **quasi-monostatic** node is achievable by **time-division**: transmit a
  frame, switch the RF path, then receive the echo. This needs an external
  switch and a delay long enough (≥ round-trip + switching) that the echo can be
  captured — which for room-scale ranges at 2.4 GHz is trivial in time
  (~ns–tens of ns) but *impossible* without bypassing the standard MAC.

**Note.** ESPARGOS One *can* transmit for active radar, but **transmission is
disabled by default and only enabled by an operator authorised to transmit**
(ESPARGOS One datasheet). It remains a receive array with an RF switch — it too
cannot do simultaneous monostatic Tx/Rx.

---

## 4. Waveform and duty design

**ESTABLISHED.**
- **Range resolution** (monostatic): `ΔR = c/(2B)`. With WiFi B=20 MHz → **7.5 m**;
  B=40 MHz → **3.75 m**. A single 20 MHz channel cannot resolve a person-scale
  range; it can resolve *change* against a static background.
- **Doppler resolution**: `Δv ≈ λ/(2·T_int)`, λ=12.5 cm at 2.4 GHz. A 1 s
  coherent integration gives ~6 cm/s bins — person walking speeds sit 0.5–2 m/s,
  so **Doppler is the useful dimension at WiFi bandwidth**, not range.
- **Ambiguity function**: OFDM/CP-OFDM/FBMC give near-**thumbtack** ambiguity
  functions (IET RSN 2016, DOI 10.1049/iet-rsn.2015.0414); CP insertion creates
  two weak sidelobes in delay, and CP length sets the Doppler sidelobe structure.
- **Duty / gaps**: passive 802.11 traffic is bursty and unsaturated; a
  purpose-built TX node can choose duty, PRF and frame rate. Gapped/duty-limited
  illumination broadens the ambiguity function in Doppler and limits the
  coherent integration window (this is the core of prompt 06; the same physical
  limit appears in the multistatic CRLB of arXiv 2512.22686).
- **`esp_wifi_80211_tx` primitives**: sends a raw 802.11 buffer, `24 ≤ len ≤
  1500` bytes, with `en_sys_seq` controlling sequence-number handling, plus an
  optional TX-done callback (`esp_wifi_register_80211_tx_cb`)
  (https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/api-reference/network/esp_wifi.html).
  This is the TX primitive the whole geometry menu depends on; legal envelope is
  prompt 07's job.

---

## 5. Cluster spectral sharing

**ESTABLISHED (system property).** A single ESP32 radio is half-duplex, so within
a cluster a node cannot Tx and Rx at once. Across a cluster sharing one 20/40 MHz
channel:

- **TDM is mandatory** for any shared-LO / shared-channel TX+RX cluster: only one
  node transmits per slot; others receive. This is the ESPARGOS mode by
  construction — one WiFi transmitter is illuminated and all eight (or 4×8)
  receive antennas capture it.
- **FDM/CDM** require either additional radios per node or a synchronized
  frequency plan (e.g. overlapping 20 MHz channels on 2.4 GHz are not orthogonal;
  only ~3 non-overlapping 20 MHz channels exist). Co-channel simultaneous TX is
  both an interference problem and (prompt 07) a legality problem.
- The **polyhedron + shared-clock** question — whether a co-located volumetric
  array with a common clock removes the TDM requirement for AoA — is prompt 09's
  overlap, but geometry's contribution is: a *co-located* array with one common
  reference makes all faces **coherent by construction**, so a single
  transmission is received on all N faces at once (no TDM across faces), and 3D
  DoA follows from one frame. That is the strongest argument for the polyhedron.

---

## 6. Spherical / volumetric vs planar arrays for 3D DoA

**ESTABLISHED (antenna theory).**
- Spherical arrays have **spherical symmetry → 3D DOA with near-constant
  directivity over the full sphere**, and avoid the scan loss / aperture
  shrinkage / beam broadening that planar arrays suffer off broadside
  (Khaykin & Rafaely, *Coherent signals DOA using a spherical microphone array*,
  IEEE 2009; MDPI Appl. Sci. 13(5):3067, 2023, DOI 10.3390/app13053067).
- **Volumetric arrays** are the M-MIMO answer to the aperture constraint of
  linear arrays (arXiv 1810.01487, *Closed-Form Directivity Expression for
  Arbitrary Volumetric Antenna Arrays*), and give *both azimuth and elevation*
  coverage that a planar array cannot (Procedia CS 2015,
  S1877050915021158, DOI 10.1016/j.procs.2015.08.291).
- **Geodesic / dome phased arrays** are a mature, low-cost realisation of
  hemispherical coverage; the geodesic dome subarray was built and demonstrated
  (Tomasic & Turtle, 2003, IEEE, semanticscholar
  f64ec3c7ed8855ce25ae3cbca7bb82e8a46dc0bf), and modern CubeSat designs use a
  **truncated-icosahedron / geodesic dome** with patch arrays per face
  (DSpace ZČU, 2025, "geodesic dome structure with a truncated equilateral
  triangular patch antenna array").
- **PCA note:** *passive* spherical/volumetric matching is exactly what a
  combinatorially-arranged **soldered-PCB polyhedron** would provide — an
  icosahedron has 20 triangular faces, matching the BRIEF's 20-node 3D aperture.

**ESP32-specific caveats (HYPOTHESIS).**
- **Mutual coupling** is worse, not better, in tightly-packed volumetric arrays
  (mutual-coupling review, researchgate 258392365; near-field MC degrades
  direction-finding, IEEE 10842515). A polyhedron with λ/2 (60 mm) spacing is
  large; a compact one is not. This is a *design* constraint, not a blocker.
- **Face shadowing / panel blockage**: each face's ground plane and the PCB body
  occlude other faces. A single transmission from outside will illuminate
  roughly a hemisphere; faces on the far side see attenuated, scattered signal.
  This is physically the same reason geodesic domes are built as *separate*
  panels and it limits the coherent aperture to the illuminated subset.
- There is **no published ESP32 volumetric-array WiFi-sensing demonstration** I
  could find. Folded/origami PCB arrays exist for CubeSats (PMC9862758,
  PMC11680017) and 3D-folded strip arrays exist at mmWave (CityU 2022), but the
  soldered-PCB-polyhedron *sensing* system is unbuilt.

---

## 7. Hub-face Ethernet-switch / FPGA interconnect (volumetric array wiring)

This is the concrete engineering of an N-face array, and the literature gives a
clean separation: **a data plane and a sync plane are different wires.**

**ESTABLISHED, from ESPARGOS (closest thing to an Aquila-like hub):**
- ESPARGOS One = eight sensor ESP32s + **one central controller ESP32** that
  configures, orchestrates and aggregates all eight, exposing **100BASE-TX
  Ethernet with PoE (802.3at Type 1, Class 3)** and USB-C; CSI streams over UDP
  or WebSocket (ESPARGOS One datasheet/manual; `pyespargos`).
- Multi-board coherence is *not* over Ethernet: it is a **separate SYNC cable**
  star topology (master SYNC OUT → PA → splitter → slave SYNC IN), while all
  boards also connect by Ethernet to a common switch for data
  (`https://espargos.net/docs/combined-arrays.html`; manual Fig. 12/13).
- **No FPGA.** The hub is an ESP32; sync is a conducted analog reference. This
  is the cheap end of the spectrum.

**ESTABLISHED, from SDR/FPGA sounders (the expensive end, for gap quantification):**
- USRP sounders: 10 MHz + PPS to every radio, RFNoC FPGA blocks on a transport
  router, Ethernet to host (arXiv 2201.04471, arXiv 2403.11856).
- 256-antenna D-MIMO: per-panel **ZCU216 FPGA** at 16 DBB chains, 10 MHz fed via
  CLK104 PLLs from a CDA-2990 octoclock tree, timing pulse over
  UFL/SMA daisy-chain with per-node software delay trim; measured channel
  crosstalk **−44.5 dBc** (arXiv 2605.02388).
- RoF TDD D-MIMO: digital up/down-conversion at the central unit *eliminates* the
  LO-sync problem entirely (arXiv 2403.17476).

**Implication for Aquila's hub-face array (HYPOTHESIS).** The design that scales
to a 20-face polyhedron is **Ethernet switch hub for data + conducted 40 MHz
clock and 2.4 GHz phase reference distributed over the faces for sync** — i.e.
the ESPARGOS architecture extended from 8 to 20 faces, possibly with one
controller per face. An **FPGA hub** would add what the ESP32 cannot:
deterministic hardware timestamping/triggering (sub-µs → ns) and a common
trigger for simultaneous *TX* across faces. Without an FPGA, cross-node time
sync is bounded by the ESP32 µs timestamp and by cabled trigger distribution.
Data-plane bandwidth is not the bottleneck: at 20 MHz, 20 nodes × ~52 CSI
subcarriers × complex × frame rate is well within 100BASE-TX; ~1 GbE would be
needed only for high-rate raw-I/Q (prompt 05).

---

## 8. The ping-and-unique-reply round-trip scheme

Scheme: node A pings; every other node replies with a **unique** packet; A times
each round trip.

**ESTABLISHED — this is textbook two-way time transfer / RTT ranging.** Per NIST
(`https://tf.nist.gov/time/twoway.htm`): "the one-way delay between the
transmitter and receiver is estimated as one-half of the measured round trip
delay"; the half-duplex variant "only requires 'reflecting' hardware at one
station, but is susceptible to fluctuations in the delay." The two-way technique
"provides stable and accurate time transfer since nearly all the propagation
delay cancels out due to symmetry."

**What it can measure:**
- **Clock offset** between A and each replier (BDW; timestamp-free BDW is exactly
  the scheme in arXiv 2512.22686 §V-2).
- **Range**: `ToF = RTT_raw/2`, `d = RTT_raw·c/2` where `RTT_raw=(T4−T1)−(T3−T2)`
  (IEEE 802.11mc; ESP32 FTM exposes `rtt_raw` in ns).
- **Coarse Doppler / CFO**: from the carrier-frequency offset on the reply.
- **Identity**: each unique packet carries its sender's MAC/sequence — the
  "unique packet" is free identity.
- **Channel**: every hop yields CSI (or raw-I/Q on ESPARGOS-class nodes), so one
  round-trip exchange yields a *set* of link channels A→i and i→A. Applying
  OTA reciprocity (`Ĥ_Ai·Ĥ_iA⁻¹`) over the same exchange is the natural way to
  cancel per-node RF-chain asymmetry.

**Where it breaks:**
- **Processing/loopback asymmetry** — the single largest error source even for
  TWSTFT: "there is no reason for the transmit and receive delays of the earth
  station equipment to cancel perfectly" (NIST).
- **Multipath bias** — FTM is ~1–2 m LoS, degrading to ~**5 m NLoS**
  (arXiv 2511.17935; DOI 10.1145/3576914.3587520). At 20 MHz ESP32 FTM is coarse.
- **Doppler and diffuse multipath** degrade the two-tone frequency-sync tones
  (arXiv 2512.22686): both tones can fade in deep multipath, and delay spreads
  >100 ns (indoor) raise CFO error.
- **No simultaneous phases** across replies unless time slots are controlled —
  so a "ping" with N replies either serialises (slow, drift accumulates) or
  overlaps (collisions, prompt 09).
- It gives **timing, not carrier-phase coherence** unless the two tones / BDW are
  used specifically for phase (Merlo/Nanzer class).

---

## ESP32-S3 feasibility

| Geometry | ESP32-S3 without added RF | With added RF (switch/circulator + ref) | Feasibility |
|---|---|---|---|
| Passive monostatic (echo, Tx then Rx) | no — half-duplex single radio, MAC | yes via TDM switch | **HYPOTHESIS**, needs external switch |
| True monostatic (full-duplex SI-cancelled) | no | needs circulator + ~40 dB analog SIC (external) | **blocked** on-chip |
| Bistatic (node A Tx, node B Rx) | yes — `esp_wifi_80211_tx` + CSI RX | — | **ESTABLISHED** primitive exists |
| Coherent multistatic | no (independent LO) | yes with conducted clock + phase ref (ESPARGOS-style) | **HYPOTHESIS**, unproven on S3 |
| Incoherent multistatic | yes (TDM, CSI amplitude/Doppler) | — | **ESTABLISHED** |
| Volumetric/polyhedron 3D DoA | no (no common ref) | yes with hub + conducted ref across faces | **SPECULATION** for S3 |

- **CSI capability**: whole ESP32 family; S3 = 20/40 MHz, ~52–114 subcarriers
  (forced L-LTF gives a same-53-subcarrier representation per ESPARGOS docs).
- **I/Q capability**: ESPARGOS/esp-sdr is **ESP32-S2, receive-only, gapped** —
  an Aquila S3 I/Q path is a *port*, not a given.
- **TX**: `esp_wifi_80211_tx` on S3, len 24–1500 B — the geometry menu's enabler.
- **FTM**: S3 supports 802.11mc; ~1–2 m LoS typical, coarse.

---

## TX needed?

**Yes, for every geometry except fully-passive receive-only sensing.** The
geometry taxonomy is *defined* by TX placement: monostatic and bistatic require a
cooperative (or opportunistic) transmitter. `esp_wifi_80211_tx` is the enabling
TX primitive, and it is what converts "one receiver listening" (prompt 01) into a
controlled bistatic/multistatic network. Chaining note: capability 3 (TX) enables
capabilities 2 and 1 to be used as *radar* rather than as *listening*.

---

## Evidence strength: **6 / 10**

Justification: the geometry taxonomy, sync requirements, monostatic SI budgets
and volumetric/geodesic array theory are all **firmly ESTABLISHED**, cited to
radar texts (Skolnik; Willis; Chernyak), a standards body (IEEE 802.11mc), and
peer-reviewed work with measured numbers (77 dB ISAC-Fi SIC; sub-ns TWSTFT;
10 MHz/PPS sounders). ESPARGOS demonstrates *conducted* multi-array coherence on
**ESP32-S2**. However, **nothing here is demonstrated on ESP32-S3 with TX**, no
polyhedron/geodesic WiFi-sensing array has been built, and the ESP32 lacks
on-chip full-duplex SIC. The physics is solid; the S3 instantiation is
hypothesis/speculation. Hence 6, not 8.

---

## Primary sources

1. IEEE Technology Navigator, *Multistatic radar* — https://technav.ieee.org/topic/multistatic-radar/
2. arXiv 2512.22686, *Multistatic Radar Performance in the Presence of Distributed Wireless Synchronization* — https://arxiv.org/html/2512.22686v1
3. Colone et al., *WiFi-based passive bistatic radar: data processing schemes and experimental results*, IEEE TAES 48(2):1061–1079, 2012 — DOI 10.1109/TAES.2012.6178049
4. IET RSN 2016, *Feasibility of multi-carrier modulation signals as... passive radar* — DOI 10.1049/iet-rsn.2015.0414
5. ISAC-Fi, *Enabling Full-fledged Monostatic Sensing over Wi-Fi Communication* — arXiv 2408.09851
6. arXiv 2412.08612, *An SDR-Based Monostatic Wi-Fi System with Analog Self-Interference Cancellation for Sensing* — Dec 2024
7. *Self-Interference-Resistant IEEE 802.11ad-Based Joint Communication and Automotive Radar* — 2021 (researchgate 355319603)
8. ESPARGOS One datasheet / manual / combined-arrays docs — https://espargos.net/docs/combined-arrays.html ; https://espargos.net/espargos-one/
9. ESPARGOS papers — arXiv 2502.09405 ; arXiv 2408.16377 ; https://github.com/ESPARGOS/pyespargos
10. DICHASUS distributed massive MIMO channel sounder — https://dichasus.inue.uni-stuttgart.de/ ; Euchner et al. 2021
11. arXiv 2403.11856 (wideband distributed massive MIMO sounder); arXiv 2605.02388 (256-antenna D-MIMO, ZCU216); arXiv 2403.17476 (RoF TDD D-MIMO); arXiv 2201.04471 (RFNoC)
12. Tomasic & Turtle, *The geodesic dome phased array antenna*, IEEE 2003 — semanticscholar f64ec3c7ed8855ce25ae3cbca7bb82e8a46dc0bf
13. Rose et al. / Khaykin & Rafaely, spherical-array DOA — MDPI Appl. Sci. 13(5):3067, DOI 10.3390/app13053067
14. arXiv 1810.01487, *Closed-Form Directivity Expression for Arbitrary Volumetric Antenna Arrays*
15. Procedia CS 2015, *A Comparative Study of Several Array Geometries for 2D DOA Estimation* — DOI 10.1016/j.procs.2015.08.291
16. NIST, *Two-Way Time Transfer* — https://tf.nist.gov/time/twoway.htm
17. Merlo, Mghabghab & Nanzer, *Wireless picosecond time synchronization for distributed antenna arrays*, IEEE TMTT 71(4):1720–1731, 2023 — DOI 10.1109/TMTT.2022.3227878
18. WiCAL, *Accurate Wi-Fi-Based 3D Localization Enabled by Collaborative Antenna Arrays* — arXiv 2505.21408
19. ESP-IDF `esp_wifi_80211_tx` / Wi-Fi API (ESP32-S3) — https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/api-reference/network/esp_wifi.html
20. FTM benchmark — DOI 10.1145/3576914.3587520; arXiv 2511.17935 (802.11mc vs 802.11az)

---

## Residual unknowns

1. **Residual two-way sync error on ESP32-S3** using a conducted 40 MHz clock +
   2.4 GHz phase reference, measured as an Allan deviation / phase-noise floor.
   Unquantified.
2. Whether a soldered-PCB polyhedron's **mutual coupling and face shadowing**
   leave enough coherent faces to form a useful 3D aperture at 60 mm spacing.
3. Whether `esp_wifi_80211_tx` frames can be time-slotted tightly enough
   (and legally) to serve as a multistatic illumination without collisions.
4. Whether a same-node TDM Tx→Rx echo (quasi-monostatic) can be captured at all
   given MAC turnaround and AGC settling.
5. The **raw-I/Q port** of ESPARGOS (S2) to S3, and its gapped/uncalibrated
   penalty for pulse-Doppler (prompt 05/06).
6. No published ESP32 volumetric-array WiFi-sensing demonstration exists.

---

## (A) Chaining

- **TX (`esp_wifi_80211_tx`) enables every active geometry.** Without controlled
  TX, Aquila is one passive receiver (prompt 01) or a passive network (prompt 02);
  with it, bistatic/monostatic/multistatic radar becomes possible.
- **Conducted clock + phase reference enables coherent multistatic and the
  polyhedron** — the single highest-leverage enabling capability for geometry.
  It converts N independent receivers into one phase-synchronous aperture
  (demonstrated on ESP32-S2/ESPARGOS). It **unlocks** 3D DoA, coherent spatial
  gain, and virtual-large-aperture resolution (WiCAL), which in turn unlock
  channel charting with geometry (prompt 03) and self-localisation (prompt 15).
- **Conducted reference → OTA reciprocity becomes testable** (prompt 10): with a
  known ground-truth reference, `Ĥ_AB·Ĥ_BA⁻¹` can be validated, which would
  unlock *wireless-only* coherence — the biggest prize.
- **Monostatic is blocked without RF hardware**: ESP32-S3 cannot Tx and Rx at
  once; true monostatic needs a circulator + ~40 dB analog SIC that the part does
  not have. Quasi-monostatic TDM depends on the switch (prompt 09).
- **Spectral sharing forces TDM** under one shared LO/channel (prompt 09), which
  caps update rate and interacts with oscillator drift (prompt 12).

Highest-leverage single capability: **a conducted common clock + phase reference
across nodes** (the ESPARGOS escape). It is the hinge that turns geometry from a
non-coherent diversity story into a coherent aperture story, and it is the only
escape already proven on the ESP32 family (S2).

## (B) Existing models and data

- **ESPARGOS datasets** (`espargos-0001..0008`, including `espargos-0002` a
  4-board phase-synchronous 4×8 array with total-station ground truth, and
  `espargos-0005` with four phase- and time-synchronous arrays) — open,
  downloadable, used for channel charting, ray tracing and JCaS.
  Aquila can consume the **input format directly via `pyespargos`** (CSI tensor
  `H ∈ C^{B×2×4×N_sub}`), but the hardware is **ESP32-S2 receive-only** — Aquila
  would need to port to S3 and add TX.
- **DICHASUS** `"Big CSI Data"` channel sounder datasets (DOI per dataset) —
  distributed massive MIMO CSI with positioning labels; licence generally open
  for research.
- **MM-Fi** and other CSI-sensing datasets (prompt 19) are *not* multistatic
  radar geometries but are useful for the ML side.
- **Open-source implementations**: `pyespargos` (ESPARGOS), GNU Radio OFDM
  decoders, IEEE 802.11 OFDM passive-radar reference-channel processing
  (Colone-line). No open multistatic ESP32 radar stack exists.
- **Licence/hardware fit**: ESPARGOS code is open and Python; the *hardware*
  divergence (S2 vs S3, RX-only vs TX) is the real gap, plus Aquila's legal
  obligation under prompt 07.

## (C) Own-data / new technology

**Could a purpose-built Aquila dataset unlock something not currently
demonstrated? Yes — specifically a demonstrated ESP32-S3 multistatic radar and a
volumetric-aperture 3D DoA.**

Minimal decisive experiment (cheapest falsifiable form):

- **E1 — 3-node conducted-reference multistatic radar.** Three ESP32-S3 nodes;
  one TX, two RX; a single conducted 40 MHz clock + 2.4 GHz phase reference
  fanned out by SMA. TX node sends `esp_wifi_80211_tx` frames at a controlled
  rate; RX nodes capture CSI on the direct path and on a moving human. Test
  whether the *cross-node* phase is stable enough to form a coherent bistatic
  range–Doppler map and to localise via ellipsoid intersection.
  - **Proves**: ESP32-S3 can host a coherent multistatic geometry with a
    conducted reference.
  - **Falsifies**: cross-node phase residual > π/2 over the integration time, or
    dead full-duplex due to MAC, or CSI noise floor dominating at room range.

- **E2 — soldered-PCB polyhedron 3D DoA.** Icosahedral 20-face PCB, one S3 per
  face, common conducted reference over the faces; one external TX. Test 3D AoA
  (azimuth+elevation) with MUSIC/spherical-harmonic methods against ground truth
  from a total station, and measure the coherent-face count (how many faces see
  a usable direct path simultaneously).
  - **Proves**: volumetric array gives 3D DoA and full-sphere coverage on S3.
  - **Falsifies**: mutual coupling + face shadowing collapse the usable aperture
    to an effective planar subset, or the 2.4 GHz reference cannot be distributed
    to 20 faces without prohibitive phase unbalance.

- **E3 — residual sync metrology.** The measurement E1/E2 both assume: a
  two-node Allan-deviation / phase-noise characterisation of the conducted
  reference on S3, establishing the coherence time and the multistatic CRLB
  (arXiv 2512.22686 gives the BCRLB template). This is the *decisive* first
  experiment — it tells you whether coherent multistatic is even on the table
  before building a polyhedron.

**Single most decisive next experiment: E3** — quantify ESP32-S3 cross-node
phase/timing residual with a conducted reference. Everything coherent (bi/multi-
static, polyhedron 3D DoA, virtual large aperture) is gated on it.

---

*End of note 08.*
