# 22 — The Clutter Clock: a home of cheap nodes as one self-calibrating coherent 3D aperture

Status: invention proposal (agent `r`). Australian English. Research note — advisory,
out of band; it never defines v1 scope (`AGENTS.md`). Evidence grades are
**ESTABLISHED / HYPOTHESIS / SPECULATION** and are stated per claim. This note is
written against the synthesis [`00-synthesis.md`](00-synthesis.md) and notes
`08`, `09`, `10`, `11`, `12`, `13`, `17`, `20`; it does not upgrade their claims.

## 0. The answer in one page

**The invention.** *Aperture-referenced phase transfer* — a volumetric coherent
sub-array (the polyhedron puck) that uses the spatial structure of **any** audible
transmission to separate the local oscillator phase from the channel and thereby
**transfer phase from that transmission to its own node**. Because every sensing
transmission is also a common phase reference, and because the room's static field
is stationary for seconds to hours, a home of pucks **self-calibrates and stays
phase-locked with no cable, no dedicated synchronisation signal, and no two-way
exchange** — using the room's ordinary radio traffic (including its own
illumination) as the clock. The residual after the static field is subtracted is
the sensing signal. The room is the clock; the world is what is left.

**The mechanism.** Each puck manufactures *exact* intra-puck phase relations (shared
40 MHz clock + conducted 2.4 GHz reference — the hardware premise). It then behaves
as a *phase meter*: for one received frame, the 20 coherent faces share one LO phase
(a common scalar) and see one wavefront (a structured vector); fitting the calibrated
array manifold to the known direction isolates the scalar, with array gain and with
static multipath absorbed into a fixed calibration constant. Two pucks hearing the
same frame obtain their **relative** LO phase per frame. A Kalman state estimator
over the whole mesh tracks per-node LO phases (fast, Wiener phase noise), static
channel phases (slow), and targets (the residual). Geometry is bootstrapped to
λ/2 by FTM + per-puck 3D bearings + carrier-phase ambiguity resolution, after which
the carrier phase fixes it to mm — all without an external survey.

**The decisive experiment.** **LOCK-1:** two pucks 3–8 m apart in a furnished room,
with a *conducted reference used only as ground truth*; measure the clutter-clock
residual σ_φ over 30 minutes, with and without a person moving, versus frame rate;
accept if σ_φ ≲ 0.4 rad (λ/16), reject if ≳ 1 rad.

**Feasibility and cost.** Every component is either demonstrated on the ESP32 family
(shared clock, conducted reference, phase-stable CSI, ns RX timestamps) or is a
protocol/estimator; the S3-specific gaps are measurable on a bench. Prototype puck
≈ US$120–250, volume ≈ US$60–100; a six-puck home ≈ US$700–1500 — a home-scale
radio interferometer for the price of a television.

**TX needed?** For the lock: no (listening suffices). For illumination and FTM:
yes (one puck at a time, ordinary ESP-NOW/`esp_wifi_80211_tx` frames).

**If it fails.** The closing quantity is the phase-coherence budget
`B = Δν_res·T_obs` of the relative LO (residual linewidth × observation interval).
The minimal reference that crosses it is one shared clock/reference per connected
component — in practice a single coax *coherence splice* between adjacent pucks
or a TCXO upgrade; a second, speculative escape is a tone carried on the home's
existing mains wiring.

---

## 1. What the sensor must beat

The synthesis (`20`) fixes the information-theoretic floor: with independent LOs,
CSI has the form `ĥ_ij = g_ij·a_i·b_j·e^{j(θ_i−ψ_j)}·e^{−j2πf_kτ}` and only
`θ_i−ψ_j` (plus the channel) is observable; on a connected measurement graph the
node phases are fixed only up to one global `U(1)` gauge. The three published
escapes are a common-transmitter reference, over-the-air reciprocity, and a
shared/conducted reference (`00` §1). "Sharp, self-calibrating, generalising"
therefore has a quantitative meaning:

* **Sharp.** A distributed aperture of `M` coherent faces spread over the home has
  angular resolution `≈ λ/D`: with `D ≈ 6–10 m` at 2.4 GHz, **1–2°**; and, once
  coherent, *carrier-phase differential* measurements track path-length *changes*
  to **cm** (one-time 6 cm ambiguity). Range *per epoch* stays coarse (`c/2B ≈ 7.5 m`
  at 20 MHz, ≈ 2 m if 13 channels are stitched coherently across a puck).
