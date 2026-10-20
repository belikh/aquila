# 04 — Through-wall / NLoS CSI and material inference

*Aquila v2 deep-research note. Reference hardware: ESP32-S3. Australian English.
Primary sources only; ESTABLISHED / HYPOTHESIS / SPECULATION kept separate.*

---

## Question

How do walls and materials attenuate and shape CSI, and what can be inferred
about NLoS geometry and material type? Required content:

- (a) attenuation vs frequency for concrete / plaster / glass / water;
- (b) through-wall range / resolution limits;
- (c) direct vs multipath separation;
- (d) specular vs diffuse modelling.

---

## Findings by epistemic status

### ESTABLISHED

**E1 — Material dielectric parameters are standardised and free to use.**
ITU-R P.2040 gives the real relative permittivity and a frequency-dependent
conductivity fit for building materials, valid 1 MHz–450 GHz (P.2040-2/-3/-4,
2021/2023/2025). Representative values (real part of relative permittivity,
`εr`, with conductivity modelled as `σ = c·f_GHz^d` S/m):

| Material | εr | c | d |
|---|---|---|---|
| Concrete | 5.24 | 0.0462 | 0.7822 |
| Brick | 3.91 | 0.0238 | 0.16 |
| Plasterboard | 2.73 | 0.0085 | 0.9395 |
| Wood | 1.99 | 0.0047 | 1.0718 |
| Glass | 6.31 | 0.0036 | 1.3394 |
| Chipboard | 2.58 | 0.0217 | 0.7800 |
| Ceiling board | 1.48 | 0.0011 | 1.0750 |
| Metal (perfect) | 1 | σ = 10⁷ | — |

Because conductivity rises with frequency, loss through a lossy dielectric
(concrete, plaster) grows with frequency, while the real permittivity is
roughly constant. P.2040 also supplies a layered-slab (ABCD matrix) reflection
and transmission formulation, plus a roughness attenuation term — the canonical
model-based route to predict through-wall amplitude *and phase slope*.

