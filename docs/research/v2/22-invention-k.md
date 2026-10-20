# 22 — Invention K: the Reciprocal Strobe

### Stroboscopic common-view phase referencing for a home of ESP32-S3 polyhedra — and one bench that measures the missing number

Status: invention proposal + boundary analysis (agent `k`). Date: 2026-10-15.
Australian English. Grades: **ESTABLISHED** (primary source; works on the stated
hardware), **HYPOTHESIS** (physically argued, testable, not yet demonstrated),
**SPECULATION** (design conjecture, flagged). Hardware tiers per claim:
**ESP32-S3 / ESP32 family / other COTS Wi-Fi / SDR-grade**. Cross-references are
to `00-synthesis.md` and notes `01`–`20` of this folder; sibling invention notes
are cited where they already claim adjacent ground. This note does not upgrade
any synthesis grade.

---

## 0. Mission and answer in one page

> **Mission.** Discover the new invention that makes a home full of cheap
> ESP32-S3 nodes behave as one sharp, self-calibrating, generalising 3D sensor —
> or prove that no such invention exists and state the exact physical boundary
> that blocks it.

**Answer in one paragraph.** The boundary is real but it is **not one number —
it is three, and they have different fixes.** With independent LOs, every
cross-node coherent scheme on commodity silicon lives or dies on the *residual
phase of a reciprocal exchange after CFO removal* (the corpus's unmeasured M2).
This note decomposes M2 into (i) the **LO phase-noise wander over the exchange
gap**, (ii) the **ESP32 receive-side CFO-correction artefact** — a deterministic,
possibly invertible function of the measured carrier-frequency offset, which
ESP-PPB (2026) sidestepped by clock equalisation rather than inverting — and
(iii) **channel non-reciprocity** over the gap (a moving person). The invention
is the **Reciprocal Strobe**: a protocol and estimator in which every sensing
frame is *bracketed* by two reference events at the fastest exchange rate
commodity ESP32 actually achieves (100 µs frame spacing; ~0.7 ms ESP-NOW
round trip with a fixed MCS7 rate; not the 3–20 ms the corpus assumes), the
relative LO phase is carried by the **channel-free two-way ratio** while the
channel is carried by the **two-way product**, the phase continuation between
exchange points uses **common-mode/subspace extraction** against the room's own
multipath vector (the aperture separates the scalar clock from the world), and
the hub keeps a **timestamped phase ledger** so coherence is applied *after the
fact* in batch — sense first, cohere later. The composite does not need cables,
does not need a VCTCXO, and degrades gracefully into gauge-free modes. Whether
it exists is settled by **one bench curve** (E-1, §6): σ_φ of the two-way ratio
and product versus exchange gap, with a conducted path as ground truth, in three
scenes, plus an inversion test of the CFO artefact. Every coherent capability in
the corpus — cross-node AoA, coherent aperture synthesis, carrier-phase ranging,
3D fusion — becomes a protocol product on stock S3 if that curve passes.

**The physical mechanism** (§3): (1) a scheduled *strobe* of reference frames
one-way broadcast by a coherent polyhedron (the common-transmitter escape) at
the native frame-spacing rate; (2) *bracketing + interpolation* so the phase fix
at a sensing frame is measured, not extrapolated; (3) *two-way unique-reply
exchanges* at the fastest attainable cadence giving the channel-free ratio
(LO phase) and the LO-free product (doubled channel sensitivity); (4) *subspace
common-mode extraction* so a single reference broadcast transfers phase even
through multipath; (5) hub-side *batch coherence* over recorded CSI with
nanosecond RX timestamps. Sharpness: cross-node coherent fusion over home-scale
baselines → 1–2° bearings; coherent integration → M× SNR; carrier-phase
differential → sub-centimetre motion; per-epoch range remains c/2B ≈ 7.5 m at
20 MHz (≈ 2 m with intra-puck channel stitching), cm only differentially.

**What must be true** (§5): the CFO-correction artefact is single-valued and
invertible in Δf; the ESP32 CSI phase carries no per-packet random rotation; the
native frame spacing is usable under hub scheduling; non-reciprocity over a
≤ 1 ms gap is small enough in a furnished room with people moving.

**The decisive experiment** (§6, E-1): two ESP32-S3 nodes, one conducted path
(truth only), measure the per-exchange phase residual *as a function of gap*
(100 µs / 0.7 ms / 3 ms / 20 ms) *in three scenes* (LOS, furnished, walker on
the path), separately for the ratio and the product, with and without CFO-
artefact inversion. Pass: σ_φ ≤ 0.3 rad at the design cadence for ≥ 95 % of
exchanges with motion → the home is one aperture, no cabling, no discipline,
protocol-only. Fail: the residual is non-invertible and ≥ 1 rad → the closing
quantity is named (§7) and the minimal references are a conducted splice, a
per-node VCTCXO + FTM discipline (ESP-PPB class), or per-frame common-TX
referencing with a focused reference beam.

**ESP32-S3 feasibility.** All primitives exist on the family: ns RX timestamps
(note `17`), ESP-NOW HT20 MCS0–7 with LTF-bearing frames and CSI, 100 µs frame
spacing and ~700 µs round trips (community-measured), a 20-face coherent puck by
conducted reference (ESPARGOS principle, productised), FTM for coarse geometry.
No S3 polyhedron has been built; mutual coupling and face shadowing are
unmeasured (sibling notes `08`/`09`); every remaining gap is bench-testable
(§9). **Cost:** puck prototype US$150–400, fixed node US$10–20; a two-puck home
US$400–1000 (SPECULATION on prices). **Transmit needed?** Yes — ordinary
compliant ESP-NOW/action frames only (reference strobe + calibration exchanges);
sensing itself can be passive, and the strobe is ≪ 1 % airtime at 10 Hz.

**Evidence strength: 6/10.** Constituent physics and the parts are 7–9/10
(ESPARGOS productised; WiDRa peer-reviewed two-way carrier phase on COTS; LoSRef
2026 phase-coherent referencing; ESP-PPB unrefereed but detailed). The composite
and the CFO-artefact inversion are 4–5/10 — a bench afternoon decides them.

---

## 1. Method, and what this note adds to the corpus

Two files read in full (`BRIEF.md`, `00-synthesis.md`), the fourteen sibling
invention notes (`22-invention-a…t`) scanned for claimed ground, then layered
`parallel-search` → `parallel-fetch` waves over the six mission directions plus
the round-trip scheme, from primaries (GitHub projects and issue threads, vendor
documentation, arXiv/IEEE, standards text, datasheets). The corpus is strong; the
deltas this note brings are facts and decomposition, not a fifth dialect of
"make the nodes coherent":

1. **The achievable exchange rate is 3–30× better than the corpus assumes.**
   The synthesis (note `09`, `00` §2) uses "MAC turnaround is milliseconds" and a
   "20 ms update-rate optimum". Measured community data: ESP-NOW round trip
   5.4–7.5 ms at the default 1 Mbps, **< 700 µs with a fixed MCS7_SGI rate**
   [ESP-IDF issue 3238]; ESP-PPB reports **~100 µs between consecutive frames**
   when the rate is pushed [ESP-PPB `story.md`]; a dual-S3 board-side benchmark
   gives median 3.6 ms, P99 12.4 ms (scheduling matters) [RichJ233]. The
   fixed-rate API is public in modern IDF (`esp_now_set_peer_rate_config`, HT20,
   MCS0–7) [IDF ESP-NOW]. This changes the M2 arithmetic qualitatively:
   the post-CFO residual (10–100 Hz per the synthesis) accrues 0.06–0.6 rad over
   a 1 ms bracket, not the radians of a 20 ms holdover.
2. **M2 is not one quantity.** ESP-PPB's central finding — with mismatched
   clocks, "the CFO correction is different on each side, and that messes up the
   CSI phase"; with 1 ppb clocks "CSI is clean" [ESP-PPB `story.md`] — means the
   dominant ESP32 per-packet phase indeterminacy is the **receive-side
   CFO-correction artefact**, a function of the measured Δf, not pure LO phase
   noise. Nobody has tested whether it is invertible (E-1).
3. **The two-way observables are already peer-reviewed on COTS.** WiDRa
   (JSAC 2024) isolates the **sum-carrier-phase** from a round-trip exchange,
   explicitly recommends a **SIFS (16 µs)** turnaround, and reports that a 0.5 ms
   round trip "leads to a significant amount of residual phase noise… and can
   cause cycle slips" [WiDRa]. That is the target curve's neighbourhood — on
   Intel silicon with PicoScenes, not on S3.
4. **Phase-coherent referencing without wires now exists on COTS.** LoSRef
   (arXiv 2602.05344, v3 2026) uses the LoS path as an OTA delay/phase reference
   and resolves target dynamics 20 dB below static clutter on unmodified
   hardware [LoSRef]. ESP-PPB demonstrates 1 ms reference-frame strobing across
   ESP32-C3 nodes (with discipline) [ESP-PPB]. The strobe shape is therefore
   prior art in pieces; the composite on stock S3 is not.
5. **Peers' claimed ground** (avoided and credited): coherence-first wireless
   locking (`a`, `i`, `n`, `t`), geometry-first reverse bootstrap (`s`), closure
   phases / interferometry (`o`, `h`), static field as clock (`r`, `q`), the
   coherent puck itself (`b`, `e`, `m`, `p`). This note differs by taking the
   **rate as the design variable**, decomposing M2, and specifying the bench
   that measures each term.

---

## 2. The boundary, with today's numbers

### 2.1 The gauge (ESTABLISHED — note `20`)

With independent LOs, a CSI observation across nodes is
`ĥ_ij(f) = g_ij·a_i·b_j·e^{j(θ_i−ψ_j)}·e^{−j2πfτ}`; only `θ_i − ψ_j` is
observable, and across a connected graph the node phases are fixed only up to
one global `U(1)` constant (plus time/frequency gauges). Gauge-invariant:
amplitudes; per-link subcarrier phase slope (delay); intra-node inter-face phase
(AoA *within* one coherent puck); temporal differentials (Doppler); and two-way
products and ratios. Gauge-blocked: absolute cross-node phase, absolute
phase-of-flight, cross-node AoA. One constraint per connected component crosses
it; the three published escapes are a common transmitter, OTA reciprocity, and a
shared/conducted reference. The hardware premise's conducted clock + 2.4 GHz
phase reference makes each polyhedron one gauge-fixed 20-element aperture
(ESPARGOS principle, [ESPARGOS]; multi-array coherence by sharing the reference
[ESPARGOS-COMBO]); *between* pucks the gauge is what the strobe must fix.

### 2.2 The exchange budget, decomposed (the corpus's M2, split)

For a reciprocal same-channel exchange (A transmits at `t1`, B replies at `t2`,
gap `Δ = t2 − t1`, channel estimate at each side over the same subcarriers):

- **Product (sum-CP):** `∠(ĥ_AB·ĥ_BA) = 2∠h + hw_P + 2π Δf·(t1−t2)…` — the
  PLL initial phases cancel *exactly* (the same physical LO supplies transmit
  and receive phases at both instants); the channel appears at **doubled**
  sensitivity (ambiguity λ/2 = 6.1 cm at 2.45 GHz). [WiDRa, Chronos; corpus
  notes `10`, `20`]
- **Ratio (difference-CP):** `∠(ĥ_AB/ĥ_BA) = hw_R + 2(θ_A−ψ_B) + …` — the
  channel cancels *if reciprocal over Δ*; the oscillator phase difference
  appears at doubled sensitivity. The ratio is the **channel-free gauge fix**;
  the product is the **LO-free channel**.

This is the only pair of observables a ping/unique-reply protocol needs; my
contribution is to write the *error* of each as a sum of three physically
distinct, separately measurable terms:

    σ²_exch(Δ, scene)  =  σ²_artefact(Δf)        (ESP32 receive-side CFO correction)
                        + σ²_wander(Δ)           (LO phase noise after CFO removal)
                        + σ²_nonrecip(Δ, scene)  (channel change between the two legs)

- **σ_artefact:** consequence of [ESP-PPB]: each ESP32 estimates and removes CFO
  internally, per packet; two chips with different offsets apply *different*
  corrections, and the residual intercept varies per packet [ESP-PPB `story.md`;
  esp-csi issue #121]. It is plausibly a *single-valued function of the measured
  Δf* (the chip's own estimator is deterministic) — hence **invertible by
  calibration** (§5, E-1). This is the term the corpus never separated, and it
  is the one a *software* fix can remove.
- **σ_wander:** differential LO phase noise over Δ after removing the *measured*
  frequency difference. Crude arithmetic: 10–20 ppm raw gives 5–10 µs to π/2
  [note `17`]; but between *exchanges* the relevant number is the post-CFO
  residual the synthesis puts at 10–100 Hz (MegaMIMO), i.e. 0.06–0.6 rad over a
  1 ms bracket. ESP-PPB measures < 1° best, < 10° typical single-frame phase
  *with 1 ppb clocks* — i.e. 0.02–0.17 rad, with the artefact term suppressed
  [ESP-PPB]. WiDRa observes cycle slips at 0.5 ms on its pipeline [WiDRa].
- **σ_nonrecip:** a walker (1 m/s) crossing a link produces a round-trip
  Doppler of ≈ 2v/λ ≈ 16 Hz (sum of both legs); over Δ = 100 µs the channel
  phase moves ≈ 0.01 rad, over
  3 ms ≈ 0.3 rad, over 20 ms ≈ 2 rad. **Bracket cadence, not coherence
  technology, is what defeats multipath dynamics** — this resolves the live
  tension between sibling `p` (per-frame common-TX referencing is
  "multipath-biased in exactly the moments sensing matters") and siblings
  `r`/`t`/`n` (the static field as reference): at 10 kHz strobing the bias is the
  *change* over 100 µs, not the total dynamic phase.

Composability at the hub: the *ratio* is channel-free, so the ledger's phase
fixes never require channel knowledge; the *common-view* continuation does, but
only to first order, and the static calibrants (ranges, bearings, static CIR)
are re-estimated continuously by the same estimator (§3.2).

### 2.3 What each resource buys (honest sharpness accounting)

| Resource | Physical limit | Consequence |
|---|---|---|
| 20-face puck, one LO | λ/D ≈ 12.5 cm / 30 cm ≈ 24° beamwidth; 56 tones × 20 MHz | 3D bearing sensor, "compound eye"; range per epoch c/2B ≈ 7.5 m |
| 13-channel intra-puck stitching (absolute phase anchored by the conducted reference + per-channel calibration) | ~83.5 MHz in band | range per epoch ≈ 1.8 m; HYPOTHESIS on S3 (note `08`, sibling `i`/`t`) |
| Cross-node phase at strobe rate | baselines up to home scale, 6–10 m | cross-node AoA 1–2°; coherent integration ×M; virtual TDM-MIMO array with M×20 phase centres |
| Carrier-phase *differential* tracking (gauge-free) | λ/2 ambiguity, phase noise | sub-cm motion; Doppler per link |
| Static multipath (environment as calibrant) | stability seconds–hours | virtual anchors for geometry; the "reference vector" of §3.2 |
| Two-way products/ratios at 1 ms | σ_exch above | gauge fix O(N) per cycle; hardware fingerprints; obstruction maps (|product|) |

Nothing here beats diffraction: the puck alone is coarse. **The home is sharp
because the home is the aperture** — a claim that is only true if σ_exch is
small. That is why one curve decides everything.

---

## 3. The invention: the Reciprocal Strobe

**Name.** The Reciprocal Strobe (RS): *stroboscopic common-view phase
referencing with a reciprocal calibration channel and a hub-side phase ledger*.
Three layers, each independently useful, plus one hardware re-use of the
polyhedron premise.

### 3.1 S1 — the Strobe protocol (the rate is the design variable)

1. **The strobe.** A coherent polyhedron (the "hub puck") emits one-way
   reference frames on a fixed schedule — nominally every 10 ms for the slow
   ledger, and **in bursts of 8–16 frames at the native spacing (~100 µs)** with
   each frame's CSI recorded by every node in earshot. Each frame gives every
   listener a measurement `(θ_h(t) − ψ_i(t)) + ∠h_hi(t) + hw` per face/subcarrier;
   because both the transmitter phase and the (static-calibrated) channel term
   are common to all listeners, one broadcast fixes the *relative* LO phases of
   all listeners at that instant up to each link's static channel phase — the
   common-transmitter escape at O(1) airtime per event [t/n/r claim the
   principle; ESP-PPB strobes reference frames at 1 ms on C3].
2. **Bracketing.** Every sensing illumination (the hub's own ESP-NOW sensing
   frame, or an ambient AP frame the nodes all hear) is scheduled *between* two
   strobe bursts. The phase applied to a sensing frame is an **interpolation**
   between the bracketing fixes, not a holdover. At 100 µs brackets the
   interpolation error from σ_wander is second order; the residual is dominated
   by the *measurement* error of the bracketing fixes, which is exactly what E-1
   measures.
3. **Reciprocal calibration channel.** A low-rate (e.g. 10 Hz) ping + unique
   reply round on the fastest attainable ESP-NOW schedule gives, per pair, the
   *ratio* (channel-free LO phase — the ledger's ground truth) and the *product*
   (doubled channel + hardware asymmetry). Unique reply payloads are required
   only for identity/de-collision (BRIEF's round-trip scheme); a star around the
   hub is CRB-optimal (note `20`) and keeps the airtime O(N) per cycle.
4. **Within-frame CFO probe.** Each HT20 ESP-NOW frame reports both L-LTF and
   HT-LTF estimates separately (the IDF CSI buffer stores each LTF's response
   in order [IDF CSI]; disable `ltf_merge`). Their phase difference is a
   **within-frame, channel-constant relative-frequency probe** with a lever
   of the L-LTF→HT-LTF separation (~8–20 µs); the same trick standardises CFO
   measurement across multiple HT-LTFs 4 µs apart [Tsinghua WST]. This gives the
   interpolator a *per-frame* frequency estimate without any exchange,
   suppressing the linear term of σ_wander. HYPOTHESIS on S3 (depends on both
   LTF estimates being reported unmerged).
5. **Scheduling.** The hub (FPGA/Ethernet hub face of the premise) owns the
   schedule: slots for strobe bursts, sensing frames, and calibration exchanges;
   nodes transmit only when polled (GPIO-synchronised start, ns RX timestamps
   record the truth). The dual-S3 benchmark's P99 of 12 ms shows why open
   contention is not acceptable: scheduled slots, not best effort.

### 3.2 S2 — the estimator: the room is the reference vector

The distinguishing estimation move. A node receiving *any* transmission (strobe,
ambient AP, peer) measures a vector: 20 faces × 56 subcarriers (×polarisation).
Model it as `y = α(t)·ĥ(t) + n`, where `α(t)` is the **scalar** common-mode
phase (LO phase difference + the CFO artefact, identical on every face and
subcarrier) and `ĥ(t)` the channel **vector**. Then:

- **Clock extraction:** given the previous channel vector estimate, the
  maximum-likelihood scalar is `α̂ = (ĥ†y)/(ĥ†ĥ)`; its phase tracks the clock
  change. This is common-phase-error estimation (standard in OFDM receivers
  using pilots) applied **cross-node with the aperture as the pilot** — the
  multipath that normally corrupts the reference is *projected onto* ĥ and thus
  becomes part of the reference, not noise. HYPOTHESIS (novel composition;
  closest art: CPE estimation, subspace channel tracking [e.g. arXiv
  1704.06765]).
- **World extraction:** the orthogonal residual `y − α̂ĥ` is the sensing signal;
  a change in the scene is a change of ĥ, which *rotates the reference* only in
  the direction of the old ĥ and is otherwise contained in the residual.
- **The confound and its mitigation:** a scene change is indistinguishable from
  a clock change if it happens to look like `α·ĥ`. Mitigations: (i) *two
  illuminators from different bearings* (the hub puck can radiate from different
  faces/channels; ambient APs add more) — the clock term is common across
  illuminators, so the confound is over-determined; (ii) *two-way ratios*, which
  are channel-free, arbitrate; (iii) *bracket interpolation*, which makes the
  clock term continuous while the sensing event is impulsive. This is the
  quantitative bridge between "the environment as a reference" (`r`, `q`, `n`)
  and "the environment as the unknown" (`p`).
- **Ledger and batch coherence.** The hub fuses strobe fixes, ratio/product
  exchanges, and within-frame CFO probes in a factor graph / Kalman smoother per
  node, per channel, with ns-timestamped records (note `17`; ESP-PPB uses the
  same hidden ns timestamp on C3-family). Corrections are then applied to *all*
  recorded CSI in batch. Nothing needs to be locked in real time — **coherence
  becomes a data product, re-solvable at will.** A Kalman phase tracker over
  bi-directional measurements is exactly what the 2025 distributed-arrays
  literature recommends when LOs drift and links are weak [arXiv 2504.11411].

### 3.3 S3 — hardware re-use of the polyhedron (what the premise buys the strobe)

- The hub puck's **conducted 2.4 GHz phase reference** calibrates each face's
  TX *and* RX chain per channel — so the hub can also *steer* the reference: a
  coarse (24°) but multipath-suppressing focused reference beam toward each
  room, and per-face polarisation. (Sibling `i` claims the reversed-bus TX
  calibration; the stroke here is its use as a *reference beam*.)
- **Within-puck face-to-face exchanges** (simultaneous TX on one face, RX on
  another, both coherent by the conducted reference) give exact-geometry
  two-way calibration of every face's chain constants — the "ruler" that turns
  the ledger's static constants into measured quantities.
- **FTM** from each puck face pins coarse ranges (1–2 m [note `15`]) to other
  pucks and satellites; per-puck 3D bearings + per-link carrier-phase
  differentials refine to cm; the static scene (walls resolved by bearings and
  the static CIR) is the fiducial set. ⚠ Note `09` stands: a switched single
  chain would need TDM, but the premise's one-radio-per-face means every face
  receives *the same packet simultaneously* — no face switching in RX.

### 3.4 What the composite delivers (the sensor)

Per strobe cycle (~10 ms), the hub holds a phase-coherent snapshot of all nodes
and combines them. The deliverables, in order of certainty:

1. **Relative geometry** (unblocked without any coherence): FTM ranges +
   per-puck 3D bearings + static-scene registration; the classic gauge-free
   route. Self-surveying; no tape measure.
2. **Cross-node coherent reception**: every node's CSI of the same frame
   combined as one aperture for that frame — cross-node AoA at home baselines,
   coherent integration gain, and the *reconstruction of every node's own
   carrier phase* in the hub frame.
3. **3D targets**: multistatic bearings × ranges × Doppler per node, fused in
   the hub frame; motion resolved to sub-cm by carrier-phase differentials
   (gauge-free, so available even in degraded modes).
4. **Absence and obstruction maps**: |product| attenuation per link at 10 Hz;
   non-reciprocity dynamics (a body crossing a link) as a distinct observable
   else invisible to one-way methods.
5. **Identity/fingerprints**: the static hardware-asymmetry terms (from
   product and ratio) per pair per channel — a device-level fingerprint ledger
   (direction 4's "round-trip hardware-asymmetry signature"), usable for
   commissioning and for spoof detection.

---

## 4. What is genuinely new (claim list, closest art, and grade)

| # | Claim | Closest art | Delta | Grade |
|---|---|---|---|---|
| N1 | **The rate theorem for commodity coherence**: the design variable for wireless-only cross-node coherence is the *exchange cadence*, and commodity ESP32 cadence is 0.1–3 ms (not 3–20 ms); at that cadence even a 10–100 Hz post-CFO residual is a 0.06–0.6 rad bracket error, and multipath dynamics over the bracket is ≤ 0.01–0.3 rad | Corpus `09`/`00` assume ms MAC turnaround; ESP-PPB uses 1 ms reference strobing with clocks; AirSync/BeamSync re-sync at slot/10-min cadence on SDR | Quantify the cadence limit on S3 and make cadence the sanctioning variable | HYPOTHESIS (arithmetic ESTABLISHED; S3 numbers community) |
| N2 | **M2 decomposition** into artefact + wander + non-reciprocity, each separately measurable | Corpus treats M2 as one residual | Makes three fixes possible instead of one | HYPOTHESIS (terms individually ESTABLISHED) |
| N3 | **Two-way observables as a pair**: ratio = channel-free gauge fix; product = LO-free (doubled) channel; the ledger always fixes phase via the *ratio* so no channel knowledge is needed for coherence | Sibling `s` derives the same algebra; WiDRa uses the sum-CP for ranging | The *protocol consequence*: phase fixes never depend on the environment being static | ESTABLISHED algebra, new composition |
| N4 | **Common-mode/subspace clock extraction**: the coherent aperture projects the received vector onto the tracked channel vector, making the room itself the reference vector and the *scalar clock* observable from a single illuminator | CPE estimation (within receiver, pilots); subspace channel tracking; sibling `r` uses the static field as clock via scalar ARPT | Population of a *scalar/vector* decomposition with the 20-face cone as the separator | HYPOTHESIS |
| N5 | **Strobe + bracket + ledger**: schedule reference bursts to bracket every sensing frame; interpolate; batch-correct recorded CSI at the hub ("sense first, cohere later"); reprocessable | ESP-PPB strobing (with discipline); sibling `r` Kalman tracking (real-time flavour); BeamSync/AirSync real-time | Bracket-interpolation error bound + batch ledger as the systems answer to drift, retunes, dropouts | HYPOTHESIS |
| N6 | **CFO-artefact inversion** as the cheapest crossing on stock S3 | ESP-PPB *equalises clocks* to sidestep it | Tests whether the artefact is a single-valued, invertible function of measured Δf — if yes, no discipline needed | HYPOTHESIS (falsifiable in a day) |
| N7 | **Dual-LTF within-frame CFO probe** on ESP32 | Standard practice on other chips [Tsinghua WST] | Enables per-frame interpolation without an exchange | HYPOTHESIS on S3 |
| N8 | **Reference-beam strobe**: the hub puck radiates the reference through a coarse focused beam (conducted-ref-calibrated faces), suppressing reference multipath | Sibling `t`'s "reference frame radiated from a calibrated aperture" | Framed as a bias-reduction lever with the strobe cadence | HYPOTHESIS |

Honest positioning: **the invention is a composition with two testable kernels**
(N6 and N4) and one design principle (N1). If N6 and N4 fail, the composition
retains N5's batch ledger on top of a conducted or disciplined reference —
still useful, no longer "wireless-only".

---

## 5. What must be true (assumption ledger with falsifiers)

| # | Assumption | Falsifier | How it is checked |
|---|---|---|---|
| A1 | ESP32-S3 CSI phase carries **no random per-packet rotation** (unlike the COTS chips of [WiDRa], which rotate by 2πn/N per frame); the variability is the CFO-correction artefact | Repeated exchanges at zero Δf show random per-packet phase beyond the AR(1) wander | E-1 arm 0 (equal clocks; repeatability test) |
| A2 | The CFO-correction artefact is **single-valued in measured Δf** and invertible to ≤ 0.2 rad residual | Same Δf, different packets agree; sweeps of Δf reproduce one curve; random component dominates | E-1 arm 2 (rate-fix + VCTCXO sweep) |
| A3 | The channel is reciprocal over Δ ≤ 3 ms in a furnished home with people moving (σ_nonrecip ≤ 0.3 rad) | Scene-dependent residual grows with motion beyond the Doppler bound 2π·2v/λ·Δ | E-1 arm 3 (walker on the link) |
| A4 | Both LTF estimates are reported unmerged, giving a clean within-frame frequency probe | IDF merges LTFs or reports one estimate | IDF config audit + bench |
| A5 | A 20-face S3 puck can radiate and receive with coupling/shadowing manageable by calibration (or by rejecting bad faces) | Coupling so strong that faces are mutually incoherent after calibration | Sibling `08`/`09` open question; puck bench E-4 |
| A6 | Hub-scheduled slots keep P99 exchange gaps ≤ 3 ms at 20-node scale | Contention/turnaround make the cadence unobtainable | Scheduler bench, scaling test |
| A7 | Static channel phases are stable enough (seconds–minutes) for common-view continuation between calibration updates | A person *standing* on a link biases continuation | E-3 room test with furniture and a stationary human |
| A8 | The reference strobe's own multipath bias is bounded by the *change* over the bracket (N1 arithmetic) | Sudden scene change exactly at bracket time (a door slamming) | E-3; mitigation: ratio arbitration |

---

## 6. The decisive experiment family

### E-1 (THE decisive one): the wander curve and the inversion test

**Rig.** Two ESP32-S3 nodes (devkits acceptable), a conducted path between them
(splitter + attenuators) used *only as ground truth* — the same trick the
polyhedron hub provides. Ideally one node carries the ESP-PPB VCTCXO + DAC so
Δf can be *forced*; otherwise temperature + module spread gives the natural
range, and a signal generator injects a known offset into one node's clock path.

**Procedure.**
1. Alternate ESP-NOW HT20 frames at the fastest schedule the firmware can hold
   (fix MCS7_SGI; burst spacing measured); both nodes record per-frame CSI
   (both LTFs unmerged), ns timestamps, RSSI, sig fields.
2. Per exchange compute: ratio phase (LO difference), product phase (sum-CP),
   dual-LTF within-frame frequency probe, measured gap Δ from ns timestamps.
3. Ground truth: the conducted path gives the true relative LO phase and true
   Δf to ~mrad and ~Hz, so **every term of σ_exch is separable**:
   `measured − truth` decomposed into a function of Δf (artefact) and a zero-mean
   process in time (wander), with the scene arm adding non-reciprocity.
4. Arms: (0) equal clocks (VCTCXO locked) — tests A1; (1) natural Δf — baseline;
   (2) Δf swept over 0…±50 kHz — maps the artefact law and tests A2 by
   *inverting* it and re-measuring; (3) scenes: LOS lab / furnished room /
   walker crossing the link — tests A3; (4) gaps: as-low-as-possible (~0.1–1 ms
   sustained, 3 ms, 20 ms) — the σ_φ(Δ) curve.
5. Deliverable: **σ_φ(Δf, Δ, scene) for ratio and product, before and after
   artefact inversion** — the field's missing calibration curve.

**Pass/fail tree.**
- *Pass (green):* after inversion, σ_φ ≤ 0.3 rad for ≥ 95 % of exchanges at
  Δ ≤ 3 ms in the walker scene → wireless-only coherence is a protocol product
  on stock S3; proceed to E-3 and E-4; every coherent capability in the corpus
  is unlocked without cabling or discipline.
- *Amber:* σ_φ ≤ 0.6 rad at Δ ≤ 0.3 ms only → coherence exists but demands the
  sub-ms cadence and bricks with contention; beam-focus the reference (N8) and
  re-test with the puck.
- *Red:* non-invertible artefact or wander ≥ 1 rad at any usable Δ → the closing
  quantity is named (§7), and the minimal reference is chosen from the three
  crossings.

### E-2: the artefact law (one day, one node pair, no room)

Same rig, sweep Δf (VCTCXO DAC or injected clock offset), hold everything else
fixed; measure CSI phase intercept of the peer's frames versus true Δf. Repeat
with different power, different channel, after reboot, at two temperatures. If
one curve reproduces (a 1-D lookup over ±50 kHz), invert it in firmware and
proceed to E-1 arm 3. If 256-valued (the int8 CFO estimate) but *consistent*,
invert on the state index. If neither, the red path.

### E-3: two pucks in a room (the self-calibration test)

Two small coherent sub-arrays (e.g. 8-face ESPARGOS-style or one polyhedron),
unroved and un-surveyed, in a furnished room, 3–8 m apart, running the strobe
for ≥ 1 h across a ≥ 5 °C swing. Recover relative pose from FTM + bearings +
static scene + carrier-phase refinement; validate to < 30 cm / < 10° against a
tape measure. Simultaneously track a person to sub-decimetre with a camera as
truth. (This subsumes sibling `p`'s E-FIDUCIAL, with the strobe added.)

### E-4: the polyhedron (the premise test)

Build one 20-face puck (or a 12-face variant first): verify intra-puck coherence
(conducted reference), measure mutual coupling and shadowing versus face pair,
verify simultaneous multi-face RX of the same packet, and the face-to-face
exchange constants. This is the only part that needs new hardware.

**Why E-1 is *the* decisive experiment:** it measures precisely and separately
the three terms that the entire corpus, across fourteen invention notes,
currently treats as one unmeasured number. No engineering decision above it is
well-posed until it exists.

---

## 7. If the space is closed: the quantity, and the minimal references

**The closing quantity** (if the red path fires) is the **non-invertible part of
the ESP32 receive-side CFO-correction phase plus the post-CFO LO wander at the
exchange gap** — equivalently, the measured lower bound of `σ_exch(Δ)` in the
sub-millisecond regime. It is *not* a fundamental physics constant; it is a
device-pipeline quantity. The physical floor beneath it is the crystal's phase
noise, which is small at 1 ms; therefore a red result would indict the *chip's
correction pipeline*, not electromagnetism.

**Minimal references that cross it** (in ascending cost of change):

1. **Software only (no hardware change):** none — this is what E-1/E-2 decide.
2. **Per-node clock equalisation:** VCTCXO + dual DAC + FTM discipline (ESP-PPB
   class, ~US$1–2 BOM, no cabling). Equal clocks ⇒ equal CFO corrections ⇒
   "CSI is clean" [ESP-PPB]; ~1 ppb gives π of holdover in ~200 ms, so the
   strobe relaxes to Hz-rate. This is *not* a common reference; it is N
   independent good clocks.
3. **One conducted splice per connected component:** the ESPARGOS principle
   (40 MHz clock + 2.4 GHz phase reference shared by wire), proven on the
   family, productised; the polyhedron is already this, so the minimal crossing
   is *one coax between adjacent pucks* — the "coherence splice".
4. **Per-frame common-TX referencing with a focused reference beam:** no
   inversion needed (the transmitter's phase is common and known), at the cost
   of airtime and the multipath bias, which the bracket analysis bounds (§2.2).

**Honest degradation modes if only parts survive:**
- *Mode A (green):* full strobe — home-scale coherent aperture; sharp.
- *Mode B (amber):* coherence only within a puck + gauge-free cross-node
  (products, ratios, bearings, FTM, fingerprints) — sibling `p`'s
  coherence-locality; a good 3D sensor, not one aperture.
- *Mode C (red):* receive-only learned sensing as in the v2 commodity plan —
  unchanged from `00-synthesis.md` §8.

---

## 8. The six mission directions, answered; and the round-trip scheme

| # | Direction | What the search found | Verdict for this design |
|---|---|---|---|
| 1 | Wireless coherence on commodity silicon | Two-way carrier phase is standardised on commodity BLE (Channel Sounding PBR) and peer-reviewed on Wi-Fi (WiDRa; Chronos); OTA clock discipline on ESP32-C3 at 1 ppb (ESP-PPB); reference strobing exists (ESP-PPB) | **Core.** The strobe's rate + ratio fixes it; the residual is the whole question (E-1) |
| 2 | Self-calibrating learned array; transfer across homes | Geometry-first estimation (FTM + bearings + static scene + bundle adjustment) beats learned calibration; learned models stay for targets (note `19`; Sim2Real 82.1 % F1 at 20 % labels) | Procedural, physics-first; ML optional on the target layer; no per-home learned calibrator |
| 3 | Environment as shared reference | LoSRef (2026) proves OTA LoS referencing on COTS; static-field-as-clock is claimed by `r`/`q`; the *confound* (clock vs world) is the crux | **N4**: common-mode/subspace extraction is the cleanest formulation; brackets bound the residual bias |
| 4 | Sharp 3D from many coarse apertures | No ESP32 volumetric array exists; spherical/geodesic geometry established for DoA; aperture convolution gives virtual arrays; polarisation adds diversity (ESPARGOS dual-pol) | Cross-node coherent fusion + TDM-MIMO virtual array; polyhedron supplies TX/RX aperture and polarisation; range via stitching only |
| 5 | Ping + unique replies + two-way products | Products (WiDRa, Chronos, RTCSI, ML-Track), ratios (TDD reciprocity literature, BeamSync), fingerprints (IQ imbalance); all prior art in pieces | **Core**: ratio = gauge, product = channel; unique replies only for identity; here is what is *new*: the ledger, the artefact decomposition, the rate theorem |
| 6 | Different excitation/composition | 802.11bf standardises SIFS-separated sensing sounding; TDM MIMO across pucks is legal but slow; passive illumination (ambient) is free and central; coherent TX *within* a puck is enabled by the conducted reference | Strobe + brackets; coherent TX within puck for the reference beam; ambient frames as zero-cost strobe supplements |

**Round-trip scheme audit** (BRIEF: one node pings; every other replies with a
unique packet; round trips timed — what can this measure, where does it break?).

*Measures:* (i) per-reply ToF from µs timestamps — ±150 m per sample, useless
alone, FTM-class only after heavy averaging (note `17`); (ii) **ratio phase** —
relative LO phase at doubled sensitivity (channel-free); (iii) **product
phase** — channel at doubled sensitivity (LO-free); (iv) per-link Doppler from
successive products; (v) identity/de-collision (unique payloads required);
(vi) |product| attenuation → obstruction maps; (vii) hardware asymmetry terms —
fingerprints. *Breaks:* (a) exchange gap vs σ_exch (the core issue, E-1);
(b) non-reciprocal scene change within the gap (a walker on the link biases both
observables — syndromes: product and ratio disagree in a specific way);
(c) the ESP32 CFO-correction artefact (E-2); (d) non-OFDM replies carry no CSI
(11b ACK/CTS are LTF-free — the SIFS-hardware-reply hope fails on CSI grounds,
not timing grounds; this note records the negative result); (e) scheduling cost:
O(N) replies per ping; a star hub keeps it linear and CRB-optimal; (f) any
per-packet random phase rotation (A1) would defeat both observables equally —
test first.

---

## 9. ESP32-S3 feasibility, element by element; cost; transmit

| Element | Status | Basis |
|---|---|---|
| ESP-NOW HT20 MCS7 with LTF/CSI | **Works** | Public rate API [IDF ESP-NOW]; esp-csi uses ESP-NOW frames for CSI soundings [esp-csi repo] |
| Fast exchange (~0.7 ms RTT; 100 µs frames) | **Measured by community** | [ESP-IDF 3238]; [ESP-PPB story] |
| Per-frame phase from CSI | **Works on ESP32 family; quality depends on clock agreement** | [Wi-ESP: "usable phase values"; esp-csi #121 slope stability; ESP-PPB CFO finding] |
| ns RX timestamp | **Works on S3** | note `17`; hidden ns fields on C3+ [ESP-PPB; Arduino forum 2026] |
| Dual-LTF within-frame CFO probe | **Untested on S3** (config audit + bench) | IDF CSI LTF layout [IDF CSI]; method standard elsewhere [Tsinghua WST] |
| Two-way product/ratio on S3 | **Untested** — the corpus's M2; E-1 | WiDRa on COTS [WiDRa]; sibling `s` algebra |
| 20-face polyhedron with conducted reference | **Proven in planar form** (ESPARGOS One, 8 faces, productised; Aperture Kit 6×4 combines coherently); 3D instance unbuilt | [ESPARGOS; ESPARGOS-COMBO] |
| Hub aggregation | Data budget: per-face decimation required (scalars at frame rate, CSI at sensing rate) | ESPARGOS streams per-packet CSI over 100BASE-TX [ESPARGOS datasheet] |
| FTM ranging on S3 | 1–2 m, 2.4 GHz HT20 only | note `15` |
| Coherent TX within puck (reference beam) | Plausible via conducted TX-chain calibration (sibling `i` claims the reverse-bus trick) | [ESPARGOS calibration principle] |

**Cost (SPECULATION on prices).** Fixed node: ESP32-S3 module + PCB + antenna
US$10–20. Puck prototype: 20 modules + face PCBs + hub + clock/reference
distribution US$150–400 (compare ESPARGOS One at €350 for 8 faces; the 20-face
puck amortises the hub). Two-puck home + four nodes: US$400–1000. No new chips,
no special silicon, no spectrum beyond ordinary Wi-Fi.

**Transmit needed?** Yes, but only ordinary compliant frames: reference strobe
(one-way broadcasts), calibration exchanges (ping/unique-reply), sensing
illumination (ESP-NOW HT frames). No protocol-violating frames; no jamming; no
higher duty than standard Wi-Fi at 10–50 Hz (note `07` governs the legal
envelope; the strobe is ≪ 1 % airtime). Sensing can be passive between strobes.

---

## 10. (A) Chaining, (B) existing models/data, (C) own data

**(A) Chaining.** The strobe is deliberately placed *before* the coherence-
dependent capabilities: it is the missing enabling capability, not a consumer of
one. Chains: strobe ⇒ cross-node phase ⇒ (cross-node AoA, coherent integration
⇒ sharp 3D; coherent ranging ⇒ RF-SLAM prerequisites; interferometric closure
becomes *optional* rather than load-bearing). Gauge-free capabilities
(products, Doppler, channel charting, RTI, FTM self-survey) run *without* the
strobe and are its commissioning instruments. If E-1 goes red, the chain
degrades to Mode B; the commodity learned route of `00-synthesis.md` §8 is
untouched.

**(B) Existing models/data.** Reusable as in the synthesis §5: SenseFi (MIT),
MM-Fi, ESPARGOS datasets (DaRUS DOI), CSI-Bench, OpenCSI; domain adaptation
mandatory (L-CSI vs ESP32 CSI). New this note: ESP-PPB (GPL-3.0 firmware,
VCTCXO discipline — reusable reference implementation); pyespargos
(calibration/CSI pools, radar utilities); esp-crab (Espressif two-chip common
crystal). For the estimator: classical CPE/subspace tracking literature (e.g.
arXiv 1704.06765) and Kalman OTA phase tracking (arXiv 2504.11411). Licences:
sense/adapt freely; ESP-PPB is GPL — treat as reference until licence review.

**(C) Own data / new technology.** The minimal new artefact is *not* a dataset
but a *calibration curve*: **E-1's σ_φ(Δf, Δ, scene) for ESP32-S3 two-way
carrier phase.** No public primary contains it; on it hang every coherent Aquila
capability (and, plausibly, other S3 users' designs). The minimal exhorting
dataset that proves the sensor: E-3's two-puck room run — relative pose to
< 30 cm, person tracked to sub-decimetre against camera truth, across a
temperature step. Falsified if pose recovery needs a tape measure or if
tracking collapses under multipath dynamics at the design cadence.

---

## 11. Evidence strength and residual unknowns

**Evidence strength: 6/10** (new chips of 4–5/10 embedded in a corpus of 7–9/10
evidence). Strongest: the parts are demonstrated somewhere (conducted array,
VCTCXO sync, COTS two-way carrier phase, LoS referencing); the rate limits are
community-measured but reproducible; the gauge analysis is settled. Weakest: no
S3 two-way carrier phase has been demonstrated; no ESP32 volume array exists;
the CFO-artefact law and the dual-LTF probe are untested; the subspace clock
extraction is new and unproven; the whole composite has never been built.

**Residual unknowns.** (1) The artefact law and its invertibility — E-2.
(2) Per-packet phase determinism on S3 at zero Δf — E-1 arm 0. (3) Non-
reciprocity over ≤ 3 ms in real homes — E-1 arm 3. (4) Whether both LTF estimates
are reported separately by the S3 CSI engine. (5) Mutual coupling/shadowing on
a soldered solid (sibling `08`/`09` open question) and whether a 12-face variant
suffices. (6) Scheduler jitter at 20-node scale over Ethernet. (7) The
temperature coefficient of the exchange residuals (the corpus's CFO-vs-
temperature unknown, refined here to *artefact* vs *wander*). (8) Legal airtime
at 10–50 Hz strobe rates across multiple homes in one band (note `07` bounds
per-device duty; aggregate co-channel effects untested).

---

## 12. Primary sources (with DOIs/URLs, as verified in the search waves)

1. **ESP-PPB** — J. Muller, "ESP-PPB: 1-PPB wireless sync for phase coherent
   Wi-Fi CSI", GitHub, created 2026-02-19, GPL-3.0:
   <https://github.com/jonathanmuller/esp-ppb>; mechanism and lessons in
   `story.md` (10–20 ppm ⇒ ~150 rad/ms; VCTCXO + dual DAC; 1 ppb typical /
   0.1 ppb enclosed; existing CFO-correction corruption; ~100 µs ESP-NOW frame
   spacing; 1 ms reference-frame strobing). Unrefereed; graded as such.
2. **ESPARGOS** — project and product: <https://espargos.net/>;
   ESPARGOS One datasheet (8 dual-pol faces, shared 40 MHz + 2.4 GHz reference,
   L-LTF 12-bit / HT-LTF 8-bit, PoE; Aperture Kit 6×4) — <https://espargos.net/espargos-one/espargos-one-datasheet.pdf>;
   multi-array coherence by shared reference and re-lock invalidation —
   <https://espargos.net/docs/combined-arrays.html> and `pyespargos`
   `sensor_calibration` docs. Papers: F. Euchner, S. ten Brink, arXiv:2408.16377;
   ESP-SDR / ESPARGOS arXiv:2502.09405.
3. **WiDRa** — V. V. Ratnam et al., "WiDRa: Enabling Millimetre-Level
   Differential Ranging Accuracy in Wi-Fi Using Carrier Phase", IEEE JSAC 2024,
   DOI 10.1109/JSAC.2024.3413985; arXiv:2405.12168 (sum-carrier-phase from a
   round-trip exchange; recommends SIFS 16 µs; CFO 4–8 kHz; cycle slips at
   0.5 ms round trip; per-frame random phase rotation 2πn/N on COTS chips).
4. **Chronos** — D. Vasisht et al., "Sub-Nanosecond Time of Flight on Commercial
   Wi-Fi Cards", arXiv:1505.03446 (two-way phase, squared-channel delay peaks,
   CFO correction, 35 bands).
5. **LoSRef** — K. Yamamoto, "Wi-Fi Radar via Over-the-Air Referencing", 
   arXiv:2602.05344 (v1 Feb 2026, v3 Mar 2026): LoS path as OTA delay/phase
   reference; 160 MHz Intel AX210 + PicoScenes; target dynamics 20 dB below
   static clutter; phase-coherent delay–Doppler without wired reference.
6. **ESP-NOW rate/latency** — ESP-IDF issue 3238, "ESPNOW Round trip takes 6 ms"
   (default 1 Mbps; `esp_wifi_internal_set_fix_rate(..., MCS7_SGI)` ⇒ < 700 µs
   RTT): <https://github.com/espressif/esp-idf/issues/3238>; public API
   `esp_now_set_peer_rate_config` (HT20, MCS0–7) in
   <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/network/esp_now.html>;
   community dual-S3 benchmark (median 3.6 ms, P99 12.4 ms):
   <https://github.com/RichJ233/ESP32-wireless-link-benchmarks>.
7. **ESP32 CSI layout and LTF structure** — ESP-IDF Wi-Fi vendor features (CSI
   from LLTF/HT-LTF/STBC-HT-LTF; per-LTF buffer order):
   <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/wifi-driver/wifi-vendor-features.html>;
   esp-csi repo and ESP-Techpedia (esp-crab common-crystal two-chip sync; C3
   transmits ESP-NOW while S3 receives CSI through a clock buffer;
   Chip CSI ranking C5 > C6 > C3 ≈ S3 > ESP32):
   <https://github.com/espressif/esp-csi>;
   <https://docs.espressif.com/projects/esp-techpedia/en/latest/esp-friends/solution-introduction/esp-csi/esp-csi-solution.html>.
8. **ESP32 CSI phase behaviour** — esp-csi issue 121 (stable slopes, varying
   intercepts from residual CFO; timing stability): 
   <https://github.com/espressif/esp-csi/issues/121>; Wi-ESP (JCDE 2020,
   DOI 10.1093/jcde/qwaa048) phase-usage table ("usable phase values").
9. **CFO measurement from LTFs** — Tsinghua *Hands-on Wireless Sensing* CSI
   sanitisation notes (multiple HT-LTFs 4 µs apart; CFO from phase difference;
   STO/SFO limits): <https://tns.thss.tsinghua.edu.cn/wst/docs/sanitization>.
10. **Distributed-array OTA phase sync** — "Breaking the TDD Flow for
    Over-the-Air Phase Synchronization in Distributed Antenna Systems",
    arXiv:2504.11411 (2025: bi-directional phase measurements; Kalman phase
    tracking; frequent re-estimation essential); BeamSync, arXiv:2311.11070
    (periodic re-sync; LO drift vs gains); AirSync, arXiv:1205.6862 (real-time
    phase correction within a slot; few-degree coherence on WARP).
11. **802.11bf** — C. Chen et al., "An Overview on IEEE 802.11bf: WLAN Sensing",
    arXiv:2207.04859 (SIFS-separated sensing sounding frames; TB/non-TB sensing
    instances).
12. **Two-way time/phase transfer precedents** — Optical two-way time-frequency
    transfer, Nature Photonics 2013, DOI 10.1038/nphoton.2013.69; carrier-phase
    TWSTFT over 9 000 km, arXiv:1403.3193; GPS carrier-phase time transfer,
    NIST tf.nist.gov/general/pdf/2712.pdf (100 ps). Evidence that the two-way
    principle is the standard route to clock-phase transfer across free space.
13. **Subspace/common-phase estimation** — e.g. "Subspace Tracking Algorithms
    for mmWave MIMO Channel Estimation", arXiv:1704.06765; OFDM common phase
    error (CPE) estimation literature (closest art for N4).
14. **Corpus** — `00-synthesis.md` and notes `01`–`20` of this folder, in
    particular `07` (legal/primitives), `08`/`09` (geometries; TDM necessity;
    polyhedron), `10`/`11`/`12`/`17`/`20` (reciprocity, common reference,
    oscillator drift, time precision, gauge).

---

## 13. One-paragraph statement

The home becomes one sensor not by inventing new electromagnetics but by
discovering that **coherence on commodity silicon is a scheduling problem
measured by one curve**: strobe reference frames between sensing frames at the
native 100 µs–1 ms cadence, fix each node's LO phase from the channel-free
two-way ratio, extract the common-mode clock from the aperture's own channel
vector, and let the hub solve the phase history in batch over ns-timestamped
CSI. Whether stock ESP32-S3 suffices — or whether a US$2 VCTCXO, one coax
splice, or a focused reference beam is required — is decided by a two-node bench
that measures the exchange residual and inverts the chip's CFO-correction
artefact. Everything else in Aquila v2 already waits on that number.


