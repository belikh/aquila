# 22 — Invention M: The Ambient-Coherent Aperture (ACA)

### A home that inherits coherence from its own traffic — a receive-only, self-surveying 3D aperture built from ESP32-S3 polyhedra

Status: invention proposal + boundary analysis. Australian English. Grades as in the v2
notes: **ESTABLISHED / HYPOTHESIS / SPECULATION**. Cross-references to notes `01`–`20`;
`00-synthesis.md` integrates them. Sibling proposal `22-invention-a.md` (the *Coherent
Weave*) pursues the **active** route (TDM rounds, dedicated reference frames, disciplined
oscillators). This note pursues the **passive** route. The two share the boundary, the
polyhedron, and the decisive measurement; they differ in what crosses the boundary and who
pays for it.

---

## 0. Mission, verdict, and the one-line invention

> Discover the new invention that makes a home full of cheap ESP32-S3 nodes behave as one
> sharp, self-calibrating, generalising 3D sensor — or prove that no such invention exists
> and state the exact physical boundary that blocks it.

**Verdict.** The space is **not** closed by physics. The exact boundary is the
relative-LO-phase gauge (§1). It is crossed for free in any home, because the home's own
access point already transmits the one thing that is missing: frames that every node can
observe *simultaneously*. The invention is to stop treating those frames as interference
and start treating them as **the shared phase reference and the illumination at once**:

> **Ambient-Coherent Aperture (ACA)** — *any 802.11 frame that two Aquila nodes both
> receive is, at that instant, a free, simultaneous, whole-home coherent phase reference.
> The frequency-flat part of the cross-node CSI phase is their relative LO phase; the
> frequency-slope difference is their differential time of flight. A home of polyhedral
> pucks that harvests these frames forms one coherent 3D aperture with no cables, no
> disciplined oscillators, and — for sensing — no transmissions at all.*

Honest grading: the *mechanism* is assembled from established pieces (CSI phase
sanitisation; two-way carrier phase; cell-free OTA calibration; conducted ESP32 phase
references; Kalman phase tracking). The *combination is new* and, as far as this sweep can
determine, unpublished: ambient, non-cooperative OFDM frames used as a **continuous
cross-node phase ledger** to federate independent apertures, with the OFDM subcarrier axis
substituted for the *temporal* non-stationarity that related 2026 work requires of a
*cooperative* source [GHR]. The one unmeasured quantity that can still close the space in
practice is **M2** — the residual per-frame cross-receiver phase after CFO removal,
*without* a dedicated reference frame. The decisive experiment (§9, E1) measures exactly it.

---

## 1. The boundary, stated exactly

### 1.1 What is unobservable (ESTABLISHED — note `20`, primaries below)

With independent local oscillators, one CSI observation takes the form

```
Ĥ_ij(f_k) = g_ij · a_i · b_j · e^{ j(θ_i − ψ_j) } · e^{ −j2π f_k τ }        (1)
```

Only the *difference* `θ_i − ψ_j` (node `i`'s receive-LO phase against node `j`'s
transmit-LO phase) is observable. Across any connected measurement graph the node phases
are fixed only up to one global additive constant — a `U(1)` gauge — plus time and
frequency gauges [20; synthesis §1]. Invariants: amplitudes; per-link subcarrier
differential phase (delay/ToF); per-receiver inter-antenna differential phase (AoA *inside*
one coherent receiver); two-way products; temporal differentials. **Not observable from
node-local data:** cross-node phase, cross-node AoA, phase-of-flight range, absolute
synchronisation.

### 1.2 The minimal reference, and the one hard theorem

Three escapes are published: a **common-transmitter reference**, **OTA reciprocity**, a
**shared/conducted reference** [20; 11]. Two facts sharpen this note:

1. **Any one common observation of a node pair fixes their relative phase for that
   instant** — nothing more is needed, nothing less works. A common frame *is* a common
   transmitter observation: the transmitter's own unknown phase cancels in the
   *cross-receiver difference*.
2. **If two individually calibrated arrays share no common observation, no processing
   aligns them.** Vieira & Larsson state this as a "myth/reality" pair: two R- or
   F-calibrated arrays still require a bidirectional OTA measurement to be jointly
   calibrated [V&L]. This is the only genuine wall, and it is about *connectivity*, not
   about distance or hardware.

A corollary that kills a common over-claim:

> **The environment is a phase *memory*, not a phase *source*.** A static scatterer
> observed by two receivers cannot tell them their relative LO phase, because the
> scatterer's illumination phase is unknown to both. Once a reference *has* been
> established — by a common frame, a two-way exchange, or a conducted link — the static
> part of the channel becomes a *stable holdover standard*: it moves only when the
> environment moves. Radar has used exactly this for years: clutter-based synchronisation
> in distributed coherent aperture radar [CJA22] and coherent-on-receive synthesis on a
> dominant scatterer [RS23]. It is memory, and memory is what a ledger needs.

### 1.3 The quantity that could still close the space (M2)

Raw LO offset on ±10 ppm crystals wraps the relative phase on a 5–10 µs scale; after CFO
removal the residual governs, and COTS practice shows the *common phase rotates per frame*
(PLL re-lock, CFO accumulation) — on Intel AX210 WiDRa observes a two-valued `2πn/N`
per-frame rotation with `N = 2` [WiDRa]. The governing number is therefore not a coherence
time but a **per-frame cross-receiver phase residual without a dedicated reference frame**.
Call it **M2**. It is unmeasured on ESP32-S3.

Two pieces of 2026 evidence bracket it:

- **ESP-PPB** (ESP32-C3, GPL-3.0, *unrefereed*, Feb 2026): FTM-steered VCTCXO (~1 PPB
  typical) plus a master **reference frame 1 ms after every external CSI frame**; the
  project reports **single-frame phase accuracy < 1° (lab), < 10° (typical)** and stable,
  flat cross-node phase offsets across 52 subcarriers [PPB]. This is grey literature and
  must be replicated, but it says the *hardware class* can hold the per-frame
  constant-phase term to ≲0.17 rad — **with** a dedicated reference frame.
- **WiDRa** (COTS Intel AX210, PicoScenes): two-way sum-carrier-phase differential ranging
  at **< 1 mm RMSE** on real hardware, limited only by carrier frequency, not bandwidth
  [WiDRa]. Its MAC-imposed 0.5 ms minimum round trip is what causes residual phase noise
  and cycle slips; its authors explicitly defer direction finding as future work.

The ACA's wager, stated as a falsifiable claim: **M2 ≤ 0.2 rad per frame is achievable from
a third-party frame with no dedicated reference**, because the OFDM subcarrier axis
supplies the lever arm that ESP-PPB gets from its 1 ms reference frame and that GHR gets
from source motion.

---

## 2. The physical mechanism

### 2.1 One frame, two observables, zero exchange (the core)

For one ambient frame `p` — from *any* transmitter (the home AP's beacon, an uplink frame
from a phone, a neighbour's AP) — received by nodes `i` and `j`, write the sanitised CSI
phase (linear STO/SFO ramps removed by a per-node least-squares or PADS-style edge-tone
fit [SSDS; PADS]) as a function of subcarrier `k`:

```
∠Ĥ_i,p(k) =  α_i,p  +  2π f_k (τ_i + δ_i)  +  ε_i,p(k)                       (2)
```

where `α_i,p = θ_s,p − ψ_i,p + c_i` collects the source's phase at transmission (common to
*all* receivers of this frame), node `i`'s LO phase at reception, and node `i`'s hardware
constant (PLL initial phase, analog chain; per boot/channel); `τ_i` is the propagation
delay; `δ_i` the packet-detection delay (absorbed in the ramp). Forming the
**cross-receiver ratio** of the two CSIs of the *same* frame,

```
r(k) = Ĥ_i,p(k) / Ĥ_j,p(k) = ρ · exp{ j[ (α_i,p − α_j,p) + 2π f_k (τ_i − τ_j) ] }   (3)
```

gives, per frame, by a 2-parameter fit over `K` subcarriers:

- **Δψ := intercept** — the relative LO phase `ψ_j,p − ψ_i,p` plus the constant
  `c_i − c_j`: the *frequency-flat* term. **The source phase cancels identically.**
