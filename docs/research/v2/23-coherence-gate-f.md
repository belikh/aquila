# 23 — The coherence gate, simulated to the register level (track T1, direction f)

Status: round-2 research note (out-of-band; never defines v1 scope). Australian
English. Character `f`: the interrupted round-1 session on the phase-swept
beacon / RIPS-style hub protocols, finished here as the **Phased-Sweep Beacon
(PSB)** inside the coherence-gate simulation suite.

**Everything numerical below is a SIMULATION** (NumPy models of estimators and
oscillator processes; no device was measured), per `EXPERIMENT-ENV.md`. Code:
`docs/research/v2/code/coherence-gate-f/` — `m2_residual.py` (parts A–D),
`sweep_beacon.py` (parts E1–E4), `e2e_chain.py` (parts F–G); summaries in the
sibling `*_summary.csv` files, figures in `*_figure.png`. Seeds are fixed
(`rng(20261016)`); every script reruns with
`/home/matt/projects/aquila-v2-spec/sim-python <script>`.

---

## 1. The question

Round 1 (Finding 4 of `ROUND2-BRIEF.md`) left one number standing between
"coherent" and "not coherent": **σ_φ, the per-packet cross-node carrier-phase
residual after CFO removal, at MAC cadence, against the ≤ 0.5 rad gate.** This
note attacks it in simulation, to the register level: which estimator removes
the CFO, how often the reference must be refreshed, what the TX-instant jitter
budget is, what face/packet averaging buys, and what the whole budget does to a
complete calibration → coherence → localisation chain. It also develops the
missing direction **f** (phase-swept beacon / RIPS-style hub) into a concrete
protocol — the PSB — and computationally checks its estimator stack.

**Answer in one sentence (simulation verdict):** on stock-ESP32-S3 parameter
ranges, a **bracketed** reference fix (reference frames ≤ 1 ms on both sides of
the sensing frame, linearly interpolated) holds σ_φ ≈ **0.056–0.105 rad at a
20 ms cadence — 4.7×–8.9× inside the 0.5 rad gate** — while an **unbracketed**
20 ms holdover is **8.6× outside** it (4.28 rad). The gate is passed or killed
by the bracketing discipline, not by the oscillator.

---

## 2. Model and parameters (the honesty block)

