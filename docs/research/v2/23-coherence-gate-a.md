# 23 — The coherence gate carried through the chain: holdover, the two-way product, the ambiguity cascade and coherent 3D localisation (track T1, direction a)

Status: round-2 research note (T1, character `a`). Australian English. Research
is out-of-band and never defines v1 scope. **Everything quantitative below is a
simulation** — a NumPy model of estimators and oscillator processes, not a
measurement of any device (per `EXPERIMENT-ENV.md` honesty rules). Code, ledger
CSV and figure:

- `docs/research/v2/code/coherence-gate-a/coherence_gate_chain.py` (rerun
  end-to-end for this note; seed `20261016`; 108 ledger rows)
- `docs/research/v2/code/coherence-gate-a/gate_a_summary.csv`
- `docs/research/v2/code/coherence-gate-a/gate_a_fig1.png` (three panels:
  A holdover σ_φ vs gap; C face-averaging law vs √N; E chain RMSE vs σ_φ)

Where this note differs from its sibling T1 notes (`23-coherence-gate-f/h/b.md`)
it is because `a` carries the gate *through* the chain it gates: the two-way
exchange estimator, the face-averaging law, the ambiguity cascade, and a
complete simulated chain **conducted calibration → fence coherence → coherent
3D localisation** — plus a dedicated treatment of missing round-1 direction
`f` (phase-swept beacon / RIPS-style hub).

---

## 1. The question

ROUND2-BRIEF Finding 4 leaves one number standing between "coherent" and "not
coherent": the per-frame cross-node phase residual σ_φ (M2) at the working
cadence, with the TX-jitter × CFO term and the holdover/coherence budget B.
Round 1 measured none of it. This note asks, and answers by simulation:

1. What σ_φ does a bracketed reference scheme hold at a 20 ms cadence, with
   what margin against the ≤ 0.5 rad gate, and what kills it?
2. What does passing the gate *buy* — does σ_φ ≤ 0.5 rad propagate to cm-class
   3D localisation through a complete chain, and where else does the chain
   break (ambiguity cascade)?
3. Does the missing direction `f` (RIPS-style hub) offer a coherence-free
   fallback with useful precision?
4. What airtime does the fence cost?

## 2. Model and parameters (the honesty block)

Shared constants: fc = 2.437 GHz (Wi-Fi channel 6), λ = 123.05 mm, two-way
carrier ruler λ/2 = 61.5 mm. Residual CFO modelled as an Ornstein–Uhlenbeck
process with correlation time τ_c = 1 s and 1-sigma σ_f ∈ {2.4, 10, 30, 100} Hz
(2.4 Hz ≡ 1 ppb ESP-PPB-class disciplined VCTCXO, grey literature; 10–100 Hz ≡
post-CFO-removal residual per MegaMIMO via `00-synthesis.md`). Bracket gaps
0.1–100 ms. MC: 400 phase realisations per point, 25 µs integration step. CSI
estimator noise σ_est = 1/√(2·64·SNR) = 0.028 rad (64 subcarriers, 10 dB SNR).
Timestamps: 12.5 ns RX latch grid (ESTABLISHED from ESP-IDF) + assumed 25 ns
rms latch jitter (characterisation pending on S3). Two-way exchange: nominal
1 ms reply, software turnaround modelled as Laplace(1 µs) + 10 % probability of
an 8 µs tail (model of a scheduler, not a measurement); turnaround asymmetry
residual 30 ns after ns-timestamp correction; raw CFO 20 ppm → 48.74 kHz; fine
CFO residual 0.2 Hz (conservative: the CSI-phase ledger below achieves
0.035 Hz). Chain: a conducted-coherent 20-face icosahedral puck (face centres
on a 15 cm radius, 30 cm puck) as sole TX at (0.2, 0.2, 2.2) m; six satellite
receivers; target at (4.0, 3.0, 1.2) m; 120 sum-phase observations; static
field perfectly removed (best case); frozen gauges; 5 mm warm start; Gauss–Newton;
300 realisations per σ_φ. RIPS part: two hub TXs at 30 kHz carrier offset,
250 kS/s decimated I/Q (ESP-SDR S3 mode), integration 1–100 ms. Airtime:
130 µs frame (≈100 B, HT20 MCS0, incl. preamble/SIFS), 8 nodes. **The
pre-interruption CSV was produced by a marginally different edit state of the
script; this note's numbers come from one clean rerun of the file now on disk —
parts A/B/D reproduce the earlier ledger exactly at printed precision, parts
C/E Monte-Carlo checks moved within MC noise (e.g. E at σ_φ = 3 rad: 19.9 vs
19.2 cm). Closed forms are unchanged.**

