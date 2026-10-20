# 20 — Theoretical boundary and minimal added reference

**Prompt.** `20-theoretical-boundary`. What is provably unobservable from
independent-LO receive-only CSI, and what minimal added reference crosses the
boundary? Must contain: (a) formal unobservability (unknown LO phase/frequency,
gauge freedom); (b) invariants preserved under that gauge; (c) minimal-reference
result and the escapes; (d) Cramér-Rao bounds on calibration precision. Leads:
no-common-clock phase-unobservability results; CRB for localisation/phase;
distributed-coherence theory. Overlap: not 10, 11, 12, 13.

Australian English. All claims labelled **ESTABLISHED / HYPOTHESIS /
SPECULATION**. Hardware tier stated per claim: **ESP32-S3 / family**,
**other COTS Wi-Fi**, or **SDR-grade**. Sources are primaries (papers,
standards, upstream code, datasheets).

---

## Question

Given a set of receivers, each down-converting with its **own** local
oscillator (LO), that passively observe 802.11 CSI from some transmitter(s),
what is *information-theoretically* impossible to observe — regardless of
algorithm, SNR or averaging — and what is the *smallest* addition (one
transmission? one shared wire?) that makes the missing quantity observable?
This is the boundary that governs which Aquila capabilities are merely
hard and which are formally barred without an added reference.

The question is deliberately abstract, but every answer below is anchored to
a primary source that derived or demonstrated the corresponding fact, and to
the tier of hardware on which it was shown.

---

## Findings

### ESTABLISHED

**1. The exact observation model, and the gauge it leaves.** For a single
transmitter `i` and receiver `j`, a narrowband per-subcarrier CSI estimate is

```
ĥ_ij(k) = g_ij(k) · a_i · b_j · e^{ j(θ_i − ψ_j) } · e^{ −j2π f_k τ_ij }
```

where `g_ij` is the propagation channel, `a_i` and `b_j` are the (complex)
transmit/receive RF-chain responses of the two radios, `θ_i` is node `i`'s
transmit LO phase and `ψ_j` node `j`'s receive LO phase, and `τ_ij` is the
delay. **Only the combinations `(θ_i − ψ_j)` and `a_i·b_j` are observable, and
they are entangled with `g_ij`; no passive receiver-side processing separates
them.** This is stated in the same form by the distributed-MIMO calibration
literature: the effective channel "including the carrier phase offset" is
`h^{(n,m)}[k] = g^{(n,m)} e^{j(φ_t^{(n)}[k] − φ_r^{(m)}[k])}` (UCSB distributed
massive-MIMO survey, `06804225.pdf`; two-state oscillator model,
`06310822.pdf`). *Tier: general RF theory; demonstrated on SDR testbeds and on
ESP32-family CSI by construction.*

**2. Formal statement of the gauge (the "global phase").** If the network's
measurement graph — nodes joined by a link on which a phase was measured — is
**connected**, then all node phases are determined only **up to one additive
common constant**. The reciprocity-calibration literature states this verbatim:
"a system is said to be reciprocity calibrated if `φ_n` are known **up to a
common constant** for all `n`" (`arXiv:2606.14419`, "On Optimal Strategies for
Joint Reciprocity Calibration in Distributed MIMO"), and over-the-air methods
"essentially measure `ϕ_n − ϕ_{n'}`" only (`arXiv:2509.03722`). In gauge terms
the observable state is a `U(1)` coset: the transformation `θ_i → θ_i + γ`,
`ψ_j → ψ_j + γ` for a single global `γ` leaves every measurement unchanged.
*Tier: established theory of distributed antenna systems; the identity is
hardware-independent.*

