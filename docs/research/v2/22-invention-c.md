# 22 — Invention: WIRELOCK

## The home's existing mains wiring as a coherence spine — freezing the U(1) gauge so a house of cheap ESP32-S3 pucks becomes one persistent coherent 3D aperture

**Prompt.** `22-invention-c`. Discover the new invention that makes a home full of cheap
ESP32-S3 nodes behave as one sharp, self-calibrating, generalising 3D sensor — or prove that
no such invention exists and state the exact physical boundary that blocks it. Hardware
premise: a ~30 cm soldered-PCB polyhedron, one ESP32-S3 per face, a hub face with a shared
40 MHz clock fan-out, a conducted 2.4 GHz phase reference and data aggregation.

Status: invention proposal + boundary analysis. Australian English. Grades are **ESTABLISHED /
HYPOTHESIS / SPECULATION** and are stated per claim; hardware tiers are **ESP32-S3 /
ESP32-family / other COTS / SDR-grade / non-radio infrastructure**.

Inputs read in full: `BRIEF.md`, `00-synthesis.md`, and the sibling notes `20`, `12`, `11`,
`10`, `17`, `h`, `o`, `n`, `p`, `r`, `b`, `t` (to avoid duplication; §9 audits the overlap
and credits the one seed this note builds on). Method: layered `parallel-search` →
`parallel-fetch` over primaries (arXiv, IEEE, standards, vendor documentation, upstream
code); every load-bearing number below has a source in §13.

---

## 0. Verdict in one page