Shared across all three scripts: channel-6 centre f_c = 2.437 GHz
(λ = 12.29 cm); raw LO offset classes 10/20/40 ppm (→ raw CFO ±24.4/48.7/97.5
kHz); **residual** CFO after per-frame removal modelled as an
Ornstein–Uhlenbeck process with stationary σ_f ∈ {10, 30, 100} Hz and
correlation time τ_c = 1 s; per-frame phase measurement noise σ_n = 0.05 rad
(MCRB floor, part C, justifies it at ≥ 0 dB SNR); ESP-NOW burst frames at
100 µs spacing (ESP-PPB's measured frame rhythm); sensing superframe 20 ms
(round-1 update-rate optimum); 56 subcarriers per 20 MHz channel (HT20); two-way
Doppler 16.3 Hz per m/s, one-way 8.15 Hz per m/s; walker 1 m/s. The bracket
interpolator's error variance has a Wolfram-verified closed form
(`bracket_closed_rad`, checked against 2×10⁵-trial Monte Carlo to 5 decimals).
Scene dynamics enter as a deterministic phase rate (a bias term), not a
scattering simulation; multipath enters the end-to-end chain as a bounded
±5 cm budget term (labelled as such in `e2e_chain.py`), **not** physically
simulated. FDTD/GNU Radio were not available; nothing below pretends otherwise.

---

## 3. Findings

### ESTABLISHED (the mathematics these simulations evaluate)

**3.1 The bracket law (part A, `m2_summary.csv`).** With residual CFO σ_f and
correlation time 1 s:

| reference discipline | σ_f = 10 Hz | 30 Hz | 100 Hz |
| --- | --- | --- | --- |
| unbracketed holdover, 1 ms | 0.006 rad | 0.188 rad | 0.619 rad |
| unbracketed holdover, 20 ms | 1.249 rad | 3.744 rad | 12.36 rad |
| **bracketed (mid-point), 1 ms** | 0.0004 rad | 0.0012 rad | 0.0041 rad |
| **bracketed, 20 ms** | 0.036 rad | 0.108 rad | 0.361 rad |
| 2-state Kalman tracker, worst-between, 20 ms cadence | — | 0.182 rad | — |

Interpolation between two fixes kills the wander by ~10–30× at 20 ms because
the *low-frequency* part of the OU process — the part that integrates — is
common to both brackets and cancels. A 2-state Kalman tracker (phase + ladder
CFO fix each burst) does *not* beat mid-bracket interpolation at 20 ms
(0.182 vs 0.108 rad worst-between) — the tracker's worst point is *between*
updates, exactly where the sensing frame sits. **Brackets beat tracking** at
this cadence; use both (tracker for smoothing, bracket for the sensing frame).

**3.2 The TX-jitter × CFO law (part B).** Two-way exchanges convert reply-
instant error τ directly into phase: σ_φ = 2π·Δf_raw·σ_τ.

| raw CFO | uncorrected τ = 1 µs | ns-stamp-corrected, asym = 30 ns |
| --- | --- | --- |
| 10 ppm (24.4 kHz) | 0.153 rad | 0.005 rad |
| 20 ppm (48.7 kHz) | 0.306 rad | 0.009 rad |
| 40 ppm (97.5 kHz) | 0.612 rad | 0.019 rad |

The chip's 12.5 ns timestamp quantisation (√2 difference of two stamps → 5.1 ns
rms) is negligible next to a 30 ns turnaround asymmetry. A **deferral gate** —
discard replies whose software turnaround exceeds a threshold — works at
modest cost: gating at 5 µs keeps ~90 % of replies and cuts σ_φ from 0.851 to
0.411 rad in a 1 µs-Laplace + 10 % 8 µs-tail turnaround model; gating at 1 µs
keeps ~57 % and yields 0.154 rad. (Fractions are model-of-turnaround, not
measured ESP32 scheduler behaviour — HYPOTHESIS at the tail.)

**3.3 The measurement floor and the averaging law (part C).** Modified CRB per
reference event: σ = 1/√(2·N_s·SNR) → 0.094 rad at N_s = 56, SNR = 0 dB;
0.047 rad at 6 dB; 0.013 rad at 16 dB. Averaging N frames helps only as
√(N_eff): with common-mode correlation ρ_c between frames (shared LO, shared
path), σ_N/σ_1 = √((1+(N−1)ρ_c)/N) — at ρ_c = 0.5, N = 8 buys only 2.0×, not
2.8×. The 4-frame average used in the budget is the honest ρ_c ≈ 0.5 case.

**3.4 The design-point budget (part D, `m2_summary.csv` rows `D_*`).**
Quadrature sum of bracket wander + measurement (MCRB/√4) + two-way jitter
(ns-corrected) + scene (1 m/s walker on the sensing path, two-way Doppler over
the 1 ms bracket):

| design point | σ_φ | vs 0.5 rad gate |
| --- | --- | --- |
| **D1** stock S3, 1 ms brackets in 20 ms superframe, 4-frame avg, two-way jitter corrected (30 ns) | **0.105 rad** | PASS, 4.7× margin |
| **D2** stock S3, broadcast-only fix (no two-way term), 1 ms brackets | **0.056 rad** | PASS, 8.9× margin |
| D3 stock S3, 20 ms unbracketed holdover, walker present | 4.277 rad | **FAIL 8.6×** |
| D4 ESP-PPB-class VCTCXO (1 ppb), 20 ms unbracketed holdover, walker | 2.066 rad | **FAIL 4.1×** |
| D5 hazard: uncorrected 1 µs software reply jitter, 20 ppm | 0.324 rad | PASS but only 1.5× |
| D6 hazard: residual CFO 100 Hz, still bracketed at 1 ms | 0.105 rad | PASS, 4.7× |
| D7 hazard: 20 ms brackets, static scene (no walker), 30 Hz | 0.111 rad | PASS, 4.5× |
| D8 hazard: 5 ms brackets, walker on the *reference* path (broadcast) | 0.257 rad | PASS but only 1.9× |

