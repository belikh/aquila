# 22 — Invention D: Clockless Anchoring

### Passive phase anchors that make a home of ESP32-S3 pucks one coherent 3D sensor — the fourth escape, its mechanism, and the single experiment that decides it

Status: invention proposal + boundary adjudication (agent `d`). Australian English.
Grades per claim: **ESTABLISHED** (published/demonstrated; hardware tier noted),
**HYPOTHESIS** (physically argued, testable, not yet demonstrated), **SPECULATION**
(plausible, unsupported). Hardware tiers: **ESP32-S3 / ESP32-family / other COTS
Wi-Fi / SDR-grade / radar-metrology**.
Inputs read in full: `BRIEF.md`, `00-synthesis.md`. Prior context: notes `01`–`20` of
this folder; sibling proposals `22-invention-{a,b,g,h,i,m,n,o,p,q,r,s,t}.md` were
surveyed for overlap and are credited where they converge (§2.2). Cross-references `NN`
are to these notes. Primary sources are listed in §10.

---

## 0. Answer in one page

**Verdict.** The space is **not** closed by physics. In 2026 the first two claims the v2
synthesis left open have been answered in the field: wireless phase coherence between
independent ESP32-family nodes is demonstrated end-to-end (ESP-PPB, Feb 2026: FTM-
disciplined VCTCXO, ~1 ppb, **< 1° best / < 10° typical per-frame phase** [S1]), and
per-packet phase referencing on *unmodified* COTS Wi-Fi — without any wired or
cooperative reference — is demonstrated (LoSRef, Feb 2026 [S2]). What remains is not a
physics wall but a *design choice about where the reference lives*.

**The invention — Clockless Anchoring.** Do not manufacture coherence in the radios;
**manufacture it in the room.** A handful of cheap, clockless, *passive* objects (a foil
trihedral corner reflector, a metal picture frame, a switched "anchor tag") are placed at
known or self-estimated positions. For every packet, a receiver measures its own
acquisition phase — the one quantity that makes unsynchronised cross-node CSI useless —
*by referencing the packet's channel estimate to the known-geometry component of its own
channel*: the anchor echo. The reference is **one physical object shared by geometry**,
not a transmission shared by time, not a cable, and not a reciprocal exchange. Because
every node of the home references the *same physical anchors*, the whole mesh inherits a
common phase gauge without any radio-harmonisation protocol at all. I name this the
**fourth escape** (the corpus's three are a common transmitter, OTA reciprocity, and a
conducted reference — `BRIEF`; `20`); it is the cheapest escape: the anchors are passive,
cost ~$1, consume no power, and require no firmware partnership between nodes.

**Mechanism in one line.** Within a single received packet, the acquisition offset
(FFT-window timing, applied CFO/SFO correction, PLL state) is *common to the whole channel
estimate*; therefore, referencing each packet's CSI to any component of that same packet
whose *geometric* phase is known (a surveyed anchor echo — or the LoS path, or a mapped
wall) yields the node's **absolute per-packet channel phase**. A lattice of shared anchors
turns a *radio* problem (carrier phase across independent LOs) into a *geometry* problem
(anchor positions known to cm; static position errors are static phase biases, and static
biases either cancel in temporal sensing or are removed by the estimator itself).

**The single decisive experiment — ANCHOR-1** (§4): two ESP32-S3 boards 3–6 m apart, no
RF cable between them, one 20 cm foil trihedral on a wall; measure the per-frame phase of
the CSI referenced to the anchor echo over 30 min in a furnished room with a person
walking, plus a λ/4 (3.13 cm) geometry-step test. **Pass:** σ_φ ≤ 0.3 rad and
geometrically consistent steps → the home can be made coherent with a sticker. **Fail:**
σ_φ ≳ 1 rad or unexplained walk → fall back to the demonstrated wireless reference-frame
scheme [S1] and the named boundary of §6 holds.

**Cost.** Anchor: $0.50–5 (foil/PCB; $5–15 switched). Nodes: unchanged ESP32-S3 ($3–5 per
face). Trial: ~$100–200.

**Transmit needed?** Not for the anchors (passive). The cooperative mesh should transmit
scheduled ESP-NOW/HT frames (best cadence and identity), but a receive-only, ambient-
illuminated variant works in principle because the reference is geometric, not radiometric
(§3.3). This is the only escape in the corpus that works *without any node-to-node radio
partnership*.

---

## 1. The boundary, re-adjudicated (October 2026)

### 1.1 What the v2 corpus assumed

`00-synthesis.md` §1 and note `20` formalise the gauge: with independent LOs, only
`θ_i − ψ_j` is observable per link; absolute cross-node phase requires an added reference;
the escapes are a common transmitter, OTA reciprocity, and a shared/conducted reference;
"none of the wireless-only escapes is yet proven on ESP32-S3" (`BRIEF` shared context).

### 1.2 What the 2026 primaries changed

| Claim | Status after this search | Source |
| --- | --- | --- |
| Wireless, battery-powered phase coherence between independent ESP32 nodes | **ESTABLISHED (ESP32-C3, open source, GPL-3.0)**: FTM tracks clock drift; VCTCXO disciplined by dual 12-bit DACs to ~1 ppb typical (0.1 ppb in enclosure); each external CSI frame followed 1 ms later by an AP *reference frame*; slave records both; single-frame phase accuracy < 1° best / < 10° typical | ESP-PPB [S1] |
| The ESP32 PHY silently applies CFO correction; mismatched corrections corrupt CSI | **ESTABLISHED [S1]**: with 1 ppb relative clocks "the internal CFO correction is always the same on both sides, so CSI is clean" | [S1] |
| Nanosecond RX timestamps on ESP32 | **ESTABLISHED**: hidden fields exposed by patch (`hack_struct.patch`); FTM-capable chips (C3 and later) | [S1]; note `17` |
| Conducted multi-board phase-coherent ESP32 arrays | **ESTABLISHED (ESP32-S2)**: star topology — master SYNC OUT → PA → splitter → slaves; frequency-multiplexed 40 MHz + 2.4 GHz "reference WiFi frames" on one coax; `calibrate(per_board=False)` with cable lengths | ESPARGOS [S3,S4] |
| The PLL re-randomises its phase per boot/channel, not per packet | **ESTABLISHED (stated) / contested (community)**: ESPARGOS measured re-randomisation after reset and channel change; a community "engineer's guide" gist interprets this as per-packet and concludes cross-node coherent phase "fails as stated" | [S3]; gist [S9] — **conflict flagged; §9 R1** |
| Unmodified COTS Wi-Fi can be phase-aligned without wires or reference antennas | **ESTABLISHED (other COTS Wi-Fi, 2026)**: LoSRef identifies the LoS CIR tap and aligns it to the known Tx–Rx distance per packet; phase-coherent CIRs with delay–Doppler, gait and respiration demos, single-port RX | [S2] |
| Two-way carrier phase recovers range and clock without a common reference | **ESTABLISHED (metrology/SDR)**: carrier-phase TWSTFT (ps-class); dual one-way ranging; Wi-Wi two-way interferometry on ZigBee/USRP ~2.2 ps (≈ 1 mm) | [S5,S6]; via `11`,`h`,`m` |
| Distributed coherent apertures self-cohere on a common *passive* scatterer | **ESTABLISHED (radar, SDR)**: "coherent-on-receive" DCAR synthesis using a dominant scatterer; multi-scale DCAR for unambiguous angle; corner reflectors the standard calibration objects of imaging radar | [S7,S8,S11,S12] |
| Commodity-mmWave arrays self-calibrate on *ambient* reflectors | **ESTABLISHED (mmWave, 2026)**: AutoCalib finds "Ambient Radio Anchors", ~96–100 % of corner-reflector calibration performance | [S10] |
| Mm-level differential ranging on commodity Wi-Fi via sum-carrier-phase | **ESTABLISHED (other COTS Wi-Fi, JSAC 2024)**: WiDRa < 1 mm differential-range RMSE | [S13] |
| cm-level absolute ranging from narrowband multi-tone phase in multipath | **ESTABLISHED (BLE/COTS-class)**: phase-based ranging, MUSIC + spatial smoothing, median ≈ 10 cm at 80 tones; InPhase MAE 0.149 m | [S14,S15] |
| ESP32-S3 can emit an unmodulated 2.4 GHz carrier (a tone) | **ESTABLISHED (vendor API)**: `esp_phy_wifi_tx_tone()` ("Wi-Fi Carrier Wave (CW) TX"), `esp_phy_bt_tx_tone()`; S3 `cert_test` example | [S16,S17] |