- **Δτ := slope / 2π** — the differential time of flight `(d_i − d_j)/c`: the
  *frequency-linear* term. **Unambiguous** across a 20/40 MHz band (folding at
  `1/Δf_sc = 3.2 µs`), so no integer ambiguity — unlike carrier-phase ranging between
  unsynchronised clocks.

This is the whole trick, and it is the *spectral* analogue of the GHR framework's temporal
decoupling [GHR]: GHR needs a *cooperative* source radiating a *known non-stationary*
waveform (LFM) so that intercept (phase error) and slope (clock error) become identifiable.
Here the OFDM frame itself supplies the frequency axis, so the source may be **unknown,
non-cooperative and ordinary**. GHR's own dynamic-observability condition (`CRB → ∞` if
`Var(ω) = 0`, i.e. a stationary single-tone source) maps to: *a single-tone source cannot
be used; an OFDM frame can*, because `f_k` spans many bins within one frame [GHR].

Two further consequences of Eq. (3):

- **No π ambiguity.** Bidirectional reciprocity calibration measures `2(c_A − c_B)` and is
  π-ambiguous, resolvable only with a second frequency [V&L]. The ambient common-frame
  difference measures `Δψ` *directly*. Active and passive modes are therefore exact
  complements: the two-way mode gives range and hardware asymmetry, the ambient mode gives
  phase without the halving.
- **Simultaneity for free.** Both nodes observe the *same physical transmission*; their
  sampling instants differ by propagation only (≤ ~100 ns across a home), over which LO
  phase drift is < 1e-4 rad even at 100 Hz residual offset. There is no holdover inside a
  frame.

### 2.2 The estimator's noise (arithmetic, HYPOTHESIS)

For least-squares fits of Eq. (3) over a centred subcarrier index with per-subcarrier phase
noise `σφ`:

```
Var(Δψ̂) ≈ σφ² / K                                        (intercept; LO observable)
σ(Δτ̂)  ≈ σφ / ( 2π · Δf_sc · √(K(K²−1)/12) )             (slope; two nodes double it)
```

With `σφ = 0.1 rad`, `K = 52`, `Δf_sc = 312.5 kHz`: `σ(Δψ̂) ≈ 0.055 rad` (plus multipath
bias) and `σ(Δτ̂) ≈ 0.5–0.7 ns ≈ 15–20 cm` per frame (HT40, `K ≈ 114`: ≈ 5–8 cm). Averaging
over `N` frames improves both as `1/√N`. ESP-PPB's reported single-frame phase accuracy of
1–10° [PPB] is consistent with this budget. The **bias**, not the noise, is the real risk:
the intercept estimator is biased by multipath, and unlike the noise that bias is
receiver- and source-specific (§2.4).

### 2.3 The ledger: coherence as a tracked state, not a constant

Per node pair, the relative LO phase is a slowly-varying state `Δψ(t)` driven by residual
CFO and phase noise; every ambient frame is an (almost) instantaneous measurement of it.
The estimator the distributed-MIMO literature has converged on is the right one: a
**Kalman filter with circular measurement wrapping**, re-estimating whenever the LOs have
drifted significantly [NL24; NL25], with consensus variants reaching similar conclusions
[RN23]. Two ACA specifics:

- **Single-frame coherence.** For processing that consumes *one* frame, the aperture is
  coherent *within that frame* to `σ(Δψ̂)`. Coherent aperture synthesis therefore needs no
  holdover at all — it **inherits** coherence from the illuminator. Multi-frame coherent
  integration uses the ledger's prediction *between* frames.
- **Cycle slips are the failure mode**, not slow drift: if the gap between frames a pair
  both hears is long relative to the residual CFO, the tracked phase wraps. WiDRa reports
  exactly this at 0.5 ms gaps with 4–8 kHz residual CFO [WiDRa]. At 2.4 GHz, 20 ppm is
  ~48 kHz full offset; a 10–100 Hz residual after CFO removal wraps in 5–50 ms. Ambient
  frame rates in an occupied home run 10–1000 Hz; a quiet home degrades to beacon rates
  (~10 Hz per AP), where an optional TCXO (not a disciplined oscillator) or a single
  beacon-puck transmission per 10 ms closes the gap. This is the quantitative reason the
  active sibling proposal keeps a rare round: the ledger needs *some* frames, and in a
  silent home Aquila must provide them.

### 2.4 Bias: the environment as memory, the anchors as truth

The intercept of Eq. (3) is biased by everything that makes `∠Ĥ(k)` non-linear: the
multipath from source `s` to node `i`, which differs for every source direction and changes
when the scene changes. Consequences and cures:

1. **Static scene ⇒ constant bias**, absorbed once into the per-pair calibration constant.
   Only *environment change* moves it. The environment is the ledger's memory; the ledger
   must not confuse memory with drift.
2. **Different sources ⇒ different biases, one shared `Δψ(t)`.** This does not cancel the
   biases by itself (frames from different sources arrive at different times), but it makes
   them *identifiable* given (a) a known anchor and (b) temporal smoothness of `Δψ`. The
   practical estimator is a Kalman filter per pair with a per-source bias state, re-anchored
   periodically (§2.5) and predicted by the scatterer map once it exists (§2.6). Multi-source
   diversity shortens identifiability; it is not magic.
3. **The scatterer map is the cure.** After the self-survey (§5–6), the static multipath
   from each source direction is *predictable*, so the bias is computable and removable.
   That is the correct form of "the environment as a reference": the walls stop being noise
   once the aperture knows where they are.

### 2.5 The anchor and fallback: ping + unique replies (direction 5)

The ambient ledger needs an absolute anchor once per boot/channel (the hardware constants
`c_i − c_j`) and a fallback where no common frame exists. Both are served by the
**round-trip exchange** of the brief: one node pings (ESP-NOW/HT40/MCS0 — raw
`esp_wifi_80211_tx` defaults to 1 Mbps 11b, which yields no usable CSI, per `07`); every
other node replies with a **unique** frame (unique ID in the payload, TDM slots by default;
advanced: simultaneous unique sequences recovered offline from ESP-SDR raw I/Q). From the
two CSIs of a pair:

- **Product** `Ĥ_AB·Ĥ_BA` → round-trip phase: LO cancels; range at the carrier frequency
  (mm-class, as WiDRa shows [WiDRa]); residual hardware asymmetry as a stable per-node
  fingerprint (identity + calibration observable).
- **Difference/quotient** → `2(c_A − c_B)` (π-ambiguous) — resolved trivially by *one*
  ambient common frame, or by the two-frequency trick [V&L].
- Coarse ranges from FTM (1–2 m on S3, per `15`) and imperfect MAC timestamps [17]
  bootstrap the geometry for §6.

Cost of the anchor: one RTT per pair per boot (seconds), plus a rare refresh. The two
sibling proposals therefore differ mainly in *cadence*: Weave pays rounds continuously; ACA
pays them almost never and lets the home's own traffic carry the phase.

### 2.6 The learned layer (direction 2, in its place)

Physics does the calibration; learning does the generalisation:

- A **scatterer prior** trained across homes (generalisable wireless radiance fields à la
  GWRF, NeRF²-family [GWRF; NeRF2]) predicts each home's multipath bias structure from a
  few surveyed frames, keeping the ledger honest and speeding the joint solve (§6). The
  learned model is never asked to estimate phase; it estimates *bias*.
- A **discrete prior** on per-boot hardware constants: commodity NIC phase offsets are
  semi-static and often take only two values per boot [SJ21] — a natural per-device
  fingerprint that accelerates re-anchoring.

---

## 3. The invention, unit by unit

The composite invention decomposes into three units, each stated with the fields the
mission requires: what is genuinely new; the physical mechanism; what must be true; the
falsifying experiment; ESP32-S3 feasibility; cost; TX needed.

### U1 — The Passive Phase Ledger (PPL)

