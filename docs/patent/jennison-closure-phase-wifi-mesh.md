# Patent Application (Draft)

**Title:** Wi-Fi Sensing Mesh for Three-Dimensional Scatterer Tracking Using Jennison Closure Phase Across Directed Multi-Node Packet Loops

**Applicant:** [Applicant name]
**Inventor(s):** [Inventor name(s)]
**Priority claim:** [To be completed]
**Draft status:** For review by qualified patent attorney. Not legal advice.

---

## 1. Field of the Invention

The invention relates to passive radio-frequency sensing using Wi-Fi channel state information (CSI), and more particularly to a distributed Wi-Fi sensing mesh that estimates and tracks the three-dimensional position and motion of passive scatterers (e.g., people, animals, objects) by computing Jennison closure phase across directed multi-node packet loops. The technique is applicable to indoor presence detection, occupancy counting, fall detection, vital-sign sensing, and gesture recognition without cameras, wearables, or dedicated radar hardware.

---

## 2. Background of the Invention

### 2.1 Wi-Fi CSI sensing and its phase-coherence problem

Wi-Fi channel state information (CSI) is the complex channel response measured across the OFDM subcarriers of a received 802.11 packet. When a person moves through a Wi-Fi-covered space, their body perturbs the multipath channel, producing measurable changes in CSI amplitude and phase across subcarriers and time. This phenomenon is the basis of "Wi-Fi sensing."

A fundamental limitation of single-node and conventional multi-node Wi-Fi sensing is phase coherence. Each Wi-Fi node (e.g. an ESP32-S3) has its own independent local oscillator and free-running hardware clock. The absolute phase measured at a receiver on one node is not comparable, in any absolute sense, to the absolute phase measured at a receiver on another node, because:

- each receiver's local oscillator contributes an unknown, time-varying phase offset;
- each node's sampling clock has independent frequency offset (skew) and drift;
- there is no shared time or phase reference between nodes;
- packets are received asynchronously across nodes.

Consequently, conventional Wi-Fi mesh sensing is generally restricted to per-link amplitude changes or per-node phase time series, and cannot form a phased array, perform interferometry, or measure absolute time-of-flight or angle of arrival across nodes.

### 2.2 Closure phase in radio interferometry

Closure phase (also called phase closure) is a technique from radio interferometry, described by R. C. Jennison (1958) and used extensively in very-long-baseline interferometry (VLBI), including by the Event Horizon Telescope to produce the first images of black holes (M87* and Sagittarius A*).

For three antennas A, B, and C observing a common radio source, the measured phase on each baseline contains both (i) a geometric phase dependent on source position and baseline geometry, and (ii) an antenna-based phase error (clock/local-oscillator phase). By forming the sum of measured phases around a closed triangle of baselines — A→B, B→C, C→A — the antenna-based phase errors cancel exactly, leaving a closure phase that depends only on the source geometry and propagation path. Closure phase is therefore immune to antenna-based phase and clock errors, without requiring phase coherence between antennas.

### 2.3 The problem addressed by this invention

No known Wi-Fi sensing system applies Jennison closure phase to directed multi-node packet loops for 3D scatterer tracking. Conventional Wi-Fi sensing cannot exploit closure phase because:

- Wi-Fi nodes are not an interferometric array; they do not share a local oscillator or clock;
- Wi-Fi packet transmission is asynchronous and reception-driven, not simultaneous observation of a common coherent source;
- CSI is measured per-link (transmitter → receiver) and is directional;
- prior art Wi-Fi mesh systems treat phase as unusable across nodes and rely on amplitude only, or on single-node phase time series.

There is a need for a Wi-Fi sensing technique that (a) works across an incoherent multi-node mesh, (b) is immune to per-node clock and local-oscillator phase error, (c) yields three-dimensional scatterer position and motion, and (d) uses only passive, ambient Wi-Fi traffic without active radar-like transmission.

---

## 3. Summary of the Invention

The invention provides a Wi-Fi sensing mesh comprising a plurality of fixed Wi-Fi sensor nodes that exchange CSI frames over directed Wi-Fi links. The nodes are not phase-coherent and share no clock. Each node measures CSI on received packets from other nodes (transmitter → receiver pairs). The sensing core (which may be one of the nodes or a separate host) collects directed CSI measurements and forms directed multi-node packet loops (cycles) among the nodes — for example node A → node B → node C → node A. For each directed loop, the system computes a Jennison closure phase by summing (with appropriate sign for direction) the measured CSI phases on each directed link of the loop across selected subcarriers.