* **Self-calibrating.** The array measures its own geometry and its own phase state
  from the radio field; no survey, no manual calibration, no per-home learned model.
* **Generalising.** The core estimator is Maxwell + geometry. What changes across
  homes is *calibrated automatically*, not *learned*; ML stays where note `19`
  placed it (targets, classification), with synthetic/weak supervision.

The polyhedron premise (20 triangular faces, shared clock fan-out, conducted 2.4 GHz
reference, hub aggregation) is the enabling artifact: it is the smallest unit that
has *exact* phase and *exact* geometry, i.e. the **rigid atomic aperture**. This note
answers the question "how do atoms bind?" — and what happens if they cannot.

## 2. The invention: the Clutter Clock

### 2.1 The primitive — aperture-referenced phase transfer (ARPT)

For puck `P` (faces `i ∈ 1..N_f`), one received frame from transmitter `s`, one
subcarrier `n`:

```
y_i(n,t) = A_i(n) · e^{jθ_P(t)} · e^{−jψ_s(t_s)} · b_s · h_is(n, û_s; multipath) + w
```

where `θ_P` is the puck LO phase (common to all faces by construction), `A_i` the
face chain response — **known and held fixed against the conducted reference**
(re-measured per channel change, per power-up, and on a thermal schedule), and
`h_is` the channel. In vector form over the faces:

```
y = (A ⊙ e(û_s)) · h_LoS · e^{j(θ_P − ψ_s)} + (multipath + noise)
```

The estimator for the *phase scalar* is the normalised matched filter against the
*calibrated* LoS array response:

```
γ̂_{P,s} = arg Σ_i (A_i(n)·e_i(û_s))^* · y_i(n)
```

Three properties matter.

1. **Array gain on the reference.** The scalar's phase noise falls as `1/√N_f`
   versus a single-antenna phase measurement; the face count *is* the phase-meter
   SNR. This is why the polyhedron, not a whip antenna, is the right atom.
2. **Static multipath is calibration, not noise.** Multipath that does not move
   contributes a *fixed* bias, absorbed into the estimator's calibration constants
   (exactly the DICHASUS argument, generalised from LoS-only to *stationary* fields).
   Only *changes* of the field corrupt the lock — and changes are the sensing signal.
3. **A single frame, a single common transmitter, gives pairwise inter-puck phase.**
   If pucks `P` and `Q` both hear `s`, then `γ̂_{P,s} − γ̂_{Q,s} + 2πf(d_Qs − d_Ps)/c = θ_P − θ_Q (mod 2π)`.
   No two-way exchange, no dedicated sync waveform, no SIFS timing: **the phase
   transfer costs one frame.**

Two corollaries make the scheme cheap:

* **The illuminator is the clock.** When a puck transmits the sensing waveform, the
  same frames that sound the room transfer its phase to every listener. Illumination
  and synchronisation are one act (`s` is the sensing transmitter).
* **Ambient traffic is free synchronisation.** Any AP, phone or laptop frame heard by
  two pucks is a phase-reference opportunity. The network prefers self-beacons at a
  controlled rate, but ambient frames are free top-up.

**Full-sphere matters.** A planar array can only phase-meter sources in front of it;
a face pointing away contributes noise, and a planar puck loses its reference when
the only audible source is behind it. The polyhedron's 3D geometry gives every
direction at least a partial visible sub-array, so a puck can *never* be blind to all
simultaneous references. This is the specific reason the invention wants the
polyhedron rather than a board.

### 2.2 The estimator — a phase-field state space

The network state is small and slow compared to the raw data:

* **Fast states (per puck, per channel):** LO phase relative to a reference puck,
  modelled as a Wiener process (free-running LO) — the same model used in the
  distributed-MIMO literature (`arXiv:2509.03722`, `arXiv:2201.08931`).
* **Slow states (per link, per subcarrier):** static channel phase/amplitude — the
  room's field, re-estimated over minutes.
* **Residual:** everything the model cannot explain — moving scatterers,
  environmental change.

Observations arrive per frame: every face of every puck that heard the frame
contributes. Three robustification devices are needed and each is testable:

* **Common mode.** The LO phase is common to all links terminating on a puck at a
  given instant; channel changes are not. A joint fit (robust median / RANSAC /
  innovation gating) separates them; the *room* supplies redundancy (a person
  occludes a minority of inter-puck links at any instant).
* **Within-puck cancellation.** Faces of one puck share an LO, so cross-face ratios
  or conjugate products (CACC/CASR, established on single NICs — `PMC9721356`,
  `arXiv:2508.12614`) cancel *all* clock impairments per node. The puck's own AoA
  and differential measurements are therefore clean *before any coherence exists*:
  the fallback sensor is already there.
* **Time-scale separation.** LO drift is fast (ms–s), the room's field is slow
  (minutes–hours), a walking person is in between. A Kalman filter with two process
  noises is the natural formulation; its failure mode (target mistaken for drift) is
  exactly what LOCK-1 measures.

### 2.3 The coherence ladder — acquisition, then maintenance

| Tier | Relative phase obtained by | Supported by | Update policy |
| --- | --- | --- | --- |
| 0. Face | conducted reference + shared clock | premise; ESPARGOS (`arXiv:2502.09405`) | per channel/boot; periodic thermal re-verify |
| 1. Puck | exact, by construction (tier 0) | same | free |
| 2. Puck pair | ARPT on a common frame; *one-shot* absolute fix from geometry | §2.4 | per frame (tracking), not per exchange |
| 3. Home | graph of tier-2 pairwise phases; gauge fixed per connected component | `20` §1 | continuous estimation; graceful splits |

The key departure from the classical recipe: **acquisition and maintenance are
decoupled.** Absolute acquisition (fixing the 2π gauge per pair) is a *one-time*
geometric act (§2.4) because the pucks do not move. Maintenance is *tracking*, and
tracking never needs a two-way exchange — it needs *observations* at a rate faster
than the LO random walk. Where a link or puck pair fails (major furniture move,
channel change, reboot), that pair re-acquires from geometry; the rest keeps
tracking. The network is **partially coherent by design** (framework:
Ganesan et al., *IEEE OJCOMS* 2024, DOI 10.1109/OJCOMS.2024.3373170): coherent
where the data supports it, incoherent elsewhere, fused at the host.

### 2.4 The self-survey — bootstrapping geometry to λ/2

Coherence needs the *channel phase* seed per pair, i.e. geometry to λ/2 ≈ 6 cm.
This is a background process, not a calibration ritual:

1. **Coarse (minutes):** FTM ranges (1–2 m on S3; `15`) and per-puck 3D bearings
   from the volumetric array (few degrees, unambiguous because the 3D manifold is
   not a grating-lobe lattice) locate all pucks; multi-pair triangulation refines to
   O(10 cm). This is the DICHASUS idea (`arXiv:2206.05984`) with the roles reversed:
   the *arrays* are surveyed, not the UEs.
2. **Fine (hours, background):** carrier-phase differential measurements plus
   GNSS-style integer ambiguity resolution (wide-lane across the 13-channel 2.4 GHz
   span if cross-channel coherence inside a puck is enabled by a comb/broadband
   conducted reference, otherwise resolved by redundancy), refine to mm and lock the
   2π per pair. The mathematics is standard integer least-squares/bundle adjustment;
   the novelty is the indoor setting with ambient "satellites".
3. **Continuous re-verification:** the static field is the survey's own witness —
   a survey that has drifted shows up as unexplained common-mode error.

Marked **HYPOTHESIS** as an S3 demonstration; every ingredient has established
precedent (FTM; MUSIC; DICHASUS; GNSS carrier-phase; Espressif ESP-SDR for raw I/Q
if ever needed).

### 2.5 The sensor — coherent residual imaging

With the ladder locked, the home is one array of `M` coherent faces (dual-pol faces
double this to `2M`). A TDM round — one puck illuminates per slot, its faces
transmit sequentially, all other faces receive — yields a holographic sample of the
home's channel matrix, coherent across pucks. Products:

* **Angle–Doppler imagery.** Coherent backprojection of the residual `ΔH(t)`
  over `M²` bistatic paths: 1–2° main lobe, sidelobes ≈ `1/√N_baselines` for a
  random 3D layout (sparse-array theory; `arXiv:2604.02701` for spherical arrays,
  Jennison closure/self-cal for the estimator family, `arXiv:2012.05254`).