**3. There is also a time/frequency gauge.** With independent LOs there is a
per-link carrier-frequency offset (CFO) and a per-link timing offset. As with
phase, only **relative** frequency and time are observable: a global frequency
scale `t → αt` and a global time origin `t → t + β` applied to the whole
network are unobservable. So the full gauge group per connected component is
`U(1) × (R_{>0} × R)` — one global phase, one global frequency, one global
epoch. The clock-synchronisation literature treats exactly the relative
quantity as estimable and derives bounds on it
(`arXiv:1606.00400`, "Scalable and Passive Wireless Network Clock
Synchronization"; Chaudhari–Serpedin–Qaraqe, IEEE TIT 2010). *Tier: theory.*

**4. The gauge-invariant observables (what still works).** Under `γ` and the
time/frequency gauge the following are invariant and therefore observable from
independent-LO receive-only CSI:

- **Amplitude** `|ĥ_ij(k)|` — insensitive to all node phases. This is why
  channel-charting features use the **entry-wise absolute value** to "ignore
  phase shifts (e.g. stemming from APs that are not accurately synchronized)"
  (`arXiv:2406.13722`, "Channel Charting in Real-World Coordinates with
  Distributed MIMO"), and why RSSI/amplitude fingerprints work without
  synchronisation.
- **Subcarrier differential phase on one link**,
  `∠ĥ_ij(k₁) − ∠ĥ_ij(k₂)`. The common term `(θ_i − ψ_j)` cancels, leaving the
  delay-dependent term `−2π(f_{k₁}−f_{k₂})τ_ij`; this is the basis of ToF
  estimation and CSI phase sanitisation (SpotFi, SIGCOMM 2015; the common
  "linear-fit-and-subtract" phase sanitisation is standard, e.g.
  `arXiv:2303.16873`). Invariant to `γ`.
- **Inter-antenna differential phase within one receiver card**,
  `∠ĥ_{i,j₁}(k) − ∠ĥ_{i,j₂}(k)`. One receiver has one LO, so its chain phase
  is common and cancels; this yields **AoA at that receiver** (SpotFi). Also,
  on commodity NICs the clock-related phase error is "shared across all
  antennas on the same card, making them **antenna-common** quantities"
  (`arXiv:2606.07347`, "CSI Phase Averaging…"). Invariant to `γ`.
- **Two-way products / ratios**, `ĥ_AB · ĥ_BA`. The node phases **add**:
  `e^{j(θ_A−ψ_B)}·e^{j(θ_B−ψ_A)} = e^{j(θ_A−ψ_A)}·e^{j(θ_B−ψ_B)}`. If each
  node's transmit and receive chains share one LO, the LO terms cancel and only
  the **hardware asymmetry** `a_A b_B · a_B b_A` and `2×` propagation phase
  survive. This is precisely the identity `Ĥ_AB·Ĥ_BA⁻¹` and the basis of TDD
  reciprocity calibration (Rogalin et al., `arXiv:1310.7001`; `arXiv:2509.03722`).
- **Temporal differential CSI**, `∠ĥ(t₁) − ∠ĥ(t₂)` on one link — cancels the
  instantaneous common phase and is the basis of **Doppler** (moving targets
  only; blind to static scatter; per the BRIEF's standing assumption).

**5. What the gauge forbids — the provably unobservable list.** From
independent-LO receive-only CSI alone (no common reference) it is **impossible**
to observe:

- **Absolute cross-node phase** — hence coherent aperture synthesis across
  *geographically separated* receivers (distributed AoA/beamforming), which
  needs the relative phase between two *different* LOs.
- **Absolute range via carrier phase** (phase-of-flight across nodes), as
  distinct from one-way RSSI or two-way timing.
- **Absolute synchronisation** — a common epoch/frequency reference.
- **The split of the phase between the transmitter's LO and the receiver's LO**
  — only their difference is physical.

Note the asymmetry with AoA: **AoA within one receiver is observable** (one
LO), **AoA across receivers is not** — the boundary is between the array
element and the array node.

**6. Minimal added reference that crosses the boundary.** The gauge has exactly
one phase degree of freedom per connected component, so **one extra constraint
per component** fixes it. Three equivalent escapes, all reducing to "make the
measurement graph carry one absolute anchor":

1. **Common-transmitter reference.** One node broadcasts and every other node
   receives the *same* packet. Each receiver then measures
   `g_ij·e^{j(θ_src − ψ_j)}` — the single unknown `θ_src` is common to all, so
   the receivers' relative phases are recoverable up to one benign global
   constant. A **single anchor transmission** suffices. (Recall that Aquila's
   `esp_wifi_80211_tx` can transmit compliant frames; and the round-trip
   "one node pings, all reply" scheme is the same principle applied pairwise.)
2. **Over-the-air reciprocity (two-way exchange).** Bidirectional measurements
   between pairs yield `ϕ_n − ϕ_{n'}` directly; a connected calibration graph
   yields all phases up to the common constant. **Star topology minimises the
   Cramér-Rao bound** for per-antenna calibration accuracy (Zhu et al.,
   `arXiv:1709.07206`, "Massive MIMO Self-Calibration: Optimal Interconnection";
   Jiang et al.; `arXiv:2606.14419`).
3. **Shared/conducted reference.** Physically distribute a common clock and a
   phase-reference tone. This *removes* the gauge entirely rather than
   estimating it. ESPARGOS does exactly this: clocking all ESP32 microcontrollers
   "from the same crystal oscillator" and distributing "a phase reference signal
   to compensate for phase ambiguities introduced by the phase locked loops
   (PLLs)" makes CSI "synchronized in phase" (`arXiv:2408.16377`,
   `arXiv:2502.09405`; ESPARGOS One datasheet: "Shared 40 MHz clock and 2.4 GHz
   phase reference").

The minimal *wireless-only* reference is therefore **one common-transmitter
observation, or one reciprocal round trip per link**; the minimal *wired*
reference is a single shared clock+tone distribution. Cost scales `O(N)`
calibration exchanges (star: `N−1` round trips; the network-wide burst scheme of
Lanzisera et al. achieves full clock/carrier-phase/ToF knowledge "after `2N`
total transmissions", `Burst Mode Two-Way Ranging with Cramér-Rao Bound`).

**7. Cramér-Rao bounds on calibration precision (d).** Calibration precision is
finite and has a known form:

- **Single-phase CRB.** Estimating a complex amplitude/phase from one noisy
  observation at SNR `ρ`, the CRB on phase is `var(φ̂) ≥ 1/(2ρ)`; averaging
  `N_s` independent symbols gives `σ_φ ≳ 1/√(2 N_s ρ)`. This is the standard
  result, and its synchronisation-parameter form is the **Modified CRB**
  (D'Andrea, Mengali, Reggiannini, *IEEE Trans. Commun.* 42(2), 1994,
  `10.1109/TCOMM.1994.580247`).
- **With oscillator phase noise.** Phase noise (Wiener) makes the effective
  bound worse than the AWGN CRB; a hybrid/Modified CRB is the correct benchmark
  (Barbieri & Carbone, "On the Cramér-Rao bound for carrier frequency
  estimation in the presence of phase noise", IEEE TWC 2005; hybrid CRB for
  OFDM CFO/SFO, Springer `10.1007/s11277-022-09978-3`). This is why calibration
  must be **repeated before LO phase-noise accumulates**: `arXiv:2509.03722`
  models the phase noise spectrum `S(100 kHz)` in `[−120, −80] dBc/Hz` for
  realistic oscillators and uses a Kalman filter whose prediction variance grows
  with the calibration interval.
- **Network structural bound.** Reciprocity calibration has a **closed-form
  CRLB as a function of the measurement topology**, and — critically — the
  phase-estimation error **grows with the number of antennas for some
  topologies, in some cases without bound** (`arXiv:2606.14419`; Rogalin et al.,
  `arXiv:1310.7001`). Global calibration is provably no worse than local, and
  the star topology attains the minimum CRLB.
- **Clock bound.** Silent-node clock-offset CRLB and HCRB give
  `σ_clock ∝ σ_noise/√(epochs)` (`arXiv:1606.00400`; Chaudhari et al. 2010).
  Two-way time transfer in practice reaches ~300 ps in a 300 s average, with
  ~1 ns accuracy limited by **equipment-delay calibration**, not by the
  reciprocal-path cancellation itself (NIST TWSTT; Hanson, *43rd Freq. Control
  Symp.*, 1989).

So the boundary is not just qualitative: the recovered common reference can be
tracked, but at a precision set by a phase CRB that degrades with oscillator
quality, number of nodes, calibration topology, and time since the last
calibration.

**8. Receiver-only processing cannot synthesise the reference.** The
information-theoretic point is sharpened by the over-the-air-computation
literature: with independent CFO/phase offset, "typical estimation and
correction methods (…channel estimation, and linear equalization) relying on
the existence of independent parameters… are **not helpful in obtaining the
desired sum via receiver-only-based processing**"; transmitters **must**
precode, "which fundamentally requires phase synchronization in the network"
(`arXiv:2506.22252`, "On the Feasibility of Distributed Phase Synchronization…").
This is the cleanest published statement that the missing reference is not
recoverable by a smarter passive receiver.

### HYPOTHESIS

**H1. The wireless escapes are closable on ESP32-S3, but this is unproven.**
Per the BRIEF's standing assumption, none of the wireless-only escapes
(common transmitter, OTA reciprocity) is yet demonstrated on ESP32-S3 for
cross-node coherent phase. The supporting evidence is adjacent and strong but
*not on ESP32*: OTA reciprocity calibration is demonstrated on **SDR-grade**
hardware (Argos, `arXiv:1310.7001`; `arXiv:2509.03722`) and phase-coded pilots
for coherent superposition on **off-the-shelf SDRs** (`arXiv:2506.22252`).
Whether the ESP32 CSI estimator's per-frame phase state and the µs MAC timestamp
permit the required round-trip cancellation is an open question.

**H2. On ESP32-S3, the *invariants* (§4) are usable but *absolute phase is not*.**
OpenCSI states plainly that a "differential-phase formulation cancels per-boot
phase offsets and timing-offset trends that make **absolute ESP32 phase
unusable**" (`arXiv:2607.26665`). The ESP32 CSI path exposes amplitude and
per-subcarrier phase, but the per-boot RF-chain phase (randomised at PLL lock,
cf. the "random phase rotations… up to four possible values" on 802.11ac
commodity chips, Zubow et al., `arXiv:2005.03755`) means only differentials are
meaningful. This is consistent with the gauge theory above rather than
contradicting it.

**H3. The minimal ESP32 experiment will show the escape works for *relative*
phase (beamforming/AoA) but not *absolute* phase, at a precision limited by
phase noise.** Since the gauge is benign for coherent quantities, the realistic
ESP32 win is coherent *relative* operation (distributed aperture AoA, coherent
combining), calibrated at a cadence set by the phase-noise CRB.

### SPECULATION

**S1. A soldered-PCB polyhedron with a shared clock sidesteps the boundary
locally.** If the ~20 nodes of an icosahedral array share one clock and phase
reference (escape 3 in miniature), the gauge is removed *within the polyhedron*
with no wireless exchange, yielding 3D AoA from a single coherent aperture.
This is the ESPARGOS principle scaled to a volumetric array; whether the shared
40 MHz/2.4 GHz distribution remains phase-stable across a soldered 3D geometry
is untested.

**S2. A single common-transmitter anchor plus consensus oscillator locking could
make the reference self-sustaining across the fleet,** reducing re-calibration
overhead to that of a coupled-PLL network. This chains to Prompt 12; it is
plausible but derives no support from the sources here beyond the existence of
both mechanisms separately.

---

## Round-trip scheme (ping, unique replies) — what it measures and where it breaks

One node `A` pings; every other node replies with a **unique** packet; round
trips are timed.

**Measures (ESTABLISHED).**
- **Range**: round-trip time cancels the clock offset to first order
  (two-way ranging / two-way time transfer; NIST TWSTT; Lanzisera et al.).
- **Round-trip phase → hardware asymmetry + 2× propagation phase**: the LO
  terms cancel in the product `ĥ_AB·ĥ_BA` (the reciprocity identity, §4/§6),
  so the residual is the non-reciprocal RF-chain factor, which is what
  calibration needs.
- **Doppler**: temporal change of the round-trip phase (moving only).
- **Identity**: unique packets give per-node identity (a MAC-level fact, not a
  sensing quantity).
- **Channel**: each reply is a per-link CSI observation, giving the whole
  cross-link channel state.

**Breaks where (HYPOTHESIS/ESTABLISHED).**
1. **Turnaround drift**: LO phase drifts between the ping and the reply, so the
   two directions are not simultaneous; residual CFO is cited as the primary
   limitation for coherent phase alignment, "gradually destroy[ing] the
   coherency over time more quickly than the mobility in the channel"
   (`arXiv:2506.22252`). Two-way schemes mitigate by holding the turnaround
   time identical across nodes (the PCP procedure).
2. **Non-sharing of TX/RX LO within a node**: if a node's transmit and receive
   chains do not share phase, the round-trip product no longer cancels exactly,
   leaving the intra-node asymmetry as a residual.
3. **`2π` ambiguity**: phase-of-flight wraps; a coarse range/timing estimate is
   needed to unwrap.
4. **Mobility / non-reciprocity**: a moving scatterer between the two directions
   breaks the reciprocal-path assumption.
5. **TDM overhead**: all-pairs calibration is `O(N²)`; a star is `O(N)`.
6. **Timing quantisation**: the ESP32 MAC µs RX timestamp bounds range
   resolution coarse (`1 µs → ~150 m` one-way); FTM is the specialised,
   finer path.

---

## ESP32-S3 feasibility

- **What is directly usable on ESP32-S3** (HYPOTHESIS, supported by adjacent
  literature): the gauge **invariants** — amplitude, subcarrier differential
  phase (ToF), inter-antenna differential phase within a card (AoA), temporal
  differential phase (Doppler). Absolute phase across nodes is **not** usable
  (`arXiv:2607.26665`).
- **Escapes**: escape (3) shared/conducted reference is **demonstrated on
  ESP32-family** hardware (ESPARGOS uses ESP32-S2 with a shared 40 MHz clock +
  2.4 GHz phase reference, `arXiv:2408.16377`). Escapes (1) common transmitter
  and (2) OTA reciprocity are **demonstrated on SDR-grade** hardware
  (`arXiv:1310.7001`, `arXiv:2509.03722`, `arXiv:2506.22252`), **not yet on
  ESP32-S3**. esp-crab (Espressif) shows a partial, wired step: two ESP32-C5
  chips joined by a clock buffer to "eliminate the relative frequency offset",
  after which phase is usable for mm-level disturbance sensing.
- **Quantified gap**: ESPARGOS reaches cross-antenna/cross-board phase coherence
  only with a **wired** 40 MHz clock **and** a ~2.4–2.5 GHz phase-reference
  tone; without the phase reference the PLLs lock at random phase
  (`arXiv:2502.09405` "Source of Phase Uncertainty"). The wireless-only
  escapes have not been shown to close that gap on commodity ESP32.

---

## TX needed?

**Yes, for the wireless escapes.** The common-transmitter reference (escape 1)
and OTA reciprocity (escape 2) both require transmitting compliant 802.11
frames, i.e. `esp_wifi_80211_tx`. **Receive-only** ESP32 CSI can only ever
access the invariants (§4); to cross the phase boundary wirelessly it needs TX,
or it needs the conducted shared reference (escape 3, which ESPARGOS uses and
which requires no TX). This is the single most important chaining statement in
this note.

---

## Evidence strength

**8 / 10.** Justification: the *unobservability* itself is a formal,
hardware-independent fact with multiple independent primary derivations
(distributed-MIMO calibration theory `arXiv:1310.7001`/`2606.14419`/`2509.03722`;
channel-charting phase-insensitive features `2104.13184`/`2406.13722`;
over-the-air-computation "receiver-only is not helpful" `2506.22252`; phase
retrieval "up to a global phase" `10.1137/19M1256865`). The **minimal-reference**
result and the **CRB-on-calibration** results are equally well established
(topology CRLB; Modified CRB). The deduction is weakened by hardware tier: the
*wireless* escapes are unproven on ESP32-S3 (per the BRIEF's assumption), and
the ESP32-specific unusability of absolute phase rests on one 2026 primary
(`2607.26665`) plus the general commodity-phase-offset result
(`2005.03755`). Hence not a 10: the theory is solid; the ESP32 realisation of
the escape is a hypothesis awaiting its own experiment.

---

## Primary sources

- **Rogalin et al.**, "Scalable Synchronization and Reciprocity Calibration for
  Distributed Multiuser MIMO", arXiv:1310.7001 — reciprocity calibration,
  reference antenna, CRB for synchronisation. (SDR)
- **Zhu, Wang, et al.**, "A Framework for Over-the-Air Reciprocity Calibration
  for TDD Massive MIMO Systems", arXiv:1709.07206; and "Massive MIMO
  Self-Calibration: Optimal Interconnection for Full Calibration" — closed-form
  CRLB; **star topology optimal**.
- **"On Optimal Strategies for Joint Reciprocity Calibration in Distributed
  MIMO"**, arXiv:2606.14419 — phase known "up to a common constant"; error grows
  with antenna count for some topologies.
- **"Distributed MIMO With Over-the-Air Phase Calibration Integrated Into the
  TDD Flow"**, arXiv:2509.03722 — OTA reciprocity calibration; three noise
  sources; phase-noise spectrum −120…−80 dBc/Hz; Kalman re-calibration.
- **"On the Feasibility of Distributed Phase Synchronization for Coherent
  Over-the-Air Computation"**, arXiv:2506.22252 — phase-coded pilots;
  receiver-only processing insufficient; residual CFO limits coherence;
  demonstrated on off-the-shelf SDRs.
- **Barbieri & Carbone**, "On the Cramér-Rao bound for carrier frequency
  estimation in the presence of phase noise", IEEE Trans. Wireless Commun. 2005
  (IEEE Xplore 1577889).
- **D'Andrea, Mengali, Reggiannini**, "The Modified Cramér-Rao Bound and its
  Applications to Synchronization Problems", IEEE Trans. Commun. 42(2), 1994,
  DOI `10.1109/TCOMM.1994.580247`.
- **Chaudhari, Serpedin, Qaraqe**, "On minimum variance unbiased estimation of
  clock offset in distributed networks", IEEE Trans. Inf. Theory 56(6), 2010;
  and "Cramér-Rao Lower Bound for the clock offset of silent nodes".
- **"Scalable and Passive Wireless Network Clock Synchronization"**,
  arXiv:1606.00400 — CRB/HCRB for clock synchronisation.
- **Lanzisera et al.**, "Burst Mode Two-Way Ranging with Cramér-Rao Bound Noise
  Performance", IEEE GLOBECOM 2008 (IEEE Xplore 4697945) — TWR to the CRB; "2N
  transmissions" gives full network clock/carrier-phase/ToF.
- **NIST**, "Two-Way Satellite Time and Frequency Transfer (TWSTFT)",
  https://tf.nist.gov/time/twoway.htm ; **Hanson**, "Fundamentals of Two-Way
  Time Transfers by Satellite", 43rd Annual Frequency Control Symposium, 1989,
  https://tf.nist.gov/general/pdf/836.pdf — reciprocal-path cancellation;
  ~1 ns accuracy limited by equipment-delay calibration.
- **ESPARGOS**: arXiv:2502.09405 (channel sounder; "Source of Phase
  Uncertainty"); arXiv:2408.16377 (phase-coherent CSI datasets; shared 40 MHz
  clock + 2.4 GHz phase reference). https://espargos.net
- **Le Magoarou**, "Efficient channel charting via phase-insensitive distance
  computation", arXiv:2104.13184 (IEEE WCL 2021).
- **"Channel Charting in Real-World Coordinates with Distributed MIMO"**,
  arXiv:2406.13722 — entry-wise absolute value ignores phase shifts from
  unsynchronised APs.
- **DICHASUS tutorial** — "Channel coefficient phases are relative"; reference
  transmitter leaves constant phase offsets; global phase rotation over time.
  https://dichasus.inue.uni-stuttgart.de/tutorials/tutorial/dissimilarity-metric-channelcharting/
- **SpotFi**, SIGCOMM 2015, https://web.stanford.edu/~mkotaru/sigcomm15-spotfi.pdf
  — subcarrier differential phase = ToF; inter-antenna differential phase = AoA;
  phase sanitisation.
- **Zubow, Gawłowicz, Dressler**, "On Phase Offsets of 802.11ac Commodity
  WiFi", arXiv:2005.03755 — random per-RF-chain phase rotations, up to four
  values, frequency-dependent.
- **"OpenCSI: Self-Calibration Layer for Heterogeneous Mesh Wireless Sensor
  Networks"**, arXiv:2607.26665 — "absolute ESP32 phase unusable";
  differential-phase formulation required.
- **Grohs, Koppensteiner, Rathmair**, "Phase Retrieval: Uniqueness and
  Stability", SIAM Review, DOI `10.1137/19M1256865` — recovery from magnitude
  only "up to a global phase".
- **Espressif ESP-CSI / esp-crab**,
  https://docs.espressif.com/projects/esp-techpedia/en/latest/esp-friends/solution-introduction/esp-csi/esp-csi-solution.html
  — clock buffer to remove relative frequency offset between two chips.
- **UCSB distributed massive MIMO survey** (`06804225.pdf`) and **two-state
  oscillator model** (`06310822.pdf`), https://wcsl.ece.ucsb.edu — independent
  LOs; effective channel `g·e^{j(φ_t−φ_r)}`; Kalman phase tracking.

---

## Residual unknowns

1. **ESP32-S3-specific residual** of OTA reciprocity: does the CSI estimator's
   per-frame phase state permit the `ĥ_AB·ĥ_BA` cancellation? Unmeasured.
2. **ESP32-S3 LO phase-noise spectrum** and resulting coherence time — needed to
   set the CRB-based re-calibration cadence; not found for the S3 specifically.
3. **Whether the common-transmitter escape tolerates the ESP32's per-boot phase
   reset** across a fleet without a shared clock.
4. **Achievable two-way phase precision on ESP32-S3** given the µs MAC
   timestamp and the CSI phase quantisation.
5. **Whether the wireless escapes can be combined with the soldered-polyhedron
   shared-clock escape** without mutual interference (chains to Prompts 08/09).
6. **The exact CRB constant for the star topology under ESP32's noise
   model** — derivable but not yet instantiated for this hardware.

---

## Chaining / Existing-models-and-data / Own-data

### (A) Chaining — dependencies between Aquila capabilities

- **`esp_wifi_80211_tx` (compliant TX) is the highest-leverage enabling
  capability** for crossing the theoretical boundary *wirelessly*: it is the
  precondition for **both** escape (1) common-transmitter reference and escape
  (2) OTA reciprocity. "Common-transmitter reference **enabled by** compliant
  TX"; "OTA reciprocity **blocked without** TX". Without TX, Aquila can only
  cross the boundary via escape (3), a **shared/conducted** clock+tone — which
  ESPARGOS demonstrates needs no TX.
- **Crossing the boundary in turn unlocks**: distributed (cross-node) coherent
  aperture → 3D AoA over a fleet; coherent beamforming/combining; channel
  charting in **absolute** coordinates (the gauge removed); better RTI (higher
  fidelity cross-links); map-aided calibration (Prompt 14). These are the
  capabilities that are *blocked* by the gauge today.
- **Raw I/Q (esp-sdr)** enables finer phase/range *measurement* for calibration
  (a better calibration observable), but does **not by itself** cross the
  boundary — it is still independent-LO receive-only; it improves the precision
  (the CRB constant), not the observability (the gauge).
- **The round-trip ping scheme** is the minimal *experiment* form of escape (2);
  it is enabled by TX and by the µs RX timestamp, and it measures range,
  round-trip phase (hardware asymmetry), Doppler, identity and channel, but not
  absolute phase.
- **Polyhedron with shared clock** removes the gauge *locally* (unlocks 3D AoA
  within the array) without a wireless escape — but does not synchronise the
  polyhedron to the rest of the fleet unless it also uses escape (1)/(2).

### (B) Existing models/data

- **Distributed-MIMO calibration theory** (Rogalin/Argos `1310.7001`; Jiang/Zhu
  CRLB `1709.07206`; `2509.03722`; `2606.14419`) — the mature model of the
  gauge, the escapes, and the CRBs. **Directly usable** as the mathematical
  framework; the demonstrations are SDR-grade.
- **Two-way time transfer** (NIST TWSTT; IEEE 1588 PTP; White Rabbit) — the
  time/frequency escape family; usable as the design template for the
  ping-and-reply cadence.
- **Phase-coded pilots / coherent OAC** (`2506.22252`) — a concrete protocol for
  the wireless escape; demonstrated on SDR, licensable method (academic).
- **Channel charting with phase-insensitive features** (Studer et al.;
  `2104.13184`; `2406.13722`; DICHASUS) — **usable immediately** on ESP32,
  precisely because it is built to be robust to the gauge; it consumes the
  invariants (§4). **ESPARGOS/DICHASUS datasets** (espargos.net/datasets,
  dichasus.inue.uni-stuttgart.de) provide phase-synchronised, ground-truth CSI
  for validating Aquila's *invariant* pipelines — but being phase-synchronised
  they do **not** exercise the boundary itself.
- **Phase retrieval** (`10.1137/19M1256865`; Candès PhaseLift) — establishes
  magnitude-only recovery "up to a global phase"; useful to formalise the
  invariant structure, but the ESP32 magnitude-only route is lossy and the
  algorithms are host-heavy.

### (C) Own-data / new technology

- **The un-demonstrated thing**: closing a *wireless* escape (common
  transmitter or OTA reciprocity) on **ESP32-S3** specifically, and quantifying
  the gap to ESPARGOS (conducted) and to SDR (OTA). No published work found that
  does this on the S3.
- **Minimal experiment**: two or three ESP32-S3 boards, A, B and (optionally) a
  reference C. (i) Establish a bidirectional link A↔B with
  `esp_wifi_80211_tx`; record `ĥ_AB` and `ĥ_BA` over minutes and compute the
  round-trip product/ratio; test whether its phase is stable and equals the
  hardware asymmetry rather than drifting with the LO. (ii) Independently, have
  a common node C broadcast and have A and B both receive, recovering their
  *relative* phase up to the common constant. (iii) Track cross-node relative
  phase with both escapes and compare against the phase CRB
  (`σ_φ ≳ 1/√(2 N_s ρ)` plus phase-noise growth). **Success** = cross-node
  relative phase tracked below a set threshold for a period exceeding the LO
  coherence time, and used to form a coherent cross-node quantity (e.g. distributed
  AoA or a measurable coherent-combining gain). **Falsification** = the residual
  grows without bound, or is statistically indistinguishable from random, or the
  two escapes disagree by more than their respective CRBs. Compare
  ESP32-S3 ↔ ESPARGOS ↔ SDR to quantify the gap.
- **A complementary dataset**: measure and publish the ESP32-S3 LO phase-noise
  spectrum, the µs RX-timestamp jitter, and the per-boot phase-reset
  distribution — these are the inputs the CRB/Kalman cadence needs and are (to
  our sources) not published for the S3.

---

## One-line answer

From independent-LO receive-only CSI, **absolute cross-node phase, absolute
range-from-phase, and absolute synchronisation are provably unobservable** —
they sit behind a per-component `U(1)` phase (plus time/frequency) gauge; the
gauge-invariant observables are amplitude, per-link subcarrier differential
phase (ToF), per-receiver inter-antenna differential phase (AoA), two-way
products, and temporal differentials (Doppler); **one common-transmitter
broadcast, one reciprocal round trip per link, or one shared clock+tone** fixes
the gauge and is the minimal added reference, with precision set by a
Cramér-Rao bound that degrades with oscillator phase noise, node count and
calibration topology. The crucial Aquila chaining fact: **wireless escapes need
`esp_wifi_80211_tx`; the conducted escape (ESPARGOS) does not.**
