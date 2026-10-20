# 22 — The invention: the Coherent Star

**Prompt (22-invention-t).** Discover the new invention that makes a home full of
cheap ESP32-S3 nodes behave as one sharp, self-calibrating, generalising 3D
sensor — or prove that no such invention exists and state the exact physical
boundary that blocks it. Hardware premise: a soldered polyhedron of triangular
PCBs, ~30 cm diameter, one ESP32-S3 per face, a hub face with a shared 40 MHz
clock, a conducted 2.4 GHz phase reference, and data aggregation. Search:
(1) wireless coherence on commodity silicon; (2) self-calibrating learned array;
(3) environment as shared reference; (4) sharp 3D from many coarse apertures;
(5) ping + unique replies + two-way products; (6) new excitation/composition.

Australian English. Claims labelled **ESTABLISHED / HYPOTHESIS / SPECULATION**;
hardware tier per claim (**ESP32-S3 / ESP32 family / other COTS Wi-Fi /
SDR-grade**). Sources are primaries (papers, repos, docs, datasheets). This note
depends on notes 01–20 and `00-synthesis.md`; it does not upgrade their grades.
Research-only: several terms here (phase-coherent RF reference, ESP-NOW as
excitation, Doppler) are deliberately outside `CONTEXT.md`'s v1 vocabulary — see
§13.

---

## 0. Verdict

**An invention exists, and it is a composition invention, not new physics.**

The physics boundary is real and unchanged: with independent local oscillators
(LOs) and no common reference, cross-node carrier phase sits behind a `U(1)`
gauge and only three escapes cross it — a common-transmitter reference, over-the-
air (OTA) reciprocity, or a shared/conducted reference (`20`, ESTABLISHED). What
is *not* yet realised is the observation that the two escapes can be **composed
into a single low-cost system in which the escape is free, continuous and
multi-purpose**:

> **The Coherent Star.** A conducted-coherent 20-face polyhedron broadcasts
> reference frames; every cheap node in the home re-fixes its LO phase against
> them **per received frame** (common-transmitter escape, O(1) airtime, no mesh);
> the same polyhedron, because it *is* a 3D coherent aperture, simultaneously
> measures each node's angle, range and hardware fingerprint from the same
> traffic; and the resulting house-scale coherent fleet is operated as a **radio
> interferometer** — closure phases, phase referencing and self-calibration —
> so the home calibrates itself by *solving* its physics rather than by learning
> each home.

The space is **not closed by physics**. Four doors remain, each with a named
quantity that gates it and a named minimal reference that crosses it (§5). The
single decisive measurement is the **per-frame phase-transfer residual** (§7,
E1): plausible at ≤ ~10° per millisecond from the best current evidence, but
not yet demonstrated on ESP32-S3.

**Evidence strength: 6/10.** Highest for the constituent physics and for the
conducted-coherent array (demonstrated, productised); lowest for the wireless
per-frame transfer on S3 (one unrefereed open-source claim) and for cross-channel
phase stitching on ESP32 (untested).

---

## 1. The invention, component by component

| # | Component | Role | Status of the physics | Prior art closest to it |
| - | --- | --- | --- | --- |
| M1 | **The Star**: 20-face soldered polyhedron; hub distributes 40 MHz + 2.4 GHz; per-channel reference frames radiated from the hub | Internally coherent 3D aperture; phase oracle; illuminator; ruler | Conducted escape ESTABLISHED on ESP32-S2 (ESPARGOS); 3D ESP32 instance HYPOTHESIS | ESPARGOS One (planar 2×4), esp-crab (2 chips, C5) |
| M2 | **Per-frame common-transmitter referencing**: reference frame + sensing frame ~1 ms apart; every node re-fixes phase each frame | Makes the fleet coherent every snapshot, O(1) broadcasts, no mesh, no two-way | Common-TX escape ESTABLISHED; ESP32 implementation CLAIMED (ESP-PPB) | ESP-PPB reference frames; BeamSync (AP↔AP, no fleet) |
| M3 | **Two-way ratio/product algebra**: ping + unique replies; `r = ĥ_ab/ĥ_ba` and `p = ĥ_ab·ĥ_ba` | Separates LO phase, hardware asymmetry, channel; doubles phase sensitivity | ESTABLISHED (TDD reciprocity calibration; BeamSync) | Rogalin/Argos; Ganesan BeamSync; Dhawahir two-way CSI |
| M4 | **Interferometric self-calibration**: cycle products = closure phases; static paths as phase references; alternating solve | Removes node phases entirely from geometry constraints; makes calibration/survey the same operation | Closure phase ESTABLISHED in radio astronomy (Jennison 1958); not observed in Wi-Fi sensing | Jennison/self-cal; LoSRef, SHARP (reference paths); Fascista (coherent arrays, assumed sync) |
| M5 | **Near-field carrier-phase 3D**: fleet as one very-near-field aperture; curvature + carrier phase give range and angle | Sharp 3D at 20 MHz without bandwidth | Near-field localisation ESTABLISHED (array processing; UWB; RIS lens); house-scale Wi-Fi instance HYPOTHESIS | Fascista JSTSP 2025 (theory/sim); Abu-Shaban RIS lens; Webber 1979 |
| M6 | **Coherent channel stitching**: 13 × 20 MHz → ~83 MHz synthetic band, absolute phase anchored by M1 | 1.8 m unambiguous range; FTM resolves ambiguity → cm-level absolute ranges | Stitching ESTABLISHED on Intel 9260 (Zubow); per-channel offsets semi-deterministic; ESP32 cross-channel absolute phase HYPOTHESIS | Zubow 2005.03755; ESPARGOS per-channel calibration |
| M7 | **Per-home solve-don't-learn calibration**: factor graph over node geometry, clock state, hardware ratios, static map, targets | Generalises across homes by solving each home; physics priors fixed, learned parts optional | Components ESTABLISHED (factor-graph SLAM, atomic-norm self-cal); assembly SPECULATION | Fascista 3-step ML; WiSLAT; SparseLift; note 19 |

