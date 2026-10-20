# 23 — The coherence gate: fence tracking, the 0.5 rad budget and the kill values (track T1, direction h)

Status: round-2 research note (out-of-band; never defines v1 scope). Australian
English. Character `h`: the Closure Weave track (after `22-invention-h.md`).
The simulations were completed by an interrupted predecessor session; this note
was finished by a follow-up agent from the on-disk artefacts (CSV re-derived
bit-identically by re-running the fixed-seed script).

**Everything numerical below is a SIMULATION or an analytic derivation — no
device was measured** (per `EXPERIMENT-ENV.md` honesty rules). Code:
`docs/research/v2/code/t1-coherence-gate-h.py` (rerun with
`/home/matt/projects/aquila-v2-spec/sim-python docs/research/v2/code/t1-coherence-gate-h.py`;
seed 20261016, deterministic). Numbers: `t1-coherence-gate-h-results.csv`
(Part B) plus the script's stdout tables (Parts A, C, D, E). Figures:
`docs/research/v2/figures/t1-h-sigma-vs-gap.png` (residual vs gap, all key
runs, 0.5/0.1 rad gate lines) and `docs/research/v2/figures/t1-h-chain.png`
(coherence → localisation curve + averaging law).

---

## 1. The question

Finding 4 of `ROUND2-BRIEF.md` gates the programme on one number: **σ_φ, the
per-frame cross-node carrier-phase residual after CFO removal, at the 20 ms
working cadence, against the ≤ 0.5 rad gate.** Round 1 never measured it; this
note *simulates it to the register level* and maps the kill region: which
estimator discipline (reference fence, one-sided vs bracketed), which fence
rate, which residual-oscillator dynamics and which datum noise pass, fail, or
diverge. This is the M2 / "fabric residual" number that `22-invention-h.md`'s
Tier B (beacon-held phase) lives or dies on, and the coherence input to the
end-to-end chain (acceptance criterion 3).

**Answer in one sentence (simulation verdict).** At the 20 ms cadence the gate
is passed — σ_φ = **0.385 rad (1.30× margin)** — only in the slow-drift corner:
a **200 Hz reference fence** (5 % airtime) plus residual-CFO dynamics with
correlation time **τ_ν ≈ 100 s**; every faster-drift corner tested fails
(1.7–6 rad), a 50 Hz fence aliases catastrophically (**≈ 42 rad**), an
unmodelled 8 Hz/s thermal ramp doubles the residual, and 20 % outlier fence
frames handled by 3σ gating **without a working re-lock path diverge to 44 rad**.
The gate is decided by the *residual-drift spectrum*, not by thermal noise,
timestamp quantisation or CFO-fit precision — none of which is ever binding.

---

## 2. Model and parameters (the honesty block)

All parts share: channel-6 centre f_c = 2.437 GHz (λ = 12.29 cm). Parameter
provenance: raw CFO classes 2/10/20/40 ppm → 4.9/24.4/48.7/97.5 kHz (round-1
notes `17`, `12`); residual CFO after per-frame removal modelled as an
Ornstein–Uhlenbeck (OU) process, σ_ν = 100 Hz default (MegaMIMO-class datum via
`00-synthesis.md` §2) with correlation time τ_ν ∈ {0.1, 1, 10, 100} s —
**unmeasured on S3, swept**; per-frame datum noise σ_m ∈ {0.04, 0.175, 0.6} rad
= ML-Track/RPi 0.04 (WCNC 2023), ESP-PPB "typical" 0.175 (C3, **unrefereed**),
worst case 0.6; fence rates R ∈ {50, 200, 1000} Hz, 250 µs per fence frame;
data-to-reference gaps T_gap ∈ {0.25 … 100} ms. Tracker: 2-state Kalman
[φ, ν] (3-state adds ramp acceleration), OU-exact F and Q, 200 parallel
particles, seed frame then steady state; the reported σ_φ is the RMS
**one-sided prediction error** of the post-update state to t_k + T_gap —
i.e. the residual of a sensing frame against a reference fence, not a bracket.
Truth phase integrated trapezoidally (integration error O(T/τ_ν), ≤ 2 % for
τ_ν ≥ 1 s). Scene dynamics would enter as deterministic phase rate; thermal
ramp run at 8 Hz/s from 30 % of the run; outlier run: p = 0.20 of fence frames
perturbed by 0.5·N(0,1) rad, with and without a 3σ innovation gate after 300
acquisition frames. Part D scene: 6 nodes on a 5 × 4 m rectangle at 2.4–2.6 m
height, target at (2.0, 1.5, 1.0) m, 61³ grid over ±30 cm, one wall bounce per
link at amplitude 0.45 (−6.9 dB); effective lateral aperture D_eff = 4.96 m,
mean range r̄ = 3.04 m. GNU Radio/openems/FDTD were not provisioned; nothing
below pretends otherwise. A NumPy model is ESTABLISHED only for the mathematics
it evaluates, not for the device.

