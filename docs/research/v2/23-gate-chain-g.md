# 23 — The coherence gate as one end-to-end chain: brackets, the two-way product, the ambiguity cascade, integer resolution and 3-D position (track T1, direction g)

Status: round-2 research note. **Every number below is a simulation** (NumPy/SciPy
models of estimators) or a closed form evaluated numerically — no device was
touched, and a simulation is ESTABLISHED only for the mathematics it evaluates,
not for the hardware. Australian English. Code:
`docs/research/v2/code/gate-chain-g/gate_chain_g.py`; key numbers:
`docs/research/v2/code/gate-chain-g/gate_chain_summary.csv` (159 rows, parts
A–F); figure: `docs/research/v2/code/gate-chain-g/fig_gate.png`.

---

## 1. The question

Round 1 left one family of numbers between "coherent" and "not coherent"
(`ROUND2-BRIEF.md` Finding 4). The sibling T1 notes (f, h, b, a) each measured
one facet in simulation. This note's distinct contribution is to run the **whole
chain in one model, at packet level**, and report where each stage binds:

> **Under the best-supported ESP32-S3 parameter ranges, does the per-frame
> cross-node phase residual stay ≤ 0.5 rad at the 20 ms working cadence — with
> what margin — and does the σ_φ that passes the gate actually carry a target
> through integer ambiguity resolution to a 3-D position?**

Stage list (all simulated): (A) bracketed coherence at the sensing frame +
per-exchange two-way product residual; (B) the Kalman/bridge tracking layer
(closed forms, Wolfram cross-checked); (C) the ambiguity cascade (FTM ppm seed →
dual-LTF within-frame probe → burst unwrap); (D) products → RTK-style integer
resolution → 3-D position (CRLB + Monte Carlo); (E) reference types under
motion and the RIPS beat bound; (F) falsifier power analysis + the airtime
ledger.

## 2. Model and parameters (the honesty block)

Shared by all parts unless stated. Monte-Carlo seed **20261016**
(`numpy.default_rng`).

| Parameter | Value | Provenance / note |
| --- | --- | --- |
| Carrier | f_c = 2.437 GHz (channel 6), λ = 12.297 cm | round-1 note 12 |
| Raw relative CFO Δf_raw | 24.4 / 48.7 / 97.5 kHz (10/20/40 ppm crystal pairs, opposite signs) | round-1 notes 12, 17 |
| Residual CFO (after per-frame removal) | Ornstein–Uhlenbeck, τ_c = 1 s, σ_f ∈ {3, 10, 30, 100, 300, 1000} Hz | swept; 10–100 Hz = MegaMIMO-class post-CFO residual (`00-synthesis.md` §2) |
| Bracket cadence T_c | 0.7 / 1 / 2 / 5 / 10 / 20 ms | 20 ms = round-1 update-rate optimum |
| Per-frame CSI phase noise | σ_m = 0.0442 rad (N_s = 64 subcarriers at 6 dB SNR); ratio of two frames √2·σ_m = 0.0625 rad | MCRB form |
| Frame averaging | n_avg = 4 frames per datum | |
| Reply interval | Δ0 = 350 µs (0.7 ms RTT, fixed MCS7-SGI) | |
| Reply-instant jitter σ_τ | 0.1 / 0.33 / 1 / 3 / 10 µs sweep | 0.33 µs ≈ WCNC 2023 RPi reply std < 1 µs |
| RX timestamp grid | 12.5 ns | ESP-IDF TSF (ESTABLISHED) |
| Within-frame CFO probe | dual-LTF separation τ_LTF = 8 µs → ±62.5 kHz unambiguous | HT preamble structure |
| Room (part D) | N = 4/6/8 nodes on a 5.6 × 3.6 m rectangle at 2.3 m height; target (3.2, 1.9, 1.0) m | |
| MC sizes | bridge 4 × 10⁵; cascade 10⁵ per point; integer resolution 200 positions × 2 × 10⁴ false candidates | |
| Frame (airtime) | 300 µs conservative (preamble + payload + turnaround) | |

Multipath and the static field are **not** physically simulated: part D assumes
the static field is perfectly removed (best case) and carries calibration error
σ_cal as an explicit knob. FDTD/GNU Radio were not provisioned; nothing below
pretends otherwise.