* **cm differential tracking.** Carrier-phase differences track a person's
  path-length changes; the 6 cm ambiguity is resolved by continuity and the survey.
* **Polarisation and bandwidth diversity.** Dual-pol faces (as ESPARGOS One already
  does) give Jones-matrix channels for material/geometry discrimination; 13 channels
  stitched within a puck give a wide-lane for range and robustness.
* **Environmental change.** Slow changes of the static field (doors, furniture)
  are change detection on the baseline — the same estimator, slower states.

Honesty about sharpness: **angle-sharp and tracking-sharp, range-coarse.** The
invention does not buy bandwidth; it buys aperture and coherence, and those buy
direction and centimetre motion, not millimetre range resolution.

### 2.6 Graceful degradation

If a puck pair fails to lock, the network degrades to (a) coherent clusters
(per-room apertures), (b) the *incoherent* fallback: each puck is a standalone 3D
direction finder (within-puck cancellation, no cross-node phase), and bearings fuse
across pucks by triangulation — a coarse but real sensor (O(10–50 cm), 3D), and (c)
per-link LoSRef-style differential sensing (`arXiv:2602.05344`) that needs no
cross-node phase at all. The fallback network is itself a new composition on
commodity hardware, and it de-risks the primary claim.

## 3. What is genuinely new

| Element | Status elsewhere | New here |
| --- | --- | --- |
| Shared-clock multi-chip coherent array on ESP32-family | ESTABLISHED: ESPARGOS planar 4×2 (`arXiv:2502.09405`); esp-crab common crystal, incl. a C3+S3 clock-buffered board (ESP-Techpedia; `github.com/espressif/esp-csi/tree/master/examples/esp-crab`) | **Volumetric 20-face puck**: no ESP32-class 3D/volumetric coherent array exists; full-sphere phase metering is new |
| Conducted 2.4 GHz phase reference | ESTABLISHED on ESPARGOS (resistive splitter + RF switch) | Distribution into a *polyhedron* with a hub, plus per-channel/thermal re-verify discipline |
| Geometry-based phase calibration of distributed arrays | ESTABLISHED: DICHASUS (`arXiv:2206.05984`), LoS + known positions | Applied to *self-surveying* pucks; NLoS/static-field generalisation |
| Over-the-air phase synchronisation | ESTABLISHED on SDRs/simulations: BeamSync (`arXiv:2311.11070`, dominant direction is optimal), OTA reciprocity in TDD flow (`arXiv:2509.03722`), Nanzer real-time calibration (`arXiv:2506.07267`) | **ARPT**: array-matched-filter *phase transfer* from *arbitrary* transmissions (ambient included), not a dedicated sync waveform; tracking without any two-way exchange |
| Static clutter as a synchronisation reference | ESTABLISHED in radar: airborne DCAR clutter sync (DOI 10.1016/j.cja.2021.08.040); LoS-path referencing in Wi-Fi (LoSRef, `arXiv:2602.05344`) | **The Clutter Clock**: continuous common-mode tracking of an indoor mesh's LO phase field; the room as the clock, per frame |
| Closure phase / self-calibration | ESTABLISHED: Jennison 1958; `arXiv:2012.05254`; closure-only imaging `arXiv:1803.07088` | The one-way/two-way closure identity (§7) as a network hardware-fingerprint observable |
| Carrier-phase localisation | ESTABLISHED in GNSS (RTK/LAMBDA); phase-based ranging on commodity Wi-Fi exists (e.g. ToneTrack, SIGCOMM 2015) | Used as the *self-survey bootstrap for array coherence* in a home |
| Mains as a time reference | ESTABLISHED at 50/60 Hz for time ticks (CMU SenSys 2009) | A PLC-band tone *up-multiplied into the radio reference* (SPECULATION, §8) |

The invention is the **combination plus one sharp mechanism**: an array is not only
an aperture but also the *instrument that transfers phase*; consequently the
coherence infrastructure can be deleted. No cable, no master clock, no sync signal,
no two-way exchange — a policy the literature does not currently offer for
commodity hardware.

## 4. What must be true (and how each is checked)