---

## 3. Findings

### 3.1 ESTABLISHED (closed-form arithmetic; literature-anchored parameters)

- **Holdover law:** with no fence, σ_φ = 2π·σ_ν·T_gap. The 0.5 rad gate at
  20 ms requires **σ_ν ≤ 3.98 Hz** (0.1 rad: 0.80 Hz). A 1-ppb-disciplined XO
  (2.44 Hz at 2.4 GHz) passes unfenced: 0.307 rad at 20 ms. A 100 Hz residual
  gives 12.57 rad at 20 ms — the raw gap between "coherent" and "not".
- **TX-instant jitter × raw CFO:** σ_φ = 2π·Δf_raw·σ_τ. Killing σ_τ for the
  0.5 rad gate: **1.63 µs at 48.7 kHz (20 ppm)**, 3.32 µs at 24.4 kHz,
  16.6 µs at 4.9 kHz; the WCNC 2023 RPi datum (< 1 µs) passes at ≤ 20 ppm but
  fails the 0.1 rad tier (0.33 µs at 20 ppm). **Unmeasured on S3.**
- **Timestamp quantisation:** 12.5 ns RX grid × raw CFO → ≤ 0.0038 rad.
  Never binding.
- **Thermal CRB:** per-frame common phase from 64 subcarriers =
  1/√(2·64·SNR) → **0.0088 rad at 20 dB** (0.0497 at 5 dB). Thermal noise is
  never the limit; the assumed σ_m = 0.175 rad is impairment-dominated
  (20× CRB), consistent with its measurement provenance.
- **CFO-fit CRB:** √(12·σ_m²/((2π)²·R·T_w·T_w²)) → **0.019 Hz** at
  R = 200 Hz, T_w = 0.5 s, σ_m = 0.175 rad. Fit noise never binds; the *drift*
  (process noise) does.
- **Coherence gain and budget:** E[cos φ] = e^(−σ²/2): 0.985 at 0.175 rad,
  0.882 at 0.5 rad, 0.607 at 1 rad. Budget B = σ_ν·T_obs (cycles): the gate is
  **B ≤ 0.0796 cycles**; at 20 ms this is the σ_ν ≤ 3.98 Hz line above.
- **Target coherence time** λ/2v: **61.5 ms at 1 m/s**, 12.3 ms at 5 m/s —
  the 20 ms cadence oversamples walkers 3×, runners marginally.
- **Airtime:** one 250 µs fence frame → duty **1.25 % (50 Hz) / 5 % (200 Hz) /
  25 % (1 kHz)**. The feasible operating point is 200 Hz.

### 3.2 SIMULATED (this note's numbers; estimator behaviour, not device)

σ_φ (rad) versus data-to-reference gap, selected runs (full set in the CSV and
`t1-h-sigma-vs-gap.png`):

