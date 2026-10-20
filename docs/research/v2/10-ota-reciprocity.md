# 10 — OTA reciprocity self-calibration

**Prompt.** When does over-the-air reciprocity (`Ĥ_AB·Ĥ_BA⁻¹`) cancel hardware and
yield a usable shared reference, and what residual defeats it on ESP32? Must
contain: (a) the identity and what cancels; (b) residual sources — RF-chain
asymmetry, LO drift, reciprocal mixing; (c) cadence vs channel coherence; (d)
published residual magnitudes.

Overlap excluded: 11 (common-transmitter / two-way), 12 (consensus / oscillator
locking). This note keeps to reciprocity itself; where it touches the
common-reference escapes it does so only to distinguish them.

---

## Question

The BRIEF's second published escape from independent-LO phase unobservability is
**over-the-air (OTA) reciprocity**: the claim that `Ĥ_AB·Ĥ_BA⁻¹` cancels the
propagation channel and leaves a channel-independent quantity that restores a
shared reference across radios that do not share a clock. This note asks what
exactly cancels, what remains, whether the remaining term is usable on an
**ESP32-S3** node, and at what rate it must be refreshed.

It is crucial to separate two regimes the literature blurs together:

- **Intra-node OTA calibration** — antennas/RF chains *within one radio (or one
  node) that share a single local oscillator*. Here OTA reciprocity is
  well-established and demonstrated.
- **Inter-node OTA calibration** — *geographically separated nodes with
  independent LOs*. Here the LO phase is a time-varying state variable that
  reciprocity calibration either estimates (and must track) or does not remove.

Aquila's multi-node, independent-LO setting is squarely the second regime.

---

## Findings

### ESTABLISHED

**1. The identity and what cancels.**
Following Rogalin et al. (2014) and Jiang/Kaltenberger et al. (2018), the
composite channel an OFDM receiver *observes* is the air channel pre- and
post-multiplied by the RF chains:

```
H_{A→B} = R_B · C · T_A          H_{B→A} = R_A · Cᵀ · T_B
```

where `C` is the reciprocal physical propagation channel, `T` the transmit-chain
response (DAC → PA → antenna) and `R` the receive-chain response (antenna → LNA →
ADC). `H` is what appears in the CSI; `C` is what is reciprocal. For a
single-antenna pair (scalar reciprocal `c`):

```
Ĥ_AB · Ĥ_BA⁻¹ = (r_B c t_A) / (r_A c t_B) = (t_A/r_A) / (t_B/r_B) = f_A / f_B ,
```

defining the *per-radio calibration coefficient* `f_i = t_i / r_i`. **`c` cancels
identically.** What remains is the ratio of Tx/Rx hardware asymmetry between the
two radios — and that ratio is precisely the thing being estimated. The OTA
exchange thus does not "cancel the hardware"; it *exposes* the hardware so a
coefficient `F = diag(f_i)` can be inverted, after which
`Ĥ_DL ≈ F · Ĥ_UL`. Only the *relative* calibration is identifiable: the estimator
is invariant to a common complex scalar (the Fisher information matrix is
structurally singular; a constraint such as `f_1 = 1` must be imposed — Jiang et
al. 2018, §VI-A). That residual scalar is harmless for beamforming because it
vanishes in the beam pattern.

*Generalisation:* for MIMO with a full channel matrix, `C` is eliminated only
because the *same* physical channel is measured in both directions within the
coherence interval (`C_ij = C_jiᵀ`), not because ratios of arbitrary matrices
cancel. The whole method therefore rests on **bidirectional measurements taken
close enough in time** that `C` (and the LO phase, below) is effectively frozen.

