# 12 — Consensus, oscillator locking, drift vs update rate

*Aquila v2 deep-research note. Reference hardware: ESP32-S3. Australian English.
All claims tagged ESTABLISHED / HYPOTHESIS / SPECULATION. Primary sources listed at
the end with URLs/DOIs.*

---

## Question

Can nodes agree on **time and frequency** via consensus or coupled PLLs, and how
does **update rate trade against oscillator drift**? Specifically:

- (a) consensus clock synchronisation and convergence;
- (b) PLL injection/locking (carrier-frequency and phase locking);
- (c) ESP32 crystal/TCXO drift (ppm) and the resulting coherence time;
- (d) update rate vs holdover.

The note must also **reconcile** an apparent conflict: a raw between-node relative
phase that wraps every ~5–10 µs (untracked CFO) versus a much longer coherence
after CFO removal — and say **which timescale governs each calibration method**.

Overlap guard: not `10-ota-reciprocity` (the ratio identity), not
`11-common-ref-two-way` (the reference transfer), not `17-time-frequency-precision`
(what the ESP32 clock *is*). This note is about the **control law** (consensus / PLL
/ feedback), the **drift budget**, and the **update-rate↔holdover optimum** that sits
on top of 17's hardware facts.

---

## Summary answer

Three different things are routinely conflated under "synchronisation" on a
receive-only, independent-LO mesh. Keeping them apart is the whole answer:

| Layer | What is shared | Achievable on ESP32-S3? | Timescale that governs it |
|---|---|---|---|
| **Time** (offset + skew) | *when* a sample/frame happened | **Yes** — µs-class; ±30 µs measured (OpenWiFiSync), RFA ~100 µs | Update interval; TSF 1 µs tick |
| **Frequency** (carrier) | LO frequency, to some Hz | **Yes, weakly** — track/estimate CFO per frame; residual sets the number | Residual CFO error + drift |
| **Carrier phase** (coherence) | LO phase at the sampling instant | **No** wirelessly on stock parts; **yes** with a **common reference** (wired clock+phase, or common TX) | Residual CFO + phase noise; needs continuous re-track |

- **Consensus clock synchronisation** over a Wi-Fi mesh is a *time* (and weak
  *frequency*) primitive. It converges in tens to hundreds of beacon intervals and
  has **never been shown to deliver carrier phase coherence** across independent
  LOs (ESTABLISHED across the WSN and distributed-array literature).
- **Coupled-PLL / injection-locking** does lock carriers, but the literature
  (Mudumbai; Quitin; AirSync; MegaMIMO) builds it on **dedicated SDR/FPGA radios or
  a wired reference**, and every working system **re-tracks continuously** rather
  than trusting a one-time calibration. It is not demonstrated on ESP32-S3.
- **ESP32-S3 drift** is set by a **±10 ppm** external 40 MHz crystal
  (Espressif requirement; no TCXO ships by default). Two in-spec modules can sit
  **~20 ppm ≈ 49 kHz** apart and wander with temperature (ESTABLISHED).
- **Update rate vs holdover** has a genuine interior optimum: too slow and
  oscillator drift dominates; too fast and estimation noise dominates. In the
  distributed-array analysis the optimum lands at an update interval of **≈20 ms**
  for a 1 GHz array (DFPC/KF-DFPC) — the same order as the post-CFO-removal
  coherence that MegaMIMO measures (10 Hz → 0.35 rad in 5.5 ms; 100 Hz → π in
  20 ms).

### The reconciliation (the task's central request)

**The raw ~5–10 µs wrap is the *untracked-CFO* timescale; the longer coherence is
the *post-CFO-removal residual* timescale. Both are real; they govern different
calibrations.**

The relative phase between two independent LOs evolves as `φ(t) = 2π·Δf·t`. It
crosses the phase-detector ambiguity (`π`) at

```
t_π = 1 / (2·Δf)        t_2π = 1 / Δf
```

| Differential CFO `Δf` | at 2.437 GHz | `t_π` (half-wrap) | `t_2π` (full wrap) |
|---|---|---|---|
| 40 ppm (two ±20 ppm radios, opposite signs) | ~97.5 kHz | **5.1 µs** | 10.3 µs |
| 20 ppm (two ±10 ppm ESP32 modules) | ~48.7 kHz | **10.3 µs** | 20.5 µs |