## 3. Findings

### 3.1 ESTABLISHED (the mathematics these simulations evaluate)

**The gate PASSES at the 20 ms working cadence with brackets, with 1.35–3.8×
margin** (part A, "bracketed coherence" ledger quantity):

| σ_φ at the bracketed sensing frame [rad] (20 ppm, σ_τ = 0.33 µs, 4-frame avg) | T_c = 1 ms | T_c = 5 ms | **T_c = 20 ms** |
| --- | --- | --- | --- |
| residual 3 Hz | 0.075 | 0.075 | **0.076** (6.6×) |
| residual 10 Hz | 0.075 | 0.075 | **0.083** (6.0×) |
| residual 30 Hz | 0.075 | 0.076 | **0.132** (3.8×) |
| residual 100 Hz | 0.075 | 0.087 | **0.370** (1.35×) |
| residual 300 Hz | 0.076 | 0.155 | 1.091 — **FAIL** |
| residual 1000 Hz | 0.085 | 0.460 | 3.628 — FAIL |

Two structural results sit behind the table:

- **Brackets beat tracking, and the floor is the measurement noise.** The
  bracket interpolator's error has the closed form σ = √(a/48 + r/2)
  (a = (2π)²·q_f·T_c³ the OU-driven process noise, r the datum variance),
  Wolfram-DARE-checked at the hardest ladder point (a = 3.553 × 10⁻⁵ →
  p₁₁ = 0.00292 rad² analytic vs 0.0029197 simulated) and MC-verified to
  ≤ 0.5 % at three points (e.g. a = 0.05, r = 0.01: 0.07773 closed vs 0.07778
  MC). The 1 ms rows are flat in σ_f because r/2 dominates — at fast cadence
  the gate is set by per-frame CSI noise, not oscillator wander.
- **The per-exchange product residual is cadence-independent** (part A,
  second ledger quantity). σ_φ^prod at 20 ppm, 30 Hz residual, 4-frame avg:
  0.038 rad (σ_τ = 0.1 µs), **0.103 rad (0.33 µs)**, 0.307 rad (1 µs),
  0.919 rad (3 µs), 3.06 rad (10 µs) — identical at T_c = 1 ms and 20 ms to
  five digits. The product's only cadence-dependent term, the corrected
  shared-CFO × reply-interval term 2π·f_post·Δ0, is negligible: the tracked
  posterior residual CFO is 0.0025 Hz (1 ms cadence) or 0.067 Hz (20 ms),
  giving ≤ 1.5 × 10⁻⁴ rad (part B). **The two ledger quantities must be
  reported separately**: coherence at the sensing frame improves with faster
  brackets; the product noise does not.

