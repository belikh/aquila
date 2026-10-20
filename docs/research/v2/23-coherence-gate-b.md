# 23 — The coherence gate: bracketed references, the Kalman tracker and the end-to-end cascade (track T1, direction b)

Status: round-2 research note (track T1, agent **b**). Australian English.
Companion to `23-coherence-gate-f.md` (reference-supply design) and
`23-coherence-gate-h.md` (fence tracking); cross-reconciled in §8. Research is
out-of-band; it never defines v1 scope.

**Everything numeric in this note is a SIMULATION, not a measurement.** Code:
`docs/research/v2/code/t1_gate_b.py` (reproducible; run with
`/home/matt/projects/aquila-v2-spec/sim-python docs/research/v2/code/t1_gate_b.py`).
All scalar results: `docs/research/v2/code/t1_gate_b_results.csv` (177 rows).
Figures: `docs/research/v2/figures/23-t1-b-fig1-gate.png`,
`23-t1-b-fig2-kalman.png`, `23-t1-b-fig3-twoway.png`,
`23-t1-b-fig4-averaging.png`, `23-t1-b-fig5-cascade.png`.

---

## 1. The question

`ROUND2-BRIEF.md` Finding 4 gates the whole programme on one number: **σ_φ,
the per-packet cross-node carrier-phase residual after CFO removal at MAC
cadence**, with targets ≤ 0.3–0.5 rad. Round 1 never measured it; round 2 must
simulate it to the register level and state the killing parameter values. This
note asks, for a few-board ESP32-S3 fleet:

1. With a **bracketed reference** (reference frames at ±T/2 around each sensing
   frame) instead of one-sided holdover, what σ_φ is achievable at the 20 ms
   working cadence, and with what margin against the 0.5 rad gate?
2. Can a **Kalman tracker** on [phase, frequency] substitute for bracketing?
3. What does the **TX-instant jitter × raw CFO** term cost, and when does it
   kill the two-way product?
4. Does **face/packet averaging** follow √N, and what is the optimal split
   between averaging depth M and cadence?
5. Does the **ambiguity cascade** (FTM → stitched delay → carrier phase)
   survive the resulting σ_φ, and what actually breaks it?
6. What does all this cost in **airtime**, and can a stock S3 carry it?

---

## 2. Model and parameters (the honesty block)

Oscillator-level Monte-Carlo plus closed forms, evaluated with NumPy/SciPy.
**No device was touched; every number below is a property of the model, not of
hardware.**

| Parameter | Value | Provenance |
| --- | --- | --- |
| Carrier | 2.437 GHz (channel 6 centre) | round-1 note 12 |
| λ | 12.30 cm | derived |
| β_rel ladder (effective *relative* Lorentzian linewidth of the oscillator pair) | 0.05 / 0.5 / 4 / 40 Hz | 0.05 = ESP-PPB "<1° best" implied; 0.5 = ESP-PPB "<10° typical" implied; 4 = free-running optimistic (round-1 "10 Hz residual" reading under the Wiener map); 40 = MegaMIMO-class post-CFO residual ("100 Hz → π in 20 ms") |
| σ_p ladder (per-packet white common-phase noise) | 0.03 / 0.17 / 0.5 rad | 0.025–0.247 bracket the ESP-PPB implied decomposition (CSV rows 30–31); 0.5 = pessimistic stock |
| Raw differential CFO Δf | 48.7 kHz (20 ppm at 2.437 GHz) | note 12 |
| Reply-instant jitter σ_τ ladder | 0.3 / 1.6 / 16 µs | 0.3 ≈ WCNC 2023 RPi reply std (< 1 µs); 1.6 ≈ pessimistic software turnaround; 16 = worst case |
| σ_Δmeas (knowledge of the reply interval from RX timestamps) | 30 ns | **modelling assumption, unverified on S3** — flagged in §12 |
| Reference gaps T | 0.1–100 ms | ladder |
| Cadences (Kalman) | 0.2–100 ms | ladder |
| Stitched bandwidth (cascade) | 83 MHz (13 × HT20 stitching); per-channel 20 MHz | brief §Finding 4 |
| SNR (cascade CRBs) | 20 dB | round-1 datum |
| Airtime frame model | 40 µs preamble + payload at 6.5 Mb/s (MCS0) or 65 Mb/s (MCS7-SGI), 127 B, + 16 µs turnaround; cycle = (N+3) frames (ping + N replies + 2 bracket references) | script §6 |
| Monte-Carlo | seed 20261016 (`numpy.default_rng`), 192 phase walks per point, dt = 250 µs, 6 s durations | script |
| Target coherence time | λ/2v = 62 ms @ 1 m/s; 123 ms @ 0.5; 615 ms @ 0.1 | λ/2v |

