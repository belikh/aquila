# 22 — The invention: coherence-locality

**Prompt.** `22-invention-p`. As inventor-researcher: discover the new invention
that makes a home full of cheap ESP32-S3 nodes behave as one sharp,
self-calibrating, generalising 3D sensor — **or** prove that no such invention
exists and name the exact physical boundary that blocks it. Hardware premise: a
~30 cm soldered-PCB polyhedron (≈20 triangular faces, one ESP32-S3 per face) with
a hub face carrying a shared 40 MHz clock fan-out, a conducted 2.4 GHz phase
reference, and data aggregation (Ethernet switch or FPGA).

Australian English. Every claim labelled **ESTABLISHED / HYPOTHESIS /
SPECULATION** and tiered **ESP32-S3 / ESP32-family / other COTS / SDR-grade**.
Sources are primaries (papers, standards, upstream code, datasheets); DOIs/URLs
in §13. The v2 corpus (`00-synthesis.md`, notes `01`–`20`) is used as prior
context and its vocabulary is kept (`CONTEXT.md`).

---

## 0. Answer in one page

**An invention exists — and one specific hope is provably closed.**

**Closed.** A *single house-wide carrier-coherent aperture*, built wirelessly
from independent ESP32-class LOs with no house-wide cabling, is closed by the
**phase-accrual inequality**: to keep the cross-node phase error below
`ε` radians across a re-measurement gap `T`, the **post-tracking residual
relative frequency** must satisfy `2π·Δf_res·T < ε`. With `ε = 0.1 rad` and an
ESP32 half-duplex schedule gap `T ≈ 3 ms`, that is **`Δf_res < 5 Hz` — i.e.
≈2 ppb at 2.44 GHz**, four orders of magnitude better than free-running
crystals (±10–20 ppm). The only known way to cross it is a *reference*: a
conducted clock+tone makes `Δf_res` identically zero; a per-frame wireless
common-transmitter pilot makes the *gap* small but pays airtime and — the
physical killer — is **multipath-biased in exactly the moments sensing matters**
(a person moving). *(ESTABLISHED arithmetic; the residual input is the
unmeasured M2 of `10/17`.)*

**Open.** Home-scale coherent *sensing* needs no house-wide coherence at all,
because every observable the sensor needs across apertures is **gauge-invariant
or per-aperture-local**. The invention is the architecture that splits the
problem accordingly — **coherence-locality**:

> Buy hard carrier coherence only where it is cheap — inside one polyhedron,
> where one wire fixes it exactly and durably — and bind many such apertures
> with gauge-invariant wireless geometry: two-way phase *ranges*, per-aperture
> *bearings*, and the home's own static scene as a *fiducial* for self-
> calibration. Sharpness comes from multiplying local coherence (20-element,
> sub-degree 3D bearings) with global geometry (cm-precise two-way ranges and
> scene registration), not from one giant phased array.

The three constituent inventions — the **Puck** (a 20-face coherent phase
camera), the **Handshake** (a ping/unique-reply two-way protocol that yields
ranges, fingerprints and obstruction data without any common clock), and the
**Fiducial** (pose self-recovery by registering the static room) — are specified
in §3 with their novelty, mechanism, preconditions, falsifiers, feasibility,
cost and TX requirement.

**The single decisive experiment (E-FIDUCIAL).** Two unroved, un-surveyed pucks
in a furnished room, running for ≥1 h across a ≥5 °C swing, must recover their
relative pose (position + orientation) purely from two-way ranges plus static-
scene registration, validated to <30 cm / <10° against a tape measure. This one
test simultaneously gates conducted-coherence durability, ESP32 two-way phase
ranging, static-scene observability, and the fiducial thesis. §7.

---

## 1. Method

Two files read in full (`BRIEF.md`, `00-synthesis.md`); the four other named
research directions (2, 4, 5, 6) were searched alongside direction 1 with
`parallel-search` → `parallel-fetch`, following citations to primaries. The most
decision-relevant primaries were fetched in full: the ESPARGOS hardware paper
and its multi-array/radar documentation (the only ESP32-family demonstration of
conducted phase coherence), Chronos (multi-band carrier-phase ToF on commodity
Wi-Fi), the round-trip carrier-phase ranging CRLB analysis (precision and
multipath bounds), the TDD-flow over-the-air phase-calibration paper (phase-
noise/Kalman treatment of cross-LO drift), multipath-assisted single-anchor
localisation (virtual anchors / Channel-SLAM), switched/single-RF-chain array
antennas, RF neural fields (NeRF², RF-3DGS, Sionna RT), and the IEEE 802.11bf
standard record. Numerical CRBs in §5 were recomputed from standard forms for
this hardware; they are marked as derivations.

---

## 2. The boundary — the exact quantity that closes the strongest form

### 2.1 What is already closed (carrying note `20` forward)

**ESTABLISHED.** With independent LOs the per-subcarrier observation is

```
ĥ_ij(k) = a_i·b_j·g_ij(k)·e^{j(θ_i − ψ_j)}·e^{−j2π f_k τ_ij},
```

so only `θ_i − ψ_j` is observable and the node phases are fixed only up to one
global `U(1)` constant per connected measurement graph (`10`, `20`; Rogalin et
al. arXiv:1310.7001; arXiv:2606.14419). Absolute cross-node phase and one-way
phase-of-flight are not observable; amplitude, per-link subcarrier differential
phase (delay), per-receiver inter-antenna differential phase (bearing), two-way
products, and temporal differentials (Doppler) are the invariants. One common-
transmitter broadcast, one reciprocal round trip per link, or one shared
clock+tone fixes the gauge — `O(N)` cost.