**Unbracketed stock hardware fails** (part A, OU holdover rms
2π·σ_f·T/√3 over 20 ms): 0.218 rad (3 Hz), **0.726 rad (10 Hz — FAIL)**,
2.18 rad (30 Hz), 7.26 rad (100 Hz). The unbracketed kill sits near
σ_f ≈ 7 Hz under this model (see §10 for the √3 bookkeeping difference vs the
siblings' 3.98 Hz).

**Killing values, stated explicitly** (single-term crossings, part A):

| killer | crossing | where it binds |
| --- | --- | --- |
| Reply-instant jitter σ_τ (20 ppm) | **> 1.63269 µs** for 0.5 rad; > 0.327 µs for the 0.1 rad sharp bar | the product, always |
| same at 10 ppm / 40 ppm | 3.265 µs / 0.816 µs | 40 ppm-class parts fail 2× sooner |
| Bracketed residual-CFO wander | σ_f > **137.8 Hz** at T_c = 20 ms; > 27.6 Hz for 0.1 rad | coherence at the sensing frame |
| same at T_c = 1 ms | 12 328 Hz (never binding) | fast brackets make wander irrelevant |
| Unbracketed holdover | σ_f ≈ 7 Hz (0.5 rad at 20 ms) | no-reference operation |
| Non-reciprocal motion (1 m/s walker) | reference-to-sensing gap > **≈ 4.9 ms** (bias 2π·f_D·T crosses 0.5 rad; f_D = 16.26 Hz two-way) | exchange-internal timing; 0.7 ms gap costs only 0.072 rad, 20 ms costs 2.04 rad |
| Airtime | star N = 10 at T_c = 5 ms → **108 %** duty | cadences below 5 ms break the channel at home scale (part F) |
| Cascade burst spacing | T_b > 2 ms → unwrap success < 58 % | stage-2 CFO unwrap (part C) |

**The ambiguity cascade resolves, but only with sub-ms reference bursts**
(part C). Dual-LTF probe: per-frame CFO σ = 1246 Hz, unambiguous range
±62.5 kHz — comfortably wider than the 48.7 kHz raw CFO at 20 ppm; the FTM ppm
seed's worst error (1200 Hz at 1 ppm resolution) is 19× inside the probe range,
so **the seed always resolves the probe branch**. Averaging K frames:
623 Hz (4), 312 Hz (16), 156 Hz (64). Stage-2 burst-unwrap success (stage-1
error after 16 frames + OU wander over a 20 ms cycle inside |Δf| < 1/2T_b):

| burst spacing T_b | 0.7 ms | 1 ms | 2 ms | 5 ms | 20 ms |
| --- | --- | --- | --- | --- | --- |
| success, σ_f = 30 Hz | **97.9 %** | 89.1 % | 57.8 % | 25.1 % | 6.4 % |
| success, σ_f = 100 Hz | 97.8 % | 89.2 % | 57.6 % | 25.4 % | 6.4 % |

Insensitive to σ_f (stage-1 dominates); **binding requirement: bursts at
≤ 1 ms**, decoupled from the 20 ms sensing cadence. This is the cascade stage
no sibling quantified.

**The chain: σ_φ → integers → position** (part D; static field ideally
removed, σ_cal explicit).

- 3-D position CRLB (trace) over all node pairs: at σ_φ = 0.3 rad,
  **5.9 mm (N = 4) / 3.2 mm (N = 6) / 2.6 mm (N = 8)**; at 0.5 rad,
  9.8 mm (N = 6). A lower bound — it says coherence, once bought, is cheap.
- RTK-style integer resolution (position-candidate χ² test, 1.5 m prior,
  σ_cal = σ_φ): **95 % at σ_φ = 0.1 rad (N = 6)**; 39.5 % at 0.3 (N = 6),
  rising to **90.5 % at N = 8**; **0 % at N = 4** (expected false candidates
  4.67 — the array is too small to vote integers down) and 0 % at σ_φ = 0.5.
  Absolute carrier positioning needs σ_φ ≲ 0.1 rad *and* N ≥ 6; the 0.3–0.5 rad
  designs that pass the gate do **not** resolve integers in absolute mode.
- Differential (motion) mode needs no integers: per-link range-sum precision
  λ·σ_φ/2π = **2.0 mm (0.1 rad) / 5.9 mm (0.3) / 9.8 mm (0.5)**.

**Falsifier power analysis** (part F, χ² variance test, α = 0.05, power 0.90):
discriminating σ_φ = 0.1 vs 0.3 rad takes **5 windows** (100 ms); 0.1 vs 0.5 →
4 windows; 0.05 vs 0.1 → 11 windows (220 ms); 0.3 vs 1.0 → 5 windows. The
hardware round can settle the gate in well under a second of exchanges; the
binding cost is instrumentation, not dwell time.

**Airtime ledger** (part F, 300 µs frames, star = 1 ping + N−1 replies × 2
brackets): duty at T_c = 20 ms: **15 % (N = 6) / 27 % (N = 10) / 57 % (N = 20)**;
at 5 ms: 60 / 108 / 228 %; at 1 ms: 300 / 540 / 1140 %. All-pairs at 20 ms:
45 % (N = 6), 135 % (N = 10). Working point confirmed: **20 ms star is the only
home-scale regime; sub-5 ms cadences and all-pairs meshes are off the air**.

**Coherence budget B** (part F ledger rows): B = Δν·T_obs = 0.021 cycles
(30 Hz × 0.7 ms exchange), 0.07 (100 Hz × 0.7 ms), 0.6 (30 Hz × 20 ms), 2.0
(100 Hz × 20 ms). Gate ⇔ **B ≤ 0.0796 cycles** — the fourth independent
derivation of the same threshold (h, b, a, now g).

**Reference-type bounds** (part E): target coherence time λ/2v = **61.5 ms at
1 m/s** (the 20 ms cadence oversamples 3×); motion bias vs gap 0.072 / 0.102 /
0.511 / 2.043 / 6.23 rad at 0.7 / 1 / 5 / 20 / 61 ms. RIPS beat-phase CRB:
0.0198 rad at K = 256, 10 dB (MC 0.0140 — estimator within 1.4× of CRB), down
to 0.0011 rad at K = 8 × 10⁴; range resolution 3.75 m per 40 MHz epoch, 1.80 m
stitched (83.5 MHz); product-phase ambiguity λ = 12.3 cm (monostatic-equivalent
6.15 cm).

### 3.2 HYPOTHESIS (plausible, not established here)

- The OU residual-CFO model with τ_c = 1 s is a convenient parameterisation of
  an unmeasured process; the *device* σ_f on S3 after per-frame CFO removal is
  unknown (siblings sweep 2.4–100 Hz; this note sweeps to 1000 Hz to find the
  cliff).
- The 0.33 µs reply-jitter design point extrapolates a Raspberry Pi WCNC 2023
  datum to S3; the real S3 software-turnaround distribution (Laplace body +
  scheduler tail) is unmeasured. The product's insensitivity to cadence means
  this single number governs the two-way route.
- 4-frame averaging is assumed to deliver the full √4 within a coherence
  window (consistent with b's √N-within-window law; a's β ≤ 0.08 face law says
  faces do *not* average — this note averages frames on one link, a different
  quantity).
- 300 µs frames at MCS7-SGI; if real frames are a's 130 µs, all duties divide
  by ~2.3.

### 3.3 SPECULATION

- If the dual-LTF probe can be run every burst at 0.7 ms, the residual CFO
  seen by the tracker may approach the ESP-PPB ppb class, collapsing the
  coherence ledger to the measurement floor (~0.075 rad) at every cadence.
- The 90.5 % integer success at N = 8, σ_cal = 0.3 suggests adding low nodes
  (floor-level) buys more than per-link SNR; a fleet-design rule worth testing.

## 4. Decisive numbers (shared ledger, acceptance criteria 2 and 5)

| quantity | value | provenance |
| --- | --- | --- |
| **σ_φ at 20 ms cadence, bracketed, 30 Hz residual** | **0.132 rad — PASS, 3.8× margin** (0.370 rad at 100 Hz — 1.35×; 1.09 rad at 300 Hz — FAIL) | SIMULATION, part A, seed 20261016 |
| σ_φ per two-way exchange (product) | 0.103 rad at σ_τ = 0.33 µs, 20 ppm — cadence-independent | SIMULATION |
| σ_φ unbracketed, 20 ms | 2.18 rad at 30 Hz — **FAIL** | SIMULATION (closed form) |
| Coherence budget B | gate ⇔ **B ≤ 0.0796 cycles**; design point B = 0.021 (30 Hz × 0.7 ms) | closed form, 4th confirmation |
| Killing values | **σ_τ > 1.63 µs** (20 ppm); σ_f > 137.8 Hz bracketed / ≈ 7 Hz unbracketed at 20 ms; motion gap > 4.9 ms at 1 m/s; burst spacing > 2 ms; star duty > 100 % below 5 ms at N ≥ 10 | SIMULATION / arithmetic |
| Airtime (300 µs frames) | **15 % (N = 6) / 27 % (N = 10) / 57 % (N = 20)** at 20 ms; 5 ms cadence: 60/108/228 % | arithmetic |
| Ambiguity cascade | FTM seed (≤ 1200 Hz) ≪ dual-LTF probe (±62.5 kHz); K = 16 → 312 Hz; **burst unwrap ≤ 1 ms required** (89–98 %) | SIMULATION |
| Chain payoff | CRLB 3.2 mm (N = 6, σ_φ = 0.3); integers: 95 % at σ_φ = 0.1 (N = 6), 0 % at N = 4 or σ_φ ≥ 0.5; differential 5.9 mm/link at 0.3 rad | SIMULATION (CRLB + MC) |
| Target coherence time | 61.5 ms @ 1 m/s; motion bias 0.072 rad over the 0.7 ms exchange | closed form |
| Falsifier dwell | 4–11 windows (80–220 ms) per discrimination at α 0.05, power 0.90 | SIMULATION |

**The named falsifier** — the same two-board M2 rig the siblings converged on,
now with a power spec: one ESP32-S3 pings, one replies; brackets at 1 ms around
a 20 ms sensing gap; conducted split of a common reference as ground truth;
≥ 4 × 10⁴ exchanges (≈ 5 windows at the 0.1-vs-0.3 discrimination). The chain
**dies if any of**: (i) bracketed σ_φ > 0.5 rad at 20 ms; (ii) residual reply
jitter after timestamp correction > 1.63 µs-equivalent; (iii) burst spacing
cannot reach ≤ 1 ms, so the cascade's stage-2 unwrap stays below ~60 %;
(iv) at N ≤ 6 nodes the integer-resolution success stays 0 because σ_φ or
σ_cal ≥ 0.3 rad — relegating the system to differential mode. Each is one
scalar on a CSV row.

## 5. ESP32-S3 feasibility

- **Compute:** every stage simulated is a few kHz of scalar arithmetic (phase
  unwrap, χ² tests, a 2-state KF) — trivial on the S3's 240 MHz cores. The
  dual-LTF probe needs HT-preamble CSI access, which the ESP32 CSI toolkit
  exposes.
- **Timing registers:** the 12.5 ns RX TSF grid is ESTABLISHED; there is no TX
  timestamp outside FTM (ESP-IDF; upstream issue #9843) — hence the product's
  dependence on σ_τ and the necessity of the reply-interval correction path.
- **Frame rhythm:** 1 ms brackets + a 0.7 ms-burst cascade + 20 ms sensing fits
  the 15 % star duty at N = 6 (27 % at N = 10); sub-5 ms sensing cadences do
  not fit the channel and are not needed against the 61.5 ms target coherence
  time.
- **Unknown on S3:** whether software turnaround holds σ_τ ≤ ~0.3 µs with a
  deferral gate, and the true post-CFO σ_f. Both are one-experiment numbers.

## 6. TX needed?

Yes for the two-way product route — the product needs each node to transmit
replies (that *is* the reference supply); this is ESP-NOW-class traffic, not
high power. The beacon/RIPS alternatives avoid per-node TX timing but buy
carrier-phase blindness (h's part E; the beat CRB here is favourable, 0.02 rad,
but delivers no per-link carrier phase). A single always-on reference TX with
one-way brackets would remove the σ_τ kill but forfeits the two-way
asynchrony cancellation that RTCSI validated.

## 7. Evidence strength: **6 / 10**

Same tier as the sibling T1 notes, for the same reasons, plus two offsets.
Up: five independent derivations of the jitter kill (1.63269 µs here vs 1.63 in
f, h, b, a), a Wolfram-DARE cross-check of the tracking closed form, MC
verification of the bridge law to ≤ 0.5 %, and a falsifier power analysis the
others lack. Down: the chain stage (part D) assumes ideal static-field removal
and carries multipath only as σ_cal; the CRLB is a lower bound and the
integer-resolution MC is small (200 positions); and every device parameter
(σ_f, σ_τ, σ_cal, frame time) is literature-anchored, not measured. Nothing
here is a measurement.

## 8. Primary sources

- ESP-PPB — FTM-disciplined VCTCXO reference frames, < 1°/< 10° single-frame
  phase on ESP32-C3 (unrefereed; source of the ppb-class anchor):
  https://github.com/jonathanmuller/esp-ppb
- ESPARGOS — conducted phase-coherent CSI pattern: F. Euchner et al.,
  arXiv:2502.09405, https://doi.org/10.48550/arXiv.2502.09405; datasets
  https://doi.org/10.18419/darus-4456; https://espargos.net
- ESP-SDR — raw I/Q, S3 continuous decimated mode: https://espargos.net/espsdr
- RTCSI — round-trip CSI cancels asynchrony (0.96 → 0.04 rad), reply std
  < 1 µs on RPi CM4, Shi et al., IEEE WCNC 2023; UCL Discovery eprint 10161602:
  https://discovery.ucl.ac.uk/id/eprint/10161602
- ML-Track — multi-link RTCSI tracking, IEEE TMC 2025,
  DOI: 10.1109/TMC.2025.3529897
- WiDRa — sum-carrier-phase differential ranging, JSAC 2024,
  https://arxiv.org/abs/2405.12168
- LoSRef — LoS path as OTA delay+phase reference, https://arxiv.org/abs/2602.05344
- AirSync — Kalman OTA phase tracking, IEEE/ACM ToN,
  https://doi.org/10.1109/TNET.2012.2230449 (residual-CFO ladder provenance)
- ESP-IDF, *Wi-Fi (esp_wifi) API Reference — ESP32-S3* (12.5 ns TSF grid; no TX
  timestamp outside FTM, issue #9843):
  https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/api-reference/network/esp_wifi.html
- Internal: `ROUND2-BRIEF.md` Finding 4; `22-invention-{g,s,t}.md`; sibling T1
  notes `23-coherence-gate-{f,h,b,a}.md`.

## 9. Residual unknowns

1. S3 reply-jitter distribution (body and tail) after any deferral gate — the
   single number that decides the product route.
2. Post-CFO residual σ_f on S3 (and its τ_c) — decides the bracket cadence
   actually needed; this note shows ≤ 100 Hz suffices at 20 ms.
3. Whether the dual-LTF probe is accessible via the stock CSI API on S3
   (HT-LTF fields) or needs ESP-SDR.
4. Per-packet phase artefact σ_art (assumed 0 here); a's face-averaging law
   suggests it may not average.
5. Multipath-induced σ_cal of the static-field subtraction — part D's binding
   knob for integers; needs the repo's `aquila-capture synth` ray model.
6. Real frame time on S3 (300 µs assumed; a's 130 µs would halve all duties).

## 10. Cross-reconciliation with the f / h / b / a sibling notes

- **The gate at 20 ms: PASS confirmed, and g brackets the spread from above.**
  Sibling consensus σ_φ = 0.056 (f, broadcast + 1 ms brackets) / 0.385 (h, best
  fenced) / 0.164 (b, ±0.5 ms brackets at β = 4 Hz) / 0.318 rad (a,
  unbracketed 1 ppb). g's ladder spans the same range with the same
  explanation: **reference supply, not hardware**. g at 20 ms: 0.076 rad
  (3 Hz residual — the ppb-class case; cf. a's 0.318 unbracketed, f's 0.056
  bracketed), 0.132 (30 Hz), 0.370 (100 Hz — the MegaMIMO-class worst case;
  cf. h's 0.385 fenced). Unbracketed, g fails at σ_f ≥ 10 Hz (0.73 rad) — the
  same unanimous "stock unfenced fails" verdict as f (4.28 rad), h (12.57 rad),
  b (holdover fails for β ≥ 4 Hz), a (1.20–12.7 rad).
- **The jitter kill: now confirmed five times.** g's single-term crossing
  0.5/(2π·Δf) gives **1.63269 µs at 20 ppm** — identical to h's and b's 1.63 µs
  and a's 1.633 µs by three independent routes; 3.265 µs at 10 ppm and
  0.816 µs at 40 ppm are new ladder points.
- **Unfenced σ_f kill: a bookkeeping difference, not a disagreement.** h/a
  quote **3.98 Hz** from B = σ_f·T ≤ 0.0796 cycles (pure linear drift,
  σ_φ = 2πσ_f·T); g's OU-increment rms carries a √3 (σ_φ = 2πσ_f·T/√3),
  giving ≈ 6.9 Hz, and f's unbracketed table (3.744 rad at 30 Hz/20 ms) matches
  the pure-linear form. Same model family, factor-√3 convention; the shared
  ledger should carry the label (g: OU rms; h/a/f: linear). Both agree stock
  residuals (10–100 Hz) are 3–20× over the line.
- **B ≤ 0.0796 cycles: quadruple-confirmed** (h, b, a, g).
- **The cascade: g numbers the stage the others left closed.** b resolved the
  cascade's *bias* constraint (stitched-delay bias ≥ ~3 cm; a's MC brackets it
  at 1.67 cm → 93.5 % vs 5.27 cm → 44 %, ~70 % at 3 cm — consistent with b's
  threshold). g adds the *noise* stage: the dual-LTF probe (±62.5 kHz) always
  resolves the FTM seed, but stage-2 unwrap needs **bursts ≤ 1 ms** (89–98 %)
  and dies at 20 ms spacing (6.4 %). The two constraints are orthogonal and
  both mandatory.
- **The chain: g explains why a's payoff looks the way it does.** a reports
  σ_φ = 0.3 rad → 2.0 cm 3-D RMSE (MC, full estimator); g's CRLB at the same
  σ_φ is 3.2 mm (N = 6) — the gap is estimator realisation and a's
  static-field/bias budget, as expected for a bound. g's integer-resolution MC
  adds the structural point neither reported: **absolute carrier positioning
  needs σ_φ ≲ 0.1 rad and N ≥ 6** (N = 4 gives 0 % with 4.7 expected false
  candidates), so the 0.3–0.5 rad designs that pass the coherence gate live in
  differential mode (5.9 mm/link at 0.3 rad).
- **Motion: consistent and complementary.** g's 61.5 ms target coherence time
  and gap law (0.072 rad at the 0.7 ms exchange; 0.51 rad at 5 ms) match the
  ledger values used by h and b; g's new contribution is the exchange-internal
  *non-reciprocity* crossing at ≈ 4.9 ms, well above the 0.7 ms design gap.
- **Airtime: g is the most conservative and still passes.** 300 µs frames vs
  a's 130 µs and h's 250 µs; star duty 15 % (N = 6) at 20 ms sits between a's
  0.5–2.6 % (bracket/strobe, 8 nodes) and h's 5 % fence duty, and agrees with
  b's ≤ 13 % at N = 20 once frame length is normalised. Unanimous conclusion:
  20 ms star fits, sub-5 ms cadences and all-pairs meshes do not.
- **Averaging:** g's n_avg = 4 frames follows b's √N-within-window law; a's
  β ≤ 0.08 face law concerns cross-face averaging, a different quantity — no
  conflict, but the ledger should record *which* N averages.
- **Vocabulary conflict (flagged, per the brief):** brackets, fences, the
  two-way product and the RIPS beat all live in the space `CONTEXT.md`
  currently bans ("phase-coherent RF reference", ESP-NOW, Doppler, raw I/Q);
  any graduating invention needs the glossary change and an ADR, as the
  siblings also flag.

## 11. Chaining / existing models and data / own data

### (A) Chaining

This note is literally a chain: bracket law (f, b) → product residual
(f, a) → cascade (b) → **integer resolution + position (new)** → airtime +
power analysis (new). Inputs consumed from siblings: the 0.0796-cycle budget
(h), the bracket-vs-tracking comparison (f), the 3 cm stitched-bias threshold
(b, a), the 1.63 µs jitter identity (h/b/a). Outputs offered to the next
synthesis: the cadence-independence of the product ledger; the ≤ 1 ms burst
requirement; the N ≥ 6 / σ_φ ≤ 0.1 rad integer-resolution boundary; the
4–11-window falsifier dwell; the 40 ppm jitter ladder point.

### (B) Existing models and data

OU residual-CFO process (τ_c = 1 s) — the common round-2 model; MCRB
per-subcarrier phase noise at N_s = 64; ESP-IDF 12.5 ns TSF grid; WCNC 2023
reply-jitter datum (< 1 µs RPi); ESP-PPB ppb-class anchors (unrefereed);
Wiener-vs-OU and linear-vs-rms bookkeeping differences documented in §10. The
repo's deterministic simulator (`cargo run -p aquila-capture -- synth`) was
**not** used here — the channel is analytic; it is the right next step for
σ_cal (multipath bias into the static field), which this note carries as a knob.

### (C) Own data / new technology

None — this note produces no measurements. All 159 CSV rows are simulations or
closed forms at the stated parameters, reproducible with
`/home/matt/projects/aquila-v2-spec/sim-python gate_chain_g.py` from
`docs/research/v2/code/gate-chain-g/`. The figure
(`fig_gate.png`: gate table vs cadence; position CRLB vs σ_φ; motion bias vs
gap) is generated by the same run.
