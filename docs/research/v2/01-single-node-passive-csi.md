# 01 — Single-node passive CSI sensing

*Research note, Aquila v2 deep-research brief. Hardware reference: ESP32-S3.
Australian English. Primary sources only. Claims are tagged ESTABLISHED /
HYPOTHESIS / SPECULATION.*

## Question

What can **one ESP32-S3 receiver** infer from 802.11 CSI alone about **motion,
presence and amplitude/Doppler**, and what bounds detection? Specifically:

- (a) amplitude/phase features used;
- (b) temporal CSI ratios and their blindness to static scatter;
- (c) single-link detection range/rate limits;
- (d) sensitivity to AGC, per-frame rate, null-subcarriers.

Scope overlaps deliberately avoided: multi-node localisation is 02, through-wall
materials are 04, raw I/Q is 05, I/Q passive radar is 06. This note stays with
**one receiver, CSI only**.

Structural fact that frames everything below: a single passive CSI receiver has
**no common clock with the illuminator**, so the measured per-subcarrier phase
carries an unknown, packet-varying offset (timing offset, TO; carrier frequency
offset, CFO; per-antenna PLL phase). The information that survives that
impairment — and the operations that recover it — *are* the single-node
question.

---

## Findings

### ESTABLISHED

**E1 — What CSI is on an ESP32, per frame.** ESP32 CSI is the per-subcarrier
channel frequency response, estimated on the received LTF preamble fields. Each
subcarrier is stored as two signed 8-bit integers (imaginary then real). Up to
three fields are exposed depending on packet type: LLTF, HT-LTF, STBC-HT-LTF.
For a non-HT / 20 MHz packet the LLTF block is 64 subcarrier indices (128 bytes);
for HT20, LLTF + HT-LTF yields 256 bytes (two 64-subcarrier blocks); 40 MHz
yields larger blocks. The API reports which bytes are valid. Null/guard
subcarriers and pilot subcarriers are included in the raw buffer and must be
discarded by software. A hardware limitation marks `first_word_invalid` — the
first four CSI bytes are invalid and must be zeroed. Sources: ESP-IDF *Wi-Fi
Vendor Features*; `espressif/esp-csi` issues #114, #185, #224; ESP-IDF issue
#14271. **This is demonstrated on the ESP32 family, explicitly including S3.**

**E2 — A single ESP32 receiver genuinely infers motion and presence.** Espressif
ships `esp-radar` (motion/presence detection) and `esp_wifi_sensing` (a
motion/presence finite-state machine with multi-channel management, dynamic
baseline, hysteresis, debounce, on-site calibration and event callbacks). The
documented application scenarios are motion, presence and activity recognition,
and the "get router CSI" mode uses *one* ESP32 plus an existing router.
Independent open implementations corroborate: ESPectre reports ~97 % motion
accuracy at 3–8 m from a router (community documentation; GPL-3.0 with a
commercial option); Espressif's own chip ranking is ESP32-C5 > C6 > C3 ≈ S3 >
ESP32. Sources: Espressif ESP-Techpedia *Introduction to ESP-CSI Solution*;
`espressif/esp-csi`; `esp_wifi_sensing` component README.

**E3 — Amplitude features are the workhorse; phase is secondary.** The
recurring, field-proven feature set is built on CSI **amplitude/power**:
per-subcarrier amplitude `|H|`, squared power `|H|²`, moving variance
segmentation (MVS) over a sliding window, Doppler-spread spectrum (Fourier
transform of the temporal amplitude stream), and per-subcarrier variance/top-K
selection. Statistical summaries (mean, std, range, skewness, kurtosis,
entropy) feed lightweight classifiers. ESPectre's on-device algorithm is
MVS + NBVI (Normalized Baseline Variability Index) subcarrier selection +
gain lock. Sources: Zeng et al. *WiFi Sensing with CSI: A Survey* (CSUR 2019);
MDPI *Tools and Methods for Achieving Wi-Fi Sensing in Embedded Devices* (2025);
ESPectre / ESPectre-fork docs; Wu et al. *Indoor Motion Detection Using Wi-Fi
CSI* (Sensors 2018).