**2. The dominant time-varying residual is LO phase, not the fixed hardware.**
The fixed Tx/Rx asymmetry (`f_i`) is stable over *hours* (see magnitude (4)).
But the transmitter LO injects `+φ(t)` and the receiver LO `−φ(t)` (Nissel 2022),
so `Ĥ_AB` and `Ĥ_BA` are each corrupted by the instantaneous LO phases of the
two radios at their own measurement instants. Unless the two directions are
measured within the LO-phase coherence interval, the difference of their phases
carries a residual `φ_A(t₁) − φ_B(t₂)` that is **not** removed by the calibration
coefficient. In a distributed system with independent LOs, Rogalin et al. state
explicitly that residual CFO causes slow relative drift of the AP phasors and
that "calibration must be performed at the same rate as synchronization" — i.e.
every frame, or every few frames, not every 10 minutes. This is the crux: OTA
reciprocity gives a clean *static* hardware calibration, but the *dynamic* LO
term must be re-estimated at the oscillator-drift rate. It is not a self-contained
answer to independent-LO phase.

**3. Cadence is set by two very different timescales.**
- *Hardware coefficients `f_i`:* centrally clocked arrays need only one calibration
  round "every 10 min" (Rogalin et al.); measured drift is small over 4 h (below).
- *Residual LO phase:* frequency errors fluctuate with temperature with a
  coherence time "on the order of 1 s" (Rogalin et al.); 802.11ac system
  simulations assume an 800 ms channel coherence time, and WARP experiments
  measured ~1 s. Distributed-MIMO work (arXiv 2509.03722) finds "frequent phase
  re-estimation is critical … when the LOs exhibit rapid phase drift" and applies
  a Kalman filter to track the phase between re-estimations. A ping/reply pair
  must also fall *inside* the channel coherence time for `C` itself to cancel.
- Net: **inter-node phase must be refreshed on a sub-second-to-few-second cadence;
  intra-node hardware calibration on a many-minute-to-hourly cadence.** Conflating
  the two is the most common error.

**4. Published residual magnitudes (all non-ESP32; SDR or commercial testbeds).**
- *Jiang et al., experimental* (cited in arXiv 1708.07811): calibration
  coefficients vary from their mean by < **2.6% in phase (max 6.7%)** and
  < **0.7% in amplitude (max 1.4%)** over **4 hours** — evidence that the static
  part is genuinely stable and rarely needs recalibration.
- *Argos, 64-antenna prototype* (Shepard et al. 2012): OTA relative calibration
  with a reference antenna; reciprocity-based UL/DL achieved EVM **2.51%
  (−32.0 dB) / 4.23% (−27.5 dB)** for 2 users and **2.89% (−30.8 dB) / 6.09%
  (−24.3 dB)** for 4 users. This is the canonical demonstration that OTA
  reciprocity "works" — on a *centrally clocked* array.
- *Per-chain asymmetry,* measured (arXiv 393642973, Amp-Phase Cal): chain
  responses deviating from nominal by **11.1%, 5.7%, 13.6%, 1.6%** in amplitude;
  inter-AP amplitude mismatch **~18%**; full amplitude+phase calibration lands
  within **3.5%** of the perfect-calibration sum rate, and beats phase-only cal by
  ~14–23%.
- *Required calibration accuracy* (arXiv "How accurately should we calibrate…"):
  for ≤3 dB SINR loss at DL SNR 20 dB, conjugate beamforming tolerates
  `MSE_Fd < 0.07` while **zero-forcing needs `MSE_Fd < 10⁻³`** — i.e. ZF imposes
  ~100× tighter calibration. Usable regimes sit at `NMSE_F ≈ 10⁻⁵…10⁻³`.
- *−32 dB EVM* corresponds to roughly **1–2° RMS residual phase error** — this is
  the order of magnitude any ESP32 implementation would have to beat to matter.

**5. Calibration accuracy is SNR- and topology-limited.**
The calibration estimate is bounded below by (i) measurement SNR, (ii) variation
of `F` between the calibration and beamforming phases, and (iii) network topology.
Jiang et al. (2018) give the constrained Cramér–Rao bound; LS estimation in a
daisy-chain/line topology is ill-conditioned and error propagates — some graph
topologies give *unbounded* phase-estimation variance as node count grows
(arXiv 2509.03722). At low SNR the LS estimator is biased with MSE saturating at
`≤ 4|f|²`. Argos-style methods require careful placement of a reference antenna
with high SNR to every other antenna.

