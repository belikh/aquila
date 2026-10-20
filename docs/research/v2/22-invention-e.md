# 22-e — The Lighthouse Aperture: a coherent polyhedron hub that makes a home of cheap ESP32-S3 nodes one distributed 3D sensor

Status: research proposal for the Aquila v2 invention sprint. Australian English.
Grades: **ESTABLISHED** (published primary source), **HYPOTHESIS** (physical
argument + partial evidence, untested on ESP32-S3), **SPECULATION** (design
conjecture). Cross-references use the synthesis (`00-synthesis.md`) and note
numbers (`01`–`20`). This note does not upgrade any claim in the synthesis.

## 0. Answer in one paragraph

The space is **not closed**, and the invention is a *coherence architecture*, not
a sharper lens. A 30 cm coherent aperture at 2.4 GHz is optically coarse
(~24° beamwidth); no amount of cleverness breaks diffraction. What *can* be
built is a **hierarchical phase-coherence protocol anchored by one
known-geometry, shared-clock polyhedron hub** — the *Lighthouse Aperture* — under
which (a) the hub is coherent across its own faces by construction, (b) it
illuminates the home with **known-phase, scheduled face/channel/polarisation
transmissions** so that every single-antenna ESP32 in the home obtains a
geometric hologram and can **self-localise to decimetre level with no survey**,
(c) a scheduled **ping-and-unique-reply** round trip transfers coherence and
millimetre-precision *differential* range across the mesh using an LO-free
two-way phase sum plus a per-pair hardware-asymmetry ledger, and (d) each node
**locks its own oscillator phase to the static (LoS + static multipath) channel
of the hub**, so that every node's measurements can be expressed in the hub's
frame at packet rate. With (b)–(d) in place the *house* — not the 30 cm sphere —
becomes the aperture: scatterers seen by many phase-aligned nodes are localised
against metre-scale baselines, giving decimetre-scale 3D point clouds of sparse
scatterers from ~A$10 nodes. The physical quantity that would close the space if
absent is the **residual cross-node LO phase after per-frame CFO removal over
the protocol interval (the synthesis' M2)**; the minimal reference that crosses
the boundary is **one common-transmitter exchange per coherence interval** (a
star, CRB-optimal per `20`), and the polyhedron is what makes that reference
manufacturable, permanent and local. Everything in §2.1 and significant parts of
§2.2–2.6 rest on mechanisms already demonstrated in the literature — on ESP32
hardware for (a), on commodity WiFi or COTS SDRs for the rest. The invention is
the *assembly* and the *protocol*; the falsification risk is concentrated in
three measurements, not in the physics.

## 1. The boundary, restated precisely (what physics closes)

From `20` and the synthesis §1: independent-LO CSI has the form
`ĥ_ij = g_ij·a_i·b_j·e^{j(θ_i−ψ_j)}·e^{−j2πf_kτ}`. On a connected measurement
graph the node phases are fixed only up to one global additive constant (a
`U(1)` gauge, plus a time/frequency gauge). Observable without any reference:
amplitude; per-link subcarrier differential phase (delay); per-receiver
inter-antenna differential phase (AoA *within* one receiver); two-way products;
temporal differentials. Not observable: absolute cross-node phase, absolute
phase-of-flight range, absolute synchronisation.

Three things are worth making explicit before the invention, because the
invention plays exactly in the gaps the textbook statement leaves open:

1. **The gauge is one scalar per connected component, not per link.** A single
   *known* geometric or coherent constraint anywhere in the component fixes it
   everywhere (`20`). A hub whose internal geometry is *known by manufacture* is
   therefore a physically realisable, zero-survey gauge anchor.
2. **Observability is temporally as well as algebraically limited.** Two links
   through a common transmitter *can* be phase-compared algebraically, but only
   if the LO phases have not drifted between the two measurements. The rate
   limit is the post-CFO residual, not the raw ±10–40 ppm wrap: raw wrap is
   `t_{π/2} = 1/(4Δf) ≈ 5–10 µs` (`17`), but after per-frame CFO removal the
   governing quantity is the residual (10–100 Hz scale, `12`) over the
   exchange interval. **This quantity on ESP32-S3 (M2) is unmeasured** — it is
   the one number on which the wireless-only variants of the invention hang.
3. **Coherence is not the same as resolution.** A 30 cm aperture at
   λ = 12.5 cm gives ~24° beamwidth no matter how perfect the phase. Decimetre
   localisation of *sparse* scatterers is still available from phase precision
   (Cramér–Rao: `σ_θ ≈ λ σ_φ /(2π D √K)`); *imaging* of extended scenes is not.
   Sharpness at home scale must come from making *separated* nodes coherent so
   the baseline is the house. That is the invention's real content.

## 2. The invention — Lighthouse Aperture

### 2.0 Architecture in one view

```
                 ┌───────────────────────────────────────────────┐
                 │   HUB  ("lighthouse")                         │
                 │   20-face ESP32-S3 polyhedron, ~30 cm          │
                 │   • shared 40 MHz clock fan-out                │
                 │   • conducted 2.4 GHz phase reference          │
                 │     (resistive split + microstrip, per face)   │
                 │   • data aggregation (Ethernet switch / FPGA)  │
                 └───────────────────────────────────────────────┘
                          │  scheduled coherent illumination
                          │  (face × channel × pol)
        ┌─────────────────┼───────────────────┬────────────────┐
        ▼                 ▼                   ▼                ▼
   ┌─────────┐      ┌─────────┐         ┌─────────┐      ┌─────────┐
   │ node 1  │      │ node 2  │  ...... │ node K  │      │ hub RX  │
   │ 1× S3   │      │ 1× S3   │         │ 1× S3   │      │ faces   │
   └─────────┘      └─────────┘         └─────────┘      └─────────┘
   each node:                                hub also listens on all faces
   • self-localises from hologram            coherently (shared LO) →
   • scene-locks its LO to hub static path      per-packet 3D AoA (sectors)
   • answers ping with unique reply (round trip + asymmetry ledger)
```

Design principle that makes this tractable: **coherence is one-sided.** The hub
is always coherent within itself (conducted reference). No mesh node ever needs
to be coherent with another *simultaneously*; each node needs only to know its
own phase offset relative to the hub at packet time, which the hub's
illumination + the node's own static-reference tracking provide. This converts
an N-node synchronisation problem into N independent single-node tracking
problems plus one engineering problem (the hub).

### 2.1 A1 — The hub: a coherent 3D ESP32 polyhedron ("the lighthouse")

**What is genuinely new.** ESPARGOS proves the *principle* on ESP32 family:
shared clock + conducted 2.4 GHz phase reference, per-receiver offset
calibration, and MUSIC AoA work (`arXiv:2502.09405`; `espargos.net`). ESPARGOS
is a planar 4×2 array; combined boards via SMA sync links are documented
(espargos.net/docs/combined-arrays). Geodesic dome phased arrays — a polyhedron
of triangular planar subarrays with sector activation — are established in
satellite communications (URSI GA 2002, `ursi.org/proceedings/procGA02/papers/p1274.pdf`).
Spherical 3D arrays are a live 6G research area (arXiv:2604.02701), with mutual
coupling explicitly an open measurement problem. **No coherent ESP32 3D
volumetric sensing array exists** (synthesis §7). The invention here is the
*assembly*: an icosahedral hub with per-face S3s, sector-activated, dual-pol,
40 MHz clocked and phase-referenced — plus the fact that a *known-by-
manufacture* geometry anchors the mesh gauge (§2.5).

**Physical mechanism.** Each S3's WiFi PLL re-locks per boot and per channel,
producing an unknown (but, once locked, stable) LO phase offset. The
ESPARGOS calibration equation `ĥ_CAL,m = ĥ_OTA,m / ĥ_REF,m · e^{−jφ_path,m}`
removes both PLL offsets and network path offsets from a conducted reference
frame sequence; the paper explicitly finds "reference clock distribution is
insufficient for achieving phase coherence" but that frequency sync alone gives
*long-term* phase stability, while a *periodically performed phase calibration*
is required after resets/channel changes. The premise's hub design (shared 40
MHz fan-out + conducted 2.4 GHz phase reference + aggregation) is exactly this,
scaled to 20 faces in 3D. Sector activation solves the non-convex-solid problem
(the far-side faces are shadowed by the body; only the cone of faces facing a
direction is activated — the GDPAA principle).

