# 22 — The invention: the Coherent Polyhedral Mesh (CPM)

Status: full invention proposal for the 22nd prompt. Australian English. Grades:
**[ESTABLISHED]** = primary source showing it works (on any hardware);
**[HYPOTHESIS]** = physically argued, testable, not yet demonstrated;
**[SPECULATION]** = plausible extrapolation, flagged as such. Every claim carries
a source; every primitive carries a falsifier.

Question: *is there an invention that makes a home full of cheap ESP32-S3 nodes
behave as one sharp, self-calibrating, generalising 3D sensor — and if not, what
physical quantity closes the space and what is the minimal reference that
crosses it?*

Short answer, stated up front:

> **The space is not closed.** As of 2026 the two hard primitives exist in
> isolation but have never been combined: (a) **wireless phase-coherent Wi-Fi CSI
> between independent ESP32 nodes** is now demonstrated end-to-end by the
> community project ESP-PPB (FTM + VCTCXO discipline, ~1 ppb, per-frame phase
> reference, AoA working) [ESP-PPB]; and (b) **coherent multi-array ESP32
> capture** is demonstrated by ESPARGOS (wired reference, phase-coherent arrays,
> polarimetry) [ESPARGOS]. Neither does **3D volumetric apertures**, neither does
> **phase-coherent transmit**, neither **stitches bandwidth**, and neither
> **self-calibrates its own geometry**. The invention is the two-tier coherent
> mesh that combines them, and its enabling trick is a small, cheap piece of
> hardware nobody uses: **run the puck's conducted reference network backwards
> as a per-dwell TX+RX chain calibration bus** — which converts the polyhedron
> from a receive-only "camera" into a coherent 3D *transceiver*, and makes
> wideband stitching and coherent illumination possible on commodity silicon.
>
> The quantity that *would* close the space, if it turns out large, is the
> **per-packet cross-node phase residual in multipath on S3** (target:
> ≤ λ/10 ≈ 0.6 rad). The minimal reference that crosses every remaining gap is
> **one common frame per measurement cycle plus one reciprocal exchange per
> link** — both already within ESP32 reach.

---

## 0. What the hardware premise already buys (Tier 1)

The premise — a ~30 cm polyhedron of triangular PCBs, one ESP32-S3 per face,
hub with 40 MHz fan-out, conducted 2.4 GHz phase reference, hub aggregation —
is, in effect, a miniaturised, transceiver-capable ESPARGOS [ESPARGOS]
[ESTABLISHED]:

1. **Frequency coherence across faces** comes free from the shared 40 MHz clock.
   ESPARGOS measured that with only a shared clock the inter-antenna phase
   differences stay stable over minutes in a static environment; fractional
   clock agreement is not enough for phase *coherence* because each chip's PLL
   locks with an unknown initial phase that changes on reset/channel change
   [ESPARGOS, §II-A]. That is precisely why the premise includes the conducted
   2.4 GHz phase reference: a Wi-Fi-format reference frame distributed by a
   resistive divider/microstrip network lets each receiver estimate and remove
   its PLL/hardware offset, `ĥ_CAL,m = ĥ_OTA,m / ĥ_REF,m · e^{−jφ_path,m}`
   [ESPARGOS, §III-B] [ESTABLISHED].
2. **Simultaneous 20-channel coherent receive.** One radio per face means every
   face receives the *same* packet simultaneously: a full 20-element 3D
   snapshot per frame at packet rate, with no face switching in the receive
   path (ESPARGOS likewise has one chip per antenna; its switch is only to
   select the calibration reference).
3. **Known geometry by construction.** Face positions and orientations are
   fixed by the PCB design; the conducted path delays `φ_path,m` are fixed by
   layout and calibrated once [ESPARGOS] [ESTABLISHED]. The intra-puck "array
   manifold" is therefore exact and free of the self-calibration problem that
   dominates camera-style sensor networks.
4. **Polarimetric diversity.** ESPARGOS One shows the ingredient: dual-slant
   patch feeds and a Jones-calculus pipeline that isolates polarisation of
   individual paths in real time [WiFiCamera] [ESTABLISHED on planar hardware].
   A polyhedron of linearly-polarised patches samples polarisation in three
   dimensions instead of two — new on ESP32 [HYPOTHESIS].
5. **Near-field coupling as a modality.** Faces 10–15 cm apart are in each
   other's near field; the complex mutual-coupling matrix among faces is a
   high-dimensional, static fingerprint of the ~50 cm shell around the puck.
   ESP-PPB's Board 9 work is the warning and the proof of richness at once:
   *"you cannot put multiple PCB antennas on the same ground plane and expect
   them to be independent… the fundamental limit of multi-ESP32 single-PCB
   designs"* [ESP-PPB story] [ESTABLISHED]. Coupling is a calibration
   nuisance for far-field work (it is static and invertible — see P1) and a
   new sensing modality for near-field work [SPECULATION].

What Tier 1 alone gives — a 3D, full-sphere, polarimetric *coherent receive*
aperture with a 30 cm baseline (D/λ ≈ 2.5 at 2.45 GHz) — is real but coarse
(~20° beamwidth, a "compound eye"), and does not exist on ESP32 today
[ESTABLISHED that no such array exists; synthesis notes 08/09]. Sharpness at
home scale must come from Tier 2: mesh coherence.

---

## 1. The boundary, restated precisely

The synthesis (note 20) formalises the gauge: with independent LOs the measured
channel is `ĥ_ij = a_i b_j e^{j(θ_i−ψ_j)} e^{−j2πf_kτ}`; only `θ_i−ψ_j` is
observable, the node phases are fixed only up to one global constant per
connected component, and one reference per component fixes it. Larsson's phasor
formalism makes the algebra exact and gives the calibration taxonomy
(R-calibration vs F-calibration, and a rigorous method for *F-calibrating a
joint array* from two individually calibrated sub-arrays: a bidirectional
exchange plus one unidirectional measurement at a second frequency)
[CalTheory] [ESTABLISHED, theory + simulation].