| Field | Statement |
| --- | --- |
| **Genuinely new** | Ambient, *non-cooperative* OFDM frames used as a continuous cross-node phase ledger; the intercept/slope decomposition of the cross-receiver CSI ratio giving LO phase and differential ToF simultaneously from one frame; multi-source bias handling with a scatterer-map predictor. Nearest neighbours each miss a piece: GHR (cooperative LFM source; simulation), LoSRef (a single link's own LoS), ESP-PPB (dedicated master reference frame), PCL reference channels (dedicated, per-transmitter), distributed coherent aperture radar (radar's own clutter), the sibling Weave (active rounds). |
| **Physical mechanism** | Eq. (1)–(3): the source phase is common to all receivers of one frame and cancels in cross-receiver differencing; the LO term is frequency-flat, the propagation term frequency-linear; OFDM supplies the lever arm inside the frame. |
| **Must be true** | M2 ≤ ~0.2 rad/frame with no dedicated reference (E1); third-party frames are CSI-capturable on S3 (§10); per-boot constants semi-static; ambient frame rate ≥ drift rate in occupied homes. |
| **Falsifier** | E1: the OTA-minus-oracle residual is per-frame independent with σ > 0.5 rad, or drifts without bound between anchors. Then no passive home aperture exists on this silicon; only the active/conducted routes remain (`22-invention-a`, `11`). |
| **S3 feasibility** | CSI on third-party frames in sniffer mode: community-established (ESP32/S2/C3 repo; S3 used in CSI-Radar projects). Per-frame phase accuracy on the hardware class: ESP-PPB reports < 1–10° on C3 **with** a reference frame. The S3-specific, reference-free residual is the open number. |
| **Cost** | ≈ zero marginal cost: firmware in existing S3 nodes; no VCTCXO, no cables. Optional TCXO ~$2/node for silent homes. |
| **TX needed?** | **No** — the ledger is receive-only in the ambient mode. Optional: one beacon-puck frame per ~10 ms in silent homes (a compliant ESP-NOW/beacon TX). |

### U2 — The federated coherent aperture

| Field | Statement |
| --- | --- |
| **Genuinely new** | A whole-home aperture that is formed *per ambient frame* and therefore never has to *hold* coherence; the gauge for the whole home is fixed by frames no one transmitted for Aquila. Prior coherent distributed Wi-Fi required wired clocks (ESPARGOS, WiCAL), a cooperative master (AirSync, ESP-PPB), or continuous active rounds (WiDRa, Weave). |
| **Physical mechanism** | For any frame `p`, Eq. (3) evaluated across all node pairs gives every node's relative phase at the same instant; steering/imaging then uses the physical node geometry (§5). Multi-frame coherent integration uses the Kalman ledger; incoherent integration is always available as a floor. |
| **Must be true** | M2 (same as U1); all pucks hear a common frame (single-channel home, or one designated channel); geometry known to ≪ λ (§6); coherent gain loss `ε²/2` acceptable at `ε ≈ 0.2 rad` (≈ 2 %). |
| **Falsifier** | E2: a two-puck coherent aperture shows no gain and no resolution improvement over the better single puck against a known reflector, once incoherent baselines are computed with the same data. |
| **S3 feasibility** | ESP-PPB demonstrates cross-node phase-coherent CSI and AoA matrices on four C3 nodes, wireless, battery-powered [PPB]; the physics is the same with ambient frames. ESP-SDR shows the S3 can stream phase-coherent raw I/Q (80 MSa/s with an FPGA) [ESPSDR]. |
| **Cost** | Per node ≈ $3–8 (S3 module); the aperture is software over hardware already specified for Aquila v2. |
| **TX needed?** | No for the aperture itself. |

### U3 — The Aperture Node: a 3D-coherent polyhedron puck

| Field | Statement |
| --- | --- |
| **Genuinely new** | The premise's polyhedron deployed as a *known-geometry ruler*: because the puck's internal 20-face geometry is rigid and factory-known, the home self-survey reduces from "20 unknown node positions per puck" to "one 6-DoF pose per puck" — a pose-graph problem instead of a 120-dimensional one; plus 3D (elevation) sensitivity, polarisation diversity, ~13 dB local array gain, and 20 *simultaneous* coherent receivers (no TDM inside a puck, because each face has its own S3). No ESP32 volumetric sensing array exists today (`00` §7); this note supplies the reason to build one: it is the anchor geometry that makes the home-scale aperture solvable. |
| **Physical mechanism** | Conducted 40 MHz + 2.4 GHz phase reference on the hub face (ESPARGOS topology [ESPARGOS]) fixes intra-puck phases per boot; each face's S3 receives every ambient frame simultaneously; the known face offsets turn cross-face phase differences into direction/carrier-phase constraints. |
| **Must be true** | The conducted reference works on S3 (proven on S2/C61 in ESPARGOS; plausible but unpublished on S3); mutual coupling and face shadowing tolerable; hub bandwidth/aggregation (Ethernet switch or FPGA) sustains 20 streams; the solid-body geometry survives thermal drift to ≪ λ (≈ mm — trivially true mechanically). |
| **Falsifier** | E4: measured 20-face phase residuals after conducted calibration destroy the 3D pattern (e.g. face-to-face coupling shift > 30°) or the 3D DoA of a known reflector is no better than a single face. |
| **S3 feasibility** | 20 × ESP32-S3 per puck: feasible and cheap; ESPARGOS (8 faces, S2) is the existence proof at one quarter the face count; ESP-SDR demonstrates S3 raw-I/Q phase-coherent capture. |
| **Cost** | ≈ $120–250 per puck in materials at qty 100 (20 S3 modules ~$60–100; triangular PCBs and assembly ~$30–60; clock + 2.4 GHz reference distribution ~$20–40; hub switch/FPGA ~$10–50; mechanics/power ~$10). A 6-puck home ≈ $0.7–1.5k, plus the home AP it repurposes. Labelled SPECULATION (price estimates). |
| **TX needed?** | No. Optional coherent TX from one face-set via the shared reference (ESPARGOS states phase-coherent TX is possible on the hardware but unreleased [ESPSDR]) — that is the sibling Weave's "Knot", not this note's core. |

---

## 4. The Aperture Node: design against the hardware premise

The premise is a polyhedron of triangular PCBs (~30 cm diameter), one ESP32-S3 per face,
flex or soldered edges, a hub face carrying a shared 40 MHz clock fan-out, a conducted
2.4 GHz phase reference, and data aggregation. This section says how to use it, and what
must not be assumed.

### 4.1 What the polyhedron is for

1. **Elevation.** A planar array sees a hemisphere; a polyhedron sees the sphere with no
   scan loss — the established motivation for geodesic/spherical arrays and Goldberg
   polyhedra built from flat subarrays [SAA26; GOL25; GDPAA02]. Local angular resolution is
   `λ/D ≈ 12.4 cm / 0.3 m ≈ 24°` (Rayleigh, per plane) — deliberately coarse; the polyhedron
   supplies **3D direction and array gain**, while sharpness comes from the *federation*
   (§5).
2. **Known rigid geometry.** The 20 face centres and their orientations are factory-known to
   ≪ λ. The home's self-survey therefore only solves a pose per puck (§6) — the single most
   valuable thing the polyhedron contributes to the *invention*, as opposed to the hardware.
3. **Polarisation and shadowing diversity.** Faces on opposed sides see different
   polarisation projections and different local clutter; for the ambient ledger this means
   per-puck multipath biases are averaged across faces, not shared — a free decorrelator for
   the bias states of §2.4.
4. **Simultaneity.** Each face has its own S3, so all 20 receive any ambient frame
   simultaneously: no TDM, no switching. (The distinction from `09`'s verdict that a
   *switched* face array would need TDM: switching is not needed if every face has a
   receiver. This is the one place the premise's "one S3 per face" changes the conclusion of
   `08`/`09`.)

### 4.2 What is not yet known (unmeasured risks)

- **Mutual coupling** between faces on a conducting solid: shifts face phases and patterns;
  bounded in classical array practice but unmeasured for this geometry. Mitigation: the
  conducted reference calibrates *total* per-face response including coupling, at the
  reference frequency; coupling changes with the environment much less than with the body.
- **Face shadowing**: the solid blocks ~half the sphere from any face; that is the point of
  the polyhedron, but it also means the *effective* aperture for one source is the
  illuminated subset (≈ 8–12 faces), reducing per-puck gain from 13 dB to ~9–11 dB. Free
  and predictable: face selection is part of beamforming.
- **S3 conducted reference**: ESPARGOS is S2/S3-family parts; ESP-SDR works on S3. Applying
  the exact ESPARGOS calibration loop to S3 is a verification task, not a research risk
  (E2 in `22-invention-a` already targets boot-loop stability).

### 4.3 A cheap variant (worth building first)