**Consequence.** The synthesis's "undemonstrated on S3" clauses have narrowed to
chip-family bookkeeping (ESP-PPB runs on C3; C5 is Espressif's own phase-sync vehicle via
esp-crab [S18]) rather than physics. What the corpus calls M2 — the per-exchange residual
phase — is now better understood: it is **the receiver's per-packet acquisition phase**, an
*implementation* quantity (unreported timing/CFO state), not a gauge theorem. It can be
crossed by *anything that supplies one known phase carrier per packet*. Three crossings are
demonstrated (cable [S3]; reference frame from a common transmitter [S1]; known LoS
geometry [S2]). This note adds and develops the fourth: **a known object**.

### 1.3 The quantity that actually closes the space (if anything does)

> **φ_acq — the receiver's per-packet acquisition phase**: the phase error introduced by
> (i) the FFT-window position chosen by the packet detector/timing synchroniser, (ii) the
> digitally applied CFO/SFO corrections, (iii) the PLL phase state at that instant.
> Nothing in a commodity ESP32 exposes these to the application in calibrated form. It
> differs per packet at an unknown rate (community reports conflict; ESP-PPB's working
> < 10°/frame result implies it is *stable enough to average* at least over 1 ms, and
> possibly *deterministic* per packet [S1]).

**The exact adversarial statement.** *Transmit-free, reference-free, unsynchronised
cross-node carrier phase on commodity silicon is blocked — not by the U(1) gauge, which is
crossable, but by the unreported acquisition state.* The minimal reference that crosses it
is **one known fact per packet** — and the cheapest known fact is a passive object at a
known place. A cable, a beacon frame, a wall, or a sticker: each is a different price for
the same bridge. §6 states the fallbacks if the anchor route also fails.

---

## 2. The invention

### 2.1 The physical mechanism, step by step

**(a) The observable: the anchor-referenced packet phase.**
Model one received packet at node B. The CSI estimate on subcarrier k is

    ĥ_B(k) = Σ_p c_p(k) e^{-j2πf_k τ_p} · e^{j(φ_A(t_tx) − φ_B(t_win))} · e^{j φ_acq,B} · a_A(k) b_B(k)

where the sum is over propagation components p (direct, walls, furniture, the anchor),
`τ_p` their delays, `c_p` their complex coefficients, `a_A`, `b_B` the TX/RX chain
responses, `φ_A(t_tx)` the transmitter's LO phase at its own transmission instant,
`φ_B(t_win)` the receiver's LO phase at its (packet-specific) FFT-window instant, and
`φ_acq,B` the *acquisition phase* — the residual of the timeline the receiver itself
chose (detector window position, applied CFO/SFO corrections, PLL state) **which is
common to every subcarrier and every propagation component of that one packet**
[ESTABLISHED: this is the standard structure of the commodity-CSI phase defects; see
S19–S22 and note `17`].

Now suppose one component of the *same packet's* channel — the anchor echo — has a phase
that is a **known, clockless, geometric quantity**:

    τ_anchor = (|A→X| + |X→B|)/c  and  c_anchor(k) known (metal scatterer: real, geometry-only)

for an anchor X at a surveyed position. Then

    arg(ĥ_B(k)) − arg(c_anchor(k) e^{-j2πf_k τ_anchor}) = [residual channel structure]
                                                        + φ_A(t_tx) − φ_B(t_win) + φ_acq,B + chains.

The unknown per-packet nuisance on the right — **LO and acquisition, lumped** — is now
*measured* (not modelled) every packet, without any assumption that it cancels. The rest
of the packet's channel is expressed **relative to a geometric fact about the physical
world**, not relative to a radio. This is the whole trick, and it is the generalisation of
LoSRef [S2] (which uses the *direct path*, if present and surveyed, as that fact) and of
radar's corner-reflector calibration [S11,S12] and DCAR's dominant-scatterer
"coherent-on-receive" synthesis [S7,S8] to *deliberately placed objects and to Wi-Fi CSI*.

**(b) The lattice: a shared gauge across nodes.**
Two nodes A and B that both observe the *same physical anchor* X, and that both know X's
position, each correct their own packets with the same geometric quantity. Their relative
phase is therefore fixed **by geometry alone** — no common transmission, no exchange, no
clock. With T anchors visible to each node, the correction is over-determined and robust
(two anchors also let a receiver estimate its own acquisition phase *and* check the
consistency of its geometry knowledge). The set of anchors is a **metrology frame**: it
plays the role RTK base stations play for GNSS, except that the anchors are clockless
*reflectors*, so nothing in the frame can drift.

**(c) Why this is a fourth escape.**
Note `20` (and `BRIEF`) list three escapes: a common-transmitter reference (a shared
*transmission*), OTA reciprocity (a shared *round trip*), and a shared/conducted reference
(shared *hardware*). The anchor route requires **neither a radio partner nor a wire**: the
shared thing is an inert object. Nothing about it needs to transmit, agree on a schedule,
or run Aquila firmware. In the taxonomy of `20` it is the *only* escape whose cost does not
grow with the mesh (no per-link beacons, no per-pair exchanges, O(anchors) total).