Three physical statements I will use throughout; they bound every design
choice, and they are the honest "boundary":

- **B1 — Reference-and-signal degeneracy.** Any *wireless* reference is itself
  a scene path. Its phase is therefore corrupted exactly when the scene moves
  along the reference path. A wired reference is immune; a wireless one is
  only as good as the *distinguishability* of its reference path from the
  signal. This is not a nuisance to be averaged away — it is a coupling between
  reference quality and scene dynamics [derived here; consistent with
  Channel-SLAM's "virtual transmitter" model [ChannelSLAM] and with BeamSync's
  decision to point its sync signal along the dominant AP–AP direction
  [BeamSync]] [HYPOTHESIS].
- **B2 — Delay-resolution wall at commodity bandwidth.** 20 MHz OFDM resolves
  paths ~50 ns apart (15 m); HT40 halves that; a home's delay spread (20–50 ns
  RMS) is therefore *unresolved* in one channel. Carrier-phase *slope* gives
  millimetre-to-decimetre **precision** for a dominant path [Chronos gives
  0.47 m median; π-Splicer refines phase error structure; ESP32 boards show
  >1 ns delay estimation error vs 0.2–0.7 ns for NICs and 0.12 ns for an x310
  [SameSignalDifferentStory]], but **resolution** of nearby paths needs
  bandwidth. Stitching is the only lever on 2.4 GHz-only silicon [ESTABLISHED
  on Intel NICs: Chronos; π-Splicer; SWAN; not on ESP32].
- **B3 — Clock-quality wall for packet-rate protocols.** The relative phase of
  two free-running oscillators advances at `2πΔf·t`. At ±20 ppm the phase wraps
  every ~10 µs; at 1 ppb it advances 0.015 rad per millisecond. Therefore:
  with raw TCXOs, any two-way exchange must complete inside ~10 µs (hard on
  ESP32); with ppb-class discipline, ordinary millisecond MAC scheduling
  suffices. This one number — `Δf_rel` — decides whether the whole home-scale
  protocol is comfortable or impossible [ESTABLISHED physics; UTC-traceable
  demonstration on ESP32-C3 by ESP-PPB below].

**What has changed since the synthesis was written (important).**
The synthesis states "none of the wireless-only escapes is yet proven on
ESP32-S3". That is *no longer the state of the art for the ESP32 family*:
ESP-PPB (2026, GPL-3.0, ESP32-C3) is a working, battery-powered,
cable-free platform in which each node disciplines its own **VCTCXO** over the
air using **Wi-Fi FTM** and the chip's ~ns RX timestamps, reaching **0.1–1 ppb**
clock agreement, then anchors the carrier phase per external frame with a
**reference frame** sent by the master 1 ms later; it demonstrates phase-coherent
multi-node CSI, stable flat phase differences, and functional AoA
[ESP-PPB]. Its hard-won lesson is directly load-bearing here: *the ESP32
internally estimates and compensates CFO per receiver; if two nodes' CFO
corrections differ, the CSI phase is silently corrupted; when clocks agree to
~1 ppb the corrections match and the CSI is clean* [ESP-PPB story]
[ESTABLISHED for C3; HYPOTHESIS for S3, which shares FTM + ns timestamps but
differs in PHY details].

This is a research note; it does not overrule any ADR. But notes 10/11/20
should be updated: the *wireless* escape is proven on the ESP32 family, by a
combination of (i) **frequency discipline** (FTM + VCTCXO), (ii) a **common
transmitter** reference frame, and (iii) **ns timestamps**. What remains open
is everything *above* the primitive: 3D apertures, transmit coherence,
bandwidth, scale, and self-calibration.

---

## 2. The invention

**Name.** The **Coherent Polyhedral Mesh (CPM)**: a two-tier architecture in
which each polyhedral puck is a self-calibrating coherent 3D *transceiver*, and
pucks are locked into a single carrier-phase frame wirelessly, so that a home
becomes one sharp, polarimetric, wideband, self-calibrating multistatic
aperture whose illumination can be steered.

Seven primitives, grouped by tier. For each: novelty, mechanism, what must be
true, falsifier, S3 feasibility, cost, transmit needed.

### Tier 1 — inside the puck

#### P1. Reverse Calibration Bus (RCB)

*What is genuinely new.* ESPARGOS and every ESP32 coherent-array project use
the conducted network **receive-only**: faces receive a reference *frame* to
calibrate `r_i`. Nobody runs it backwards. In the RCB, during a calibration
slot every face switches its antenna port to the bus; **one face transmits into
the bus and all others receive through it**. The bus (a passive, reciprocal,
layout-fixed network) then yields measurements

`M_ij = t_i − r_j + φ_bus,ij  (mod 2π)`,

with `t_i` the face's transmit-chain phase, `r_j` the receive-chain phase, and
`φ_bus,ij` known from design/manufacture. With N faces this is an N² equation
system in 2N unknowns (one global gauge) — over-determined and solvable in
closed form by least squares. During the slot **no antennas are connected**
(all switches on the bus), so there is no over-the-air leakage and the
calibration is purely conducted and exact [HYPOTHESIS — trivially testable].

*Mechanism.* Passive reciprocal network + SPDT switches (already needed for the
reference injection) + one calibration slot per dwell. Solves *both* chain
phases; ESPARGOS solves only `r` [ESPARGOS], and states that phase-coherent
*transmission* is possible but provides no transmitter [ESP-SDR]. The RCB is
the missing TX-side calibration, and it is what P6 and P5 stand on.

*Must be true.* (i) The bus path phases are stable (they are PCB traces — yes,
with temperature coefficients of a few hundred ppm/°C; at 10 GHz-scale delays of
a few cm, a 10 °C swing moves phase by a small fraction of a radian —
recalibration per dwell makes even that moot). (ii) Switch isolation ≥ ~25 dB
(unremarkable at 2.4 GHz). (iii) The TX power into a 20-way resistive divider
does not disturb the chip (10–18 dBm into a 50 Ω passive network; ESPARGOS
drives the same network in the other direction) [ESPARGOS].