So the "~5–10 µs wrap" is exactly `t_π = 1/(2Δf)` for a 20–40 ppm differential
CFO — i.e. **two stock ESP32-S3 modules (or two arbitrary 802.11 radios)** before any
CFO handling. It is a property of the *untracked* offset, not of the crystal's
long-term stability.

Once CFO is **estimated and removed** (per-frame, from the preamble/HT-LTFs) or
**continuously tracked**, the residual frequency error `σ_f` is what remains, and it
is tiny compared with the raw offset — CRB-class (a few Hz for a good estimator;
Mudumbai's own digital bound: `σ̂_f = 2.5 Hz`). The phase then stays usable far
longer: MegaMIMO measures that a residual of **100 Hz** produces a π error only after
**~20 ms**, and even **10 Hz** takes **5.5 ms** to reach 0.35 rad.

**Which timescale governs each calibration:**

| Calibration / method | Governing timescale | Why |
|---|---|---|
| Naive **absolute cross-node phase**, per-node AoA, absolute ToF, "calibrate once and predict" | **~5–10 µs (raw CFO)** | No CFO removal; the common phase rotation is a sawtooth at `1/Δf`. This is why stock multi-NIC arrays need a phase reference, not just a clock. |
| **Per-frame CFO-sanitised CSI**, CSI-ratio, per-packet phase sanitisation | **post-removal residual (ms–tens of ms)** | CFO is measured each frame (HT-LTF pair, 4 µs apart) and divided out; residual drifts on the oscillator timescale. |
| **Continuous-tracking coherent TX** (AirSync cyclic-prefix Kalman; MegaMIMO sync header; one-bit feedback) | **residual (ms), refreshed every packet** | Re-measures phase against a reference every joint transmission; rejects elapsed-time prediction. |
| **Consensus / PLL frequency locking** | **update interval vs drift** (optimum ≈20 ms) | Must update faster than differential drift accumulates; slower than the estimator's noise floor. |
| **Holdover** (no updates) | **drift × elapsed time** | ±10 ppm ⇒ 10 µs/s time, 24 kHz·2π rad/s phase; coherence lost in µs–ms. |

This is why "the crystal question" fails for coherent phase but holds for
sanitised phase and for time (the cross-node phase-coherence adjudication reaches the
same split, independently).

---

## (a) Consensus clock synchronisation

### ESTABLISHED

- **The canonical algorithm is Average TimeSync (ATS).** Schenato & Fiorentin
  (Automatica 2011; earlier CDC 2007) define a *cascade of two consensus
  algorithms*: the first drives all nodes to a common **virtual clock rate**
  (skew), the second to a common **offset**. ATS is fully distributed (no root),
  asynchronous, robust to packet drop and node failure, and adaptive to
  time-varying drift and topology. It is the first such scheme with a **rigorous
  convergence proof** under ideal (noise-free, delay-free) links.
- **Convergence is slow in rounds, not instant.** For a 5×7 grid (network diameter
  10) ATS converges after ~**120** synchronisation rounds; a line network
  (diameter 8) needs ~60. Convergence time grows with the **diameter**, not just N.
- **Second-order / max / weighted / cluster variants speed it up.** SO-DCTS
  (Xiong & Kishore 2008), MTS, WCCS, and MACTS (Shi et al., IEEE TII 2020) report
  hundreds-of-times faster convergence than ATS by shortening the effective
  diameter (multi-hop virtual links, cluster heads).
- **Pulse-coupled oscillators (fireflies) are an alternative control law.**
  Werner-Allen et al. (SenSys 2005) built the Reachback Firefly Algorithm and
  synchronised a real indoor multi-hop testbed to within **~100 µs**; PCO
  synchronisation is "parsimonious" in packets and can be phase-advance-only or
  include skew compensation. Convergence is governed by the **coupling strength,
  topology and phase-response function**; phase-response design can raise the sync
  rate without more power.
- **Consensus is a *time* primitive.** Every one of these algorithms synchronises
  **clock readings** (offset/skew). None carries the LO's carrier phase; the phase
  offset of the RF PLL is not even *represented* in a TSF/clock model.
- **Consensus is not optimal under delay.** Xiong & Kishore show consensus
  performance degrades with message delay; belief-propagation estimators beat it.
  Modern distributed time sync (OpenWiFiSync, PTP/802.1AS over Wi-Fi, TSN) uses
  **master–slave offset/skew estimation**, not average consensus, because it is
  cheaper and delay-robust.

### HYPOTHESIS

- A **consensus time-sync** layer could run over Aquila's compliant-TX
  round-trip scheme (every node timestamps every frame; iterate on skew/offset)
  and reach **µs-class agreement on ESP32-S3**. The pieces exist (TSF 1 µs tick,
  µs/ns RX timestamps, `esp_wifi_80211_tx`), but **no published ESP32 consensus
  implementation** was found; OpenWiFiSync is master–slave RBIS, not consensus.
- Weighted/cluster consensus (WCCS, MACTS) is the right shape for a
  soldered-polyhedron cluster: an intra-cluster consensus (shared clock possible)
  plus an inter-cluster consensus (wireless, µs).

### SPECULATION

- A **consensus *frequency* lock** (average the FO estimates, steer each PLL) could
  reduce the *differential* CFO below the one-pair 49 kHz — but only down to the
  estimator/loop floor, and it still cannot pin the **per-boot PLL phase**. It buys
  a smaller number, not coherence.

---

## (b) PLL injection / coupled-oscillator locking

### ESTABLISHED

- **Master–slave PLL locking for distributed transmit beamforming** is the
  reference design. Mudumbai et al. (2004–2009) broadcast a **reference carrier**
  from a designated master; each slave locks a **second-order PLL/VCO** to it. In
  steady state the slave VCO reproduces the master carrier, and the residual is:
  (i) a constant per-node phase offset `γᵢ` (calibrated once, round-trip), (ii)
  channel-estimation error, and (iii) **VCO drift `φᵈᵢ(t)` during transmission** —
  the dominant term.
- **The residual drift forces a time-division lock/transmit cycle and creates the
  overhead↔gain tradeoff.** Because slaves transmit open-loop (VCO held), they drift
  apart; the master must periodically re-broadcast to re-lock. T₁ = lock/PLL-settle
  time (overhead), T₂ = transmit time (useful). Larger tolerated phase error ⇒
  larger T₂ ⇒ less overhead.
- **Phase-error tolerance is generous.** ≥70% of maximum beamforming gain with
  phase errors up to **60°**; ≥91% average gain with RMS error **<24°**. So
  "synchronisation does not have to be perfect" (Mudumbai).
- **Injection-locked (ILOA) and coupled (COA) oscillator arrays** are the
  hardware-level analogue: Adler's equation governs lock range and stable
  phase differences; the **comparison of ILOA and COA** (MTT 2015) quantifies
  probability-of-lock and beam-steering error against the coupling network and
  free-running frequency detuning.
- **Practical commodity-radio locking uses a Kalman filter on wrapped phase.**
  Quitin et al. (TWC 2013) lock each transmitter's LO to the **feedback packets**
  from the receiver; an **EKF** tracks the **unwrapped** LO phase from **wrapped**
  phase/frequency measurements, with process noise derived from the oscillator's
  **Allan variance** (`σ²_y(τ) = q₁²τ + q₂²τ³`). This works with **low-quality
  oscillators** and no manual calibration.
- **AirSync** (Balan et al., MobiHoc 2012) locks APs to a **common over-the-air
  reference** and **continuously tracks phase drift with a Kalman filter**, reaching
  a few degrees — a digital circuit in the WARP FPGA.
- **MegaMIMO** (Rahul, Kumar, Katabi; SIGCOMM 2012) re-measures the AP-to-AP phase
  difference **against the lead AP's sync header on every joint transmission**, and
  explicitly **rejects** "frequency-offset × elapsed-time" prediction because the
  error accumulates (`100 Hz → π in 20 ms`).
- **Receiver-feedback (one-bit) control** (Mudumbai et al., arXiv cs/0603072) is a
  different closed loop: each transmitter dithers its phase and keeps the change
  if the receiver's one-bit SNR feedback says so. It converges **asymptotically to
  perfect coherence** with convergence time **linear in N**, and tracks a
  time-varying channel **iff** the channel time-scale is longer than the
  convergence time.

### HYPOTHESIS

- The one-bit-feedback loop is attractive for Aquila because it needs **no
  phase measurement at the TX** and no common reference — only an RX that can
  broadcast one bit of SNR feedback. It is the least hardware-demanding path to
  *coherent* gain, but its convergence (hundreds–thousands of timeslots) must beat
  the channel/oscillator coherence.

### SPECULATION

- A **conducted/OTA common reference** (see 11) plus a per-node PLL is the only
  route to sustained carrier lock on ESP32-class parts; injection-locking two
  stock ESP32 LOs to each other over the air is not plausible without exposing the
  PLL/VCO (not available on the S3).

---

## (c) ESP32 crystal/TCXO drift (ppm) and coherence time

### ESTABLISHED

- **The ESP32-S3 requires an external main crystal, specified ±10 ppm**, and
  Espressif *requires* the Wi-Fi crystal within ±10 ppm (Hardware Design Guidelines;
  the S2/S3 guidelines state the selected crystal accuracy must be within ±10 ppm
  and recommend trimming the centre offset to within ±10 ppm via load capacitance).
  The part **cannot operate without the external crystal**; the internal fast RC
  (~17.5 MHz, ±1% typ) is not a coherent clock source.
- **No TCXO is standard.** A stock ESP32-S3 module uses an **AT-cut crystal**, not
  a TCXO (1–5 ppm) or OCXO (0.01–0.05 ppm). Any claim of better than ~10 ppm needs
  a different oscillator, not firmware.
- **Worst-case two-node spread.** Two in-spec modules (±10 ppm) can differ by
  **20 ppm ≈ 48.7 kHz at 2.437 GHz**. Two arbitrary 802.11 radios can differ by
  **±40 ppm ≈ 97.5 kHz** (802.11 mandates ±25 ppm at 2.4 GHz, ±20 ppm at 5 GHz;
  ±40 ppm = two opposite-sign 20 ppm parts).
- **Measured ESP32 skew.** OpenWiFiSync measured a **constant ~3.96 ppm** TSF skew
  between two ESP32-S3 modules over 3000 beacons (~5 min) — consistent with a
  non-trivial fraction of the ±10 ppm budget, and a usable servo input.
- **The PLL initial phase is a separate, random offset.** ESPARGOS (Euchner et al.)
  determined by measurement that the ESP32's LO **PLL initial phase changes after
  every chip reset and every channel change**, *even when all chips share a
  daisy-chained 40 MHz reference clock*: **"reference clock distribution is
  insufficient for achieving phase coherence."** A frequency ledger carries zero
  information about this offset. ESPARGOS therefore distributes a **wired Wi-Fi
  phase-reference signal** in addition to the clock.
- **Coherence times implied.** Single-module +10 ppm at 2.4 GHz = 24 kHz ⇒ phase
  half-wrap in ~20.8 µs; two-module differential 20 ppm ⇒ ~10 µs. After CFO
  removal, coherence is `1/(2σ_f)`; at a tracking residual of 10–100 Hz that is
  **5–50 ms** — the MegaMIMO regime.
- **Allan structure.** Crystal frequency stability follows
  `σ²_y(τ) = q₁²τ⁻¹ + q₂²τ` style models (white FM + random walk/drift); the
  cross-node phase-coherence budget is CFO + SFO + **CPO (PLL initial phase)** +
  PDD (packet-detection delay), and only CFO/SFO are removable by per-frame
  arithmetic.

### HYPOTHESIS

- On a real Aquila fleet the **relative** CFO time series is dominated by
  **temperature**, not by the initial ±10 ppm, and can wander by tens of kHz over a
  room-heating cycle. The gist's proposed bench test (log per-packet CFO for ≥30 min
  with a temperature step) would settle this; no ESP32-specific CFO-drift curve was
  found in the literature.