### 2.2 The new quantitative boundary: the phase-accrual inequality

**ESTABLISHED (arithmetic), input HYPOTHESIS (M2).** Consider three ways to hold
carrier phase across *independent* clocks, at ESP32 frame cadence:

1. **Wireless common transmitter** (AirSync-class: a master broadcasts a pilot;
   every node measures its LO phase against it per pilot frame, then
   *extrapolates* to its next transmit/receive instant). AirSync reached
   σ ≈ 2.4° across two WARP APs in an office with an FPGA and a dedicated pilot
   path (arXiv:1205.6862; DOI 10.1109/TNET.2012.2230449). ESP32-class hardware
   must receive the pilot on its single half-duplex radio.
2. **Two-way products per pair**: these do not track phase at all — they
   *cancel* LO phase per exchange. Residual error is the accrued relative phase
   `2π·Δf_res·T_exchange` plus multipath bias on both directions (Chronos shows
   the cancellation mechanism on an Intel 5300; arXiv:1505.03446 §7).
3. **Conducted reference**: `Δf_res = 0` by construction; per-lock phase offsets
   are static and measured once (ESPARGOS, arXiv:2502.09405).

Case 2 is *immaterial of drift over long spans* but accrues error over the
exchange; cases 1 and 3 need re-measurement/extrapolation. All three collapse
to the same inequality for the *gap* between a phase sample and its use:

```
phase-accrual inequality:   2π · Δf_res · T_gap  ≪  1 rad
```

**Numbers (derivation from §2.1 and standard ppm figures).** ±10–20 ppm at
2.44 GHz is `Δf ≈ 24–48 kHz`. After per-frame CFO estimation/tracking the
residual `Δf_res` is the unmeasured M2 of `10`/`17`; plausible values are
10–100 Hz. Then:

| Gap `T_gap` | `Δf_res` needed for 0.1 rad | What it means |
| --- | --- | --- |
| 3 ms (one ESP32 TDM slot cycle) | ≤ 5 Hz (≈ 2 ppb) | A wire; or a 2 ppb-class reference |
| 1 ms | ≤ 16 Hz | Aggressive wireless tracking, high pilot duty |
| 100 µs | ≤ 160 Hz | Only within a single duplex-exchange window |

**So the closed quantity is not "phase coherence" but the product
`Δf_res × T_gap`.** A house-wide coherent aperture on ESP32-class silicon is
closed because the half-duplex MAC cannot push `T_gap` small enough while
sensing, and the wireless escapes that shrink it (frequent common-transmitter
pilots) are (a) airtime-expensive and (b) **multipath-biased**: the pilot's
channel to each node adds an unknown phase that changes when things move —
AirSync's 2.4° was in a static office with a dedicated pilot antenna; a person
crossing a home changes the pilot multipath by large fractions of a radian on
every node *simultaneously*, corrupting the aperture exactly when it has
something to see. *(HYPOTHESIS: magnitude in homes unmeasured; falsifiable by
the pilot-bias experiment in §7.)*

**ESTABLISHED contrast:** *within* one polyhedron, the same inequality is
satisfied identically. Integer-N-class PLLs fed from one 40 MHz reference are
frequency-locked, so relative phase is constant between locks; ESPARGOS
demonstrates that frequency-locking alone gives long-term phase stability, with
the conducted 2.4 GHz reference resolving the per-lock PLL phase roll
(arXiv:2502.09405, "Source of Phase Uncertainty"; espargos.net/docs). The 20
faces of the premise hardware are therefore *one coherent 30 cm aperture* —
not by tracking, but by physics.

**The minimal reference that crosses the closed space:** one shared clock+tone
*per aperture* (the polyhedron hub). Cost O(1) wire per puck, zero house-wide
cabling. This is the premise's hub face, and it is why the premise hardware is
the right one.

---

## 3. The invention — coherence-locality

**Definition.** *Three composable mechanisms whose union makes N cheap
independent radios one sharp, self-calibrating, generalising 3D sensor without
any house-wide coherence.*

The system is named here **AQUILA LATTICE** (working name). Components:

- **Puck** — a phase camera: one coherent 3D aperture (the premise polyhedron).
- **Handshake** — a scheduled ping + unique-reply protocol whose two-way
  products yield ranges, hardware fingerprints and obstruction data between
  pucks, with **no carrier phase** and **no common clock** across pucks.
- **Fiducial** — the static home itself as the registration target that puts
  all pucks into one frame (position *and* orientation), and keeps them there.

### 3.1 Invention N1 — the Puck: a 20-face coherent phase camera with phase-known illumination

**What is genuinely new.** (a) A *volumetric* (icosahedral/geodesic) coherent
aperture built from commodity single-chain radios made coherent by one shared
40 MHz clock plus one conducted in-band phase reference — ESPARGOS demonstrated
the coherence *mechanism* on ESP32-S2 in a **planar 2×4** array, receive-only
(arXiv:2502.09405; ESPARGOS One: 8 dual-polarised patches, λ/2, 20/40 MHz,
PoE; Aperture Kits pre-wired 6×4/8×4 — espargos.net). No ESP32 volumetric
sensing array exists (`00-synthesis` §7). (b) **Phase-known illumination**: on
ESP32 there is no transmit phase control (`esp_wifi_80211_tx` exposes no phase
knob; no TX timestamp — `17`), so coherent *transmit beamforming* is
impossible — but *sensing does not need it*. Each face's transmit phase is
**measured, not steered**: the hub reference ledger fixes each face's LO phase
per lock, and a conducted PA→LNA loopback (or the distribution network in
reverse) fixes each face's TX+RX chain phase per channel; the receiver side
then compensates known phases in processing. *(This inversion — "phase-known,
not phase-controlled" — is the design principle that makes bistatic imaging
possible on chips with no phase shifters.)*