*Falsifier.* Measured M_ij inconsistent with a reciprocal bus model (i.e.,
residual phase scatter after LS fit ≫ 0.1 rad after averaging).

*S3 feasibility.* High: per-face GPIO drives one SPDT; no new silicon. Divider
loss ~13 dB each way (20 ports) still leaves ≥ –30 dBm at the receiver with a
0 dBm chip — far above the ~ –90 dBm sensitivity.

*Cost.* ~$0.15–0.3 per face for the switch; divider is cents; PCB area.

*TX needed?* Yes — this primitive *is* a transmit calibration.

#### P2. Band Stitching under a re-calibrated PLL (Stitch)

*What is genuinely new.* Chronos/π-Splicer/SWAN stitch channels on Intel NICs
by bidirectional exchange ([Chronos], [π-Splicer], [SWAN]). On ESP32, each
channel change re-locks the PLL with a **new random phase**
[ESPARGOS §II-A], which is exactly what defeats naive stitching. The RCB
defeats it *inside a puck*: after each hop, the bus recalibrates every face,
and the conducted reference removes the PLL jump entirely. A mesh that hops
together (one schedule, one reference frame per hop) can therefore synthesise
**13 × 20 MHz = 260 MHz of coherent aperture** (or an overlapping HT40 set)
across channels 1–13. The result is ESP32-class hardware behaving like a
low-end wideband channel sounder [HYPOTHESIS; the components are each
established].

*Mechanism.* Scheduled whole-mesh hop; RCB slot; reference frame(s); capture;
data-plane stitching with per-hop phase bookkeeping. TCXO accuracy makes the
subcarrier *grid* error negligible: 20 ppm over the 52-subcarrier index span is
~10⁻² rad at the band edge — irrelevant; only the per-hop *offset* matters, and
that is what calibration removes.

*Must be true.* All nodes hop together and the reference is available on every
hop channel (the master can transmit a reference on each hop).

*Falsifier.* Stitched delay spectra show discontinuity artefacts larger than
the per-hop calibration residual under static-channel conditions.

*S3 feasibility.* High mechanically (channel switching is a standard API);
medium operationally (timing: a full 13-hop cycle at ~10 ms/hop is 130 ms —
fine for static scenes, marginal for fast motion).

*Cost.* Zero extra hardware.

*TX needed?* Yes (per-hop reference and two-way exchanges; a passive-only
variant is possible only with a hopping common transmitter, which the home's
APs do not provide).

#### P3. Coherent Illumination Steering (CIS)

*What is genuinely new.* **Phase-coherent transmit from commodity ESP32 face
arrays — and across pucks.** ESPARGOS declares it possible and does not ship it
[ESP-SDR]; no published ESP32 work transmits coherently from more than one
chip with controlled relative phase. With `t_i` known from P1 and the mesh's
common phase frame from Tier 2, each face transmits the same OFDM waveform
with a commanded phase offset. Because the host's raw TX path
(`esp_wifi_80211_tx`) emits standard frames and the faces share a clock, the
composite field at a remote receiver is the array factor: the receiver's CSI
amplitude must trace `|Σ a_i e^{j(φ_i^cmd + φ_i^chan)}|²` as the commanded
phases sweep. Within a puck this works with the 40 MHz clock alone (all faces
frequency-locked); across pucks it needs the Tier-2 frame [HYPOTHESIS].

*Mechanism.* Simultaneous transmission from multiple faces/chips with
calibrated phase. The combining requirement is timing within the OFDM cyclic
prefix (0.8 µs at 20 MHz); ordinary driver-queued raw TX has ~150 µs latency
with ±25 µs jitter [community source: esp32.com forum thread 3335, weak], which
is *outside* that budget, so the hub must trigger transmissions from the shared
40 MHz clock (ns-class) — either an FPGA trigger line or a PPS-style gating
signal. Given that trigger, the composite is phase-coherent. This timing
question and the per-packet TX-phase question are the two largest engineering
unknowns in the proposal; the decisive experiment below isolates both.

*Must be true.* The S3's per-packet TX PLL/resampler phase is deterministic or
calibratable; cross-face timing is FPGA-triggered; coexistence does not gate.

*Falsifier.* Receiver power shows no dependence on commanded relative phase —
i.e., the composite is additive in power, not field.

*S3 feasibility.* Medium: needs the hub to trigger TX (a 1-bit "TX now" from
the FPGA, or PPS-style gating), and RCB calibration. The RF and PHY do not
otherwise prevent it.

*Cost.* Hub trigger line (~$0); FPGA already in the premise.

*TX needed?* Yes — this is the transmit capability itself.

### Tier 2 — across pucks

#### P4. Tone-Lock (frequency discipline)

*What is genuinely new (in this context).* Wireless clock distribution to
array elements is a known research area: AirShare transmits two tones separated
by the reference frequency and feeds the recovered clock into each node
[AirShare]; RFClock adds timing/phase synchronisation via bidirectional
waveform exchange [RFClock]; a 2025 GNU Radio demonstration achieves ~1 Hz
pairwise differences at 10 MHz with frequency hopping [TwoToneDemo]; a
two-way time-transfer (TWTT) variant with COTS SDRs reaches ~60–70 ps time/phase
precision and 3.73 ppb frequency RMSE in a dynamic scenario [TWTT]
[ESTABLISHED on SDRs]. What is new is doing it **on ESP32-S3 pucks**, where the
proven route is *not* two-tone injection but **FTM + VCTCXO discipline**: each
puck (or each face group) disciplines its own VCTCXO against the master's FTM
exchanges using the chip's ns RX timestamps, exactly as ESP-PPB does on C3
[ESP-PPB] [ESTABLISHED on C3].