**3.5 The killing parameter values (each alone reaches 0.5 rad):**

- unbracketed holdover: T* = **7.96 ms** (σ_f = 10 Hz), **2.65 ms** (30 Hz),
  **0.80 ms** (100 Hz);
- bracket gap: T* = **116.7 ms** (10 Hz), **55.7 ms** (30 Hz), **24.8 ms**
  (100 Hz);
- uncorrected reply jitter: τ* = **3.27 µs** (10 ppm), **1.63 µs** (20 ppm),
  **0.82 µs** (40 ppm);
- scene: a 1 m/s walker on the reference path costs 0.5 rad after **9.79 ms**
  (broadcast fix) / **4.89 ms** (two-way);
- target coherence time λ/2v = **61.5 ms** at 1 m/s — the superframe must stay
  ≪ this (20 ms is).

**3.6 Coherence budget ledger.** B = σ_f · T_obs in cycles; the 0.5 rad gate is
**0.0796 cycles**:

| T_obs | 10 Hz | 30 Hz | 100 Hz |
| --- | --- | --- | --- |
| 1 ms | 0.010 cyc | 0.030 cyc | 0.100 cyc |
| 5 ms | 0.050 cyc | 0.150 cyc | 0.500 cyc |
| 20 ms | 0.200 cyc | 0.600 cyc | 2.000 cyc |
| 61.5 ms | 0.615 cyc | 1.845 cyc | 6.150 cyc |

B ≤ 0.08 cycles at the working cadence is achievable **only** with a reference
fix inside the last ~1–2 ms (bracketed), or with σ_f held ≲ 4 Hz (ESP-PPB-class
disciplining) *and* no moving target on the path.

**3.7 The PSB estimator stack (parts E1–E4, `sweep_summary.csv`).** The hub
radiates bursts of K frames with a known carrier-phase ladder u_k; every
listener regresses y_k = φ_m + 2πf_m t_k + u_k + n_k.

- **E1:** intercept σ = σ_n/√K (K = 8 → 0.0177 rad); prediction σ to a sensing
  frame 1 ms after burst end = 0.106 rad (K = 8), 0.049 rad (K = 16) —
  Monte Carlo matches the closed form to ~0.5 %. Two K = 8 bursts averaged give
  the chain's 0.0747 rad fix.
- **E2 (what kills the fix):** a CFO-correction artefact constant per burst is
  absorbed into the intercept (harmless, 0.157 rad pred); random per-frame
  0.1 rad averages (0.233 rad); a **per-frame full rotation
  (U(−π, π)) is fatal — 3.84 rad**; int8-quantised CFO correction with a 100 µs
  unknown internal window costs only 0.108 rad. So the A1 requirement on any
  CFO-correction path: **no per-frame phase rotation**; constants and slow
  drifts are free.
- **E3 (RIPS-style q-range bearing, 20-face dodecahedron, r = 15 cm):**
  bearing CRLB 0.176° at σ_θ = 0.05 rad, R = 3 m; achieved RMS 0.19° among
  successes — but Gauss–Newton's basin is narrow: success 99.75 % with a 5°
  prior, 87 % at 10°, **55 % at the 17° FTM-scale prior, 5 % at 60°**. Range
  R barely matters (1 m and 8 m both ≈ 57 % at prior 17°). The q-range gives
  exquisite bearings **iff** something supplies a ≤ 10° prior (intra-puck coarse
  AoA or the previous fix — a tracking, not acquisition, mode).
- **E4 (ambiguity cascade, 13 × 5 MHz stitched + carrier phase):** carrier
  integers resolve with P ≥ 97 % at σ_φ = 0.02 rad, **83 % at 0.05 rad**,
  22 % at 0.30 rad; σ_τ̂ = 5.7 cm at 0.05 rad vs the 6.1 cm half-cycle at
  2.472 GHz. The cascade works at the design point but has little margin at
  2.4 GHz; at 5–6 GHz the half-cycle halves and the design point fails — the
  cascade is a 2.4 GHz result.