The β↔Hz mapping: for a Lorentzian oscillator pair, Var[Δθ(T)] = 4π·β_rel·T.
This is the *Wiener* (random-walk) model; note h uses the *linear-drift* model
σ_φ = 2π·σ_ν·T. The difference matters for the kill values (§8) and is flagged
wherever the two notes are compared.

---

## 3. Findings

### 3.1 ESTABLISHED (the mathematics these simulations evaluate; MC- and Wolfram-checked)

- **Bracketing beats holdover 4× in variance.** With references at ±T/2 and the
  sensing frame at centre, σ_bracket = √(π·β·T + σ_p²/2) against one-sided
  holdover σ_hold = √(4π·β·T + σ_p²). Eight Monte-Carlo checks agree with the
  closed forms to ≤ 2 % (CSV rows 2–9; worst: bracket β=4 Hz, T=20 ms,
  0.514 vs 0.516 rad).
- **The jitter kill is exact arithmetic:** σ_φ = 2π·Δf·σ_τ, so at 48.7 kHz the
  0.5 rad gate ⇔ σ_τ ≤ 1.63 µs per exchange (single exchange); M = 10 averaged
  exchanges relax this to 5.2 µs. This reproduces note h's kill value
  independently.
- **The unfenced kill, Wiener form:** at 20 ms and negligible σ_p the gate ⇔
  β_rel ≤ 0.995 Hz ≈ 1 Hz. Note h's linear-model equivalent, σ_ν ≤ 3.98 Hz, is
  the same statement in the other noise model (both reproduce in one line;
  verified in this session).
- **Coherence budget:** the gate ⇔ **B = σ_ν·T_obs ≤ 0.0796 cycles** (h's
  ledger row, reproduced here). CSV rows 161–166 tabulate B for
  Δν_res ∈ {1, 10, 100 Hz} × T_obs ∈ {20 ms, 1 s}: a 10 Hz residual over 1 s is
  B = 10 cycles — hopeless unfenced; 1 Hz over 20 ms is B = 0.02 — comfortable.
- **Within a coherence window, packet/face phase noise averages as √N**
  (MC check at M = 16, T_c = 1 ms: 0.262 vs 0.263 rad, CSV row 86). Averaging
  beyond the coherence window buys nothing — the walk term does not average.

### 3.2 SIMULATED (this note's numbers; estimator behaviour, not device)

**(a) The gate at the 20 ms cadence** (`23-t1-b-fig1-gate.png`; CSV rows 22–29;
σ_p = 0.17 rad, the ESP-PPB-typical per-packet noise):

| design point @ 20 ms cadence | β = 0.05 Hz | β = 0.5 Hz | β = 4 Hz | β = 40 Hz |
| --- | --- | --- | --- | --- |
| holdover (one-sided ref) | 0.204 rad (2.45×) | 0.393 rad (1.27×) | **1.017 rad FAIL** | **3.175 rad FAIL** |
| bracket, ±1 ms (2 ms span) | 0.122 (4.1×) | 0.133 (3.8×) | 0.199 (2.5×) | **0.516 (0.97×, marginal fail)** |
| bracket, ±0.5 ms (1 ms span) | 0.121 (4.1×) | 0.127 (3.9×) | 0.164 (3.0×) | 0.374 (1.34×) |
| bracket ±0.5 ms + F = 20 faces | 0.030 (16×) | 0.048 (10×) | 0.115 (4.3×) | 0.356 (1.40×) |