**What must be true.** (i) Cross-face phase offsets stay constant for tens of
minutes after one calibration in a static environment (the ESPARGOS stability
experiment supports the frequency-sync part; the reference is *re-available
on demand*, so periodic recalibration is a scheduling choice, not a physical
risk). (ii) Per-channel offsets are calibratable with the same conducted
reference at 2.4 GHz channels 1/6/11 (expected: yes — the reference network is
broadband; the ESPARGOS reference generator uses "WiFi frames" that can be
placed on each channel). (iii) Mutual coupling / face shadowing on the solid
does not destroy the array manifold beyond repair (unmeasured anywhere;
spherical-array literature flags it as open). Measured manifold calibration
(recorded per hub) is the mitigation.

**Falsifying experiment.** Calibrate a 5-face cluster once via the conducted
reference; measure cross-face phase offsets every 30 s for ≥30 min plus a
≥10 °C temperature step. Falsified if offsets wander >10° without
recalibration (the hub then needs frequent recalibration packets — viable but
a design change, not a collapse).

**ESP32-S3 feasibility.** High. The receive-side coherence chain is
demonstrated on ESP32 family (`arXiv:2502.09405`); Espressif's own reference
board bridges an ESP32-C3 and an ESP32-S3 with a **clock buffer to eliminate
relative frequency offset** (ESP-Techpedia, esp-csi solution), so S3-with-fixed-
clock operation is a vendor-endorsed pattern. HT40 CSI (40 MHz, ~108 usable
subcarriers) is supported on S3 (esp-csi issue #114; ESP-IDF Wi-Fi docs).
**TX needed?** Hub transmit is needed for A2/A4 (and is unneeded for A6).
**Cost (engineering estimate, SPECULATION).** 20× S3 modules A$100–160; clock
fan-out + resistive splitter + 20 RF switches A$60–120; 20 triangular PCBs +
hub carrier A$80–200 small-run; aggregation (Ethernet switch or small FPGA)
A$30–150; power/mechanics A$50. Prototype ≈ **A$350–700**; per-node pucks ≈
A$10–15 (S3 devkit) or A$20 dual-pol.

### 2.2 A2 — Coherent illumination holography: single-antenna receivers self-survey

**What is genuinely new.** Locata proves the *physics* industrially: a network
of wirelessly synchronised beacons (~30 ps via TimeLoc) lets a **single-antenna
rover do carrier-phase point positioning at centimetre level**, without base
stations, against surveyed antennas (PMC6514591, Sensors 2019). RIS-localisation
theory proves the related principle that a *controllable-phase* illumination
makes a single-antenna receiver's channel geometry-identifiable (survey:
arXiv:2312.07288). Wi-Fi holography proves coherent scene reconstruction is
possible with consumer transmitters, but with a *scanned* receive aperture and
a fixed reference antenna (Holl & Reinhard, PRL 118, 183901, 2017;
arXiv:1612.03194). **What has not been shown is the inverse arrangement on
commodity WiFi: a fixed, known-phase, multi-face transmitter whose illumination
schedule *is* the aperture, with cheap ESP32 receivers extracting geometry from
the CSI it generates.** That is A2.