**(d) Identification: which tap is the anchor?**
The receiver must know *which* part of its channel estimate corresponds to X. Three
mechanisms, in increasing cost:

1. **Surveyed LoS.** If the direct path is present and node poses are known, the earliest
   resolved tap is the reference (LoSRef [S2]). ESTABLISHED, but fails in NLoS rooms.
2. **Dominance.** Make the anchor echo the dominant component in its delay cell: a 20–25 cm
   trihedral corner reflector has max RCS ≈ 12πa⁴/λ² [S11] — 3.9–9.4 m² (5.9–9.7 dBsm) at
   2.4 GHz, i.e. human-scale or above, for a foil-on-cardboard object (both citations and
   the formula: [S11,S12]). Then arg(ĥ) ≈ anchor phase + a *static* clutter bias that
   calibration absorbs. HYPOTHESIS; the clutter bias and its motion-induced wobble are
   measured in ANCHOR-1.
3. **Switched/identity anchors** ($5–15): a pin-diode or mechanical switch on the tag, or a
   backscatter-style modulator, toggles the echo; *amplitude* differencing between
   consecutive packets identifies the anchor tap unambiguously (amplitude needs no phase
   coherence), and a code gives the anchor an identity. Chipless-RFID/backscatter
   technology supplies the parts [S23]; the *use* as a mesh phase reference is what is new
   here.

**Delay separation is optional, not required.** Geometric excess path
`Δ = |A→X|+|X→B|−|A→B|` is only 3–6 ns for wall-mounted reflectors (e.g. X 2 m off-axis on
a 4 m link ⇒ Δ ≈ 1.7 m ≈ 5.5 ns), below even the coherent-stitched 12 ns resolution of the
full 2.4 GHz band; so resolvability is not the workhorse — *dominance and identification*
are. (Engineered delay tags — coax or SAW delay lines — can separate the tap by tens of
nanoseconds but add loss, size and a temperature-dependent delay, ≈ +2.6°/K estimated for
a 30 ns PTFE-coax line; SPECULATION, not needed for the core claim.)

**(e) Self-survey.**
Each anchor's sub-metre position is enough to *cohere* the mesh (residual position error
appears as a *static* phase bias per anchor, not noise); the same mesh can then refine
anchor positions jointly with node poses by bundle adjustment over many packets and many
links (the estimator of §2.4). Initial positions come from a tape, FTM (1–2 m on S3
[`15`,`17`]), or phase-based ranging between nodes through the anchors (multi-tone PBR:
cm-level absolute range, mm differential — [S13,S14,S15]). **This is the 'joint discovery
of node geometry and targets' required by the mission, with the anchors as the fixed
points that make the problem well-posed across homes** — a room's walls differ, but a
room's *anchors are the same type of object*.

### 2.2 What is genuinely new (precise claim list, with the closest prior art)

| # | Claim | Closest prior art | Verdict |
| --- | --- | --- | --- |
| N1 | A *passive clockless object* at a known position is used, **per packet**, as the phase reference that de-nuisances a commodity Wi-Fi node's acquisition phase, without any radio partner. | LoSRef [S2] uses the pair's own LoS path per packet — same principle, but requires LoS and works per link; radar uses corner reflectors for *array* calibration [S11] and dominant scatterers for cross-aperture coherence [S7,S8]; AutoCalib does it for mmWave arrays against ambient objects [S10]. | **New for Wi-Fi as an engineered, deployable object.** The *principle* is a generalisation; the *instantiation and the per-packet Wi-Fi mechanism* are new. |
| N2 | A **lattice of shared anchors** supplies a *common gauge to an entire unsynchronised mesh*: coherence-by-common-geometry, with no beacons, exchanges or cables; cost does not grow with N. | DCAR dominant-scatterer referencing coheres *radar apertures* [S7,S8]; the corpus's three escapes all grow with the mesh or its protocol (`20`,`11`). | **New as a Wi-Fi system property.** HYPOTHESIS pending ANCHOR-1. |
| N3 | Anchor **identity/dominance engineering**: switched/backscatter tags to identify the reference tap without phase information (amplitude differencing), and RCS sizing rules (trihedral `12πa⁴/λ²`) to dominate indoor clutter. | Chipless RFID/backscatter tags [S23] used as localisation targets; corner-reflector sizing is textbook [S11,S12]. | **New combination (identification + phase role).** |
| N4 | **Anchors as the transfer anchor for learning**: sensing features and learned models expressed in the anchor frame (metres/degrees from the lattice), so models port across homes by construction. | Corpus's gauge-invariant features and cross-room transfer (`19`,`h`,`b`); SAR/RTK analogues. | **SPECULATION** (testable, but needs a separate programme). |
| N5 | The **boundary re-adjudication** (§1): φ_acq named as the true commodity blocker; LoSRef/ESP-PPB as demonstrated crossings; the "one known fact per packet" minimal-reference statement. | Corpus `20` (gauge), `17` (timing), sibling `m` (LoSRef cited), sibling `r`/`q` (scene referencing). | **Synthesis + sharpening.** |
| N6 | **OTA self-calibration of the polyhedron**: because the 20 faces share a clock and each face is an independent radio, the puck can calibrate its own face-to-face phases *over the air* through its interior, replacing part of the hub's 2.4 GHz distribution burden (the premise's hardest RF task) with geometry. | ESPARGOS calibrates per-face phases via the conducted reference [S3]; the PLL re-randomises per boot/channel, so a *per-boot* phase calibration is mandatory either way. | **HYPOTHESIS**; a strictly optional hardware simplification (§3.4). Not original to the corpus (sibling `i` touches mutual coupling), so claimed only as a corollary. |

**I explicitly disclaim** as *not* new: the U(1) gauge result; the three escapes; the
principle of geometric referencing (LoSRef/DCAR/AutoCalib); two-way products and closure;
near-field curvature ranging (siblings `h`,`q`,`s`,`t`); polarisation diversity from face
orientation (`b`,`i`,`m`,`n`,`q`,`r`,`s`,`t`); multi-scale ambiguity resolution (DCAR
[S8]); the polyhedron as a rigid known-geometry ruler (`m`). Where this note converges
with those, it cites them.

### 2.3 What must be true (assumptions and their falsifiers)