The invention is the *composition plus three specific tricks* — (i) the
reference frame radiated from a **phase-calibrated aperture** rather than a point
source, (ii) the **dual use** of every calibration exchange (gauge fix + AoA +
range + hardware fingerprint), and (iii) **closure-phase** treatment of the
sensing graph. Each is argued new or newly-composed in §3.

---

## 2. Mechanism, with numbers

### 2.1 The gauge, and why the Star makes it cheap

The observation model (`20`, ESTABLISHED) is
`ĥ_ab(k,t) = g_ab(k)·a_a·b_b·e^{j(θ_a(t)−ψ_b(t))}·e^{−j2πf_k τ_ab}`.
Only `θ_a − ψ_b` is observable; on a connected graph the node phases are fixed
only up to one global phase. **Escape 1 (common transmitter):** if nodes `m, n`
both receive one packet from node `s`, the product `ĥ_sm·ĥ_sn*` cancels `θ_s`
and exposes `ψ_m − ψ_n` relative to the channel — the fleet is mutually coherent
*per received packet*. **Escape 3 (conducted):** the Star removes its own gauge
entirely (ESPARGOS principle, `arXiv:2502.09405`/`2408.16377`: shared 40 MHz +
~2.4 GHz phase reference; PLL phase re-randomises per boot/channel, so a
*periodic* phase calibration against a reference signal is mandatory; a radiated
reference packet is what receivers calibrate against). **Escape 2 (two-way):**
bidirectional measurements yield `ϕ_a − ϕ_b` directly (BeamSync, Rogalin,
`arXiv:2311.11070`, `1310.7001`; best-case CRB at star topology, `1709.07206`).

The design inversion relative to the v2 synthesis: **the fleet's coherence is
maintained by escape 1 (free broadcasts), not escape 2 (O(N) exchanges).**
Escape 2 is demoted to a slow dirtier job — hardware fingerprints and the
Star's own TX-chain calibration — because hardware chain coefficients drift
< 2.6 % over 4 h (`10`) while LO phase must be re-fixed at the coherence cadence
(`12`: raw wrap 5–10 µs; post-CFO residual ~10–100 Hz ⇒ ms–tens of ms).

### 2.2 The crux quantity: per-frame phase-transfer residual

Protocol per node per snapshot (ESP-PPB structure, HYPOTHESIS on S3):
`[reference frame] → ~1 ms → [sensing frame(s)]`. The node measures its own
phase in both and uses the difference to remove its own drift; because all nodes
share the reference packet, they are also coherent with one another *at the
reference instant*; the residual for the sensing frame is governed by the
post-CFO residual over `τ_ref`:

- `σ_φ(1 ms) ≈ 2π·Δf_res·τ_ref`: **3.6° at 10 Hz residual, 36° at 100 Hz.**
- Modified CRB floor: `σ_φ ≳ 1/√(2·N_s·ρ)` (`20`), degraded by oscillator phase
  noise (−120…−80 dBc/Hz, `arXiv:2509.03722`).
- ESP-PPB claims `< 1°` best case, `< 10°` typical single-frame on ESP32-C3 with
  a VCTCXO disciplined by FTM (`github.com/jonathanmuller/esp-ppb`; **unrefereed
  implementation claim**, pre-production, GPL-3.0).

Everything downstream scales with this one number. At 10° (λ/36 ≈ 3.4 mm path)
the fleet is a carrier-phase instrument; at 60–180° it is an amplitude/Doppler
instrument only.

### 2.3 Sharp 3D from a compact 3D aperture and a distributed one