**E4 — Phase-derived features require a reference; single-antenna raw phase is
not usable.** With independent TX and RX clocks the measured phase contains
unknown packet-varying terms (PBD, SFO, CFO, PLL initial phase) that cannot be
inverted from one antenna. Established remedies are (i) **conjugate
multiplication / cross-antenna phase difference** across antennas that share a
receiver clock, and (ii) **linear detrending across subcarriers** (phase
sanitisation). PhaseBeat and TensorBeat use the inter-antenna **phase
difference**; FarSense uses the **CSI ratio** of two adjacent antennas. TensorBeat
shows the phase difference is highly stable for back-to-back packets because the
per-packet TO/CFO terms are common to antennas on the same NIC and cancel.
Sources: Wang et al. *PhaseBeat* (IEEE ICDCS 2017, DOI 10.1109/ICDCS.2017.206);
Wang et al. *TensorBeat* (ACM TIST 9(1):8, 2017, DOI 10.1145/3078855;
arXiv:1702.02046); Zeng et al. *FarSense* (IMWUT 3(3):121, 2019,
DOI 10.1145/3351279; arXiv:1907.03994).

**E5 — The CSI ratio and CSI power cancel clock asynchrony and AGC, and their
observable is driven by the static–dynamic cross term.** Formally, a measured
CSI sample is `CSI = α·e^{−j(2π f τ_TO + φ_CFO + φ_PO)}·(H^S + H^X)` with AGC
factor α, static CFR `H^S`, dynamic CFR `H^X`. The **cross-antenna ratio** (of
two co-clocked antennas) cancels the common TO/CFO/PLL terms and common gain;
the **element-wise power** `|CSI|²` cancels *all* phase terms. Both leave the
cross term `|H^S H^X|·cos(φ_Doppler + φ_delay + φ_AoA)`, which retains
Doppler-, delay- and AoA-dependence. The purely dynamic term `|H^X|²` is usually
weaker than the cross term and is treated as noise. Source: Wang et al.
*Bistatic Passive Sensing via CSI Power* (arXiv:2511.22144); Wang et al.
*Rethinking RSSI for WiFi Sensing* (arXiv:2602.14004 / npj Wireless Technology);
Zeng et al. *FarSense*.

**E6 — "Blindness to static scatter" is real but two-sided.** Because the
surviving observable is a *cross* term, a scene with **only static scatter**
produces a constant (no time variation) and is invisible — consistent with the
brief's premise that non-coherent temporal ratios give Doppler "moving only,
blind to static". Conversely, the ratio/power mechanism **needs a static
reference path present**: the dynamic phasor is observed as a rotation *around*
the static phasor, so the ratio model is not blind to static in the sense of
ignoring it — it *uses* the static (LoS + static multipath) as the reference.
This yields the classic **blind-spot** phenomenon: when the dynamic and static
phasors are in phase (or the target angle equals the effective static angle),
the observable collapses. FarSense addresses this by combining the orthogonal
I/Q (amplitude and phase) of the CSI ratio, projecting onto a fine-grained axis
to avoid blind spots. Source: Zeng et al. *FarSense*.

**E7 — Temporal CSI ratios yield Doppler (moving targets only).** Doppler
velocity is extracted from the temporal evolution of the CSI (or CSI
ratio/power) via a Fourier/STFT transform; the Doppler shift maps to the
line-of-sight velocity component. Because the observable is the *time-varying*
cross term, a stationary target gives no Doppler. Widely replicated
(PhaseBeat/TensorBeat, WiDance, Widar2.0, WiDFS, MORIC). Sources: Qian et al.
*Widar2.0* (MobiSys 2018); Zhang et al. *WiDance* (MobiCom 2017);
MDPI *CSI-F* (2024); Zhang et al. *WiDetect* (IMWUT 3(3):77, 2019,
DOI 10.1145/3351280).

**E8 — A calibration-free statistical motion statistic exists and is
hardware-agnostic in principle.** WiDetect models the *autocorrelation function*
of CSI and shows it maps to target motion via statistical electromagnetics,
producing a motion statistic that is (claimed) independent of environment,
location, orientation and subject; it reports 99.68 % accuracy at ~zero false
alarm with through-wall, whole-home coverage on a **single commodity link**.
Source: Zhang et al. *WiDetect* (IMWUT 2019, DOI 10.1145/3351280).