Because each directed link contributes a transmitter-side phase, a propagation phase, and a receiver-side phase, and because the loop returns to its starting node, every node's local oscillator and clock phase appears once as a transmitter term and once as a receiver term with opposite sign, and therefore cancels. The residual closure phase is determined by the geometric path lengths of the links and by any moving scatterer that perturbs those paths.

By forming multiple directed loops of different geometry and orientation among the same nodes, and by tracking closure phase over time and across subcarriers (frequency diversity), the system solves for the three-dimensional position and motion of one or more moving passive scatterers. No phase coherence between nodes is required. No active elicitation waveform is required — the system is passive and uses ambient Wi-Fi traffic (or opportunistic transmissions between mesh nodes).

---

## 4. Detailed Description

### 4.1 System architecture

The system comprises:

1. **A plurality of fixed Wi-Fi sensor nodes**, each comprising:
   - a Wi-Fi radio capable of measuring and reporting CSI (complex channel coefficients per OFDM subcarrier) on received packets;
   - a local oscillator and free-running hardware clock, independent of all other nodes;
   - a transmitter and receiver for Wi-Fi packet exchange with other nodes;
   - a network interface (e.g. UDP over Wi-Fi) to a sensing core.

2. **A sensing core** (a host or a designated node) that:
   - collects CSI frames from the nodes, timestamped at reception;
   - maintains a topology model of the mesh (node positions, floors, rooms, zones);
   - forms directed multi-node packet loops among the nodes;
   - computes Jennison closure phase for each directed loop;
   - estimates scatterer position and motion from closure-phase measurements across loops, subcarriers, and time;
   - outputs presence, count, pose, and/or tracking results per zone.

The nodes are "dumb sensors" in the sense that they perform no DSP, inference, or fusion; all sensing computation is performed at the sensing core. This is consistent with the Aquila node model, but the invention is not limited to it.

### 4.2 Directed CSI measurements

For each received packet at receiver node R from transmitter node T, the receiver measures:

- CSI: a complex coefficient H_{T→R}[k] for each OFDM subcarrier k = 0, ..., N−1;
- phase: φ_{T→R}[k] = arg(H_{T→R}[k]), wrapped to (−π, π];
- metadata: receiver timestamp, transmitter node id, receiver node id, subcarrier layout, centre frequency, bandwidth, RSSI, sequence number.

The measurement is directed: H_{T→R} generally differs from H_{R→T} because the transmitter and receiver hardware chains differ and the channel is reciprocal only in the propagation path, not in the radio front ends.

Each CSI phase contains:
- a transmitter local-oscillator phase θ_T(t);
- a receiver local-oscillator phase θ_R(t);
- a propagation phase −2π f_k · d_{T→R}(t) / c, where d_{T→R}(t) is the effective path length (including multipath) and c is the speed of light;
- scatterer-induced perturbations to d_{T→R}(t);
- noise.

### 4.3 Directed multi-node packet loops

A directed L-node loop is an ordered cycle of distinct nodes (n₀, n₁, ..., n_{L−1}, n₀), where each consecutive pair (nᵢ → nᵢ₊₁) is a directed Wi-Fi link on which CSI is measured (indices mod L). The loop is "directed" because each link has a defined transmitter and receiver, and reversing a link changes the sign of its contribution to closure phase.

The system forms loops of length L ≥ 3 (triangles and larger polygons) among available nodes. Multiple loops may share nodes and links. Loops of different geometry (different node triples, different orientations, different planar areas, including cross-floor loops) provide geometrically diverse closure-phase constraints.

Example: with nodes A, B, C, the directed triangle loop A→B→C→A uses CSI measurements H_{A→B}, H_{B→C}, H_{C→A}.

### 4.4 Jennison closure phase computation

For a directed L-node loop (n₀ → n₁ → ... → n_{L−1} → n₀) and subcarrier k, the closure phase is:

Ψ_loop[k] = Σ_{i=0}^{L−1} s_i · φ_{nᵢ→nᵢ₊₁}[k]  (mod 2π)

where s_i = +1 for a link traversed in its measured direction and −1 for a link traversed opposite to its measured direction (phase conjugation). For a consistently directed loop where every link is measured in the loop direction, all s_i = +1:

Ψ_loop[k] = φ_{n₀→n₁}[k] + φ_{n₁→n₂}[k] + ... + φ_{n_{L−1}→n₀}[k]  (mod 2π)