- **The Star alone** (D = 0.30 m, λ ≈ 12.4 cm): classical resolution `λ/D ≈ 24°`;
  Rayleigh distance `2D²/λ ≈ 1.45 m`, so targets indoors are in its radiative
  near field up to ~1.5 m, where curvature gives range. Curvature phase across
  the half-aperture (`πρ²/(λr)`, ρ = 0.15 m) is 0.57 rad at 1 m → single-puck 3D
  of a scatterer within a few metres is well-conditioned (HYPOTHESIS).
- **The coherent fleet** (D = 5–10 m): Rayleigh distance `2D²/λ` = 0.4–1.6 km,
  so *every* target in a home is deep in the near field — the plane-wave model is
  invalid and curvature is enormous (e.g. edge–centre path difference 1.0 m at
  r = 3 m for D = 5 m). Classical resolution `λ/D` = 1.4° (5 m), 0.7° (10 m).
  Noise-only precision from phase error `σ_φ` across effective baseline `D` at
  range `r`: `σ_x ≈ r·σ_φ·λ/(2πD)` ⇒ **~2 mm at r = 3 m with `σ_φ = 10°` and
  D = 5 m**; range from curvature `σ_r ≈ r²·λ·σ_φ/(2πρ²√N)`-scale ⇒
  **~1 cm at r = 3 m**. These are CRB-class numbers; the practical floor is
  multipath and calibration, not thermal noise (HYPOTHESIS).
- **TDM-MIMO virtual array**: faces transmit in calibrated slots; all faces and
  fleet nodes receive — up to 20 × (19 + fleet) bistatic channels per cycle,
  with TX phases tied across slots by M1 (TDM mandatory on S3, `09`; virtual
  array = established radar practice).
- **Polarisation**: faces of a polyhedron present differently oriented
  polarisation projections; diversely-polarised direction finding is established
  (phase-interferometer DF with polarisation diversity). The Star is thus a
  full-sphere polarisation-aware aperture with no scan loss (HYPOTHESIS for the
  composite structure; mutual coupling/shadowing on a conducting solid is
  unmeasured — `00-synthesis` §7).

### 2.4 Ranging without (much) bandwidth

20 MHz gives 7.5 m resolution; the subcarrier phase slope gives fine but
ambiguous delay (SpotFi-class, ESTABLISHED). M6 stitches the 13 overlapping
2.4 GHz channels into ~83 MHz **with absolute phase** (Zubow: on Intel 9260 the
per-channel offsets are semi-deterministic — up to four values, smooth in
frequency — and correctable on the fly; ESTABLISHED on that NIC). For the Star,
the hub radiates a reference frame **on each channel**, giving every face a
per-channel phase table (ESPARGOS's calibration generalised across channels —
HYPOTHESIS). Then: ~83 MHz synthetic band ⇒ 1.8 m unambiguous range; FTM (1–2 m,
`15`) resolves the ambiguity chain; the stitched carrier phase supplies the
centimetre-level remainder. The measurably falsifiable seam test is E4 (§7).

### 2.5 The two-way algebra (ping + unique replies)

For a reciprocal TDD pair with one LO per node (`20`; ESTABLISHED identity):

- **Ratio** `r = ĥ_ab/ĥ_ba = C_ab·e^{j2(θ_a−θ_b)}` — channel cancels, hardware
  `C_ab = a_a b_b / a_b b_a` (stable, `< 2.6 %/4 h`) and **twice the LO phase**.
- **Product** `p = ĥ_ab·ĥ_ba = C'_ab·h_ab·h_ba` — LO cancels entirely; leftover
  is hardware × the **two-way channel**, i.e. a *virtual monostatic echo* with
  doubled propagation phase (`e^{−j4πfτ}`) and doubled Doppler sensitivity.
- **Temporal product** `p(t₂)·p*(t₁) = (Δh)²·(stable)` — **gauge-free,
  hardware-free environmental change** at 2× phase sensitivity; usable even if
  every coherence assumption fails. This is the honest common denominator for
  the whole architecture.

With ≥3 mutually coherent faces of the Star, each reply also yields the node's
**AoA at the Star** (common-TX within the Star frame) and a two-way range — so
**calibration traffic and self-survey traffic are the same frames.** No prior
work found combines a coherent multi-antenna anchor with fleet calibration in
this dual-use way (closest: `arXiv:2603.19903`, repeater-assisted OTA sync, one
repeater, no aperture and no sensing role; BeamSync: AP↔AP only).

### 2.6 Interferometry: closure phases, phase referencing, self-cal