| # | Assumption | Grade | Falsifier |
| --- | --- | --- | --- |
| A1 | φ_acq is common to all subcarriers and components of one packet, and therefore cancels in a within-packet geometric reference. | **ESTABLISHED** (structure of CFO/SFO/PDD in commodity CSI [S19–S22]); the *magnitude* of the leftover reference error is what ANCHOR-1 measures. | Within-packet relative phase between two geometrically known components is itself unstable (> 0.3 rad over minutes). |
| A2 | At least one known-geometry component exists in typical furnished rooms, per link, with sufficient SNR. | **HYPOTHESIS**: direct path (LoSRef) OR a mapped wall OR an engineered anchor; the anchor makes it *guaranteed* if placed deliberately. | In 5 rooms, no anchor placement yields a stable identifiable component on a majority of links. |
| A3 | A 20–25 cm trihedral achieves anchor-to-clutter dominance in the majority of home links (RCS 5.9–9.7 dBsm [S11]). | **HYPOTHESIS** (indoor clutter/human RCS at 2.4 GHz poorly documented — see §9). | Measured anchor tap SNR < 0 dB relative to same-cell clutter in the target rooms. |
| A4 | The anchor's complex response is stable (metal: geometry-only, micron/°C expansion; mount flex negligible). | **HYPOTHESIS** (textbook, unmeasured here as a phase reference). | Anchor-referenced phase walks without an environmental cause. |
| A5 | Anchor position errors act as *static* biases (harmless for temporal sensing; removable by self-calibration for absolute imaging). | **ESTABLISHED reasoning** (linear phase in delay/geometry). | Anchor-referenced phases for different links do not close geometrically (§4 arm iv). |
| A6 | Switched-tag identification works without phase coherence (amplitude differencing). | **HYPOTHESIS**; backscatter amplitude modulation is established [S23]. | Tag on/off amplitude contrast < 3 dB in-room. |
| A7 | The mesh can aggregate and process per-packet anchored ratios at sensing cadence (20–200 Hz × pucks × faces). | **HYPOTHESIS** (compute; hub aggregation exists by premise). | Node/hub throughput cannot sustain the rate with margin. |

### 2.4 The estimator (sizing)

Per receiver, per packet: (i) estimate the CIR from all available subcarriers
(one 20/40 MHz frame; optionally coherent multi-channel snapshots, siblings `i`,`m`); (ii)
identify anchor taps (surveyed delay, dominance, or switch code); (iii) compute the
anchored phase `ψ̂(k) = arg(ĥ(k)) − arg(anchor model)`; (iv) feed `ψ̂` into the mesh-level
joint estimator:

    minimise over {node poses (6-DoF each), anchor positions (3 each), chain phases (per
    node per channel), clock/LO nuisance (per node per epoch), scatterer positions and
    amplitudes (per epoch)}:
    Σ || anchor-ratio residuals ||² + Σ || link-channel residuals ||² + geometric priors

with integer-ambiguity handling for ranging terms (λ/2 at 2.4 GHz ⇒ 6.25 cm; resolved by
FTM/PBR coarse ranges [S13,S14]). This is a bundle adjustment; it is the same class of
problem as distributed-array self-calibration [S24] plus multipath SLAM (virtual anchors,
`16`,`n`), with the anchors making the gauge explicit. Estimator detail and convergence
are deliberately left to implementation; the *physics* does not depend on the algorithm,
which is the point of an invention note.

---

## 3. The system: a home of pucks, anchored

### 3.1 The polyhedron's role (as given by the premise, used honestly)

Each puck (≈ 30 cm, 20 faces, one S3 per face, hub with shared 40 MHz + a phase reference
+ aggregation) is, first, a **coarse but unambiguous full-sphere 3D aperture**: 20
coherent elements on a sphere give ~λ/D ≈ 24° bearings with no scan loss and no
front/back ambiguity — the *coarse unambiguous scale*. ESPARGOS proves the conducted
coherence architecture at board scale on ESP32-S2 [S3,S4]; esp-crab is Espressif's own
C5 phase-sync vehicle [S18]. Second, because each face is an independent radio on a shared
clock, a puck can run **simultaneous receive on all 20 faces and transmit on any subset**
— no TDM *inside* the puck, unlike a switched single-chain array; this is the reason to
build the object at all (sibling `m` reaches the same conclusion for a different
invention). Third, with 20 faces and 13 channels, the puck can hold **multi-channel
coherent snapshots** (faces partitioned across channels; per-channel PLL phases calibrated
once per boot through the hub reference [S3]) — the coherent 83.5 MHz aperture of the
2.4 GHz band, worth 1.8 m delay resolution and, more importantly, multi-tone phase
continuity for anchoring and ranging.

### 3.2 The protocol (cooperative mode)

A compliant, legal schedule (all frames are ESP-NOW/HT or standard 802.11 as in esp-csi;
`07`):

1. **Anchored ping** (per epoch, per channel of interest): each node transmits one
   ESP-NOW HT40 MCS0 frame in its slot (TDM across nodes; identity in the MAC). Every
   node that hears it stores CSI + ns RX timestamp.
2. **Per-packet anchoring**: each receiver resolves the anchor reference in the frame's
   CIR and outputs an anchored phase for every subcarrier (geometry-referenced, no
   synchronisation state).
3. **Mesh solve**: hub(s) collect anchored phases; update node/anchor geometry, scatterer
   estimates, and (optionally) a wireless reference-frame correction for the residual
   after anchoring (ESP-PPB-style, at a much lower rate than sensing — anchoring already
   removes the bulk [S1]).
4. **Sensing epochs** (20–200 Hz): anchored phases are differenced against the static
   background; moving scatterers are imaged by near-field matched-field processing over
   the puck apertures and the (now coherent) inter-puck baselines of §3.5.
5. **Refresh schedule**: geometric anchoring is per-packet and needs no refresh; the
   optional radio-side reference-frame correction runs at the measured phase-holdover rate
   (~20 ms at 1 ppb per `12`), costing a minority duty cycle.

### 3.3 The ambient variant (transmit-free sensing)

Because the reference is geometric (a scatterer), **any** 2.4 GHz frame can be anchored:
neighbours' traffic, the home AP, IoT chatter. A puck pair in promiscuous mode can collect
CSI of ambient frames (ESPARGOS does exactly this for sensing [S3]; ESP-PPB anchors an
*external* frame with a reference frame 1 ms later [S1]). In the anchor scheme no
reference frame is needed — the tag echo inside the same external frame is the reference.
The mesh therefore has a *receive-only* sensing mode: TX recommended for cadence,
identity and ranging; not required for coherence.
**Legality of any special emissions:** every frame in the cooperative mode stays inside
`esp_wifi_80211_tx`/ESP-NOW as scoped by `07`. The ESP-IDF CW test tones
(`esp_phy_wifi_tx_tone`) are *lab* facilities; if used at all they belong in shielded or
compliant test configurations, not as a home beacon (§3.4, SPECULATION).

### 3.4 Optional corollaries (flagged, not core)