Substituting the phase model:

Ψ_loop[k] = Σ_i (θ_{nᵢ}(t) − θ_{nᵢ₊₁}(t)) − (2π f_k / c) Σ_i d_{nᵢ→nᵢ₊₁}(t) + perturbations + noise  (mod 2π)

The telescope/local-oscillator terms telescope: each θ_{nᵢ} appears once positively (as receiver of link nᵢ₋₁→nᵢ) and once negatively (as transmitter of link nᵢ→nᵢ₊₁), so they cancel exactly:

Ψ_loop[k] = −(2π f_k / c) · D_loop(t) + Δ_scatterer[k](t) + noise  (mod 2π)

where D_loop(t) = Σ_i d_{nᵢ→nᵢ₊₁}(t) is the total effective path length around the loop, and Δ_scatterer[k](t) is the closure-phase perturbation induced by moving scatterers.

Thus closure phase is independent of per-node clock and local-oscillator phase, requires no inter-node phase coherence, and is a direct observable of the loop's effective geometric path length.

### 4.5 Phase unwrapping and frequency diversity

The measured phase φ is wrapped to (−π, π]. For closure phase across subcarriers, the system:

- unwraps phase across subcarrier frequency for each link (correcting 2π wraps as a function of f_k);
- computes closure phase on unwrapped phases, then re-wraps the result;
- exploits frequency diversity: closure phase varies linearly with f_k through D_loop(t), so a slope across subcarriers estimates D_loop(t) robustly, and curvature/deviations from linearity indicate multipath/scatterer structure.

### 4.6 Three-dimensional scatterer tracking

Each moving scatterer j at position p_j(t) perturbs the effective path length of links that pass near it. For a loop, the scatterer-induced closure-phase perturbation is a function of scatterer position relative to the loop geometry:

Δ_scatterer,loop[k](t) ≈ Σ_j (2π f_k / c) · δd_{loop,j}(p_j(t))

where δd_{loop,j} is the path-length perturbation of the loop caused by scatterer j.

By forming multiple loops of different geometry (different node triples, areas, orientations, floor crossings), the system obtains multiple independent closure-phase measurements at the same time. Each loop constrains scatterer position to a surface (analogous to an interferometric fringe). The intersection of surfaces from multiple loops localises the scatterer in 3D.

Tracking over time: as a scatterer moves, D_loop(t) and Δ_scatterer,loop[k](t) change. The time derivative of closure phase across loops yields scatterer velocity; fitting a motion model yields trajectory. Breathing and chest-wall motion produce sub-millimetre path-length perturbations that appear as low-frequency (0.1–0.5 Hz) closure-phase modulation; gross body motion produces larger, faster perturbations.

### 4.7 Subcarrier and band selection

The system operates on HT20 (~64 subcarriers at 20 MHz) or HT40 (~128 subcarriers at 40 MHz) Wi-Fi channels. Not all subcarriers carry equal scatterer information; some are nulls or dominated by hardware ripple. The system selects a subset of subcarriers (e.g. those with highest scatterer sensitivity / SNR) during calibration and freezes the selection for runtime, consistent with a deterministic pipeline.

### 4.8 Calibration and baseline

The system maintains an empty-home baseline: per-link, per-subcarrier CSI phase and amplitude measured with no person present. Closure phase is computed relative to baseline, so static multipath and fixed furniture contribute a constant closure phase that is subtracted. Moving scatterers produce deviations from baseline closure phase. Calibration also learns per-node clock skew models to assist time alignment across nodes (though closure phase itself is clock-error immune).

### 4.9 Determinism and reproducibility

The signal-conditioning and closure-phase pipeline is deterministic and bit-exact for identical inputs on a pinned target: no runtime trigonometric functions outside pinned tables, fixed accumulation order, no parallel reductions, no wall-clock dependence. This permits replay of recorded sessions and commit-keyed measurement ledgers, so closure-phase metrics are reproducible rather than anecdotal.

### 4.10 Witness grading

Every sensing claim (presence, count, position, pose, vitals) carries an evidence grade: Gated, Measured, Claimed, Data-gated, or Hardware-gated (strongest first). Closure-phase-derived claims are graded according to the quality and quantity of loop coverage, baseline availability, and reference-ground-truth support.

---

## 5. Brief Description of Drawings

[Figures to be provided by patent attorney / draftsperson]