Radio astronomy solved the same gauge problem 68 years ago. **Closure phase**
(Jennison 1958; arXiv:2012.05254): the phase of a closed-loop product over a
triangle of baselines is invariant to all element-based phase corruption; it is
the basis of **self-calibration** (Schwab 1980; Cornwell & Wilkinson 1981;
Pearson & Readhead 1984). Mapped to a Wi-Fi sensing graph: for a directed cycle
each node appears once as transmitter and once as receiver, so **every
time-varying LO phase cancels in the cycle product**, leaving the channel loop
(`2πf` times the sum of path delays + multipath) plus slowly-varying chain-cycle
terms. Consequences (HYPOTHESIS, cleanly falsifiable):

1. **Gauge-free geometry**: cycle products give range-sum constraints usable
   without any reference — a "gauge-free GPS" for node self-survey, over-
   determined by the graph's fundamental cycles.
2. **Gauge-free NLoS/change detection**: cycle-phase drift flags unstable paths.
3. **Phase referencing**: static paths (LoSRef, `arXiv:2602.05344`; SHARP,
   `2402.09048`) and *virtual anchors* (fixed wall reflections; multipath SLAM,
   `MDPI 13/9/1625`) are calibrator sources; the Star is the primary calibrator.
4. **Self-cal**: alternate between target reconstruction and node-phase solution
   (Fascista's 3-step ML is the sub-6 GHz coherent-array analogue,
   `10.1109/JSTSP.2025.3533111`).

To our searches, closure-phase/self-cal has not been applied to a Wi-Fi sensing
network. The mathematics is established; its transfer is the framing invention.

### 2.7 Excitation and composition (direction 6)

- **TDM MIMO across faces** (calibrated slots) + **coherent TX within the
  Star** (per-channel phase table) — feasible; full-duplex monostatic on one
  radio is blocked (~110 dB SIC, `08`/`09`), but ISAC-Fi (`2408.09851`)
  demonstrates 2.4 GHz monostatic sensing with an external direct-IQ modulator
  and a 40 MHz clock board, i.e. a non-S3 option for a monostatic face.
- **Passive illumination variant**: an ambient AP is the common transmitter
  (escape 1); the Star is a wireless *passive* coherent reference array +
  calibrator (ESPARGOS is passive by design). Snapshot coherence holds for any
  ambient packet; between packets, static-path referencing and the Star's own
  coherence carry holdover. No TX, weaker calibration (HYPOTHESIS).
- **Synthetic aperture by switching**: channel hopping with the M1 phase table
  (M6) is the practical form; mechanical switching is unnecessary.

---

## 3. What is genuinely new (with closest prior art)

| Claim | Nearest prior art | Novelty judgement |
| --- | --- | --- |
| First carrier-phase-coherent **volumetric** ESP32 aperture (20 faces, full-sphere, polarisation-aware) | ESPARGOS One is planar 2×4 (productised); geodesic arrays exist in radar | Instance-first; low scientific novelty, high enabling value |
| Reference frames radiated from a **phase-calibrated multi-face aperture**, re-fixing a fleet **per frame** | ESP-PPB (point-source reference frames, C3); ESPARGOS (wired array, no fleet) | New composition; the *fleet* half is an unrefereed claim |
| Calibration exchange **doubles as AoA/range/hardware survey** ("calibration = survey") | Fascista assumes calibrated APs; WiSLAT jointly estimates stations+target without phase; repeater-sync has no aperture | Believed new |
| **Closure-phase / self-cal interferometry** for a Wi-Fi sensing network | Established in astronomy since 1958; absent from the wireless-sensing literature we could find | New transfer; strong grounding |
| **Absolute cross-channel phase** from a conducted reference (83 MHz synthetic band, FTM-resolved) | Zubow stitched channels on Intel with a NIC-specific correction; ESPARGOS calibrates per channel within one band | New on ESP32; hinges on E4 |
| Two-way **product** used as a LO-free, hardware-free *sensing* observable in a fleet protocol | Reciprocity calibration (comm); two-way CSI deep learning (Dhawahir 2026); MovISAC (async coherent imaging of moving targets) | New as a protocol primitive in this architecture |
| Per-home **solve-don't-learn** calibration graph with physics priors | Factor-graph SLAM, atomic-norm self-cal, Fascista, channel charting (`03`) | New assembly; components established |
| *Not new*: the three escapes; the coherence principle; near-field localisation; channel stitching per se; TDM-MIMO virtual arrays; static-path referencing; multipath SLAM; two-way calibration theory | — | — |

---

## 4. What must be true (assumption ledger)