**E9 — Single-link range and rate are bounded by physics and the MAC.**
- **Range resolution** `Δr = c/(2B)` and **unambiguous range**
  `R_max = cN/(2B)`, where B is the occupied bandwidth and N the subcarrier
  count. At B = 20 MHz, `Δr = 7.5 m`; at 40 MHz, `Δr = 3.75 m`. Wi-Fi bandwidth
  is the hard limiter on delay/range resolution.
- **Velocity resolution** `Δv = c/(2 M f_c T)` and **maximum unambiguous
  velocity** `V_max = c/(2 f_c T)`, where T is the inter-frame interval and M
  the number of frames in the coherent window. Frame rate therefore sets both
  the velocity resolution and the unambiguous velocity window.
- Worked ESP32-S3 case (assumptions stated): f_c = 2.437 GHz (channel 6),
  λ = 0.123 m. At T = 1/20 s, `V_max = λ/(2T) ≈ 1.23 m/s`; at T = 1/100 s,
  `V_max ≈ 6.15 m/s`. With M = 32 frames at 20 Hz, `Δv ≈ 0.04 m/s`.
  (Radar texts and the cited papers differ by a factor of two in the `V_max`
  convention; treat the number as order-of-magnitude pending a controlled
  measurement.)
- **MAC throughput bound**: the ESP32 CSI callback fires on received packets,
  so the per-frame rate is capped by transmission and serialisation. A 2025
  ESP32 collection tool measured ~180 pps configured, empirically saturating at
  ~85 pps (binary) to ~165 pps; other ESP32 builds stream ~20 Hz sustained.
  Far higher rates (≥1000 pps) starve Wi-Fi throughput (>90 % loss).
Sources: *Extracting Range-Doppler Information of Moving Targets from Wi-Fi
CSI* (arXiv:2508.02799); MDPI *Tools and Methods…* (2025); *WiFi-CSI Difference
Paradigm* (IMWUT 8(2), 2024, DOI 10.1145/3659608).

**E10 — Demonstrated single-link ranges.** FarSense (Intel 5300, 3 antennas,
CSI ratio) reports respiration at 8–9 m with 100 % detection rate, extending
the prior 2–4 m state of the art, and is first to do through-wall respiration
with commodity Wi-Fi. ESP32-specific through-wall presence/HAR has been shown
at long range with F-measure ≥0.93, and community ESP32-S3 builds report
through-wall presence out to ~5 m. Sources: FarSense (above); Zenodo dataset
*WiFi CSI-Based Long-Range Through-Wall HAR with the ESP32* (2023);
`euaziel/WiFi-CSI-Human-Pose-Detection`.

**E11 — ESP32 AGC is recoverable and controllable.** The ESP32 CSI buffer does
**not** embed AGC gain, but `agc_gain` and `fft_gain` are readable from
`wifi_pkt_rx_ctrl_phy_t`; Espressif ships `esp_csi_gain_ctrl`, which computes a
gain baseline, derives a compensation factor, and can force a receive gain to
stabilise CSI (supported on ESP32, S2, S3, C3, C5, C6, C61). Community
ESPectre builds "gain-lock" the AGC/FFT gain via PHY register access on
S3/C3/C5/C6. Sources: `espressif/esp-csi` issue #185; `esp_csi_gain_ctrl`
component README; ESPectre documentation.

### HYPOTHESIS

**H1 — On ESP32-S3 specifically, amplitude/power-based single-link sensing is
the reliable floor; phase-coherent single-link sensing is not yet demonstrated
without a shared/conducted reference.** Espressif's own phase-capable motion
board deliberately connects an ESP32-C3 (TX) and ESP32-S3 (RX) through a
**clock buffer** to remove the relative frequency offset *before* the S3 uses
phase. That is a shared-clock escape, not a wireless one. Absent that, an S3
receiver must fall back to phase-free observables (amplitude, CSI power,
temporal self-conjugate). The brief's premise — wireless-only shared-reference
escapes are unproven on S3 — is consistent with what Espressif ships.

**H2 — CSI power / temporal self-conjugate is the correct default observable
for a *single-antenna* ESP32-S3 receiver.** Cross-antenna ratio needs ≥2
co-clocked antennas; a single-antenna S3 cannot form it. Element-wise power
cancels the random phase and AGC and preserves the static–dynamic cross term
(Doppler), giving a hardware-honest path at the cost of the mirror ambiguity
and loss of delay/AoA phase structure. Source basis: arXiv:2511.22144,
arXiv:2602.14004.