### SPECULATION

- A thermal mass / oven or a shared 40 MHz clock reduces drift but not the
  **per-boot CPO**; only a phase reference (wired or OTA common-TX) fixes CPO.

---

## (d) Update rate vs holdover

### ESTABLISHED

- **Holdover drift is linear in elapsed time.** ±10 ppm ⇒ **10 µs of timing per
  second**, i.e. 36 ms/hour; the phase equivalent is 24 kHz·2π rad/s. Without
  updates, coherence is lost in microseconds and timing in milliseconds.
- **There is an interior optimum in the update interval.** The DFPC/KF-DFPC
  analysis (Rashid & Nanzer, arXiv 2201.08931) states it explicitly: increasing the
  observation window T **reduces** frequency/phase estimation error but
  **increases** the drift error. For a 1 GHz array with `f_s = 10 MHz`, both DFPC
  and KF-DFPC reach minimum total phase error at **T ≈ 20 ms**; above it drift
  dominates, below it estimation noise dominates. KF-DFPC then beats plain consensus
  for **T ≤ 20 ms**.
- **Consensus needs many iterations, so its effective update rate is low.** DFPC
  "takes a large number of iterations to converge for moderately connected arrays
  with fewer nodes," raising energy and delay. This is the cost side of the
  update-rate trade.