Margin = 0.5/σ_φ. **Answer to acceptance criterion 2: the gate holds at 20 ms
with 1 ms brackets — ≥ 3× margin for any β ≤ 4 Hz, and even MegaMIMO-class
β = 40 Hz passes at 1.34–1.40×.** Without bracketing (holdover) the gate fails
for β ≥ 4 Hz — i.e. a stock free-running S3 pair at the round-1 "10 Hz
residual" reading fails holdover by 2–6×. Bracketing is not an optimisation,
it is the gate.

**How late can the reference be?** (CSV rows 10–21, σ_p = 0.17.) Max
reference-to-sensing gap for 0.5 rad: β = 0.05 Hz: 352 ms holdover / 1500 ms
bracket; β = 0.5: 35 / 150 ms; β = 4: **4.4 / 18.7 ms**; β = 40: **0.44 /
1.9 ms**. So at free-running-optimistic β = 4 Hz a ±1 ms bracket sits 9× inside
the fence; at β = 40 Hz the bracket span itself must shrink below ~2 ms.

With σ_p = 0.5 rad per packet the holdover curve *starts* on the gate (no
crossing; CSV rows 12/15/18/21 record `inf`) — per-packet noise alone then
leaves zero margin, and only the bracket's σ_p²/2 term saves it (bracket ±0.5
ms at β = 4 Hz: 0.371 rad, 1.35×). **σ_p = 0.5 rad is itself a killing value
for holdover designs.**

**(b) Kalman tracker** (`23-t1-b-fig2-kalman.png`; CSV rows 32–70). A scalar
Kalman filter on [phase, freq] with process noise from the β ladder; the
steady-state covariance is the discrete-Riccati solution and matches
Monte-Carlo (innovation 0.389 vs 0.392 rad; 20 ms holdover 1.061 vs 1.044 rad,
rows 68–69).

- At β ≤ 0.5 Hz the tracker at **20 ms cadence holds the 20 ms prediction under
  the gate with no extra frames**: 0.425–0.435 rad (1.15–1.18× margin). A
  disciplined XO needs no bracket airtime.
- At β = 4 Hz the tracker cannot beat the walk: innovation 0.392 rad at 2 ms
  cadence (passes) but the 20 ms-ahead prediction saturates at ≈ 1.04 rad
  (fails). Prediction horizon, not innovation, is what fails.
- The tracker's steady-state residual frequency error σ_f = 0.66–0.82 Hz
  (β = 0.05), 1.14–1.39 Hz (β = 0.5), 1.91–2.05 Hz (β = 4). At β = 4 Hz and
  2 ms cadence, σ_f = 1.91 Hz — a factor 2 inside note h's unfenced linear kill
  of 3.98 Hz, which is *why* the innovation passes while the 20 ms prediction
  fails: 2π·1.91·0.02 = 0.24 rad of linear drift is fine, the remaining walk
  variance is not.

**(c) Two-way TX-jitter budget** (`23-t1-b-fig3-twoway.png`; CSV rows 71–85;
Δ = reply-to-reference interval, M = exchanges averaged, F = 20 faces,
σ_Δmeas = 30 ns contributing 0.009 rad — negligible):

| σ_τ (reply-instant jitter) | M = 1 | M = 10 |
| --- | --- | --- |
| 0.3 µs (RPi-class) | 0.216 rad | 0.069 rad |
| 1.6 µs | 0.527 rad (0.95×, marginal fail) | 0.167 rad (3.0×) |
| 16 µs | 4.90 rad (fail) | 1.549 rad (fail) |

**Killing values: σ_τ > 1.63 µs single-exchange; > 5.2 µs with M = 10** (at
48.7 kHz raw CFO). The β-dependence is mild (0.493 rad at β = 0.05 → 0.527 at
β = 4 → 0.771 at β = 40, rows 80–83); the interval dependence is not (0.527 at
Δ = 0.7 ms → 1.143 rad at Δ = 20 ms, rows 77–79): two-way products want the
reply bracketed tightly against its reference.