A1. Per-frame phase-transfer residual ≤ ~10° at `τ_ref ≈ 1 ms` and ≥ 50 Hz
cadence on S3 (measured cross-node, not just cross-frame). **Gate: E1.**
A2. The Star's conducted reference survives a soldered 3D geometry: per-channel
phase tables stable; mutual coupling/shadowing calibratable (unknown; `00` §7).
A3. Cross-channel absolute phase is achievable with hub-fed reference frames
(⇒ M6). **Gate: E4.**
A4. The MAC permits interleaved reference/sensing frames at ≥ 50 Hz with the
nanosecond RX timestamp (`17`) and without power-save TSF corruption.
A5. Static paths suffice as holdover references between frames in lived-in
homes; the per-home baseline/furniture changes are handled by M7 re-solving.
A6. Legal: only compliant 802.11 frames (`07`); reference frames and pings are
ordinary management/data frames; duty stays within LIPD/Part 15.
A7. If the fleet must also transmit (not just receive), its TX chains are
calibrated by M2/M3 at slow cadence (not per frame).

---

## 5. The physical boundary (open, but named)

The space is **not closed**; each door has a quantity and a minimal reference:

| Door | Governing quantity | Crossing reference | Residual after crossing |
| --- | --- | --- | --- |
| **Gauge** (cross-node carrier phase) | No common `θ`: unobservable; raw wrap 5–10 µs; post-CFO residual 10–100 Hz | Conducted reference (Star) or one common-TX packet | Per-frame transfer residual (A1); phase-noise floor |
| **Resolution** (true imaging) | Time-bandwidth product: 20 MHz ⇒ 7.5 m; 83 MHz stitched ⇒ 1.8 m; human is not a point target | Coherent channel stitching (M6); wider bands (C5/C6/C7 at 5–6 GHz) | Localisation of coherent scattering centres is sharp (mm–cm); diffuse/composite body imaging is not |
| **Update/airtime** | Fleet must re-fix within holdover (~20 ms optimum, `12`); TDM cycle across 20 faces ~6 ms at MCS0 | Broadcast topology (reference frames are O(1)); coupled-oscillator/consensus holdover (`12`) | Scheduling; multi-Star homes (clusters of coherence, stitched by closure phases) |
| **Target coherence** | λ/(2v) ≈ 62 ms at 1 m/s ⇒ coherent dwell ≤ ~60 ms without motion compensation | Doppler pre-compensation (MovISAC/MOSAIC, `arXiv:2502.08236`) | Composite bodies: partially; extended targets smear |

The deepest honest boundary: **a passive receiver cannot synthesise the
reference** (`arXiv:2506.22252`, ESTABLISHED) — transmitter cooperation is
mandatory, i.e. *some* radiating reference must exist. The minimal wireless
reference is one common-transmitter packet per coherence interval; the minimal
wired reference is one shared clock+tone. Sharp 3D additionally requires the
fleet to be coherent, i.e. the common-transmitter packet must arrive often
enough — the E1 number.

---

## 6. The six search directions — verdicts

1. **Wireless coherence on commodity silicon — YES, three routes.** Per-frame
   common-TX referencing (ESP-PPB-class), two-way reciprocity (BeamSync-class),
   static-path referencing (LoSRef/SHARP-class). ESP32-S3-specific fleet
   demonstration: **absent** (the C3 repo is the only claim found).
2. **Self-calibrating learned array — YES, with a design choice.** Joint
   geometry+target discovery is established algorithmically (Fascista; WiSLAT;
   atomic-norm self-cal). The invention chooses *solve-don't-learn* per home:
   physics priors fixed; learning only for robust features/outliers. Transfer
   across homes is by solving each home, with Sim2Real priors from note 19.
3. **Environment as shared reference — PARTLY.** Static paths and virtual
   anchors supply references and calibrators; they **cannot** break the gauge
   (they are part of the channel and carry the same `e^{j(θ−ψ)}`). They *can*
   extend the effective aperture (passive reflectors; note 14) and enrich the
   coherent inverse problem (Fascista exploits wall reflections).
4. **Sharp 3D from many coarse apertures — YES, via near-field carrier phase
   once coherent.** House-scale coherence makes every target near-field; with
   10° phase transfer, ~mm lateral and ~cm range are CRB-plausible at 3 m. The
   Star adds full-sphere DoA, polarisation diversity, and Doubled-sensitivity
   round-trip signatures.
5. **Ping + unique replies + two-way products — YES, three observables per
   exchange** (LO phase; hardware fingerprint; channel/change), and with the
   Star's aperture the same burst surveys geometry. This is the protocol core.
6. **New excitation/composition — YES for TDM-MIMO, coherent TX within the
   Star, channel-hopping stitching, passive illumination.** Full-duplex
   monostatic on one S3 radio: NO (SIC ~110 dB); external DQM at 2.4 GHz is the
   demonstrated non-S3 escape (ISAC-Fi).

---

## 7. Decisive experiments