## 3. Findings

### 3.1 ESTABLISHED (the mathematics these simulations evaluate; closed forms Wolfram-verified, MC-checked to ≤ 6 %)

- **Holdover law.** After CFO removal, the cross-node LO phase error over an
  unbracketed gap T is σ_φ = 2π·√(Var[∫f dt]) with
  Var = 2σ_f²τ_c(T − τ_c(1 − e^(−T/τ_c))) for OU(f; σ_f, τ_c) — short-gap form
  σ_φ ≈ 2π·σ_f·T. At 20 ms the gate ⇔ **σ_f ≤ 3.98 Hz**; at 1 ms ⇔ 79.6 Hz.
- **The jitter × CFO identity.** An uncorrected reply-instant error τ enters
  the two-way product as σ_φ = 2π·Δf_raw·σ_τ. The 0.5 rad gate at 20 ppm
  (48.74 kHz) ⇔ **σ_τ ≤ 1.63 µs** per exchange. The chip's 12.5 ns RX grid
  contributes ≤ 0.0038 rad — never binding.
- **Coherence budget.** B = σ_f·T_obs (cycles); the 0.5 rad gate is
  **B ≤ 0.0796 cycles** (reproduces note h's ledger row; note b reproduced it
  independently). Target coherence time λ/2v = **61.5 ms at 1 m/s** — a 20 ms
  cadence oversamples walkers 3×.
- **The separability theorem (chain-level, E0).** The one-way bistatic sum
  phase over (TX face s, RX node i) is *exactly* additively separable in
  per-node gauges: the target's phase signature lies inside the span of the
  gauge design (max |P_orth·J_p| = 1.8e-13, machine zero, 25-dim gauge space in
  120 observations). If gauges are re-solved freely per snapshot the target is
  **unidentifiable**. Coherence must be *frozen* (fixed gauges from the
  calibration), not re-derived per frame. This is the algebraic reason the
  coherence gate exists at all.
- **Measurement floor.** MCRB per reference event σ = 1/√(2·N_s·SNR):
  0.028 rad at 64 subcarriers, 10 dB. Thermal noise never binds.
- **Ambiguity arithmetic.** Two-way carrier ruler 6.15 cm; stitched 13×20 MHz
  band ruler c/2B = 1.874 m. An m-class FTM prior resolves the *band* integer
  (99.8 % at 0.3 m prior) but essentially never the carrier integer directly
  (8.2 % at 0.3 m, 2.5 % at 1 m); carrier-only fitting with per-link integer
  spread degenerates (~66 near-equal minima in a ±1 m window; 1.5 % success).

### 3.2 SIMULATED (this note's numbers; estimator behaviour, not device)

**(a) σ_φ versus bracket gap (part A, CSV `A_holdover`)** — unbracketed
holdover after CFO removal, MC over 400 OU realisations (closed form in the
CSV notes; agreement ≤ 6 %):

| σ_f \ gap | 1 ms | 3 ms | 8 ms | 20 ms | 100 ms |
| --- | --- | --- | --- | --- | --- |
| 2.4 Hz (1 ppb) | 0.016 | 0.048 | 0.128 | **0.318** | 1.575 |
| 10 Hz | 0.060 | 0.181 | 0.483 | 1.198 | 5.845 |
| 30 Hz | 0.187 | 0.561 | 1.499 | 3.765 | 18.44 |
| 100 Hz | 0.642 | 1.922 | 5.118 | 12.75 | 62.87 |

Holdover time to the 0.5 rad gate: T* = 0.5/(2πσ_f) = **33.2 ms** (1 ppb),
7.96 ms (10 Hz), 2.65 ms (30 Hz), 0.80 ms (100 Hz).

**Gate verdict at the 20 ms working cadence (acceptance criterion 2):** at
σ_f = 2.4 Hz, σ_φ = **0.318 rad — PASS with 1.57× margin** (closed form 0.301
rad, 1.66×). In budget terms B = 0.048 cycles against the 0.0796 gate. Any
residual σ_f > 3.98 Hz fails at 20 ms unbracketed — the 10 Hz "residual"
reading of round 1 fails by 2.4×, stock 100 Hz by 25×. The unbracketed ppb
point is the *weakest* passing design; bracketing (f, b) multiplies the margin.