| run (R, σ_ν, τ_ν, σ_m) | 1 ms | 5 ms | 10 ms | 20 ms | 50 ms | 100 ms |
| --- | --- | --- | --- | --- | --- | --- |
| 1 kHz fence, τ_ν = 10 s, σ_m 0.175 | 0.364 | 0.441 | 0.545 | **0.759** | 1.410 | 2.503 |
| 200 Hz fence, τ_ν = 10 s, σ_m 0.175 | 1.259 | 1.341 | 1.455 | **1.697** | 2.450 | 3.728 |
| 50 Hz fence, τ_ν = 10 s, σ_m 0.175 | 41.51 | 41.64 | 41.82 | **42.19** | 43.33 | 45.29 |
| 200 Hz fence, **τ_ν = 100 s**, σ_m 0.175 | 0.164 | 0.197 | 0.244 | **0.343** | 0.657 | 1.191 |
| 200 Hz fence, τ_ν = 1 s, σ_m 0.175 | 2.328 | 2.520 | 2.785 | 3.341 | 5.075 | 8.029 |
| 200 Hz fence, τ_ν = 0.1 s, σ_m 0.175 | 4.011 | 4.404 | 4.943 | 6.069 | 9.587 | 15.61 |
| 200 Hz, τ_ν = 10 s, σ_m 0.04 | 2.197 | 2.264 | 2.354 | 2.542 | 3.122 | 4.108 |
| 200 Hz, τ_ν = 10 s, σ_m 0.6 | 0.552 | 0.659 | 0.808 | 1.129 | 2.143 | 3.868 |
| 200 Hz, τ_ν = 10 s, ramp 8 Hz/s, 2-state | 2.686 | 2.771 | 2.890 | 3.140 | 3.916 | 5.232 |
| 200 Hz, τ_ν = 10 s, ramp 8 Hz/s, 3-state | 1.234 | 1.315 | 1.430 | 1.671 | 2.423 | 3.697 |
| 200 Hz, τ_ν = 10 s, 20 % outliers, ungated | 0.238 | 0.332 | 0.464 | 0.744 | 1.610 | 3.067 |
| 200 Hz, τ_ν = 10 s, 20 % outliers, gated, **no re-lock** | 43.09 | 43.29 | 43.54 | **44.04** | 45.61 | 48.35 |

- **The gate verdict at the 20 ms cadence.** Adding the irreducible per-frame
  datum noise in quadrature (σ_total = √(σ_track² + σ_m²)):
  - **Best passing point: 200 Hz fence, τ_ν = 100 s → 0.385 rad at 20 ms
    (margin 1.30×; 0.300 rad at 10 ms, 1.66×; fails by 50 ms, 0.680 rad).**
    P(prediction error > 0.5 rad) at 5 ms = 0.003.
  - Marginal alternative: 1 kHz fence, τ_ν = 10 s at a **5 ms cadence** →
    √(0.441² + 0.175²) = **0.474 rad (1.05× margin)** — but at 25 % airtime.
  - Everything else at 20 ms fails: 1.71–1.71+ rad (τ_ν = 10 s at 200 Hz),
    0.776 rad (1 kHz), 3.16 rad (ramp, 2-state), 1.68 rad (ramp, 3-state),
    0.752 rad (outliers ungated; passes ≤ 5 ms only), 42–48 rad (50 Hz fence;
    outliers gated without re-lock).