| # | Assumption | Grade | Test |
| --- | --- | --- | --- |
| A1 | Intra-puck face phases are stable for hours with a shared clock | ESTABLISHED (ESPARGOS Fig. 6: stable unless TX moved) | re-run on S3; add thermal step |
| A2 | Conducted reference + shared clock work on S3 specifically | HYPOTHESIS (proven on ESP32 family; esp-crab pairs C3+S3) | bench: 8-face S3 board, phase stability |
| A3 | Per-frame CSI phase noise, after sanitisation, permits `√N_f` averaging | HYPOTHESIS (B2B calibration 0.117 rad RMSE, `PMC6263436`; per-frame errors decorrelate ≥50 ms, `arXiv:2307.12126`) | LOCK-1 residual decomposition |
| A4 | Sustained CSI capture ≥100–1000 frames/s/node | HYPOTHESIS (current tools 20–180 pps are *streaming* limits) | rate ramp on S3 + UDP aggregation |
| A5 | CSI available for control frames (hardware-ACK round trip) | SPECULATION (IDF exposes a ctrl filter, `docs.espressif.com` S3 Wi-Fi guide) | LOCK-0: capture ACK CSI; if it works, add 16 µs LO-clean round trips as a bonus observable |
| A6 | Legal beacon rate/duty at 2.4 GHz under ACMA LIPD / FCC 15.247 | ESTABLISHED (note `07`) | n/a |
| A7 | Multipath bias is *stationary* rather than *absent* | HYPOTHESIS (supported by DICHASUS; challenged by moving people) | LOCK-1 with a person walking |
| A8 | Geometry survey converges to λ/2 without external anchors | HYPOTHESIS | survey sub-experiment; simulate ambiguity resolution |
| A9 | The residual after static subtraction is dominated by targets (not drift) | HYPOTHESIS | LOCK-1 coherent-gain measurement |

## 5. The decisive experiment: LOCK-1

**Rig.** Two pucks (start with two 8-face S3 boards in the ESPARGOS/CACC spirit),
3–8 m apart, in a furnished room with NLoS and LoS configurations. A coax carrying
the shared 40 MHz and a 2.4 GHz tone connects them **as ground truth only** — the
estimator never uses it. A host records CSI; a separate S3 transmits controlled
frames at 20–1000 Hz; a person walks; a heat source provides a temperature step.

**Procedure.**
1. Record ground-truth relative phase `θ_P−θ_Q(t)` from the conducted tone.
2. Estimate it from ARPT + robust common-mode tracking using **only** over-the-air
   frames (self-beacons and ambient traffic), at each rate and condition.
3. Residual analysis: `σ_φ(t)`, its autocorrelation and Allan-type curve; split into
   measurement noise, tracker process noise, and multipath bias drift.
4. Coherent gain: matched-filter SNR of a moving target's residual, coherent vs
   incoherent combining.
