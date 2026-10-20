# 22 — The Coherent Home: a phase-lighthouse polyhedron that turns a mesh of ESP32-S3 nodes into one self-surveyed 3D coherent aperture

Status: invention-research note (agent `b`). Australian English. Grades as elsewhere in `v2/`:
**ESTABLISHED** (published, reproduced, or on the ESP32 family), **HYPOTHESIS** (supported by
adjacent evidence; no direct demonstration), **SPECULATION** (plausible, unsupported).

Inputs read in full: `BRIEF.md`, `00-synthesis.md`. Method: primary-source search via Parallel
(layered, citations followed), then extraction of the strongest papers/repos. The six search
directions in the mission are each addressed; §4 grades what is genuinely new against the prior art
found.

---

## 1. Question and verdict

**Question.** Discover the invention that makes a home full of cheap ESP32-S3 nodes behave as one
sharp, self-calibrating, generalising 3D sensor — or name the physical quantity that closes the
space and the minimal reference that crosses it.

**Verdict.**

1. **The space is not closed; it is *gated* by one measurable quantity.** The quantity is
   `σ_φ,res(Δt)`: the residual *relative-LO phase* between two commodity oscillators after
   carrier-frequency-offset (CFO) estimation and tracking, over the interval `Δt` between phase
   re-anchorings. Raw coherence is ~5–10 µs; tracked coherence is a *measurement*, not a theorem
   (§3.1, §5). No result in the literature or the ecosystem closes this on S3 to the precision the
   system needs; equally, no result shows it is impossible. It is the go/no-go number.