**(d) Averaging law** (`23-t1-b-fig4-averaging.png`; CSV rows 86–94). √N holds
within the coherence window. Best (M, cadence) splits at 1 ms cadence: with
F = 20 faces, M = 1 is optimal for β ≥ 0.5 Hz (σ = 0.038 rad — the faces do
the averaging); with F = 1, M = 32 at β = 0.05 Hz gives 0.051 rad and M = 4 at
β = 4 Hz gives 0.161 rad. Deeper packet averaging only pays when faces are
few or the LO is very quiet.

**(e) Ambiguity cascade** (`23-t1-b-fig5-cascade.png`; CSV rows 95–131;
§5 below). Headline: **the cascade breaks on stitched-delay bias, not on
σ_φ.**

**(f) Airtime** (CSV rows 132–151; model in §2). Frame: 270 µs (MCS0) / 63 µs
(MCS7-SGI) for 127 B HT20. Cycle = (N+3) frames + 16 µs turnarounds. Airtime
at 20 Hz cadence: N=2: 2.9 % / 0.8 %; N=8: 6.3 % / 1.7 %; N=20: 13.2 % / 3.6 %
(MCS0 / MCS7). At 50 Hz: N=20 MCS0 reaches **32.9 %** — the practical ceiling.
Below 20 Hz, airtime is never binding for N ≤ 20.

**(g) RIPS-style beacon corroboration** (missing direction f; CSV rows
152–160). Beat-phase σ at ρ = 0 dB, T_w = 1 ms: 0.063 rad (→ 1.24 mm
equivalent); 10 dB: 0.020 rad. Cross-node time alignment needed for 0.1 rad
beat phase at 48.7 kHz: **0.33 µs** — the RIPS route has its own, tighter,
alignment kill, consistent with note h's Part E conclusion that the
beat route is LO-immune but alignment-limited.

### 3.3 HYPOTHESIS

- Stock S3 software reply turnaround can meet 1.63 µs: the WCNC 2023 Raspberry
  Pi CM4 datum (< 1 µs reply std) suggests yes, but ESP-IDF's Wi-Fi task
  scheduling on S3 is unproven at this bound. **Single most valuable
  measurement left.**
- The ESP-PPB implied decomposition transfers from C3 to S3 (CSV rows 30–31:
  "<1° best" ⇒ β = 0.10 Hz, σ_p = 0.025 rad; "<10° typical" ⇒ β = 9.70 Hz,
  σ_p = 0.247 rad). Even the *typical* corner passes the gate with ±0.5 ms
  brackets: √(π·9.7·0.001 + 0.247²/2) = 0.247 rad, 2.0× margin.
- Kalman at 2 ms cadence ≈ bracketing in practice (innovation 0.392 vs bracket
  0.199 rad at β = 4 Hz — bracket still 2× better; the tracker additionally
  costs no airtime).
- Face averaging stays √N across faces of a dual-pol array (polarisation and
  mutual coupling may break it — T3's question, not simulated here).

### 3.4 SPECULATION

- Combining f's broadcast-only reference supply (no two-way term, no jitter
  exposure) with F = 20 face averaging lands σ_φ ≈ 0.03–0.06 rad at 20 ms —
  at which carrier-phase integers resolve almost surely (cascade rows: ≥ 99 %
  at σ_φ ≤ 0.3 rad given bias ≤ 1 cm) and the gate stops being the binding
  constraint. The binding constraint would move to scene stability and the
  stitched-delay bias.
- If calibration holds the stitched bias ≤ 1 cm, house-scale cm-class 3D
  follows from this budget alone (RMSE|ok 0.1–0.8 cm in the cascade MC).

---

## 4. Shared ledger (acceptance criterion 5)