*Mechanism.* FTM exchanges (every 100–1000 ms) estimate fractional drift;
dual-DAC steering of a VCTCXO. Within a puck the 40 MHz reference is shared by
construction, so Tone-Lock's job is *puck-to-puck* frequency agreement — it
brings the mesh's nodes to ppb-class relative frequency. Consequence (B3): the
two-way exchange budget goes from ~10 µs at raw TCXO drift (practically
impossible on this MAC) to ~ms (comfortable) [ESP-PPB].

*Must be true.* The S3 has FTM and ns RX timestamps equivalent to the C3's
(synthesis note 17 says yes for the timestamp; note 15 says FTM is 2.4 GHz HT20
and coarse for ranging — but Tone-Lock needs only *drift*, where averaging
beats coarse per-measurement accuracy).

*Falsifier.* Measured S3 clock agreement cannot be held below ~50 ppb over
minutes with a cheap VCTCXO (then the two-way protocol must fall back to SIFS-timed
exchanges, see P5).

*S3 feasibility.* High; precedent on C3 [ESP-PPB].

*Cost.* VCTCXO ~$0.5–2, dual DAC ~$1, control loop firmware. ESP-PPB node cost
$50–80 total.

*TX needed?* The master transmits (FTM/reference frames). Slaves
receive/initiate FTM.

#### P5. Reference Frame + Two-Way Phase Ledger (phase coherence)

*What is genuinely new.* ESP-PPB anchors phase per external frame with a
reference frame sent 1 ms later — the "common transmitter" escape, proven on
ESP32-C3 [ESP-PPB]. What is added here is a **ledger**: a scheduled all-to-all
round (ping, then unique replies) whose *two-way products* are logged together
with ns receive timestamps, so that the network maintains a running, per-pair,
per-dwell estimate of (i) the reciprocal channel phase, (ii) each node's
chain-asymmetry sum `t_i−r_i` (the "hardware asymmetry signature"), and (iii)
the residual relative phase after removing the modelled drift. Because
exchange intervals `Δ` are known to ns precision, the phase equation of each
exchange — `Σφ = 2π[Δf_iΔ_1 + Δf_jΔ_2] + (t_i−r_i)+(t_j−r_j) − 2T_ij` — can be
solved across *intervals of different length*, separating the channel delay
`T_ij` from the chain sums and the common frequency offset. The theory of the
underlying calibration is established for distributed MIMO ([CalTheory],
[BeamSync], [OtaTDD]); the *interval-diversity ledger* on ESP32 is new
[HYPOTHESIS].

*Mechanism.* Scheduled pings with unique replies (identify responders), two-way
products, ns timestamps, Kalman/least-squares tracking of the remaining phase
drift; a master reference frame per external measurement for per-frame anchor.
With P4's ~1 ppb the ledger is *slow*: a full N-node round at 1–2 ms per slot
is 20–40 ms for 20 nodes — 0.015 rad of unmodelled drift per round at 1 ppb
(B3) [ESP-PPB].

*Must be true.* (i) The two-way exchange is reciprocal over its duration
(static scene; the *network's own* links, not the target's); (ii) ESP32's
coarse int8 internal CFO estimate [ESP-PPB] does not corrupt the fine estimate
we take from the subcarrier phase slope; (iii) per-packet phase noise ≤ ~10°
after discipline — measured on C3, plausible on S3 [ESP-PPB];

*Falsifier.* Residual relative phase after the ledger grows faster than ~0.1
rad per full mesh round. This is the M2 measurement the synthesis demanded; on
C3 the ESP-PPB evidence says the answer is favourable [ESP-PPB]
[ESTABLISHED on C3 for single-antenna nodes].

*S3 feasibility.* High (all pieces exist in ESP-IDF/community firmware).

*Cost.* Firmware + airtime; hardening against the int8-CFO quirk: drive the
relative CFO to zero by P4 so the chip's coarse correction is a no-op
[ESP-PPB] [HYPOTHESIS].

*TX needed?* Yes (two-way exchanges and reference frames).

#### P6. Static-Field Anchoring (SFA)