**6. Reciprocal mixing and RF impairments are real but largely RX-side.**
Reciprocal mixing — LO phase noise convolved with in-band signal/blockers — raises
the receiver noise floor and thus the calibration measurement noise; it is an
RX-chain effect and does not itself appear in the reciprocal `C`. The named RF
impairments in this literature are I/Q imbalance, oscillator phase noise, and HPA
non-linearity (Mohammadian 2021; the EU H2020 OAI thesis). Nonlinear HPA gain
makes `f` *power-dependent* ("nonlinear reciprocity mismatch", arXiv 2009.14481),
so a linear calibration is insufficient at low output back-off (IBO); practical
systems keep IBO high. I/Q imbalance is also the documented root of *non-linear
CSI phase errors* on commodity Wi-Fi (Zhang et al., "Perceiving accurate CSI
phases"), and is stable over time but frequency-band-sensitive — i.e. correctable
per-band.

**7. ESP32's leading phase-coherence solution is a *conducted* reference, not OTA
reciprocity.**
This is the decisive hardware-honesty finding. ESPARGOS (arXiv 2502.09405), the
best-known ESP32 phase-coherent platform, does **not** use OTA reciprocity for
cross-receiver phase. It uses (i) a shared 40 MHz reference clock, and (ii) a
**WiFi-based phase-reference signal distributed over a resistive-divider +
microstrip network** to a second RF port on each board (RF switch selects antenna
or reference). Calibrated CSI is `ĥ_CAL,m = ĥ_OTA,m · ĥ_REF,m · e^{−jφ_path,m}`;
the reference channel supplies the PLL/analog phase offsets that frequency lock
alone cannot remove ("Reference clock distribution is insufficient for achieving
phase coherence", because the PLL introduces phase uncertainty). Espressif's own
`esp-crab` uses the same idea at chip scale: **two ESP32 chips sharing one crystal
via a clock buffer** to eliminate relative frequency offset. In other words, on
ESP32 the **shared/conducted-reference escape is the proven one; OTA reciprocity
is the *unproven* one.** No primary source demonstrates OTA reciprocity
calibration between independently-clocked ESP32-S3 nodes.

### HYPOTHESIS

**8. ESP32-S3 can exploit OTA reciprocity for *amplitude* and slow *relative*
phase, but not for stable *absolute* cross-node phase without a cadence that
likely exceeds what the MAC can sustain.**
The evidence for this is indirect but convergent:
- Commodity CSI phase is "randomly corrupted" and "unusable in practice" without
  processing, because of CFO, SFO and a per-frame random phase rotation from the
  independent LOs (survey, Methods; B2B Calibration paper, PMC6263436). This is
  exactly the `φ(t)` term above.
- Yet ESP32 key-generation work reports "highly reciprocal CSI" and a key
  generation rate of ~**1788 bits/s** by using the IEEE 802.11 **ACK/response
  mechanism** to *bound the uplink/downlink time gap* (Zhang et al. 2023). That is
  reciprocity exploited over a *very short* bidirectional gap. It is strong
  evidence that the physical channel is reciprocal over the required interval, and
  that the ESP32 can measure a *correlated* bidirectional quantity — but it is
  quantised amplitude/key material, not a calibrated absolute phase.
- Therefore: OTA reciprocity on S3 plausibly yields a usable *relative* phase that
  decays on the LO-drift timescale, needing periodic re-estimation; whether the
  residual is small enough to be a *reference* (rather than a thing to be
  continuously re-measured) is untested.

**9. A tight ping/reply exchange is the enabling primitive, and the radio
turnaround time is its dominant error.**
ESP32's bounding of the UL/DL gap via the response mechanism is the same structure
required for OTA reciprocity: a stimulus whose reply is sent promptly. For phase,
the requirement is that `Ĥ_AB` and `Ĥ_BA` be captured within the LO-phase
coherence interval.

### SPECULATION

**10. On ESP32-S3 specifically, OTA reciprocity is more likely to serve as a
*periodic re-anchor* for a mostly-conducted or common-transmitter system than as a
standalone wireless-only escape.** A hybrid — conducted/shared clock for coarse
frequency coherence (as ESPARGOS and esp-crab do) plus occasional OTA reciprocity
to correct residual per-node phase — is likely the only route to stable
cross-node phase on S3 without cabling, but no source demonstrates this on ESP32.

**11. The soldered-PCB polyhedron aperture may help OTA reciprocity by geometry,
not by signal processing.** With many nodes mutually visible at high SNR and rich
connectedness, the calibration graph is well-conditioned and the CRB floor drops;
the volumetric array also makes the inter-node channel strongly multipath, which
*reduces* the coherence time, cutting the other way. Unknown.

---

## ESP32-S3 feasibility

| Aspect | Verdict | Basis |
|---|---|---|
| Bidirectional CSI capture (the raw input) | **Yes** | `esp_wifi_set_csi_rx_cb`, ESP32-CSI-Tool, esp-csi; ESP32-S3 CSI quality ranks ≈ ESP32-C3, below C5/C6 (Espressif techpedia). |
| Low-latency reciprocal exchange (bound the UL/DL gap) | **Demonstrated for amplitude** | Zhang et al. 2023 ESP32 response-mechanism key gen, KGR ≈ 1788 bits/s. |
| Absolute cross-node phase via OTA reciprocity | **Not demonstrated on ESP32** | No primary source; leading ESP32 platform (ESPARGOS) uses a conducted reference instead. |
| Intra-node / same-LO OTA calibration | **Plausible, undemonstrated on S3** | Method is established on SDR/commercial arrays (Argos); S3 has a single RF port per radio. |
| Sub-second phase re-estimation cadence | **Questionable under stock MAC** | Requires TX/RX turnaround + host feedback within ~1 s; half-duplex TDM; not characterised on S3. |
| Holding residual below the ~1–2° (EVM −32 dB) band useful for AoA/beamforming | **Unproven** | ESP32 CSI phase is documented as corrupted by CFO/SFO/random rotation. |

**Bottom line:** OTA reciprocity is physically sound and well-characterised on
SDR/commercial hardware. On ESP32-S3 it is an attractive but **unvalidated**
escape; the only ESP32 phase-coherence routes with published success are
shared-crystal / conducted-reference (esp-crab, ESPARGOS). Any claim that
Aquila's S3 nodes can share a phase reference "for free" via OTA reciprocity
should be treated as a hypothesis to be measured, not a capability.

## TX needed?

**Yes — unavoidably.** OTA reciprocity is intrinsically bidirectional: each node
must both transmit and receive to obtain `Ĥ_AB` *and* `Ĥ_BA`. This distinguishes
it from:
- the **receive-only** passive CSI mode, which cannot perform it at all;
- the **conducted/shared-reference** escape, which needs no TX (esp-crab,
  ESPARGOS reference network);
- the raw-I/Q receive mode, also receive-only.

Concretely it requires compliant frame TX (`esp_wifi_80211_tx`, prompt 07) plus
CSI capture on the reverse path, and a scheduling discipline that keeps the two
directions inside one coherence interval (TDM; prompt 09). So OTA reciprocity
*chains on top of* the TX primitive and is a downstream consumer of the TDM
scheduling decision.

---

## Evidence strength

**6 / 10.**

*Justification.* The identity, the residual sources and their magnitudes, and the
cadence/bias trade-offs are **strongly established (8–9/10)** across many
independent primary sources in the distributed-MIMO/TDD literature (Argos,
Kaltenberger, Rogalin, Jiang, Nissel, and 2024–2025 follow-ups). But the question
is posed for **ESP32-S3**, and there the evidence is **weak (2–3/10)**: no primary
source demonstrates OTA reciprocity calibration on ESP32 of any variant, and the
best ESP32 phase-coherent platform deliberately avoids it in favour of a conducted
reference. The composite of a well-understood theory with an absent device proof
lands at **6**: high confidence in *what* the method does and *what defeats it*,
low confidence that S3 can realise it to a useful residual. The most likely
failure is not the physics of `c` cancelling but the inability of the stock MAC to
refresh the LO-phase term fast enough.

---

## Primary sources

- C. Shepard, H. Yu, N. Anand, E. Li, T. Marzetta, R. Yang, L. Zhong,
  *"Argos: Practical Many-Antenna Base Stations"*, MobiCom 2012.
  https://doi.org/10.1145/2348543.2348553 — first working OTA relative
  calibration; 64 antennas; EVM −32/−27.5 dB; reference-antenna placement
  sensitivity.
- F. Kaltenberger, H. Jiang, M. Guillaud, R. Knopp, *"Relative channel
  reciprocity calibration in MIMO/TDD systems"*, Future Network & Mobile Summit
  2010. https://ieeexplore.ieee.org/document/5722383 — origin of relative
  calibration.
- R. Rogalin, O. Y. Bursalioglu, H. Papadopoulos, G. Caire, A. F. Molisch,
  A. Michaloliakos, V. Balan, K. Psounis, *"Scalable Synchronization and
  Reciprocity Calibration for Distributed Multiuser MIMO"*, arXiv:1310.7001
  (IEEE TWC 2014). https://arxiv.org/abs/1310.7001 — distributed calibration;
  calibration cadence = synchronization cadence; coherence ~1 s.
- X. Jiang, M. Čirkić, F. Kaltenberger, G. Larsson, L. Deneire, R. Knopp,
  *"MIMO-TDD reciprocity under hardware imbalances: Experimental results"*, ICC
  2015. https://ieeexplore.ieee.org/document/7249107 — measured 2.6%/0.7% drift
  over 4 h.
- X. Jiang, K. Gopala, F. Kaltenberger, D. Slock, *"A Framework for Over-the-air
  Reciprocity Calibration for TDD Massive MIMO"*, arXiv:1710.10830 (IEEE TWC
  2018). https://arxiv.org/abs/1710.10830 — general framework, CRB, coherent vs
  non-coherent accumulation, singular-FIM / scale ambiguity, low-SNR bias.
- X. Jiang, F. Kaltenberger et al., *"Channel reciprocity calibration in TDD
  hybrid beamforming massive MIMO systems"*, arXiv:1708.07811 (2017).
  https://arxiv.org/abs/1708.07811 — `F_BS`, `F_UE` calibration matrices; UE-side
  phase error immaterial; diagonal-`F` approximation.
- *"How accurately should we calibrate a Massive MIMO TDD system?"* (EURECOM,
  Kaltenberger group), https://www.eurecom.fr/en/publication/4862 — accuracy
  targets: `MSE_Fd < 0.07` (conjugate) vs `10⁻³` (ZF) for 3 dB SINR loss.
- *"Real-time Performance Evaluation of TDD Reciprocity Calibration for
  Distributed Antenna Systems"* (EURECOM thesis, 2021),
  https://www.eurecom.fr/publication/6405 — OAI testbed; RF impairment taxonomy
  (I/Q imbalance, phase noise, HPA); topology conditioning.
- *"Impact and Calibration of Nonlinear Reciprocity Mismatch in Massive MIMO"*,
  arXiv:2009.14481 (IEEE TWC 2020). https://arxiv.org/abs/2009.14481 — power-
  dependent mismatch; linear calibration insufficient at low IBO.
- *"Distributed MIMO With Over-the-Air Phase Calibration Integrated Into the TDD
  Flow"*, arXiv:2509.03722 (2025). https://arxiv.org/abs/2509.03722 — inter-AP
  phase calibration with independent drifting LOs; Kalman phase tracking;
  frequent re-estimation; topology-dependent error growth.
- *"Over-the-Air Amplitude and Phase Reciprocity Calibration for Distributed
  MIMO"* (2025). https://www.researchgate.net/publication/393642973 — measured
  chain-amplitude deviations 11.1/5.7/13.6/1.6%; ~18% inter-AP amplitude mismatch;
  within 3.5% of perfect.
- R. Nissel, *"Correctly Modeling TX and RX Chain in (Distributed) Massive MIMO —
  New Fundamental Insights on Coherency"*, IEEE Commun. Lett. 26(10), 2022.
  https://doi.org/10.1109/LCOMM.2022.3189968 (arXiv:2206.14752) — LO effect is
  `+φ` in TX, `−φ` in RX; LO phase is *inside* the reciprocity calibration; free-
  running LOs + assumed-perfect calibration is not useful; prefer locked LOs +
  relative calibration + DMRS.
- *"Perceiving accurate CSI phases with commodity WiFi devices"* (Zhang et al.,
  INFOCOM 2017) — I/Q imbalance as root of non-linear CSI phase error; stable in
  time, band-sensitive.
- *"Mitigation of CSI Temporal Phase Rotation with B2B Calibration Method"*,
  Sensors/PMC6263436 — per-frame random phase rotation from unsynchronised LOs;
  back-to-back (conducted) ground truth for calibration.
- K. Ruttik / ESPARGOS group, *"ESPARGOS: An Ultra Low-Cost, Realtime-Capable
  Multi-Antenna WiFi Channel Sounder"*, arXiv:2502.09405 (2025).
  https://arxiv.org/abs/2502.09405 — ESP32 phase coherence via distributed
  reference signal + shared 40 MHz clock; **not** OTA reciprocity.
- ESPARGOS datasets, arXiv:2408.16377; https://espargos.net/docs/combined-arrays
  — multi-array coherence via SYNC IN/OUT reference distribution.
- Espressif, *"Introduction to ESP-CSI Solution"* (ESP-Techpedia),
  https://docs.espressif.com/projects/esp-techpedia/en/latest/esp-friends/solution-introduction/esp-csi/esp-csi-solution.html
  — `esp-crab` common-crystal phase sync; CSI chip ranking C5 > C6 > C3 ≈ S3.
- H. Zhang et al., *"A Low-cost ESP32-driven Wireless Key Generation System Based
  on Response Mechanism"* (2023).
  https://ieeexplore.ieee.org/document/10151089 — "highly reciprocal CSI";
  KGR ≈ 1788 bits/s; IEEE 802.11 response mechanism bounds the UL/DL gap.
- Espressif, *Wi-Fi Vendor Features / CSI* (ESP-IDF),
  https://docs.espressif.com/projects/esp-idf/en/v6.0-beta2/esp32/api-guides/wifi-driver/wifi-vendor-features.html
  — `wifi_csi_info_t` (RSSI, noise floor, **microsecond RX time**, antenna).
- A. Mohammadian et al., *"RF Impairments in Wireless Transceivers: Phase Noise,
  CFO, and IQ Imbalance — A Survey"*, IEEE Access 2021.
  https://doi.org/10.1109/ACCESS.2021.3101845 — reciprocal mixing, phase-noise
  models.
- NIST, *"A review of time and frequency transfer methods"* (2008),
  https://tf.nist.gov/general/pdf/2311.pdf — two-way transfer assumes path-delay
  symmetry; TX/RX delay asymmetry is a leading error (relevant to the round-trip
  scheme below).

---

## Residual unknowns

1. **The measured OTA-reciprocity residual on ESP32-S3.** No published number for
   `|Δ phase|` after `Ĥ_AB·Ĥ_BA⁻¹` on an S3 pair, static or mobile.
2. **The LO-phase coherence interval on S3.** Datasheet specifies crystal accuracy
   (ppm) and PLL behaviour is undocumented ("internal structure of the ESP32 chip
   is not publicly known", ESPARGOS); the resulting phase-random-walk rate, hence
   the required re-calibration cadence, is unknown.
3. **Whether `esp_wifi_80211_tx` + CSI callback can measure `Ĥ_AB` and `Ĥ_BA`
   within one coherence interval in practice**, and what the achievable turnaround
   / feedback latency is.
4. **Absolute vs relative phase after calibration.** The CRB is singular; the
   identifiable quantity is relative up to a common complex scalar. Whether that
   scalar is harmless for Aquila's *sensing* (as it is for beamforming) is not
   established.
5. **Mutual coupling / crosstalk** between co-located polyhedron nodes — assumed
   diagonal in almost all OTA-calibration work; a dense soldered array may violate
   this.
6. **Whether a reference-signal/conducted hybrid is required on S3** (my
   hypothesis 10), and its cabling cost versus a purely wireless OTA scheme.
7. **The round-trip / ping-and-unique-reply scheme** (below): its accuracy budget
   on S3 for offset, range, Doppler, identity and channel is unquantified.

### The round-trip scheme (ping + unique replies, timed)

The BRIEF's scheme — one node pings, every other node replies with a **unique**
packet, round trips timed — is one-to-many **two-way time transfer**. What it can
measure, and where it breaks:

- **Time/phase offset:** differencing the timed round trips (PTP/SS-TWR style)
  cancels the common clock offset and, if the two directions are within the
  coherence interval, the propagation channel — the same cancellation as
  `Ĥ_AB·Ĥ_BA⁻¹`. Reconstructing *CSI* of ping and each unique reply extends this
  from timing to a full relative calibration.
- **Range:** ToF = round-trip/2 **only if the path delay is symmetric** (NIST).
  The dominant error is the responder's **radio turnaround** (SIFS + software
  delay between RX event and TX reply), which directly biases the round trip and
  is exactly *radio TX/RX delay asymmetry* — the same non-reciprocity OTA
  calibration exists to remove. ESP32's **microsecond RX timestamp** gives ~300 m
  round-trip resolution, far too coarse for room-scale ranging; FTM's finer
  resolution is coarse but usable (prompt 17). So range is weak; offset/relative
  phase are the real products.
- **Identity:** the unique reply disambiguates which node answered — enabling
  association, not geolocation.
- **Doppler:** successive-round CSI ratios give Doppler only for moving scatterers
  (blind to static) — as assumed.
- **Breaks when:** the round trip straddles the channel or LO coherence time; the
  turnaround is uncalibrated (bias); paths are asymmetric; the half-duplex reply
  collides in a shared channel (needs TDM); the microsecond timestamp resolution
  cannot resolve the quantity of interest; and multipath makes "the" round trip
  ill-defined.

---

## Chaining

- **X enables Y.** Compliant bidirectional TX (`esp_wifi_80211_tx`, 07) + TDM
  scheduling (09) **enable** OTA reciprocity (10). OTA reciprocity, *if its
  residual is small enough*, **enables** cross-node coherent phase, which
  **enables** distributed MIMO / coherent TX, cross-node AoA, and phase-based
  localisation (15) and channel charting (03) without cabling.
- **Y blocked without Z.** Inter-node OTA reciprocity is blocked without a
  **bidirectional, coherence-interval-bounded CSI exchange**; and it is blocked
  from being a *stable reference* without **LO phase tracking** (11/12) at the
  drift rate. It is blocked from the RX-only modes entirely (raw-I/Q mode, passive
  CSI cannot generate `Ĥ_BA`).
- **Highest-leverage enabling capability.** A **low-latency bidirectional CSI
  exchange primitive** (the same structure the ESP32 key-gen "response mechanism"
  already realises for amplitude). Everything in this prompt hangs off it; it also
  serves prompt 11's two-way transfer and the round-trip scheme.
- **Capability that unlocks a further one.** OTA reciprocity, once validated on
  S3, would unlock the *wireless-only* path to cross-node phase — removing the
  cabling that esp-crab/ESPARGOS require — and would thereby unlock the compact
  soldered-PCB polyhedron as a *coherent* volumetric array rather than merely a
  set of independent sensors.
- **Direct competitors (not dependencies):** the common-transmitter reference (11)
  and the shared/conducted reference (esp-crab, ESPARGOS). These are *alternative*
  escapes; on ESP32 the conducted one is the proven one, OTA is the gamble.

## Existing models and data

The OTA-reciprocity *methods* are public and licence-clean (journal/arXiv papers;
implementable in any language):
- **Argos** (MobiCom 2012) and **Kaltenberger relative calibration** (2010) —
  canonical algorithms, reference-antenna and bidirectional-pilot variants.
- **Jiang et al. framework + CRB** (arXiv 1710.10830) — full estimator suite,
  coherent/non-coherent accumulation, constraint choices for the scale ambiguity.
- **Rogalin et al.** (arXiv 1310.7001) — distributed topology generalisation;
  reusable for a mesh/polyhedron.
- **arXiv 2509.03722** (2025) — TDD-flow-integrated inter-AP phase calibration with
  a Kalman tracker; the most directly transferable design.

ESP32-specific **data and platforms** (usable, permissive licences):
- **ESPARGOS** — CSI datasets (arXiv 2408.16377; https://espargos.net/datasets),
  phase-coherent *via conducted reference*; open docs; the natural baseline.
- **ESP32-CSI-Tool** (MIT) and **esp-csi** (Apache) — CSI capture.
- **esp-crab** (Espressif) — common-crystal reference design.
- **CSI datasets** from Oregon State (Basha & Hamdaoui,
  https://research.engr.oregonstate.edu/hamdaoui/...) for reciprocity/key-gen.

Gap: **no public dataset pairs ESP32-S3 nodes with *independent* LOs and captures
simultaneous bidirectional CSI.** ESPARGOS gives conducted-reference coherence;
key-gen datasets give amplitude reciprocal material. Neither gives the
`Ĥ_AB`,`Ĥ_BA` phase pairs this prompt needs.

## Own data / new technology

**Could an Aquila dataset unlock something not currently demonstrated? Yes —
specifically, whether independent-LO ESP32-S3 nodes can recover a usable relative
phase over the air.**

**Minimal experiment.** Two (then N) ESP32-S3 nodes, independent LOs, on a static
indoor channel. Interleave A→B and B→A frames within one coherence interval using
the 802.11 response mechanism; capture CSI and the microsecond RX timestamps.
Compute `Ĥ_AB · Ĥ_BA⁻¹` per subcarrier over ≥ 10 minutes at ≥ 100 Hz. Measure
(a) the residual's magnitude after removing the mean, (b) its autocorrelation /
random-walk rate, and (c) the cadence at which a re-estimated calibration keeps
the residual below the ~1–2° (−32 dB EVM) band. Repeat on a second board pair with
a **conducted reference** (esp-crab-style shared crystal) as ground truth.

**What it proves.** The residual magnitude and required refresh rate of OTA
reciprocity on S3, and hence whether it is a usable escape or only a
short-coherence measurement.

**What would falsify it.** If, after calibration, the residual phase is dominated
by an unpredictable random walk at the LO-drift timescale that cannot be tracked
below the useful band by any feasible refresh cadence, then OTA reciprocity is
**not** a usable shared reference on S3 without a conducted/common reference —
settling the question in the negative and redirecting effort to prompt 11/12.

**Secondary dataset value.** A bidirectional, timestamped, multi-node S3 CSI
corpus (static + mobile) would also feed calibration-accuracy targets (CRB
validation), the round-trip scheme's error budget, and the polyhedron's mutual
coupling study — none of which exist publicly.
