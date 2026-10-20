# 22 — Invention A: The Coherent Weave

### A wireless-locked 3D sensing aperture woven from ESP32-S3 polyhedra — the invention, its physical mechanism, and the single experiment that decides it

Status: invention proposal + boundary analysis. Australian English. Grades as in the v2
notes: **ESTABLISHED / HYPOTHESIS / SPECULATION**. Cross-references to notes `01`–`20`
of this folder (`00-synthesis.md` integrates them).

---

## 0. Mission and method

> Discover the new invention that makes a home full of cheap ESP32-S3 nodes behave as one
> sharp, self-calibrating, generalising 3D sensor — or prove no such invention exists and
> state the exact physical boundary that blocks it.

Method: Parallel web search over six directions (wireless coherence; self-calibrating
learned arrays; environment-as-reference; sharp-3D from coarse apertures; ping/reply
protocols; new excitation/composition), then parallel extraction from primaries
(arXiv/IEEE/GitHub/datasheets). Findings are graded and cited. Two facts discovered in
this sweep are newer than the v2 note set and change the picture:

1. **ESP-PPB (2026, GPL-3.0)** — an open-source ESP32-C3 platform claiming the first
   *wireless*, phase-coherent CSI capture across nodes: FTM-steered VCTCXO discipline
   (~1 PPB) plus a master "reference frame" 1 ms after every external CSI frame;
   demonstrated multi-node phase stability and AoA [14]. Nothing about it is published in
   a venue yet; treat the *claim* as grey literature, the *mechanism* as sound.
2. **WiCAL (2025)** — switched-array, three-stage phase alignment across distributed
   3×4 URAs on COTS Intel 5300s, forming a "virtual large-scale array" for 3D direct
   position determination: 1°/1.5° median intra-array AoA and **15.6 cm median 3D error
   with two arrays** [16]. This is prior art for *cabled* coherent distributed apertures
   on commodity Wi-Fi.

**Verdict.** The invention exists; call it the **Coherent Weave**. It is a composition of
five primitives (Knot, Fence, Round, Mesh, Loom) whose parts are each established
somewhere, whose *combination* has no precedent, and whose one unmeasured physical
quantity — the residual cross-node carrier phase after CFO removal across a
MAC-timed exchange-and-holdover interval, "M2′" — decides whether the home is **one
coherent aperture** or **a constellation of small ones**. The decisive experiment is E1
(§6): two S3 nodes, no cable between them, one number, pass/fail thresholds stated.

---

## 1. The physics ledger

### 1.1 The gauge (ESTABLISHED — note `20`, with primaries below)

With independent local oscillators, a CSI observation takes the form
`ĥ_ij(f_k) = g_ij·a_i·b_j·e^{j(θ_i−ψ_j)}·e^{−j2π f_k τ}`. Only `θ_i−ψ_j` — the *difference*
between node `i`'s receive-LO phase and node `j`'s transmit-LO phase — is observable.
Across a connected measurement graph the node phases are fixed only up to one global
additive constant: a `U(1)` gauge (plus time and frequency gauges). **Invariants**:
amplitudes; per-link subcarrier differential phase (delay/ToF); per-receiver inter-antenna
differential phase (AoA *inside* one coherent receiver); **two-way products** `ĥ_AB·ĥ_BA`
(LO cancels, hardware asymmetry survives, **propagation delay survives**); temporal
differentials (Doppler). **Not observable**: absolute cross-node phase, phase-of-flight
range, cross-node AoA. One extra constraint per connected component fixes the gauge;
minimal references are `O(N)` [20; 1,2].

### 1.2 The four crossings (three ESTABLISHED + one new)

| Reference | Mechanism | Demonstrated where |
| --- | --- | --- |
| Conducted clock + **WiFi-based phase reference** | 40 MHz alone is insufficient: the RF PLL's initial phase re-randomises per reset and per channel. A conducted *frame* from a shared generator measures each receiver's PLL/hardware phase and a division `ĥ_CAL = ĥ_OTA/ĥ_REF` restores coherence | ESPARGOS, ESP32-S2/C61 [11,12,13] |
| Common transmitter | A master broadcasts a reference; receivers track its phase and de-rotate; AirSync: "phase coherence within a few degrees" | WARP FPGA testbed [8]; theory [1,2] |
| Reciprocity / two-way products | Time-slotted round-trip carrier synchronisation; two-way sum-CP cancels LO and hardware impairment; per-link gauge-free | Theory [1], WARP [8], **COTS WiFi (WiDRa)** [15] |
| **Wireless reference-frame anchoring** *(the new one)* | FTM-disciplined TCXO (~1 PPB) + a reference frame 1 ms after each data frame; per-frame phase anchoring gives stable cross-node offsets | **ESP32-C3 (ESP-PPB, 2026, unrefereed)** [14] |

### 1.3 What a two-way exchange gives (ESTABLISHED, [15], and note `20`)