**(b) Where the CFO fix comes from (part A2, CSV `A_timestamp_cfo`).** The ns
RX timestamps alone give the relative CFO only slowly: 470 Hz after 20 frames
at 5 ms spacing, 21.3 Hz after 100 frames at 10 ms (≈1 s). That is nonetheless
enough to *unwrap* 5 ms-spaced CSI phases (2π·21·5 ms = 0.66 rad < π), after
which the CSI-phase slope supplies the fine fix: **0.035 Hz from 20 frames over
100 ms; 0.003 Hz from 100 frames** (CSV rows 45–46). The 0.2 Hz fine residual
used in the two-way MC is therefore conservative; its phase cost over a 1 ms
reply is 1.3e-3 rad — negligible.

**(c) The two-way exchange estimator (part B, CSV `B_twoway`)** — 1 ms nominal
reply, 20 ppm raw CFO, 30 Hz wander, 2000 frames:

| design | σ_φ [rad] | vs 0.5 rad gate |
| --- | --- | --- |
| raw software reply (1 µs Laplace + 10 % 8 µs tail) | **0.876** | **FAIL 1.75×** |
| ns-timestamp corrected (30 ns asymmetry residual) | **0.266** | PASS 1.88× |
| corrected + 3 µs deferral gate | 0.274 | PASS 1.83× |
| corrected + 1 µs deferral gate | 0.272 | PASS 1.84× |
| ratio-fix variant (two stamps, no turnaround term) | 0.040 | PASS 12.6× |

The raw product fails **even with perfect oscillators** — the failure is
turnaround-jitter × raw CFO (a 1 µs rms jitter alone is 0.306 rad at 20 ppm;
the 8 µs tail pushes the realised distribution to 0.876 rad). Timestamp
correction is mandatory and sufficient; a deferral gate is nearly free
(≈ 0.008 rad cost). Scene non-reciprocity over the 1 ms reply is not killing at
home speeds: 0.051 rad at 0.5 m/s radial, 0.102 rad at 1 m/s.

**(d) Face/packet averaging law (part C, CSV `C_averaging`, fig. panel C).**
With components wander 0.188 rad (30 Hz, 1 ms), per-frame artefact 0.05 rad,
estimator 0.028 rad, frozen per-face bias 0.10 rad: the snapshot σ falls only
from 0.221 rad (N = 1) to 0.196 rad (N = 40) — effective exponent β ≤ 0.08
against the √N ideal. Wander and per-frame artefact are common across faces and
**do not average**; only estimator noise and frozen bias do. Face averaging
buys ~nothing at the gate; the budget must be won before averaging.

**(e) The ambiguity cascade (part D, CSV `D_cascade`/`D_degeneracy`).** The
chain FTM → stitched band → carrier phase survives or dies on rung 2→3:
stitched-band two-way range σ = 5.27 cm at 0 dB SNR (64 subcarriers) resolves
the carrier integer only 44 % of the time; **1.67 cm at 10 dB → 93.5 %**;
0.53 cm at 20 dB → 100 %. Carrier-only fitting is hopeless regardless of σ_φ
(~66 near-equal minima). So the cascade needs ≥ ~10 dB on the stitched band —
σ_φ passing the gate is necessary but not sufficient.

**(f) End-to-end chain (part E, CSV `E_e2e`, fig. panel E)** — frozen-gauge
GLS CRLB and MC (300 realisations) for 3D localisation, σ_n = 0.028 rad,
per-RX fence residual σ_φ common across faces:

| σ_φ [rad] | CRLB [cm] | MC RMSE [cm] |
| --- | --- | --- |
| 0.05 | 0.23 | 0.33 |
| 0.1 | 0.31 | 0.64 |
| 0.2 | 0.43 | 1.36 |
| 0.3 | 0.54 | 1.99 |
| 0.5 | 0.79 | 3.16 |
| 1.0 | 1.45 | 11.9 |
| 2.0 | 2.81 | 16.5 |
| 3.0 | 4.18 | 19.9 |