- **OTA intra-puck self-calibration** (HYPOTHESIS): the 20 faces share a clock; each
  face transmits in turn within the puck, the other 19 receive *through free space* across
  a rigid, factory-known 30 cm geometry. Because the geometry is known, the face-to-face
  channels give the per-face PLL/RF-chain phases directly; the hub's 2.4 GHz distribution
  network can then be simplified (clock only, or none). The physics is the same
  anchoring argument at 30 cm range with enormous SNR. Risk: mutual coupling and
  diffraction on a conducting solid perturb the "known" in-puck channels (unmeasured,
  `08`; sibling `i` flags the coupling matrix as a modality in its own right).
- **CW tone** (SPECULATION): a face can emit an unmodulated carrier [S16,S17]; a puck
  could provide its own tone beacon, and a beacon node could serve as the corpus's
  "common transmitter" escape with zero extra hardware. Legality and utility are open.
- **BLE CTE** (SPECULATION): ESP-IDF ships a Bluetooth 5.1 Constant Tone Extension
  direction-finding example (IQ phase of an unmodulated tone) [S25]; if a future face
  carries a CTE-capable SoC, a *standardised* carrier-phase meter exists on the same
  board. ESP32-S3's BLE (5.0) is not confirmed for CTE; do not design around it.

### 3.5 Sharp 3D: what the anchored mesh can actually deliver

With anchoring (or any escape) the mesh is a coherent array of `20·M` elements spanning
the home. Resolution ledger for a *coherent point scatterer at 3–5 m*, 2.4 GHz:

| Dimension | Property | Number |
| --- | --- | --- |
| Angle (long baselines) | λ/D; D = 5–15 m across a home | ~0.9–2.5°; sub-degree with SNR/multi-snapshot super-resolution |
| Angle (per puck, disambiguation scale) | λ/0.3 m | ~24°, unambiguous, full sphere |
| Depth (near-field curvature) | `δr ≈ 4λr²/(πD²) · σ_φ` (sibling `h`'s paraxial form; near-field CRB lineage [S26,S27]) | D = 8 m, r = 4 m, σ_φ = 0.1 rad → **≈ 4 mm** |
| Range (delay) | c/2B; B = 83.5 MHz coherent across 13 channels | coarse ~1.8 m — used only to resolve fringe order |
| Differential range (phase) | WiDRa-class sum-carrier-phase [S13] | < 1 mm differential on commodity Wi-Fi |
| Absolute inter-node range | PBR multi-tone + FTM integer fix [S13,S14,S15] | cm-scale |

**The honest limit is not aperture and not coherence — it is scene rank.** At 2.4 GHz with
83.5 MHz, indoor scatterers are *not* sparse in (delay, angle); the network must
super-resolve a dense field, and the binding resources are element count × SNR × coherence
time against the *number of significant scatterers per resolution cell* (order 10–30).
That is a *quantitative* boundary, not a categorical one — more faces, more pucks and
longer coherent integration buy it back; and it is exactly the regime where the anchored
phases (stable, per-packet, no protocol) help most, because the mesh can integrate as long
as the scene is static. Where it bites: absolute 3D *of arbitrary furniture* (hard);
mm-level 3D *motion fields of people* (easy — differential, clutter-cancelled by
differencing).

**Sharpness claim to test (SPECULATION→HYPOTHESIS):** with M = 6 pucks and 4 anchors in a
furnished room, a cooperative reflector's *motion* is localisable to cm in 3D at 20 Hz,
and a cooperative reflector's *position* to decimetres; absolute 3D *passive* localisation
to sub-decimetre requires the dense-scene inversion above and is not claimed here.

---

## 4. The single decisive experiment — ANCHOR-1

**Claim under test.** *On ESP32-S3, with no RF cable and no radio partnership, a passive
anchor yields per-frame absolute channel phase stable enough (σ_φ ≤ 0.3 rad over 10 min in
a furnished room with a person walking) that a mesh of such nodes is phase-coherent by
geometry.* This is the one measurement the invention stands or falls on. (It is the
mission's "falsifying experiment" and the corpus's M2 re-aimed at the fourth escape; it
complements, and can run in the same firmware cycle as, the sibling decisive tests
`a`'s Fence and `g`'s R1.)

**Rig (≈ $150).** Two ESP32-S3 devkits (A: TX slots; B: RX), 3–6 m apart, line of sight
in one configuration and NLoS (around a corner/through a doorway) in another; **no RF
cable between them**. One 20 cm foil trihedral on a wall/tripod (RCS ≈ 3.9 m², $1); a
second, 25 cm (9.4 m²) at a lateral offset; optional switched tag (pin diode + coin cell,
$10); a tape/laser for surveying anchor and node positions to ±1 cm; a cheap fan/hairdryer
for a temperature step; a volunteer. Optional: a conducted SMA splitter as an *oracle*
for scoring only (never an input). No VCTCXO required (that is the point).

**Procedure.**
(i) A transmits ESP-NOW HT40 MCS0 at 200 Hz; B logs CSI (114 subcarriers), ns RX
timestamps, RSSI, AGC; 30 min, including 10 min with a person walking and a ≥ 10 °C step.
(ii) Arms, computed offline on the same packets:
 (a) **naked** one-way phase (baseline: expect random/slow-walk ≫ π over minutes);
 (b) **anchored** phase: CIR-referenced to the anchor model (surveyed delay+geometry;
     dominance identification), per packet;
 (c) **anchored+switched**: tag on/off amplitude identification, same phase extraction;
 (d) **LoS-referenced** (LoSRef logic [S2]) as a positive control in the LoS configuration;
 (e) **reference-frame** (ESP-PPB-style [S1]) as the demonstrated fallback arm.
(iii) Geometry test: translate B along a measured direction in λ/4 = 3.13 cm steps over
≥ 10 steps; the anchored phase must advance by 2π·δ/λ for the direct component, and the
*two-path structure* (direct vs anchor) must reproduce the predicted difference — this is
the discriminator against "stable but wrong" phase (a stuck LO would also look stable).
(iv) Blinding: the anchor is moved to an unknown (to the analyst) one of two nearby
positions between runs; the recovered phase difference must match the surveyed geometry.

**Metrics.** σ_φ of each arm over 10 s windows across the 30 min; drift character
(white/ramp/walk); number of packets with resolvable anchor identification; anchor-tap
SNR distribution; temperature-step response; λ/4 step residuals; anchor displacement
recovery error.

**Pass:** anchored arms σ_φ ≤ 0.3 rad (17°) over ≥ 10 min in the furnished, occupied
room, λ/4 residuals ≤ 0.3 rad, displacement recovery to ±1 cm, and anchored arm beating
naked arm by ≥ 10× in σ_φ. → Clockless Anchoring is real; scale to ANCHOR-2.
**Fail:** σ_φ ≳ 1 rad or unexplained walk → the anchor route fails as specified; record
which sub-assumption (A1–A6) broke, and fall back (§6). A partial pass (stable only when
static) still supports the *differential* sensing use.

**ANCHOR-2 (only after ANCHOR-1 passes; not the decisive one).** 6 nodes + 3–4 anchors in
two different rooms; cross-node phase coherence matrix vs the conducted oracle; the
two-room transfer check (same estimator, no retuning); then the 3D motion-field demo of
§3.5. This is where the "generalising" claim gets its first number.

---

## 5. ESP32-S3 feasibility, element by element

| Element | S3 status | Basis |
| --- | --- | --- |
| CSI per frame (LLTF/HTLTF), 56/114 subcarriers at 20/40 MHz | **ESTABLISHED** | esp-csi, ESP-IDF CSI docs [S19]; `01`,`07` |
| ESP-NOW HT40 MCS0 excites CSI; identity; ACK | **ESTABLISHED** | esp-csi; `07`,`00` §8 (the CSI "ping" primitive) |
| ns RX timestamp | **ESTABLISHED (needs a struct patch)** | ESP-PPB [S1]; note `17` |
| Per-frame acquisition phase is *the* nuisance | **ESTABLISHED** as structure; magnitude to be measured here | [S19–S22]; ESP-PPB [S1] |
| CSI phase useful after in-frame referencing | **ESTABLISHED in kind** | LoSRef [S2] (COTS); ESPARGOS [S3] (phase over antennas) |
| Anchored absolute phase per packet | **HYPOTHESIS — this note's core claim** | ANCHOR-1 |
| Passive anchor echoes at 2.4 GHz | **ESTABLISHED physics / HYPOTHESIS in-room** | corner-reflector RCS [S11,S12]; AutoCalib [S10] |
| Switched/backscatter tags | **ESTABLISHED technology; HYPOTHESIS as phase reference** | chipless RFID review [S23] |
| Wireless phase coherence without anchors | **ESTABLISHED (ESP32-C3)** | ESP-PPB [S1] |
| Conducted intra-puck coherence | **ESTABLISHED (ESP32-S2)**; polyhedron form unbuilt | ESPARGOS [S3,S4]; esp-crab C5 [S18] |
| Coherent multi-channel (13-channel) snapshots | **HYPOTHESIS** (per-channel PLL phase calibratable per boot [S3]) | siblings `i`,`m`,`s` converge |
| Near-field curvature / multi-scale 3D | **HYPOTHESIS on S3** (established in array processing and DCAR) | [S8,S26,S27]; `h`,`q`,`s`,`t` |
| CW tone TX (optional beacon) | **ESTABLISHED API** | [S16,S17] |
| BLE CTE phase meter (optional) | **UNVERIFIED on S3** | [S25] |

**Transmit needed?** No for the anchors; yes (recommended) for the mesh's *sensing*
cadence, identity and ranging. A receive-only ambient mode is coherent §3.3. No special
emissions are required by the core proposal; all frames are standard compliant frames
within `07`'s envelope. The CW tone is *not* part of the core and is flagged for lab use.

**Cost (2026 street prices, order-of-magnitude).**

| Item | Cost |
| --- | --- |
| ESP32-S3 module/face | $3–5 |
| 20-face puck BOM (faces + hub + PCB + switch/FPGA/Ethernet) | $100–200 assembled prototype; descend with volume |
| Anchor (foil trihedral, 20–25 cm) | $0.50–5 |
| Switched/backscatter anchor | $5–15 (+ coin cell; years at µW duty) |
| Optional per-node clock discipline (VCTCXO + dual DAC, ESP-PPB-class) | $10–15 |
| ANCHOR-1 trial | ~$100–200 |

**The cost asymmetry is the invention's point:** $5 of foil replaces $100 of cabling or a
whole coherence protocol, *per home*.

---

## 6. If it fails — the exact quantity, the minimal reference, and the fallbacks

If ANCHOR-1 fails, the failure is one of: (i) no stable identifiable known-geometry
component (A2/A3/A6); (ii) within-packet relative phase itself unstable (A1 — would
contradict LoSRef/ESPARGOS-class evidence and the structure of CFO/SFO; a major result if
true); (iii) anchor response instability (A4). The **named quantity** standing between the
home and coherence remains **φ_acq**, and the **minimal reference** that crosses it is
**one known phase carrier per packet**:

| Escape | What is shared | Demonstrated on | Cost per home | Growth |
| --- | --- | --- | --- | --- |
| Conducted clock+tone | hardware | ESP32-S2 (ESPARGOS) [S3] | cables, O(N) | O(N) |
| Common-transmitter reference frame | a transmission | ESP32-C3 (ESP-PPB) [S1] | a master + cadence | O(1) + airtime |
| OTA reciprocity/two-way | a round trip | metrology/SDR [S5,S6]; unproven on S3 | protocol, O(N²) airtime | O(N²) |
| **Clockless anchors (this note)** | **an object** | LoSRef [S2] (LoS form); radar/mmWave analogues [S7,S8,S10] | **$5–20 of stickers** | **O(anchors)** |

Fallback order if the anchor route degrades: (1) anchors + reference frames at low rate
(hybrid: anchoring removes the fast phase, the frame removes the residual bias); (2)
ESP-PPB-style wireless frames alone [S1]; (3) conducted pucks with radio-side incoherent
fusion (the corpus's Track 3 baseline). Even in the worst case the mission's degraded
product survives: per-puck 3D bearings + FTM + anchored *differential* phases (which need
much less than absolute coherence) give a generalising 3D *motion* sensor.

---

## 7. Chaining, existing models/data, own data (per `BRIEF`)

**(A) Chaining.**
- *Anchors enable per-packet absolute phase*; absolute phase enables **cross-node coherent
  products** (AoA across nodes, near-field depth, coherent imaging) — this is the
  highest-leverage edge, and it is the *cheapest* input in the whole dependency graph
  (a sticker, no protocol).
- *Per-packet absolute phase enables geometry self-survey* (phase-based ranging, bundle
  adjustment) without any radio-side synchronisation; *geometry enables multi-scale
  disambiguation*; *disambiguation enables sharp 3D* (the mission's end).
- *Anchors also enable the ambient/receive-only mode*, which removes the TX duty question
  and enables sensing in homes where the sensor must not transmit (privacy/regulatory).
- *Blocked without anchors-or-equivalent*: everything in the coherent column of the
  synthesis's §3 table. The chain is: **anchor ($5) → coherence → geometry → sharpness**.
- The puck's conducted coherence remains the local enabler (faces within one 3D aperture);
  anchors are the *inter-puck* enabler. The two compose: conducted inside 30 cm, anchored
  beyond it.

**(B) Existing models and data.** The scheme needs no trained model to function; it
changes the *input representation* (anchored, geometry-referenced phases) that any sensing
model consumes. Reusable assets from `00` §5 apply unchanged (SenseFi, MM-Fi, CSI-Bench,
OpenCSI, ESPARGOS datasets; `19`). The LoSRef/DCAR/AutoCalib literature supplies the
reference-extraction and coherence-combination algorithms [S2,S7,S8,S10]; the corpus's
channel-charting route (`03`) and cross-room transfer (`19`) supply the label-free and
transfer machinery; DCAR supplies multi-scale unambiguous angle estimation [S8]. Nothing
in the anchor idea invalidates any of them; it removes their synchronisation precondition.

**(C) Own data / new technology — the minimal dataset and experiment.** ANCHOR-1 *is* the
minimal dataset: two S3s, one anchor, 30 min, five arms, in two room configurations
(LoS/NLoS), with the λ/4 geometry test and a blinded anchor displacement. Its falsifier is
stated in §4. The first *new-technology* artefact is the anchor itself: a printed anchor
kit (trihedral + switched variant) with a published RCS/phase-stability characterisation
at 2.4 GHz — which does not exist for Wi-Fi and is a plausible standalone contribution.
The second is the anchored-phase representation and the joint estimator of §2.4, published
with the dataset. What would falsify the *programme* (not just the experiment): if
anchored phases are stable *and* geometry-consistent but the mesh still cannot beat
non-coherent fusion in imaging, then the binding constraint really is scene rank (§3.5),
and the invention's value collapses to the *coherence layer* alone (still an enabling
result, not a sensor).

---

## 8. Evidence strength

**6.5/10** for the invention as stated (was 0: it did not exist in this form). Justification:
- The *structure* of the claim rests on established physics (within-packet phase
  commonality [S19–S22]; corner-reflector metrology [S11,S12]; per-packet geometric
  referencing demonstrated on COTS Wi-Fi [S2]; dominant-scatterer cross-aperture coherence
  in radar [S7,S8]; ambient-anchor calibration in mmWave [S10]; wireless ESP32 coherence
  and its numbers [S1]; mm differential ranging on COTS Wi-Fi [S13]).
- The *specific integration* (latticed passive anchors as a Wi-Fi mesh gauge, per-packet,
  no partnership) is new and **undemonstrated**; it is gated by one cheap experiment whose
  pass/fail is sharp.
- Primary-source density is high and recent (2024–2026); the main uncertainty is the
  indoor anchor-to-clutter ratio and the acquisition-phase magnitude on S3 specifically —
  both measured, not argued.

**Score by sub-claim:** mechanism 8/10 (structure established, magnitude open); mesh
gauge 6/10 (new composition); sharp-3D consequence 5/10 (scene-rank limits; convergent
with siblings); feasibility on S3 7/10 (all primitives vendor-documented or
family-demonstrated); originality vs prior art 7/10 (LoSRef/DCAR/AutoCalib are close
ancestors; the engineered lattice and Wi-Fi instantiation are new).

---

## 9. Residual unknowns and required follow-ups

1. **φ_acq magnitude/structure on S3, per packet** — the community conflict (§1.2, gist
   [S9] vs ESP-PPB [S1]) must be resolved by measurement; ANCHOR-1's naked arm settles it.
   *If φ_acq were truly i.i.d. per packet at ~2π scale, then even within-packet referencing
   would fail (A1), and LoSRef's results would be inexplicable — a decisive contradiction
   worth publishing either way.*
2. **Indoor clutter vs anchor RCS at 2.4 GHz** (human RCS, furniture clutter statistics) —
   poorly documented; measured in ANCHOR-1.
3. **Anchor phase stability**: mounting flex, temperature, occlusion by furniture/people —
   a characterisation dataset (the anchor kit).
4. **Bistatic anchor response**: trihedrals are optimum for *monostatic* RCS; bistatic
   response (A TX → anchor → B RX at arbitrary angles) is reduced and shape-dependent
   [S12 notes passive bistatic calibration targets are an open problem]. A dihedral, plate
   or purpose-designed bistatic tag may be better; **do not assume monostatic RCS transfers
   to the bistatic link** — this is the weakest physics link in the note and is measured
   first in ANCHOR-1 (anchor-tap SNR on non-LoS links).
5. **Joint estimator convergence** (anchor+node geometry, ambiguities) — simulations, then
   ANCHOR-2.
6. **Polyhedron mutual coupling/shadowing** — unmeasured (`08`,`00` §9); affects the
   OTA self-calibration corollary and the 20-face manifold.
7. **Legal envelope for anchors**: fully passive — no emission — so no change to `07`'s
   conclusions; switched tags with a coin cell are unintentional radiators at µW — within
   class-licence fields, but flag for ACMA review.
8. **Where the fourth escape sits in the corpus's CRB**: the minimal-reference result of
   `20` (one constraint per component, σ_φ ≳ 1/√(2N_sρ) MCRB) should be re-expressed for a
   *geometric* reference: the constraint is a known *length*, so the equivalent statement
   is "one known interferometer baseline per component"; the anchor lattice is a
   physical realisation of that. (Analytical follow-up.)

---

## 10. Sources (primary; retrieved October 2026)

- **[S1]** J. Muller, "ESP-PPB: 1-PPB wireless sync for phase coherent Wi-Fi CSI", GitHub
  (created 2026-02-19), GPL-3.0. https://github.com/jonathanmuller/esp-ppb ; history:
  https://github.com/jonathanmuller/esp-ppb/blob/main/story.md (FPGA/patch details,
  `hack_struct.patch`, accuracy table, CFO-correction lesson).
- **[S2]** K. Yamamoto et al., "Wi-Fi Radar via Over-the-Air Referencing: Bridging Wi-Fi
  Sensing and Bistatic Radar" (LoSRef), arXiv:2602.05344 (v1 Feb 2026; v3 Mar 2026).
  https://arxiv.org/abs/2602.05344
- **[S3]** F. Euchner et al., "ESPARGOS: An Ultra Low-Cost, Realtime-Capable Multi-Antenna
  WiFi Channel Sounder", arXiv:2502.09405. https://arxiv.org/abs/2502.09405 (PLL phase
  re-randomisation; reference signal distribution; calibration).