**H3 — Gain locking materially changes single-link, amplitude-based detection.**
Because AGC rescales amplitude per packet, and gain control is only
approximated as a global scalar, an uncompensated amplitude pipeline conflates
channel change with gain change. ESPectre and `esp_csi_gain_ctrl` exist
precisely because this matters; the magnitude of the improvement on S3 (versus
RSSI-rescaled or ℓ2-normalised amplitude) is not yet quantified in a
controlled single-link study.

**H4 — The ping-and-unique-reply round-trip scheme can measure more than range.**
With one node pinging and every other node replying with a **unique** packet:
it can (i) attribute each CSI sample to a specific TX node (identity via the
unique reply), (ii) provide a distinguishable, near-simultaneous measurement of
each TX→RX link, and (iii) yield coarse two-way timing (RTT/FTM-like range) and
a TX/RX frequency-offset estimate. Caveats: RTT is biased by variable MAC
reply latency (it is not a symmetric physical round trip unless the reply
latency is measured), the µs RX timestamp floors precision, multipath biases
range, and no common clock exists. FTM (802.11mc) is supported on S3 but coarse.

### SPECULATION

**S1 — A "controlled duty" illuminator could unlock single-link Doppler at
levels commodity infrastructure cannot.** If Aquila controls the TX (ESP32
`esp_wifi_80211_tx` / ESP-NOW), the frame rate T becomes a design parameter,
setting `V_max` and `Δv` directly. This is a *chaining* argument: TX control
(07/08) is what makes single-node Doppler bounds tunable rather than inherited.

**S2 — Sub-Nyquist/compressed approaches may recover Doppler from sparse
sampling.** The CSI-difference paradigm claims Doppler from as few as three
samples and 5 % of the usual packet count; if it holds on S3, it would relax
the rate/throughput tension in E9. Unverified on ESP32.

---

## ESP32-S3 feasibility

| Capability | Feasibility on ESP32-S3 (single passive receiver) | Basis |
|---|---|---|
| Motion detection (amplitude/MVS) | **Demonstrated** | esp-radar, esp_wifi_sensing, ESPectre |
| Presence detection (including static) | **Demonstrated, weaker** | `waveform_wander` FSM feature; calibration required |
| Breathing / heart rate | **Demonstrated but experimental** | ESPectre/community; 6–30 BPM / 40–120 BPM |
| Doppler (moving-only) from CSI power/temporal ratio | **Feasible in principle; not yet characterised on S3** | E5/E7; needs a static reference path |
| Range/velocity resolution (radar-grade) | **Blocked** — 20/40 MHz bandwidth and ≤~180 pps cap it | E9 |
| Single-link phase-coherent sensing w/o shared clock | **Not demonstrated on S3** | Espressif ships a clock buffer for phase use |
| Coarse ranging (FTM/RTT) | **Demonstrated, coarse** | brief premise; 802.11mc on S3 |

Hardware honesty: E1–E4, E6, E7, E10, E11's *mechanisms* are demonstrated on
**ESP32 family / ESP32-S3**, or on **other COTS Wi-Fi** (Intel 5300, Atheros,
Wi-Fi 6 NICs) where noted. The high-accuracy range/velocity work (E9) and the
CSI-ratio CRB theory are on **Intel 5300 / Wi-Fi 6E laptop NICs**; the
quantitative gap is **bandwidth (20–40 MHz vs 160 MHz), antenna count (1 vs 3),
per-frame rate (≤~180 vs ~1000 pps), and phase availability (unavailable vs
co-clocked)**. No single-link result reviewed here required SDR-grade hardware;
SDR-grade capability is a separate, higher ceiling (notes 05/06).

## TX needed?

**Yes — but not necessarily Aquila-controlled TX.** Passive sensing needs *an
illuminator*; the smallest viable single-node setup is one ESP32-S3 receiver
plus a router (`esp-csi` "get router CSI" mode). However:

- For **controllable duty/rate** (to set `V_max`, `Δv`; S1) Aquila needs a TX
  it controls (`esp_wifi_80211_tx` / ESP-NOW) — enabling capability 07/08.
- For **phase-coherent** single-link sensing, a **shared/conducted reference**
  or **co-clocked** peer (esp-crab-style) is needed; this is the escape the
  brief flags as unproven on S3 and overlaps notes 10/11/12.
- The **ping-and-unique-reply** scheme *is* a TX-driven primitive.