At the design point σ_φ = 0.3 rad the chain delivers **≈ 2 cm 3D RMSE**
(CRLB 0.54 cm; MC ≈ 3.7× CRLB — the gap is warm-start/wrap effects, flagged in
residual unknowns). dCRLB/dσ_φ = 0.24 mm/rad at 0.3 rad. The CRLB-scaled
σ_φ for a 10 cm bar is 6.9 rad — i.e. **precision is not what binds**; the
gate binds through *ambiguity control* (phase wrapping beyond ~π, and the
cascade in (e)). A chain that holds σ_φ ≤ 0.5 rad and ≥ 10 dB stitched-band
SNR localises at cm class in the simulated LoS scene.

**(g) Direction f: the RIPS-style hub (part F, CSV `F_rips`).** Two hub TXs at
30 kHz offset; the beat phase is invariant to receiver LO error by common-mode
algebra — **MC spread 2.5e-15 rad over 200 random LO phases** (machine zero).
Beat-phase noise → range-difference error at 250 kS/s: 0.554 mm (ρ = 0 dB,
T = 10 ms), 0.175 mm (10 dB, 10 ms), 0.006 mm (30 dB, 100 ms). Cross-receiver
alignment on the 12.5 ns grid costs 0.236–4.712 mrad per step at 3–60 kHz
beat. The hub route buys sub-mm range *differences* with **no cross-node
carrier coherence at all** — the coherence-free fallback if the M2 gate fails
on hardware.

**(h) Airtime (part G, CSV `G_airtime`).** 130 µs frames: round of 8 nodes =
1.17 ms → duty 1.2 % (10 Hz), 2.3 % (20 Hz), 5.8 % (50 Hz). Reference strobe
(hub broadcast): 2.6 % (200 Hz), 6.5 % (500 Hz), 13 % (1 kHz). Bracket pair
per sensing frame: **0.5 % (20 Hz), 1.3 % (50 Hz), 2.6 % (100 Hz)**. The fence
is airtime-free at home scale; the protocol question (T4) is arbitration, not
duty.

### 3.3 HYPOTHESIS (plausible, not established here)

- That a 1-ppb-class residual is *achievable on S3* between 1 ms brackets:
  ESP-PPB demonstrates FTM-disciplined VCTCXO behaviour on ESP32-C3 only,
  unrefereed, no S3 port. The whole unbracketed verdict rests on it.
- That the turnaround-asymmetry residual after timestamp correction is ~30 ns,
  and that the Laplace-plus-tail turnaround model resembles S3 scheduler
  behaviour. Both are assumptions wearing a distribution.
- That the per-frame artefact is 0.05 rad (ESP-PPB "typical" is 0.175 rad; at
  0.175 the N = 1 floor in (d) rises to ≈ 0.25 rad — still passing, margin 2×).
- That static-field removal is perfect and the scene is LoS. Multipath shifts
  the frozen-bias term and the cascade rung, not the holdover law.