- **[S4]** ESPARGOS documentation, "Phase-Coherent Multi-Array Systems"
  https://espargos.net/docs/combined-arrays.html ; datasets: arXiv:2408.16377.
- **[S5]** M. Fujieda et al., "Carrier-phase two-way satellite frequency transfer over a
  very long baseline", Metrologia 51:253–262, 2014; NIST TWSTFT overview
  https://www.nist.gov/pml/time-and-frequency-division/time-distribution/two-way-satellite-time-and-frequency-transfer
- **[S6]** N. Shiga et al., "Demonstration of wireless two-way interferometry (Wi-Wi)",
  IEICE Commun. Express 6(2):77–82, 2017, DOI 10.1587/comex.2016XBL0181; Y. Yamasaki et
  al., IEEE Access 9:85084–85100, 2021, DOI 10.1109/ACCESS.2021.3087866.
- **[S7]** S. Coutts, K. Cuomo, J. McHarg, F. Robey, D. Weikle, "Distributed Coherent
  Aperture Measurements for Next Generation BMD Radar", IEEE SAM Workshop 2006,
  pp. 390–393.
- **[S8]** "Coherent-on-Receive Synthesis Using Dominant Scatterer in Millimetre-Wave
  Distributed Coherent Aperture Radar", Remote Sensing 15(6):1505, 2023,
  DOI 10.3390/rs15061505; "High Accuracy Unambiguous Angle Estimation Using Multi-Scale
  Combination in DCAR", IET Radar Sonar Nav. 11:1090–1098, 2017,
  DOI 10.1049/iet-rsn.2016.0450.