**Physical mechanism.** In one scheduled sweep, the hub transmits from face `f`,
channel `c`, polarisation `p`, with a *known* phase `θ_{f,c,p}` (A1 calibration).
A node measures `φ_{f,c,p} = θ_{f,c,p} − ψ_n(t) + k_c·d(n, p_f) + chain_n,c +
multipath`. The node's own LO offset `ψ_n(t)` is a common-mode across faces
within a sweep; the hub's per-(face,channel,pol) illuminations are known and
scheduled, so the *spatial* phase ramp across faces gives the hub→node bearing
(λ/(2π·0.3 m·√K) precision — millidegree-class in clean conditions), the
*inter-channel* phase slope gives the delay (mm precision, 15 m unambiguity at
20 MHz channel spacing, metres resolved by FTM), and the *polarimetric* response
gives node orientation. Joint estimation over the full schedule resolves the
per-node, per-channel common-mode offsets — the schedule is designed for
conditioning (a small, concrete contribution: illumination-schedule design for
single-antenna self-survey, the active-transmitter analogue of RIS phase-profile
design). Identifiability follows the Rockah–Schultheiss structure — array shape
and source locations jointly identifiable from enough distinct illuminations
(IEEE TASSP 35(3), 1987) — but here the *array is the transmitter* and the
*source* is the receiver.

**What must be true.** (i) ESP32 CSI phase across packets is stable enough for
the schedule-joins to hold at the achieved packet rate (M2-adjacent; the
per-packet residual must be a *common-mode* nuisance removable by schedule
redundancy — it is, algebraically, provided faces are non-coplanar and offsets
are common-mode). (ii) Multipath does not bias the hull beyond ~20–30 cm in a
furnished room (the honest, load-bearing risk; mitigations: multi-hub fusion,
robust estimation, learned priors from `19`, and round-trip ranging in A3 as an
independent constraint). (iii) The hub's own geometry (PCB-manufactured) is
known to better than λ/20 (0.6 cm) — trivially true for a soldered PCB stack.

**Falsifying experiment.** The decisive experiment (§8): node localisation
against a laser/tape ground truth in a real multipath room, with and without a
moving occupant; falsified if median error is ≫20–30 cm or unstable across
minutes after self-calibration, or if schedule-redundancy fails to absorb
common-mode drift.

**ESP32-S3 feasibility.** Medium-high. Requires two things S3 does well
(receive CSI; switch channels) and one it does less well than 802.11ax NICs
(40 MHz vs 160 MHz, so delay-domain separation is weak; A2 leans on *angular*
structure, where the hub's multi-face geometry substitutes for bandwidth —
see A4). **TX needed?** Yes (hub). **Cost.** Marginal over A1; nodes unchanged.