2. **The invention exists, and it is a *system*, not a single trick: LIGHTHOUSE — a compact
   phase-coherent polyhedral beacon (the mission's premise hardware) that *exports* its local
   conducted coherence to the entire mesh over the air, and a protocol that turns that export into
   geometry, coherence, and sensing.** Its three functions:
   - **M1 Coherence export** — one antenna-coherent beacon broadcasts; every node locks its LO
     phase to the beacon's frames in software. One beacon replaces O(N) cabling. This is the
     O(1)-vs-O(N) reference optimisation and it is what makes "no house-wide cabling" possible.
   - **M2 Inverted-GNSS self-survey** — the beacon's 20 non-coplanar phase-locked faces form a
     small *known-geometry constellation*; double-differenced CSI carrier phase (DD-CSI) cancels
     every LO, every receiver clock and every per-face chain phase, leaving geometry. The mesh
     self-surveys to the beacon frame with no total station and no manual measurement.
   - **M3–M5 Coherent sensing, integrity, and a label engine** — once phases are anchored, the
     static room field becomes a reference hologram; the differential complex field is imaged with
     a house-scale aperture; a closure-phase *gauge audit* detects tracking slips; and coherent
     epochs generate self-labels that train the same mesh to operate *incoherently* afterwards
     (the "generalising" leg).
3. **What remains provably closed** (name-and-number, §4.6): monostatic operation of the polyhedron
   (self-interference cancellation ≈110 dB required, ESP32 provides ~none); single-shot centimetre
   3D *range* resolution at 20 MHz bandwidth (`c/2B` ≈ 7.5 m); absolute cross-node phase with no
   added reference (U(1) gauge), and — a new closing number found here — *range* geometry from a
   compact constellation alone, diluted by the factor `r/b` (node distance over beacon baseline,
   ~30× at 5 m). The mesh, not the beacon, must supply range baselines; the beacon supplies
   phase and bearings.

---

## 2. The boundary, made exact for this design

### 2.1 The gauge, and what is invariant {ESTABLISHED}

With independent LOs, node `i` transmitting and node `j` receiving subcarrier `k` measures

```
ĥ_ij(k) = t_i · r_j · g_ij(k) · e^{ j(θ_i(t) − ψ_j(t)) } · e^{ −j2π f_k τ_ij }
```

where `t_i/r_j` are TX/RX chain responses, `g_ij(k)` the propagation channel, `θ_i, ψ_j` the LO
phases, `τ_ij` the path delay. Only the difference `θ_i − ψ_j` is observable; on a connected
measurement graph node phases are fixed **up to one global additive constant** (a U(1) gauge, plus
time/frequency gauges) — `00-synthesis.md` §1, from note 20, and standard in the distributed-arrays
literature. Invariants that survive the gauge: amplitude; per-link subcarrier phase slope
(delay); per-receiver inter-antenna phase (AoA *within* one receiver); temporal differentials
(Doppler); and the two-way combinations below. AoA *across* independent receivers is unobservable
without a reference.

### 2.2 The five usable observables {ESTABLISHED algebra}

| # | Observable | LO content | Channel content | Use |
| --- | --- | --- | --- | --- |
| O1 | one-way `ĥ_ij` | `θ_i − ψ_j` | full | needs anchoring |
| O2 | **sum-carrier phase** `ĥ_ij·ĥ_ji` | cancels (to chain constants + tracking residual) | `g²` | **differential ranging**; WiDRa, mm-level |
| O3 | **ratio** `ĥ_ij/ĥ_ji` | **2×(LO difference)** | cancels (reciprocity) | **LO tracker immune to motion** + hardware fingerprint |
| O4 | **common-TX difference** `ĥ_ip/ĥ_jp` | `θ_i − ψ_j` only | geometry difference only | **phase anchor** (transmitter phase cancels entirely) |
| O5 | **double difference** `(ĥ_ip/ĥ_iq)/(ĥ_jp/ĥ_jq)` | cancels | geometry only | **self-survey** (GNSS DD analogue) |

O3 deserves emphasis: because the propagation channel is reciprocal, the *ratio* of the two
directions of an exchange is the one observable in which the environment cancels and solely
hardware asymmetry and LO phase remain — an environment-blind, 2×-sensitive LO tracker and a stable
physical fingerprint for node identity. O2's "square-root product cancels the LO offset" trick is
already used for multiband CSI delay estimation (Shu et al.-style multiband CSI papers; see
sources). O5 is textbook GNSS RTK double differencing; what is new here is its application to
Wi-Fi CSI against a *coherent polyhedral beacon* (§3).

### 2.3 The escaping reference, and why one beat is not enough {ESTABLISHED + HYPOTHESIS}

The published escapes are a common-transmitter reference, OTA reciprocity, and a shared/conducted
reference. For a *home mesh* there is a subtlety the source notes under-weight: a single common
broadcast fixes the gauge **at one instant**; keeping the mesh coherent *between* anchors requires
tracking `ψ_i(t) − ψ_j(t)` across dead time. So the reference must be a *service*, not an event:
either a continuously broadcasting anchor (this invention) or continuous two-way exchanges (the
alternatives in §5.1). The distinguishing quantity is exactly `σ_φ,res(Δt)`.

### 2.4 Numbers that matter

- Relative crystal offsets ±10 ppm → 0–20 ppm differential; raw `τ_{π/2} ≈ 5–10 µs` (note 17).
- Post-CFO residual 10–100 Hz ⇒ a tracking interval of ~1.6–16 ms before π of phase is lost;
  the CFO *estimation error* over the epoch is what matters: to hold `σ_φ ≤ π/4` over `Δt = 20 ms`
  requires `σ_Δf ≲ 6 Hz`. {HYPOTHESIS on S3; WiDRa achieves usable CFO removal with a piecewise fit
  every 0.5 s on COTS Wi-Fi, because its exchange is short (FTM burst), not because 0.5 s of
  open-loop hold is coherent.}
- WiDRa's demonstrated limit: differential range RMSE <1 mm on commodity hardware in Rician
  channels; cycle slips above ≈250 mm/s at 25 ms sampling; error accumulates with time since the
  last fix; mm accuracy needs Rician ≥ 7 (strong LoS). {ESTABLISHED, COTS not ESP32}
- ESPARGOS: with a shared 40 MHz clock, the *phase difference* between its eight ESP32-S2s is
  arbitrary (PLL initial phase re-randomises per reset and per channel change) but **stable over
  long intervals**; a periodically switched-in "WiFi-frame" conducted phase reference measures the
  constants. {ESTABLISHED on ESP32-S2}
- ESP-PPB (open-source, unrefereed, Feb 2026): ESP32-C3 + VCTCXO disciplined over the air by FTM,
  master sends a reference frame 1 ms after each external CSI frame; reported single-frame phase
  accuracy <1° best case / <10° typical, clock alignment ~1 PPB. {observed artifact; not
  peer-reviewed}

---

## 3. THE INVENTION — LIGHTHOUSE

**One-line definition.** A compact, 3D, conducted-coherent, 20-face ESP32-S3 polyhedron (the
*Lighthouse*) that continuously broadcasts a scheduled, face-swept sounding waveform, thereby
(a) phase-anchoring every cheap receiver in the home within milliseconds, (b) providing a
known-geometry coherent constellation that lets the mesh self-survey itself by DD-CSI, and
(c) acting as the transmit aperture of a home-scale coherent sensor whose differential field is
imaged and whose epochs self-label a learned model.

Why a *polyhedron* specifically (vs a single extra beacon node): (i) its faces are phase-locked to
one conducted reference, so it is a coherent array, and its geometry is a fabricated rigid body —
a pre-surveyed constellation; (ii) non-coplanar faces resolve the front/back mirror ambiguity that
a planar array cannot (a planar aperture measures only a half-space — standard spherical-array
result); (iii) full-sphere faces give usable geometry dilution of precision from every direction,
no scan loss, and (from PCB fab) a 30 cm aperture in a single part; (iv) 20 faces give 20
"satellites" for O5 — massively overdetermined for DD-CSI; (v) face orientations differ, so the
beacon is a *polarimetric* constellation for free (Jones calculus across faces — ESPARGOS already
does polarisation from two feeds). Geodesic-dome phased arrays of triangular panels are an
established antenna architecture (GDPAA, URSI 2002); putting one ESP32-S3 behind every triangle and
using it as a *sensing phase reference* is not established anywhere.

### 3.1 M1 — Coherence export (the core enabling mechanism)

**Statement.** The Lighthouse transmits an ESP-NOW HT40 (OFDM, MCS0) frame from one face every
`T_b ≈ 5–20 ms` (and sweeps faces). Every receiver logs CSI of each beacon frame with its
nanosecond RX timestamp. Because all receivers observe the *same* transmitted waveform, the
transmitter's phase is common and cancels in any cross-node difference (O4). Each node therefore
tracks its own `ψ_i(t)` relative to the transmitted frame, and `ψ_i(t) − ψ_j(t)` is observable at
every beacon frame. Between frames each node extrapolates with its tracked CFO. The mesh is
coherent in software, with one physical beacon.

*The economics:* pairwise two-way exchanges also fix the gauge (connected graph, N−1 edges). But
each pair needs MAC-level turnaround — milliseconds on S3, with no TX hardware timestamp — so an
N-node pairwise epoch (N² exchanges) cannot complete inside a coherence window and does not scale.
A single broadcast × N listen is *simultaneous*: one timeline, one transmitter phase, N parallel
receipts. This one-for-one architectural difference is the reason a beacon beats the mesh's own
transmissions, and it is why the conducted-coherent polyhedron — the only device with enduring
phase certainty — must be the anchor.

**Genuinely new.** Common-transmitter phase referencing is established in radio interferometry
(phase referencing, self-cal), in distributed arrays (master/secondary tone transfer; BeamSync;
AirShare; TDD-flow OTA calibration) and — as an artisanal open-source artifact — on ESP32-C3
(ESP-PPB). New here: exporting a *conducted-coherent 3D array's* phase to an arbitrary mesh as a
continuous service; the O(1)-vs-O(N²) argument made explicit for ESP32 MAC turnaround; the
closure-phase *gauge audit* as a runtime integrity monitor; and the use of O3 (the ratio) as a
motion-immune LO tracker.

**Physical mechanism.** O4 with the beacon as `p`; CFO tracking between O4 samples; per-frame
phase = common transmitted phase + channel(beacon→node) + node chain − node LO phase.

**What must be true.**
1. The ESP32-S3 CSI retains a *common phase per frame that drifts smoothly* (CFO), without
   per-frame random phase re-locks. Evidence: esp-csi issue #121 users observe exactly this
   (consistent subcarrier slopes; per-packet intercept drift = residual CFO); Wi-ESP finds ESP32
   phase "usable" where Intel/Atheros phase is random. {HYPOTHESIS on S3, strongly indicated}
2. `σ_φ,res(20 ms) ≲ 0.3 rad` after tracking (see §2.4) — the E1 test.
3. The driver does not silently apply a random per-frame common-phase correction. (Issue #121
   hints some compensation may exist in the CSI chain; a firmware experiment must confirm the raw
   intercept is preserved, or an option must be added to disable it.)

**Falsifier.** Feed two independent S3 receivers the same source through a coax splitter (ground
truth) *and* wirelessly; if the wireless cross-node phase is statistically flat (no trackable
CFO drift), or if residual after tracking exceeds ~1 rad over 20 ms with no model structure, M1
fails on S3 and the wireless-only route closes; the conducted polyhedron would then only cohere
*itself* (Track-3-ceiling).

**ESP32-S3 feasibility.** CSI per frame: yes (all ESP32 family; 64 LLTF subcarriers at HT20, 128
HT40 per Espressif docs; ns RX timestamp available; ESP-NOW HT40 MCS0 is the established CSI
excitation primitive in esp-csi). No TX timestamp is needed for phase work. Per-channel PLL
re-lock invalidates phase — so the beacon and mesh must stay on one channel per epoch, or
re-anchor after every channel change (the conducted reference makes the *beacon* immediately
re-coherent; the mesh re-locks in one frame interval).

**Cost.** Beacon: 20 × (S3 module ~US$3 + antenna/face PCB) + hub (Ethernet switch or small FPGA)
+ 40 MHz fan-out + 2.4 GHz reference distribution ≈ US$250–500 at hobby volume; receivers
US$5–10 each. **TX needed: yes** (one beacon; receivers need never transmit for M1).

### 3.2 M2 — DD-CSI self-survey (inverted GNSS against a fabricated constellation)

**Statement.** For node `i` and beacon faces `p, q`:
`DD = [∠ĥ_ip − ∠ĥ_iq] − [∠ĥ_jp − ∠ĥ_jq]` → receiver LO phases cancel (O5); face chain phases and
the beacon's common phase cancel *because the faces are conducted-coherent* (their residual
per-boot constants are stable and are absorbed into the ambiguity terms once, exactly as GNSS
ambiguities absorb receiver-clock and hardware constants). What remains is geometry:
`(τ_ip − τ_iq) − (τ_jp − τ_jq)` times `2π f_k` per subcarrier — one equation per (node pair,
face pair, subcarrier, frame). The beacon's face coordinates are known by fabrication (rigid
body), which *fixes the gauge: node positions come out in the beacon's frame, not up to rotation*.