| Quantity | Value | Provenance |
| --- | --- | --- |
| σ_φ @ 20 ms, bracket ±0.5 ms, β = 4 Hz, σ_p = 0.17 | **0.164 rad (3.0× margin)**; 0.115 with F = 20 faces (4.3×) | SIMULATION (closed form + MC ≤ 2 %) |
| σ_φ @ 20 ms, holdover, β = 4 Hz | 1.017 rad — **FAIL** (0.49×) | closed form |
| σ_φ @ 20 ms, bracket ±0.5 ms, β = 40 Hz (worst ladder) | 0.374 rad (1.34×); 0.356 with F = 20 | SIMULATION |
| σ_φ @ 20 ms, ESP-PPB "typical" implied (β = 9.7 Hz, σ_p = 0.247), bracket ±0.5 ms | 0.247 rad (2.0×) | derived from CSV rows 30–31 |
| σ_φ @ 20 ms, Kalman, β = 0.5 Hz, 20 ms cadence | 0.425 rad (1.18×), no extra frames | SIMULATION (DARE + MC) |
| Coherence budget B | gate ⇔ **B ≤ 0.0796 cycles** (σ_ν·T_obs); Wiener equivalent β_rel ≤ ≈ 1 Hz @ 20 ms | closed form |
| Killing values | reply jitter **> 1.63 µs** single-exchange / **> 5.2 µs** M = 10 at 20 ppm; unfenced **β_rel ≥ 1 Hz** (Wiener) ⇔ **σ_ν ≥ 3.98 Hz** (linear, h's value); **σ_p = 0.5 rad** per packet (holdover: zero margin); stitched-delay **bias ≥ 3 cm** (cascade: ok 99.3 % → 68.8 %); airtime **> 33 %** only at N = 20, MCS0, 50 Hz | SIMULATION / arithmetic |
| Not killing (never binding, in-model) | airtime ≤ 13.2 % for N ≤ 20 at 20 Hz; the √N averaging law; two-way β-sensitivity (0.49–0.77 rad across the whole β ladder at σ_τ = 1.6 µs); σ_Δmeas = 30 ns (0.009 rad) | SIMULATION |
| Ambiguity cascade | FTM ~m-class → stitched c/2B = 1.81 m unambiguous @ 83 MHz, delay CRLB 2.6 mm @ 20 dB (HT20 alone: 3.9 cm) → carrier λ/2 = 6.15 cm; carrier-phase range noise 2.9 mm at σ_φ = 0.3 rad | SIMULATION (CRB + MC) |
| Airtime (bracket superframe, 127 B) | 20 Hz: 2.9 % (N=2) / 6.3 % (N=8) / 13.2 % (N=20) at MCS0; ÷3.6 at MCS7 | arithmetic |
| Target coherence time | 62 ms @ 1 m/s; 123 ms @ 0.5; 615 ms @ 0.1 | λ/2v |

---

## 5. The ambiguity cascade and the end-to-end chain

The chain simulated: FTM positional fix (RMSE 2.13 m) → stitched-delay ranging
(83 MHz, CRLB 2.6 mm; unambiguous c/2B = 1.81 m) → carrier-phase integers
(λ/2 = 6.15 cm). Monte-Carlo over bias ∈ {5 mm…30 cm} × σ_φ ∈ {0.05…1.0 rad}
(CSV rows 95–131):

- **σ_φ is nearly irrelevant to integer resolution** in 0.05–1.0 rad: at
  stitched bias ≤ 1 cm, full-chain integer success stays 93.9–99.7 % across
  the whole σ_φ ladder; conditional RMSE|ok degrades only 0.1 → 1.6 cm.
- **Bias is the killer:** at bias 3 cm, full-chain success collapses to
  8–10 % (per-link 68 %); at 5 cm, ~1 %. The rung diagnosis (rows 130–131):
  per-link carrier-resolution success 99.3 % at bias 1 cm → 68.8 % at 3 cm.
- Per-link carrier-phase range noise at σ_φ = 0.3 rad is 2.9 mm — two orders
  below the 49 cm stitched RMSE that unresolved links fall back to.

**Conclusion:** the coherence gate (σ_φ ≤ 0.5 rad) is *necessary but far from
sufficient*; the end-to-end chain is gated by the stitched-delay bias, i.e. by
calibration and multipath, not by oscillator phase. This sharpens Finding 4:
the one-number programme needs *two* numbers — σ_φ and the stitched bias.

---

## 6. ESP32-S3 feasibility

- **Feasible path on stock S3 (free-running XOs, 20 ppm spread):** bracketed
  superframe — reference frames ±0.5 ms around each sensing frame, F = 20 face
  averaging, two-way replies only if σ_τ ≤ 1.63 µs can be shown. Predicted
  σ_φ = 0.115–0.164 rad (3.0–4.3× margin) at 20 ms cadence for β ≤ 4 Hz.
  Airtime 6.3 % (N = 8) to 13.2 % (N = 20) at MCS0 — fits.
- **The one unproven register-level requirement:** software reply-instant
  jitter σ_τ ≤ 1.63 µs (single exchange) or ≤ 5.2 µs (M = 10). S3 has no TX
  hardware timestamp (`ESP-IDF S3 Wi-Fi API`: TSF and RX timestamps only), so
  the two-way product must be *bracketed* by RX-timestamped references; the
  30 ns σ_Δmeas assumption must be verified against S3 RX-timestamp
  granularity.
- **Disciplined route (ESP-PPB-class, unrefereed, C3-only today):** β ≤ 0.5 Hz
  lets a Kalman tracker at 20 ms cadence hold the gate with *no bracket
  airtime* (0.425 rad, 1.18×); with brackets the margin exceeds 10×. If the
  claimed discipline ports to S3, the gate closes with room to spare.
- **Worst-case corner still passes:** MegaMIMO-class β = 40 Hz with 1 ms
  brackets and F = 20 faces gives 0.356 rad (1.40×) — the gate does not fail
  anywhere in the simulated parameter box *provided bracketing exists*.

## 7. TX needed?

- **Yes, from at least one node:** the reference supply (bracket frames or the
  broadcast fix) is transmitted. A hub-broadcast reference (f's design D2)
  suffices — sensing nodes can then be receive-only, which also removes the
  reply-jitter kill entirely (no two-way term, predicted σ_φ = 0.03–0.06 rad
  with face averaging).
- **Yes, for two-way ranging:** every ranging reply is a transmission, and the
  1.63 µs kill applies to each. Monostatic/receive-only sensing against a
  broadcasting hub avoids it but gives one-sided geometry only.
- Airtime verdict: transmissions for N ≤ 20 nodes at 20 Hz cost ≤ 13.2 %
  (MCS0) — TX is not an airtime problem at home scale; it is a *jitter*
  problem.

## 8. Cross-reconciliation with the f and h sibling notes

| quantity | **b (this note)** | **f** (`23-coherence-gate-f.md`) | **h** (`23-coherence-gate-h.md`) | verdict |
| --- | --- | --- | --- | --- |
| σ_φ @ 20 ms, best passing case | 0.115–0.164 rad, bracket ±0.5 ms, 3.0–4.3× | 0.056 (D2 broadcast, 8.9×) / 0.105 (D1 two-way corrected, 4.7×) | 0.385 rad (0.343 track ⊕ 0.175 datum), 200 Hz fence, 1.30× | **All three PASS the gate.** The 0.056 → 0.385 spread is entirely the reference-supply assumption (broadcast + interpolation vs 200 Hz fence vs ±0.5 ms brackets), not the arithmetic |
| Killing reply jitter | **1.63 µs** (M=1) / 5.2 µs (M=10) at 48.7 kHz | gates replies at 5 µs (~90 % kept) | **1.63 µs at 20 ppm** | identical; b and h derive it independently from the same 2π·Δf·σ_τ arithmetic — a genuine cross-check |
| Unfenced kill | β_rel ≥ ≈ 1 Hz @ 20 ms (Wiener) | residual CFO 100 Hz still passes *if bracketed* (D6: 0.105 rad) | **σ_ν ≥ 3.98 Hz** (linear) | same conclusion, different noise model: h's linear drift spends all variance at DC; b's Wiener walk spreads it. In variance terms at 20 ms: (3.98 Hz)² linear ≈ β ≈ 4 Hz Wiener-scale drift — the models bracket the truth; both say *unfenced stock fails, bracketed passes* |
| Worst simulated case | β = 40 Hz bracketed: 0.374 rad (1.34×) | D8 hazard (5 ms brackets, walker on reference path): 0.257 rad (1.9×) | 50 Hz fence: 42.19 rad — killed | h's fence at 50 Hz is below b's β = 40 Hz bracket max-gap (1.87 ms span) — consistent: a 5 ms-spaced fence cannot track a 40 Hz walk |
| Airtime | (N+3)·286 µs cycle: 13.2 % N=20 @ 20 Hz | broadcast line 9.8 ms | fence 250 µs frames: 5 % @ 200 Hz | compatible; b's cycle model is the most conservative |
| Cascade | breaks on bias ≥ 3 cm, not σ_φ | integers P ≥ 97 % @ σ_φ = 0.02, 83 % @ 0.05 (no-bias MC) | FTM 3 m → 5.2 cm → 6.1 cm rungs | consistent; b adds the bias-dominance result the other two did not isolate |

**Why the numbers differ (expected):** f optimises the reference *supply*
(broadcast + two-sided interpolation + averaging), h optimises the reference
*tracker* (fence + PLL/particles) and stresses its failure modes, b optimises
the *bracket geometry* and cross-checks with a Kalman alternative. Inside the
bracket/fence interval the three estimators agree; outside it they diverge by
construction. The shared, unambiguous conclusions: bracketing/fencing is
mandatory on stock hardware; the reply-jitter kill is 1.63 µs; airtime is
never binding below 20 Hz; the gate passes everywhere in the simulated box
with references ≤ 1 ms away.

## 9. Evidence strength: **6 / 10**

- **For:** every closed form is reproduced by Monte-Carlo to ≤ 2 % and the
  Kalman filter against its DARE (independent numerical routes to the same
  numbers); the jitter kill is derived two ways (b, h) and matches f's gating
  practice; the parameter box is anchored to named literature values (§2).
- **Against:** zero S3 hardware data — σ_p, σ_τ and β on S3 are *assumed*
  ladders, not measurements; ESP-PPB is unrefereed and C3-only; the 30 ns
  reply-interval knowledge is a modelling assumption; the Wiener-vs-linear
  model choice moves the unfenced kill by ~4× in Hz-equivalent terms. The
  mathematics is strong; the device mapping is the weak link.

## 10. The falsifier

**Claim:** *on stock ESP32-S3, σ_φ ≤ 0.5 rad at 20 ms cadence is achievable
with ±0.5 ms brackets and F = 20 face averaging, margin ≥ 3× for β ≤ 4 Hz.*
**Falsified by any one of:** (i) a conducted two-board measurement of
bracketed per-packet residual > 0.5 rad; (ii) S3 reply-instant jitter
> 1.63 µs (single-exchange two-way) or > 5.2 µs (M = 10) — measurable with the
E1/M2′ conducted-splitter rig; (iii) S3 per-packet CSI phase noise > 0.5 rad;
(iv) effective β_rel > 1 Hz with no bracket clock available (holdover-only
operation at 20 ms). The minimal reference experiment that crosses the gate
remains the round-1 E1/M2′ family: two boards, conducted splitter, bracketed
superframe, σ_φ versus gap. The cascade claim is falsified in the same rig by
a stitched-delay bias > 3 cm with integer resolution success collapsing below
70 %.

## 11. Primary sources

- ESP-PPB — FTM-disciplined VCTCXO reference frames, < 1°/< 10° single-frame
  phase on ESP32-C3 (unrefereed; source of the β = 0.05–0.5 Hz and
  σ_p = 0.025–0.247 rad ladder anchors): https://github.com/jonathanmuller/esp-ppb
- ESPARGOS — conducted phase-coherent CSI reference pattern: https://espargos.net
- ESP-SDR — raw I/Q on ESP32, S3 continuous decimated mode (250 kS/s used in
  the RIPS rows): https://espargos.net/espsdr
- RTCSI — round-trip CSI cancels asynchrony (0.96 → 0.04 rad), reply-interval
  std < 1 µs on Raspberry Pi CM4, Shi et al., IEEE WCNC 2023; UCL Discovery
  eprint 10161602: https://discovery.ucl.ac.uk/id/eprint/10161602
- ML-Track — multi-link RTCSI tracking, IEEE TMC 2025,
  DOI: 10.1109/TMC.2025.3529897
- WiDRa — sum-carrier-phase differential ranging, JSAC 2024,
  arXiv:2405.12168: https://arxiv.org/abs/2405.12168
- LoSRef — LoS path as OTA delay+phase reference, arXiv:2602.05344:
  https://arxiv.org/abs/2602.05344
- ESP-IDF, *Wi-Fi (esp_wifi) API Reference — ESP32-S3* (TSF/RX timestamps; no
  TX timestamp outside FTM): https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/api-reference/network/esp_wifi.html
- IEEE 802.11bf-2025 (procedure, not cross-node coherence).
- Round-1 parameters: CFO and 2.437 GHz channel (`12-*`); FTM/timestamp
  registers (`17-time-frequency-precision.md`); two-way product physics
  (`22-invention-g.md`, `22-invention-s.md`).

## 12. Residual unknowns

1. **S3 reply-instant jitter σ_τ** — the single decisive unmeasured number
   (kill at 1.63 µs). Needs hardware.
2. **S3 per-packet CSI phase noise σ_p** — the ladder 0.03/0.17/0.5 rad spans
   a 17× variance range; the gate outcome is insensitive to it *given
   brackets*, but the holdover verdict is not.
3. **S3 RX-timestamp granularity** vs the assumed 30 ns reply-interval
   knowledge.
4. Whether ESP-PPB-class VCTCXO discipline ports to S3 (unrefereed, C3-only).
5. Face-averaging √N under polarisation/mutual coupling (T3).
6. Real-scene static-clutter subtraction *before* the gate applies — this
   note's gate is hardware-only; the scene adds its own reference-decorrelation
   term (f's D8 hazard and LoSRef's 20 dB dynamic-range datum both point
   here).

## 13. Chaining / existing models and data / own data

### (A) Chaining

Consumes ROUND2-BRIEF Finding 4 items 1 (σ_φ), 2 (TX jitter), 3 (averaging
law), 4 (holdover budget B) and 5 (cascade sharpness), and closes criterion 2's
margin question for the bracketed design. Feeds forward: the σ_φ = 0.115–0.164
rad bracketed design point is the input any localisation note should assume;
the bias-dominance result (§5) tells the calibration notes what actually gates
the cascade; ledger rows are aligned with h's §4 table for like-with-like
synthesis. Complements f (which designs the reference supply this note
consumes) and h (which stress-tests the tracker this note reduces to at
20 ms cadence).

### (B) Existing models and data

The repo's deterministic CSI simulator (`cargo run -p aquila-capture -- synth`)
was **not** used: the gate is oscillator/protocol-level, below the channel
model's granularity; the σ_φ it would produce given per-packet channel realis
isations is a natural follow-up (chain this script's β/σ_p ladders into the
synth harness). Literature-anchored parameters: WCNC 2023 reply jitter, ESP-PPB
implied decomposition, MegaMIMO-class residual, ESPARGOS conducted pattern.

### (C) Own data / new technology

All artefacts from this note (SIMULATIONS, seed 20261016):

- `docs/research/v2/code/t1_gate_b.py` — the full suite (gate curves + MC,
  Kalman/DARE tracker, two-way jitter, averaging law, cascade MC, airtime,
  RIPS rows).
- `docs/research/v2/code/t1_gate_b_results.csv` — 177 scalar results, every
  number in this note traceable to a row.
- Figures: `docs/research/v2/figures/23-t1-b-fig1-gate.png` (bracket vs
  holdover gate curves), `23-t1-b-fig2-kalman.png` (innovation/holdover vs
  cadence), `23-t1-b-fig3-twoway.png` (jitter budget), `23-t1-b-fig4-averaging.png`
  (√N law and best-M trade), `23-t1-b-fig5-cascade.png` (integer-resolution
  success vs bias and σ_φ).

Reproduce: `/home/matt/projects/aquila-v2-spec/sim-python
docs/research/v2/code/t1_gate_b.py` (writes only the figures and CSV above,
≈ 300 KB total — within the disk discipline).