**E1 (the single decisive experiment) — per-frame phase-transfer residual.**
Two ESP32-S3 boards + one conducted ground-truth reference (ESPARGOS-style
clock+tone or a cable-tied reference frame). The anchor radiates
`[reference frame] → 1 ms → [sensing frame]` at ≥ 50 Hz for ≥ 30 min with a
temperature step. Measure (i) cross-frame phase residual, (ii) cross-node
relative phase via the common-TX escape (add 3–4 nodes), against ground truth
and the Modified CRB. **Success:** cross-node residual ≤ 10° at 1 ms delay, no
unbounded drift; then 200 ms / 2 s / 10 s holdover curves. **Falsified if:** the
residual is statistically random or grows without bound — then carrier-phase
3D dies and only amplitude/Doppler routes survive (the boundary quantity is the
per-frame residual; minimal crossing reference is the conducted tone — i.e. the
Star itself).

**E2 — Star internal coherence and 3D AoA.** 20-face prototype (or 2 faces
first): coherent MUSIC/spherical-harmonic DoA of a known reflector; volumetric
vs planar aperture gain; mutual-coupling check.

**E3 — Self-survey.** 5 nodes, two-way + closure phases + near-field
estimation; compare node geometry to a tape measure. Success: ≤ 5 cm RMS
3D geometry error; falsifies A5/M7 if closure phases are dominated by multipath.

**E4 — The channel seam.** Calibrate per-channel tables; stitch ch1/ch6/ch11;
verify the phase slope across the seam is linear and predicts a coax/known
delay. Falsifies M6 on ESP32 if the seam is random (then only per-channel
ranging survives).

**E5 — Person localisation.** Coherent fleet, cm-level 3D of a walking person
vs camera/FTM ground truth; report localisation error vs phase residual.

---

## 8. ESP32-S3 feasibility

| Mechanism | S3 feasibility | Evidence |
| --- | --- | --- |
| Conducted coherent array | Demonstrated on S2 (ESPARGOS), C5 (esp-crab); S3 same radio family | `2502.09405`, `2408.16377`, esp-crab docs |
| Per-frame common-TX referencing | Claimed on C3 (ESP-PPB, `<10°` typical, unrefereed); S3 needs the ns-timestamp hack + a disciplined reference | repo claim; `17` |
| CSI excitation | ESP-NOW HT40 MCS0 frames excite CSI; raw `esp_wifi_80211_tx` defaults 1 Mbps 11b (no OFDM preamble, no CSI) | `00` §8; `07` |
| ns RX timestamp | Exists (`WIFI_PKT_RX_TIMESTAMP_NSEC`), 80 MHz grid, latch ~20.8 µs; µs field unreliable under power save | `17` |
| FTM | Supported, 2.4 GHz HT20, 1–2 m | `15` |
| Channel stitching | Not tried on ESP32; per-channel phase table needed (M1) | `2005.03755` (Intel) |
| Near-field estimation | Arithmetic same as established; no ESP32 instance | M5 refs |
| On-chip inference | ESP-DL/ESP-NN INT8 exists (vision); CSI fusion likely host-side | `19` |

No ESP32 volumetric coherent array is known to exist (`00` §7; searches found
none), so M1 is the first instance — engineering risk is real but bounded
(mutual coupling, PLL re-lock across channel bounces, thermal drift of the
reference distribution).

---

## 9. TX needed? Cost

**TX: yes, for the full design.** Reference frames (M2) and Star illumination
(M5) are transmissions; the two-way suite (M3) is bidirectional. A **passive
variant** exists (ambient AP as common transmitter; the Star as passive coherent
reference array + calibrator) but still needs the Star's own reference radiated
during calibration (ESPARGOS does this) — i.e. *some* compliant TX must exist
in the home. Receive-only operation cannot cross the gauge (`2506.22252`).

**Cost (order-of-magnitude):** Star = 20 × (~US$8–12: S3 + antenna + RF switch
+ coupler/passives) + hub (TCXO/VCTCXO, 2.4 GHz synthesis + splitter/PA,
Ethernet switch, PCB/assembly) ≈ **US$300–450** at small volume; fleet nodes
US$5–10 each; a 10-node home < US$550. Compare ESP-PPB at US$50–80/node
(VCTCXO + dual DACs per node) — the Star pays once, the fleet stays cheap.

---

## 10. Evidence strength: 6/10

The constituent physics is strong: the gauge and escapes (`20`), ESPARGOS
conducted coherence (productised), BeamSync/reciprocity calibration, closure
phase (astronomy), near-field localisation, channel stitching on one NIC. The
composition is plausible and cheap. The score is held down by: (i) the wireless
per-frame transfer on S3 resting on one unrefereed repo (and none on S3);
(ii) cross-channel absolute phase untested; (iii) mutual coupling/shadowing of
the soldered Star unmeasured; (iv) no end-to-end demonstration of a coherent
house-scale Wi-Fi fleet anywhere. Each is directly testable by E1–E5; a
successful E1 alone moves this to 8/10.

---

## 11. Primary sources (annotated)

- ESPARGOS channel sounder — `arXiv:2502.09405` (PLL phase uncertainty; ref
  signal distribution; per-channel calibration; passive sniffer).