5. In the same rig, report the two other decisive numbers: the raw and post-CFO
   relative-LO phase random walk (the synthesis's M2), and the CSI-rate ceiling.

**Decision.** `σ_φ ≲ 0.4 rad` (λ/16) with motion ⇒ the coherent home is viable;
0.4–1 rad ⇒ partial coherence only (per-room apertures); `≳ 1 rad` ⇒ **the space is
closed for cable-free home-scale coherence on these radios**, and the boundary
(§8) applies. Falsified outright if the estimator's residual *tracks the person*
(the lock is being stolen by the signal) or if the coherent gain equals the
incoherent gain.

**Why this is decisive.** It tests the entire invention in its simplest form — one
pair, one scalar, one reference — with a ground truth that separates "*no
coherence*" from "*coherence but bad estimator*", and it simultaneously answers
M2, the S3 CSI-rate ceiling, and the multipath-stationarity question. It is
buildable from two ESPARGOS-style boards and a scope; no polyhedron is required
for the first result.

Supporting experiments, cheapest first: **LOCK-0** (ACK-CSI / SIFS round trip —
bench, one day); **PUCK-0** (single puck: conducted re-verify cadence, thermal
drift, 3D manifold calibration, dual-pol); **SURVEY-0** (two pucks + FTM + bearings
→ geometry; then carrier-phase refinement offline); **HOME-0** (six pucks, static
field baseline, ±1–2° imaging of a walker versus camera/ground truth).

## 6. ESP32-S3 feasibility

| Requirement | S3/ESP32-family evidence | Gap |
| --- | --- | --- |
| Shared clock across chips | ESPARGOS daisy-chained 40 MHz; esp-crab clock buffer (C3+S3 board) | none material; fan-out skew budget in CAD |
| Conducted 2.4 GHz phase reference | ESPARGOS resistive splitter + RF switch; per-channel PLL re-randomisation known | replicate on S3; hub design |
| CSI phase, stable over long intervals | ESPARGOS Fig. 6 | S3-specific noise/temperature |
| CSI capture rate | ESPARGOS "realtime-capable"; community tools 20–180 pps (streaming-limited) | measure true capture ceiling |
| ns RX timestamps | `WIFI_PKT_RX_TIMESTAMP_NSEC`, 12.5 ns grid (`17`) | none |
| TX of arbitrary frames | `esp_wifi_80211_tx`; ESP-NOW is the CSI-friendly excitation (`07`, `00` §8) | legal duty; no TX timestamp |
| Coherent TX across faces | shared clock + hub trigger gives fixed phase relations; coarse steering by face subset; fine steering needs per-face phase shifters (~$1.5 × 20) | SPECULATION/design |
| Raw I/Q (optional) | ESP-SDR: phase-coherent gapped I/Q on ESP32 (`espargos.net/espsdr/`) | bandwidth/gaps; not needed for CSI path |
| Compute | host-side estimator; ESP-DL not required; hub FPGA optional for aggregation/triggering | none material |

Nothing in the critical path requires SDR-grade hardware; the *only* S3-specific
unknowns are A2–A4 and A7, all bench-measurable.

## 7. One algebraic bonus: the mixed closure identity

For three nodes `i,j,k`, one-way phases give the closure
`φ_ij + φ_jk + φ_ki = (channel closure) + (LO drifts over the round)`; the
round-trip products give `arg(P_ij P_jk P_ki) = 2(c_i+c_j+c_k) + 2(channel closure)`,
where `c` are per-node chain/Lo terms. Subtracting twice the one-way closure from
the two-way closure leaves `2(c_i+c_j+c_k)`: a **per-node hardware/LO fingerprint**
that is invariant to the channels. This is a "what does the round-trip scheme
recover that is new" answer: *per-node chain identity*, useful for authentication,
for seeding the estimator, and as a fault detector. Its practical value is bounded
by the drift residual over the round-trip interval — i.e. by M2 — and is therefore
part of LOCK-1's analysis, not a separate programme. (HYPOTHESIS; algebra is
elementary, utility is unproven.)

## 8. If it fails: the boundary, the closing quantity, the minimal reference

**The closing quantity.** The relative-LO phase random walk over the observation
interval. Define the **coherence budget** `B = Δν_res · T_obs`, where `Δν_res` is
the relative LO's residual linewidth after CFO cancellation and `T_obs` the interval
over which the phase must be known (for tracking, the interpolation interval
between reference frames; for a single frame, the frame duration). The lock holds
if the resulting `σ_φ = sqrt(2π Δν_res T_obs)` (plus estimator and multipath terms,
divided by the array gain) is ≲ λ/16 ≈ 0.4 rad. The synthesis's numbers put
`Δν_res` at 10–100 Hz after CFO removal, i.e. `σ_φ = 1 rad` at `T_obs ≈ 2–16 ms`:
**the electrically closed boundary sits near a few-millisecond observation interval
for a single link.** The array gain of ARPT is precisely what moves that boundary
by `√N_f` (and more, with multi-frame integration). If observation rates cannot
reach the required interval — the MAC/frame ceiling — the space closes.

**Minimal references that cross it** (in ascending cost):
1. the polyhedron itself (intra-puck, already given) — all sharpness claims
   survive *within 30 cm*;
2. one coax **coherence splice** between adjacent pucks — O(number of splices)
   cables turn disconnected atoms into small rigid apertures (0.5–2 m);
3. a TCXO/OCXO reference upgrade per puck (~$2–10) — reduces `Δν_res`, raising the
   maximum observation interval;