### 2.3 A3 — Reciprocal round-trip coherence: ping + unique replies + asymmetry ledger

**What is genuinely new.** The two-way phase primitive is established on
commodity silicon: **WiDRa** shows that the *sum* of the carrier phases of the
two directions cancels the un-synchronised LO and tracks **differential range
at <1 mm RMSE** with 20 MHz bandwidth on COTS WiFi NICs (arXiv:2405.12168,
JSAC 2024), and explicitly notes the sum-CP sequence over a moving device
"generates a virtual antenna array". RTCSI demonstrates round-trip CSI Doppler
sensing on a Raspberry Pi CM4 (Shi et al., IEEE WCNC 2023; UCL Discovery
10161602), and RTCSI+factor-graph gives decimetre indoor localisation
(JSAC 2023, DOI 10.1109/JSAC.2023.3322812). Nanzer's group demonstrates
**fully digital wireless time/frequency/phase coherence on unmodified COTS
SDRs with no cables and no GNSS** — TWTT-based, 60–70 ps, >99 % coherent gain,
multipath- and Doppler-tolerent while the channel is reciprocal over the sync
epoch (arXiv:2506.07267). **What is new here is the protocol and its products
for a home mesh:** a scheduled *star* — one ping, every node answers with a
unique packet (unique MAC/sequence so every listener attributes replies) — from
which (i) every *pair* obtains an LO-free round-trip phase sum (range), (ii)
everyone aligned to the ping obtains a common-transmitter reference for the
same interval, and (iii) the products `ĥ_AB·ĥ_BA` yield each pair's **sum of
TX/RX chain phases — a differential hardware asymmetry** — a *per-node
property difference* stable over hours (synthesis `10`: chain coefficients
drift <2.6 % over 4 h). The ledger of asymmetries across the mesh is a
calibration state: once estimated it explains the static part of the round-trip
phase, leaving geometry; it is also a **cooperative device fingerprint** (a
pair can recognise its own asymmetry). To our knowledge the asymmetry ledger as
a network self-calibration state on commodity WiFi is new; the phase physics is
not.

**Physical mechanism.** Forward `m_AB = θ_A(t₀) − ψ_B(t₀+τ) + ch_AB(+asym_A)`;
reply `m_BA = θ_B(t₁) − ψ_A(t₁+τ') + ch_BA(+asym_B)`. With a common LO per node
(TX and RX share the synthesiser), the sum telescopes to `ch_AB + ch_BA` plus
*known* frequency-offset terms `2πδ(f_A−f_B)` and a small phase-noise residual
over the exchange: the LO phase — the gauge obstruction — cancels. Star
topology is the CRB-optimal single reference per coherence interval (`20`).

**What must be true.** (i) **M2**: the post-CFO residual over the achievable
RX→reply→RX interval on S3 keeps the sum's phase usable. The synthesis
estimates turnaround at the API level in milliseconds; raw `esp_wifi_80211_tx`
and ESP-NOW paths may be far faster — this is exactly the decisive M2
experiment `10`/`17`. If the interval cannot be made short, fall back: the
*hub's* conducted coherence replaces the wireless round trip for hub-facing
operations (A2/A4 still work), and the mesh keeps only the common-ping
alignment (which needs no round trip at all). (ii) The reply frame must excite
CSI (OFDM); raw TX defaults to 1 Mbps 11b with no CSI (`07`), but
`esp_wifi_config_80211_tx_rate()` exists to set raw-TX rates (ESP-IDF Wi-Fi
driver docs v4.4.3) and ESP-NOW is the proven CSI-exciting primitive (esp-csi).
(iii) Multipath: absolute range from the round trip is composite-channel-
biased (dm–m); *differential* range is mm (WiDRa) because the static composite
cancels over time. Use differential for tracking; absolute for coarse geometry
with A2/A4 refinement.

**Falsifying experiment.** Two S3s at a fixed distance, coax-and-attenuator
ground truth, ping/reply at the fastest achievable cadence; measure the
round-trip sum-phase residual against a conducted reference. Falsified if the
residual is not a slowly varying nuisance (random per packet), which would
make the ledger and coherent sums unusable on S3 without further tricks.
**ESP32-S3 feasibility.** Medium (this is the gate). **TX needed?** Yes (both
directions). **Cost.** Zero marginal (protocol only).

### 2.4 A4 — Scene-locked coherence: the environment as the shared phase reference