- ESPARGOS datasets — `arXiv:2408.16377` (shared 40 MHz + 2.4 GHz reference;
  combined arrays via power splitters; phase-coherent CSI).
- ESPARGOS combined arrays — https://espargos.net/docs/combined-arrays.html
  (master/slave star topology; `CSIPool.calibrate(per_board=False)`; global
  phase calibration); ESPARGOS One datasheet (shared 40 MHz + 2.4 GHz ref).
- **ESP-PPB** — https://github.com/jonathanmuller/esp-ppb (ESP32-C3; FTM +
  VCTCXO dual-DAC; reference frame ~1 ms after external frame; `<10°` typical
  single-frame; unrefereed, pre-production).
- esp-crab / ESP-CSI solution docs (Espressif) — two ESP32-C5 joined by a clock
  buffer; phase used for mm-level disturbance sensing.
- Zubow, Gawłowicz, Dressler — `arXiv:2005.03755` (13 × 20 MHz stitching to
  ~80 MHz; semi-deterministic four-value phase offsets; Intel 9260).
- Rogalin et al. — `arXiv:1310.7001` (distributed MIMO sync & reciprocity
  calibration). Zhu et al. — `arXiv:1709.07206` (star topology optimal).
  `arXiv:2606.14419` (global phase "up to a common constant"; CRLB topology).
  `arXiv:2509.03722` (OTA phase calibration in the TDD flow; phase-noise
  spectrum −120…−80 dBc/Hz).
- Ganesan, Sarvendranath, Larsson — BeamSync `arXiv:2112.00592`, `2311.11070`.
  Larsson & Vieira — `arXiv:2304.05144` (R- and F-calibration principles).
- Repeater-aided OTA sync — `arXiv:2603.19903` (intermediate node; closed-form
  phase; RMSE 0.01 rad in simulation).
- Fascista et al. — "Joint Localization, Synchronization and Mapping via
  Phase-Coherent Distributed Arrays", IEEE JSTSP 2025,
  DOI `10.1109/JSTSP.2025.3533111` (`arXiv:2409.12478`): near-field distributed
  arrays, unsynchronised UE, 3-step ML, phase-coherent mapping.
- Phase-coherent D-MIMO ISAC — `arXiv:2512.21953` (sub-wavelength localisation
  with coherent processing). Doppler-only — `arXiv:2609.19401`.
- MovISAC/MOSAIC — `arXiv:2502.08236` (coherent imaging of composite
  anisotropic moving targets; Doppler pre-compensation).
- WiSLAT — `arXiv:2603.17283` (joint station localisation + target tracking;
  mutually dependent).
- ISAC-Fi — `arXiv:2408.09851` (2.4 GHz monostatic over Wi-Fi; DQM analog
  canceller; 20 MHz).
- Bistatic ISAC clock asynchronism survey — `arXiv:2402.09048` (CACC/CASR;
  delay-domain reference-path (SHARP); open problem: coherently combine
  multiple receivers).
- LoSRef — `arXiv:2602.05344` (OTA LoS-path referencing gives phase-coherent
  CIR on practical Wi-Fi; respiration).
- "Primer on ISAC-izing commodity Wi-Fi" — `arXiv:2601.12980` (four diversity
  axes; ESP32 sensing landscape).
- Two-way CSI device-free localisation — Dhawahir et al., IEEE (2026),
  `ieeexplore.ieee.org/document/11339481` (two-way CSI cross-correlation).
- Reflection/phase reference in COTS: WiEps, SoundiFi (via `2602.05344`).
- Closure phase: Jennison (1958); "A Geometric View of Closure Phases",
  `arXiv:2012.05254`; self-cal (Schwab 1980; Cornwell & Wilkinson 1981;
  Pearson & Readhead 1984); https://en.wikipedia.org/wiki/Closure_phase.
- Near-field: Webber, "Range Estimation from Wave-Front Curvature", DTIC
  `ADA081640`; Le Cadre (1995); Abu-Shaban et al., `arXiv:2010.05617` (RIS lens);
  beam focusing `ieeexplore.ieee.org/document/10925873`.
- Multipath-assisted SLAM / virtual anchors — `MDPI 13/9/1625`.
- AoA phase offsets on commodity NICs — `arXiv:2005.03755` (above); receiver
  effects across devices — `arXiv:2605.26836`.
- ESP32 CSI phase unusability for absolute phase — `arXiv:2607.26665`
  (OpenCSI, cited in `20`).
- Time precision — `17`; legality/primitives — `07`; TDM/full-duplex — `09`;
  oscillator consensus/holdover — `12`; boundary/CRB — `20`.
- Over-the-air computing (receiver-only insufficient) — `arXiv:2506.22252`.

---

## 12. Residual unknowns