**The wireless-only route to one home-wide coherent aperture is gated, not closed, and the
gate is arithmetically brutal.** Every wireless escape (common transmitter, OTA reciprocity)
must fight a *relative frequency error* `Δf_res` that integrates into phase over the interval
`T` between references: the phase-accrual quantity `2π·Δf_res·T` (note `p`'s inequality;
note `b`'s M2′; the synthesis §2). With post-CFO residuals of 10–100 Hz, coherence survives
milliseconds, not minutes: the whole fleet must be re-anchored at frame cadence, forever,
over the air, in the same band it senses with.

**The invention is not a new wireless escape. It is the recognition that the escape does not
need to be wireless.** Call it **WireLock**:

> **A dwelling already contains a reciprocal, high-SNR, house-wide conductor network — its
> mains wiring. Inject one phase-coherent reference tone into it. Every node recovers that
> tone and locks its 40 MHz clock to it (phase transport, not frequency metering). Then the
> cross-node LO phase stops being a per-epoch nuisance and becomes a *lock-state constant*:
> it is random only at discrete re-lock events (boot, channel change, thermal step), and one
> over-the-air phase-calibration round per lock state fixes it for the entire state.**

The load-bearing physics is a lemma already measured by ESPARGOS on ESP32-family silicon:
PLL output phase "changes after a chip reset and after modifying the LO frequency… every time
the PLL has to acquire a new frequency lock" — and *nothing else* — while "synchronizing
receivers in frequency using the 40 MHz reference clock signal **should be sufficient for
achieving long-term phase stability**" (`arXiv:2502.09405`). The gauge of note `20` is a
*U(1) constant per connected component*; WireLock makes that constant **time-invariant per
lock state** instead of per-epoch, and measures it once.

**The three-tier system:** (1) the **Spine** — a ~US$20–40 master dongle injects a
2–30 MHz reference into the mains; each node couples to it (US$3–10 of certified-PLC-class
analogue front-end) and phase-locks its 40 MHz reference (external-clock injection or a
40 MHz VCTCXO, à la ESP-PPB); (2) the **Lock-State Protocol** — one OTA calibration round
(`esp_wifi_802x` exchange or fanned common-transmitter broadcast) per lock state, triggered by
boot, channel hop, thermal step (on-chip temperature sensor) or mains-topology event; and (3)
the **Closure Audit** — the always-on gauge-invariant health monitor (notes `h`,`o`) that
detects any slip between calibrations and orders a repair. The spine is radio-silent: no
reference airtime, no contamination of the sensing band.

**Sharpness is inherited, not re-invented.** Once the gauge is frozen, the cohort's coherent
machinery becomes *persistent* rather than episodic: home-scale baselines give `λ/D ≈ 1°`
bearing resolution at 5–10 m; the home sits deep in the near field of its own mesh, so depth
comes from wavefront curvature at millimetre scale (note `h`'s 4 mm estimate) rather than the
7.5 m delay bin of 20 MHz; two-way carrier-phase ranging becomes a *drift-free ruler* (the
round-trip product's hardware asymmetry is a true constant), giving the self-survey; and the
closure invariant makes the system self-diagnosing. The polyhedron puck remains what the
premise says it is: the coherent atomic 3D sub-aperture (ESPARGOS principle, in the round).

**The decisive experiments are cheap and ordered.** **E0 (channel):** log the *phase* of
2–30 MHz S21 between outlet pairs in real homes for days with appliance cycling and extract
the phase-stability interval, the mains-synchronous modulation depth (PLC channels are
linear periodically time-varying at 50/100 Hz — Corripio et al. 2006) and the topology-event
statistics. **E1 (system):** two mains-powered ESP32-S3 nodes, one spine master, measure the
cross-node 2.4 GHz LO phase against a conducted ground truth for 24–72 h; **pass** if phase
is piecewise constant within ±0.4 rad per lock state, events are rarer than one per ten
minutes and every event is detected and repaired within one OTA round (≤ ~100 ms with
ESP-NOW bursts). If E0/E1 fail, the boundary stands exactly as the cohort states it, and
WireLock degrades to its documented fallbacks (coax splice → OTA per epoch → closure-only).

**Feasibility, cost, TX.** Every component is demonstrated somewhere: PLC channels and
certified AFEs (ESTABLISHED, non-radio infrastructure); ESP32-family clock injection and
VCTCXO discipline (ESTABLISHED on C3/S2 — grey literature + product); the static-gauge lemma
(ESTABLISHED on ESP32-S2, ESPARGOS). The untested parts are the composition (mains-carried
*phase* reference) and S3-specific clock injection — both bench-measurable in a week.
Prototype spine ≈ US$50–150 per home; pucks as per the premise. **Transmit: the spine needs
none; the calibration layer needs one round per lock state (vs per epoch today); sensing
transmits as usual.**

**Evidence strength: 6/10** — see §10 for the per-claim breakdown. The load-bearing unknown
is measured by E0, and E0 requires no ESP32 work at all.

---

## 1. What is closed and what is not, carried forward exactly

### 1.1 The gauge (ESTABLISHED — note `20`, re-verified)

With independent LOs, a per-subcarrier CSI observation is

```
ĥ_ij(k) = g_ij(k)·a_i·b_j·e^{j(θ_i−ψ_j)}·e^{−j2πf_kτ_ij},
```

only the difference `θ_i−ψ_j` is observable, and on a connected measurement graph the node
phases are fixed only **up to one global additive constant per component** (`U(1)`, plus a
time/frequency gauge). Invariants: amplitudes; per-link subcarrier differential phase
(delay); per-receiver inter-face differential phase (3D bearing *within* one puck);
temporal differentials (Doppler); two-way products. Not observable: absolute cross-node
phase, absolute phase-of-flight, absolute synchronisation. The minimal references: one
common-transmitter broadcast, one reciprocal round trip per link, or one shared
clock+tone — cost `O(N)`.

### 1.2 The gate, named (ESTABLISHED arithmetic; input HYPOTHESIS)

The quantity that closes the *wireless-only, always-coherent* home is the phase accrued by
the residual relative frequency over the reference gap:

```
Δφ = 2π·Δf_res·T    ⟶   coherence interval  T* = ε/(2π·Δf_res).
```

- Raw, untracked crystals ±10–20 ppm ⇒ `T_{π/2}` ≈ 5–10 µs (note `17`).
- Post-CFO-removal residuals today: ~10–100 Hz ⇒ **ε = 0.4 rad at T ≈ 0.6–6 ms** (synthesis
  §2; note `r`'s "coherence budget" `B = Δν_res·T_obs`).

So the wireless escapes impose a *per-epoch* reference cadence. Sibling notes solve the
protocol economics of that (a, b, g, i, q, r, s, t) or refuse the game entirely and work
in the gauge-invariant subspace (h, o, n). This note takes the remaining box in the design
space: **change the reference cadence by changing what the reference is.**

### 1.3 The fourth corridor (the mission's "beyond"): pre-existing conductors

The published escape taxonomy lists common transmitter, OTA reciprocity, and
shared/**conducted** reference — but in every published use the conducted reference means
*new cable* (ESPARGOS distributes clock+tone over coax/microstrip; esp-crab over short PCB
traces). Sibling `r` was the first to say, in one speculative paragraph (graded 1/10,
targeting a 625 kHz tone), that *the home's existing wiring is a cable reaching every room*.
WireLock promotes that seed to an engineering proposal with physics, protocol, standards,
fallbacks and falsifiers. The corridor is the home's **mains wiring plus protective earth**:
not new cabling, not radio.

---

## 2. The physics of the two references: why phase transport freezes the gauge

### 2.1 Frequency metering vs phase transport (ESTABLISHED distinction)

A reference can be used in two inequivalent ways, and the difference is the entire invention:

- **Frequency metering** (count crossings, steer a VCTCXO to the reference's *rate*): the
  residual frequency error integrates linearly into phase. Discipline to 1 ppb at 2.44 GHz
  still means 2.44 Hz of drift — a full cycle every ~0.4 s. This is the ESP-PPB regime:
  excellent, but still requiring per-frame phase anchoring.
- **Phase transport** (a PLL *locks to the phase of the received reference waveform*, so
  each node's 40 MHz output is a fixed function of that waveform): the per-node LO phase
  becomes a *constant* (the loop's static phase error — a calibration datum), and all that
  moves it thereafter are discrete re-lock events and slow thermal gradients, not a
  frequency offset. ESPARGOS is the existence proof at system level: distributed 40 MHz
  clock *and* periodic phase calibration against a reference network; between calibration
  events the phase is stable over long intervals in a static environment.

**WireLock requires phase transport.** Frequency metering is the cheap fallback (Tier 2).

### 2.2 The static-gauge lemma (ESTABLISHED on ESP32-family; the note's core statement)

> **Lemma.** If every node in a connected network is phase-locked to one common reference
> signal, then every cross-node carrier-phase offset is *piecewise constant in time*: it
> changes only at discrete lock-state transitions — boot, channel change, PLL re-lock,
> thermal-event beyond loop authority — and is invariant otherwise.

Evidence, from ESPARGOS's measurements on ESP32-S2 arrays (`arXiv:2502.09405`, §II-A,
IV-A):

1. "the initial phase changes after a chip reset and after modifying the LO frequency
   (WiFi channel), likely every time the PLL has to acquire a new frequency lock" — i.e.
   the phase randomisation is *event-indexed*, not continuous;
2. "Synchronizing receivers in frequency using the 40 MHz reference clock signal **should be
   sufficient for achieving long-term phase stability**, even if no phase reference signal is
   provided" — measured over long intervals in a static environment (their Fig. 6);
3. the 2.4 GHz phase-reference network exists to *calibrate* the per-lock-state offsets and
   distribution-line delays, not to chase drift — "an additional, periodically performed
   phase calibration step is vital" where "periodically" is set by resets/channel changes.

The residual that must be respected (HYPOTHESIS, quantified by grey-literature measurement):
even a *shared* 40 MHz leaves a small differential thermal wander — ESP-PPB measured ~1 ppb
(1 ns/s) between two ESP32s on the *same* board with one TCXO, attributed to thermal
gradients (`esp-ppb/story.md`, Board 6) — plus temperature-dependent analogue phase in each
RF chain (ESPARGOS §II-A). This is a *bounded, event-linked* term, not a free-running
frequency offset: it is handled by (i) temperature-triggered re-anchoring using the S3's
on-chip temperature sensor, and (ii) the closure audit (§4.3). **Consequence: the reference
cadence moves from every frame (ms) to every lock state (seconds–hours).**

### 2.3 Why the master oscillator may be cheap (ESTABLISHED algebra + HYPOTHESIS)

All nodes track the *same* reference waveform. The master's own phase noise and frequency
wander are **common-mode**: they move every node's clock together and cancel in every
cross-node observable (they do not cancel a node's own TX/RX asymmetry, which is separate).
The spine therefore does not need an OCXO or a GPS-disciplined source: an ordinary TCXO —
even a mediocre one — suffices, *provided the transport channel itself is phase-stable*
(the per-node channel delay is what differs between nodes). This inverts the cost model of
the OTA escapes, which need ppb-class discipline at every node (ESP-PPB: VCTCXO + dual DAC
per node).

### 2.4 What the frozen gauge buys (composition with the cohort; ESTABLISHED algebra)

- **One-time calibration economics.** Escapes 1/2 cost `O(N)` exchanges **per epoch**;
  WireLock costs one round **per lock state**. The sensing band stays radio-silent between
  events — a first-class system property, not a nicety (the common-transmitter escapes
  pollute exactly the CSI they calibrate; the first and last frames of every reference
  burst carry extra multipath).
- **Absolute two-way ranging becomes drift-free.** The two-way product `ĥ_AB·ĥ_BA` cancels
  LO in *every* scheme, but only under WireLock does the surviving hardware-asymmetry term
  `a_Aa_Bb_Ab_B` hold still *indefinitely*: the round-trip phase becomes a ruler with a
  one-time offset, so `O(N²)` link ranges (FTM-coarse + carrier-phase-fine) give the
  self-survey, and note `s`'s geometry-first route and note `p`'s Fiducial gain a stable
  substrate.
- **Persistent near-field depth.** Note `h`'s result — depth from wavefront curvature at
  ~4 mm over an 8 m baseline — stops being an episode and becomes the default mode.
- **Closure audit is exact.** Loop products of the (now stable) link phases should be
  constant; any deviation flags a lock-state transition, a channel event, or a real scene
  change — three cases separable by their time signature (§4.3).

---

## 3. The medium: mains wiring as an HF phase channel

### 3.1 What is known (ESTABLISHED, non-radio infrastructure)

- **Band and physics.** In-home power line communication operates 1.8–86 MHz (HomePlug AV2;
  ITU-T G.9960/G.hn), with regulatory PSD practice around −50 dBm/Hz (1.8–30 MHz) and
  −80 dBm/Hz (30–86 MHz) (HomePlug AV2 overview, Wiley 2013, `10.1155/2013/892628`). The
  channel is a **multipath transmission-line medium**: the Zimmermann & Dostert analytic
  transfer-function model (IEEE Trans. Commun. 50(4):553–559, 2002) describes branches and
  impedance mismatches; measured indoor responses show **near-linear phase in the passbands
  between notches** and severe frequency selectivity (Zimmermann–Dostert; Mosalaosi thesis;
  UKZN 2015).
- **Three conductors = path diversity.** In-home wiring exposes live (P), neutral (N) and
  protective earth (PE): up to 8 transmission paths (ETSI TR 101 562-1; Hashmat et al.
  ISPLC 2010, measurements to 100 MHz). A notch at the chosen reference frequency on P–N is
  the common failure mode; L/N/PE diversity and frequency selection are the mitigations.
- **Time variation has three known modes.** (i) *Quasi-static*: the channel is fixed between
  topology changes; (ii) *load-switching events*: plugging/unplugging appliances shifts
  notches and reflections (Hashmat et al.; Barmada et al. 2011); (iii) *mains-synchronous
  cyclostationarity*: the channel behaves as a **linear periodically time-varying system**
  at 50/100 Hz (Corripio et al., IEEE JSAC 24(7):1327–1338, 2006) — the impedance of
  nonlinear loads varies within the mains cycle. Mode (iii) is deterministic and periodic:
  it is a *bounded phase modulation of the reference*, not unbounded drift, and it is
  measured in E0 (§6) and averaged or tracked out.
- **The mains is already used as a time/frequency reference — at infrastructure coarse
  level.** The CMU SenSys 2009 "Syntonistor" (Rowe, Gupta, Rajkumar; best paper) tuned to the
  50/60 Hz field radiating from wiring: eight nodes across a building floor stayed
  synchronised to <1 ms over 11 days with no radio traffic, on the argument that "the power
  grid is phase-coherent" (differential delay stability ~1 part in 10⁻⁸ over 24 h, cited
  therein). Feng et al. (2007, IEEE FCS/EFTF) demonstrated a **time-transfer method over
  low-voltage power line**; IFAC 2015 (`10.1016/j.ifacol.2015.12.031`) gives PLC time sync
  via zero-crossings at ms level; arXiv:2608.12730 (2026) proposes **reciprocal two-way
  time distribution over power conductors using travelling waves** (µs level, simulation).
  The pattern is clear: PLC is an accepted clock/time conduit. **What has never been built
  is the carrier-phase-grade use that a coherent radio array needs.**
- **It is legal as in-home carrier current.** US: an in-house carrier-current system is an
  *unintentional radiator* ("In-House Broadband over Power Line", 47 CFR §15.3(gg)) subject
  to conducted limits (§15.107/§15.207); Australia: in-home PLT apparatus is covered by
  ACMA-mandated EMC standards (EN 50065-1; the in-home PLT apparatus standard), not by a
  radiocommunications transmitter licence. Using a **certified PLC analogue front-end**
  (coupler + filter + driver) converts compliance from a research problem into a
  procurement step.

### 3.2 What must be measured (the invention's load-bearing empirical questions)

1. **Phase-stability interval** of narrowband S21 between outlet pairs at candidate
   reference frequencies (2–30 MHz), in lived-in homes, with normal appliance cycling.
2. **Mains-synchronous phase-modulation depth** at those frequencies (mode iii) — predicted
   bounded and periodic; depth sets the tracking-bandwidth requirement.
3. **Topology-event statistics**: how often does the channel phase step by more than the
   repair budget, and does it step at all without a plug action?
4. **SNR headroom** against SMPS noise: recovered-clock jitter versus reference frequency,
   bandwidth and conductor pair (L/N/PE).
5. **Cross-circuit/cross-phase coupling**: are all outlets in a home one electrical
   coupled component at HF, or do breaker/phase boundaries partition it? (A partition is
   benign — it just means one master per component; note `20`'s gauge is per component.)

E0 answers 1–5 with instruments only.

### 3.3 Collector's note: the same corridor in other skins

Existing coaxial TV cable, telephone pairs, doorbell and intercom wiring, and any in-wall
Ethernet are the same class of pre-existing conductor. Where present, they are simpler
(shielded, matched) than mains. The mains is the universal one (every fixed node site has an
outlet) and the one with regulatory framing already in place.

---

## 4. The invention, component by component

### S1 — The Spine (transport). [HYPOTHESIS; components ESTABLISHED]

**Mechanism.** A line-powered master dongle plugs into any outlet and injects a
phase-coherent narrowband reference — a sine at `f_ref` in 2–30 MHz chosen per home by E0 —
into the mains via a safety-rated coupling network, on L–N (and PE as redundant path, as in
MIMO PLC). Each node has a coupler + bandpass + limiting amplifier; the recovered waveform
drives either (a) the node's 40 MHz reference via external-clock injection into XTAL_P
(ESP-PPB confirms "a full external oscillator on XTAL_P alone" is accepted on ESP32-family
chips; ESPARGOS clocks S2s from a distributed 40 MHz), or (b) a 40 MHz VCTCXO phase-locked
to `f_ref × N` (the ESP-PPB discipline architecture, with the reference transported over
copper instead of FTM), or (c) a clock-generator IC (jitter-attenuating PLL) with `f_ref` as
its reference input. Option (a) is simplest when the node need not hop channels; (b)/(c)
are the general cases.

**New.** Phase-grade transport of a common reference over *pre-existing in-dwelling
conductors* for cross-node radio LO coherence. Prior art transports clock/tone over
purpose-laid cable (ESPARGOS) or over the air (ESP-PPB, AirSync), or uses mains for coarse
time only (SenSys 2009, Feng 2007, IFAC 2015). No primary found that does carrier-phase
coherence over in-home mains. (Sibling `r`'s speculative one-liner is credited in §9.)

**Must be true.** §3.2 items 1–5. Falsifier: continuous (non-stepwise) phase wander, or
mains-synchronous modulation depth so large that a practical PLL cannot track it within
budget.

### S2 — The Lock-State Protocol (calibration economics). [HYPOTHESIS; algebra ESTABLISHED]

**Mechanism.** Per lock state, one over-the-air phase-calibration round fixes the whole
network's relative phases:

- cheapest: a fanned common-transmitter broadcast (escape 1) whose per-receiver phase
  differences are the relative LO phases (one frame, one transmitter, all nodes);
- or one two-way exchange per puck pair (escape 2, star-optimal) if absolute puck-to-puck
  phases are also wanted (they are needed for coherent transmit; not for receive-side
  sensing).

Lock-state transitions are declared by: boot; channel hop; PLL re-lock flag; on-chip
temperature step beyond a dead-band; mains-topology event (S3); or closure-audit alarm.
Between transitions: radio silence, pure sensing. The protocol's key inversion versus the
cohort's OTA schemes: **the calibration is event-driven because the physics changed the
event rate, not because the algorithm got smarter.**

**Must be true.** The lemma of §2.2 holds for the assembled stack (S3 RF PLLs under a
common 40 MHz). Falsifier: meaningfully continuous phase wander within a lock state —
caught quantitatively by the closure audit and by E1.

### S3 — The Closure Audit (self-diagnosis). [Composition; ESTABLISHED mathematics]

**Mechanism.** Compute the gauge-invariant loop products (closure phases, notes `h`/`o`;
Jennison 1958; Cornwell & Wilkinson 1981) continuously from ordinary sensing traffic. Their
temporal signature separates the three deviation causes:

| Signature | Cause | Response |
|---|---|---|
| All loops jump together at one node | lock-state event at that node | schedule one OTA round |
| Subset of loops drift smoothly | thermal wander on the affected nodes | temperature-triggered re-anchor |
| Loops change with no phase discontinuity and spatially coherently | real scene change (a person) | keep sensing; do not recalibrate |

This is the mechanism that makes "self-calibrating" credible: the system *knows* when it is
calibrated, using the invariant subspace that needs no calibration to compute.

### S4 — Anchor economics and the battery tier. [HYPOTHESIS]

Mains-locked nodes are *stable phase anchors*. Battery/mobile nodes (no mains coupling) use
the common-transmitter escape against the anchor set per frame — but now the anchors do not
drift, so the OTA work is a single, O(1) broadcast from a stable source rather than a
network-wide two-way round. The fleet is thus two-tier by construction: **anchors spend no
airtime; wanderers spend O(1) airtime.** The polyhedron pucks, with their internal
conducted 40 MHz+2.4 GHz reference, are the atomic coherent apertures in both tiers.

### 4.1 The architecture in one diagram

```
           [spine master: TCXO -> f_ref (2–30 MHz) -> coupler]        (mains outlet)
                               │
                        mains wiring (L/N/PE, 1.8–86 MHz PLC band)
              ┌────────────────┼─────────────────┐
        [puck A]           [puck B]           [node C]
        coupler -> PLL-lock 40 MHz (XTAL_P / VCTCXO / clock-gen)
              │                                   │
         ── one-time OTA phase round per lock state (ESP-NOW / esp_wifi_80211_tx) ──
              │                                   │
         closure audit (always on, zero extra airtime) -> event triggers -> repair round
```

### 4.2 Fallback tiers (ordered, honest)

1. **Tier 1 (this note):** phase transport over mains; cadence = lock-state events.
2. **Tier 2:** frequency metering over mains (count/steer); cadence = seconds, set by ppb
   holdover; still radio-cheap.
3. **Tier 3:** no mains at a node or a home: per-epoch OTA referencing (cohort routes a, b,
   n, t) or closure-only sensing (notes h, o) — the sensor degrades exactly as the cohort
   says, no faster.
4. **Tier 0 (bench):** a coax coherence splice between two adjacent pucks (`r`'s fallback):
   a local laboratory stand-in for E1's ground truth.

---

## 5. What must be true — the complete precondition list

| # | Condition | Tier | Test |
|---|---|---|---|
| C1 | In-home mains S21 phase is piecewise-stable at a usable `f_ref` (steps only at topology events), with mains-synchronous modulation depth trackable within PLL budget | 1 | E0 |
| C2 | A 2–30 MHz reference survives SMPS noise at every outlet with enough SNR to yield sub-ns recovered-clock jitter | 1 | E0 |
| C3 | ESP32-S3 accepts external clock injection on XTAL_P **or** a 40 MHz VCTCXO can be phase-locked to the recovered reference (ESP-PPB architecture) | 1 | bench (E1) |
| C4 | Under a common 40 MHz, the S3's RF PLL relative phase is lock-state-constant to within the phase budget (residual thermal wander ≤ budget over the lock state) | 1 | E1 |
| C5 | One OTA round per lock state measures relative phases to ≤ λ/16 within the MAC's timing | 1/2 | E1 |
| C6 | The closure audit detects all lock-state/channel events before they corrupt sensing | 1/2 | E1/E2 |
| C7 | Legal + safe: certified PLC-class coupling; injected PSD within conducted limits; mains isolation | 1 | compliance review |
| C8 | Triggered common-transmitter referencing works against mains-locked anchors (battery tier) | 2 | E1 variant |

C1 and C2 decide everything. Both are measurable without an ESP32.

---

## 6. Falsifying experiments

### E0 — Mains phase-stability sounding (days, instruments only) — the decisive one

Two calibrated ports (VNA or a two-node tone-phase logger with GPS-referenced timestamps) on
outlet pairs in ≥5 real homes, at ≥3 candidate `f_ref` in 2–30 MHz, over ≥72 h each,
sampling S21 amplitude and **phase** every second, with a log of every appliance action,
temperature, and occupancy. Deliverables: phase-stability interval distribution; event
rate and step sizes; mains-synchronous (100/50 Hz) modulation depth; SNR vs conductor pair
and frequency; component partition map.

**Pass thresholds (design budget `ε_chain` = 0.3 rad allocated to the channel):**
- 95th-percentile interval without any phase step > 0.05 rad exceeds 1 h;
- step events larger than 0.3 rad occur < 1 per 10 min and are *detectable* (they are
  steps, not ramps);
- 100 Hz modulation depth ≤ 0.1 rad at the chosen `f_ref`.
**Falsified if:** the phase wanders continuously (a random walk or drift exceeding 0.3 rad
over 10 min with no event), i.e. the channel is a *filter* whose phase the PLL cannot hold.

### E1 — Two-node WireLock drift test (the system decision)

Two mains-powered ESP32-S3 nodes on different circuits, one spine master; chip A also
connected to chip B by a coax reference used **only as ground truth**. Measure the
cross-node 2.4 GHz LO phase via the two-way product on a stable over-the-air link and via
the coax reference, continuously for 24–72 h, with scripted appliance cycling and a
temperature box over one node.

**Metrics and pass criteria:**
- within a lock state: `σ_φ ≤ 0.4 rad` (λ/16 ≈ 2 cm) and no monotone drift trend beyond it;
- lock-state transitions: detected by the temperature/closure triggers in ≤ 1 s;
- repair: relative phase restored to ≤ 0.4 rad within one OTA round (target ≤ 100 ms using
  ESP-NOW bursts at ~100 µs frame spacing, ESP-PPB Board 7's demonstrated rate);
- compared against the wireless-only control (same two nodes, spine off, ESP-PPB-style
  FTM discipline): the control must show shorter stable intervals by ≥ 10×, or WireLock's
  advantage is unproven.
**Falsified if:** relative phase wanders at the same rate with and without the spine, or
S3's PLL cannot be locked to the mains-recovered reference (C3).

### E2 — Closure-audit end-to-end (three pucks)

Three mains-locked pucks; induce deliberate lock-state events (channel hop, temperature
step, plug/unplug) and real scene changes (a person); verify the audit's three-way
signature classification and that no scene change is ever "repaired away".

_E0 alone is the cheapest decisive test in the entire programme: if mains channel phase is
not step-wise stable, WireLock dies and the boundary stands unchanged._

---

## 7. ESP32-S3 feasibility and cost

### 7.1 Hardware path (tiered, with the S3-specific unknowns named)

- **Clock injection, family tier (ESTABLISHED, grey literature + product):** ESP-PPB
  found "the ESP32 accepts a clock two ways: either a crystal on XTAL_P/XTAL_N… or a full
  external oscillator on XTAL_P alone" (`esp-ppb/story.md`, Board 4). ESPARGOS clocks all
  ESP32-S2s of an array from a daisy-chained distributed 40 MHz reference
  (`arXiv:2502.09405` §II). Espressif's own C3+S3 CSI motion board shares a clock buffer
  between chips (synthesis §2, `20`). External clock on XTAL_P is therefore a used
  technique on the **family**.
- **S3-specific caveat (HYPOTHESIS):** the ESP32-S3 Hardware Design Guidelines list only the
  "external crystal clock source (compulsory)… firmware only supports 40 MHz" and the
  optional 32.768 kHz RTC source; community threads on driving S3's XTAL pins from an
  external 40 MHz oscillator exist but the S3 datasheet does not bless it as the original
  ESP32's "External 2~40 MHz" root clock does. **Bench item C3.** Two sanctioned fallbacks
  exist and both are ESP-PPB-proven *architectures*: (i) a 40 MHz **VCTCXO** in the crystal
  position, phase-locked (not merely frequency-disciplined) to the mains-recovered
  reference — the same DAC/varactor loop ESP-PPB built on C3, with a different reference;
  (ii) a **jitter-attenuating clock generator** that multiplies `f_ref` to 40 MHz and drives
  the XTAL input or the VCTCXO control node. Either keeps the invention intact; only the
  BOM changes.
- **Why the S3's own CFO interpolation helps, not hurts (ESTABLISHED, grey):** ESP-PPB
  discovered that the ESP32's PHY estimates CFO per packet and silently compensates; when
  two nodes' CFO corrections disagree, "the results are garbage". With a common reference
  both nodes compute the *same* correction and CSI phase is clean — a second, independent
  reason a shared clock beats per-node discipline: it removes a hidden corruption mode.
- **Recovery stack is commodity:** the node side of a certified PLC analogue front-end
  (coupling network + band-pass + LNA + comparator) is a small, licensable design; PLC
  silicon vendors ship reference schematics meeting conducted-emission limits.

### 7.2 Cost (prototype/volume, USD)

| Item | Prototype | Volume |
|---|---|---|
| Spine master (TCXO + driver + coupler + housing) | 30–50 | 15–25 |
| Per-node coupler + recovery + clock-gen/VCTCXO | 8–15 | 3–8 |
| Pucks / faces | per premise (ESP32-S3 ≈ 3–5 each) | — |
| **Six-node home, spine total** | **≈ 80–140** | **≈ 35–75** |

Against the alternative: ESP-PPB-class OTA coherence costs a VCTCXO + dual DAC per node
(≈ 5–10/node) *and* perpetual reference airtime; WireLock moves cost to one master and one
coupler per node, and deletes the airtime. A home-scale radio interferometer for the price
of a television still holds (note `r`'s estimate), now without the coherence candle
burning at the frame rate.

### 7.3 Transmit needed?

- **Spine:** none (conducted; radio-silent).
- **Lock-State Protocol:** yes — one round per lock state (a single fanned frame, or one
  two-way exchange per puck pair for transmit-coherent operation). Versus per-epoch under
  escapes 1/2.
- **Sensing:** yes as for any active scheme (ESP-NOW HT40 MCS0 as the CSI excitation, per
  note `07`/synthesis §8); receive-only sensing still works throughout, using household
  traffic, because the puck is internally coherent whether or not the spine is present.

---

## 8. The boundary, stated exactly (the mission's closing clause)

### 8.1 If WireLock works

The quantity that gates the wireless-only home is `Δf_res·T` (phase accrual between
references). The **minimal reference in a dwelling** is:

> **one pre-existing conductor pair + a coupler at each node (phase transport), plus one
> over-the-air phase-calibration round per lock state per component — O(1) airtime, not
> O(N) per epoch.**

The gauge is not crossed by more reference: it is *frozen* — made time-invariant — and then
measured once. Every capability the cohort flags as Track 3 (distributed AoA, near-field
depth, carrier-phase ranging, coherent mapping) becomes continuously available at
commodity cost. Radio silence in the sensing band is the dividend.

### 8.2 If WireLock fails (E0/E1 negative)

Then the boundary stands exactly as notes `p`/`20` state it, with no softening: **without a
trustworthy conducted corridor, cross-node carrier coherence on ESP32-class silicon is
bounded by `Δf_res·T` and can only be bought per epoch (OTA escapes) or per placement (new
cable) — while sharp *relative* geometry and all sensing remain available through the
gauge-invariant route (closure observables and per-puck coherence).** What dies with
WireLock is not sensing; it is the *persistent* home-wide phase standard. The fallback
ladder of §4.2 applies; the closure audit, the near-field puck, and the two-way ruler all
survive in per-epoch form.

---

## 9. Direction-by-direction audit and what is genuinely new

| Mission direction | What this note did with it |
|---|---|
| 1. Wireless coherence on commodity silicon | Audited (ESPARGOS conducted, ESP-PPB wireless-claimed); **not improved**. WireLock replaces the wireless transport with pre-existing copper. |
| 2. Self-calibrating learned array | WireLock makes self-calibration *structural* (lock-state constants + closure audit) rather than learned; learning retains the target-generalisation role of note `19`. |
| 3. Environment as shared reference | Preserved as the cohort's fallback family (q, r, n, o); WireLock removes the burden on it. The mains is a *deterministic* shared reference, unlike clutter. |
| 4. Sharp 3D from many coarse apertures | Not re-invented: home baselines (λ/D ≈ 1°), near-field curvature depth (h), two-way ruler (s), polarisation (below) are *inherited and made persistent*. |
| 5. Ping + unique replies + two-way products | Economic re-composition: the round trip moves from per-epoch calibration to per-lock-state calibration and to the drift-free ruler. |
| 6. Different excitation/composition | This is where the invention lives: a different reference medium, not a different waveform. |

### 9.1 Genuinely new (against all sources found)

1. **Mains-carried carrier-phase reference for radio LO coherence** — no primary found;
   adjacent works use mains for coarse time (SenSys 2009, Feng 2007, IFAC 2015) or carry a
   phase reference over purpose-laid cable (ESPARGOS) or the air (ESP-PPB, AirSync).
2. **The static-gauge lemma as a design principle**: exploit ESPARGOS's measured fact that
   PLL phase re-randomises only at lock events, and extend it to the whole home via
   transported phase: *calibration cadence becomes an event rate, not a coherence time*.
3. **The lock-state calibration protocol + closure-audit event classifier** (the three-way
   signature table in §4/S3) — a specific, testable algorithm.
4. **Common-mode master-quality cancellation** as an architectural licence to use a cheap
   master oscillator.

### 9.2 Credit and positioning versus the cohort

- **`r` (Clutter Clock)**, §8 item 4: the one-sentence mains-tone seed (625 kHz, "up-
  multiplies", graded SPECULATION 1/10, no PLC channel/standards grounding, no lock-state
  economics). WireLock is the promotion of that seed: phase-vs-frequency transport choice,
  PLC physics and legality, lock-state protocol, closure audit, fallback tiers, E0/E1.
  Honest delta: `r` deserves the seed; this note carries it.
- **`h`, `o`, `n` (closure lattice family)**: their invariants are used *as the health
  monitor*, not duplicated as the invention; WireLock addresses the case they decline
  (cross-node absolute phase) with a non-radio reference.
- **`a`, `b`, `g`, `i`, `q`, `s`, `t` (coherence-engineering family)**: WireLock composes
  with their protocols (any of their OTA rounds becomes the lock-state calibrator) and
  removes their cadence burden.
- **`p` (coherence-locality)**: its phase-accrual inequality is the boundary quantity this
  note attacks; its Fiducial route becomes optional under a frozen gauge.

### 9.3 The composable sensor axis not claimed as the headline: the polarimetric shell

Direction 4 lists polarisation; the cohort mentions it only in passing. With the puck's
faces phase-coherent, equipping faces with dual-polarised elements (ESPARGOS One already
ships 8 dual-polarised faces, mixed linear/circular — product page) yields a coherent
**vector** field sampler on a 30 cm sphere: per path, a polarisation state alongside delay
and coarse bearing; cross-pol channels in which specular walls and LoS weaken while human
scatter depolarises (dual-pol propagation is a mature model class — Shafi et al., MERL
TR2006-020; RIS-ISAC dual-pol work treats polarimetric scattering as a target
discriminant). This is a *de-cluttering and generalisation* axis (material-sensitive,
home-independent features), not a coherence escape, and it is left as a composable upgrade
with its own falsifier: dual-pol features must reduce cross-home classifier degradation
versus co-pol-only features on the same sensor. It is deliberately **not** the invention:
it sharpens inference; it does not cross the boundary.

---

## 10. Evidence strength

Per claim (1–10):

| Claim | Grade | Why |
|---|---|---|
| The gauge and its invariants (carried from `20`) | 8 | Formal, multi-source |
| PLL phase re-randomises only at lock events; shared clock gives long-term stability | 8 | Measured on ESP32-S2 (ESPARGOS) |
| Phase transport ⇒ piecewise-constant cross-node phase | 8 | Direct corollary; residual thermal term quantified |
| PLC channels are quasi-static + LPTV(100 Hz) + event-driven; bands/limits/standards | 7 | Mature primary literature + standards |
| Mains HF as a *phase-grade* reference conduit | 4 | Physics supports; no direct measurement for this purpose — **E0** |
| External clock/VCTCXO lock path on ESP32-S3 | 5 | Family-proven; S3-specific unverified — **C3/E1** |
| One OTA round per lock state suffices; closure audit classifies events | 5 | Algebra established; protocol untested — **E1/E2** |
| Cost estimates | 6 | Commodity BOM reasoning |

**Overall: 6/10.** The invention is a composition whose *components* are each established
somewhere and one of whose *load-bearing measurements* (E0) is cheap, instrument-only, and
can kill it cleanly. That asymmetry — high payoff, cheap decisive test — is the reason to
fund E0 immediately.

---

## 11. Chaining, existing models/data, own data

### (A) Chaining

- **PLC analogue front-end ⇒ (S1) ⇒ phase transport ⇒ static gauge ⇒ one-time calibration
  ⇒ persistent coherent aperture ⇒ near-field 3D + two-way ruler + coherent mapping.**
- **Static gauge ⇒ two-way products become drift-free rulers ⇒ self-survey (note `s`) ⇒
  geometry ⇒ map-aided calibration (note `14`/`q`).**
- **Closure audit (S3) ⇐ sensing traffic; it is the independent monitor enabling the whole
  chain to be trusted without external truth.**
- **Mains availability partitions the fleet**: anchors (spine-locked) vs wanderers
  (per-frame OTA). Highest-leverage enabling capability remains compliant TX
  (`esp_wifi_80211_tx`) — now used at `O(1)` airtime rather than `O(N)` per epoch.
- **Blocked without S1's measurement**: nothing else in the cohort's coherent family can
  operate *persistently*; all of it still operates episodically.

### (B) Existing models and data

- **PLC channel**: Zimmermann & Dostert multipath model (transfer functions, linear
  passband phase); Corripio et al. LPTV model; ETSI TR 101 562-1 (MIMO L/N/PE paths);
  Hashmat et al. ISPLC 2010 measurements. Directly reusable for E0 design and for
  predicting notch structure.
- **Mains-as-reference**: SenSys 2009 (Syntonistor); Feng 2007 (LV power-line time
  transfer); IFAC 2015; arXiv:2608.12730 (reciprocal two-way over power conductors).
- **Coherent ESP32**: ESPARGOS datasets + `pyespargos` (phase-coherent CSI, MIT-style open
  licensing on datasets); ESP-PPB (GPL-3.0, code + hardware lessons) — the reference
  platform for C3/C4.
- **Closure/self-calibration**: Jennison 1958; Cornwell & Wilkinson 1981; closure-only
  imaging literature — ready-made algorithms for S3.
- **Dual-pol**: Shafi et al. model; ESPARGOS One product precedent.

### (C) Own data / the minimal experiment

1. **E0 dataset (new, publishable):** multi-home, multi-day, multi-conductor *phase*
   stability of the mains HF channel at reference-candidate frequencies, with event logs.
   No such dataset exists for this purpose; it settles a question of general interest
   (can smart-grid-grade timing be extended to carrier phase?).
2. **E1 dataset:** first cross-node 2.4 GHz phase-drift measurement on ESP32-S3 against a
   conducted ground truth with an active spine, versus the wireless-only control.
3. **Falsification is clean:** E0's "continuous wander" outcome falsifies the spine's
   premise with instruments costing far less than one puck.

---

## 12. Residual unknowns

1. Mains S21 phase-stability interval and event statistics in real homes (E0; no primary).
2. Mains-synchronous LPTV phase-modulation depth at reference-candidate frequencies (E0).
3. Whether ESP32-S3 accepts external clock on XTAL_P in silicon/firmware, or whether the
   VCTCXO-locked fallback is required (bench, C3).
4. Differential thermal wander of S3 RF PLLs under a *common* 40 MHz (E1; ESP-PPB's 1 ppb
   is adjacent, on C3, timestamp-domain).
5. Cross-breaker/cross-phase HF coupling topology in Australian single-phase-per-home vs
   multi-phase properties (E0 scope).
6. Conducted-emission compliance margin for a narrowband reference tone at chosen PSD
   (compliance review with a certified front-end).
7. Interaction with existing PLC modems in the home (a modulation source; also a
   certification precedent).
8. Security/integrity of the spine (a physical master can be removed or spoofed; the
   closure audit detects phase inconsistency but not who caused it).
9. Long-run (months) stability of coupler components under mains transients.

---

## 13. Primary sources

**ESP32 coherence and clocks**

- ESPARGOS: D. Uvaydov et al., "ESPARGOS: An Ultra Low-Cost, Realtime-Capable Multi-Antenna
  WiFi Channel Sounder", arXiv:2502.09405 — PLL initial-phase re-randomisation at
  reset/channel change; shared 40 MHz sufficient for long-term phase stability; reference
  network purpose. https://arxiv.org/abs/2502.09405
- "ESPARGOS: Phase-Coherent WiFi CSI Datasets", arXiv:2408.16377 — shared 40 MHz clock +
  ~2.4 GHz phase reference, frequency-multiplexed on one coax.
  https://arxiv.org/abs/2408.16377
- ESPARGOS One product page (dual-polarised faces; reference in/out; PoE).
  https://espargos.net/espargos-one
- J. Muller, "ESP-PPB: 1-PPB wireless sync for phase coherent Wi-Fi CSI", GPL-3.0,
  GitHub 2026 — external oscillator on XTAL_P; jumper-wire clock sharing signal-integrity
  lesson; VCTCXO + dual-DAC discipline; 1 ppb thermal wander on one PCB; ESP-NOW 100 µs
  burst tool; CFO-matching corruption mechanism. https://github.com/jonathanmuller/esp-ppb
- Espressif, ESP32-S3 Hardware Design Guidelines §1.3.5 (clock sources) and ESP-IDF Clock
  Tree (S3 root clocks). https://docs.espressif.com/projects/esp-hardware-design-guidelines/
- Note `17` (time/frequency precision), `12` (oscillator drift), `20` (boundary) — internal
  corpus.

**Mains as a channel and as a reference**

- M. Zimmermann, K. Dostert, "A multipath model for the powerline channel", IEEE Trans.
  Commun. 50(4):553–559, 2002. CorpusID 26273828.
- F. J. Corripio, J. A. Cortés, L. Díez, J. T. Entrambasaguas, "Analysis of the cyclic
  short-term variation of indoor power line channels", IEEE JSAC 24(7):1327–1338, 2006
  (LPTV at 50/100 Hz). https://doi.org/10.1109/JSAC.2006.874417
- R. Hashmat, P. Pagani, A. Zeddam, T. Chonavel, "MIMO communications for inhome PLC
  networks: Measurements and results up to 100 MHz", IEEE ISPLC 2010.
- ETSI TR 101 562-1 V1.3.1, "PowerLine Telecommunications (PLT); MIMO PLT; Part 1"
  (L/N/PE paths; up to 8). https://www.etsi.org/deliver/etsi_tr/101500_101599/10156201/
- L. Yonge et al., "An Overview of the HomePlug AV2 Technology", J. Electrical and Computer
  Engineering, 2013 — 1.8–86 MHz; PSD practice −50/−80 dBm/Hz. DOI 10.1155/2013/892628.
- A. Rowe, V. Gupta, R. Rajkumar, "Low-power Clock Synchronization using Electromagnetic
  Energy Radiating from AC Power Lines", ACM SenSys 2009, best paper — grid phase
  coherence; <1 ms over 11 days, 8 nodes, building scale. DOI 10.1145/1644038.1644060.
- P. Feng, G. Wu, G. Li, Y. Bai, "Study of a new time transfer method of low voltage power
  line", IEEE Int. Frequency Control Symp. / EFTF 2007, pp. 823–826.
- "Time Synchronization in Power-line Communication", IFAC-PapersOnLine 2015 — zero-crossing
  sync, ms level. DOI 10.1016/j.ifacol.2015.12.031.
- "Time Distribution Principle Using Measured Traveling Waves in Power Grid",
  arXiv:2608.12730 (2026) — reciprocal two-way timing over power conductors.
- 47 CFR §15.3(gg) (In-House BPL), §15.107, §15.207 — conducted limits.
  https://www.ecfr.gov/current/title-47/part-15
- ACMA, "ACMA-mandated EMC standards" — EN 50065-1 and the in-home PLT apparatus standard.
  https://www.acma.gov.au/technical-standards
- J. E. Hardis, D. Matsakis, B. Fonville, "Time and frequency from electrical power lines",
  PTTI 2017 (NIST) — the 50/60 Hz mains as a (coarse) time/frequency reference and its
  TEC caveats. https://tf.nist.gov/general/pdf/2895.pdf

**Closure / gauge / calibration (composed, not re-derived)**

- R. C. Jennison, "A phase sensitive interferometer technique…", MNRAS 118:276, 1958
  (closure phase).
- T. J. Cornwell, P. N. Wilkinson, "A new method for making maps with unstable radio
  interferometers", MNRAS 196:1067, 1981 (self-calibration).
- Notes `h` (Closure Weave), `o` (Closure Lattice), `n` (Gauge Lattice), `p`
  (coherence-locality), `r` (Clutter Clock), `s` (Reciprocal Gauge Mesh), `b` (Lighthouse),
  `t` (Coherent Star) — internal corpus.

**Dual-polarisation (composable axis)**

- M. Shafi et al., "Polarized MIMO Channels in 3-D: Models, Measurements and Mutual
  Information", MERL TR2006-020 — cross-pol propagation model class.
- RIS-ISAC dual-polarised channel work, arXiv:2501.00909 — polarimetric scattering as a
  target discriminant.

---

## 14. One-line answer

The invention is **WireLock**: inject one phase-coherent tone into the home's *existing*
mains wiring, **transport its phase** into every node's clock, and the cross-node `U(1)`
gauge stops being a per-epoch nuisance and becomes a *lock-state constant* — one
over-the-air calibration round and a closure audit per state, radio-silent in between, so
that many cheap ESP32-S3 pucks plus their conducted-coherent polyhedra behave as one
persistent home-scale coherent 3D aperture; the decisive measurement is E0, the mains
channel's phase-stability interval, and if it fails the boundary stands exactly as
`Δf_res·T` with the cohort's per-epoch and invariant-only fallbacks.