**Two deliverables, honest split (the compact-constellation dilution).** With `D ≈ 30 cm` faces
and node ranges `r ≈ 3–8 m`, the faces' direction vectors differ by ~`D/r ≈ 3°`: DD-CSI from the
beacon alone measures node *bearings* superbly (coherent aperture: `σ_θ ≈ λ/(2πD·√(...)) ≈ 0.3–1°`)
but *ranges* poorly diluted (`σ_r ≈ σ_phase · r/b ≈ 20–50 cm` at 5 m). The mesh fixes this: once
phases are anchored, node-to-node links (O2/O4/O5 between nodes, and the nodes' own transmissions
used as additional constellation points) provide the long baselines. The system is a *total
station in reverse*: the Lighthouse is the theodolite (absolute bearings, from its 20-face coherent
aperture receiving the nodes' replies — no timing needed), and the mesh is the EDM (distances).
The room's walls, once their reflection delays are resolved by the same solver, add virtual
anchors (established multipath-assisted localisation) with no hardware at all.

**Genuinely new.** The pattern "compact coherent multi-antenna beacon + carrier-phase DD for
inverted indoor positioning" has serious antecedents: pseudolites and the GNSS inverted-radar
proposals (Kao & Tsai 2003; Samama et al.), and above all **Locata**, a commercial terrestrial
constellation (LocataLites, 2.4 GHz ISM, OTA TimeLoc synchronisation to ~30 ps, carrier-phase
centimetre positioning, 11 mm indoor in EU tests). Locata is the closest prior art and must be
credited. What is new in M2: (i) commodity $3 radios with *no* OTA time transfer — the faces are
made coherent by *physical construction* (conducted reference in one rigid body), so there is no
TimeLoc-equivalent negotiation and no per-site survey of the constellation; (ii) the observable is
*OFDM CSI subcarrier phase* rather than GNSS/Pseudolite chipping signals; (iii) the rovers are a
self-surveying *mesh* that then turns around and senses, rather than a mobile receiver; (iv) the
cost is ~2–3 orders of magnitude lower.

**What must be true.** Per-boot/channel face constants are stable during operation (ESPARGOS:
ESTABLISHED on S2; S3 must be re-verified); integer ambiguities resolvable from coarse CSI delay +
face geometry (HYPOTHESIS — the compactness cuts both ways; ambiguity resolution with a degenerate
constellation relies on the mesh baselines and on temporal continuity); multipath does not bias
carrier phase beyond tolerance in LoS-dominant links (multipath is the classic killer — Locata and
pseudolite literature agree; WiDRa's ≥7 Rician requirement quantifies it).

**Falsifier.** E2: two coherent transmitters (shared 40 MHz clock, ESPARGOS-style, no phase
reference needed yet) with known separation, and two independent receivers at a measured distance;
check that the DD-CSI phase history (a) is constant when nothing moves (⟹ constants stable),
(b) tracks a physically moved receiver to cm-level (⟹ geometry works), and (c) whose residual
matches the ambiguity-solver's predicted condition number. If DD-CSI residual is dominated by
per-frame randomness rather than by resolvable ambiguity, M2 degrades to bearings-only.

**ESP32-S3 feasibility.** Transmission of ESP-NOW HT40 for sounding; CSI of HT40 gives 128
subcarriers (16 MHz of usable span → coarse delay ~60 ns, enough to *bootstrap* ambiguity search);
ns timestamps aid frame pairing across nodes (ESPARGOS clusters frames by MAC + timestamp +
headers — established). **TX needed: yes** (beacon; optionally nodes for their own constellation
points).

### 3.3 M3 — The Coherence Epoch protocol (scheduled sweep + unique replies + two-way products)

**Statement.** A repeating epoch (target 20–50 ms, bounded by `σ_φ,res`):
1. *Anchor phase:* beacon broadcasts `F` face-swept HT40 frames (one per face, or a subset); every
   node logs CSI → O4 → per-node LO phase for this epoch.
2. *Mesh answers:* nodes transmit in assigned micro-slots with unique payloads (identity,
   sequence); each reply is itself a common-transmitter broadcast for every *other* node → O4
   between all node pairs, plus O2/O3 with the beacon and with each node that receives it.
3. *Two-way products:* the beacon's receipt of replies closes O2 (range change, WiDRa-style) and
   O3 (hardware fingerprint).
4. *Gauge audit:* triples of receivers form closure products against the same frame —
   `(ĥ_ip/ĥ_jp)·(ĥ_jq/ĥ_kq)·(ĥ_kp/ĥ_ip)⁻¹...` — which must equal ~1 (all LO and channel terms
   cancel for a common transmitter); a departure flags a slip/tracking failure *before* it
   corrupts a solver. {New as a runtime integrity layer; closure phase itself is ESTABLISHED
   radio-interferometry physics (Jennison; Cornwell & Wilkinson).}
5. *Recovered per epoch:* per-node phase ledger; full complex CSI tensor on all captured
   node–node and beacon–node links (an "exposure" of the home); ranges/range-rates from O2;
   identity from O3; geometry updates from O5.

**What is genuinely new:** the epoch *as an aperture* — every node in the home is on one timeline,
phase-referenced to one coherent body, N² links captured with O(N) transmissions; and the
audit-by-closure layer. The round-trip scheme requested in the brief is *inside* this protocol,
and we can now answer its question precisely: ping + unique replies + two-way products recover
(a) **differential ranges** (O2; mm-level demonstrated by WiDRa on COTS), (b) **hardware
fingerprints / identity** (O3: channel cancels, only chain asymmetry × LO remains), and (c)
**relative geometry** (O5 + reply-as-constellation), but *not* absolute synchronisation and *not*
coherent "obstruction maps" by themselves; and they break (i) on node mobility above ~λ/(2Δt)
(cycle slips: 250 mm/s at 25 ms — WiDRa), (ii) when the LoS is weak (Rician ≲ 7), (iii) whenever
the exchange interval exceeds the tracking residual, and (iv) on ESP32 as a *pairwise mesh*
because MAC turnaround × N² exceeds coherence — the beacon protocol exists precisely to avoid
(iv).

**ESP32-S3 feasibility.** All frames are ESP-NOW HT40; slotting at ~1–5 ms dominates the epoch
budget; the beacon face-switch inside a sweep is fast (RF switch / scheduled TX per face); the
hub aggregates CSI from faces and (optionally) from nodes via Wi-Fi or the data plane.
**TX needed: yes.**

### 3.4 M4 — Coherent field sensing (the "sharp" leg)

**Statement.** With phases anchored per epoch, the mesh treats the room as a radio hologram:
*record the static complex field as the reference beam; every epoch's differential field is the
object beam; coherently back-propagate (or time-reverse) the differential tensor through the
estimated geometry to focus energy on whatever changed.* This is coherent change detection
(SAR/InSAR technique, ESTABLISHED) and time-reversal/DORT focusing (ESTABLISHED physics for waves
in rich scattering) applied to a phase-anchored Wi-Fi mesh (no published demo — HYPOTHESIS).
Three concrete sensing gains follow mechanically:

- **mm-scale motion**: temporal carrier-phase differencing — the node-pair phase history changes
  at `2π·2·Δr/λ` with motion; sub-mm changes are visible at ~SNR-limited phase noise (~0.05 rad ≈
  0.5 mm at 2.4 GHz). WiDRa already demonstrates mm differential range with un-synchronised COTS
  hardware; the mesh extends it to many concurrent pairs (a phase-coherent RTI, replacing
  variance-based RTI).
- **Sharpness from the network aperture**: with node geometry known and phases common, the
  combined aperture is house-scale: `λ/D ≈ 0.7°` at D = 10 m plus the beacon's 20-face transmit
  diversity; the *angular* point spread is sharp. Range resolution stays ~metres (§4.6), so the
  instrument is a *direction-and-change* camera, not a voxel-perfect tomograph: a "sharp 3D
  sensor" in bearing × bearing × differential-phase, with range supplied by multipath geometry.
- **Multipath as extra aperture (environment-as-reference, made coherent)**: because phases are
  anchored, the room's static reflections stop being nuisance and become *coherent virtual
  anchors* (established virtual-anchor/multipath-SLAM mathematics; here they are coherent images
  of the beacon, so their positions are solvable by the same DD/ambiguity machinery and they
  densify the effective constellation from 20 points to the room's own surfaces). The reference
  hologram (static field) is the "environment as shared reference" in its strongest form: it is
  *measured and stored*, need not be modelled, and its changes are the signal.

**What must be true.** Static multipath coherence at 2.4 GHz (wall roughness ≪ λ = 12.5 cm: true
for painted plasterboard — ESTABLISHED by indoor channel models and by Coherent Change Detection
practice); phase anchoring holds across the epoch; reciprocity holds for O2/O3 (TDD, balanced
chains, no motion during the exchange — ESTABLISHED caveats). **Falsifier.** E3: 4–6 anchored
nodes; capture baseline + a person moving; if the differential complex field back-projects to a
focused spot whose position tracks the person (vs an unfocused amplitude-only smudge), M4 passes;
if focusing fails even though E1 passes, either the field model, multipath dominance, or the
anchoring cadence is inadequate.
**ESP32-S3 feasibility.** Compute is host-side; the S3s only stream CSI (esp-csi does ~100 Hz ×
128 subcarriers comfortably; binary output exists in the ecosystem). **TX needed: yes** (the
beacon again).

### 3.5 M5 — The label engine (the "generalising" leg)

**Statement.** The coherent epoch is a *self-labelling* instrument: it yields geometry and mm
motion without human labels. Those labels supervise a model that runs on ordinary, incoherent,
beacon-free deployments: train on coherent-epoch data (features: amplitude, per-link delay, intra-
receiver AoA, Doppler), deploy on ambient-CSI-only nodes. Cross-home transfer then rides on
physics invariants (delay/AoA/Doppler are device- and room-independent constructs) plus synthetic
pre-training — the route the synthesis already identifies as the strongest generalisation lever
(ray-traced Sim2Real reached 82.1% F1 with 20% real labels; cross-modal distillation is the
strongest known lever). **Genuinely new:** a coherent instrument used as a *label oracle* for an
incoherent fleet; the mesh pays for coherence only during "exposure" epochs and spends the rest of
its life cheap. **What must be true:** labels are accurate enough (M2/M4 pass); features are
stable across homes. **Falsifier.** Hold out a home; if a model trained only on coherent-epoch
labels from other homes fails on it (no better than same-home supervised baselines, or worse than
the synthesis's synthetic-pretrain route), the label engine adds nothing. **TX needed: yes, but
only during training.** Cost: nil extra (the same mesh).

### 3.6 What is genuinely new — summary table

| Component | Closest prior art | What LIGHTHOUSE adds |
| --- | --- | --- |
| Conducted-coherent ESP32 array | ESPARGOS (S2, planar, 8 faces) | 3D polyhedron, 20 faces, ESP32-S3, hub clock/reference/aggregation |
| Common-TX phase referencing | interferometry phase referencing; BeamSync; TDD-flow calibration; ESP-PPB (C3, unrefereed) | Continuous export from a conducted-coherent body to an arbitrary mesh; O(1) vs O(N²) argument; closure-phase audit |
| Carrier-phase DD positioning | GNSS RTK; pseudolites; Locata (2.4 GHz, cm, OTA ps timing) | DD-CSI on commodity OFDM CSI; construction-coherent constellation (no TimeLoc, no survey); rovers are a sensing mesh |
| Sum-carrier-phase ranging | WiDRa (COTS, mm differential) | Used inside an epoch protocol with a coherent reference; extended to N² simultaneous links |
| Ratio observable `ĥ_ij/ĥ_ji` | reciprocity-calibration literature | Environment-blind LO tracker + identity fingerprint at 2× sensitivity |
| Coherent imaging of a room | Holl & Reinhard "Holography of Wi-Fi radiation" (mechanical scanner); coherent multistatic imaging in 6G ISAC (theory/sim) | Distributed phase-anchored mesh as the aperture; static-field reference hologram; walls as coherent virtual anchors |
| Coherent distributed arrays without cables | Nanzer group (SDRs, two-tone ranging/self-mixing; >90% coherent gain) | Commodity S3 radios via CSI tracking rather than dedicated waveforms |

---

## 4. Directions searched — findings and grades

### 4.1 (1) Wireless coherence on commodity silicon
ESTABLISHED: master/secondary tone-based frequency sync in distributed arrays (Nanzer et al.,
self-mixing receiver), OTA inter-AP calibration (BeamSync; TDD-flow paper with Kalman tracking and
explicit warning that "frequent phase re-estimation is critical when LOs drift rapidly"; estimates
are only up to a common additive constant), and WiDRa's sum-CP. HYPOTHESIS: ESP32-S3 specifically
— indicated by ESP-PPB (C3, 1–10° single-frame, unrefereed) and by esp-csi phase behaviour, but
**not demonstrated**; the synthesis's M2 measurement remains the gap. The protocol that tracks
phase "fast enough" is exactly M3's beacon cadence + CFO extrapolation; the required cadence is
set by `σ_Δf` (few Hz) and the epoch length.

### 4.2 (2) Self-calibrating learned array; joint geometry + targets; transfer
ESTABLISHED: multipath-assisted localisation and multipath-SLAM factor graphs solve joint
anchor/target geometry from delay statistics; "joint self-calibration of receiver geometry, timing
and target positions for multistatic radar autofocus" (2026) shows joint identifiability of a
common receiver offset with target position given enough receivers. M2 is the *physics* answer
(geometry from a coherent known body, no learning needed for the survey); M5 is the learning
answer (labels for what physics cannot name). Transfer: synthesis §5 route (synthetic + weak
supervision).

### 4.3 (3) Environment as shared reference / distributed aperture
ESTABLISHED: virtual-anchor geometry; multipath-assisted positioning; coherent change detection in
SAR; time reversal focusing exploits the environment as part of the lens. HYPOTHESIS: the
*coherent* version on a phase-anchored mesh (M4) — no Wi-Fi demo found. SPECULATION: super-
resolution beyond the diffraction limit via multiple scattering in a furnished room (real in
controlled experiments — Lerosey et al.; contested in general practice; do not promise it).

### 4.4 (4) Sharp 3D from coarse apertures: geometry, polarisation, multi-band, asymmetry
ESTABLISHED: non-coplanar/spherical arrays measure a full sphere with no mirror ambiguity and
better worst-case CRB than planar; geodesic-dome phased arrays are a built architecture (GDPAA);
mutual coupling on 3D arrays is hard to characterise (spherical-array survey, 2026). The
*round-trip hardware-asymmetry signature* is real and stable (ratio O3; BeamSync cites reciprocity
calibration drift ≈2.7% over 4 hours, i.e., the fingerprint is slowly varying, not fixed). Multi-
band: ESP32-C5 is dual-band 2.4/5 GHz (esp-csi-rs lists C5) — a multi-band Lighthouse would
sharpen ambiguity resolution; SPECULATION for S3-only builds.

### 4.5 (5) The round-trip protocol
Answered in §3.3: recovers differential range, identity/fingerprint, relative geometry; breaks on
cycle slips, weak LoS, coherence window, and MAC-turnaround scaling. Two-way products (O2) need
*bidirectional reciprocity* and balanced chains; long-term residual is dominated by reciprocity
calibration error, not by LO noise (BeamSync).

### 4.6 (6) Different excitation/composition — and what is provably closed
Excitation: ESP-NOW HT40 MCS0 is the only ESP32 primitive that both excites CSI and is licit
under the class licence; raw `esp_wifi_80211_tx` defaults to 1 Mbps 11b (no OFDM preamble → no
usable CSI) — already in note 07. TDM across nodes is mandatory (no simultaneous TX on one
channel; ESP32 has no TX timestamp). Coherent transmit *within* the polyhedron requires a hub
trigger (premise hardware provides the shared clock; an FPGA hub can trigger faces) — HYPOTHESIS.
Passive illumination: coherent passive location from ambient APs is the alternative A2 below.
Synthetic aperture by switching: the face sweep *is* a switched synthetic aperture on transmit;
the mesh *is* one on receive.

**Closed by physics (name + number):**

| Claim that will fail | Closing quantity | Minimal reference that crosses it |
| --- | --- | --- |
| Cross-node coherent sensing with no reference | U(1) gauge; raw `τ_{π/2}` 5–10 µs | one common broadcast per coherence interval (beacon), or one two-way exchange per connected edge |
| Monostatic polyhedron radar | ~110 dB self-interference cancellation needed; ESP32 offers ~none | separate TX/RX bodies (beacon transmits, mesh receives) — bistatic/multistatic |
| Centimetre single-shot 3D *range* resolution | `c/2B` ≈ 7.5 m at B = 20 MHz | frequency aggregation, or long coherent integration; never single-shot |
| Absolute range from a compact constellation alone | geometry dilution `σ_r ≈ σ_phase·r/b` (≈20–50 cm at 5 m, 30 cm baseline) | long baselines supplied by the mesh itself |
| Absolute cross-node phase between two isolated nodes | no reference at all | one of the three escapes; there is no fourth |

---

## 5. Alternatives (each graded with the required fields)

### 5.1 A1 — The two-way tracked mesh (no beacon, no new hardware)
- **Statement:** nodes continuously exchange tracked two-way frames (O2/O3) and maintain a
  Kalman-filtered LO-phase ledger; coherence is established edge-by-edge, like the TDD-flow
  calibration paper and WiDRa, but continuously.
- **Genuinely new:** modest — the protocol composition and the O3-based motion-immune tracker.
- **Mechanism:** O2 removes the LO (product), O3 gives `2×LO + asymmetry`, tracked per pair.
- **Must be true:** O(N²) exchanges fit the coherence budget — on ESP32 they do not (§3.1);
  with MAC turnaround ~ms, only a sparse tree fits, so the mesh is coherent as a tree, not as an
  aperture. WiDRa's 25 ms cadence and cycle-slip limits cap dynamics.
- **Falsifier:** measure end-to-end phase residual around a 4-node loop; if closure error grows
  faster than the tracker can absorb, A1 fails for apertures (still fine for ranging).
- **ESP32-S3 feasibility:** high per pair; low as an aperture. **Cost:** zero extra.
  **TX needed: yes** (ubiquitous).
- **Verdict:** the fallback if the polyhedron is unavailable; strictly dominated by M1 for
  apertures, but it is the *only* option that needs no special hardware.

### 5.2 A2 — Ambient-frame interferometry (no TX at all)
- **Statement:** pair frames received by ≥2 nodes (any third-party AP/phone as common
  transmitter, identified by MAC/sequence/ns timestamp); the cross-node phase difference is
  observable per frame (O4 with an uncooperative `p`); a radio-interferometry-style gain solver
  (self-calibration à la Cornwell & Wilkinson) fits per-node phase drift and (with geometry)
  positions; closure triplets give TX-free integrity.
- **Genuinely new:** applying interferometric self-cal / closure to commodity Wi-Fi CSI pairs —
  no direct Wi-Fi demo found.
- **Must be true:** enough ambient frames hit ≥2 nodes (yes in a normal home); the per-node LO
  phase is trackable frame-to-frame even across variable frame timing (harder than a regular
  beacon — dead time is irregular); unknowns (node phases, node positions, AP positions) are
  identifiable only up to a gauge and need either motion, a known anchor, or the mesh's own
  transmissions.
- **Falsifier:** E1 with ambient frames substituted for the beacon; if per-node phase cannot be
  tracked across irregular gaps, A2 collapses to per-frame differences (still useful for
  closure-phase change detection, not for geometry).
- **ESP32-S3 feasibility:** highest of all (receive-only, promiscuous CSI). **Cost:** nil.
  **TX needed: no.** **Verdict:** the zero-hardware upgrade path; weaker (irregular cadence,
  unknown AP geometry), but it is the *only* fully passive route to cross-node phase.

### 5.3 A3 — Coherent virtual anchors (walls as the aperture)
As §3.4: with phase anchored, resolve each link's multipath components (delay + per-face angle),
mirror each known beacon face about detected surfaces; the room's walls become coherent virtual
faces, densifying the constellation. HYPOTHESIS; falsifier via E3's focus sharpness with and
without the virtual-anchor model; TX: beacon only.

### 5.4 A4 — Coded lighthouse illumination (compressive imaging)
The beacon sweeps known random phase codes across faces (coherent TX, hub-triggered); receivers
measure the *incoherent* intensity of the coded illumination; the inverse problem recovers the
room's reflectivity at ~1 m (amplitude-only, bandwidth-limited) with no cross-node phase at all.
Genuinely new as a Wi-Fi modality; SPECULATION on ESP32 until coherent multi-face TX is built
(hub trigger + identical PPDUs); TX: beacon only. Useful as a *degraded mode* if M1/E1 fails.

### 5.5 A5 — Polarimetric beacon (faces as a vector sensor)
Face orientations differ → Jones-calculus polarisation of every path per ESPARGOS's two-feed
method, extended to 20 faces. Assists material/orientation inference (M4's material layer) and
improves virtual-anchor discrimination. TX: beacon.

---

## 6. Decisive experiments

**E0 (characterise the observable).** On one S3 pair, coax-coupled with a known delay, log CSI;
verify the phase model: slope = delay (stable), intercept = CFO (drifting smoothly), and check
whether firmware applies a per-frame common-phase correction that would destroy the intercept.
*Falsifier:* intercept is random per frame ⟹ stop; the rest of the programme needs an IDF patch
or a different chip.

**E1 — the single decisive experiment: "Wireless phase-anchor residual" (the go/no-go).**
Rig: three ESP32-S3 devkits; a 3-way splitter/attenuator harness so that two "receivers" can each
switch between their antenna and a *conducted common source* (ground truth), ESPARGOS-style; the
third node is the beacon, ESP-NOW HT40 at 100–200 Hz. Both receivers log CSI of every beacon frame
with ns timestamps; each also periodically samples the conducted reference to measure its true
relative LO phase. Compute the wireless cross-node phase difference time series, fit CFO, and
report `σ_φ,res(Δt)` for Δt = 1, 5, 20, 100 ms, plus triple-node closure phase. *Decision rule:*
`σ_φ,res(20 ms) ≤ 0.3 rad` and closure stable ⟹ coherent modes are on (proceed to E2/E3);
`σ_φ,res` growing without trackable structure ⟹ the wireless-only escape is closed on S3 and only
the conducted polyhedron coheres itself. This experiment converts the synthesis's M2 from an open
question into a number, and it is the cheapest test that can stop or justify the whole programme.

**E2 (self-survey).** Two shared-clock transmitters at known separation (ESPARGOS-style clock
injection) + two receivers at a tape-measured baseline; DD-CSI phase constant when static; move one
receiver and check cm-level recovery of the *change* against the tape. *Falsifier:* DD-CSI noise
dominates the geometric term over metre-scale moves.

**E3 (coherent change imaging).** 4–6 anchored nodes (E1 output), baseline record, person walks;
back-project the differential complex tensor; measure the focus spread and position error against
ground truth. *Falsifier:* no better than amplitude-only variance RTI.

**E4 (label engine).** Train a model on coherent-epoch labels from two rooms; evaluate on a third
with coherent labels withheld. *Falsifier:* no gain over synthetic-pretrain without labels (the
synthesis route).

---

## 7. ESP32-S3 feasibility summary

| Capability | S3 status | Evidence |
| --- | --- | --- |
| CSI capture (LLTF/HT20/HT40) | ESTABLISHED | Espressif docs; esp-csi; esp-csi-rs |
| CSI phase with driftable common intercept | ESTABLISHED structure, S3-specific residuals unquantified | esp-csi issue #121; Wi-ESP |
| ns RX timestamp | ESTABLISHED (note 17) | ESP-IDF; ESP-PPB patch |
| Same-frame capture across receivers | ESTABLISHED | esp-csi multi-receiver broadcast pattern; ESPARGOS clustering |
| Conducted clock+phase-reference coherence on ESP32 | ESTABLISHED on S2 (ESPARGOS); S3 unverified | ESPARGOS papers |
| Wireless phase tracking on ESP32 | observed on C3 (ESP-PPB, unrefereed); **S3 unverified — the gap** | ESP-PPB repo |
| Coherent multi-face TX | HYPOTHESIS (needs hub trigger) | premise hardware; FPGA |
| Monostatic full-duplex | BLOCKED (~110 dB SIC) | note 08/09 |
| TX hardware timestamp for raw TX | ABSENT (do not design around it) | note 17 |

**Cost envelope (home):** lighthouse US$250–500; 10–20 receivers US$100–200; host compute
(existing). Compared with ESPARGOS-class instrumentation and Locata-class systems, this is the
point of the design: **commodity silicon, one special part.**

---

## 8. Evidence strength, residual unknowns, and the three strategic sections

**Evidence strength: 6/10.** Justification: every *component* is established somewhere
(conducted ESP32 coherence; wireless ESP32-C3 phase locking, unrefereed; COTS mm differential
ranging; Locata cm carrier-phase constellations; RTK theory; interferometric self-cal; TR imaging;
multipath localisation), and the premise hardware is a known antenna architecture realised in
commodity parts. The 4 points withheld are for the *unmeasured S3 numbers*: `σ_φ,res(Δt)`, DD-CSI
residual and ambiguity behaviour on CSI (not GNSS signals), S3 conducted-reference viability and
per-channel re-lock behaviour, coherent multi-face TX, and the fact that no phase-anchored Wi-Fi
mesh imaging exists outside this note's proposal. A passing E1 moves this to 7/10; E2+E3 to 8/10.

**Residual unknowns.** (i) S3-specific CFO-vs-temperature curve and holdover; (ii) whether
firmware preserves the raw intercept; (iii) per-channel PLL re-lock magnitude; (iv) mutual
coupling/shadowing between adjacent faces on a solid (an unmeasured item flagged by the
synthesis); (v) DD integer ambiguity resolution with a compact, near-degenerate constellation;
(vi) reciprocity asymmetric drift (2.7%/4 h claimed for another platform); (vii) wall reflection
coherence in real furnished rooms; (viii) the licence envelope for continuous ESP-NOW at 100–200 Hz
(duty is ~2–4%, within class licence — but verify against note 07's numbers).

**Chaining.** (A) Conducted coherence ⇒ wireless coherence export (M1) ⇒ DD-CSI geometry (M2) ⇒
coherent aperture (M4) ⇒ self-labelling (M5); M3 is the protocol spine. Blocked without M1: any
mesh-wide coherence. Blocked without M2: absolute geometry (bearings only). Blocked without M4:
"sharp" imaging (revert to incoherent RTI/fingerprinting).

**Existing models and data.** ESPARGOS datasets (four time- and phase-synchronous arrays in a lab
room; a passive-target bistatic dataset — directly usable for M4 offline testing before hardware);
channel-charting tutorials built on ESPARGOS (DiChasus); SenseFi/MM-Fi for transfer experiments;
802.11bf-2025 (published 26 September 2025) as the standards direction for sensing sounding (ESP32
does not implement bf; ESP-NOW NDP-like sounding is the pragmatic primitive).

**Own data / new technology.** The minimal new dataset is E1's residuals + E2's DD-CSI survey logs;
the minimal new hardware is the polyhedron hub face (clock + reference + trigger). The brief's
hardware premise is exactly the necessary and sufficient enabler: **a compact coherent body to
export coherence from.** If E1 passes, this is the invention; if E1 fails on S3, the invention
shrinks to the conducted polyhedron plus A2's opportunistic mesh — and the exact physical boundary
is `σ_φ,res(Δt)` with the beacon as the minimal crossing reference.

---

## 9. Sources (primary, with DOIs/URLs)

1. ESPARGOS hardware — "An Ultra Low-Cost, Realtime-Capable Multi-Antenna WiFi Channel Sounder",
   arXiv:2502.09405. https://arxiv.org/abs/2502.09405
2. ESPARGOS datasets — arXiv:2408.16377; DaRUS DOI 10.18419/DARUS-4973 (dataset 0007);
   https://espargos.net/datasets/
3. ESPARGOS WiFi camera (AR angular spectrum/delay/polarisation) — arXiv:2608.25996.
4. ESP-PPB (wireless phase-coherent CSI on ESP32-C3; FTM+VCTCXO; reference frame; unrefereed) —
   https://github.com/jonathanmuller/esp-ppb
5. WiDRa (sum-carrier-phase differential ranging, mm-level, COTS) — arXiv:2405.12168;
   IEEE TWC 2024, DOI 10.1109/TWC.2024.3409241 (approx.).
6. BeamSync (OTA phase sync, distributed massive MIMO; reciprocity drift 2.7%/4 h) —
   arXiv:2311.11070.
7. Distributed MIMO with OTA phase calibration integrated into the TDD flow (bidirectional
   measurements; Kalman tracking; common additive constant) — arXiv:2509.03722.
8. Locata terrestrial positioning: "Background and Recent Advances in the Locata Terrestrial
   Positioning and Timing Technology", Sensors 2019 — https://pmc.ncbi.nlm.nih.gov/articles/PMC6514591/
   ; LocataNet cm carrier-phase proof — 
   https://www.researchgate.net/publication/228954359
9. Indoor pseudolite carrier phase: Kao & Tsai, ION GPS/GNSS 2003 —
   https://www.ion.org/publications/abstract.cfm?articleID=5445
10. GNSS inverted radar / repealites: Samama et al. — 
    https://www.researchgate.net/publication/310497964
11. RIPS — Maróti et al., "Radio interferometric tracking of mobile wireless nodes", MobiSys 2007 —
    https://www.usenix.org/events/mobisys07/full_papers/p139.pdf ; SRIPS — SenSys 2011, DOI
    10.1145/2070942.2070954
12. RTK double differencing — Remondi, Navigation 1985, DOI 10.1002/j.2161-4296.1985.tb00918.x;
    "The Differences in Differencing", Inside GNSS 2011 —
    https://insidegnss.com/the-differences-in-differencing/
13. OFDM DD carrier phase (simulation; DD removes clock errors; ~0.3 m at 90%) — Sensors 21(20):6731,
    DOI 10.3390/s21206731; 5G-Advanced CP — arXiv:2209.01183.
14. Multipath-assisted localisation — Meissner & Witrisal, IWSSIP 2012,
    https://www2.spsc.tugraz.at/www-archive/downloads/MeissnerIWSSIP2012.pdf ; multipath-based SLAM
    — Leitinger et al., IEEE TWC 2019, DOI 10.1109/TWC.2019.2938162.
15. Radio-interferometry closure phase & self-calibration — Jennison 1958; Cornwell & Wilkinson
    1981; geometric view arXiv:2012.05254; NRAO VLA calibration notes —
    https://science.nrao.edu/facilities/vla/docs/manuals/obsguide/calibration
16. Wi-Fi holography — Holl & Reinhard, PRL 118, 183901 (2017), DOI
    10.1103/PhysRevLett.118.183901.
17. Coherent multistatic imaging and phase synchronisation in networked sensing — arXiv:2311.07212.
18. Time reversal / DORT — Prada & Fink, JASA 99:2067 (1996), DOI 10.1121/1.414696; RF
    super-resolution TR review, PIER 2025 — https://www.jpier.org/issues/volume.html?paper=25071806
19. Open-loop coherent distributed beamforming, wireless, SDRs (>90% coherent gain) —
    arXiv:2010.10396; real-time digital wireless time/freq/phase calibration (~60–70 ps) —
    https://www.osti.gov/pages/biblio/3020870
20. Spherical arrays survey (full-sphere, coupling challenges) — arXiv:2604.02701; geodesic dome
    phased array — URSI GA 2002 — https://www.ursi.org/proceedings/procGA02/papers/p1274.pdf ;
    3D-array CRB — DOI 10.1007/s11045-011-0160-5
21. esp-csi (phase behaviour, subcarrier ordering) — issues #121, #224;
    https://github.com/espressif/esp-csi ; Espressif CSI docs
    (https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/wifi-driver/wifi-vendor-features.html);
    Wi-ESP (usable phase; DOI 10.1093/jcde/qwaa048)
22. IEEE 802.11bf-2025 (published 26 Sep 2025) —
    https://standards.ieee.org/ieee/802.11bf/11574 ; overview arXiv:2310.17661.
23. KrakenSDR (coherent RTL-SDR array; ESPARGOS's stated inspiration) — https://www.krakenrf.com/
24. Contemporaneous convergence (unrefereed): RuView ADR-042 "Coherent Human Channel Imaging"
    (coax-distributed 40 MHz + 2.4/5 GHz phase reference; bf-style NDP sounding) —
    https://github.com/ruvnet/RuView/discussions/165
25. Synthesis inputs: `docs/research/v2/00-synthesis.md` and notes 07–20 (for TX legality,
    timestamps, TDM constraints, RTI, channel charting, through-wall, raw I/Q).

---

## 10. Bottom line

The invention is not a better estimator; it is an *architectural* move: **make one cheap coherent
body and export its coherence; let physics (double differencing) do the calibration; then spend
the coherence on sensing.** The polyhedron is exactly the minimal such body, and the home mesh is
exactly the right client: many cheap receivers, one beacon, no house-wide cabling. The space is
gated by a single measurable number — the residual tracked phase `σ_φ,res(Δt)` on S3 — and E1
measures it with three devkits and a splitter. Everything else (geometry, sharpness, labels) is a
consequence if that number is small, and a boundary if it is not.