The **switched single-chain puck**: one S3, one switch, 20 faces, WiCAL-style calibration
point antennas and redundant sampling to align packets across switch states [WiCAL]. This
gives a *time-multiplexed* aperture, coherent only if the ledger does not slip between
switch slots (it will not for static scenes). It is 20× cheaper and is the natural first
hardware; the 20-S3 puck is the end state.

---

## 5. Sharpness: what actually makes the 3D sensor sharp

### 5.1 The aperture budget

For a home deployment of `P` pucks spanning a baseline `D` (typically 5–15 m), with
per-pair phase residual `ε` (M2 after averaging):

- **Angular resolution:** Rayleigh `λ/D` ⇒ 0.5–1.5° at 2.4 GHz for `D = 5–15 m`
  (practically, elevation from each polyhedron plus house azimuth).
- **Cross-range at range `R`:** `R·λ/D` ⇒ ≈ 4–15 cm at `R = 5 m`. This is the "sharp"
  claim, and it is *not* an aperture synthesis trick: the nodes physically form the
  aperture; the ledger merely makes it usable.
- **Coherent gain:** up to `10·log10(N_faces·P)` dB; loss from residual phase error
  ≈ `ε²/2` ⇒ ≈ 2 % at `ε = 0.2 rad`. Coherent integration across frames extends the
  effective integration time; incoherent integration is the guaranteed floor.
- **Range:** differential ToF per frame (15–20 cm single-frame; 1–5 cm after averaging,
  §2.2) is used for geometry constraints; the *illuminator is uncontrolled*, so target
  range is bistatic (source–target–receiver) and best handled as a *change* variable
  (target motion) plus a mesh of nodes, not as monostatic range.
- **Doppler:** differential phase *rate* across nodes and frames gives the target-induced
  motion signature; this is the one dimension that works at 20 MHz bandwidth on one node
  (`01`, `06`) and is *sharpened* by the aperture (per-node angle × per-node Doppler).

### 5.2 Why "the environment as an antenna" is true, and how far it goes

Stated honestly (§1.2): the environment does not add phase reference; it adds **structure**.

- **Virtual anchors.** Static specular reflections act as extra, though weak, sources at
  known positions once mapped — the same mechanism as Channel-SLAM/SALMA/virtual anchors
  [VA16; SALMA18], and the reason 2026 work can do joint localisation/synchronisation/
  mapping with phase-coherent distributed arrays in theory [FAS24].
- **Aperture extension by reflection.** A wall or a deliberately placed cheap reflector
  (a painted metal sheet) can act as a fold mirror, letting two pucks see a target from a
  third apparent direction. Passive repeater gain is real and old news in radio links
  (100–130 dB cited for large panels at UHF/microwave [RM69]); at home scale the gain is
  modest but the *geometric* value (extra independent look direction) is the point.
- **The limit:** diffuse scatterers (plasterboard, cloth, people) do not obey the
  single-bounce model and must not be trusted as anchors; the CIR must be modelled with
  diffuse components or the maps will hallucinate anchors (the `04`/`16` caveats apply).

### 5.3 The sensing pipeline (one paragraph)

Every ambient frame that two or more pucks hear is (i) a ledger measurement (§2), and
(ii) a coherent snapshot: cross-node phases de-rotated by the ledger, per-face responses
calibrated, steering performed over the known pose graph, and a matched-field/beamforming
map computed over the home volume. Moving targets appear as *differential* changes between
snapshots (the only robust discriminator at commodity bandwidth, per `01`/`06`/`13`);
static structure is accumulated into the scatterer map (§6). Localisation of cooperative
targets (phones, pucks) is the same machinery with the target as the source; FTM and the
two-way anchor supply absolute ranges when needed.

---

## 6. Self-calibration: one joint solve, physics-first

### 6.1 The unknowns and the constraints

| Unknown | Dimension | Constraint source |
| --- | --- | --- |
| Puck poses (6-DoF each; faces rigidly known) | 6·P | differential ToF slopes `Δτ` across pairs and sources (§2.1); FTM coarse ranges; two-way round-trip phases |
| Per-node hardware constants `c_i` (per boot) | N | one anchor (two-way or one conducted calibration); ambient frames track thereafter |
| Per-pair, per-source multipath biases | N·S (sparse, slow) | known source directions after pose solve; static-scene constancy; scatterer map prediction |
| Source positions (APs, phones) | 3·S (or directions) | differential ToF/DoA across the aperture — the aperture *is* the sensor |
| Static scatterer map (walls, furniture) | ~10²–10³ points | phase-coherent snapshots over time [FAS24; VA16]; cross-home prior [GWRF] |
| LO ledger states | 2 per pair | per-frame `Δψ` measurements (§2.3) |

The natural solver is a **factor graph / bundle adjustment**: initialise pose from FTM+MDS
(`15`), phases from one anchor, then iterate. Fascista et al. give the ML estimator and FIM
for exactly this family (with APs assumed already coherent) and show how LoS phase, wall
reflections and scatterers each contribute [FAS24] — the ACA contributes the *previously
missing* AP-side coherence mechanism.

### 6.2 Why this is "self-calibrating" and generalising

- **Self-calibrating:** no surveyor, no tape measure, no known anchor positions required;
  the gauge is fixed by physics (a common frame / one exchange) and the geometry emerges
  from the aperture itself.
- **Generalising across homes:** the calibration pipeline is physics, identical everywhere.
  What varies per home is clutter, which is precisely what a *learned radiance-field prior*
  handles (GWRF generalises across scenes without per-scene training [GWRF]). The learned
  layer never touches phase or geometry; it only initialises and regularises the scatterer
  map. Transfer of the *hardware* constants is per-device, not per-home (a cheap fingerprint
  [SJ21]).

---

## 7. The six directions, answered

| # | Direction | Verdict |
| --- | --- | --- |
| 1 | **Wireless coherence on commodity silicon** | **Solved in principle, passively.** The common-transmitter escape is established [20; 11]; this note makes the common transmitter *ambient and non-cooperative*, gives the per-frame estimator, and removes the dedicated reference frame ESP-PPB needed [PPB]. Decisive residual: M2 (§1.3, E1). |
| 2 | **Self-calibrating learned array; joint geometry+targets; cross-home transfer** | **Adopted, split by role.** Joint geometry/scene/phase estimation is established theory [FAS24; VA16]; channel charting handles relative geometry label-free [CC24]. The *new* part is gauge-fixing by ambient frames; learning is confined to the scatterer prior (GWRF-style generalisation [GWRF]) and per-device phase priors [SJ21]. |
| 3 | **Environment as shared reference → distributed aperture** | **Corrected and adopted.** Static scatterers are a phase *memory*, not a source (§1.2) — established in radar as clutter/dominant-scatterer sync [CJA22; RS23]. With the ledger, they become predictable bias (removable) and virtual anchors (added constraints), i.e. the environment is turned into *structure*, not into a clock. |
| 4 | **Sharp 3D from many coarse apertures** | **Adopted with numbers.** Many 24° pucks federated by the ledger and the pose graph ⇒ 0.5–1.5° house-scale resolution (§5.1); 3D from polyhedron elevation; polarisation/shadow diversity as bias decorrelators; the round-trip *hardware-asymmetry signature* as the identity/calibration observable (§2.5). Injection locking (§B.1) and monostatic operation (`08`) rejected. |
| 5 | **Ping + unique replies + two-way products** | **Adopted as anchor/fallback, not as the primary clock.** Product → mm-class body-scale range [WiDRa]; difference → π-ambiguous phase constants resolved by one ambient frame or two frequencies [V&L]; unique IDs → identity and TDM/CDM separation. The ambient mode supersedes it for cadence; the two-way mode is where the ledger is blind (no traffic, no common source). |
| 6 | **Different excitation/composition** | **Mostly rejected, one keystone kept.** Passive illumination by incumbents is the whole idea. TDM across pucks is unnecessary in receive-only ambient mode; intra-puck TDM is unnecessary with one S3 per face. Coherent TX within a puck via the conducted reference is *possible* [ESPSDR] but is the sibling Weave's primitive, not needed here. Synthetic apertures by *motion* (`16`) remain out of scope (static nodes). |

---

## 8. Prior art, and the exact deltas