- **Continuous-tracking systems pay the update cost every packet.** AirSync,
  MegaMIMO and the one-bit feedback loop refresh **at frame rate**; MegaMIMO notes
  a single channel measurement is amortised over the channel coherence time
  (**several hundred ms indoors**) *only if* the phase re-track happens per joint
  transmission.

### HYPOTHESIS

- For Aquila, a sensible default is **CFO estimation every frame** (cheap, from the
  HT-LTF pair) plus a **slower consensus/PLL update** (tens of ms) for the
  *frequency* estimate; holding carrier phase across a polyhedron cluster would need
  a **shared clock+phase reference**, not a faster loop.
- The optimum update interval scales with the oscillator: with a TCXO it moves out;
  with the stock crystal it is the ~20 ms order.

### SPECULATION

- A **two-timescale** estimator (fast per-frame CFO, slow skew-consensus) could
  approach the KF-DFPC residual without per-node Kalman state, if the ESP32 can
  afford the arithmetic.

---

## The round-trip scheme (ping, unique reply, timed)

The brief's scheme — one node pings, every other node replies with a **unique**
packet, round trips timed — is a **two-way timestamp exchange**. What it can and
cannot do, in this note's terms:

- **Offset + skew (time):** yes. Each reply yields `(T_M, T_S)` tuples; the
  OpenWiFiSync/RBIS estimator gives `θ̂ = T_S − T_M` and skew
  `γ̂ = (θ̂(k)−θ̂(k−1)) / (T_M(k)−T_M(k−1))`. This is **the** consensus input.
  ESP32 demonstration: ±30 µs; σ ≈ 5.35 µs.