- The OU τ_c = 1 s for real oscillator ADEV; a Wiener model (b's β) is the
  other regime, and σ_f = 4 Hz ≠ β = 4 Hz across models.

### 3.4 SPECULATION

- If σ_φ ≤ 0.1 rad *and* the cascade closes at ≥ 20 dB stitched-band SNR, the
  near-field curvature of a house-scale coherent fleet (Rayleigh distance
  0.4–1.6 km) puts mm–cm 3D within reach — round 1's Finding 4(5), now with a
  chain that reaches 2 cm at σ_φ = 0.3 rad in LoS simulation.
- The RIPS hub route (g) may make cross-node coherence *unnecessary* for
  ranging altogether, collapsing the coherence-first/geometry-first split from
  the measurement side rather than the algebraic side.

## 4. Decisive numbers (the shared ledger, acceptance criterion 5)

| quantity | value | provenance |
| --- | --- | --- |
| σ_φ at 20 ms cadence, 1 ppb class, unbracketed | **0.318 rad** (MC; 0.301 closed form) — **PASS, 1.57× margin** | `A_holdover`, seed 20261016 |
| σ_φ at 20 ms, bracketed two-way, corrected | **0.266–0.274 rad** — PASS 1.8× | `B_twoway` |
| σ_φ raw software reply, uncorrected | **0.876 rad — FAIL** | `B_twoway` |
| Killing σ_f at 20 ms (unbracketed) | **3.98 Hz** | `A_budget` |
| Killing reply jitter at 20 ppm | **σ_τ > 1.63 µs** | `A_budget` |
| Coherence budget gate | **B = σ_f·T_obs ≤ 0.0796 cycles**; design point B = 0.048 | `A_budget` |
| Airtime | bracket 0.5 % @ 20 Hz; strobe 2.6 % @ 200 Hz; round 2.3 % @ 20 Hz | `G_airtime` |
| Target coherence time | **61.5 ms @ 1 m/s** (20 ms cadence oversamples 3×) | `G_ledger` |
| Chain payoff | σ_φ = 0.3 rad → **2.0 cm 3D RMSE** (MC); wrap floor ≈ 19 cm beyond ~3 rad | `E_e2e` |
| Cascade requirement | stitched-band σ ≲ 1.7 cm (≥ 10 dB) → 93.5 % carrier-integer | `D_cascade` |
| Face-averaging exponent at the gate | β ≤ 0.08 (not √N) | `C_averaging` |

## 5. The falsifier (named)

**The bracketed two-way M2 rig on two stock ESP32-S3 boards:** one board pings,
the other replies; 1 ms brackets around a 20 ms gap; a conducted split of one
common reference as ground truth; ≥ 10⁴ exchanges. The coherence-first
programme **dies if any of**: (i) the corrected two-way σ_φ exceeds 0.5 rad;
(ii) the reply-interval residual cannot be corrected below the 1.63 µs
equivalent (i.e. corrected asymmetry ≫ 30 ns); (iii) the per-packet phase
artefact alone is ≥ 0.5 rad. The *localisation* chain additionally dies if the
stitched-band delay bias is ≥ ~3 cm (rung 2→3 success falls through ~70 %
between this note's 1.67 cm/93.5 % and 5.27 cm/44 % — agreeing with note b's
independent 3 cm threshold). Each condition is a single scalar on a CSV row.

## 6. ESP32-S3 feasibility

Feasible in simulation, unproven on hardware, with three named hardware gaps:
(i) **no TX hardware timestamp outside FTM** (esp_wifi issue #9843) — the
correction must run on RX-timestamp differences of the same frame at both
ends, which the 12.5 ns grid supports; (ii) **ESP-PPB has no S3 port** — the
1-ppb-class reference supply is C3-only grey literature; (iii) S3 CSI phase
quality at a 20 ms cadence is uncharacterised (esp-csi issue #121 documents
C3 quirks only). With those closed, the simulated design point (bracketed
two-way, timestamp-corrected, 20 ms cadence) holds σ_φ ≈ 0.27–0.32 rad with
1.6–1.9× margin and costs ≤ 3 % airtime. A stock free-running S3 pair fails
twofold: uncorrected turnaround (0.876 rad) and wander (1.2–12.7 rad at
20 ms for 10–100 Hz residual).

## 7. TX needed?

Yes for the two-way product: the fence node must transmit the ping/strobe and
each node must transmit its unique reply (the ratio-fix variant needs TX on
both ends of every link; it is also the most accurate, 0.040 rad). A
receive-only node can ride the hub broadcast strobe (f's D2 route, 0.056 rad)
for the *fence fix*, but ranges and two-way products require TX. The RIPS hub
(g) needs two hub TXs only; satellites stay receive-only at 0.55 mm
range-difference class — the lowest-TX option that still delivers numbers.

## 8. Evidence strength: **6 / 10**

The mathematics is ESTABLISHED and over-determined: closed forms Wolfram-verified
and matched by MC to ≤ 6 % across four independent parts; the separability
theorem is a machine-zero algebraic check; the B-gate and the 1.63 µs jitter
kill are reproduced independently by notes f, h and b. What keeps this at 6:
every device-facing parameter (1 ppb holdover, 30 ns asymmetry, 0.05 rad
artefact, Laplace-plus-tail turnaround, τ_c = 1 s) is grey-literature or
assumed; there is no S3 measurement anywhere in the chain; and the MC sits
3.7× above the CRLB at the design point (understood, but unresolved). Same
tier as the sibling T1 notes — the gate is now *specified* to the register
level, not *measured*.

## 9. Primary sources

- ESP-PPB — FTM-disciplined VCTCXO reference frames, < 1°/< 10° single-frame
  phase on ESP32-C3 (unrefereed; source of the 1 ppb / 0.05–0.175 rad anchors):
  https://github.com/jonathanmuller/esp-ppb
- ESPARGOS — conducted phase-coherent CSI reference pattern: F. Euchner et al.,
  arXiv:2502.09405, https://doi.org/10.48550/arXiv.2502.09405; datasets
  https://doi.org/10.18419/darus-4456; https://espargos.net
- ESP-SDR — raw I/Q on ESP32, S3 continuous decimated mode (250 kS/s used in
  part F): https://espargos.net/espsdr
- RTCSI — round-trip CSI cancels asynchrony (0.96 → 0.04 rad), reply-interval
  std < 1 µs on Raspberry Pi CM4, Shi et al., IEEE WCNC 2023; UCL Discovery
  eprint 10161602: https://discovery.ucl.ac.uk/id/eprint/10161602
- ML-Track — multi-link RTCSI tracking, IEEE TMC 2025, DOI: 10.1109/TMC.2025.3529897
- WiDRa — sum-carrier-phase differential ranging, JSAC 2024,
  https://arxiv.org/abs/2405.12168
- LoSRef — the LoS path as an OTA delay+phase reference,
  https://arxiv.org/abs/2602.05344
- AirSync — Kalman over-the-air phase tracking, 2.37°, IEEE/ACM ToN,
  https://doi.org/10.1109/TNET.2012.2230449 (residual-CFO ladder provenance)
- ESP-IDF, *Wi-Fi (esp_wifi) API Reference — ESP32-S3* (12.5 ns RX TSF grid; no
  TX timestamp outside FTM, issue #9843):
  https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/api-reference/network/esp_wifi.html
- ESP32 CSI phase behaviour: https://github.com/espressif/esp-csi/issues/121
- Internal: `ROUND2-BRIEF.md` Finding 4; `22-invention-{a,c,s,t,k}.md`;
  sibling T1 notes `23-coherence-gate-{f,h,b}.md`.

## 10. Residual unknowns

1. S3 reply-instant distribution (the Laplace-plus-tail model is a guess) —
   the single most load-bearing unknown; one rig measurement closes it.
2. ESP-PPB-class discipline on S3 (port + verification).
3. S3 per-packet CSI phase artefact at 20 ms cadence (0.05 vs 0.175 rad —
   a 5× swing in the averaging floor).
4. The MC-vs-CRLB gap at σ_φ ≤ 0.2 rad (warm start + wrapping; a better
   initialised estimator should close it).
5. Real oscillator ADEV regime (OU τ_c = 1 s vs Wiener β) — decides how much
   of the bracket gain survives.
6. Multipath: the chain is simulated LoS with perfect static-field removal;
   the frozen-bias term absorbs real multipath only if it is truly static.
7. Mutual coupling/polarisation of the 20-face puck (T3's problem; assumed
   ideal manifolds here).

## 11. Cross-reconciliation with the f/h/b sibling notes

- **The gate at 20 ms: all four notes PASS, spread explained by reference
  supply.** a: **0.318 rad** (unbracketed, σ_f = 2.4 Hz, MC); f: 0.056 rad
  (D2, broadcast fix with 1 ms brackets) and 0.105 rad (D1, two-way corrected);
  h: **0.385 rad** (best passing point, 200 Hz fence, τ_ν = 100 s) and
  0.307 rad (closed-form 1-ppb unfenced); b: **0.164 rad** (bracket ±0.5 ms at
  β = 4 Hz) and ≥ 3× margin for any β ≤ 4 Hz. a and h are the *same design
  point* — h's 0.307 rad is the closed form of a's 0.318 rad MC (3.5 %, within
  MC error). f and b sit lower because they *bracket* the reference, which
  suppresses the low-frequency wander 10–30× (f's part A; b's σ_bracket =
  √(πβT + σ_p²/2) vs σ_hold = √(4πβT + σ_p²), a 4× variance win). Consistent
  picture: **unbracketed ppb-class is the weakest passing design (1.3–1.6×
  margin); 1 ms brackets buy 3–9× for ≤ 0.5 % airtime.**
- **The jitter kill: independent agreement.** a's `A_budget` row gives
  1.633 µs at 20 ppm — h and b both derive **1.63 µs** from the same identity
  (2π·Δf·σ_τ = 0.5 rad) by different routes, and b states it reproduces h. a's
  contribution is the *realised* MC: with a 1 µs Laplace + 10 % 8 µs tail the
  raw product is 0.876 rad (tail-dominated, worse than the 1-σ identity's
  0.306 rad), and timestamp correction to 30 ns recovers 0.266 rad — matching
  f's deferral-gate finding (0.851 → 0.411 rad at a 5 µs gate) in shape.
- **Unfenced stock hardware fails: unanimous.** a: 1.20–12.7 rad at 20 ms for
  σ_f = 10–100 Hz, plus the oscillator-independent turnaround failure
  (0.876 rad); f's D3: 4.277 rad; h: 12.57 rad at 100 Hz; b: holdover fails
  for β ≥ 4 Hz. a adds a second failure mode none of the others separates:
  without a TX timestamp the two-way product fails *even with perfect
  oscillators*.
- **The cascade: a numbers-out b's threshold.** b alone resolves the cascade's
  binding constraint (stitched-delay bias ≥ 3 cm, not σ_φ). a's rung 2→3 MC
  brackets it independently — 1.67 cm → 93.5 % carrier-integer success, 5.27 cm
  → 44 %, interpolating to ~70 % at 3 cm — and a's `D_degeneracy` shows why a
  band rung is mandatory at all (carrier-only fitting: ~66 near-equal minima,
  1.5 % success). a and b agree the cascade, not σ_φ, is where end-to-end
  chains die.
- **Noise-model caveat (b's point, adopted).** a and h use OU/linear σ_f
  (τ_c = 1 s); b uses Wiener β (ADEV slope). The ladders are numerically
  similar but not the same process; the shared ledger should carry both
  labels. h's B ≤ 0.0796 cycles is reproduced here (and by b) — that row is
  now triple-confirmed.
- **Averaging: three concordant laws.** a: wander + per-frame artefact do not
  average across faces (β ≤ 0.08 at the gate); f: √N_eff with ρ_c = 0.5 (8
  frames buy 2.0×); b: √N within a coherence window, nothing beyond. Same
  conclusion, three parameterisations: **averaging cannot rescue a failed
  fence.**
- **Airtime: minor frame-length divergence.** a assumes 130 µs/frame, h 250 µs
  — a is ~2× more optimistic in duty, but both conclude the fence costs ≤ 5 %
  at working rates; the disagreement is cosmetic and resolved by one frame-time
  measurement.
- **Vocabulary conflict (flagged, per the brief):** this note's "fence",
  "bracket", "two-way product" and the RIPS route all live in the space
  `CONTEXT.md` currently bans ("phase-coherent RF reference", ESP-NOW, Doppler,
  raw I/Q). Any graduating invention needs the glossary change and an ADR, as
  the sibling notes also flag.

## 12. Chaining / existing models and data / own data

### (A) Chaining

σ_φ (this note) is the input to: the chain CRLB (E: dCRLB/dσ_φ = 0.24 mm/rad
at the design point — T2/T3 can lift the number directly); the coherence
budget B (gates holdover-based designs and the T4 protocol's re-fix cadence);
the ambiguity cascade D (gates whether FTM→band→carrier localisation is
reachable — T4's scheduling must protect ≥ 10 dB stitched-band exchanges); and
the RIPS fallback F (gives T2 a coherence-free sub-mm range-difference sensor).
The falsifier's three scalars are the hand-off to any round-3 hardware agent.

### (B) Existing models and data

The repo's deterministic simulator (`cargo run -p aquila-capture -- synth`,
needs `nix develop .#sim`) can replace this note's idealised sum-phase model
with channel-level CSI for the same geometry; the ESPARGOS DaRUS datasets
(DOI 10.18419/darus-4456) provide measured conducted-coherent phases to
calibrate the artefact term; the RTCSI/UCL eprint supplies the only measured
reply-jitter datum (< 1 µs, RPi CM4 — not S3). Round-1 parameter sources:
`12-*` (CFO, 2.437 GHz), `17-time-frequency-precision.md` (FTM/timestamp
registers), `22-invention-{a,c,g,s,t,k}.md` (two-way product physics).

### (C) Own data / new technology

None — everything in this note is simulation; no new hardware data exists in
the repo. The minimal new measurement is the falsifier rig of §5: two S3
boards, one conducted reference split, 10⁴ bracketed exchanges — it returns
σ_φ, the reply-jitter distribution and the per-packet artefact in one CSV.
Genuinely new technology would be an S3 port of the ESP-PPB discipline
(firmware work, unrefereed upstream) and a 3-state tracking bracket that
fuses h's tracker with f's interpolation; both are specified, neither exists.