| Prior art | What it establishes | What it does *not* do (ACA's delta) |
| --- | --- | --- |
| **AirSync** (WARP FPGA) [AIR13] | OTA master-pilot phase sync between distributed APs; 2.37° std, ≤ 4.5° at 95 % | Needs a dedicated master's out-of-band pilot; FPGA; transmit-heavy; not commodity. ACA: no master, no dedicated pilot. |
| **Argos** (WARP) [ARG12] | Internal calibration stable < 2.6 % phase over 4 h; 300 µs per pair; reciprocity-based | Wired clock between antennas; not wireless; not passive. |
| **Cao et al.** (COTS RRU, 5G) [CAO23] | OTA reciprocity calibration on COTS RRUs: 1.08–1.53° averaged absolute phase error; calibration staleness > 10 ms degrades 10 % of channels past 15° | Purpose-built RRUs with fronthaul; a *common clock* in their prototype removes CFO; no ambient sensing. ACA: the staleness law is exactly why per-frame ambient re-referencing wins. |
| **Ngo & Larsson** [NL24; NL25] | OTA phase sync formalised; phase *drift during offsets* is a first-order noise source; Kalman tracking with circular wrapping; re-estimate every ~2 slots when LOs are poor | Cooperative TDD systems; no ambient frames; no commodity hardware. ACA adopts their estimator, removes the cooperative exchange cadence. |
| **Vieira & Larsson** [V&L] | Two calibrated arrays still need a *bidirectional* OTA measurement to be jointly calibrated; `2(c_A−c_B)` is π-ambiguous; two-frequency resolution | Assumes arrays and a calibration protocol; ACA adds the ambient mode that provides the missing observation for free and resolves the π ambiguity with one common frame. |
| **ESPARGOS** [ESPARGOS; ESPDS; ESPSDR] | ESP32-family phase coherence via shared 40 MHz + conducted 2.4 GHz frame reference; 8-antenna arrays combined into a larger phase-synchronous system; phase-coherent raw I/Q; coherent TX possible | Wired distribution; no wireless cross-node coherence; single array per wire tree. ACA: keeps the conducted reference *inside* the puck (where it is cheap and robust) and replaces the *inter-node* wire with ambient frames. |
| **ESP-PPB** (grey, unrefereed) [PPB] | ESP32-C3 wireless phase lock (FTM + VCTCXO), **reference frame 1 ms after each external frame**, single-frame phase accuracy < 1° lab / < 10° typical; four-node AoA matrices | Dedicated master + disciplined oscillator + reference frames; no sensing story; not peer-reviewed. ACA: removes master, disciplined oscillator and reference frame; pays for it in M2 risk. |
| **WiDRa** (COTS, 5 GHz) [WiDRa] | Two-way sum-carrier-phase differential ranging < 1 mm RMSE on Intel AX210; CFO refinement; cycle-slip and multipath limits; defers direction finding | Active exchange; single link; LoS/Rician assumption; no multi-node aperture. ACA uses the *one-way common-frame* observable and leaves the two-way mode as a rare anchor. |
| **WiCAL** [WiCAL] | Switched-array phase alignment (PLL offsets, switch paths, CFO across groups), calibration-point antennas, inter-array sync to a *known* reference source; 0.57 m median 3D in an NLoS room | Requires a cooperative, positioned calibration source; cabled arrays. ACA: ambient sources, unknown positions, self-survey. |
| **LoSRef** [LSR26] | Commodity Wi-Fi phase-coherent delay–Doppler using a *single link's* LoS path as an OTA reference; 20 dB below static multipath | One transmitter–receiver pair; no cross-node phase; no aperture. ACA generalises "the earliest path is a reference" from one link to *many receivers of one frame*. |
| **GHR** [GHR] | Joint time–phase decoupling for distributed receive-only nodes from a **cooperative** non-stationary (LFM) source; CRB ∝ 1/Var(ω); simulation-validated; subarray coherent integration | Cooperative known waveform; temporal non-stationarity; simulation. ACA: unknown source, uses the *frequency* axis of a single ordinary OFDM frame. |
| **Fascista et al.** [FAS24] | Joint localisation, synchronisation, mapping with phase-coherent distributed arrays; ML estimator + FIM; LoS/wall/scatterer contributions | APs already coherent; no mechanism to make them so wirelessly. ACA supplies the mechanism and adds the pose-graph structure. |
| **Clutter/dominant-scatterer sync** [CJA22; RS23] | Using the *environment's* returns as a phase reference across distributed radar apertures | Radar's own cooperative transmissions; not ambient, not CSI, not commodity. ACA's "memory not source" framing comes from here. |
| **PCL reference channel** [PCL05] | Reference channel recovers the illuminator waveform per receiver; master/slave sync required across receivers | Dedicated reference antenna per receiver and explicit inter-receiver sync; ACA needs no reference antenna (one frame serves all) and no inter-receiver sync hardware. |
| **GWRF / NeRF²** [GWRF; NeRF2] | Cross-scene generalisable RF radiance fields; RF scene reconstruction from sparse samples | Learned fields do not estimate phase/geometry/carriers; ACA uses them only as scatterer priors. |
| **Channel charting (D-MIMO)** [CC24] | Label-free relative geometry from multi-AP CSI; bilateration loss with known AP positions | No carrier phase, no absolute coherence, no aperture. ACA's carrier-phase self-survey is complementary. |
| **Sibling `22-invention-a` (Weave)** | Active route: TDM rounds, per-frame reference anchoring, sum-CP ranging, disciplined oscillators | ACA is the passive route: ambient frames, zero sensing TX, no disciplined oscillator; shares boundary, polyhedron and E1-style measurement, differs in cadence and cost. |

**The invention in one sentence of delta:** *others made distributed coherence work by
transmitting, cabling or disciplining; ACA makes it work by listening to the traffic that
is already there, and pays only in the per-frame residual M2.*

---

## 9. The decisive experiments

### E1 — THE decisive experiment: the **Ambient Ledger test** (two S3 nodes, no RF cable between them)

**Question.** With no dedicated reference frame, no disciplined oscillator and no exchange,
what is the residual cross-receiver phase `M2` extracted from *third-party* frames?

**Rig.** Two ESP32-S3 boards 3–6 m apart, single antenna each, no RF connection between
them, both in promiscuous/CSI mode on the home AP's channel. An *algorithm-blind oracle*
(coax splitter to both boards' conducted test ports, as in `22-invention-a` E1 and
ESPARGOS) records the true instantaneous relative phase and is used only for scoring.
Frames scored: (a) home-AP beacons; (b) uplink frames from any client on the channel;
(c) a third ESP32 in AP mode; (d) an ESP-NOW HT40 MCS0 stream. Ground-truth channel: a coax
delay line with a switched known offset to validate the `Δτ` slope estimate.

**Measurements.** Per frame: `Δψ̂` (intercept of the cross-receiver ratio), `Δτ̂` (slope),
`σφ` per subcarrier; then the residual `Δψ̂ − oracle`. Report: per-frame σ, Allan deviation,
autocorrelation, and time-to-wrap; repeat over a 10 °C temperature ramp and with a person
walking between source and nodes (bias drift test).

**Pass/fail.**
- PASS: single-frame `σ(Δψ̂ − oracle) ≤ 0.2 rad` (≈ 11.5°), residual correlated over ≥ 2
  frames (trackable), no wrap at ≥ 50 Hz pair-common frame rate.
- PARTIAL: 0.2–0.5 rad ⇒ aperture usable for 2–4-puck federations and incoherent
  integration; house-scale sharpness requires the rare active anchor.
- FAIL: > 0.5 rad, per-frame independent, or unbounded drift ⇒ the passive ledger is dead
  on this silicon; the boundary stands and the home-scale aperture requires continuous
  active anchoring (`22-invention-a`) or conducted links.

**Falsifies U1 and U2** in one shot. Cheap: two S3 boards, a splitter, a scope/FPGA oracle,
a firmware week.

### E2 — Federated gain (the sharpness test)

Two pucks (or one puck + one single-antenna node) ~5 m apart, one ambient AP, a movable
known reflector/human. Compare coherent federated processing (ledger de-rotation +
known geometry) against: single-puck, incoherent multi-puck, and oracle-phase baselines.
**Success:** coherent gain ≈ `N` with loss ≤ 3 dB vs oracle, and cross-range resolution
approaching `R·λ/D`, i.e. ≲ 20 cm at 5 m against the single puck's ≈ 2–3 m.

### E3 — Self-survey (the geometry test)

Six single-antenna S3 nodes, tape-measured truth, poses/positions estimated from `Δτ` slopes
over ≥ 10⁴ ambient frames + FTM + one anchor exchange. **Success:** ≤ 2 cm RMS node position
error (≪ λ/5 ≈ 2.5 cm); **fail:** > 10 cm, which caps the useful baseline at small `D`.

### E4 — The polyhedron (the 3D test)

One 20-face puck: conducted-reference boot calibration; measure per-face phase residuals and
3D DoA of a known reflector at known elevation; then federate two pucks in different rooms
looking through a doorway (NLoS) to test bias prediction and virtual-anchor use.
**Success:** ≤ 15° 3D DoA per puck, ≤ 5° federated; **fail:** coupling/shadowing destroys
the pattern or the conducted reference does not hold across S3 reboots.

### E5 — Cross-home transfer (the generalising test)

Train the scatterer prior on 2–3 instrumented homes (or ray-traced scenes à la `19`), deploy
in a new home with *no labels*. **Success:** the joint solve converges from ambient frames
alone with no site survey; **fail:** per-home hand-tuning is required, in which case the
"self-calibrating" claim is downgraded to "self-calibrating within a commissioned site".

---

## 10. ESP32-S3 feasibility, element by element

| Element | Status on S3 | Evidence / gap |
| --- | --- | --- |
| CSI capture for third-party frames (beacons, others' traffic) | **ESTABLISHED in the field** (ESP32/S2/C3 repo; S3 used by CSI-Radar projects); family-wide IDF API | Sniffer + CSI coexists; verified on classic/S2/C3 [SNIFF]; S3 community usage strong; no primary measurement of S3 CSI *quality* for third-party frames — E1 fills it. |
| CSI of encrypted traffic usable | **ESTABLISHED by physics** (CSI from L-LTF preamble, pre-decryption) | Standard; no reference needed. |
| HT40 ESP-NOW MCS0 excitation (for anchors, self-survey) | **ESTABLISHED** (esp-csi default; CSI-Beacon at 100 Hz HT40 MCS0) [ESPCI; BEACON] | — |
| Per-frame CSI phase intercept observable | **ESTABLISHED on ESP32** (varying intercepts reported and attributed to residual CFO) [EC121] | — |
| Cross-node per-frame phase residual (M2) on S3, ambient, no reference frame | **UNMEASURED — the gate** | ESP-PPB gives < 1–10° on C3 *with* a dedicated frame [PPB]; E1 measures S3 without one. |
| Conducted 40 MHz + 2.4 GHz phase reference per puck | **ESTABLISHED pattern on ESP32-family (S2)**; **HYPOTHESIS on S3** | ESPARGOS hardware and datasets [ESPARGOS; ESPDS]; ESP-SDR phase-coherent I/Q [ESPSDR]; S3-specific replication is an engineering task. |
| Raw I/Q phase-coherent capture (ESP-SDR) on S3 | **ESTABLISHED on S3** (80 MSa/s with FPGA assist; phase-coherent across antennas on ESPARGOS One) [ESPSDR] | Duty/gaps remain (`05`); offline processing only. |
| Coherent TX within a puck | **HYPOTHESIS** (vendors state it is possible; implementation unreleased) [ESPSDR] | Optional for ACA; needed for the sibling Weave's illumination gains. |
| FTM ranging | **ESTABLISHED but coarse** (1–2 m; 2.4 GHz HT20 only) [`15`] | Used only to bootstrap geometry. |
| ns RX timestamps / no TX timestamps / 2.4 GHz only | **ESTABLISHED constraints** [`17`; ESP-PPB patch] | The ledger is a *phase* mechanism and does not need ns timestamps; it needs frame identity, which TSF/sequence numbers and beacon periodicity provide. |
| Monostatic radar on-chip | **BLOCKED** (SIC) [`08`] | Not used: ACA is passive bistatic. |

---

## 11. Cost, in one place (estimates; graded SPECULATION for prices)

| Item | Per unit | Note |
| --- | --- | --- |
| ESP32-S3 module | $3–5 | 20 per puck ⇒ $60–100 |
| Triangular face PCB + assembly | $1.5–3 each | 20 ⇒ $30–60 |
| 40 MHz clock fan-out + 2.4 GHz phase-reference distribution + RF switches | $20–40 | ESPARGOS topology, miniaturised |
| Hub: Ethernet PHY + switch (or small FPGA) | $10–50 | FPGA adds ns triggering and coherent-TX option |
| Mechanics, power, connectors | $10–20 | flex/soldered edges per premise |
| **Per puck** | **≈ $120–250** | qty 100 |
| **Home (6 pucks)** | **≈ $0.7–1.5k** | plus the AP it repurposes; no cabling |
| Marginal S3 node (no polyhedron) | $3–8 | the ledger needs no special silicon, no VCTCXO, no per-node calibration kit |

Compare: ESP-PPB nodes are quoted at ~$50–80 each (VCTCXO + DACs) [PPB]; ESPARGOS One is a
purpose-built array. ACA's passive mode specifically *removes* the disciplined-oscillator
cost, because it re-references on every common frame instead of holding frequency.

---

## 12. Chaining (A), existing models/data (B), own data (C) — per the BRIEF

### (A) Chaining

1. **Ambient common frames ⇒ per-frame cross-node phase (U1) ⇒ federated aperture (U2) ⇒
   3D localisation/imaging ⇒ scatterer map ⇒ bias prediction ⇒ better ledger.** This is a
   *closed loop*: sensing pays for calibration and calibration sharpens sensing. The
   highest-leverage enabling primitive is E1's per-frame residual; it gates U1, U2, §5 and
   §6 in one stroke.
2. **Conducted intra-puck reference (U3) ⇒ known pose graph ⇒ self-survey conditioning ⇒
   federated geometry** — without U3 the self-survey is a 120-dimensional problem; with it,
   a pose-graph problem. U3 unlocks U2's *sharpness* (as opposed to its existence).
3. **Two-way anchor ⇒ π-ambiguity resolution + absolute ranges ⇒ bootstrapped geometry ⇒
   absolute localisation** (FTM is too coarse alone).
4. **Scatterer map ⇒ virtual anchors ⇒ extra independent look directions ⇒ NLoS/through-door
   operation** (path `13`/`14` → `16`).
5. **Sensing only needs U1+U2; anything that transmits is optional.** That is the deliberate
   inversion of the active sibling: coherence is a *by-product of other people's traffic*.

### (B) Existing models and data

- **ESPARGOS datasets** (phase-coherent CSI; DaRUS/DOI) — the natural training/validation
  set for the *conducted* pieces and the studied substitute for U3 until an S3 puck exists
  [ESPDS].
- **GWRF / NeRF² / WRF-GS code and datasets** — adapt as the scatterer prior for §6
  [GWRF; NeRF2]; licences per the respective papers (verify before use).
- **Channel-charting code** (e.g. the distributed-MIMO real-world-coordinate code) — reuse
  bilateration losses for initialisation [CC24].
- **SenseFi / MM-Fi / CSI-Bench / OpenCSI** — the learned-sensing layers and domain-adaptation
  baselines, with the `19` caveats (not ESP32 CSI).
- **ESP-PPB (GPL-3.0, unrefereed)** — the closest firmware pattern for per-frame phase
  anchoring; its VCTCXO discipline and reference-frame trick are directly reusable for the
  *fallback* mode (note its licence and its non-certified hardware) [PPB].
- **WiDRa** concepts (sum-CP, CFO refinement) — reusable for the anchor mode [WiDRa].
- **PicoScenes** — COTS tooling for reproducing the COTS-side experiments quickly.

### (C) Own data / new technology

- **E1 dataset (the Ambient Ledger dataset):** cross-node `Δψ̂`, `Δτ̂` and oracle phase vs
  frame source, gap, temperature and scene change, on ESP32-S3. No equivalent exists for
  *any* commodity part (the nearest, ESP-PPB, is one-sided and unrefereed). This is the
  dataset that decides the passive route.
- **E3/E4 dataset:** home-scale pose-graph self-survey + federated coherent snapshots with
  truth poses — a first-of-kind public artefact for *passive coherent* distributed sensing.
- **Falsification is built in:** each experiment above names its fail state; a negative E1
  is publishable and closes the passive branch cleanly.

---

## 13. Residual unknowns, and what would change the verdict

1. **M2 on ESP32-S3 without a dedicated reference frame** — the whole issue (§1.3, E1).
2. **S3 CSI third-party frame quality** (subcarrier count, smoothing, quantisation, filters)
   — cheap to measure; WiDRa's `2πn/N` and Intel's smoothing show vendor surprises are real
   [WiDRa; SSDS].
3. **Per-boot constants on S3** — semi-static or re-randomised per channel switch? (COTS
   evidence says two-valued per boot [SJ21]; ESPARGOS says PLL phase re-randomises per
   boot/channel.) Determines anchor cost.
4. **Quiet-home degeneracy** — how often a home produces pair-common frames; determines
   whether the optional beacon-puck is mandatory. Firmware loggable in a day.
5. **Polyhedron coupling/shadowing** — the `00` §9 unknown, unchanged; E4.
6. **Learnability of the scatterer prior across real homes** — E5; a negative here downgrades
   "generalising" to "self-calibrating after commissioning", not the aperture itself.
7. **Legal/EMC**: passive receive-only sensing is unconstrained by the TX envelope of `07`;
   the optional beacon-puck falls under ordinary 802.11 TX rules (ACMA LIPD / FCC 15.247,
   per `07`). Not re-litigated here.

**What would close the space:** a measured M2 that is per-frame-independent and large
(≫ 0.5 rad) *and* an ambient frame rate too low to average, *and* no willingness to transmit.
Under those conditions no passive home aperture exists; the home is a constellation of
small apertures and the exact blocked quantity is the cross-frame phase continuity of
commodity receivers — nothing about propagation, and nothing about antennas.

---

## 14. Evidence strength (1–10) and sources

| Claim | Strength | Basis |
| --- | --- | --- |
| Gauge/boundary; three escapes | 10 | `20`, established theory [20; V&L] |
| Two common-observation escapes behave as stated; bidirectional requirement | 9 | [V&L] theorem + [AIR13; ARG12; CAO23] practice |
| Cross-receiver intercept/slope decomposition (the estimator) | 8 | direct algebra + phrase-calibration practice [SSDS; EC121]; not yet published as a *system* |
| Common-frame phase reference physically sound | 8 | Eq. (1)–(3); consistent with all cited phase work; ESP-PPB's stable flat cross-node offsets [PPB] |
| Ambient (non-cooperative) ledger works at ≤ 0.2 rad on S3 | **4 — the wager** | bracketed by [PPB] (< 1–10° with a reference frame) and [WiDRa] (mm two-way); unmeasured without a reference frame |
| Self-survey to ≤ 2 cm from `Δτ` slopes over 10⁴ frames | 6 | arithmetic (§2.2) + FIM [FAS24] + partial demonstrations [WiCAL; CC24] |
| House-scale sharpness (0.5–1.5°) once phase is known | 7 | standard aperture theory; the open part is purely M2 and geometry |
| Polyhedron 3D aperture value (elevation, pose ruler, diversity) | 7 | spherical/geodesic array literature [SAA26; GOL25; GDPAA02]; ESPARGOS pattern [ESPARGOS]; coupling unmeasured |
| Passive/ambient being genuinely new *as a system* | 7 | survey of the table in §8 found no prior combination; negative-results caveat: absence of evidence in one sweep |
| Cost estimates | 4 | vendor pricing, SPECULATION |

**Primary sources (numbered as cited).**

1. `20-theoretical-boundary.md` and `00-synthesis.md`, this folder (internal synthesis).
2. `22-invention-a.md` (the Coherent Weave) and `10-ota-reciprocity.md`, `11-common-ref-two-way.md`, `17-time-frequency-precision.md`, `07-tx-legal-primitives.md`, `08-radar-geometries.md`, `09-mimo-tdm-fullduplex.md`, `15-node-self-localisation.md`, `16-rf-slam.md`, `19-model-vs-learning.md` (internal).
3. **[V&L]** E. G. Larsson, J. Vieira, "Phase Calibration of Distributed Antenna Arrays", IEEE Commun. Lett. 27(6):1619–1623, 2023. arXiv:2304.05144 · https://arxiv.org/abs/2304.05144
4. **[AIR13]** H. V. Balan et al., "AirSync: Enabling Distributed Multiuser MIMO with Full Spatial Multiplexing", IEEE/ACM ToN 21(6):1681–1695, 2013. DOI 10.1109/TNET.2012.2230449 · arXiv:1205.6862
5. **[ARG12]** C. Shepard et al., "Argos: Practical Many-Antenna Base Stations", MobiCom 2012. DOI 10.1145/2348543.2348553
6. **[CAO23]** Y. Cao et al., "Experimental Performance Evaluation of Cell-Free Massive MIMO Systems Using COTS RRU With OTA Reciprocity Calibration and Phase Synchronization", IEEE JSAC 41(6):1620–1634, 2023. DOI 10.1109/JSAC.2023.3276057 · arXiv:2208.14048
7. **[NL24]** K.-H. Ngo, E. G. Larsson, "Breaking the TDD Flow for Over-the-Air Phase Synchronization in Distributed Antenna Systems", arXiv:2504.11411 · https://arxiv.org/abs/2504.11411
8. **[NL25]** K.-H. Ngo et al., "Distributed MIMO With Over-the-Air Phase Calibration Integrated Into the TDD Flow", arXiv:2509.03722 · https://arxiv.org/abs/2509.03722
9. **[RN23]** R. Rashid, J. Nanzer et al., "Frequency and Phase Synchronization in Distributed Antenna Arrays Based on Consensus Averaging and Kalman Filtering", IEEE TWC 22(4):2789–2803, 2023.
10. **[GHR]** "Joint Time–Phase Synchronization for Distributed Sensing Networks via Feature-Level Hyper-Plane Regression", arXiv:2603.28121 · https://arxiv.org/abs/2603.28121
11. **[WiDRa]** V. V. Ratnam, B. Sadiq et al., "WiDRa — Enabling Millimeter-Level Differential Ranging Accuracy in Wi-Fi Using Carrier Phase", arXiv:2405.12168 · https://arxiv.org/abs/2405.12168
12. **[LSR26]** "Wi-Fi Radar via Over-the-Air Referencing: Bridging Wi-Fi Sensing and Bistatic Radar", arXiv:2602.05344 · https://arxiv.org/abs/2602.05344
13. **[FAS24]** A. Fascista et al., "Joint Localization, Synchronization and Mapping via Phase-Coherent Distributed Arrays", IEEE JSTSP 19(2):412–429, 2025. DOI 10.1109/JSTSP.2025.3533111 · arXiv:2409.12478
14. **[CC24]** "Channel Charting in Real-World Coordinates with Distributed MIMO", arXiv:2406.13722 · https://arxiv.org/abs/2406.13722
15. **[WiCAL]** "WiCAL: Accurate Wi-Fi-Based 3D Localization Enabled by Collaborative Antenna Arrays", arXiv:2505.21408 · https://arxiv.org/abs/2505.21408
16. **[ESPARGOS]** "ESPARGOS: An Ultra Low-Cost, Realtime-Capable Multi-Antenna WiFi Channel Sounder", arXiv:2502.09405 · https://arxiv.org/abs/2502.09405
17. **[ESPDS]** "ESPARGOS: Phase-Coherent WiFi CSI Datasets for Wireless Sensing Research", arXiv:2408.16377 · https://arxiv.org/abs/2408.16377 · espargos.net
18. **[ESPSDR]** ESP-SDR: Raw IQ Capture with Espressif's ESP32 Chips · https://espargos.net/espsdr
19. **[SSDS]** "Same Signal, Different Story: Demystifying Receiver Effects in Wi-Fi Channel State Information", arXiv:2605.26836 · https://arxiv.org/abs/2605.26836
20. **[EC121]** espressif/esp-csi issue #121, "Phase offsets in ESP32 CSI (AEGHB-250)" · https://github.com/espressif/esp-csi/issues/121
21. **[SNIFF]** zikunliu/ESP32_Sniffer_beacon_csi — beacon CSI in sniffer mode · https://github.com/zikunliu/ESP32_Sniffer_beacon_csi
22. **[BEACON]** Evil-M5Project CSI Radar + CSI-Beacon (ESP32-S3, HT40 MCS0 at 100 Hz) · https://github.com/7h30th3r0n3/Evil-M5Project/wiki/CSI-Radar
23. **[ESPCI]** StevenMHernandez/ESP32-CSI-Tool (active/passive CSI collection) · https://github.com/StevenMHernandez/ESP32-CSI-Tool
24. **[PPB]** jonathanmuller/esp-ppb — "1-PPB wireless sync for phase coherent Wi-Fi CSI", ESP32-C3, GPL-3.0, 2026 (unrefereed) · https://github.com/jonathanmuller/esp-ppb
25. **[SJ21]** "Calibrating Phase Offsets for Commodity WiFi" (semi-time-invariant, two-valued per-boot offsets) · https://github.com/Intelligent-Perception-Lab/Calibrating-Phase-Offsets-for-Commodity-WiFi
26. **[SAA26]** "Spherical Antenna Arrays: Principles, Applications, and Research Directions", arXiv:2604.02701 · https://arxiv.org/abs/2604.02701
27. **[GOL25]** "Enabling Scalable and Manufacturable Large-Scale Antenna Arrays Through Hexagonal Subarray Implementation over Goldberg Polyhedra", MDPI Electronics 14(23):4618, 2025. DOI 10.3390/electronics14234618
28. **[GDPAA02]** "A Geodesic Sphere Phased Array Antenna for Satellite Control and Communication", URSI GA 2002 · https://www.ursi.org/proceedings/procGA02/papers/p1274.pdf
29. **[NeRF2]** X. Zhao et al., "NeRF2: Neural Radio-Frequency Radiance Fields", MobiCom 2023. DOI 10.1145/3570361.3592527 · arXiv:2305.06118
30. **[GWRF]** "GWRF: A Generalizable Wireless Radiance Field for Wireless Signal Propagation Modeling", arXiv:2502.05708 · https://arxiv.org/abs/2502.05708
31. **[CJA22]** "Time and phase synchronization using clutter observations in airborne distributed coherent aperture radars", Chinese J. Aeronautics, 2022 · https://www.sciencedirect.com/science/article/pii/S1000936121003629
32. **[RS23]** "Coherent-on-Receive Synthesis Using Dominant Scatterer in Millimeter-Wave Distributed Coherent Aperture Radar", MDPI Remote Sensing 15(6):1505, 2023. DOI 10.3390/rs15061505
33. **[VA16]** C. Gentner et al., "Multipath Assisted Positioning with Simultaneous Localization and Mapping", IEEE TWC 15(9), 2016.
34. **[SALMA18]** B. Grosswindhager et al., "SALMA: UWB-based Single-Anchor Localization System using Multipath Assistance", MobiCom 2018.
35. **[PCL05]** H. D. Griffiths, C. J. Baker, "Passive coherent location radar systems. Part 1/2", IEE Proc. RSN 152, 2005. DOI 10.1049/ip-rsn:20040039 / 20040040
36. **[PADS]** PADS-style edge-tone phase sanitisation, as used and cited in [SSDS].
37. **[Wi-Wi]** N. Shiga et al., "Demonstration of wireless two-way interferometry (Wi-Wi)", IEICE Commun. Express 6(2):77–82, 2017. DOI 10.1587/comex.2016XBL0181 · Y. Yamasaki et al., IEEE Access 9:85084–85100, 2021. DOI 10.1109/ACCESS.2021.3087866 · NICT Wi-Wi page: https://www.nict.go.jp/en/sts/wi-wi.html
38. **[Exel13]** R. Exel, "Carrier-based ranging in IEEE 802.11 wireless local area networks", IEEE WCNC 2013 (sub-centimetre reported).
39. **TWSTFT**: NIST, "Two-Way Satellite Time and Frequency Transfer" · https://www.nist.gov/pml/time-and-frequency-division/time-distribution/two-way-satellite-time-and-frequency-transfer · M. Fujieda et al., "Carrier-phase two-way satellite frequency transfer over a very long baseline", Metrologia 51:253–262, 2014.
40. **NIST carrier-phase RF ranging** (round-trip carrier phase; 27 µm CRLB at SNR 10 dB): GPS World, "Innovation: Carrier-Phase RF Ranging" · https://www.gpsworld.com/innovation-carrier-phase-rf-ranging
41. **[AMBIENT13]** V. Liu, A. Parks, V. Talla, S. Gollakota, D. Wetherall, J. R. Smith, "Ambient Backscatter: Wireless Communication Out of Thin Air", SIGCOMM 2013. DOI 10.1145/2486001.2486015

---

## Appendix A — the estimator, worked

Write the per-node sanitised phase as `φ_i(k) = α_i + β k + ε_i(k)` with `β = 2π Δf_sc τ_i`
(centred index `k`). The cross-receiver ratio Eq. (3) has phase `Δα + Δβ k`. Weighted
least-squares over `K` subcarriers gives `Var(Δα̂) = σφ²/K` and
`Var(Δβ̂) = σφ² / Σk² = 12 σφ²/(K(K²−1))`, so
`σ(Δτ̂) = σ(Δβ̂)/(2π Δf_sc) = σφ √12 / (2π Δf_sc √(K(K²−1)))`. Numbers are in §2.2. The
`π`-period of `Δα` and the wrap rate are in §2.3. Cycle-slip detection: a slip appears as a
jump of ≈ π in `Δψ̂` beyond the Kalman gate; recovery is the same as the anchor's
π-resolution (one two-way difference, or two adjacent subcarrier bands compared — the
intercept is frequency-flat, so bands must agree).

**Signal-to-noise aside.** The intercept is estimated from a *ratio* of two CSIs, so its
noise depends on both link SNRs: `σφ ≈ 1/√(2·SNR_i,eff)` per subcarrier in radians for
high SNR. At 20 dB per-subcarrier SNR, `σφ ≈ 0.07 rad`; the budget above used 0.1 rad. At
0–5 dB (weak beacons), `σφ > 0.3 rad` and averaging across frames becomes mandatory —
another reason the ledger is a filter, not a one-shot estimator.

## Appendix B — mechanisms considered and rejected

### B.1 Mutual injection locking of the LOs (SPECULATION; rejected)
Concept: let every node's oscillator be pulled into a collective mode by strong mutual
coupling; a constant (if unknown) relative phase pattern is then calibrated once and holds.
Rejection reasons: the ESP32 synthesiser is a PLL off a 40 MHz crystal; injection pulling
into a PLL is heavily suppressed relative to a free-running oscillator, OTA coupling at
2.4 GHz does not reach the 40 MHz reference, and lock-range scales with injected amplitude —
a house is not a locking-range environment. Falsifier if pursued: measure LO pulling vs
injected power on an S3; expectation: negligible.

### B.2 Fully passive *illuminator-free* coherence (rejected)
Without any common source, common frame or conducted link, cross-node phase is in the gauge
and nothing recovers it — this is the boundary theorem [20; V&L]. No amount of multipath
processing changes it, because the unknown phase is a single scalar that multiplies every
cross-node product.

### B.3 Monostatic operation (rejected, per `08`)
On-chip self-interference cancellation (~110 dB) is out of reach; the passive bistatic
configuration is strictly better here and is legal without new emissions.

### B.4 Environment-only reference (rejected as *source*, kept as *memory*)
See §1.2. A static scatterer adds geometry (virtual anchors), not phase.

### B.5 Ambient backscatter modulation (rejected for sensing)
Modulating reflections off ambient signals is a communication technique [AMBIENT13] and adds
controlled emissions; ACA wants the aperture, not a tag.

## Appendix C — vocabulary introduced here

- **Coherence inheritance** — obtaining per-frame coherence from a common illuminator rather
  than maintaining it locally between frames.
- **Passive Phase Ledger (PPL)** — the tracked state of every node pair's relative phase,
  updated by each pair-common ambient frame, anchored rarely.
- **Cross-receiver CSI ratio** — Eq. (3); the per-frame observable carrying both `Δψ` and
  `Δτ`.
- **Phase memory** — the property of the static channel as a holdover standard; contrasted
  with a phase source.
- **Aperture Node** — the 20-face ESP32-S3 polyhedron puck.
- **Pose-graph aperture** — the federated array whose unknown geometry is one 6-DoF pose per
  known-shaped puck.
- **M2** — the residual per-frame cross-receiver phase after CFO removal without a dedicated
  reference frame; the single gate quantity.