For a reciprocal channel, the **sum-carrier-phase** of an uplink/downlink pair —
"WiDRa" measures exactly this on unmodified COTS 5 GHz hardware — cancels both LO initial
phases and CFO, leaving a phase that scales with **range** at the carrier frequency, not
the bandwidth: measured differential-range RMSE **0.2 mm (50 ms gap) to 0.9 mm (5 s gap)**
in LoS, with per-0.5 s piece-wise CFO fits and a nested frame schedule [15]. The same
product leaves a **hardware-asymmetry term** (the per-node TX/RX chain ratio), which is
stable and separately calibratable by `O(N)` pairwise exchanges (TDD reciprocity
calibration, ESTABLISHED in distributed MIMO) [10].

### 1.4 The one unmeasured quantity — M2′

The v2 synthesis identifies M2 (reciprocity residual after CFO correction) as the gate.
The 2026 sweep sharpens it. Because COTS/ESP32 receivers rotate the **common phase per
frame** (PLL re-lock; documented on COTS as `2πn/N` rotations [15]; reproduced on ESP32
as "initial phase different for every time index" [11]), coherence is not a fixed
calibration but a **tracked** quantity:

> **M2′ := the residual cross-node carrier-phase error after two-way CFO removal,
> measured across a data-to-reference-frame gap `T_gap` and a holdover interval; units
> rad; the sensor is coherent iff `σ_φ(M2′) ≲ 0.5 rad` at the working cadence.**

Elementary coherent-gain arithmetic sets the threshold: for Gaussian phase error,
`E[cos φ] = e^{−σ²/2}`; `σ = 0.5 rad` (≈ 2 cm of path at 2.44 GHz) retains 88 % coherent
gain, `σ = 1 rad` retains 61 %. No published number exists for ESP32-S3. ESP-PPB's
`< 1° best / < 10° typical` single-frame anchoring on ESP32-C3 [14] is the only adjacent
number, and it is consistent with success.

### 1.5 ESP32-S3 facts that shape the design (ESTABLISHED)

- FTM (802.11mc): S3 is initiator (STA) **and responder (AP)**; t1–t4 carry picosecond
  fields; achieved accuracy is metre-class [17; ESP-IDF].
- `esp_wifi_80211_tx()` sends beacon/probe/action/non-QoS data frames (no encrypted/QoS);
  raw TX defaults to 1 Mbps 11b (no OFDM preamble → no CSI); **the CSI "ping" must be an
  ESP-NOW HT40 MCS0 frame**, as esp-csi uses [07; ESP-IDF].
- The MAC exposes a **nanosecond RX timestamp** (latched on an 80 MHz / 12.5 ns grid);
  on ESP32 it needs a small IDF patch (ESP-PPB ships one: `hack_struct.patch`) [14,17].
- There is **no TX hardware timestamp**; TX instants must be inferred (reply ordering,
  RX stamps of other nodes, FTM).
- Raw IQ is now available on the **S3** too: continuous decimated I/Q over USB
  (15.625–250 kSa/s) and 80 MSa/s bursts with an FPGA, phase-coherent across the ESPARGOS
  array; ESPARGOS One states **phase-coherent transmission is possible** (not released)
  [13]. 5 GHz exists on the in-family C5.

---

## 2. State of the art, 2026 — what already exists (and what does not)

| Piece | State | Source |
| --- | --- | --- |
| Conducted coherent ESP32 array (2.4 GHz, 8 ch) | Demonstrated (S2/C61); coherent TX possible | [11–13] |
| Wireless coherent CSI across nodes | Claimed on C3 (unrefereed, GPL-3.0); no sensing results | [14] |
| Wireless-coherent distributed *arrays* for radar | SDRs only: wireless syntonization + internode ranging; bistatic wall std 1.7 cm, bias 14.9 cm @4.8 GHz | [5,6,7] |
| Coherent distributed aperture on COTS WiFi | **Cabled** inter-array sync; virtual large array; 15.6 cm 3D median | [16] |
| Two-way carrier-phase ranging on COTS WiFi | mm-level *differential* range, LoS; single antennas; direction-finding left to future work | [15] |
| LoS path as over-the-air delay+phase reference | Single Tx–Rx link, 802.11ax COTS; sub-wavelength respiration | [17bis=LoSRef] |
| Round-trip carrier sync protocol | Theory: two-source distributed beamforming; resync/overhead trade | [1] |
| Master-pilot phase tracking across APs | WARP FPGA; few-degrees coherence | [8] |
| TDD reciprocity calibration (hardware asymmetry) | Distributed MIMO theory + testbeds; `O(N)` pairwise exchanges | [10] |
| Virtual anchors / multipath SLAM | UWB/5G: Channel-SLAM, DC-SLAM, soft-coherent MP-SLAM | [19,20,21] |
| Distributed coherent aperture radar (DCAR) | Defence/SDR: cohere-on-receive, dominant-scatterer calibration, stepped-frequency | [22,23,24] |
| Spherical/geodesic arrays for full-sphere DoA | Established theory + hardware (e.g. geodesic dome satcom) | [25,26,27] |
| Channel charting in real-world coordinates | Bilateration loss with only AP positions; no accurate AP sync needed | [28] |
| RF fingerprinting from CSI/hardware impairments | >90 % device ID from 100 packets; CSI²Q; I/Q-imbalance surveys | [29,30,31] |
| A polyhedral ESP32 sensing array; a wireless coherent *sensing* aperture | **Does not exist** | [08,09,16; 00] |

---

## 3. THE INVENTION — The Coherent Weave