- **Regime structure.** (i) *Fence-rate floor:* a 50 Hz fence (20 ms spacing)
  aliases the 100 Hz OU completely — 42 rad regardless of gap; the fence must
  sample well inside the residual's coherence interval. (ii) *Process-noise
  regime:* at 200 Hz with τ_ν ≤ 10 s the residual is estimator-limited, not
  datum-limited — improving σ_m from 0.175 to 0.04 rad does **not** fix the
  gate (2.54 rad at 20 ms, worse than 0.175's 1.70 rad); the non-monotonic
  σ_m-dependence (0.6 rad datum → 1.13 rad, the best of the three) reflects
  this Kalman tuning trusting the (correct) process model more when
  measurements are noisier. A filter-tuning caveat, not a hardware claim.
- **Thermal ramp:** an unmodelled 8 Hz/s ramp doubles the 20 ms residual
  (1.70 → 3.14 rad); adding the acceleration state recovers no-ramp
  performance (1.67 rad). Model the ramp or die.
- **Outliers and gating (firmware lesson):** 20 % outlier fence frames,
  3σ-gated **without a working re-lock path**, diverge (44 rad): once the state
  error exceeds the gate, updates are rejected forever and the phase free-runs.
  *Caveat, stated plainly: the script's documented "re-lock after 25
  consecutive rejections" branch is computed but not implemented — the 44 rad
  figure is the no-recovery case, and it is a real warning, not a filter
  property.* Ungated, the same outliers cost only 0.744 rad at 20 ms.
- **Averaging law (Part C; σ_face = 0.30 rad assumed, worst-ish; σ_mp = 0.08
  assumed):** common-mode components cap the gain: N* = 2.2 faces, K* = 3.1
  packets. σ_φ = 0.361 (N_f = 1) → 0.251 (N_f = 4) → 0.212 (N_f = 20) rad —
  not 1/√20. With K = 12 packets at N_f = 4: **0.120 rad** — averaging can
  buy ~3× but saturates at the track + multipath-wander floor (≈ 0.10 rad).
- **End-to-end chain (Part D; 6 nodes, 5 × 4 m, one −6.9 dB wall bounce per
  link; see `t1-h-chain.png`, left panel):** D_eff = 4.96 m, r̄ = 3.04 m.

  | datum σ_φ (rad) | precision about biased peak (cm) | CRLB scale r̄λσ/2πD (cm) |
  | --- | --- | --- |
  | 0.05 | 2.98 | 0.06 |
  | 0.10 | 11.86 | 0.12 |
  | 0.175 | 20.14 | 0.21 |
  | 0.35 | 36.70 | 0.42 |
  | 0.70 | 39.88 | 0.84 |

  The single-bounce multipath bias floor is **32.56 cm**; realised precision
  is ~100× the noise CRLB — **at home scale the chain is multipath-limited,
  not coherence-limited**: for σ_φ ≤ 0.35 rad, sharpening the LO budget buys
  almost nothing in 3D position until the static field is subtracted (the
  round-1 architecture's next gate, note `13`). (Total RMSE to truth is
  non-monotonic in σ_φ because jitter partially cancels the fixed bias —
  an MC artefact; use the precision column.)

### 3.3 HYPOTHESIS

- That S3 residual-CFO dynamics sit in the passing corner (τ_ν ≈ 100 s at
  σ_ν ≲ 100 Hz, or σ_ν ≤ 3.98 Hz outright via an ESP-PPB-class disciplined
  VCTCXO). This is the decisive unknown; both are unmeasured on S3.
- That σ_m ≈ 0.175 rad transfers from C3 to S3 (ESP-PPB's "typical", unrefereed;
  the RPi 0.04 datum is a different silicon and MAC).
- That the bracketing discipline (sibling note `23-coherence-gate-f.md`:
  two-sided reference interpolation, σ_φ ≈ 0.056–0.105 rad at 20 ms with
  τ_c = 1 s and σ_m = 0.05 rad) rescues the fast-τ_ν corners this note's
  one-sided fence fails. The two T1 notes **agree the undisciplined 20 ms
  holdover fails by 5–25×** and disagree only inside the estimator design
  space — bracket vs fence — which the same M2 measurement decides.
- That face/packet averaging obeys the common-mode law above on real hardware
  (σ_face, σ_mp assumed).

### 3.4 SPECULATION

- That a 3-state tracker with robust (Huber-weighted) innovations removes the
  gating-divergence failure entirely.
- That a conducted-truth channel (the Closure Weave's anchor) can audit the
  fence in situ, converting the slow-drift assumption into a continuously
  verified property rather than a hope.

---

## 4. Shared ledger (acceptance criterion 5)

| Quantity | Value | Provenance |
| --- | --- | --- |
| σ_φ @ 20 ms, best simulated case | **0.385 rad** (0.343 track ⊕ 0.175 datum), margin **1.30×** | SIMULATION |
| σ_φ @ 20 ms, unfenced, 1 ppb XO | 0.307 rad | closed form |
| σ_φ @ 20 ms, unfenced, 100 Hz residual | 12.57 rad | closed form |
| σ_φ @ 20 ms, killing cases | 50 Hz fence: 42.19; τ_ν = 1 s: 3.34; ramp 8 Hz/s (2-st): 3.14; outliers gated w/o re-lock: 44.04 rad | SIMULATION |
| Coherence budget B | gate ⇔ B ≤ 0.0796 cycles; B = σ_ν·T_obs (e.g. 2.44 Hz × 20 ms = 0.0488 → 0.307 rad) | closed form |
| Airtime (fence, 250 µs frames) | 1.25 % / **5 %** / 25 % at 50/200/1000 Hz | arithmetic |
| Target coherence time | 61.5 ms @ 1 m/s; 12.3 ms @ 5 m/s | λ/2v |
| Ambiguity cascade | FTM ~3 m → per-link delay CRB 0.172 ns (40 MHz, 64 sc, 20 dB) ≈ 5.2 cm → carrier mod λ/2 = 6.1 cm | lit + CRB |

**Killing values, stated explicitly:** unfenced σ_ν ≥ 3.98 Hz; fence rate
≤ 50 Hz (any dynamics); τ_ν ≤ 10 s at a 200 Hz one-sided fence; unmodelled
thermal ramps ≥ 8 Hz/s; TX-reply jitter > 1.63 µs at 20 ppm raw CFO; gated
outlier handling without re-lock. **Not killing (never binding):** thermal
CRB (0.0088 rad @ 20 dB), 12.5 ns timestamp quantisation (≤ 0.0038 rad),
CFO-fit noise (0.019 Hz).

---

## 5. Bounds on the beacon route (Part E — complement to direction f)

- **RIPS-style two-tone beat is LO-immune but carrier-phase-blind.** Beat LO
  leakage = 2π·Δf·σ_t: 6.3×10⁻⁴ rad at (Δf = 1 kHz, σ_t = 100 ns), 6.28 rad at
  (1 MHz, 1 µs). The same df/f_c ≈ 4×10⁻⁷ factor that kills the LO kills the
  geometric phase: the beat audits clocks, it cannot deliver cross-node
  carrier phase; range must come from delay (0.172 ns CRB → 5.2 cm/link).
- **Swept/differential beacon:** within-receiver face differencing cancels the
  LO, but the geometry term survives only to receiver-position accuracy:
  2π·δp/λ → **0.5 rad at δp = 9.79 mm**; FTM's metre-class positions are
  2 orders too coarse — the geometry must come from the self-survey/static-field
  machinery, not FTM. With geometry known, a 20-face puck fit at 3 m yields a
  datum of 0.019/0.043/0.137 rad for σ_m = 0.04/0.175/0.6 — the one beacon
  datum that beats per-frame noise (2–4× reduction).

---

## 6. ESP32-S3 feasibility

| Subsystem | Verdict | Basis |
| --- | --- | --- |
| Fence TX (anchor) | Feasible: 200 Hz × 250 µs = 5 % duty; ESP-NOW frames 100–200 µs | arithmetic; synthesis `07` |
| Tracker | 2/3-state Kalman per link — trivial on-node or host-side | Part B runtime |
| σ_m | 0.175 rad is a C3, unrefereed claim; S3 untested | ESP-PPB (GitHub) |
| TX jitter | No TX hardware timestamp on S3; < 1.63 µs software turnaround needed at 20 ppm | Part A; WCNC 2023 RPi datum |
| Disciplined XO route | ESP-PPB-class (sub-ppb) passes the 20 ms gate **unfenced** (0.307 rad) | Part A; port to S3 unproven |
| Timestamps | 12.5 ns RX grid negligible; no TX timestamp | synthesis `17` |
| Vocabulary | "phase-coherent RF reference" conflicts with `CONTEXT.md`; research is out-of-band; graduation needs a glossary change + ADR | ROUND2-BRIEF Finding 5 |

**Verdict:** the gate is *arithmetically* passable on S3-class hardware in two
ways — disciplined XO with no fence, or 200 Hz fence with slow residual
dynamics — and the binding uncertainties are three measured-on-nothing
numbers: (σ_ν, τ_ν) and σ_m on S3, and reply jitter.

## 7. TX needed?

Yes, for every passing configuration: the fence/beacon requires the reference
node to transmit (common-TX escape, 5 % duty at 200 Hz; the disciplined-XO
route still needs periodic reference frames per ESP-PPB). TX need not be
phase-coherent; receivers pre-rotate in software. Two-way variants are not
simulated here beyond the jitter budget (Part A), which bounds them.

## 8. Evidence strength

| Claim | 1–10 | Justification |
| --- | --- | --- |
| Closed-form budgets (holdover, jitter, B, duty, CRBs) | 9 | Exact arithmetic; parameter provenance cited; not device-specific |
| Fence Monte Carlo (specific rad values) | 7 | Standard estimator, exact OU discretisation, fixed seed; parameters swept, not measured |
| Gate verdict at 20 ms (passes in slow-drift corner) | 6 | Simulation only; hangs on unmeasured (σ_ν, τ_ν, σ_m) on S3 |
| Chain result (multipath floor ≈ 33 cm dominates) | 7 | Single-bounce geometric model, amplitude assumed 0.45; standard theory |
| Part E bounds (beat/face-fit) | 8 / 6 | Leakage identity exact; fit MC parameterised |
| **Overall** | **6–7 (simulation)** | One rig measurement (below) moves the verdict to 9 either way |

## 9. The falsifier

Pre-registered, on the CLO-1-style rig of `22-invention-h.md` §7 (two S3
nodes + reference master, 60 s captures at fence rates 50/200/1000 Hz,
temperature log): extract (σ_ν, τ_ν), σ_m and reply jitter on S3.
**Kill:** measured per-frame cross-node residual > 0.5 rad at a 20 ms cadence
with a 200 Hz fence and a disciplined clock — concretely, τ_ν ≤ 10 s at
σ_ν ≈ 100 Hz (→ ≥ 1.7 rad) or σ_m > 0.5 rad typical (→ ≥ 0.69 rad even at
τ_ν = 100 s). **Pass:** residual ≤ 0.5 rad at 20 ms for the measurement
duration, opening Tier B of the Closure Weave. The same capture decides the
bracket-vs-fence discrepancy with `23-coherence-gate-f.md`.

## 10. Primary sources

- ESP-PPB — wireless sub-ppb sync + reference frames ~1 ms after CSI, ESP32-C3,
  GPL-3.0: https://github.com/jonathanmuller/esp-ppb (accessed 2026-10;
  unrefereed; source of the σ_m = 0.175 rad "typical" figure).
- ESPARGOS — conducted-coherent ESP32-S2 array: F. Euchner et al.,
  arXiv:2502.09405, https://doi.org/10.48550/arXiv.2502.09405; datasets DaRUS,
  https://doi.org/10.18419/darus-4456; https://espargos.net/; ESP-SDR (S3 raw
  IQ): https://espargos.net/espsdr.