- **Range:** yes, coarsely — RTT/2, but **processing-delay asymmetry** biases it
  (OpenWiFiSync saw a consistent **+4 µs** shift, i.e. ~1.2 km of apparent range if
  read naively). FTM is the better ranger (see 15/17).
- **Doppler:** only the **differential** part (relative frequency drift), and only
  on the timescale over which CFO is *not* removed.
- **Identity:** yes, trivially (unique packets = unique MAC/payload).
- **Channel:** yes, per reply (CSI), but cross-node phase still wraps at ~5–10 µs
  unless referenced.
- **Where it breaks:** (1) **asymmetric processing delays** break one-way ranging;
  (2) **1 µs TSF tick** caps offset precision; (3) the **~5–10 µs raw phase wrap**
  makes any phase-based use of the round trip require per-frame CFO removal; (4)
  multi-node fan-in needs the replies to be **distinguishable** (unique packets) and
  **non-colliding** (TDM/CDM — see 09).

---

## ESP32-S3 feasibility

| Capability | Verdict | Evidence |
|---|---|---|
| **Consensus time sync** (offset/skew, µs-class) | **Feasible (HYPOTHESIS)**; the pieces are est. | TSF 1 µs tick; OpenWiFiSync ±30 µs on ESP32-S3; ATS/MACTS/RFA algorithms free to port. No ESP32 consensus paper found. |
| **Per-frame CFO estimation/removal** | **Feasible (ESTABLISHED pattern)** | HT-LTF 4 µs baseline (Tsinghua CSI sanitisation tutorial); OpenCSI fits/removes the linear phase trend on ESP32 mesh. |
| **Continuous CFO tracking** (EKF/Kalman) | **Feasible in principle (HYPOTHESIS)** | Quitin/AirSync on SDR/FPGA; ESP32 has the compute; needs a reference tone or feedback path. |
| **Consensus *frequency* lock** | **Partial (SPECULATION)** | Reduces differential CFO, cannot fix per-boot PLL phase. |
| **Carrier phase coherence across independent LOs** | **Not wirelessly on stock S3 (ESTABLISHED negative)**; **yes** only with a **common reference** | ESPARGOS (wired 40 MHz + Wi-Fi phase reference); esp-crab (two C5 on a common crystal); MegaMIMO/AirSync on SDR/FPGA. |

**Demonstrated on ESP32-S3 specifically:** µs-class *time* sync (OpenWiFiSync);
per-frame CFO sanitisation (OpenCSI, CSI-ratio-style); the *negative* result that a
shared clock alone does not give phase coherence (ESPARGOS).
**Demonstrated only on SDR/FPGA/COTS-NIC:** PLL master–slave locking, AirSync,
MegaMIMO, one-bit feedback beamforming.
**Gap:** coherent phase on ESP32-class silicon requires a **wired phase reference**
(ESPARGOS) or a **common crystal** (esp-crab) — SDRs get it with FPGA-timed digital
front ends and continuous re-tracking.