- **Figure 1.** System block diagram: Wi-Fi sensing mesh of fixed nodes, directed links, sensing core, closure-phase computation, scatterer tracking output.
- **Figure 2.** Directed three-node packet loop (A→B→C→A) and cancellation of per-node local-oscillator/clock phase in closure phase.
- **Figure 3.** Phase model of a directed link showing transmitter phase, propagation phase, receiver phase, and scatterer perturbation.
- **Figure 4.** Closure phase vs. subcarrier frequency for a loop, showing linear dependence on effective path length and scatterer-induced deviation.
- **Figure 5.** Multiple directed loops of different geometry providing intersecting constraints for 3D scatterer localisation.
- **Figure 6.** Time-series of closure phase showing breathing (low-frequency) and gross motion (higher-amplitude) perturbations.
- **Figure 7.** Method flowchart: collect directed CSI → form loops → compute closure phase → unwrap → estimate scatterer 3D position → track over time → output per-zone results.

---

## 6. Claims

**Claim 1.** A Wi-Fi sensing system for tracking moving passive scatterers in three dimensions, comprising:
- a plurality of fixed Wi-Fi sensor nodes, each node having an independent local oscillator and an independent free-running clock, with no shared phase or time reference between any pair of nodes;
- each node configured to measure channel state information (CSI) on Wi-Fi packets received from other nodes, yielding, for each directed transmitter-to-receiver link and each of a plurality of OFDM subcarriers, a complex channel coefficient and a measured phase;
- a sensing core configured to:
  - collect the CSI measurements from the nodes;
  - form one or more directed multi-node packet loops, each loop being an ordered cycle of distinct nodes in which each consecutive pair is a directed link for which CSI is measured;
  - for each directed loop and each subcarrier, compute a Jennison closure phase by summing the measured phases of the directed links around the loop, wherein each node's local-oscillator phase and clock phase appears once as a transmitter term and once as a receiver term with opposite sign and therefore cancels, so that the closure phase is independent of per-node clock and local-oscillator phase error and requires no inter-node phase coherence;
  - estimate a three-dimensional position of at least one moving passive scatterer from closure-phase measurements across a plurality of directed loops of different geometry;
  - output a tracking result for the scatterer.

**Claim 2.** The system of claim 1, wherein the sensing core is further configured to unwrap the measured phase across subcarrier frequency for each directed link before computing closure phase, and to estimate the effective loop path length from the slope of closure phase versus subcarrier frequency.

**Claim 3.** The system of claim 1, wherein the directed multi-node packet loops include triangular loops of three nodes and polygonal loops of more than three nodes, and wherein loops of different planar area and orientation provide geometrically diverse closure-phase constraints.

**Claim 4.** The system of claim 1, wherein the system is passive and uses ambient Wi-Fi traffic or opportunistic transmissions between mesh nodes, and wherein no node transmits a dedicated elicitation waveform for sensing.

**Claim 5.** The system of claim 1, further comprising:
- an empty-home baseline of per-link, per-subcarrier CSI phase measured with no scatterer present;
- wherein the sensing core computes closure phase relative to the baseline so that static multipath and fixed obstacles contribute a constant closure phase that is subtracted, and moving scatterers produce deviations from baseline closure phase.

**Claim 6.** The system of claim 1, wherein the sensing core tracks the scatterer over time by measuring changes in closure phase across the directed loops, the time derivative of closure phase yielding scatterer velocity, and low-frequency closure-phase modulation yielding breathing or chest-wall motion of a person.

**Claim 7.** The system of claim 1, further comprising a calibration module that selects a subset of subcarriers carrying scatterer information and freezes the selection for runtime, such that the signal-conditioning and closure-phase pipeline is deterministic and bit-exact for identical recorded inputs.

**Claim 8.** The system of claim 1, wherein every sensing claim carries one of five evidence grades — Gated, Measured, Claimed, Data-gated, or Hardware-gated — derived from a commit-keyed measurement ledger.

**Claim 9.** A method of operating a Wi-Fi sensing mesh, the method comprising:
- measuring, at each of a plurality of fixed Wi-Fi nodes having independent local oscillators and independent free-running clocks, CSI on packets received from other nodes, the CSI comprising a measured phase per OFDM subcarrier for each directed transmitter-to-receiver link;
- forming at least one directed multi-node packet loop comprising an ordered cycle of distinct nodes;
- computing a Jennison closure phase for the loop by summing measured link phases around the loop, such that per-node local-oscillator and clock phases cancel;
- estimating a three-dimensional position of a moving passive scatterer from closure-phase measurements across multiple directed loops of different geometry;
- and outputting a per-zone tracking result.