**What is genuinely new.** The ingredients are established separately:
`LoSRef` aligns an entire coherent-radar time series to the Tx–Rx LoS path on
unmodified COTS WiFi (Intel AX210 + PicoScenes), achieving phase-coherent
delay–Doppler analysis with a *single-antenna, single-port* receiver
(arXiv:2602.05344); distributed coherent aperture radars synchronise with
**clutter returns** (airborne DCAR, DOI 10.1016/j.cja.2021.08.040) and with a
**dominant scatterer** (Remote Sensing 15(6):1505, 2023 — phase tolerance
27.9° for 3 dB synthesis loss); LOFAR-scale blind calibration shows static
scenes support gain/phase self-calibration (MNRAS 474, 1028). LoSRef's own
limitation is stated plainly: it works because 160 MHz resolves the LoS in
delay; "many existing … studies based on 20 MHz or 40 MHz … result in a less
distinct delay-domain structure". **The new move is to substitute angular
separation for delay separation:** on ESP32-S3 (40 MHz max), the hub's
20 coherent faces can *angularly* isolate the dominant hub→node path (sector
beams are ~24° wide but multipath typically arrives 30–90° off-axis in a home),
so the node can track the phase of the dominant static component and lock its
LO to it — trading the bandwidth commodity AX cards have for the aperture the
polyhedron has. Combined with A2's self-survey, the locked reference phase is
*interpretable* (its path length is known), so all nodes' measurements become
expressible in the hub frame at packet rate, with no round trips. That is
"the environment as a shared reference" turned into "the environment completes
the distributed aperture".

**Physical mechanism.** Node `n` observes the hub's illumination; its CSI
contains a dominant static component plus moving-target residuals. The node
tracks `arg(dominant)` per packet (a per-node phase-locked loop over a known
geometric constraint), removing `ψ_n(t)`; `m_n(t) − baseline_n` then isolates
moving scatterers whose phase is, by construction, already in the hub frame.
Across nodes, echo phases are mutually coherent; the distributed aperture is
the set of node positions (baselines of metres), and the residual multipath
bias is the *same composite* on both baseline and signal (it cancels in the
difference, up to its own drift, which the same tracker absorbs).

**What must be true.** (i) A dominant, temporally stable static component is
identifiable at each node despite human motion — true for the direct path in
most home geometries, and robust statistics handle intermittent blockage.
(ii) The angle-based isolation actually improves reference purity at 40 MHz
(testable with 4 faces). (iii) Drift of the static composite (furniture moved,
doors) is detectable and triggers re-baselining (a feature, not a bug).

**Falsifying experiment.** With two S3 nodes and one hub face (no mesh), show
that static-locked phases at the two nodes, converted through known geometry to
a common frame, stay within <10° RMS relative to the hub's own conducted
reference for ≥5 minutes while a person walks between the nodes. Falsified if
locking fails or the standardised phases decorrelate.
**ESP32-S3 feasibility.** Medium-high; per-node PLL over CSI phase is cheap
DSP. **TX needed?** Only hub (ambient illuminator could substitute, degraded).
**Cost.** Zero marginal.

### 2.5 A5 — Gauge-SLAM: joint self-calibration of mesh geometry and static scene, transferring across homes

**What is genuinely new.** Blind array self-calibration with unknown sources is
classical (Rockah & Schultheiss, TASSP 1987; unsynchronised-sensor extensions,
arXiv:2102.03565). Channel charting gives label-free relative geometry from CSI
(Studer et al.; ESPARGOS datasets `arXiv:2408.16377`), and **passive channel
charting** already locates passive targets from distributed ESPARGOS arrays
without geometry labels (arXiv:2504.09924), outperforming triangulation. RF-SLAM
is established at coarse accuracy (synthesis `16`, waveSLAM). None of these
closes the loop *coherently* with a manufactured anchor. The new assembly is a
**factor graph whose gauge is fixed once, physically, by the hub's known
geometry**, with three factor families: (a) lighthouse hologram factors (A2);
(b) round-trip range factors with per-pair asymmetry parameters (A3); (c)
static-scene factors — a learned transfer operator for the room baseline,
initialised from ray-traced synthetic data, refined per home (synthesis §5:
ray-traced Sim2Real reaches 82.1 % F1 with 20 % real labels, `19`). Solution by
bundle adjustment; the output is (i) each node's pose, (ii) each hub's pose in
the mesh frame, (iii) the static scatterer map, (iv) the asymmetry ledger, (v)
per-boot/per-channel face offsets — a *single* state that explains all coherent
observations. Consistency with `20`'s minimal-reference result: one
known-geometry coherent hub per connected component is exactly one gauge fix.

**Why this generalises where fingerprinting does not.** The learned inputs are
physical quantities in a common frame — bearings, delays, differential ranges,
Dopplers, polarimetric ratios — not room-specific CSI vectors. The room-specific
part of the model is *estimated per home* in the same coordinates, so
cross-home transfer is a prior on wall/scatterer statistics (ray-traced) plus
physical invariants, not weight transfer. Falsifying test: zero-shot transfer
of the *physics-based* pipeline to a new home after its self-survey vs an
amplitude-fingerprint baseline trained on the old home, both with the same
number of labelled examples in the new home.