**3.8 The end-to-end chain (parts F–G, `e2e_summary.csv`, `e2e_figure.png`).**
4 anchors + hub puck in an 8 × 5 m room; self-survey from FTM prior (σ 1.5 m,
note 17) refined by two-way 13-channel stitched delay (σ 8 cm) + PSB q-range
bearing (0.18°), median-of-3 against integer slips; target localised by robust
Levenberg–Marquardt on two-way stitched ranges ± coherent phase differences
(2.1 mm differential noise at the 0.0747 rad fix):

| scenario | 3D RMS | P95 | horiz RMS | vert RMS |
| --- | --- | --- | --- | --- |
| S1 stitched ranges only (design) | 33.6 cm | 67.3 cm | 10.0 cm | 32.1 cm |
| **S2 S1 + coherent phase differences** | **23.7 cm** | **45.3 cm** | **7.3 cm** | **22.5 cm** |
| S2b S2 + bounded ±5 cm multipath bias | 25.3 cm | 46.8 cm | 7.8 cm | 24.1 cm |
| **S2c S2, perfectly surveyed anchors (coherence floor)** | **7.0 cm** | **13.4 cm** | **1.8 cm** | **6.7 cm** |
| S3 σ_φ = 0.30 rad: integers fail (22 %), delay-only 22 cm | 152.9 cm | 285.2 cm | 78.6 cm | 131.2 cm |
| S4 cascade dead (FTM-only) | 230.1 cm | 406.0 cm | 138.1 cm | 184.1 cm |

- Self-survey achieves **6.2 cm anchor RMS** — and it is the dominant term:
  S2c shows the coherence/estimation floor is 1.8 cm horizontal, so
  **~94 % of S2's horizontal error variance is self-survey, not coherence**.
- The vertical axis is the weak one everywhere (z-DOP of a ceiling-height
  anchor layout): 22.5 cm vs 7.3 cm horizontal in S2.
- Airtime (part G): PSB bursts 8 × 100 µs per 20 ms superframe = **4.0 % duty**
  (they double as sensing illumination); one two-way round per superframe =
  3.5 %; total **7.5 % at any N ≤ 20**, with two-way refresh every 80 ms
  (N = 4) to 400 ms (N = 20). Target coherence time 61.5 ms at 1 m/s > 20 ms
  superframe: OK.

### HYPOTHESIS (plausible, not established by these simulations)

- **H1 — The OU residual-CFO model at σ_f = 30 Hz, τ_c = 1 s represents a stock
  S3.** The class is motivated by ESP-PPB's < 10°-typical single-frame phase on
  C3 and ML-Track's 0.96 → 0.04 rad CFO cancellation, but no S3 measurement
  exists. If a stock S3's post-removal residual were 100 Hz *white* (τ_c → 0),
  brackets would still pass (0.004 rad at 1 ms, part A) — the model is benign
  in that direction; the dangerous direction (slow drift, τ_c ≫ 1 s, acts as a
  static bias) is absorbed by the bracket too. The model's *unfalsified* corner
  is heavy-tailed residual bursts.
- **H2 — Per-frame phase noise 0.05 rad at ≥ 6 dB SNR indoor.** MCRB says the
  floor allows it; real ESP32 CSI phase noise includes quantisation, AGC and
  residual sync artefacts that no CRB captures. E2's artefact scenarios bound
  the damage for the two benign classes; the fatal class (per-frame rotation)
  is a real risk in any closed-loop AFC path.
- **H3 — The turnaround-tail model (10 % of replies at 8 µs).** The deferral
  gate's keep-rates depend entirely on this invented tail; only the τ* lines
  (3.5) are model-free.