- **[S9]** "ESP32 Wi-Fi-CSI sensing mesh — RF & system-architecture engineer's guide"
  (community gist, Sep 2025) — the per-packet PLL claim; **contested**; retained as a
  flagged conflict. https://gist.github.com/YoraiLevi/efa0ce2b02a26f735ccbf6f7df323de8
- **[S10]** "Automatic Phase Calibration for High-Resolution mmWave Sensing via Ambient
  Radio Anchors" (AutoCalib), arXiv:2506.23472; IEEE TMC 25(8):11885–11899, 2026.
- **[S11]** Microwaves101, "Corner Reflectors" (trihedral RCS `12πa⁴/λ²`, aspect
  tolerance) https://www.microwaves101.com/encyclopedias/corner-reflectors ; A. W. Doerry
  and B. C. Brock, "Radar Cross Section of Triangular Trihedral Reflector with Extended
  Bottom Plate", Sandia/OSTI report, 2009 (formula lineage).
- **[S12]** K. Sarabandi and T.-C. Chiu, "Optimum corner reflectors for calibration of
  imaging radars", IEEE Trans. Antennas Propag. 44(10):1348–1361, 1996; T. Pienaar et al.,
  "Active calibration target for bistatic radar cross-section measurements", Radio Science
  2016, DOI 10.1002/2015RS005931 (passive targets unsuitable for bistatic calibration).
- **[S13]** V. V. Ratnam et al., "WiDRa: Enabling Millimeter-Level Differential Ranging
  Accuracy in Wi-Fi Using Carrier Phase", IEEE JSAC 42(9):2410–2423, 2024,
  DOI 10.1109/JSAC.2024.3413985; arXiv:2405.12168.
- **[S14]** A. Sheikh et al., "Phase-Based Ranging in Narrowband Systems With
  Missing/Interfered Tones", arXiv:2211.00502 (median ≈ 10 cm with 80 tones and MUSIC +
  spatial smoothing).
- **[S15]** "InPhase: Phase-based Ranging and Localization", ACM Trans. Sensor Networks
  18(1), 2022, DOI 10.1145/3494542 (MAE 0.149 m).
- **[S16]** Espressif ESP-IDF, "RF Calibration" API reference: `esp_phy_wifi_tx_tone()`,
  `esp_phy_bt_tx_tone()` (Carrier Wave TX commands)
  https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/RF_calibration.html
- **[S17]** Espressif ESP-IDF, "PHY — ESP32-S3" (`phy/cert_test` example)
  https://docs.espressif.com/projects/esp-idf/en/release-v6.0/esp32s3/api-guides/phy.html
- **[S18]** Espressif, "Introduction to ESP-CSI Solution" (esp-crab common-crystal RF
  phase synchronisation; C3 TX + S3 RX clock-buffer pair)
  https://docs.espressif.com/projects/esp-techpedia/en/latest/esp-friends/solution-introduction/esp-csi/esp-csi-solution.html
- **[S19]** Espressif `esp-csi` (CSI acquisition; ESP-NOW excitation) and ESP-IDF Wi-Fi
  driver CSI tables (20/40 MHz subcarrier counts)
  https://github.com/espressif/esp-csi ; esp-csi issue #121 (per-packet phase offsets,
  vendor response).
- **[S20]** Tsinghua WST, "CSI Sanitization" (radio-chain offset, CFO via multiple
  HT-LTFs, SFO/PDD)
  https://tns.thss.tsinghua.edu.cn/wst/docs/sanitization
- **[S21]** "On Phase Offsets of 802.11ac Commodity WiFi", arXiv:2005.03755 (PLL initial
  phase differs among RF chains; averaging needs many packets).
- **[S22]** "Mitigation of CSI Temporal Phase Rotation with B2B Calibration Method…",
  Sensors 18(11):3828, 2018, PMC6263436 (wired back-to-back reference).
- **[S23]** "Chipless-RFID: A Review and Recent Developments", Sensors 19(15):3385, 2019,
  PMC6695767 (delay-line and resonator tags; phase encoding).
- **[S24]** "Phase Calibration of Distributed Antenna Arrays" (over-the-air calibration;
  multipath coherence-bandwidth condition), arXiv:2304.05144; "Relative channel
  reciprocity calibration in MIMO/TDD systems", EuCAP/VTC 2010 (Bourdoux et al.).
- **[S25]** ESP-IDF Bluetooth example `ble_connection_peripheral_with_cte`
  ("BLE Constant Tone Extension … Direction Finding (AoA/AoD)") —
  https://github.com/espressif/esp-idf/blob/master/examples/bluetooth/bluedroid/ble_50/ble_connection_peripheral_with_cte/README.md
  (chip support unverified for S3).
- **[S26]** Near-field localisation CRB lineage: Fascista et al., IEEE JSTSP, 2025;
  Wang et al., "CRBs for Near-Field Sensing with XL-MIMO", IEEE 2024 (both cited via
  sibling `t`/`q`; not independently re-verified here).
- **[S27]** J. Holl and F. Reinhard, "Holography of Wi-Fi radiation", Phys. Rev. Lett.
  118:183901, 2017 (coherent imaging prior art; via sibling `b`).
- Internal: `00-synthesis.md`; notes `07`,`11`,`12`,`15`,`16`,`17`,`19`,`20`;
  sibling inventions `a` (Fence test), `g` (R1), `m` (LoSRef), `p`,`q`,`r` (scene
  referencing), `h`,`o`,`s`,`t` (closure/near-field), `i`,`n` (polyhedron/geometry).

---

## 11. One-paragraph closing statement

The v1 question — *is there an invention?* — had the shape "find a way to make the radios
coherent". The 2026 evidence says the radios can be made coherent in at least two wireless
ways already. The remaining invention is to notice that **coherence is a property of the
shared physical world, not of the mesh**: a $1 foil trihedral on a wall is a clock that
never drifts, a beacon that never transmits, and a cable that never needs laying; anchor
every packet to it, and a home of ESP32-S3 pucks becomes one phase-coherent 3D aperture
whose sharpness is limited only by scene rank and SNR — a quantitative, purchasable
boundary, not a physical wall.