One sentence: **weave many cheap, face-rich ESP32-S3 polyhedra into a single phase-coherent
3D aperture by (a) making each polyhedron internally coherent by conduction, (b) locking
polyhedra to one another with a wireless reference frame plus a periodic two-way "round",
(c) fixing the gauge with two-way sum-CP geometry and the home's own static reflections,
and (d) turning the result into a radar-grade aperture by algebra — not by new silicon.**

Terminology: a **knot** is a polyhedron puck; the **fence** is the wireless coherence
loop; a **round** is the scheduled exchange that fixes space, time and phase; the **loom**
is the static environment's multipath; the **fabric** is the resulting coherent home
aperture.

Each invention follows as **New / Mechanism / Must be true / Falsifier / S3 feasibility /
Cost / TX needed**.

---

### I1 — **Knot: the polyhedral coherent aperture** (20 faces, one clock, one conducted reference)

- **New.** No ESP32-class volumetric (3D) sensing array exists; no commodity 2.4 GHz
  polyhedron exists [00 §7]. The Knot is a 20-element *switched coherent 3D transceiver*,
  not a receiver.
- **Mechanism.** (i) 40 MHz fan-out from the hub locks every face's *frequency*. (ii) The
  hub face transmits an ordinary WiFi frame down a resistive-divider/microstrip network
  to every face's antenna port through an RF switch — ESPARGOS's exact architecture
  [11,12]. Each face computes `ĥ_REF`, dividing out its per-boot/per-channel PLL phase and
  RX chain: **one conducted frame per boot (and per channel switch) buys full internal
  coherence.** The 40 MHz alone does not (PLL phase re-randomises) — this must be stated
  loudly because the hardware premise lists both. (iii) Faces are switched, not
  simultaneous: a 3D switched array with redundant samplings for phase alignment, as in
  WiCAL's three-stage scheme [16]. (iv) **Coherent TX**: in TDD the TX phase rides the
  same LO that the reference frame measures; per-face TX-chain constants are fixed by
  `O(20²)` internal round-trip exchanges at boot (each face transmits, the others and the
  hub receive; solve the reciprocity-calibration equations [10]). The hub may also use the
  conducted network itself for a direct TX/RX phase matrix.
- **Must be true.** Conducted reference frames received by all faces; mutual coupling and
  face shadowing tolerable; switched-array alignment works in 3D as it does in 2D [16].