## Evidence strength

**7 / 10.**

Justification: the qualitative claims — that a single ESP32 receiver infers
motion/presence, that amplitude features dominate, that CSI ratio/power cancels
clock asynchrony and AGC, that temporal observables are Doppler and
moving-only, and that bandwidth/frame-rate bound range/velocity — are all
backed by **multiple independent primary sources**, several peer-reviewed
(CSUR 2019, IMWUT 2019, ACM TIST 2017, IEEE ICDCS 2017) and several
vendor/IP primary (ESP-IDF docs, esp-csi issues, Espressif components). The
score is held below 8 because (i) the *quantitative* single-link bounds are
demonstrated on **Intel 5300 / Wi-Fi 6 NICs, not S3**; (ii) ESP32-S3
**single-antenna phase** usability without a shared clock is not demonstrated;
(iii) the `V_max` convention is inconsistent across sources and needs a
controlled measurement; and (iv) AGC-compensation benefit on S3 is asserted by
tooling, not measured here.

---

## Primary sources

1. Espressif, *Wi-Fi Vendor Features — Wi-Fi Channel State Information*,
   ESP-IDF Programming Guide (v6.1 / latest).
   https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/wifi-driver/wifi-vendor-features.html
2. Espressif, *Introduction to ESP-CSI Solution*, ESP-Techpedia.
   https://docs.espressif.com/projects/esp-techpedia/en/latest/esp-friends/solution-introduction/esp-csi/esp-csi-solution.html
3. `espressif/esp-csi` (GitHub) — esp-radar, esp-crab, examples; issues #114,
   #185 (AGC), #224 (subcarrier indices).
   https://github.com/espressif/esp-csi
4. Espressif, `esp_wifi_sensing` component (v0.1.1~3) README.
   https://components.espressif.com/components/espressif/esp_wifi_sensing
5. Espressif, `esp_csi_gain_ctrl` component (v0.1.5) README.
   https://components.espressif.com/components/espressif/esp_csi_gain_ctrl/versions/0.1.5/readme
6. ESP-IDF issue #14271 (LLTF/HT-LTF CSI layout, subcarrier order).
   https://github.com/espressif/esp-idf/issues/14271
7. Yang, Zheng, et al. *WiFi Sensing with Channel State Information: A Survey*,
   ACM Computing Surveys 52(3):46, 2019. DOI 10.1145/3310194.
8. Wang, X., Yang, C., Mao, S. *PhaseBeat: Exploiting CSI Phase Data for Vital
   Sign Monitoring with Commodity WiFi Devices*, IEEE ICDCS 2017, 1230–1239.
   DOI 10.1109/ICDCS.2017.206.
9. Wang, X., Yang, C., Mao, S. *TensorBeat: Tensor Decomposition for Monitoring
   Multi-Person Breathing Beats with Commodity WiFi*, ACM TIST 9(1):8, 2017.
   DOI 10.1145/3078855; arXiv:1702.02046.
10. Zeng, Y., Wu, D., Xiong, J., Yi, E., Gao, R., Zhang, D. *FarSense: Pushing
    the Range Limit of WiFi-based Respiration Sensing with CSI Ratio of Two
    Antennas*, IMWUT 3(3):121, 2019. DOI 10.1145/3351279; arXiv:1907.03994.
11. Zhang, F., et al. *WiDetect: Robust Motion Detection with a Statistical
    Electromagnetic Model*, IMWUT 3(3):77, 2019. DOI 10.1145/3351280.
12. Wang, Z., Zhang, J.A., Wu, K., Chen, K., Xu, M., Guo, Y.J. *Bistatic Passive
    Sensing via CSI Power*, arXiv:2511.22144.
13. Wang, Z., et al. *Rethinking RSSI for WiFi Sensing*, arXiv:2602.14004 /
    npj Wireless Technology 2026. DOI 10.1038/s44459-026-00053-y.
14. Hu, Wu, et al. *Performance Bounds and Optimization for CSI-Ratio based
    Bi-static Doppler Sensing in ISAC Systems*, arXiv:2401.09064.
15. *Extracting Range-Doppler Information of Moving Targets from Wi-Fi Channel
    State Information*, arXiv:2508.02799.
16. *Tools and Methods for Achieving Wi-Fi Sensing in Embedded Devices*, Sensors
    25(19):6220, 2025. DOI 10.3390/s25196220.