- **H4 — Slip model in the chain:** per-measurement integer-slip probability
  0.026 at the design point, with slips drawn as ±λ-class outliers and killed
  by median-of-3 / robust weighting. E4 bounds the per-shot rate at ≤ 17 %
  (σ_φ = 0.05) before median-of-3; the 0.026 figure is a modelling choice, not
  a derivation — the chain's S2 numbers are robust to it (S2b's ±5 cm multipath
  term moves 3D RMS by only 1.6 cm), but the slip *rate* on real silicon is
  unmeasured.
- **H5 — The PSB is legal and buildable:** ESP-NOW at 4 % duty in the 2.4 GHz
  ISM band with known-phase ladders per face, per-face VCTCXO pulling à la
  ESP-PPB or alternating conducted-calibrated faces. Mechanism is
  ESTABLISHED arithmetic; the *hardware* path is unproven on S3 (ESP-PPB is
  C3, unrefereed; nothing on S3).

### SPECULATION

- **SP1 — The gate closes the coherence-first/geometry-first split (bridge s/t
  confirmed in simulation).** At the design point the cross-node phase fix is
  good enough (0.075 rad at the sensing frame) that *derived* coherence from
  geometry costs nothing extra: the chain's mm-class differential observable
  improves horizontal RMS 10.0 → 7.3 cm despite self-survey dominance, and
  would reach 1.8 cm with a better survey. If the self-survey is refined to
  the q-range bearing's potential (sub-degree, repeatedly), the two camps'
  difference collapses to the self-survey problem — which is gauge-free
  anyway.
- **SP2 — A 2.4 GHz-only programme.** E4's 5–6 GHz fragility suggests the
  ambiguity cascade should be designed *for the channel you have*: 13 × 5 MHz
  at 2.4 GHz + carrier phase at the design σ_φ is already inside its integer
  basin with ~20 % margin; nothing wider is needed for cm-class *relative*
  positioning.
- **SP3 — Walker-on-reference-path as the real enemy.** D8 (0.257 rad) and the
  9.8 ms broadcast line (3.5) say the gate is threatened more by the scene
  moving through the reference path than by the oscillators. A reference
  beacon at 1.8 m ceiling height, mostly above walker height, may be the
  difference between PASS and FAIL in a lived-in room.

---

## 4. The falsifier

**Named falsifier for the whole note (one experiment):** conduct one hub and
two listeners from a common 40 MHz reference, play recorded/PSB-style reference
bursts at 20 ms cadence, and log per-packet cross-node phase residuals against
a wired-truth sensing frame. The simulation verdict —
**σ_φ ≤ 0.105 rad at 20 ms cadence with 1 ms brackets** — is falsified if the
measured residual exceeds **0.5 rad** with brackets in place and the reply
jitter under 30 ns; per 3.5, the specific killing values to instrument are:
bracket gap > 24.8 ms at 100 Hz residual (or > 55.7 ms at 30 Hz), uncorrected
reply jitter > 1.63 µs at 20 ppm, or a CFO-correction path that rotates per
frame (E2c). Sub-falsifiers: E4's integer cascade (P < 50 % at the design
σ_φ on wire); E3's bearing basin (success < 50 % with a 10° prior).

---

## 5. ESP32-S3 feasibility