- **Falsifier.** E2 (§6): 3D DoA RMSE of a cooperative transmitter swept over the sphere
  `> 20°` (no better than a single face's half-space amplitude discrimination) or
  swing depending on face pattern.
- **S3 feasibility.** Demonstrated in pieces on the ESP32 family: CSI capture, conducted
  reference [11], switch arrays [16], per-boot calibration stability over hours [10,12].
  S3-specific CSI quality is the only local unknown; esp-csi covers S3 [07,32].
- **Cost.** 20× S3 modules (~$70) + 20 triangular PCBs + hub/aggregator + divider network
  + 20 RF switches + fan-out ≈ **$250–400/puck** at small volume; less at scale.
- **TX needed?** Not for the aperture per se; **yes** for coherent illumination, for the
  Round, and for the internal TX calibration.

### I2 — **Fence: the wireless phase-coherence loop**

- **New.** Wireless-only, phase-coherent *sensing* across independently clocked commodity
  nodes has no precedent; ESP-PPB is a sync platform without sensing and unrefereed [14];
  everything else coherent is cabled [11,16,22] or SDR-grade [5,6].
- **Mechanism.** Generalise ESP-PPB: (a) one home clock master (a Knot, or the home's own
  AP for zero added hardware) emits **reference frames** continuously (or after each
  scheduled data frame); (b) every node discipline a VCTCXO by FTM against the master
  (~1 PPB, ESP-PPB), *or* skip the TCXO and estimate relative CFO from the reference
  stream itself; (c) every CSI frame is de-rotated by its adjacent reference-frame phase,
  tracked with an unwrapping Kalman/α-β loop keyed to the **nanosecond RX timestamps**
  [14,15]; (d) periodic two-way exchanges (the Round, I3) re-anchor phase and fix the
  per-node constant offsets. The Knot is the ideal clock-discipline site: one VCTCXO plus
  I²C DAC per puck drives all 20 faces.
- **Must be true.** M2′ ≤ 0.5 rad at the working cadence (E1); reference frames within the
  coherence budget of the data frames; MAC can sustain the schedule.
- **Falsifier.** E1 fails: cross-node anchored phase is a random walk (unbounded) or
  `> 1.5 rad` at a 20 ms gap. Then no wireless-only home aperture exists on this MAC class.
- **S3 feasibility.** ESP-PPB runs this on **ESP32-C3** (weaker chip) at PPB level; S3
  adds the ns timestamp (with the same IDF patch) and HT40. Feasible with firmware work,
  not new silicon.
- **Cost.** ~$5/node (0.5 ppm VCTCXO + I²C DACs) + firmware; zero if the home AP is the
  master (receive-only fence).
- **TX needed?** Yes for the master/reference stream unless an external AP serves; the
  slaves need only RX + occasional TX for rounds.

### I3 — **Round: scheduled ping + unique replies + two-way products (the "Coherent Round")**

- **New.** Brown & Poor synchronise two *transmitters* [1]; WiDRa does a two-party
  sum-CP exchange [15]; ESP-PPB ping-pongs per frame [14]. A **fleet-wide TDM round whose
  two-way products form a gauge-fixed channel-graph snapshot** — simultaneously measuring
  CFO, phase, range, hardware asymmetry and mobility on every edge — is new.
- **Mechanism.** One "round" = a scheduled TDM sequence: the hub (or each Knot in
  round-robin) pings; every other Knot answers in an assigned slot with a uniquely
  addressed ESP-NOW HT40 MCS0 frame (uniqueness = identity + per-sender scrambling of the
  pilot subcarriers to survive slot collisions); every node is promiscuous and stores CSI
  for *all* frames. The aggregator (hub FPGA/switch or host) then knows, for every ordered
  pair, `ĥ_ij` on all 64/128 subcarriers. For each pair it forms the **two-way product**:
  cancels LO and CFO (first order), leaving range phase (sum-CP) + stable asymmetry
  fingerprint + Doppler across rounds. Rounds run at 20–100 Hz (feasible: a round of 8–20
  short frames ≈ 1–5 ms at 1–6 Mbps raw TX; `17 ms` cadence is the note-`12` optimum).
  A round is both a *measurement* and a *sync event*: its two-way products reset the
  fence's phase estimate and its RX timestamps (12.5 ns grid) bracket the unknown TX
  instants (no TX timestamp exists — use piggybacked RX observations instead of trusting
  the sender).
- **Must be true.** TDM sufficiency (note `09`: mandatory on S3); CSI yield from ESP-NOW
  HT40 frames; MAC turnaround (0.5–3 ms) inside M2′.
- **Falsifier.** Rounds cannot run faster than the phase drift (E1's drift-rate
  measurement); or the two-way sum-CP on S3 proves non-repeatable to `< 1 cm` against a
  tape-measured rig.
- **S3 feasibility.** All pieces are S3 APIs (ESP-NOW, raw TX, promiscuous CSI, ns
  stamps); aggregator on host. Feasible.
- **Cost.** Nil beyond firmware.
- **TX needed?** Yes — the Round is an active protocol. (One may fall back to
  passive-only mode where the Round is rare and the fence tracks the home AP's traffic.)

### I4 — **Mesh: two-way sum-CP across a coherent 3D face grid → geometry without anchors**

- **New.** WiDRa's authors explicitly leave "direction finding and triangulation… with
  single antenna STAs" to future work [15]. Applying sum-CP over the **20×20 face-pair
  grid of two Knots** (400 coherent two-way links sharing two clocks) makes each
  Knot-pair a *self-measuring, self-orienting* unit: **cm-class range + relative bearing +
  relative CFO + asymmetry**, with the 3D face geometry supplying the angular resolution
  and the 64–128 subcarriers supplying the coarse delay ruler.
- **Mechanism.** Each face pair `(m,n)` gives a sum-CP `Ψ_mn` whose phase slope over
  subcarriers yields the two-way delay (multipath-biased but now *separable*: the 3D
  aperture resolves incoming directions, so the LoS/dominant component can be gated
  angularly — the "dominant-scatterer" trick of coherent distributed radar [23,24]
  applied in 3D); the phase *gradient across faces* yields each Knot's bearing to the
  other (a virtual large-scale array formed from two small ones, cf. WiCAL [16] but
  wireless); the amplitude/phase **asymmetry term** is a stable hardware fingerprint [10,
  29–31].
- **Must be true.** Sum-CP extractable from S3 CSI (COTS precedent [15]); per-pair
  multipath separable at 20–40 MHz with a 30 cm 3D aperture (the hard part).
- **Falsifier.** Inter-Knot range repeatability `> 5 cm` or bearing RMSE `> 10°` against a
  total-station truth, after angular gating.
- **S3 feasibility.** Heavy processing on host; S3 provides CSI + timestamps. WiDRa
  already does the math on COTS chips [15].
- **Cost.** Nil.
- **TX needed?** Yes (two-way). A passive variant uses a cooperative illuminator.

**Consequence (the self-calibration):** a graph of Knots with per-edge
`{range, bearing, bearing}` edges is a pose graph solvable by MDS + Procrustes **without
any external anchor** — the array finds its own geometry by conversation. Channel
charting's bilateration loss (only AP positions needed; no accurate sync) provides the
learned, weakly supervised complement when edges are missing [28]; FTM (metre-class)
provides the coarse prior [17].

### I5 — **Loom: the environment as the shared reference constellation**

- **New.** LoSRef (2026) uses *one link's* LoS path as an over-the-air phase/delay
  reference on COTS 802.11ax [18]; DCAR uses *one dominant scatterer* to cohere distributed
  apertures [23]; virtual anchors are established in UWB/5G SLAM [19,20,21]. **Turning the
  home's static specular field into the fleet's joint phase/geometry reference — a
  distributed "virtual common transmitter" constellation — and using it to calibrate the
  aperture itself is new.**
- **Mechanism.** All Knots hear the same illuminator (a Knot, or the home AP). Each
  static specular path `T→S→i` is a *coherent secondary source* at the virtual anchor `S`
  for every receiver: two Knots' phases for that component differ by
  `(φ_i − φ_j) + 2πf_c(τ_Si − τ_Sj)`. The geometry term is computable once `S` and the
  Knots' poses are crudely known (from I4/FTM), so the *phase differences themselves
  estimate the residual LO gauges*; conversely, with gauges tracked (I2/I3), the same
  equations refine `S` and the poses. This is exactly MP-SLAM's estimation with virtual
  anchors [19,20,21, 17bis], but the roles are doubled: **the static world is both the map
  and the phase reference**, which is why it holds across homes — a home-independent
  geometric object, not a learned fingerprint.
- **Must be true.** Specular components at 2.4/5 GHz in furnished rooms remain stable and
  associable across Knots and time (LoSRef shows the LoS path is stable [18]; walls should
  be; furniture is questionable).
- **Falsifier.** Across a 30-minute run, VA positions jitter `> 30 cm` or data association
  fails `> 20 %` of rounds.
- **S3 feasibility.** CSI only; algorithm host-side.
- **Cost.** Nil.
- **TX needed?** No, if an ambient illuminator (the home AP) is used; yes for controlled
  illumination.

### I6 — **Fabric: the sensing algebra of the Weave (what the sensor finally *is*)**

Once I1–I5 hold, the following are *algebra*, not new hardware:

1. **Coherent receive aperture**: cross-Knot MUSIC/direct-position-determination on the
   de-rotated CSI graph (WiCAL's DPD [16], now wireless) — angular resolution set by
   inter-Knot baselines (metres), not by the 30 cm knot.
2. **Clutter subtraction**: with phase coherence, the static channel is a *complex*
   quantity; average and subtract it across rounds (`S` minutes), leaving target-induced
   differences at sub-wavelength sensitivity — LoSRef demonstrated sub-wavelength
   respiration on one link [18]; the Weave does it across an aperture.
3. **Multistatic ranging/Doppler**: per-edge two-way delay (I4) and per-edge Doppler
   (temporal phase ratio, established) give target range/velocity *fields*; coherent TX
   within a Knot (`Knot` (iv)) adds a steerable illumination beam and coherent transmit
   gain (N² SNR vs N in coherent-on-receive [22]).
4. **Moving-puck synthetic aperture**: a carried Knot with a locked fence is a
   metre-scale SAR — WiDRa's virtual-array remark [15], deliberately exploited.
5. **Installation awareness (SPECULATION)**: the two-way asymmetry fingerprint (I4/I6)
   shifts when a face's antenna is detuned (hand, furniture, wall contact) — the Weave can
   sense its own embedding.

---

## 4. What is genuinely new (the claim, precisely)

- **W1.** A wireless-only, phase-coherent *sensing aperture* built from commodity
  ESP32-class nodes. (ESP-PPB: sync only [14]; WiCAL: cabled [16]; LoSRef: single link
  [18]; Nanzer/Schlegel: SDRs [5,6,7]; DCAR: defence/SDR [22–24].)
- **W2.** Two-way sum-CP over a coherent **3D face grid**, yielding range + bearing +
  rate + asymmetry per Knot-pair (WiDRa explicitly defers direction-finding [15]).
- **W3.** The **polyhedral coherent transceiver** itself (no ESP32 volumetric array
  exists [00]); switched-array alignment [16] raised to 3D with conducted per-boot
  calibration [11].
- **W4.** The **Coherent Round** as a sensing primitive: one scheduled fleet exchange whose
  two-way products form a gauge-fixed, time-stamped channel-graph snapshot (extends [1]
  from two transmitters to a sensor; [15] from a link to a mesh).
- **W5.** The **environment as phase gauge**: a virtual-anchor constellation that keeps
  the aperture calibrated across homes by geometry rather than fingerprint (extends [18]
  from one LoS path to the fleet; [19–21] from UE-localisation to array calibration).

**Assembled, not new** (state for honesty): the U(1) gauge; two-way products; master-pilot
tracking [8]; FTM+TCXO discipline [14]; sum-CP ranging [15]; switched-array phase
alignment [16]; virtual anchors [19–21]; reciprocity calibration [10]; DCAR
cohere-on-receive [22–24]; spherical-array DoA [25–27].

---

## 5. The assumption ledger (what must be true, and its falsifier)

| # | Assumption | Grade | Falsifier |
| --- | --- | --- | --- |
| A1 | M2′ ≤ 0.5 rad at the working cadence (20 ms holdover) | HYPOTHESIS | E1 |
| A2 | Per-frame common-phase anchoring works on S3 CSI as on C3 | HYPOTHESIS | E1 |
| A3 | Sum-CP is extractable from S3 CSI after impairment stripping | HYPOTHESIS | E1(ii) |
| A4 | Conducted reference restores S3 intra-puck phase per boot | ESTABLISHED pattern (S2/C61), unverified on S3 | E2 boot-loop stability |
| A5 | 20-face 3D switched array reaches ≤ 15° DoA over the sphere | HYPOTHESIS | E2 |
| A6 | Static specular components are stable/associable across Knots | HYPOTHESIS | E3 (VA jitter) |
| A7 | Rounds sustain ≥ 20 Hz with ≥ 8 Knots under MAC limits | HYPOTHESIS | E3 |
| A8 | Mutual coupling/shadowing in a solid polyhedron is tolerable | UNMEASURED [00 §9] | E2 pattern measurement |

---

## 6. Experiments

### E1 — THE DECISIVE EXPERIMENT: the Fence test (two S3 nodes, **no cable between them**)

Rig: two ESP32-S3 boards 3–6 m apart, each single-antenna; **no RF connection between
them**; each is otherwise minimal (no polyhedron needed). A *separate*, algorithm-blind
oracle (scope/FPGA listening on a coax splitter to both nodes' conducted test ports)
records the true relative phase — used only to score the algorithm, never as input.

Procedure:
(i) Master node emits reference frames at 200–1000 Hz; slave node emits scheduled
ESP-NOW HT40 MCS0 data frames; both capture CSI of everything (fence, ESP-PPB pattern
[14]).
(ii) Implement WiDRa-style two-way sum-CP with piece-wise CFO removal [15]; extract the
cross-node residual `σ_φ` at data–reference gaps `T_gap ∈ {1, 5, 20, 100} ms`.
(iii) Repeat for 30 min, including a ≥ 10 °C temperature step.

Metrics: `σ_φ(T_gap)`; drift character (white/ramp/walk); sum-CP range vs tape (bias +
repeatability); λ/4-step reflector tracking.

**Pass: `σ_φ(20 ms) ≤ 0.5 rad` and range repeatability ≤ 1 cm → the invention stands;
scale to E2/E3.**
**Fail: `σ_φ(20 ms) > 1.5 rad` or unbounded walk → §7 boundary holds; fall back to the
non-coherent constellation.**

Cost: ~$100 of parts, one firmware cycle. This is the single most decisive next
experiment of the whole programme: it gates every coherent method in the v2 map (already
flagged as M2 in `00-synthesis.md` §6, item 1; this design gives it a protocol, an oracle
and thresholds).

### E2 — The Knot (polyhedron DoA)
One 20-face Knot; hub reference calibration at each boot; cooperative transmitter swept
over ≥ 30 directions; measure 3D DoA RMSE, gain vs planar 4-element, pattern stability
across boots, mutual-coupling signature. Pass: ≤ 15° RMSE, no bearing-dependent blindness.

### E3 — The mesh (self-calibration)
Six Knots at unknown positions/orientations; total station truth; run Rounds; build the
pose graph from I4 edges + I5 virtual anchors; move a volunteer through the space; measure
(a) pose-graph geometry error, (b) VA jitter/association rate, (c) 3D target track error.
Pass: Knot poses ≤ 20 cm / ≤ 5°, target ≤ 30 cm at 95 %, across two different rooms with
no retuning (the generalisation claim).

---

## 7. If the boundary holds — the exact quantity and the minimal reference

Nothing in §1.1 is violated by the Weave; the gate is a *measurement*, not a theorem. If
E1 fails at any practical cadence, the **named quantity** is:

> **M2′ — the per-exchange residual phase after CFO removal across the MAC-imposed
> turnaround/holdover** (equivalently: the commodity MAC's time-transfer precision under
> PLL re-lock, ∝ residual-CFO × T_gap + per-frame phase noise).

Then the exact statement is: *a wireless-only, home-wide coherent aperture on
ESP32-class silicon is impossible*, and the **minimal reference that crosses the boundary
is one conducted reference path per pair or per hub (O(N) references)** — i.e. the
premise's hub already contains it: the same 40 MHz + WiFi-frame pair that works inside a
Knot, extended to inter-Knot baselines by coax/leaky feeder, restores coherence at a cost
of `O(N)` cable runs — which the mission's "no house-wide cabling" premise forbids. The
**fallback sensor** (still an invention-scale result, but coarse): the constellation —
per-Knot 3D DoA (I1, conducted, needs no M2′), FTM geometry, channel-charting and VA
landmarks — is a generalising 3D sensor at ≈ 0.3–1 m resolution, not centimetres. That is
the honest degraded mode.

---

## 8. ESP32-S3 feasibility, cost, TX — summary

| Primitive | Demonstrated on ESP32 family? | On other COTS? | SDR-only? | TX needed |
| --- | --- | --- | --- | --- |
| Knot (conducted coherent 3D array) | Yes in parts (S2/C61 conducted ref; C3/C5 CSI) — **not on S3, not in 3D** | 2D switched arrays (Intel 5300) [16] | — | optional |
| Fence (wireless phase lock) | **C3 (unrefereed)** [14]; S3 unverified | WARP FPGA [8] | — | yes (master) |
| Round (two-way mesh) | APIs exist on S3 [ESP-IDF]; untested | WiDRa on COTS 5 GHz [15] | — | yes |
| Mesh (sum-CP geometry) | Math on COTS [15]; S3 unverified | [15,16] | — | yes |
| Loom (virtual anchors as gauge) | No | No (single-link LoS only [18]) | 5G/UWB MP-SLAM [19–21] | no (ambient) |
| Fabric (coherent distributed aperture) | No | Cabled DPD [16] | Distributed radar [5–7,22–24] | depends |

Cost recap: **Knot ≈ $250–400**; **fence ≈ $5/node** (or $0 with the home AP as master);
host aggregation assumed. A 6–8 Knot home ≈ $2–3.5k.

---

## 9. Chaining, existing assets, own-data (per the shared brief)

**(A) Chaining.** Conducted reference ⇒ intra-Knot coherence ⇒ 3D DoA (no M2′ needed) ⇒
Knot geometry prior. Fence+Round ⇒ M2′ answer ⇒ inter-Knot coherence ⇒ Mesh ⇒ pose graph
⇒ Loom ⇒ home aperture ⇒ Fabric ⇒ radar-grade 3D. Highest-leverage enabler: **E1's fence
result** — it unlocks I2/I3/I4/I6 in one stroke. A second chain: Knot coherent TX ⇒
steerable illumination ⇒ SNR and clutter margin for Fabric; blocked without I1(iv).

**(B) Existing models/data.** ESPARGOS datasets (phase-coherent CSI; DaRUS) [12];
dichasus-cf0x (German Aerospace Centre D-MIMO dataset used by channel charting) [28];
SenseFi/MM-Fi/CSI-Bench/OpenCSI for learned layers (licences as in `00` §5); the
ChannelCharting_RealWorld code (bilateration loss) [28]; ESP-PPB firmware (GPL-3.0) [14];
ESP-SDR (GPL-3.0) [13]; esp-csi [32]. Domain gap: all ESP32 CSI research is
single-node/limited-array; no public *coherent mesh* dataset exists.

**(C) Own-data.** The decisive dataset is E1's **Fence dataset** (cross-node residual phase
vs gap/temperature, with oracle phase): nobody has published it for any ESP32-class part.
Then E3's **Weave dataset** (coherent channel-graph snapshots + truth poses/tracks) would
be a first-of-kind public artefact for distributed coherent WiFi sensing; falsification
criteria are written into E1–E3.

---

## 10. Evidence strength and residual unknowns

**Strength: 6/10 for the full Weave** (mechanisms strong, composition novel, one
unmeasured physical gate at the heart). Components: Knot 7/10 (pattern proven on ESP32
family), Fence 5/10 (unrefereed single demonstration), Round 6/10 (APIs exist; protocol
new), Mesh 6/10 (math proven on COTS, 3D untested), Loom 4/10 (mechanism plausible, no
commodity demonstration). The gate is not a gap in theory — it is one number, and E1
measures it.

Residual unknowns: M2′ on S3; S3 CSI common-phase statistics; sum-CP noise on 64-subcarrier
CSI; specular-VA stability at 2.4 GHz in furnished rooms; mutual coupling on a conducting
solid; MAC duty at 20 Hz × 8–20 nodes; whether ESP-PPB's PPB claim survives replication.

---

## 11. Sources

1. D. R. Brown III, H. V. Poor, "Time-Slotted Round-Trip Carrier Synchronization for Distributed Beamforming", IEEE TSP 56(11):5630–5643, 2008. DOI 10.1109/TSP.2008.927073 · https://spinlab.wpi.edu/pubs/Brown_TSP_2008.pdf
2. R. Mudumbai, D. R. Brown, U. Madhow, H. V. Poor, "Distributed transmit beamforming: challenges and recent progress", IEEE Comm. Mag. 47(2), 2009 (ref. list of [3]).
3. F. Quitin et al., "A scalable architecture for distributed transmit beamforming with commodity radios: design and proof of concept", IEEE TWC, 2013 · https://wcsl.ece.ucsb.edu/sites/default/files/publications/quitin_TWC_draft1.pdf
4. R. Preuss, D. R. Brown III, "Two-way synchronization for coordinated multicell retrodirective downlink beamforming", IEEE TSP 59(11):5415–5427, 2011 (ref. [16] of [3]).
5. J. A. Mghabghab, J. A. Nanzer, "Open-loop distributed beamforming using wireless frequency synchronization", IEEE TMTT 69(1):896–905, 2020. DOI 10.1109/TMTT.2020.3022385
6. A. Schlegel, J. M. Merlo, J. A. Nanzer, "Coherent Distributed Bistatic Radar Using Wireless Frequency Syntonization and Internode Ranging", IEEE, 2023 · https://ieeexplore.ieee.org/document/10185098/
7. A. Schlegel et al., "Distributed phased arrays: challenges and recent advances", IEEE TMTT 69(11):4893–4907, 2021. DOI 10.1109/TMTT.2021.3092401
8. H. V. Balan et al., "AirSync: Enabling Distributed Multiuser MIMO with Full Spatial Multiplexing", IEEE/ACM ToN 21(6), 2013. DOI 10.1109/TNET.2012.2230449 · arXiv:1205.6862
9. "Real-time Distributed MIMO Systems" (MegaMIMO 2.0), ACM MobiCom 2016. DOI 10.1145/2934872.2934905
10. R. Rogalin et al., "Scalable Synchronization and Reciprocity Calibration for Distributed Multiuser MIMO", IEEE TWC, 2014 · https://www.semanticscholar.org/paper/f844e559c698ab6a0e2fb1cefb5205549dc5ee2d
11. F. Euchner et al., "ESPARGOS: An Ultra Low-Cost, Realtime-Capable Multi-Antenna WiFi Channel Sounder", arXiv:2502.09405 · https://arxiv.org/html/2502.09405v1
12. F. Euchner, S. ten Brink, "ESPARGOS: Phase-Coherent WiFi CSI Datasets for Wireless Sensing Research", arXiv:2408.16377 · https://arxiv.org/pdf/2408.16377v1
13. ESP-SDR (ESP32 raw IQ; phase-coherent IQ on ESPARGOS One; S3 USB IQ; coherent TX noted; C5 5 GHz) · https://espargos.net/espsdr/ · https://github.com/ESPARGOS/esp-sdr (GPL-3.0)
14. ESP-PPB: "1-PPB wireless sync for phase coherent Wi-Fi CSI", ESP32-C3, FTM + VCTCXO + reference frames, GPL-3.0, 2026 (unrefereed) · https://github.com/jonathanmuller/esp-ppb
15. V. V. Ratnam, B. Sadiq et al., "WiDRa — Enabling Millimeter-Level Differential Ranging Accuracy in Wi-Fi Using Carrier Phase", arXiv:2405.12168 · https://arxiv.org/html/2405.12168v1
16. "WiCAL: Accurate Wi-Fi-Based 3D Localization Enabled by Collaborative Antenna Arrays", arXiv:2505.21408 · https://arxiv.org/html/2505.21408v1
17. ESP-IDF: FTM on ESP32-S3 (initiator/responder) · https://docs.espressif.com/projects/esp-idf/en/v5.2/esp32s3/api-guides/wifi.html ; esp_wifi_80211_tx · https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/network/esp_wifi.html ; nanosecond timestamp thread · https://esp32.com/viewtopic.php?t=35796
18. "Wi-Fi Radar via Over-the-Air Referencing: Bridging Wi-Fi Sensing and Bistatic Radar" (LoSRef), arXiv:2602.05344 · https://arxiv.org/html/2602.05344v3
19. C. Meissner, K. Witrisal et al., "Multipath Assisted Positioning with Simultaneous Localization and Mapping" (Channel-SLAM), IEEE TWC, 2016. DOI 10.1109/TWC.2016.2578336
20. "Multipath-assisted positioning enhancement via DC-SLAM with MAP-PF", Digital Signal Processing, 2024 · https://www.sciencedirect.com/science/article/abs/pii/S1051200423004670
21. "Soft-Coherent Direct Multipath SLAM", arXiv:2604.19723 · https://arxiv.org/pdf/2604.19723
22. "Distributed Coherent Aperture Measurements for Next Generation BMD Radar", IEEE, 2006 (N² coherent-on-receive gain statement).
23. "Coherent-on-Receive Synthesis Using Dominant Scatterer in Millimeter-Wave Distributed Coherent Aperture Radar", Remote Sensing 15(6):1505, 2023 · https://www.mdpi.com/2072-4292/15/6/1505
24. "Wideband distributed coherent aperture radar based on stepped frequency signal", IET RSN, 2016. DOI 10.1049/iet-rsn.2015.0221
25. "Subspace Pseudointensity Vectors Approach for DoA Estimation Using Spherical Antenna Array in the Presence of Unknown Mutual Coupling", Applied Sciences 12(19):10099, 2022 · https://www.mdpi.com/2076-3417/12/19/10099
26. "Spherical Atomic Norm-Inspired Approach for DoA Estimation…", Applied Sciences 13(5):3067, 2023 · https://www.mdpi.com/2076-3417/13/5/3067
27. "The Geodesic Sphere Phased Array Antenna for Satellite Control", DTIC ADA426107 · https://apps.dtic.mil/sti/tr/pdf/ADA426107.pdf
28. S. Taner, V. Palhares, C. Studer, "Channel Charting in Real-World Coordinates with Distributed MIMO", arXiv:2406.13722 (and IIP-Group/ChannelCharting_RealWorld code)
29. "WLAN Device Fingerprinting using Channel State Information", ACM WiSec 2018. DOI 10.1145/3212480.3226099
30. "Enhancing WiFi CSI Fingerprinting: A Deep Auxiliary Learning Approach" (CSI²Q), arXiv:2510.22731
31. "A Comprehensive Survey on Feature Extraction Techniques Using I/Q Imbalance in RFFI", arXiv:2502.02782
32. Espressif esp-csi · https://github.com/espressif/esp-csi (CSI capture across the ESP32 family, incl. S3)
33. "Path to Diversity: A Primer on ISAC-izing Commodity Wi-Fi for Practical Deployments", arXiv:2601.12980 (synchronisation gap; two-way product phase cancellation; distributed-topology sync limitation) · https://arxiv.org/html/2601.12980v2
34. "Indoor Positioning with Wi-Fi Location: A Survey of IEEE 802.11mc/az/bk" (accuracy table), arXiv:2509.03901
35. "Secure Wi-Fi Ranging Today: Security and Adoption of IEEE 802.11az/bk", arXiv:2603.18687
36. "Distributed Coherent Beamforming at 60 GHz Enabled by Optically-Established Coherence", arXiv:2509.13984 (state of the art in *conducted/optical* coherence) · https://arxiv.org/html/2509.13984v1
37. Polarisation diversity for LOS/NLOS via carrier phase, IEEE TCOM 71:1678–1690, 2023 (cited in "IEEE 802.11az Indoor Positioning with mmWave", UC3M repository).