17. Qian, K., et al. *Widar2.0: Passive Human Tracking with a Single Wi-Fi
    Link*, ACM MobiSys 2018. DOI 10.1145/3210240.3210314.
18. *WiFi-CSI Difference Paradigm: Achieving Efficient Doppler Speed Estimation
    for Passive Tracking*, IMWUT 8(2), 2024. DOI 10.1145/3659608.
19. Wu, et al. *Indoor Motion Detection Using Wi-Fi Channel State Information in
    Flat Floor Environments Versus in Staircase Environments*, Sensors 2018.
    PMC6068568.
20. Cominelli, M., Gringoli, F., Restuccia, F. *Exposing the CSI: A Systematic
    Investigation of CSI-based Wi-Fi Sensing Capabilities and Limitations*,
    IEEE PerCom 2023 (80 GB Wi-Fi 6 CSI dataset). DOI 10.1109/PERCOM56429.2023.10099368.
21. *WiFi CSI-Based Long-Range Through-Wall Human Activity Recognition with the
    ESP32*, Zenodo dataset, 2023. DOI 10.5281/zenodo.8021099.
22. `francescopace/espectre` — ESP32 CSI motion sensing (MVS + NBVI), GPL-3.0.
    https://github.com/francescopace/espectre
23. *A Survey on Wi-Fi Sensing Generalizability*, arXiv:2503.08008.
24. *Ubiquitous WiFi and Acoustic Sensing: Principles, Technologies, and
    Applications*, J. Comput. Sci. Technol. 2023.
    DOI 10.1007/s11390-023-3073-5.

## Residual unknowns

- **R1.** Quantified single-link detection/velocity performance on **ESP32-S3**
  (not Intel 5300 / Wi-Fi 6): detection probability vs range vs frame rate vs
  subcarrier subset. No controlled S3 study found.
- **R2.** Whether ESP32-S3 **phase** is usable without a shared clock through a
  wireless escape (overlaps notes 10/11/12/13); Espressif ships a conducted
  clock buffer, which is evidence *against* the wireless route.
- **R3.** Exact AGC/FFT-gain **behaviour and residual** on S3 under motion;
  whether gain-lock alone recovers amplitude fidelity, and how `first_word_invalid`
  and null-subcarrier handling interact with feature extraction.
- **R4.** The `V_max`/`Δv` **convention** (factor-of-two) and the effective
  coherent window in the ESP32 CSI callback under real MAC contention.
- **R5.** The true observable of **temporal self-conjugate** (CSI power vs
  complex temporal ratio) on a *single antenna* S3 — mirror ambiguity, DC
  leakage, and whether the linear Doppler observable is usable at 20–100 pps.
- **R6.** Whether WiDetect's calibration-free statistic transfers to 8-bit,
  single-antenna ESP32-S3 CSI (it was developed on Intel 5300).

---

## Strategic framing

### (A) Chaining