---

## TX needed?

- **Consensus / time sync:** **yes** — it is a two-way exchange (or passive beacon
  sniffing of an AP, as OpenWiFiSync does). Either Aquila's `esp_wifi_80211_tx`
  frames or an external AP's beacons.
- **CFO estimation:** **no extra TX** if a reference packet is already arriving;
  a per-frame HT-LTF pair inside one TX PPDU suffices.
- **PLL/injection locking:** **yes** — a reference tone or feedback path is
  mandatory (master carrier, common TX, or one-bit SNR feedback).
- **One-bit feedback beamforming:** **yes** — someone must broadcast the bit.

---

## Evidence strength

**7 / 10.**

Justification: the consensus and PLL theory is **strong and primary** (Automatica,
SenSys, SIGCOMM, MobiHoc, TWC, MTT, arXiv), and the ESP32 drift and time-sync facts
are **measured** (Espressif datasheet/guidelines; OpenWiFiSync on S3; ESPARGOS on
S2/S3). The **reconciliation** (raw `1/(2Δf)` wrap vs post-removal residual) is
backed by independent numbers (802.11 ppm tolerance; MegaMIMO's 100 Hz→π/20 ms) and
reproduces the quoted 5–10 µs exactly. It is **not 9–10** because (i) **no carrier
phase coherence has ever been shown on ESP32-S3 without a wired reference**, (ii) the
specific ESP32 CFO-drift-vs-temperature curve is **unmeasured**, and (iii) the
distributed-array consensus results are **simulation**, not ESP32 hardware.

---

## Primary sources

1. Schenato, L. & Fiorentin, F. *Average TimeSynch: a consensus-based protocol for
   clock synchronization in wireless sensor networks.* **Automatica** 47(9), 2011.
   (Earlier: *A distributed consensus protocol for clock synchronization in wireless
   sensor network*, CDC 2007.) https://doi.org/10.1016/j.automatica.2011.06.007
   (PDF: https://www.research.unipd.it/retrieve/e14fb270-2994-3de1-e053-1705fe0ac030/CDC07_timeSynch_v1.pdf)
2. Xiong, G. & Kishore, S. *Second-order distributed consensus time synchronization
   (SO-DCTS)*, 2008. https://ieeexplore.ieee.org/document/4697839/
3. Shi, F. et al. *Fast Convergence Time Synchronization in WSNs Based on Average
   Consensus (MACTS)*. **IEEE TII** 16(2), 2020. (Overview:
   https://arxiv.org/html/2208.00216v1)
4. Werner-Allen, G., Tewari, G., Patel, A., Welsh, M., Nagpal, R. *Firefly-inspired
   sensor network synchronicity with realistic radio effects (RFA).* **SenSys 2005**,
   142–153. https://doi.org/10.1145/1098918.1098934
5. Mudumbai, R., Barriac, G., Madhow, U. *On the feasibility of distributed
   beamforming in wireless networks* (master–slave PLL; overhead↔gain tradeoff;
   oscillator phase-noise model; CRB). IEEE TWC, 2007. (PDF:
   https://web.ece.ucsb.edu/wcsl/Publications/mudumbai_j1_revised.pdf)
6. Mudumbai, R., Brown, D.R., Madhow, U., Poor, H.V. *Distributed transmit
   beamforming using feedback control* (one-bit feedback; linear-in-N convergence).
   arXiv:cs/0603072. https://arxiv.org/pdf/cs/0603072
7. Quitin, F., Rahman, M.M.U., Mudumbai, R., Madhow, U. *A scalable architecture for
   distributed transmit beamforming with commodity radios* (EKF on wrapped phase;
   Allan-variance process noise). **IEEE TWC** 12(3), 2013.
   https://wcsl.ece.ucsb.edu/sites/default/files/publications/quitin_TWC_draft1.pdf
8. Balan, H.V., Rogalin, R., Michaloliakos, A., Psounis, K., Caire, G. *AirSync:
   Enabling Distributed Multiuser MIMO with Full Spatial Multiplexing.* MobiHoc 2012 /
   IEEE/ACM ToN. arXiv:1205.6862. https://arxiv.org/abs/1205.6862
9. Rahul, H.S., Kumar, S., Katabi, D. *MegaMIMO: Scaling Wireless Capacity with User
   Demands.* **SIGCOMM 2012** (100 Hz → π in 20 ms; per-packet re-measurement).
   https://cs.nyu.edu/~anirudh/CSCI-GA.2620-001/papers/megamimo.pdf
10. Rashid, M. & Nanzer, J.A. *Frequency and Phase Synchronization in Distributed
    Antenna Arrays Based on Consensus Averaging and Kalman Filtering (DFPC / KF-DFPC;
    T≈20 ms optimum).* arXiv:2201.08931. https://arxiv.org/html/2201.08931v2
11. *Comparison of Injection-Locked and Coupled Oscillator Arrays for Beamforming.*
    **IEEE TMTT** 63(4), 2015. https://doi.org/10.1109/TMTT.2015.2401559
12. Gundall, M., Herbst, J., Müller, R., Schotten, H.D. *Benchmarking OpenWiFiSync on
    ESP32* (±30 µs; 3.96 ppm skew; 10 ppm crystal). arXiv:2511.14457; and
    *OpenWiFiSync* arXiv:2410.08742. https://arxiv.org/pdf/2511.14457 |
    https://arxiv.org/pdf/2410.08742
13. Euchner, F. et al. *ESPARGOS: An Ultra Low-Cost, Realtime-Capable Multi-Antenna
    WiFi Channel Sounder* (per-boot PLL phase; shared clock insufficient). arXiv:2502.09405.
    https://arxiv.org/html/2502.09405v1 ; datasets arXiv:2408.16377.
14. Espressif. *ESP32-S3 Series Datasheet* v2.2, §4.1.3.3 Clock, §6 RF.
    https://espressif.com/sites/default/files/documentation/esp32-s3_datasheet_en.pdf ;
    *ESP32-S3 Hardware Design Guidelines* (±10 ppm crystal).
    https://docs.espressif.com/projects/esp-hardware-design-guidelines/en/latest/esp32s3/
15. Espressif. *esp-crab / common-crystal multi-antenna CSI* — two ESP32-C5 on a
    common crystal to "eliminate the relative frequency offset."
    https://docs.espressif.com/projects/esp-techpedia/en/latest/esp-friends/solution-introduction/esp-csi/esp-csi-solution.html
16. *CSI Sanitization* (CFO from two HT-LTFs 4 µs apart; RCO/PDD/SFO). Hands-on
    Wireless Sensing with Wi-Fi tutorial, Tsinghua.
    https://tns.thss.tsinghua.edu.cn/wst/docs/sanitization
17. *OpenCSI: Self-Calibration Layer for Heterogeneous Mesh WSNs* (ESP32 mesh;
    per-boot phase; linear-phase-trend removal). arXiv:2607.26665.
    https://arxiv.org/html/2607.26665v1
18. Tektronix. *Wi-Fi: IEEE 802.11 PHY and Transmitter Measurements* (centre-frequency
    tolerance ±25 ppm @2.4 GHz, ±20 ppm @5 GHz).
    https://www.tek.com/en/documents/primer/wi-fi-overview-80211-physical-layer-and-transmitter-measurements

---

## Residual unknowns

- **No measured ESP32-S3 relative-CFO-vs-temperature curve.** The recalibration /
  tracking rate that Aquila actually needs is therefore an estimate, not a fact.
- **No published ESP32 consensus time-sync implementation.** Convergence in
  *beacon intervals* over a real 20-node polyhedron is unmeasured.
- **Whether the DFPC T≈20 ms optimum survives on a crystal (not OCXO) array.** The
  paper uses 1 GHz/generic drift statistics; the ESP32 number may be smaller.
- **Per-frame CFO-removal residual on ESP32 specifically.** OpenCSI removes a linear
  trend but does not publish the residual in Hz.
- **Whether one-bit feedback can converge inside the ESP32 channel/drift coherence**
  given the 20 Hz-ish practical CSI rate.
- **The 802.11 S3-specific carrier CFO/drift spec is not in the datasheet** (only BLE
  CFO/drift, ~0.8–2.5 kHz, is tabulated); the 2.4 GHz ±25 ppm envelope is the
  standard's, not Espressif's measured value.

---

## Chaining

- **Compliant TX (capability 3) enables two-way timestamp exchange → consensus /
  RBIS time sync (µs-class).** This is the enabling edge for **TDM scheduling** and
  **coarse cross-node alignment**.
- **Consensus time sync does NOT enable carrier phase coherence.** It enables
  *when-sampled* agreement, not *phase-at-sample*. This is the boundary that
  `20-theoretical-boundary` formalises.
- **A common reference (wired clock+phase, or OTA common-TX with a PLL) enables
  coherent phase → AoA, absolute ToF, distributed coherent aperture.** Highest
  leverage: it unlocks a **further** capability (coherent aperture / 3D DoA from the
  polyhedron) that no amount of consensus/software alone reaches.
- **Per-frame CFO sanitisation enables** usable *differential* phase for sensing
  (respiration, motion, Doppler) **without** resolving the crystal question.
- **FTM (S3)** provides a coarse range and a per-peer ppm estimate that **seeds**
  the consensus/servo loop.
- **Blocked without a reference:** cross-node absolute phase, per-node AoA,
  absolute ToF — regardless of how good the consensus clock is.

**Highest-leverage enabling capability:** a **common reference** (shared clock
*plus* phase reference, or an OTA common transmitter). It is the only thing that
crosses from "µs sync" to "ps phase lock." **Capability that unlocks a further one:**
a shared clock+phase reference unlocks the **soldered-polyhedron coherent aperture**
(3D DoA / distributed beamforming) that is otherwise impossible on independent-LO
ESP32s.

---

## Existing models and data

- **Algorithms (reusable ideas):** ATS, MACTS, SO-DCTS, WCCS (consensus time sync);
  RFA/PCO (firefly); DFPC / KF-DFPC (consensus + Kalman for distributed arrays);
  EKF-on-wrapped-phase (Quitin); Kalman phase tracking (AirSync); one-bit feedback
  (Mudumbai). Mostly concept-level; MATLAB for DFPC.
- **Open-source implementations:** **OpenWiFiSync** (GPLv3; ESP32 RBIS time sync) —
  directly usable; **esp-csi** (Apache-2.0; CSI + `rx_ctrl` CFO fields);
  **esp-crab** (vendor reference for a common crystal, hardware files incomplete —
  see esp32.com thread); **pyespargos / ESPARGOS** (open HW+SW, wired phase
  reference) — the reference architecture for *coherent* arrays.
- **Datasets:** **ESPARGOS phase-coherent CSI datasets** (`espargos-0002`, 4×8
  array, robot-tracked) — phase-coherent but on *wired* combined arrays, not a
  wireless mesh.
- **Can Aquila use them?** OpenWiFiSync and esp-csi: **yes** (licence-compatible,
  same silicon). esp-crab hardware: **partially** (design files missing). DFPC /
  KF-DFPC: **yes** (reimplement on device). MegaMIMO/AirSync: **no** (SDR/FPGA
  timing). ESPARGOS: **as a lab reference only** (wired).

---

## Own-data / new technology

**Minimal decisive experiment (falsifiable):**

> **Relative-CFO stability on the Aquila fleet.** Two (then N) stock ESP32-S3 nodes
> receive the same beacon stream from one TX and log per-packet CFO
> (`rx_ctrl` fields, and/or the HT-LTF phase slope) for **≥30 min including a
> deliberate temperature step** (heatsink/hairdryer). Plot the relative-CFO time
> series and an Allan-deviation-style curve; report the time for which a frequency
> estimate stays within **10 Hz**.

**What it decides:** the recalibration/tracking **rate** for every calibration
method, and whether "calibrate once" is ever viable. **Falsifies** the
per-packet-sanitisation-is-mandatory architecture if relative CFO is stable to
<10 Hz over >1 s; **confirms** it if CFO re-randomises per packet (as MegaMIMO and
ESPARGOS predict). It is cheap, needs no new hardware, and directly measures the
one number the literature lacks for ESP32-S3.

**Second experiment (if pursuing coherent phase):** add a **conducted common
reference** (shared 40 MHz + injected Wi-Fi phase-reference tone, ESPARGOS-style)
across the polyhedron and measure residual cross-node phase error vs update rate —
the minimal test of whether Aquila's own hardware can reach ps-class coherence.

**What would prove a genuinely new result:** **software-only cross-node phase
coherence on independent-LO ESP32-S3** (no wired reference). No published system has
done this; the evidence (ESPARGOS, MegaMIMO, AirSync) says it fails. Demonstrating
it would require either exposing the S3 PLL/VCO for injection locking or a
fundamentally new OTA phase-transfer scheme robust to the per-boot CPO.