*What is genuinely new.* Treating the **static component of the cross-node
channels** as the network's persistent phase and geometry reference, with
re-anchoring on change. Multipath-as-anchor is an old idea (Channel-SLAM
treats reflections as "virtual transmitters" time-synchronised to the physical
transmitter [ChannelSLAM]; passive channel charting localises targets from CSI
with static TX/RX [PassiveCC]) — but neither uses the *static field* as the
carrier-phase reference for a coherent network, and the synthesis's note 13
concluded static multipath gives only per-link gauge. The correct statement,
derived here, is sharper: **a static scatterer does not fix the inter-node
phase gauge by itself** (the receiver LO difference survives), but combined
with **one common transmitter per cycle** it supplies the *calibration target*,
and with **coherence** it supplies the *sharpness*: the static field's path
phases are the common object all pucks measure, so they fix per-puck orientation
(rotation averaging) and give re-anchoring when the ledger drifts. The frame
is then: common reference fixes the gauge; static field ties geometry [derived
here; consistent with [CalTheory]'s "F-calibration of the joint array"].

*Mechanism.* Per link, decompose CSI into static + dynamic subspaces (long-term
mean/low-rank background subtraction [note 13]); the static part is retained as
a phase/geometry reference; the dynamic part is the sensing signal; when the
static reference's own phase drifts beyond a threshold (someone opened a door),
re-anchor (an RCB slot plus a ledger round is cheap).

*Must be true.* The static field is stable to ≪λ/20 ≈ 6 mm path change on the
re-anchoring interval in an occupied home — i.e., the reference must not be
consumed by the signal (B1). This is *the* environmental unknown and is
measurable in one afternoon in a lived-in room.

*Falsifier.* Static-component phase drifts >0.3 rad on the timescales between
re-anchors even with furniture static.

*S3 feasibility.* High (it is processing).

*Cost.* Zero hardware.

*TX needed?* No for the processing; yes for re-anchoring.

### Tier 3 — algorithms

#### P7. Wireless Bundle Adjustment (WBA): self-calibrating geometry, targets, and transfer

*What is genuinely new.* A joint estimator over the network's physical
parameters — per-puck position and orientation, per-face/per-pair chain phases,
the static landmark map, and moving targets — constrained by *gauge-invariant*
measurements only (amplitudes, per-link phase slopes, per-puck relative AoA,
two-way sums, Dopplers), with the ledger supplying the gauge where coherence is
available. This is "bundle adjustment" (structure-from-motion) lifted to RF:
each puck is a camera with a known intrinsic calibration (Tier 1) and an
unknown pose; multipaths are landmarks; FTM gives coarse baselines. Existing
work covers pieces (channel charting gives label-free relative geometry
[ChannelCharting]; Anchor-free self-survey; passive CC triangulates AoA across
phase-coherent arrays [PassiveCC]) but not the *coherent mesh* formulation, and
not the transfer story. The learned part is confined to *residuals* (per-unit
hardware quirks, coupling corrections, clutter models) using synthetic
pretraining plus few-shot labels — the strongest known generalisation lever
(Sim2Real reached 82.1 % F1 with 20 % real labels ≈ 98.6 % of full supervision
[synthesis note 19]) — while the bulk of the model is physical, which is *why*
it transfers across homes [HYPOTHESIS].

*Must be true.* Enough landmark observations per puck pair for pose
identifiability (a home qualifies); a small set of drive-by labels (FTM ranges,
one known wall) to fix scale/pose.

*Falsifier.* WBA on a held-out room fails to recover puck poses to better than
the FTM prior, or cross-room transfer collapses relative to in-room training.

*S3 feasibility.* Host-side compute; the pucks stream CSI. ESP-DL/ESP-NN exists
but this is not an edge-inference problem [synthesis].

*Cost.* Software.

*TX needed?* Only for the measurements it consumes.

### Summary table

| Primitive | New? | Gate quantity | Falsifier | S3 | Cost | TX? |
| --- | --- | --- | --- | --- | --- | --- |
| P1 RCB | Yes (TX-side conducted calibration) | bus model residual | LS residual ≫0.1 rad | high | ~$5/puck | yes |
| P2 Stitch | Yes on ESP32 | per-hop recalibration residual | stitch artefacts | high | $0 | yes |
| P3 CIS | Yes (coherent TX on commodity) | TX phase determinism, <CP timing | no field-level interference | medium | ~$0 | yes |
| P4 Tone-Lock | Yes on S3 (proven C3) | Δf_rel ≤ ~1 ppb | cannot hold ≤50 ppb | high | ~$2/puck | master TX |
| P5 Ledger | Yes (interval diversity) | σ_ψ ≤ 0.1–0.6 rad/round | residual grows unbounded | high | firmware | yes |
| P6 SFA | Yes (as reference strategy) | static-field stability ≤λ/20 | static phase drifts >0.3 rad | high | $0 | no |
| P7 WBA | Yes (formulation + transfer) | pose identifiability, transfer | no better than FTM | host | software | no |

---

## 3. The sharpness budget (honest numbers)

All at 2.45 GHz, λ ≈ 12.2 cm. "Precision" = estimator variance; "resolution" =
ability to separate two nearby things.

| Quantity | Value | Basis |
| --- | --- | --- |
| Puck aperture | 0.30 m → D/λ ≈ 2.5 | premise |
| Puck beamwidth (conventional) | ~20–25° | 0.89 λ/D |
| Puck angular precision (single dominant path, 25 dB SNR, calibrated) | ~0.5–3° | CRB vs multipath reality [HYPOTHESIS] |
| Virtual aperture (face TX × face RX, far field, element at r_i+r_j) | D_v ≤ 0.6 m → D/λ ≈ 5 → ~11° | MIMO-radar virtual array [ESTABLISHED technique] |
| Virtual elements | 20×19 = 380 (×2 polarimetric) | combinatorial |
| Two-puck baseline 3 m / 6 m | 2.4° / 1.2° beamwidth | D/λ = 24 / 48 |
| Cross-range precision at 5 m, 6 m baseline, σ_θ = 1° | ~9 cm | geometry |
| One-way range **precision** (52 subcarriers, 16.25 MHz span, σ_φ 0.1 rad) | ~3–4 cm | phase-slope CRB |
| One-way range **precision** (ESP32 practical, ~1 ns delay error) | ~30 cm per measurement, improves with averaging | [SameSignalDifferentStory] |
| Delay **resolution**, 20 MHz / HT40 / stitched 260 MHz | 15 m / 7.5 m / ~0.6–1.2 m | B2 |
| Doppler range at 100 Hz / 1 kHz frame rate | ±1.2 m/s / ±12 m/s | λ/2T |
| Mesh snapshot rate (20 nodes, 1–2 ms slot) | 25–50 Hz | ESP-NOW ~100 µs bursts [ESP-PPB story] |
| Coherence requirement for ≤1 dB focusing loss | σ_ψ ≤ λ/20 ≈ 0.3 rad | array theory |
| Demonstrated cross-node phase on ESP32 (C3) | <1° best / <10° typical | [ESP-PPB] |

Reading of the table: the invention yields **~10–30 cm cross-range at
5–10 m, ~dm depth resolution after stitching, ~cm-dm precision for dominant
paths, full-sphere coverage, 3D polarisation** — i.e. "sharp" relative to any
commodity Wi-Fi sensing today, and *structural*: coordinates and paths, not
scores. It does not compete with mmWave/UWB imaging for raw resolution, and the
home's multipath remains the dominant error source, which is why path-resolved
and geometry-based processing (P6, P7) is where the marginal work is.

---

## 4. The six mission directions — verdicts

1. **Wireless coherence on commodity silicon.** *Solved in principle and in
   prototype* (ESP-PPB on C3: FTM+VCTCXO, 0.1–1 ppb, per-frame reference,
   working AoA [ESP-PPB]); *proven on SDRs* (AirShare/RFClock/two-tone/TWTT
   [AirShare],[RFClock],[TwoToneDemo],[TWTT]); *not yet on S3, not at home
   scale, not with 3D pucks.* The missing quantity is per-packet phase residual
   in multipath; the mechanism that makes it tractable is discipline (B3).
2. **Self-calibrating learned array.** The physical half of self-calibration is
   the *real* contribution (Tier-1 known geometry + RCB; P7 WBA); the learned
   half should be residuals only [HYPOTHESIS]. Joint discovery of geometry and
   targets from coherent multipath is formulated (Channel-SLAM [ChannelSLAM],
   passive CC [PassiveCC]) but not in the coherent-mesh setting; transfer
   across homes is the untested claim (physics-parameterised models are the
   reason to expect it).
3. **Environment as shared reference.** The environment supplies *calibration
   targets* (static landmarks fix per-puck orientation and re-anchor the
   ledger) but **not** the phase gauge (a static scatterer leaves the
   inter-node LO difference untouched) [derived; consistent with notes 13/20].
   Reference-and-signal degeneracy (B1) is the boundary. The composite trick —
   common frame for the gauge, static field for the geometry — is the
   invention's answer here.
4. **Sharp 3D from many coarse apertures.** Four levers, ranked by
   tractability: (i) coherent inter-puck baselines (metres → ~1–2° — the big
   one, needs P4/P5); (ii) per-puck virtual aperture (2× diameter, free with
   P1/P3); (iii) 3D polarimetric diversity (free with the polyhedron; new);
   (iv) round-trip two-way products, which are LO-free and give 2× delay and 2×
   Doppler plus the chain-asymmetry signature used for calibration and node
   identity [Chronos; ledger here]. Multi-band is not available on S3 (2.4 GHz
   only); 5 GHz on C5 is the obvious future doubling [SPECULATION].
5. **The ping + unique replies + two-way protocol.** Recovers: per-pair
   reciprocal channel (delay/amplitude/multipath) with the LO cancelled up to
   modelled drift; relative CFO and its stability (the ledger); chain-asymmetry
   fingerprints (t_i−r_i sums — calibration and physical identity); fine delay
   (phase slope) plus coarse range (FTM) → node geometry by MDS; per-link
   Doppler; amplitude-based obstruction maps. It breaks when (a) the exchange
   exceeds the drift budget (fixed by P4 discipline; else use SIFS-timed
   hardware ACKs if CSI-on-OFDM-ACK proves available — open question),
   (b) multipath biases the delay (fixed by FTM coarse + slope fine + robust
   fitting), (c) reciprocity fails because the *scene* changed mid-exchange (a
   moving person near a node — mitigated by scheduling and by treating the
   dynamic part as signal, not reference), (d) the chip's int8 CFO estimate is
   the only frequency observable (avoided by reading the fine estimate from the
   subcarrier slope, and by driving CFO → 0 with P4) [ESP-PPB story].
6. **Different excitation/composition.** One radio per face gives simultaneous
   coherent *receive* (no face TDM). Transmit is scheduled TDM across faces
   unless CIS is used. Passive illumination is established (ESPARGOS datasets
   use four non-cooperative ceiling transmitters [ESPARGOS datasets];
   [PassiveCC]). The genuinely new excitation is CIS (coherent distributed
   transmit), which also enables retrodirective-style focusing by scattered-power
   feedback [SPECULATION] and wideband stitching [HYPOTHESIS].

---

## 5. The single decisive experiment: the coherent wave-step

**Question it settles:** can ESP32-S3 radios be made to transmit *phase-coherently*
enough that a remote receiver sees field-level (not power-level) combining —
first with a shared clock (isolating TX physics), then with the wireless
reference chain (P4+P5) substituting for the wire?

**Setup (two stages, one rig).**

- **Rig:** one "two-face puck" (2 × ESP32-S3, SPDT each, one resistive divider
  = RCB), one probe receiver ESP32-S3 with CSI, one SDR or third S3 as the
  independent reference, all on a bench at 3–5 m, plus a coax "truth" path
  (attenuated splitter) for ground-truth phase.
- **Stage A (wired):** both faces share a 40 MHz clock. Run an RCB slot; then
  both transmit the same ESP-NOW/OFDM frame simultaneously (hub-triggered),
  with the commanded relative phase stepped 0…2π in 16 steps over many
  repetitions. The probe's CSI amplitude must trace the predicted sinusoid
  (field addition). Also run a coax truth path to verify commanded phase.
- **Stage B (wireless):** break the shared clock; give each board its own
  VCTCXO disciplined by FTM to a master, and run the P5 ledger (ping/unique
  replies/reference frames). Repeat the wave-step. The sinusoid must *persist*
  with an RMS phase error below 0.6 rad (and ideally be tracked below 0.3 rad).

**Prediction/falsification.** [HYPOTHESIS] the amplitude traces a clean
sinusoid in both stages. *Falsified* if Stage A is flat (TX phase not
controllable/timing not inside CP ⇒ CIS dies; receive-only CPM remains), or if
Stage B's sinusoid is destroyed (wireless phase transfer insufficient on S3 ⇒
mesh coherence dies for S3 and the architecture must fall back to Tier-1
clusters joined non-coherently, or to the wired reference). Either outcome is a
hard, publishable fact about commodity silicon.