- **Single-node passive CSI is the base primitive.** It *enables* 02
  (multi-node localisation: each additional receiver is another instance of
  this primitive) and 13 (static/dynamic decomposition, which is the formal
  separation this note's E6 gestures at).
- **Amplitude/power motion+presence detection is the highest-leverage enabling
  capability** because it is *already demonstrated on S3* and needs no phase
  reference; it alone unlocks occupancy, activity and vitals product features.
- **Single-link Doppler is enabled by** (i) a controlled illuminator (07/08)
  to set duty/rate, and (ii) a static reference path (13) to make the dynamic
  phasor observable. **Blocked without** either: uncontrolled router traffic
  caps `V_max`; a static-only scene yields no Doppler.
- **Phase-coherent single-link sensing is blocked** without a common/conducted
  reference or a wireless escape (10/11/12) — none proven on S3. This is the
  *further-unlock* chain: 07 (TX primitives) → 08 (geometries) → 10/11
  (calibration) → phase-coherent 3D aperture (02 plus volumetric arrays).
- **The ping-and-unique-reply round-trip scheme** is an *enabler*: it can
  deliver per-node identity + near-simultaneous link CSI + coarse timing in one
  primitive, feeding 10 (OTA reciprocity pairing) and 11 (two-way transfer).

### (B) Existing models and data

Already address this, and usable by Aquila:

| Asset | What it is | Aquila usability |
|---|---|---|
| `esp-csi` / `esp-radar` (Apache-2.0, Espressif) | CSI capture + motion/presence | Direct, S3-supported |
| `esp_wifi_sensing` (Espressif) | Motion/presence FSM, calibration | Direct, S3-supported |
| `esp_csi_gain_ctrl` (Espressif) | AGC/FFT-gain compensation | Direct, S3-supported |
| ESPectre (GPL-3.0 / commercial) | MVS+NBVI motion, dataset | Usable; licence is copyleft — mind relicensing |
| Zenodo ESP32 through-wall HAR dataset | LOS/NLOS presence + activity spectrograms | Direct input format; ESP32-native |
| Cominelli et al. Wi-Fi 6 CSI dataset (80 GB) | Wi-Fi 6 CSI, video ground truth | Usable; **not** ESP32 (different NIC/format) |
| PhaseBeat / TensorBeat (papers) | Phase-difference vitals / multi-person | Method transferable; Intel 5300 input |
| FarSense / WiDetect (papers) | CSI-ratio range; calibration-free motion statistic | Method transferable; test on S3 |
| Widar2.0 / WiDance / Widar BVP | Doppler velocity profiles | Method transferable |

Format caveat: method papers use Intel 5300 (30 subcarriers, 8-bit) or
Atheros (56 subcarriers, 10-bit) CSI; ESP32 exposes 64+ subcarriers at 8-bit
with a different subcarrier ordering and null/pilot layout. Any port needs the
subcarrier-map and gain handling in E1/E11.

### (C) Own-data / new technology

**Could a purpose-built Aquila dataset unlock something not currently
demonstrated? Yes — a controlled, single-link, *gain-instrumented* ESP32-S3
CSI corpus with synchronous ground truth.**

Minimal experiment/dataset:

1. **Hardware:** one ESP32-S3 receiver + one ESP32-S3 transmitter (Aquila-
   controlled duty) at a fixed baseline; optional second co-clocked S3 for a
   within-device reference only.
2. **Logging:** raw CSI **with `agc_gain` and `fft_gain` per frame**, packet
   timestamps, RSSI/noise floor, channel/bandwidth, and the `first_word_invalid`
   flag.
3. **Conditions:** empty room; one person static; one person walking a known
   straight line at three known speeds; respiration at rest; each repeated
   across 3 ranges (e.g. 2, 5, 8 m) and 2 frame rates (e.g. 20 and 100 pps).
4. **Ground truth:** video or mmWave/marker timing for motion and velocity.
5. **Analysis:** amplitude-only vs amplitude+gain-compensation vs CSI-power
   Doppler vs (where a co-clocked peer exists) phase-derived Doppler.

**What it would prove:** whether AGC compensation materially lifts S3
single-link detection; the empirical detection/velocity bound vs range and
rate; and whether CSI-power Doppler is usable at S3 rates.

**What would falsify it:** if, after gain compensation and subcarrier selection,
single-link detection/velocity performance is **not materially better than
amplitude-only or RSSI**, then "gain-instrumented CSI power adds value on S3"
fails. Separately, if a **static-only** occupancy scene produces no usable
signal in CSI power (as E6 predicts), that confirms the moving-only limitation
and rules out a single-link static-presence path without a modulation of the
static term.

A secondary Aquila-specific unlock: a **conducted-reference** capture to
establish, empirically and once, whether S3 phase could be calibrated without a
wireless escape — a decisive negative would close an entire research branch
(notes 10/11) for the S3 platform.

## Round-trip scheme (one node pings; others reply uniquely; round trips timed)

**Can measure:** per-responder **identity** (unique reply packet attributes
each CSI sample to a node); **coarse range** from round-trip time (FTM/RTT-like,
µs RX-timestamp limited); **relative LO frequency offset** between nodes; and a
**channel sample per link direction**, enabling OTA-reciprocity pairing
(`Ĥ_AB·Ĥ_BA⁻¹`) — the foundation work for notes 10/11.

**Where it breaks:** RTT asymmetries from variable MAC/reply latency inflate
range unless the reply latency is itself measured; multipath gives a biased
first-path estimate; the µs RX timestamp is a hard precision floor; independent
LOs mean phase is not comparable across the round trip without a reference; and
simultaneous replies collide without TDM/FDM/CDM (note 09), forcing
sequencing that worsens update rate.