4. a **mains-borne tone** (SPECULATION): the home's existing wiring is a cable
   reaching every room. The hub divides its 40 MHz clock down to a PLC-band tone
   (e.g. 625 kHz), injects it into the mains, and each puck up-multiplies it to
   40 MHz for its reference input (external-clock support exists on ESP32-family
   XTAL_P pins). Channel delay and attenuation are static and calibratable; only
   the tone's *phase* is used, so a PLL lock suffices. Two independent 50 Hz ticks
   are already known to be grid-coherent (CMU SenSys 2009); PLC time sync is
   established at ms level (IFAC 2015, DOI 10.1016/j.ifacol.2015.12.031). Whether
   a narrowband tone survives modern SMPS noise *and* meets conducted-emission
   limits is the open question — the honest, boring fallback remains item 2;
5. OTA reciprocity at the MAC limit (if M2 permits) — explicitly deferred to
   LOCK-1's number.

## 9. Chaining, existing models/data, own data

**Chaining (A).** The ladder is a dependency chain: *conducted intra-puck coherence*
⇒ *ARPT inter-puck phase transfer* ⇒ *self-survey (geometry)* ⇒ *coherent aperture*
⇒ *residual imaging* ⇒ *mapping/change detection*. The highest-leverage enabler is
ARPT (§2.1); the capability it unlocks that nothing else does is **coherent
cross-room imaging without new infrastructure**. The fallback chain
(*within-puck cancellation* ⇒ *multi-view 3D DOA* ⇒ *triangulation*) is independent
of ARPT and works today on the same boards.

**Existing models/data (B).** Reusable: ESPARGOS datasets and `pyespargos` demos
(MUSIC spectrum, azimuth–delay, camera) — `espargos.net`, `github.com/ESPARGOS`;
DICHASUS distributed channel-sounder datasets and calibration code
(`arXiv:2206.05984`); BeamSync/OTA-calibration algorithms
(`arXiv:2311.11070`, `arXiv:2509.03722`); CACC/CASR sanitisation recipes
(`PMC9721356`); closure-phase self-cal tools from radio astronomy
(`arXiv:1803.07088`). Aquila can use all of these: licences are open for the
reference implementations, inputs are CSI/FTM — the same primitives v1 already
streams. Note `19`'s supervision ladder applies to targets, not to the coherence
core: this invention deliberately moves the burden from data to physics.

**Own data / new technology (C).** The minimal new dataset is LOCK-1's: **two
simultaneously recorded CSI streams from phase-coherent boards plus a conducted
ground-truth phase channel.** No public dataset contains ground-truth inter-node
phase on ESP32-class hardware (`00` §9); publishing one would let the whole field
falsify or adopt the method. Minimal experiment to prove the innermost claim: any
single common frame heard by two multi-face coherent node groups, showing
`σ_φ(θ_P−θ_Q)` at the array-gain limit — falsified if the per-frame estimate error
does not improve as `1/√N_f` (i.e. the array is not acting as a phase meter).

## 10. Cost

Indicative BOM per puck (20 faces): ESP32-S3 chips/modules 20 × ~$2–5 = $40–100;
4-layer PCBs + flex/soldered edges ~$25–50; RF switches, splitter, clock buffer,
connectors ~$10–20; hub (Ethernet switch or small FPGA + MCU) ~$10–30; enclosure/
assembly ~$10–25. **Prototype ≈ $120–250; volume ≈ $60–100.** Six-puck home:
≈ $700–1500 prototype, ≈ $400–600 volume. Per-node marginal cost for extra rooms
is one puck. (ESTIMATE, not quote; the polyhedron adds mechanical assembly labour
that small runs punish.)

## 11. Transmit required?

| Function | Transmit needed? |
| --- | --- |
| Phase lock (ARPT, tracking) | **No** — listening to ambient or self frames suffices |
| Absolute acquisition of the 2π gauge | No if geometry is surveyed; a beamed beacon shortens it |
| Illumination for sensing | **Yes** — one puck at a time, ESP-NOW/`esp_wifi_80211_tx` frames |
| FTM self-survey | Yes (FTM initiator) |
| Coherent TX beam within a puck | Yes; fixed patterns from shared-clock trigger; fine steering optional (phase shifters) |

## 12. Evidence strength and residual unknowns

**Overall: 4/10 for the invention as a whole** (mechanism plausible and assembled
from established parts; no part of it demonstrated end-to-end on commodity
hardware). Component grades: intra-puck coherence 8/10 (ESPARGOS, esp-crab);
OTA/D-MIMO phase calibration theory 7/10 (SDR-grade and simulations, not S3);
geometry-based calibration 7/10 (DICHASUS); static-clutter sync 5/10 (radar
literature, not indoor mesh); GNSS-style self-survey indoors 3/10 (analogous
techniques exist; no home demonstration); mains-tone phase transfer 1/10
(SPECULATION).