**Physical mechanism.** With per-lock offsets known, the 20 faces are one
digital array: bearings from superresolution (MUSIC/ESPRIT-class, 3D manifold,
full-sphere coverage, no scan loss — spherical-array theory, §13). All faces
receive **simultaneously** (one radio per face): there is no receive-time
switching. Illumination is TDM across faces (one ESP-NOW HT frame per face per
slot); every other face receives it. The station thus performs
`20 × 19` coherent bistatic measurements per cycle with *known* probe phases —
a coherent multistatic radar in a 30 cm ball. Static scatterers are visible
(unlike differential CSI, `13`): every path's two-way phase is bookkept.

**What must be true.** (i) ESP32-S3 accepts an external 40 MHz clock (S3 clock
tree exposes `SOC_ROOT_CLK_EXT_XTAL` "External 40MHz crystal" — ESP-IDF clock
tree, URL in §13; ESPARGOS does this on S2; Espressif's own esp-csi multi-
antenna board connects a C3 and an S3 through a clock buffer "to eliminate the
relative frequency offset" — ESP-CSI solution page). *(HYPOTHESIS on S3
specifically; test in §7-E0.)* (ii) Per-lock phase offsets re-roll only at
boot/channel change, not per frame (ESPARGOS docs: offsets "re-roll whenever
the receivers re-lock"; per-channel ledger entries required). (iii) Thermal
drift of offsets over hours is small (ESPARGOS multi-array coherent datasets
exist: espargos-0005, four arrays phase-synchronised in a lab room; DaRUS
DOI 10.18419/DARUS-4973 dataset family — *(HYPOTHESIS for S3/3D; quantify).*
(iv) Mutual coupling and face shadowing in a *conducting* polyhedron do not
destroy the manifold (SPECTULATION: coupling is fixed and calibratable à la
spherical-array coupling work — MDPI Appl. Sci. 12(19):10099; worst case the
effective aperture shrinks; must be measured).

**Falsifying experiment.** E0 (§7): a 4-face puck, external TCXO, phase-stable
behind a cabled splitter: (a) boot-to-boot PLL phase roll distribution;
(b) offset drift over 8 h with a 10 °C swing; (c) coherent combining gain
across 4 faces within 0.5 dB of 6 dB over hours. Falsified if offsets wander
>0.3 rad over an hour or re-roll mid-operation.

**ESP32-S3 feasibility.** ESTABLISHED at mechanism level on ESP32-family (S2),
HYPOTHESIS on S3; S3 adds CSI HT20/40 (ESP-IDF Wi-Fi doc), a 16-way antenna-
switch API (same doc; a *single-radio* 16-face switched variant is therefore
possible — cheaper, common LO by construction, TDM per face; prototype tier 0),
ns RX timestamp (`17`). No S3 TX phase control — accommodated by N1(b).

**Cost.** Per puck, prototype scale: 20× S3 module ≈ AU$60, RF switches/
passives ≈ AU$20, 20 triangular 4-layer PCBs ≈ AU$50, hub (TCXO, 2.4 GHz synth,
resistive splitter, Ethernet/PoE PHY, MCU) ≈ AU$25, assembly ≈ AU$40 ⇒
**≈ AU$200/puck**; 6 pucks ≈ AU$1.2 k. Single-radio switched variant (one S3 +
16-way switch): **≈ AU$30**. Off-the-shelf comparison: ESPARGOS One Core Kit
(8-antenna planar, no TX) — espargos.net.

**TX needed?** For mode (a) *passive* (sniff ambient APs, per-puck coherent):
no. For mode (b) *own-probe* bistatic imaging and for N2/N3: yes — plain,
legal ESP-NOW/802.11 TX (`07`), no phase control required.

### 3.2 Invention N2 — the Handshake: two-way products as *range + fingerprint + obstruction*, with no common clock

**What is genuinely new.** The *combination* of (i) the two-way product/
ratio identity (`ĥ_AB·ĥ_BA` cancels LO phase; `ĥ_AB/ĥ_BA` exposes chain
asymmetry — `20` §4), (ii) per-node conducted loopback calibration of the
TX·RX chain term at each channel, and (iii) multi-channel unwrapping — enabling
**cm-precision two-way carrier-phase ranging on ESP32 CSI**. The mechanism is
established elsewhere: Chronos measured sub-nanosecond ToF on an Intel 5300 by
hopping 2.4+5 GHz bands and *adding TX-side and RX-side CSI phases to cancel
CFO* (arXiv:1505.03446 §7: the two-way product is Chronos's own CFO cure);
multi-band splicing work formalises the per-band phase distortion/ambiguity
resolving (arXiv:2011.10402; arXiv:1903.02367); Bluetooth Channel Sounding
ships Phase-Based Ranging in commodity silicon (Bluetooth 6.0, "phase
correction terms", RTT + PBR — NXP/Silicon Labs/IEEE 802.11-adjacent material
in §13); the DSSS DMR system demonstrates <1 mm stand-alone round-trip
carrier-phase precision (CRLB 27 µm at 5.8 GHz, SNR 10 dB), 1 cm in-system,
"does not require a priori synchronisation nor high-performance oscillators"
(GPS World / carrier-phase RF ranging, §13). **New here: the ESP32 realisation,
and the *conducted ledger* that removes the handshake gymnastics Chronos needs**
— the per-lock per-channel phase constants are measured, not statistically
cancelled, because the puck hub exists.

**Physical mechanism.** Scheduled ping: puck P broadcasts an ESP-NOW HT20/40
MCS0 frame on channel `c`; every other puck receives (all faces). Unique
replies: each puck Q replies in its own slot with a unique frame (identity,
ordering, collision-free). Two-way products: both ends compute
`ĥ_PQ(k)·ĥ_QP(k)` per subcarrier `k`. LO terms cancel; with each node's chain
constant `x_n(f) = ∠(TX_n·RX_n)(f)` fixed by loopback calibration, the residual
is propagation only:

```
∠ (ĥ_PQ(k)·ĥ_QP(k))  =  −4π f_k τ  +  x_P(f_k) + x_Q(f_k)      (calibrated out)
⇒  τ = L_PQ / c    recoverable from inter-channel phase differences:
    d_max = c / (2·Δf)   (7.5 m at Δf = 20 MHz; 30 m at 5 MHz)
    σ_d  = c·σ_φ / (4π f)  ≈ 1 cm per radian of phase noise at 2.44 GHz
```

Reported to the lattice: the range `L_PQ`, the two-way attenuation (obstruction
map, chaining to RTI — `02`), the Doppler (temporal differential, `01`), and the
**chain-asymmetry fingerprint** — `ĥ_PQ/ĥ_QP` is channel-independent (wireless
reciprocity cancels `g_ij`), stable (hardware coefficients drift <2.6 % over
4 h — `10`), node-unique: a physical identity token and a calibration-staleness
monitor.

**What must be true.** (i) The exchange must sit inside the *residual*
coherence window per 2.2 — ESP32 MAC turnaround is milliseconds (`09`,`17`);
with `Δf_res ≈ 10 Hz` and `T_exchange ≈ 3 ms`, accrued error ≈ 0.19 rad ≈
2 mm — fine; with 100 Hz residual, ≈1.9 cm — still useful. *(HYPOTHESIS: M2
unmeasured; §7-E1 measures it.)* (ii) ESP32's per-subcarrier CSI phase is
usable in a *product* after the per-node calibration — OpenCSI's finding that
"absolute ESP32 phase unusable" concerns one-way absolute phase, which this
design never uses (`20` H2; arXiv:2607.26665). (iii) Packet-detection/sampling
delay and SFO contribute a per-(node, rate, channel) linear-in-frequency term;
it is static and folded into `x_n(f)` by loopback calibration with a known
cable. (Chronos handles the in-band part by interpolation to the zero
subcarrier — arXiv:1505.03446 §5, a usable cross-check.) (iv) 2.4 GHz ISM
(83.5 MHz) gives 1.8 m two-way delay resolution after channel stitching —
enough to beam/angle-separate, not to resolve every wall; S3 has no 5 GHz
(ESP32-C5 faces would add ~500 MHz — HYPOTHESIS tier).

**Falsifying experiment.** E1 (§7): two S3s with conducted-loopback couplers,
ping/reply over channels 1…13, ranges vs laser/tape over 1–20 m, for 1 h with
temperature logging. Success: ≤3 cm RMS, bias stable ≤1 cm/h. Falsified if the
two-way product phase wanders >0.5 rad (≈5 cm) over minutes, or if measured
ranges swing with channel/rate nominally corrected by the loopback.

**ESP32-S3 feasibility.** ESP-NOW HT40 MCS0 is the esp-csi excitation primitive
(`00-synthesis` §8); CSI HT20/40 exists on S3; ns RX timestamps exist (`17`);
the mechanism is demonstrated on other COTS (Chronos, Intel 5300; BLE CS in
silicon); the ESP32-specific step (loopback-calibrated two-way phase) is
undemonstrated — that *is* the experiment. **TX needed: yes** (this is the
handshake), legal under `07`.

**Cost.** Negligible beyond the radios: one RF coupler per face (≈ AU$1) for
loopback. The protocol is firmware.

### 3.3 Invention N3 — the Fiducial: the home as its own calibration target

**What is genuinely new.** Self-estimation of the *pose* (3D position **and**
orientation) of *independently clocked* coherent apertures by registering what
they coherently see of the *static* room, with no survey, anchors, external
infrastructure, or home-specific training. Adjacent art: multipath-assisted
single-anchor localisation / virtual anchors (Meissner & Witrisal 2012, 2.5 cm
P90 with 2 GHz *UWB* bandwidth; Channel-SLAM, IEEE TWC 2016, DOI
10.1109/TWC.2016.2578336 — but these localise a *moving agent against one
anchor* and need UWB delay resolution); automotive radar-network self-
calibration (Grebner et al. 2023) and multistatic self-calibration (Musallam
et al. 2026) — but engineered, surveyed-ish scenes and no phase-coherent home
apertures; ESPARGOS passive channel charting (arXiv:2504.09924) — label-free
but coordinate-free. **New: RF pose self-calibration across cheap coherent
home apertures, using the scene's *static* coherent returns as common features.**

**Physical mechanism.** Stage 1 (within a puck, TDM, phases known): each
bistatic face pair `(i,j)` measures composite path lengths `L_ij(m)` for the
static multipath components `m`, separated in the *angle domain* (the 20-face
aperture's superresolution; at 40 MHz delay separation is weak, so angle does
the work) and unwrapped across channels (N2). With known face geometry (PCB
build tolerances) each static scatterer is localised in the puck frame by
angle/angle/length. The puck accumulates a sparse 3D point set of the room
(corners, wall speculars, furniture edges) — a blurry, coarse "RF LiDAR scan".
Stage 2 (across pucks, no phase needed): two pucks' point sets of the *same
room* are registered (RANSAC + ICP-class over feature tuples, with the
handshake ranges as initialisation and as a metric prior); the solved SE(3)
transforms put all pucks in one frame. Stage 3 (targets): a person is a
*changing* scatterer visible coherently to every puck; triangulation across
pucks yields 3D position — standard multilateration, now with sub-degree
bearings and cm ranges. Cross-check map: `me` (§13).

**What must be true.** (i) Home static scenes contain registrable coherent
features at 2.4 GHz/83 MHz: wall speculars are geometrically stable; UWB
studies find speculars explain up to ~90 % of CIR energy with a floor plan
(Meissner/Witrisal) — at 2.4 GHz homes we expect fewer, coarser features
(vs SPECULATION: feature-poor/open-plan rooms register worse; fall back to
range-only MDS + channel-charting priors). (ii) Two pucks see enough of the
same room (overlap of their spheres-of-view) — trivially true in homes.
(iii) The puck's per-face phase calibration remains valid for hours (N1; E0).

**Falsifying experiment.** E-FIDUCIAL (§7, the decisive one).

**ESP32-S3 feasibility.** Same radio facts as N1/N2. Host compute: pose-graph/
registration runs off-node (hub FPGA or home server); ESP-DL INT8 (~18× SIMD)
exists for on-node inference if needed (`19`).

**Cost.** Zero marginal hardware.

### 3.4 The learned assistant layer (only where learning transfers)

**HYPOTHESIS.** Keep the *geometry* model-based (that is what makes the system
generalise across homes by construction); use learning only for (i) data
association/feature matching between RF point sets, where a small model trained
on synthetic panoramas (differentiable ray tracing: Sionna RT, DOI-grade refs
in §13; RF-3DGS arXiv:2411.19420; NeRF² MobiCom'23
DOI 10.1145/3570361.3592527; RF inverse rendering arXiv:2604.07086) transfers
because wall/furniture statistics are shared across homes, and (ii) RF point
denoising. Evidence for transfer: Sim2Real reached 82.1 % F1 with 20 % real
labels in the `19` corpus. Channel charting (`03`) is the learned *fallback*
for coarse relative geometry when scene features are weak — it needs no labels
and no phase.

---

## 4. Sharpness budget — the "one sharp 3D sensor" arithmetic

All numbers derivations from standard CRBs at 2.44 GHz, 30 cm puck, CSI-quality
SNR (per-element 10–20 dB):

| Quantity | Mechanism | Value | Tier |
| --- | --- | --- | --- |
| Bearing (3D, per puck) | 20-element coherent aperture, superresolution | σ ≈ 0.1–0.3° (SNR 20–10 dB); conventional Rayleigh ~23° | ESTABLISHED theory; HYPOTHESIS on S3 arrays |
| Range (puck↔puck) | two-way phase, multi-channel unwrap | σ ≈ 1 mm at 0.1 rad phase noise; ~1–10 cm realistic | mechanism ESTABLISHED on other COTS |
| Range resolution (imaging) | 83.5 MHz coherent stitching (two-way ⇒ ×2) | ≈1.8 m (S3); ≈0.5 m with C5 5 GHz faces | derived; HYPOTHESIS |
| Cross-puck pose | N2 ranges + N3 registration | cm–dm position, few degrees orientation | HYPOTHESIS (E-FIDUCIAL) |
| Target 3D | bearing×bearing×range fusion across N pucks | cm–dm indoors | HYPOTHESIS |
| Static/dynamic separation | coherence (not differencing): static visible | full static scene visible | ESTABLISHED contrast with `13` |
| Multipath bias on ranges | two-beam bound ±90° (α≤1); ±6° at α=0.1 (≈1 mm at 2.4 GHz) | mitigated by beamspace prefiltering | ESTABLISHED bound (GPS World analysis) |

The composite claim: **sub-degree bearings × cm ranges × scene registration ⇒
cm-class 3D in a real home, from ~AU$200 apertures, with no house-wide wiring
and no surveyed node positions.** The burden of proof is not the angular CRB
(standard) but *feature observability and phase durability* — E0/E1/E-FIDUCIAL.

---

## 5. What the round-trip protocol recovers — and where it breaks

Additional to `20` §"round-trip scheme" (range, hardware asymmetry, Doppler,
identity, channel): with the conducted ledger, the same exchanges also recover
**absolute two-way path lengths to static scatterers** (bistatic imaging, §3.3)
and **channel-coherent wideband stitching** (multi-channel phase aggregation,
turning five 20 MHz channels into one ~83 MHz synthetic aperture). Breaks:
turnaround drift (2.2; quantified); multipath bias (α-dependent; bounded
±90°); λ/2 ambiguity (unwrapped by ≥2 well-separated channels — 7.5 m at
20 MHz spacing, 30 m at 5 MHz); the *composite-channel product* subtlety (for
scatterers, `(direct+wall)²` cross-terms require angle-domain path separation
first); quantisation of ESP32 CSI (8-bit-class I/Q ⇒ phase quantisation at
high SNR; averaging `∝ 1/√(2 N_s ρ)` — `20` §7); mobility breaking reciprocity
between the two directions (both direction measurements must bracket the motion
within one coherence interval — conservatively: exchanges <10 ms).

---

## 6. Related art and novelty positioning (what is *not* new)

| Prior | What it has | What LATTICE adds |
| --- | --- | --- |
| ESPARGOS (arXiv:2502.09405; One/Aperture Kits — espargos.net) | Conducted 40 MHz + 2.4 GHz phase ref on ESP32-S2; phase-coherent *planar* 2×4; RX-only; multi-array needs a wired star; radar TX timestamp correction in tooling; AR "WiFi camera" (arXiv:2608.25996) shows angular spectrum/delay/polarisation | 3D volumetric aperture; TX/multistatic TDM; **wireless** cross-aperture binding; pose self-calibration; phase-known illumination principle |
| Chronos (arXiv:1505.03446), ToneTrack (MobiCom'15), multi-band splicing (arXiv:2011.10402) | Multi-band carrier-phase ToF on other COTS; two-way CSI addition cancels CFO; per-band PLL roll handled statistically/with handshakes | Per-node conducted ledger makes per-lock phase *measured*, removed from the statistics; ESP32 realisation; ranging as a sensor-scale primitive |
| AirSync (arXiv:1205.6862), AirShare (INFOCOM'15), BeamSync (arXiv:2311.xxxxx, IEEE TWC 23(7) 2024), OTA-TDD-flow (arXiv:2509.03722), consensus+Kalman arrays (Rashid & Nanzer, IEEE TWC 22(4) 2023) | Wireless cross-LO phase calibration on SDR/FPGA; Kalman tracking vs phase noise (−120…−80 dBc/Hz); star topology CRB-optimal | The 2.2 boundary: why this is not the primary coherence path for sensing (airtime + multipath bias + 2 ppb-class requirement); two-way *ranges* instead |
| Channel-SLAM (DOI 10.1109/TWC.2016.2578336), SALMA/Meissner-Witrisal, multipath-assisted positioning | Virtual anchors from static reflections; cm–dm accuracy — with UWB (GHz-class) bandwidth, one anchor, moving agent | Scene-fiducial *pose self-calibration between apertures*; angle-domain (not delay-domain) feature separation; 2.4 GHz/83 MHz commodity constraints |
| Radar-network self-calibration (Grebner 2023; Musallam 2026) | Extrinsic self-calibration of radar networks from shared scene | Home-scale, phase-coherent apertures, commodity chips, un-surveyed, wireless binding |
| NeRF²/RF-3DGS/Sionna RT/RFIR | Learned/differentiable RF scene models; need posed measurements | Uses fields *for* association only; poses solved geometrically by the sensor itself |
| 802.11bf-2025 (standards.ieee.org 11574) | Standards framework for WLAN sensing | S3 does not implement bf (no HE/EHT); LATTICE is an independent experimental stack — note the compatibility path for future chips |

---

## 7. Experiments — the ladder, and the single decisive one

**E0 — S3 coherence gate (days).** 4-face puck, external 40 MHz TCXO (S3
`EXT_XTAL` root), cabled/split reference port: per-lock PLL phase roll, offset
drift 8 h with 10 °C swing, coherent combining gain across faces. *Gates N1.*

**E1 — two-way ranging gate (weeks).** Two S3s + couplers; ping/reply
channels 1–13; ranges vs tape 1–20 m over 1 h; M2 residual extracted; compare
LEDGER-corrected vs Chronos-style handshake-only. *Gates N2.*

**E2 — multipath bias / pilot-bias probe.** In one room: (a) two-way phase
range vs ground truth with a person moving (range bias distribution);
(b) a third node as common-transmitter pilot; phase-tracking error vs person
position (tests §2.2's multipath-bias claim). *Gates the boundary statement.*

**E-FIDUCIAL — the single decisive experiment.** Two pucks (prototype tier:
two 4–8-face S3 apertures with the hub reference; either built or ESPARGOS-
class hardware with TX added), placed **unmoved and un-surveyed** in a
furnished room with ≥2 large walls, running the full stack for **≥1 h across a
≥5 °C temperature swing**. Steps: (1) handshake ranges between pucks vs tape;
(2) each puck's coherent static-scene measurement set (bearing/angle + unwrapped
length of its dominant static multipath); (3) solve relative pose from (1)+(2)
only. **Pass:** position error <30 cm, orientation <10°, stable over the hour
(result may refine with E0/E1 optimisations; the pass line is deliberately
generous). **Falsified if:** phase offsets drift beyond correction (E0-style
failure at 3D scale), static features are too sparse/unstable to register, or
ranges+registration give inconsistent poses. *This one experiment exercises
every pillar of the invention at once.*

**Ordering:** E0 → E1 → E-FIDUCIAL, with E2 inserted anywhere after E1;
cheapest decisive first.

---

## 8. ESP32-S3 feasibility — consolidated

| Item | Status | Note |
| --- | --- | --- |
| External 40 MHz clock | HYPOTHESIS on S3 (ESTABLISHED on S2; Espressif clock-buffers C3↔S3 on its own esp-csi board) | ESP-IDF clock tree `EXT_XTAL`; test in E0 |
| CSI HT20/40 | ESTABLISHED (ESP-IDF Wi-Fi doc; esp-csi) | 2.4 GHz only on S3 |
| ESP-NOW HT40 MCS0 excitation | ESTABLISHED (esp-csi default) | raw `esp_wifi_80211_tx` default 1 Mbps 11b has no CSI — `00-synthesis` §8, `07` |
| ns RX timestamp | ESTABLISHED (`17`) | not the ranging primitive; phase ranging bypasses timestamps |
| TX phase control / TX timestamps | ABSENT | N1(b): phase-known, not phase-controlled illumination; ranges are phase-based, so no TX timestamps needed |
| Antenna switching | ESTABLISHED API (16-way on S3, ESP-IDF Wi-Fi doc) | enables the AU$30 single-radio switched prototype tier |
| 5 GHz | ABSENT on S3 | dual-band C5 faces are the bandwidth upgrade path (CSI supported per Espressif's chip ranking C5 > C6 > C3 ≈ S3) |
| Phase coherence within puck | Mechanism ESTABLISHED on ESP32-family (ESPARGOS); not on S3 in 3D | E0 |
| Cross-puck two-way phase on ESP32 | UNDEMONSTRATED | E1 — the riskiest fabrication-independent unknown |
| On-node compute | CSI streaming to host is the demonstrated pattern (ESPARGOS); ESP-DL exists (`19`) | aggregation via hub Ethernet/FPGA per premise |

---

## 9. Cost summary

- **Tier 0 (bench):** 2× devkit S3 + couplers ≈ AU$60. E1, E2.
- **Tier 1 (4–8 face aperture):** ≈ AU$100–150 per puck. E0 + one side of
  E-FIDUCIAL.
- **Tier 2 (premise hardware, 20 faces + hub):** ≈ AU$200/puck at modest
  volume; six-puck home ≈ AU$1.2 k + install. Cheaper than one ESPARGOS
  Aperture Kit-class product per room; two to three orders below SDR arrays.

## 10. Transmit needed? — consolidated

- Passive mode (sniff existing WiFi, per-puck coherence only): **no TX**.
- Handshake/ranging/self-survey (N2), own-probe bistatic imaging (N1),
  fiducial registration (N3): **yes — plain legal ESP-NOW/802.11 TX**; duty is
  low (sounding bursts); no phase control, no beamforming.

## 11. Chaining / existing models & data / own-data

**(A) Chaining.** Conducted hub reference ⇒ per-puck coherence ⇒ 3D bearing +
static-scene visibility ⇒ (with N2 ranges) self-survey ⇒ (with N3) pose graph
⇒ target 3D and coherent tomography. Without TX: N1 passive and channel
charting still stand; N2/N3 blocked. Without the hub reference: everything
coherent in 3D is blocked; the corpus's Track-2 (receive-only learned) route is
unaffected.

**(B) Existing models/data.** ESPARGOS datasets (DaRUS; espargos-0005/0007 —
the latter explicitly recorded for bistatic/passive radar with four
phase-synchronised arrays and ceiling transmitters: the closest public template
for E-FIDUCIAL), DICHASUS, MM-Fi/SenseFi/CSI-Bench/OpenCSI (`19` §5), Sionna RT
(calibration/simulation), RF-3DGS/NeRF² (association model pre-training),
channel-charting code (ESPARGOS-WiFi-ChannelCharting, GitHub). Licences per
dataset; ESPARGOS datasets are for research use.

**(C) Own-data / new technology.** New datasets required: (i) S3 per-lock phase
roll and thermal drift (3D, hub star) — nothing public; (ii) two-way CSI
product ranging on ESP32 (E1) — nothing public on S3; (iii) home static-scene
coherent panoramas across multiple pucks with ground-truth poses (E-FIDUCIAL) —
nothing public. Each dataset is exactly one of the experiments above; success/
falsification criteria are stated there.

## 12. Evidence strength

**7 / 10 overall.** Justification: the boundary arithmetic is solid
(ESTABLISHED gauge theory + standard ppm figures; 9/10 within its assumptions);
the constituting mechanisms are each ESTABLISHED on adjacent hardware
(conducted coherence on ESP32-S2; two-way phase ranging on other COTS and in
BLE silicon; multipath-anchor SLAM in UWB; switched arrays commodity), and the
premise hardware is feasible (external clock root, HT40 CSI, ESP-NOW, antenna
switch — all documented). The deduction loses points on three unmeasured
quantities: S3-specific per-lock/thermal phase stability (E0), ESP32 two-way
phase fidelity (E1), and home static-feature observability (E-FIDUCIAL) — pre-
cisely the unmeasured M2-adjacent numbers the v2 synthesis flagged as decisive.
The novelty is a composition whose parts are individually prior art — strength
comes from the composition's necessity (the boundary) and its testability.

## 13. Primary sources (DOIs/URLs)

1. **ESPARGOS** — channel sounder: Euchner & ten Brink, arXiv:2502.09405
   (10.48550/arXiv.2502.09405). Datasets paper: arXiv:2408.16377. Docs:
   https://espargos.net/docs/ (combined-arrays; sensor_calibration; radar
   modules — URL fetched 2026-10). Product/datasheet:
   https://espargos.net/espargos-one/ ; datasets DaRUS
   https://doi.org/10.18419/DARUS-4973 (espargos-0007, bistatic/passive radar
   template), espargos-0005. AR "WiFi camera": arXiv:2608.25996. Passive
   channel charting: arXiv:2504.09924.
2. **Chronos** — Vasisht, Kumar, Katabi, NSDI 2014; arXiv:1505.03446
   ("Sub-Nanosecond Time of Flight on Commercial Wi-Fi Cards").
3. **ToneTrack** — Xiong, Sundaresan, Jamieson, MobiCom 2015, pp. 537–549.
   **Multi-band splicing**: arXiv:2011.10402; arXiv:1903.02367.
4. **Carrier-phase RF ranging (DMR)** — GPS World "Innovation: Carrier-Phase
   RF Ranging", https://www.gpsworld.com/innovation-carrier-phase-rf-ranging/
   (CRLB 27 µm; <1 mm stand-alone CPR; multipath bounds; λ/2 ambiguity).
5. **BLE Channel Sounding** — Bluetooth SIG:
   https://www.silabs.com/wireless/bluetooth/channel-sounding ;
   NXP CS deck: community.nxp.com (ChannelSounding_public.pdf); NXP product
   page: nxp.com BLUETOOTH-CHANNEL-SOUNDING (RTT + PBR, PCTs).
6. **AirSync** — Balan, Rogalin, Michaloliakos, Psounis, Caire, arXiv:1205.6862;
   IEEE/ACM ToN 21(6) 2013, DOI 10.1109/TNET.2012.2230449. **AirShare** —
   Abari, Rahul, Katabi, Pant, INFOCOM 2015, pp. 1742–1750. **BeamSync** —
   Ganesan, Sarvendranath, Larsson, IEEE TWC 23(7) 2024. **OTA phase
   calibration in TDD flow** — arXiv:2509.03722. **Consensus+Kalman** — Rashid
   & Nanzer, IEEE TWC 22(4) 2023, pp. 2789–2803; two-tone hopping, IEEE Sensors
   Lett. 8(3) 2024. **Six-element wireless TWTT array** — arXiv:2505.13248.
   **Reciprocity calibration** — Rogalin et al., arXiv:1310.7001; optimal-
   topology CRLB, arXiv:1709.07206; arXiv:2606.14419.
7. **Multipath-assisted positioning** — Meissner & Witrisal, IWSSIP 2012
   (TUGraz PDF); Channel-SLAM, IEEE TWC 2016, DOI 10.1109/TWC.2016.2578336;
   SALMA (Großwindhager et al., MobiCom 2018); specular amplitude modelling,
   MDPI Sensors 22(2):462 (10.3390/s22020462).
8. **Radar-network self-calibration** — T. Grebner et al., IEEE (2023,
   Xplore 10128981); "Target Localization and Self-Calibration in a Multistatic
   Radar", Xplore 11675454 (2026); Bayesian auto-positioning, arXiv:2207.08503.
9. **Switched/single-RF arrays** — "Single Packet, Single Channel, Switched
   Antenna Array for RF Localization", ACM MobiSys 2023, DOI
   10.1145/3596263; TyrLoc, MobiSys 2021, pp. 228–240; Phaser, MobiCom 2014;
   in-orbit switched-element calibration, MDPI Electronics 15(12):2734 (2026);
   Analog Devices phased-array calibration with loopback network
   (analog.com technical article).
10. **Spherical/volumetric arrays** — arXiv:2604.02701 (SAA principles,
    geodesic polyhedra, 3D codebooks); Subspace PV DoA with unknown mutual
    coupling, MDPI Appl. Sci. 12(19):10099 (10.3390/app121910099);
    geodesic-sphere LEO arrays (IEEE Trans. AES lineage).
11. **RF neural fields** — NeRF², MobiCom 2023, DOI 10.1145/3570361.3592527;
    RF-3DGS, arXiv:2411.19420; "Learning radio environments by differentiable
    ray tracing", IEEE TMLCN 2:1527–1539, 2024 (Sionna RT, GC Wkshps 2023
    pp. 317–321); RF inverse rendering, arXiv:2604.07086; neural reflectance
    fields, arXiv:2501.02458.
12. **Standards / platform** — IEEE 802.11bf-2025:
    https://standards.ieee.org/ieee/802.11bf/11574 ; overview arXiv:2310.17661.
    ESP-IDF ESP32-S3 Wi-Fi driver doc (CSI; HT20/40; 16-way antenna switch):
    docs.espressif.com (esp32s3/api-guides/wifi.html); Clock tree
    (`SOC_ROOT_CLK_EXT_XTAL`): docs.espressif.com (esp32s3 clk_tree). ESP-CSI
    solution incl. clock-buffered C3+S3 board:
    docs.espressif.com/projects/esp-techpedia/.../esp-csi-solution.html.
13. **Passive illumination** — Jog et al., "Enabling IoT Self-Localization
    Using Ambient 5G Signals", NSDI 2022 (usenix.org/system/files/nsdi22-paper-
    jog.pdf); Wi-Fi 3D reconstruction survey (preprints.org 202608.2267);
    Wi-Fi depth imaging, arXiv:2503.06458.
14. **Corpus** — `00-synthesis.md`, notes `01`–`20` (this folder): two-way
    identity (`20` §4/§6), M2 residual (`00` §2), nanosecond timestamp (`17`),
    polyhedron/TDM verdicts (`08`,`09`), model/learning split (`19`).

## 14. Residual unknowns

1. **M2 on S3**: residual relative-CFO behaviour after per-frame estimation
   (drives E1's attainable precision and the §2.2 boundary table) — unmeasured.
2. **S3 per-lock phase roll and thermal coefficient** in the 3D hub star — E0.
3. **Home static-feature statistics** at 2.4 GHz/83 MHz (how registrable are
   real rooms?) — E-FIDUCIAL.
4. **Mutual coupling/shadowing** on the conducting polyhedron and its effect on
   the array manifold — unmeasured (flagged in `00-synthesis` §9).
5. **ESP32 CSI phase fidelity in products** (quantisation, per-rate/AGc
   effects) — E1.
6. **Pilot multipath-bias magnitude** in occupied homes (the boundary's
   practical strength) — E2.
7. Whether `esp-csi`/blob re-locks the PLL on channel hops on S3 and at what
   cadence the per-channel ledger must be re-measured — E0 variant.
8. Legal/duty design for continuous handshake lattice traffic under ACMA/FCC
   (`07` gives the envelope; the protocol's airtime budget is a design task).

## 15. One-line answers to the mission

- **The invention:** *coherence-locality* — 20-face conducted-coherent S3
  phase cameras (Puck), bound wirelessly by a two-way handshake lattice
  (ranges + fingerprints + obstruction), self-calibrated by registering the
  static home (Fiducial); sharpness = local sub-degree 3D bearings × cm
  two-way ranges × scene registration.
- **The boundary:** house-wide wirelessly-built carrier coherence is closed by
  `2π·Δf_res·T_gap < ε` (≈2 ppb at 3 ms for 0.1 rad); the minimal reference
  crossing it is one conducted clock+tone per *aperture*, not per home.
- **The decisive experiment:** E-FIDUCIAL — two un-surveyed pucks recover
  relative pose from two-way ranges + static-scene registration to <30 cm /
  <10°, holding over 1 h with a 5 °C swing.