- RTCSI / ML-Track — round-trip CSI cancels asynchrony (0.96 → 0.04 rad),
  reply jitter < 1 µs, RPi CM4: IEEE WCNC 2023; ML-Track, IEEE TMC 2025
  (0.23 m median) — as cited in `ROUND2-BRIEF.md` Finding 3 (DOIs to pin at
  graduation).
- Residual CFO 10–100 Hz / ms–tens-of-ms coherence: MegaMIMO measurements, via
  `00-synthesis.md` §2 (note `12`); AirSync Kalman-over-the-air phase
  tracking, 2.37°: IEEE/ACM ToN, https://doi.org/10.1109/TNET.2012.2230449.
- Wi-Wi two-way carrier phase (mm-class): arXiv:1806.02130; BeamSync:
  https://doi.org/10.1109/TWC.2023.3335089; Chronos:
  https://doi.org/10.1145/2789168.2790124.
- ESP32 CSI phase behaviour: esp-csi issue #121,
  https://github.com/espressif/esp-csi/issues/121; ESP-IDF v5.5 Wi-Fi API
  (timestamps): https://docs.espressif.com/projects/esp-idf/en/v5.5/esp32/.
- Internal: `00-synthesis.md` §2 (M2, raw vs residual); `22-invention-h.md`
  (Closure Weave, CLO-1 rig); `23-coherence-gate-f.md` (bracketed T1
  companion); `docs/research/v2/code/t1-coherence-gate-h.py`.