1. The E1 number on S3: per-frame transfer residual vs cadence, temperature,
   channel bounces. Everything else is downstream.
2. Cross-channel absolute phase feasibility with hub-fed reference frames (E4).
3. Mutual coupling and face shadowing on a conducting polyhedron; the true
   array manifold; polarisation purity per face.
4. Whether closure-phase ranging is dominated by multipath in lived-in homes
   (E3).
5. Legal envelope for the reference-frame rate and any CW/illumination modes.
6. S3 MAC scheduling: interleaving reference frames with sensing at ≥ 50 Hz
   under CSMA; worst-case latency distribution.
7. Whether the ESP-PPB scheme (a) is real, (b) generalises to S3, (c) holds
   cross-node (not only cross-frame).

---

## 13. Chaining / Existing models and data / Own data

### (A) Chaining

- **M1 (conducted Star) enables M2** (a calibrated aperture to radiate from):
  without a coherent multi-face anchor the per-frame fix degenerates to
  ESP-PPB's point source (weaker: no AoA, no self-survey).
- **M2 enables M5/M6** (a coherent fleet is a near-field aperture; a phase table
  is what stitches channels). "Cross-node carrier phase **blocked without** the
  common-TX frames or the conducted reference."
- **M3 enables A4/A7 and M7** (hardware fingerprints feed the factor graph).
- **M4 is orthogonal insurance**: closure phases work when M2 degrades; they
  are also the *only* gauge-free geometry constraints.
- **Highest-leverage:** M2. A failed E1 collapses the architecture to
  amplitude/Doppler + RTI + channel charting (the receive-only v2 core);
  a successful E1 unlocks every coherent capability (AoA/3D/mapping/SLAM).
- **Capability unlocked further:** once the home is coherent, **learned pose**
  (note 19) has phase features it has never had; and channel charting becomes
  coordinate-carrying rather than affine-ambiguous (`03` → absolute geometry).

### (B) Existing models and data

- **Algorithms, directly usable:** reciprocity calibration (Rogalin/Argos;
  BeamSync; `2304.05144`), closure-phase/self-cal (astronomy libraries and
  methods), near-field ML estimation (Fascista `2409.12478`; `2512.21953`),
  joint station/target estimation (WiSLAT), channel charting (note 03),
  factor-graph SLAM (Thrun; multipath SLAM `MDPI 13/9/1625`).
- **Datasets:** ESPARGOS/DICHASUS (phase-coherent, wired ground truth —
  ideal to *validate M4* because they contain true geometry);
  MM-Fi, CSI-Bench, SenseFi (note 19) for the learned layers; ESP-PPB (if it
  ships) as the only wireless-coherent reference data.
- **Hardware/IP:** ESPARGOS design (open); esp-crab (open); Zubow's stitching
  method (paper); ESP-PPB (GPL-3.0). The Star's hub design can follow ESPARGOS
  (resistive splitter, frequency-multiplexed clock + reference over one coax).

### (C) Own data / new technology

- **The undemonstrated thing, precisely:** a *wireless* self-surveying coherent
  fleet of ≥ 4 ESP32-S3 nodes at house scale, and the E1 residual number.
- **Minimal experiment (E1 above)** — two S3s + conducted ground truth; one
  week of work; falsifies or validates the whole coherent route.
- **Minimal dataset worth publishing:** cross-frame/cross-node phase residual
  vs `τ_ref` (0.1–100 ms), cadence (10–200 Hz), temperature, and channel-bounce
  count, for S3 vs C3 vs S2 vs ESPARGOS vs SDR. No such curve exists.
- **Second dataset:** closure-phase distributions for a 5-node mesh in 3 homes
  vs known node geometry — tests M4/M7 transfer across homes.

### Vocabulary and v1 conflict

This note uses terms `CONTEXT.md` explicitly excludes for v1: *phase-coherent
RF reference*, *ESP-NOW* (as the de-facto CSI excitation), *Doppler*, *raw I/Q*
(only in cited prior art), and node roles close to *access point* (the Star as
illuminator). None is a v1 concept; adopting any of this architecture requires a
glossary change and an ADR. Per repo policy, this research note surfaces the
conflict rather than resolving it.

---

## One-line answer

**The invention is the Coherent Star: a conducted-coherent 20-face polyhedron
that fixes every cheap node's carrier phase per received frame through broadcast
reference frames, doubles every calibration exchange as an AoA/range/hardware
measurement, and lets the home be solved as one near-field radio interferometer
(closure phases, phase referencing, self-cal) — sharp, self-calibrating and
generalisable because it solves physics rather than houses. The physics does not
close the space; it gates it behind one measurable number (the per-frame
phase-transfer residual, ≤ ~10° at 1 ms is the threshold), one untested trick
(absolute cross-channel phase), and one law (time-bandwidth sets resolution, not
precision).**