**What must be true.** Identifiability from the actual observable set (three
distinct illumination geometries suffice in the classical result; the hub
supplies tens), and schedule conditioning good enough that geometry solves from
cold start within minutes. **ESP32-S3 feasibility.** Computation is host-side
(hub aggregator or a home server); S3 only streams CSI. **TX needed?** As A2/A3.
**Cost.** Software; no marginal hardware.

### 2.6 A6 — The passive 3D ambient camera (no transmit at all)

**What is genuinely new.** Receive-only coherent arrays exist (ESPARGOS One
datasheet: 8 dual-pol channels, 4×2, receive-only by default, "can also
transmit"; `espargos.net`). Per-packet AoA from a single switched-antenna
receiver is demonstrated on commodity RF (ACM DOI 10.1145/3596263: single
2 MHz receiver, sub-packet mux switching, 3.4° mean AoA, 36.4 cm 2D
localisation). A coherent **3D** ESP32 polyhedron doing per-packet 3D AoA on
*ambient* 2.4 GHz traffic — sector-activated — is new as a system.

**Physical mechanism.** Shared LO ⇒ faces are coherent once calibrated; every
received packet carries a 3D wavefront, so the 20 faces form a sparse 3D array
(sector activation for the solid's shadowing). MUSIC/sparse recovery over the
sector gives per-packet angles and, with the face-domain phase, relative
strengths; temporal baselines give Doppler; multi-packet integration gives
super-resolution for a few paths. No coordination, no TX, no compliance
question. Resolution is ~24° per snapshot (aperture-bound); accuracy for sparse
paths is far better (CRB).

**What must be true.** Manifold calibration against mutual coupling on the
built hub (the risk named in synthesis §7); that ambiguity structure of the
sectorised sparse array is manageable (measured manifold + sparse recovery).
**Falsifying experiment.** Record ambient traffic with the 3D hub and a 2D
ESPARGOS-style reference; compare AoA/delay spectra and localisation of a
known transmitter. **ESP32-S3 feasibility.** High (this is A1 receive mode).
**TX needed?** No. **Cost.** Zero marginal (hub only).

## 3. Why this is "sharp": the arithmetic

- **Hub as receiver (A6):** sparse-path CRB `σ_θ ≈ λ σ_φ /(2π D √K)` with
  λ = 0.125 m, D = 0.30 m, K = 20, σ_φ = 0.1 rad ⇒ ~0.001 rad (0.06°) for a
  resolved path at high SNR; Rayleigh beamwidth remains ~24°, so this is
  *localisation accuracy*, not resolution. Honest framing: a lone hub is a
  coarse camera with a very precise rangefinder for few paths.
- **Mesh as distributed aperture (A2–A5):** nodes phase-aligned to the hub at
  packet rate ⇒ baselines of 3–10 m enter the coherent sum. Rayleigh angular
  resolution λ/B ≈ 0.7°–2.4°; cross-range at 5 m ≈ 6–20 cm; range from
  carrier phase mm-precision/differential, dm absolute after multipath. That is
  the "sharp" claim: **decimetre 3D point clouds of sparse scatterers, using
  the house as the aperture.**
- **Bandwidth is not the lever it is elsewhere.** S3 (HT40, 40 MHz) resolves
  delays ~3.75 m; commodity 802.11ax (160 MHz) resolves ~0.6 m (LoSRef's
  prerequisite). Aquila must trade *aperture for bandwidth* — which is exactly
  what the coherent polyhedron + mesh supplies.

## 4. What the space does **not** allow (closed parts, stated exactly)

1. **Absolute phase / absolute synchronisation with no reference** — closed by
   the `U(1)` gauge (`20`). Minimal crossing reference: one common-transmitter
   exchange per connected component. What crosses it *at home*: the hub.
2. **Simultaneous cross-node coherence with no reference and no exchange inside
   the post-CFO coherence interval** — closed by the M2 quantity. Minimal
   reference: one star broadcast (or one round trip) per coherence interval;
   `20`'s CRB says a star is optimal.
3. **Sub-diffraction resolution from a 30 cm aperture** — closed by
   diffraction. The polyhedron is a *key*, not a *lens*; it cannot make a 24°
   beam sharp. Only distributed baselines can.
4. **Monostatic full-duplex radar on one S3** — closed by self-interference
   (~110 dB SIC needed, synthesis `08`/`09`); Tx and Rx must be time-separated
   or spaced.
5. **Through-wall fine imaging at 2.4 GHz/40 MHz** — lightly bounded by
   material attenuation and bandwidth (synthesis `04`); presence/HAR through
   walls is achievable, centimetre wall-penetrating tomography is not.

## 5. Alternatives considered and why they lose

- **Wireless-only home mesh, no hub** (all-pairs round trips at CFO-sanitised
  rate): gated entirely by M2, and even if M2 passes, the mesh has no
  known-geometry anchor, so the gauge must come from blind self-calibration
  (weaker, slower, similarity-ambiguous). The hub dominates: it converts a hard
  distributed problem into N single-node problems plus one calibrated part.
- **One S3 with an RF switch across 20 faces** (cheapest coherent 3D array):
  valid *receive* variant (LO never re-locks across switch states, so
  calibration is a fixed path-delay table; the MobiSys switched-array result
  supports the signal processing). Loses per-face AGC/manifold calibration
  quality, single-error-point failure, and TX flexibility; keep as a
  cost-reduced hub for A6.
- **RIS-style passive surface instead of a powered hub**: cannot self-localise
  cheaply, needs an illuminator anyway, adds no coherence source. Rejected.
- **Rely on the home AP as reference**: works in principle (common-TX escape),
  but its phase/rate/timing are uncontrolled; keep as an opportunistic fallback
  only.

## 6. ESP32-S3 feasibility summary

| Mechanism | Key S3 dependency | Status | Evidence |
| --- | --- | --- | --- |
| A1 hub coherence | shared clock + conducted 2.4 GHz ref; per-channel cal | Principle demo on ESP32 family; S3 assembly untested | arXiv:2502.09405; ESP-Techpedia (C3+S3 clock buffer) |
| A2 self-survey | CSI phase across packets/channels; TX from hub | HYPOTHESIS (physics oK; multipath bias the risk) | Locata review; RIS surveys; PRL 118,183901 |
| A3 round trip | RX→reply→RX inside coherence; CSI on replies | HYPOTHESIS gated by M2 | WiDRa arXiv:2405.12168; RTCSI; arXiv:2506.07267 |
| A4 scene-lock | per-node CSI PLL; angular path isolation at 40 MHz | HYPOTHESIS | LoSRef arXiv:2602.05344; CJA 2021; RS 15:1505 |
| A5 gauge-SLAM | host compute; S3 streams | HYPOTHESIS | Rockah–Schultheiss; arXiv:2102.03565; arXiv:2504.09924 |
| A6 ambient camera | coherent faces + ambient CSI | HYPOTHESIS built on ESTABLISHED parts | ESPARGOS One datasheet; ACM 10.1145/3596263 |

Known S3 facts used: HT40 40 MHz with ~108 usable subcarriers (esp-csi #114;
ESP-IDF Wi-Fi docs); ns RX timestamp (`WIFI_PKT_RX_TIMESTAMP_NSEC`, `17`);
residual CFO after per-frame correction is visible and packet-varying
(esp-csi issue #121) — the measurement behind M2; CSI on ESP-NOW (esp-csi);
raw-TX rate configurability `esp_wifi_config_80211_tx_rate()` (ESP-IDF Wi-Fi
driver docs), with defaults at 1 Mbps 11b (`07`); FTM 1–2 m (`15`).

## 7. Chaining · existing models and data · own data

**(A) Chaining.** A1 ⇒ A2 (known illumination phases) ⇒ A5 (self-survey fixes
geometry) ⇒ A4 (known path lengths make the static lock interpretable) ⇒ mesh
distributed aperture; A3 supplies ranges + the asymmetry ledger and the
mesh-time transfer; A6 is an independent receive-only branch that needs only
A1. Highest-leverage enabler: the conducted hub (A1) — everything else degrades
gracefully without it, but nothing else supplies the gauge anchor. M2 gates the
wireless round-trip refinement, not the core.

**(B) Existing models/data.** ESPARGOS datasets (phase-coherent ESP32 CSI;
DaRUS/`espargos.net`, arXiv:2408.16377) are directly usable for A5/A6 algorithm
development and are the only public ESP32-family phase-coherent sets; MM-Fi /
CSI-Bench / SenseFi support learned-layer pretraining with mandatory domain
adaptation (`19`); channel-charting tooling including ESPARGOS
(`github.com/Jeija/ESPARGOS-WiFi-ChannelCharting`); Locata's carrier-phase
models for A2's estimation structure; WiDRa's sum-CP formulation for A3.

**(C) Own-data / minimal experiments.** The programme is measurement-limited,
not idea-limited. Minimal dataset 1: 5-face calibrated hub vs laser-tracked
single node, every scheduled illumination recorded (A2). Minimal dataset 2:
two-to-eight S3s, ping/unique-reply at maximum cadence, coax ground truth vs
OTA, temperature log (A3/M2). Minimal dataset 3: one hub + four nodes + a
moving/stationary person, all CSI raw, with a total-station or optical ground
truth (A4/A5). Each falsifies a distinct column of §6.

## 8. Decisive experiments, with THE one

Cheapest-decisive first (consistent with synthesis §6):

1. **M2 residual on S3** (two nodes, two-way, conducted-reference ground
   truth) — gates A3 and every cross-node wireless coherence claim.
2. **Face-phase stability** (5-face cluster, 30 min + temperature step) —
   gates A1's calibration cadence.
3. **THE single decisive experiment — the minimal lighthouse:** a 5-face
   conducting cluster (shared 40 MHz + conducted 2.4 GHz reference, one RF
   switch or per-face S3s) and one single-antenna S3 node in an ordinary room
   with multipath. In one rig, measure: (E1) cross-face coherence after a
   single calibration (<10° drift over 30 min); (E2) node self-localisation
   from face/channel-scheduled illumination vs a tape/laser reference
   (target median <20–30 cm); (E3) across-face differential response to a
   moved reflector versus the single-face baseline (a reproducible,
   geometry-consistent signature). **If E1 or E2 fails, the Lighthouse
   Aperture collapses to a per-face sensor and the invention dies**; if they
   pass, A2–A6 are engineering plus learning, and the home-scale aperture
   follows from replicating the hub.
4. Home-scale distributed aperture: two hubs + six nodes, coherent scatterer
   localisation vs ground truth (A4/A5).
5. Cross-home transfer: physical-pipeline vs fingerprint baseline (A5).

## 9. Evidence strength, residual unknowns, sources

**Evidence strength: 6/10** as a *proposal*. Justification: every sub-mechanism
has at least one primary demonstration on comparable hardware (ESP32-family
conducted coherence; COTS-SDR wireless coherence; COTS WiFi two-way carrier
phase; LoS/static referencing; geodesic arrays), which is unusually strong for
a new architecture; but no complete coherent 3D ESP32 system exists, M2 is
unmeasured on S3, mutual coupling/shadowing on a solid polyhedron is unmeasured
anywhere, and multipath bias in A2 is a real, quantified-by-others risk.
Grade components: A1 8/10 (direct precedent), A6 7/10, A3 6/10 (bleeds to M2),
A2 5/10, A4 5/10, A5 4/10.

**Residual unknowns.** (i) M2 on S3 (the hinge); (ii) per-channel face-phase
calibratability and schedule conditioning; (iii) mutual coupling / sector
manifold on a conducting polyhedron; (iv) achievable CSI phase noise/quantiser
floor at HT40 (8-bit HT-LTF) under AGC; (v) multipath-induced self-survey bias
in furnished rooms; (vi) asymmetry-ledger stability across temperature/days;
(vii) whether scene-lock holds with 24° sector beams at 40 MHz; (viii) legal TX
envelope for the schedule (ACMA LIPD/FCC Part 15, `07`).

**Primary sources (key).** ESPARGOS: arXiv:2502.09405; arXiv:2408.16377;
`espargos.net` + One datasheet (2026); combined-arrays docs. Espressif:
ESP-Techpedia esp-csi solution (esp-crab; C3+S3 clock-buffer board);
esp-csi issues #121, #114; ESP-IDF Wi-Fi driver + ESP-NOW docs. Two-way
coherence: arXiv:2506.07267 (Merlo/Lancaster/Nanzer; IEEE 2025); WiDRa
arXiv:2405.12168 (JSAC 2024); RTCSI UCL 10161602 (WCNC 2023); JSAC 2023
DOI 10.1109/JSAC.2023.3322812. References-by-static-path: LoSRef
arXiv:2602.05344; clutter-sync DOI 10.1016/j.cja.2021.08.040; dominant
scatterer DOI 10.3390/rs15061505; LOFAR blind cal MNRAS 474,1028.
Synchronised beacons: Locata review PMC6514591 (Sensors 2019). Self-
calibration: Rockah & Schultheiss, IEEE TASSP 35, 1987; arXiv:2102.03565.
Imaging/holography: PRL 118,183901 (DOI 10.1103/PhysRevLett.118.183901);
SenSys 2014 feasibility/limits of Wi-Fi imaging (DOI 10.1145/2668332.2668344).
Switched arrays: ACM DOI 10.1145/3596263. Spherical/geodesic arrays:
arXiv:2604.02701; URSI GA 2002 p1274. Learning: arXiv:2504.09924 (passive
channel charting); synthesis §5 (`19`). Distributed MIMO sensing context:
arXiv:2512.21953; MulDar MobiSys'26 DOI 10.1145/3745756.3809206.

**Internal cross-references:** `00-synthesis.md` §§1–9; `08`/`09` (polyhedron
verdict and TDM); `10` (OTA reciprocity/M2); `11` (common-ref/two-way);
`13` (static self-reference); `17` (timestamps); `20` (gauge and minimal
reference).