**Why this one.** It is the cheapest experiment that simultaneously exercises
every gating primitive — RCB (P1), TX coherence (P3), wireless discipline
(P4), ledger (P5), and the phase residual that note 20 calls M2 — and it is
run with parts that fit on a desk (~$150–300, weeks). Everything else in the
proposal is processing, packaging, and scale.

Secondary, cheaper gates (run first, same rig): E0, the S3 CSI per-packet phase
noise and drift spectrum under (i) free-run TCXOs, (ii) disciplined VCTCXO,
(iii) shared clock, against the coax truth — the E0 numbers *are* M2; E1, CSI
availability and phase quality on OFDM control/ACK frames (would unlock
SIFS-timed two-way exchange).

---

## 6. If the space is closed: the closing quantities and the minimal references

Closure is quantitative, not qualitative. Three quantities decide it:

1. **σ_ψ,res — per-packet cross-node phase residual after discipline and
   ledger, for S3, in a real room.** Closes the space if ≳1 rad sustained
   (field-level combining dies; angles degenerate to power-only sensing).
   *Minimal reference that crosses it:* one common frame per measurement cycle
   (already demonstrated on C3 [ESP-PPB]) or a wire (ESPARGOS [ESPARGOS]).
2. **Δτ_path — static-landmark separability at available bandwidth (B2).**
   Closes the space *for geometry* if paths cannot be resolved enough to serve
   as landmarks (with 20 MHz, in dense multipath). *Minimal reference that
   crosses it:* bandwidth — stitched 260 MHz (P2), a 5 GHz C5 second band, or a
   cheap UWB auxiliary for landmarks [SPECULATION].