**Claim 10.** The method of claim 9, further comprising unwrapping phase across subcarrier frequency and estimating effective loop path length from closure phase versus frequency.

**Claim 11.** The method of claim 9, wherein the loops include triangular loops of three nodes and the method requires no phase coherence between nodes and no shared clock.

**Claim 12.** A computer-readable storage medium storing instructions that, when executed by a sensing core of a Wi-Fi sensing mesh, cause the core to perform the method of claim 9.

---

## 7. Abstract

A Wi-Fi sensing mesh tracks moving passive scatterers in three dimensions without requiring inter-node phase coherence. Fixed Wi-Fi nodes, each with an independent local oscillator and free-running clock, measure channel state information on received packets from other nodes. A sensing core forms directed multi-node packet loops (ordered cycles of nodes) and computes Jennison closure phase for each loop by summing measured link phases around the loop. Each node's local-oscillator and clock phase appears once as a transmitter term and once as a receiver term with opposite sign and therefore cancels, making closure phase immune to per-node phase and clock error. Multiple directed loops of different geometry provide intersecting constraints from which the three-dimensional position and motion of moving scatterers — including breathing-scale chest-wall motion — are estimated. The system is passive, uses ambient Wi-Fi traffic, requires no dedicated radar-like transmission, and produces deterministic, reproducible results with evidence-graded sensing claims.

---

## 8. Prior-art statement (for attorney review)

The closest prior art comprises:
- Wi-Fi CSI sensing systems using per-link amplitude and single-node phase time series (e.g. RuView, WiVi, and academic presence/vital-sign sensing). These do not form closure phase across directed multi-node loops and do not achieve cross-node interferometric phase cancellation.
- Radio interferometry and VLBI closure phase (Jennison 1958; Event Horizon Telescope black-hole imaging). These apply to coherent or incoherent antenna arrays observing astronomical sources, not to packet-based Wi-Fi meshes with asynchronous, directed transmitter→receiver CSI measurements.
- Wi-Fi mesh positioning and gesture systems using RSSI, CSI amplitude, or time-of-flight. These do not use closure phase and generally require either phase coherence, dedicated transmissions, or calibrated anchors.

The combination of (i) directed packet loops in an incoherent Wi-Fi mesh, (ii) Jennison closure-phase cancellation of per-node clock/LO error, and (iii) 3D scatterer tracking from multi-loop closure-phase geometry is believed to be novel and inventive. The patent attorney should conduct a novelty and inventive-step search, particularly in the intersection of Wi-Fi sensing, interferometric closure phase, and passive indoor localisation.

---

## 9. Inventor's note on the physics connection

Closure phase is the same interferometric observable that enabled the Event Horizon Telescope to image black-hole shadows: it measures structure from phase relationships around closed baselines while rejecting station-based phase corruption. This invention transplants that principle from radio astronomy to indoor Wi-Fi sensing. The "black hole" analogy extends further: a person moving through a multipath Wi-Fi channel perturbs the effective path length of links passing near them, and closure phase around loops enclosing that person encodes the perturbation — in effect, the scatterer's motion is inferred from the way it changes the phase "geometry" of invisible Wi-Fi paths through the home, much as a black hole's mass is inferred from the way it distorts light paths around it. This is an explanatory metaphor only and does not limit the claims.

---

## Appendix A — Figures

| Figure | File | Subject |
|--------|------|---------|
| Fig. 1 | `figures/fig01_system.svg` | Wi-Fi sensing mesh with Jennison closure-phase processing |
| Fig. 2 | `figures/fig02_loop.svg` | Directed three-node packet loop A&#8594;B&#8594;C&#8594;A |
| Fig. 3 | `figures/fig03_link.svg` | Phase model of a single directed link T&#8594;R |
| Fig. 4 | `figures/fig04_freq.svg` | Closure phase vs. subcarrier frequency |
| Fig. 5 | `figures/fig05_localization.svg` | Multiple loops for 3-D scatterer localisation |
| Fig. 6 | `figures/fig06_timeseries.svg` | Closure phase vs. time: breathing and gross motion |
| Fig. 7 | `figures/fig07_method.svg` | Method flowchart |

All figures are SVG vector line art in patent-drawing style (black on white, reference numerals, leader lines), suitable for conversion to the raster/TIFF formats required by most patent offices.

**End of draft.** Review with a qualified patent attorney before filing. Claims should be tailored to the actual reduction to practice, priority documents, and jurisdiction.