**E2 — Measured one-way attenuation at Wi-Fi bands is small but the round trip
is not.** WiVi (Adib & Katabi, SIGCOMM 2013) reports one-way 2.4 GHz
attenuation: glass 3 dB, 1.75″ solid wood door 6 dB, 6″ hollow wall 9 dB,
18″ concrete 18 dB, reinforced concrete 40 dB. TwSense (MDPI 2023) reports
5 GHz one-way: glass 3 dB, wood door 6 dB, gypsum 13 dB, concrete 15 dB. A
through-wall *reflection* system traverses the wall twice, so the static
"flash" is the two-way figure (18–36 dB typical); a *link_through*` (TX one side,
RX other) pays only one-way attenuation. This distinction is decisive for
Aquila: a cooperative TX–RX pair through a wall loses only ~3–18 dB, whereas a
monostatic/bistatic reflection system loses twice that before it sees any target.

**E3 — Water is strongly lossy at 2.4 GHz.** Pure water follows a single Debye
relaxation: static permittivity εs ≈ 81, relaxation time τ ≈ 8.28 ps
(critical/relaxation frequency ≈ 17–19 GHz at 25 °C); ε∞ ≈ 5.6. At 2.45 GHz the
penetration depth is of order 10 mm. Body tissue is water-dominated, which is
why humans are strong reflectors/absorbers of 2.4 GHz — the physical reason
device-free sensing works at all. Any free water (fish tank, wet wall, rain)
shadows the channel.

**E4 — Commodity Wi-Fi range resolution is metre-scale, not centimetre.** Range
resolution is `Δr = c / (2B)` for two-way (or `c/B` one-way). A 20 MHz Wi-Fi
channel gives ~7.5 m two-way / ~15 m one-way — far too coarse to time-gate wall
from target. **RF-Capture** (Adib et al., SIGGRAPH Asia 2015) needed a 20-antenna
array and **1.78 GHz FMCW bandwidth** to resolve body parts. **TagScan**
(RFID, MobiCom 2017) noted Wi-Fi/RFID bandwidths are far smaller. Therefore
commodity Wi-Fi through-wall systems do **not** separate direct and reflected
paths by time; they use spatial (MIMO nulling / AoA), phase-slope, or model-based
mechanisms instead.

**E5 — Direct/multipath separation is achieved without bandwidth by four
established techniques.**
1. *MIMO nulling* — WiVi precodes two TX antennas so static reflections (wall)
   cancel at the RX: median ~40 dB reduction. Requires ≥2 TX + 1 RX on a shared
   clock (3 USRPs). Fails on >8″ concrete (residual flash + low USRP power,
   ~20 mW vs Wi-Fi's 100 mW).
2. *Super-resolution AoA (MUSIC)* — SpotFi (SIGCOMM 2015) resolves multipath
   AoA with only 3 antennas and identifies the direct path (median 40 cm).
3. *CSI-ratio / conjugate-multiply* — ratio of two antenna CSIs cancels the
   common static `b_k` term, leaving the dynamic/direct-path-behind-wall
   component (CSI-ratio model, and the "differential phase decomposition with
   spatial steering vectors" of WiTWS, IEEE TMC 2026).
4. *Model-based Fresnel* — the **refraction-aware Fresnel zone model**
   (Zhang et al., IMWUT 2023, doi:10.1145/3569494; Ren et al., IEEE TMC 2024,
   doi:10.1109/TMC.2024.3425847) introduces refraction at the air–wall boundary
   and predicts a *squeeze/stretch* of Fresnel zones: because εr(wall) > 1,
   zone spacing becomes uneven, and the squeeze effect *increases* spatial
   resolution through the wall. This is the first model-based NLoS sensing
   theory and needs no training data.

**E6 — Material inference from RF has been demonstrated on COTS/SDR/RFID, with
polarisation as the key discriminator.**
- **IntuWition** (MobiCom 2019, doi:10.1145/3300061.3345442): commodity Wi-Fi,
  senses complex reflectivity via polarisation; 95% LoS / 92% NLoS accuracy
  classifying copper, aluminium, plywood, birch, human.
- **Wi-Painter**: model-driven permittivity estimation from V/H polarised power
  ratios across incidence angles — deliberately avoids unreliable Wi-Fi phase.
- **TagScan** (MobiCom 2017, doi:10.1145/3117811.3117830): one antenna, 4 MHz
  RFID, >94% for 10 liquids, >91% for 6 solids, through a wall.
- **TwSense** (MDPI Appl. Sci. 2023): OR-PCA + Doppler discriminates wall
  material classes; accuracy falls concrete (93.6%) < gypsum (94.8%) <
  door (96.5%) < glass (97.3%).
The recurring physics: amplitude ratio ↔ loss tangent / attenuation; phase
difference and polarised reflectivity ↔ complex permittivity.

**E7 — Specular modelling is adequate at 2.4–5.8 GHz; diffuse needs a
directive lobe.** Smooth-surface specular reflection (Snell, Fresnel) works
well at these bands; diffuse scattering is modelled with a single-lobe
directive model, scattering exponent α ≈ 1–5, and a Rayleigh roughness factor.
Relevant because a wall gives a strong specular "flash" that must be nulled,
while rough/heterogeneous interior scatter produces the diffuse multipath that
carries the human signature.

**E8 — Espressif's own ranking places ESP32-S3 below C5/C6 for CSI, and the S3
is 1T1R.** Per Espressif ESP-CSI documentation: CSI performance ranking
ESP32-C5 > ESP32-C6 > ESP32-C3 ≈ **ESP32-S3** > ESP32. ESP32-S3 is 1T1R,
802.11b/g/n, 20/40 MHz, 64 subcarriers per 20 MHz channel; antenna diversity is
supported but at most **two** antennas can be selected at once. It supports
802.11mc FTM and hardware TSF. This bounds any through-wall array work.

### HYPOTHESIS

**H1 — Through-wall presence and activity recognition on ESP32-S3 is feasible
and is the only through-wall capability demonstrated on the reference chip.**
Strohmayer & Kampel (Springer, 2023, doi:10.1007/978-3-031-44137-0_4) built an
ESP32-S3 + 2.4 GHz directional biquad link spanning **18.5 m across five
rooms**, trained CNN HAR on CSI spectrograms, and published 392 LoS + 384 NLoS
spectrograms. A follow-up evaluates four ESP32-S3 antenna systems and reports
NLOS accuracy 92.0 ± 3.5% (biquad) and 86.8 ± 4.7% (PIFA + plane reflector).
This is a *link-through-wall* configuration (one-way attenuation), not a
reflection-imaging system. It establishes presence/motion but **not** geometry
or material inference on the S3.

**H2 — Material inference on ESP32-S3 is a reasonable extension but is
undemonstrated.** Given E6, the necessary observables (per-subcarrier amplitude
ratio and phase difference across two antennas, or across two polarisations)
are within ESP32 CSI's grasp (64 subcarriers, phase obtainable with
conjugate-multiply). Whether the S3's hardware AGC, 8-bit-ish effective CSI
resolution, single radio, and lack of a **known static reference** are enough
to separate a weak material signature from link drift and multipath is an open
empirical question. This is the central Aquila hypothesis for prompt 04.

**H3 — The refraction-aware Fresnel squeeze effect may be exploitable as a
passive geometry cue.** If wall εr and incidence geometry are known or
estimated, the zone compress/stretch predicts how a moving target's CSI
signature scales with distance and wall thickness — a model-based route to NLoS
geometry that avoids training. Not yet tested on ESP32 CSI.

### SPECULATION

**S1 — A coherent multistatic ESP32 aperture (common-TX reference + the
soldered-PCB polyhedron) could perform crude through-wall reflection imaging
without UWB**, trading bandwidth for aperture: 3D DoA from a volumetric array
substitutes for range resolution, following the WiVi "treat motion as an
antenna" trick but with a real coherent array. Every prerequisite (common
reference, phase coherence, wide aperture) is unproven on ESP32-S3 (see
prompt 20). Treat as a direction, not a claim.

**S2 — Unique materials may be identifiable from the *frequency shape* of
attenuation alone.** Because σ(f) slopes differ per material (P.2040 d-exponent
0.78–1.34), a 20 MHz ESP32 CSI amplitude-vs-subcarrier curve might carry a
material slope signature. 20 MHz gives ~0.6% frequency span at 2.43 GHz — very
weak leverage. Speculative.

---

## ESP32-S3 feasibility

| Capability | ESP32-S3 status | Evidence |
|---|---|---|
| Through-wall presence / motion (link-through) | **Demonstrated** (18.5 m, 5 rooms; ~87–92% NLoS HAR) | E8, H1 |
| Through-wall activity classification | **Demonstrated** (CNN on CSI spectrograms) | H1 |
| Wall-material classification | **Not demonstrated on S3**; demonstrated on Intel 5300/USRP/RFID | E6, H2 |
| Through-wall target *imaging* (skeleton) | **Blocked** — needs ~1 GHz+ bandwidth or a large coherent aperture | E4 |
| Direct/multipath separation by time gating | **Blocked** — 20 MHz → ~15 m one-way resolution | E4 |
| Direct/multipath separation by nulling | **Blocked** — S3 is 1T1R, ≤2 antennas simultaneous | E5, E8 |
| Direct/multipath separation by CSI ratio / AoA | **Plausible** — 2 antennas + CSI phase (conjugate-multiply) | E5, E6 |
| Material ID via polarisation ratio | **Plausible** — needs 2 polarisations and stable phase | E6, H2 |
| Model-based NLoS geometry (refraction Fresnel) | **Plausible** — pure model, needs geometry × CSI | E5, H3 |

**Quantified hardware gap.** WiVi/RF-Capture ran on USRP SDRs with an external
shared clock (3 USRPs as one MIMO system), 2 TX chains, directional 6 dBi
antennas, and (RF-Capture) 1.78 GHz FMCW. ESP32-S3: 1T1R, ≤2 antennas, 20 MHz,
100 mW, no independent RX chains and — critically for Aquila — no common LO
across nodes. The gap is a shared reference and bandwidth, not raw sensitivity.

---

## TX needed?

Mixed, and the split matters:

- **Passive CSI alone is sufficient** for through-wall *presence, motion and
  activity* by perturbing an existing link (the demonstrated capability).
- **Material inference can be passive** (observe an existing AP↔node link
  through a wall; ratio amplitudes/phases) — but a *controlled* TX gives known
  geometry and higher SNR, and polarisation probing requires it.
- **Active/compliant TX is required** for: monostatic/bistatic reflection
  sensing, MIMO nulling (2 TX), FMCW-like ranging, and any coherent multistatic
  aperture. `esp_wifi_80211_tx` (prompt 07) enables controlled frames but on S3
  cannot supply two independent TX chains for nulling.
- **A shared/common reference is required** for any cross-node coherent
  through-wall aperture — the binding constraint, not the TX itself.

---

## Evidence strength

**7 / 10.**

- Strong (9–10): material dielectric constants and attenuation (ITU-R P.2040 +
  multiple measured tables agree); water Debye physics; bandwidth→resolution
  arithmetic; direct/multipath techniques on COTS/SDR.
- Moderate (6–7): ESP32-S3 through-wall *presence/HAR* (one primary + a
  follow-up, public dataset, but single research group; link-through only).
- Weak (3–4): material inference specifically on ESP32-S3; the Fresnel squeeze
  effect as an Aquila geometry cue; any coherent-aperture through-wall imaging.
The overall 7 reflects that the *physics and models* are solid and free, while
the *specific ESP32-S3 material/geometry claims* rest on extrapolation.

---

## Primary sources

1. ITU-R P.2040-2 (2021) / P.2040-4 (2025), *Effects of building materials and
   structures on radiowave propagation.*
   https://www.itu.int/rec/R-REC-P.2040/en
2. Adib & Katabi, "See Through Walls with Wi-Fi!" ACM SIGCOMM 2013.
   doi:10.1145/2486001.2486039 · https://people.csail.mit.edu/fadel/papers/wivi-paper.pdf
3. Adib, Hsu, Mao, Katabi, Durand, "Capturing the Human Figure Through a Wall"
   (RF-Capture). ACM SIGGRAPH Asia 2015 / TOG 34(6):219.
   https://rfcapture.csail.mit.edu/
4. Kotaru, Joshi, Bharadia, Katti, "SpotFi: Decimeter Level Localization Using
   WiFi." ACM SIGCOMM 2015. doi:10.1145/2829988.2787487
5. Strohmayer & Kampel, "WiFi CSI-Based Long-Range Through-Wall Human Activity
   Recognition with the ESP32." 2023. doi:10.1007/978-3-031-44137-0_4
   (ESP32-S3, 18.5 m, 5 rooms; public dataset.)
6. Zhang et al., "Understanding the Mechanism of Through-Wall Wireless Sensing:
   A Model-based Perspective." IMWUT 6(4), 2023. doi:10.1145/3569494
7. Ren et al., "Characterizing the Through-Wall Sensing Mechanism of Wi-Fi
   Signals With a Refraction-Aware Fresnel Zone Model." IEEE TMC 23(12):13438,
   2024. doi:10.1109/TMC.2024.3425847
8. "WiTWS: WiFi CSI Path Separation for Calibration-Free Through-Wall HAR."
   IEEE TMC 25, 2026. doi (IEEE Xplore 11478343).
9. "On the Feasibility of Wi-Fi Based Material Sensing" (IntuWition). MobiCom
   2019. doi:10.1145/3300061.3345442
10. Wang et al., "TagScan." MobiCom 2017. doi:10.1145/3117811.3117830
11. "TwSense: Highly Robust Through-the-Wall Human Detection…" Appl. Sci.
    13(17):9668, 2023. doi:10.3390/app13179668
12. "Wi-Painter" and "A Material Identification Approach Based on Wi-Fi Signal,"
    CMC 69(3), 2021. doi:10.32604/cmc.2021.018835
13. Liebe, Hufford, Manabe, "A Model for the Complex Permittivity of Water…"
    NTIA 1991. https://its.ntia.gov/publications/download/LiebeHuffordManabe1991.pdf
14. Espressif ESP-CSI solution docs (chip ranking; S3 1T1R).
    https://docs.espressif.com/projects/esp-techpedia/en/latest/esp-friends/solution-introduction/esp-csi/esp-csi-solution.html
15. "Through-the-Wall Imaging Exploiting 2.4 GHz Commodity Wi-Fi."
    arXiv:1903.03895 (20 MHz, FBP, ~10 cm cross-range under synthetic aperture).
16. Chetty et al., "Through-the-wall sensing of personnel using passive bistatic
    WiFi radar." IEEE TGRS 50(4):1218, 2012.

---

## Residual unknowns

- Does ESP32-S3 CSI amplitude/phase carry an *interpretable* material signature
  at fixed geometry, or does per-frame AGC and SFO/CFO noise swamp it?
- What is the achieved two-way nulling depth on an ESP32-S3 (single radio, no
  2-TX precoding) — likely near zero; unmeasured.
- Quantified Fresnel squeeze/stretch on real ESP32 CSI and whether it survives
  8-bit-ish amplitude quantisation.
- Whether a passive S3 link can distinguish *stationary* human presence through
  a wall (Wi-FiLeaks suggests yes on commodity mobile, but robustness on S3 is
  unknown).
- Wall heterogeneity (mixed wall+window in the beam) — WiVi explicitly flags
  indirect paths exceeding the direct path in this case.
- Absolute vs relative: no through-wall result on ESP32 gives absolute target
  geometry or material class without site-specific training.

---

## (A) Chaining

- **Compliant TX (`esp_wifi_80211_tx`) → controlled link/monostatic probing.**
  Passive CSI enables through-wall *presence/HAR* alone; controlled TX is needed
  for reflection sensing, polarisation probing, and known geometry.
- **Common-transmitter reference (prompt 11) → cross-node coherent aperture →
  multistatic through-wall sensing.** This is the highest-leverage enabler and
  the binding gap. Without it, no coherent polyhedron aperture, no nulling, no
  distributed reflection imaging.
- **A common reference is blocked on ESP32-S3 by independent LOs** (none of the
  wireless-only escapes is yet proven on S3); therefore through-wall *imaging*
  via an ESP32 aperture is **blocked without a shared reference**, whereas
  through-wall *detection/classification* is not.
- **CSI-ratio / conjugate-multiply + 2-antenna AoA (prompt 13)** enables
  direct-vs-multipath separation *without* bandwidth or a common reference —
  the pragmatic S3 route, and it unlocks the NLoS geometry cue of H3.
- **The soldered-PCB polyhedron** only pays off as a through-wall *aperture* if
  phase-coherent; chained after a common reference, it could substitute aperture
  for the bandwidth Aquila lacks. Otherwise it is limited to RSSI-level diversity.

Order of leverage: **common reference ≫ compliant TX ≈ CSI-ratio multipath
decomposition > polarisation probe > polyhedron aperture.**

## (B) Existing models and data

| Resource | Type | Aquila usability |
|---|---|---|
| ITU-R P.2040 | Model / material params | Free, direct — use for attenuation/phase prediction |
| Refraction-aware Fresnel (Zhang/Ren) | Model-based | Free, no training — use for NLoS geometry cue |
| CSI-ratio model | Method | Directly applicable to S3 2-antenna CSI |
| ESP32-S3 through-wall HAR dataset (Strohmayer 2023) | Public CSI dataset | Usable — same chip; LoS/NLOS spectrograms |
| WiTWS | CNN/LSTFFN + path separation | Method useful; code availability unknown |
| IntuWition / TagScan / Wi-Painter | Material-ID methods | Method useful; trained on Intel 5300/USRP/RFID, **not** S3 |
| WiVi / RF-Capture | SDR (USRP) demonstrations | Not portable to S3 hardware; methods need BW/TX chains S3 lacks |

## (C) Own-data / new technology

**Minimal decisive experiment.** Two ESP32-S3 nodes with directional 2.4 GHz
antennas on opposite sides of a wall (link-through, one-way attenuation). Hold
geometry fixed; interpose a catalogued series of panels — drywall, plywood,
brick veneer, glass pane, and a water-filled container — singly and in
thickness steps. For each, capture 64-subcarrier CSI amplitude **and**
conjugate-multiplied phase at many frames, both polarisations if an antenna
switch permits.

*Prove:* can a fixed-geometry classifier recover material class and monotonic
thickness from CSI alone, and does the recovered amplitude slope match
ITU-R P.2040 predictions? *Falsify:* if per-material CSI distributions do not
separate beyond chance once AGC drift and thermal drift are subtracted — or if
the signature changes with node position/incidence angle more than with material
— the passive material route fails on S3.

**New technology unlocked.** (1) A purpose-built **ESP32-S3 through-wall
material/attenuation dataset with ground-truth geometry and material labels**
does not exist publicly — it would be the first on the reference chip and would
let the community test H2/H3. (2) The **ping-and-unique-reply round-trip
scheme**: one node pings, every other replies with a uniquely addressed packet,
round trips timed. It can measure round-trip **range** (two-way timing cancels
the per-node clock offset), **Doppler** (phase rate across successive rounds,
moving targets only), **identity** (unique address), and the per-link **channel**.
It breaks on inter-node clock-offset/frequency drift, TX turnaround-latency
jitter at the µs scale, multipath, wall attenuation dropping SNR below
detection, and reply collisions as node count grows — but it is the natural way
to *probe many through-wall angles* cheaply, which is exactly the data H3 needs.