Residual unknowns, in order of consequence:
1. LOCK-1's number: tracked `σ_φ` under motion on S3 hardware.
2. S3 CSI capture ceiling and the true per-frame phase-noise law (A3, A4).
3. Whether the array-matched-filter phase estimate stays unbiased as the room's
   field changes (multipath *bias drift*, not exclusion).
4. Self-survey convergence to λ/2 indoors without anchors (A8).
5. Thermal behaviour of 20 hot chips on one rigid body (A1's weak spot).
6. Mutual coupling and face shadowing in a solid conducting polyhedron — no
   published measurement exists.
7. The relationship to v1 vocabulary: `CONTEXT.md` lists "phase-coherent RF
   reference" under *explicitly not used* and bans ESP-NOW by name; this invention
   is Track-3 research that would require a glossary/ADR decision before it could
   approach a spec (the same conflict `00` §8 already flags). Research note only.

## 13. Sources

* ESPARGOS: `arXiv:2502.09405`; project site `https://espargos.net/`; datasets
  `https://espargos.net/datasets/`; ESP-SDR `https://espargos.net/espsdr/`.
* esp-crab (Espressif common-crystal multi-chip CSI, incl. C3+S3 clock-buffered
  board): `https://docs.espressif.com/projects/esp-techpedia/en/latest/esp-friends/solution-introduction/esp-csi/esp-csi-solution.html`;
  `https://github.com/espressif/esp-csi/tree/master/examples/esp-crab`.
* ESP32-S3 external clock / shared-clock boot notes: `https://esp32.com/viewtopic.php?t=2742`;
  `https://forum.digikey.com/t/esp32-c5-boot-failure-with-shared-external-clock/69516`.
* ESP32-S3 Wi-Fi driver (CSI, promiscuous filters, `esp_wifi_80211_tx`):
  `https://docs.espressif.com/projects/esp-idf/en/v4.4.3/esp32s3/api-guides/wifi.html`.
* BeamSync: `arXiv:2311.11070`; DOI 10.1109/TWC.2023.3335089.
* OTA phase calibration in the TDD flow: `arXiv:2509.03722`. Repeater-aided OTA
  phase sync: `arXiv:2603.19903`. Partially coherent D-MIMO: DOI
  10.1109/OJCOMS.2024.3373170. Consensus + Kalman distributed arrays:
  `arXiv:2201.08931`. Real-time digital wireless T/F/phase calibration:
  `arXiv:2506.07267`.
* Geometry-based phase/time synchronisation (DICHASUS): `arXiv:2206.05984`.
* Clutter-based DCAR synchronisation: DOI 10.1016/j.cja.2021.08.040.
* LoSRef Wi-Fi radar OTA referencing: `arXiv:2602.05344`.
* Back-to-back CSI phase calibration: `PMC6263436` (DOI 10.3390/s18113912).
  Optimal CSI preprocessing / phase-error decorrelation: `arXiv:2307.12126`.
* CSI phase sanitisation CACC/CASR: `PMC9721356`; `arXiv:2508.12614`;
  Tsinghua WST sanitisation notes `tns.thss.tsinghua.edu.cn/wst/docs/sanitization`.
* Closure phase and self-calibration: Jennison 1958; `arXiv:2012.05254`;
  closure-only imaging `arXiv:1803.07088`.
* Spherical/volumetric arrays: `arXiv:2604.02701`; 3D array DOA precision
  DOI 10.1016/j.aeue.2017.01.023 (S1434841116310342).
* Mains as a reference: CMU SenSys 2009
  `https://www.andrew.cmu.edu/user/agr/pubpg/sensys-09.pdf`; PLC time sync
  DOI 10.1016/j.ifacol.2015.12.031.
* Internal: `00-synthesis.md` §§1–9; notes `07` (legal), `08` (geometries),
  `09` (TDM), `10` (reciprocity), `11` (common-ref), `12` (oscillators),
  `13` (static multipath), `15` (self-localisation), `17` (timing), `19`
  (learning), `20` (boundary).