## 11. Residual unknowns

1. **(σ_ν, τ_ν) of the S3 residual CFO** — the decisive unknown; picks the
   table row we live in.
2. **σ_m on S3** (ESP-PPB claim is C3 and unrefereed).
3. **TX-reply jitter on S3** (no TX timestamp; software discipline only).
4. OU adequacy for real LOs (PLL bandwidths, spurs, quantised behaviour).
5. σ_face/σ_mp of the averaging law (both assumed).
6. Rich multipath vs the single-bounce −6.9 dB model; interplay with
   static-field subtraction.
7. Fence-frame outlier statistics (p = 0.20 assumed; ESP-PPB dropout
   behaviour unknown) and a properly implemented re-lock path.
8. Bracket vs one-sided fence on equal parameters — unified simulation owed.

## 12. Chaining, existing models and data, own data

**Chaining.** (1) This gate decides whether `22-invention-h.md` Tier B
(beacon-held phase) is viable at the 20 ms cadence → (2) its 5 % fence-duty
line is the airtime input to the T4 protocol track → (3) the chain result
(multipath floor ≈ 33 cm at home scale) hands the next gate to the
static-field-subtraction track (note `13`): coherence below ~0.35 rad buys no
3D precision until multipath bias is removed → (4) Part E bounds direction f
(the PSB): beat observables cannot carry cross-node carrier phase; the
face-fit datum (0.043 rad) is the beacon's one win → (5) the averaging law
(K = 12, N_f = 4 → 0.120 rad) sets the packet budget for any estimator.

**Existing models and data.** ESPARGOS DaRUS dataset (conducted-truth phase,
robot + total-station ground truth, DOI 10.18419/darus-4456) can validate the
fence estimator's tracking law against real coherent arrays; DICHASUS
datasets similarly; the repo's own deterministic CSI simulator
(`cargo run -p aquila-capture -- synth`, via `nix develop .#sim`) supplies
conducted ground truth on the v1 chain. **No public dataset contains an
independent-LO cross-node residual-CFO spectrum** — precisely the (σ_ν, τ_ν)
pair this gate turns on.

**Own data.** Minimal new dataset, "fence-M2": two stock S3 nodes + one
ESP-PPB-style master, 60 s captures at fence rates 50/200/1000 Hz across a
10 °C temperature step; extracts (σ_ν, τ_ν), σ_m and reply jitter. A few
hours of capture; it is the CLO-1 rig's coherence module and the first public
measurement of M2 on ESP32-S3. Pre-registered thresholds in §9.