3. **T_ref — static-field coherence time in an occupied home (B1).** Closes the
   space *for the free reference* if the reference path moves faster than
   re-anchoring can track. *Minimal reference that crosses it:* one dedicated,
   monitored reference path (a master with line-of-sight to each puck, or a
   coax) so that reference corruption is *observable*.

The gauge itself is never "closed": one constraint per connected component
always fixes it, and the constraints available on commodity ESP32 are (in
increasing cost) a common transmitter's frames, one reciprocal exchange per
link, a two-tone over-the-air clock, or a coax. The physics that *is*
irreducible: absolute per-node carrier phase is unobservable (U(1) gauge);
every wireless reference is also a scene path (B1); and delay resolution is
bandwidth-bound (B2). No arrangement of ESP32-S3s escapes those three; the
invention is designed to live inside them.

---

## 7. Chaining, existing models/data, own data

**Chaining (A).** Tier 1 (RCB ⇒ calibrated TX+RX chains ⇒ coherent TX +
stitching) enables Tier 2 (ledger ⇒ mesh coherence) which enables Tier 3
(sharp 3D geometry ⇒ mapping, self-calibration, transfer). Highest-leverage
enabling capability: **the conducted calibration bus**, because it is the only
one that unlocks *transmit* coherence on commodity silicon and makes stitching
routine. Second: **clock discipline** (P4), because it converts an impossible
microsecond-scale protocol (B3; synthesis note 9's "TDM is mandatory, MAC
turnaround is milliseconds") into an ordinary millisecond one. Blocked without
them: CIS, stitching, cross-puck coherent AoA (sharp 3D), distributed
beamforming. Receive-only fallbacks survive either failure (passive sensing
with a master reference; Tier-1 clusters).

**Existing models/data (B).** Directly reusable: ESPARGOS hardware design and
the phase-coherent datasets (`espargos-0005`, `espargos-0007`, DOI
10.18419/DARUS-4973) [ESPARGOS datasets]; `pyespargos` processing (calibration,
MUSIC/root-MUSIC, beamspace, Jones calculus) [WiFiCamera]; `esp-sdr` for raw
I/Q (RX; S3 supported; phase-coherent across ESPARGOS antennas) [ESP-SDR];
`esp-csi` (Apache-2.0) for CSI plumbing; ESP-PPB (GPL-3.0) for the wireless
sync stack — the single most relevant prior art [ESP-PPB]; SenseFi/MM-Fi,
CSI-Bench, OpenCSI for pretraining (all non-ESP32, domain adaptation required
[synthesis §5]); Channel-SLAM and passive channel charting as algorithm
templates [ChannelSLAM],[PassiveCC]. Caveat: none of these models consumes
*coherent transmit* data because none has ever existed on commodity hardware.

**Own data / new technology (C).** The minimal dataset that would prove the
invention: three S3 nodes (two "faces" + master) logging, synchronised, (i)
free-run/ disciplined/ shared-clock phase against a coax truth, (ii) the
coherent wave-step in both stages, (iii) a 13-channel stitched sounding of a
test room, (iv) one 6-puck home pilot with UWB or motion-capture ground truth.
Falsified by any of the three closing quantities exceeding threshold under the
control conditions above. This dataset does not exist and cannot be bought; it
is the project's unique asset.

**Evidence strength: 6/10.** The mechanisms are each established somewhere
(ESPARGOS wired arrays; ESP-PPB wireless phase on C3; AirShare/RFClock/TWTT on
SDRs; Chronos/π-Splicer stitching on Intel; BeamSync/F-calibration theory) and
the two hardest gates (wireless phase coherence on ESP32; phase slope ranging)
have favourable primary evidence. Deductions to 6: no S3 measurements exist for
phases or TX coherence; ESP-PPB is a community prototype, not peer-reviewed;
the 3D/TX/stitching combination has never been built; the home-scale
environmental claims (SFA, transfer) are untested; and the transmit-timing and
OFDM-ACK questions remain open.

---

## 8. Residual unknowns

- S3-specific per-packet phase noise and drift spectrum (E0); S3 vs C3 PHY
  differences (the "first_word_invalid" quirk, 4-byte CSI invalidation on S3
  [ESP-IDF docs]).
- Whether `esp_wifi_80211_tx` frames can be hardware-triggered tightly enough
  for field-level combining (CIS), and whether the S3 TX chain's per-packet
  phase is deterministic.
- CSI availability on OFDM control/ACK frames (would enable SIFS-timed
  exchanges; the CSI engine emits CSI only for OFDM frames, and ACK rate is a
  configurable basic-rate choice [ESP-IDF vendor features, issue #954]).
- Mutual coupling magnitude on a soldered polyhedron and whether RCB/calibration
  inversion suffices (ESP-PPB's coupling experience is the warning [ESP-PPB
  story]).
- Static-field coherence time in an occupied home (SFA); reference-path
  corruption statistics (B1).
- Whether the int8 internal CFO correction can be disabled or made benign
  (would simplify P5) [ESP-PPB story].
- Realistic multi-puck cost at volume (BOM sketch: 20 × S3 ≈ $60–100, hub
  FPGA/Ethernet ≈ $15–40, switches/divider ≈ $5–10, VCTCXO ≈ $2, PCB/assembly
  ≈ $50–130 → $130–280 per puck; a CPM "starter" of ESP-PPB-style nodes is
  $50–80 each [ESP-PPB]).

---

## 9. Sources

Primaries and project docs (all accessed 2026-10):

1. ESPARGOS sounder — F. Euchner et al., "ESPARGOS: An Ultra Low-Cost,
   Realtime-Capable Multi-Antenna WiFi Channel Sounder", arXiv:2502.09405.
   https://arxiv.org/abs/2502.09405
2. ESPARGOS datasets — arXiv:2408.16377; dataset espargos-0007 DOI
   10.18419/DARUS-4973. https://espargos.net/research/
3. ESPARGOS multi-array coherence docs.
   https://espargos.net/docs/combined-arrays.html
4. ESP-SDR (raw I/Q on ESP32 family; S3; phase-coherent IQ; "phase-coherent
   transmission is possible" not provided) — https://espargos.net/espsdr ;
   https://github.com/ESPARGOS/esp-sdr (GPL-3.0).
5. AR "WiFi camera" — "Visualizing Wireless Propagation and Polarization in
   Augmented Reality with ESPARGOS", arXiv:2608.25996.
6. Passive channel charting — Euchner et al., arXiv:2504.09924 (SPAWC 2025);
   code github.com/Jeija/ESPARGOS-Passive-ChannelCharting.
7. Phase calibration of distributed arrays (F-/R-calibration; joint
   F-calibration of two arrays with a two-frequency method) —
   arXiv:2304.05144.
8. OTA phase calibration in the TDD flow — Ngo & Larsson, arXiv:2509.03722
   (GLOBECOM 2025 companion, "Breaking the TDD flow…", pp. 1645–1650).
9. BeamSync — Ganesan, Sarvendranath, Larsson, arXiv:2311.11070; IEEE TWC
   23(7):6824–6837, 2024; RadioWeaves version arXiv:2112.00592.
10. AirSync — Balan et al., arXiv:1205.6862; IEEE/ACM ToN 21(6), 2013, DOI
    10.1109/TNET.2012.2230449.
11. AirShare — Abari, Rahul, Katabi, Pant, INFOCOM 2015, IEEE Xplore doc
    7218555.
12. RFClock — Alemdar et al., MobiCom 2017.
13. Robust frequency-hopped two-tone OTA clock demo — arXiv:2509.25277.
14. Digital wireless TWTT for coherent distributed arrays (60–70 ps; 3.73 ppb
    dynamic) — OSTI 3020870.
15. Chronos — Vasisht, Kumar, Katabi, NSDI 2016; "Sub-Nanosecond Time of Flight
    on Commercial Wi-Fi Cards", arXiv:1505.03446.
16. π-Splicer — Zhu et al., IEEE TMC 17(9):2155–2165, 2018.
17. SWAN — Xie, Zhang, Li, MobiCom 2018.
18. Receiver effects across chipsets (ESP32 delay error >1 ns; x310 0.12 ns) —
    "Same Signal, Different Story", arXiv:2605.26836.
19. Channel-SLAM — Gentner, dissertation DOI 10.18725/OPARU-10191; ION GNSS+
    2015 paper.
20. ESP-PPB — github.com/jonathanmuller/esp-ppb (GPL-3.0), incl. story.md
    (community, non-peer-reviewed; the wireless ESP32 phase-coherence
    demonstration).
21. ESP-IDF Wi-Fi vendor features (CSI on OFDM frames; CSI fields) —
    docs.espressif.com (v4.4/v6.1) and esp-csi issues #121 (phase offsets),
    #124 (sniffer CSI); RuView issue #954 (CSI only for OFDM frames).
22. Espressif Techpedia, ESP-CSI solution / esp-crab (C5 clock-buffer phase
    sync). https://docs.espressif.com/projects/esp-techpedia/…
23. "Less is More: The Dilution Effect in Multi-Link Wireless Sensing",
    arXiv:2602.10823 (denser meshes can degrade sensing — deployment caution).
24. Spherical antenna arrays survey, arXiv:2604.02701 (full-sphere properties).
25. 802.11bf overview (multistatic sensing; sync fields are time, not carrier
    phase) — arXiv:2310.17661; NIST 802.11bf procedure paper.
26. FTM on ESP32 — arXiv:2401.16517.

## 10. One-paragraph answer

A home full of ESP32-S3 pucks can be made into one sharp, self-calibrating 3D
sensor, but not by any single trick: it takes a **two-tier coherent mesh**.
Inside each puck, run the hub's conducted reference network **backwards** as a
calibration bus to solve every face's transmit *and* receive chain phase per
dwell — this is the enabling novelty, because it gives commodity silicon a
capability it has never had: **controlled, coherent transmit**. Across pucks,
discipline each puck's clock with FTM + VCTCXO to parts-per-billion and anchor
the carrier phase with scheduled ping/unique-reply two-way exchanges and
reference frames — a stack already demonstrated on ESP32-C3 — while the home's
**static multipath serves as the calibration target and re-anchor**, never as
the gauge. Stitched across thirteen channels, the mesh then behaves as a
sub-metre-resolution, centimetre-precision, full-sphere, polarimetric,
multistatic aperture whose illumination can be steered, and whose geometry and
targets are jointly self-calibrated by physics-parameterised bundle adjustment
so the model transfers between homes. The single decisive experiment — the
**coherent wave-step** — settles whether ESP32-S3 can transmit coherently,
first with a shared clock, then with the wireless reference chain; if it
cannot, the fallback is a non-coherent network of coherent pucks, and the
quantity that closed the space is the per-packet cross-node phase residual in
multipath.