- **No new silicon is required by the design point.** D1/D2 use stock-S3
  assumptions: 20 ppm crystal class, 56-subcarrier CSI, ESP-NOW bursts at
  100 µs, FTM available (max 40 MHz, ~4 m KPI per Espressif; accuracy measured
  on S2/C3 only), 12.5 ns-class RX timestamps, **no TX-timestamp readback
  outside FTM** (issue #9843) — hence the two-way jitter term must be handled
  by ns-stamp differencing inside FTM sessions or by the deferral gate, both
  simulated above.
- **The PSB hub needs per-face phase control:** ESPARGOS-pattern conducted
  reference (shipping product, S2-proven; S3 not demonstrated) or ESP-PPB-class
  VCTCXO discipline (C3, unrefereed). On S3 specifically, both are
  **unproven — this is the single hardware risk that matters** (evidence 2/10
  for S3 per `22-invention-g.md`'s grading of the same claim).
- **Compute:** the whole estimator stack (ladder regression, wrapped GN,
  LM localisation) runs in microseconds-per-frame class on the S3's
  240 MHz core; the simulations here are vectorised NumPy, and nothing in them
  needs more than O(K) per burst plus O(F) per bearing solve.
- **Airtime 7.5 %** at N ≤ 20 (part G) fits ISM duty norms with an order of
  magnitude to spare.

**TX needed?** Yes for the PSB hub (it is a transmitter by construction —
8 × 100 µs frames per 20 ms, 4 % duty); **no TX for the sensing-only listeners**
in the broadcast-fix design point D2 (all reference energy comes from the hub);
two-way exchanges (D1) need TX from every node at ≤ 3.5 % duty. A fully
receive-only fleet is possible only if *someone* transmits the reference — the
architecture concentrates TX in one hub puck. (Cf. vocabulary conflict:
"phase-coherent RF reference" is banned in `CONTEXT.md` — research-only use
here, flag stands.)

---

## 6. Evidence strength: **6 / 10**

- **For:** every number is computed, reproducible, and cross-checked
  (closed forms vs Monte Carlo agree to ≤ 0.5 %; two independent scripts agree
  on the design point: m2's D2 = 0.0563 rad vs sweep's E1-derived 0.0747 rad at
  the sensing frame — consistent once the 1 ms prediction factor is applied).
  The estimator stack is standard (LS line fit, wrapped Gauss–Newton, MCRB),
  and the two-way/jitter arithmetic is validated prior art (RTCSI WCNC 2023,
  ML-Track TMC 2025: 0.96 → 0.04 rad).
- **Against (deductions):** no device was measured — the OU residual model
  (σ_f = 30 Hz, τ_c = 1 s), the 0.05 rad per-frame noise, the turnaround tail,
  and the slip rate 0.026 are all assumptions bounded, not derived (H1–H4);
  the S3-specific hardware claims (per-face phase control, ESP-PPB-class
  discipline) rest on C3/S2 evidence and an unrefereed repo; multipath is a
  budget term, not a channel model. The chain's *relative* conclusions
  (self-survey dominates; coherence floor 1.8 cm horizontal) are more robust
  than its absolute centimetre figures.

---

## 7. Primary sources

- ESP-PPB — FTM-disciplined VCTCXO reference frames, < 1°/< 10° single-frame
  phase on ESP32-C3 (unrefereed): https://github.com/jonathanmuller/esp-ppb
- ESPARGOS — phase-coherent WiFi CSI, conducted reference (Euchner, Schneider,
  Gauger, ten Brink, "ESPARGOS: An Ultra Low-Cost, Open-Source …" and
  "ESPARGOS: Phase-Coherent WiFi CSI Datasets…"): https://espargos.net
- ESP-SDR — raw I/Q on ESP32, S3 continuous decimated mode:
  https://espargos.net/espsdr ; https://espargos.net/news/iq-sampling-and-sdr-mode
- ML-Track — multi-link RTCSI tracking, phase error 0.96 → 0.04 rad, TMC 2025,
  DOI: 10.1109/TMC.2025.3529897
- RTCSI — round-trip CSI, reply-interval std < 1 µs on Raspberry Pi CM4, Shi
  et al., IEEE WCNC 2023; UCL Discovery eprint 10161602:
  https://discovery.ucl.ac.uk/id/eprint/10161602
- WiDRa — sum-carrier-phase differential ranging, JSAC 2024,
  arXiv:2405.12168: https://arxiv.org/abs/2405.12168
- LoSRef — the LoS path as an OTA delay+phase reference, arXiv:2602.05344:
  https://arxiv.org/abs/2602.05344
- ESP-IDF, *Wi-Fi (esp_wifi) API Reference — ESP32-S3* (TSF, FTM fields, RX
  timestamp; no TX timestamp outside FTM, issue #9843):
  https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/api-reference/network/esp_wifi.html
- ESP-IDF FTM example (raw-RTT jitter class): https://github.com/espressif/esp-idf/blob/master/examples/wifi/ftm/README.md
- IEEE 802.11bf-2025 (WLAN sensing; procedure only, no cross-node coherence).
- Round-1 corpus parameters: FTM σ 1.5 m and timestamp register detail
  (`17-time-frequency-precision.md`); strobe/burst cadence
  (`22-invention-k.md`); two-way product physics and S3 evidence grading
  (`22-invention-g.md`, `22-invention-s.md`); 8 cm stitched-delay prior
  (`22-invention-m.md`).

---

## 8. Residual unknowns

1. **The S3's actual residual-CFO process** (σ_f, τ_c, tails) — H1; the one
   measurement that would upgrade this note's verdict from "simulation verdict"
   to "measurement".
2. **Per-frame CSI phase noise on S3** at indoor SNRs, and whether the AFC loop
   imposes the fatal per-frame rotation of E2c.
3. **Reply-instant statistics of the S3 Wi-Fi driver** (deferral-gate keep
   rates); FTM-session timestamps vs generic TX.
4. **Per-face phase control on S3** (conducted ESPARGOS pattern or VCTCXO
   discipline) — the PSB hub's enabling hardware.
5. **Self-survey to < 6 cm in practice** — q-range bearings need their ≤ 10°
   acquisition prior (E3); how it is first obtained is open (intra-puck coarse
   AoA assumed, not simulated).
6. **Multipath bias statistics** — entered as ±5 cm; a ray-traced or measured
   distribution would sharpen S2b.
7. Room-frame alignment (Procrustes) outside the chain simulation — anchor
   *self*-survey is simulated, house-frame placement is not.

---

## 9. Chaining / existing models and data / own data

### (A) Chaining

- Consumes: Finding 4's ledger quantities (σ_φ, B, ambiguity cascade, airtime,
  target coherence time) — all now carry simulated values; the two-way product
  physics of `22-invention-g.md`/`22-invention-s.md`; note-k's strobe cadence
  (the PSB degrades to it when the ladder is unknown); note-17's FTM/timestamp
  register facts.
- Feeds: any Track-3/4 invention needing a reference frame ≤ 1 ms old at 20 ms
  cadence; the synthesis can now compare coherence-first vs gauge-free on the
  same numbers (S2 vs S2c: coherence buys 7.3 → 1.8 cm horizontal *if* the
  survey is solved — the survey, not coherence, is the binding constraint).
- Gates: the 0.5 rad gate passes with margin at the design point **and fails
  without brackets**; any graduation proposal must state its bracket gap and
  reply-jitter handling against 3.5's killing values.

### (B) Existing models and data

- ESPARGOS datasets/pyespargos: conducted-reference CSI with known geometry —
  the natural first validation corpus for the bracket law and the ladder
  regression (no S3 needed for the estimator checks).
- ESP-PPB (C3): reference-frame rhythm and phase-quality priors for H1; needs
  independent verification before any S3 claim inherits it.
- RTCSI/ML-Track: validates the two-way residual model (0.96 → 0.04 rad) and
  the < 1 µs reply datum used in part B.
- The repo's own deterministic simulator (`cargo run -p aquila-capture --
  synth`, needs `nix develop .#sim`) can generate ground-truth CSI for the
  chain's stage-3 estimator — not used here (Python-only scope), flagged as
  the next validation step.

### (C) Own data / new technology

- **Minimal own experiment (the falsifier of §4):** two S3 devkits + one hub
  puck (or ESPARGOS One), conducted split of one 40 MHz reference, PSB bursts
  at 20 ms cadence, log σ_φ(gap) for gaps 0.1–100 ms. Expected from this note:
  holdover curve crossing 0.5 rad at 2.65 ms (30 Hz model); bracketed curve
  flat at ≤ 0.11 rad to at least 20 ms. Any measured curve distinguishes the
  models *and* measures σ_f, τ_c, and the reply-tail in one sitting — it is
  the cheapest single experiment that closes the most unknowns (1)–(3).
- New technology if graduating: per-face phase-control hardware (ESPARGOS
  pattern on S3), glossary/ADR change for the banned vocabulary, and the PSB
  as the v2 reference-frame protocol.
