# 03 — Channel charting from unlabelled CSI

**Prompt.** `03-channel-charting`. Can channel charting (CC) recover relative
geometry from unlabelled CSI, and what does it need that Aquila lacks? Must
contain: (a) pipeline (dim-reduction + neighbour embedding); (b) assumptions
(anchors, temporal continuity); (c) metric vs geographic distortion; (d) labels
needed for absolute alignment. Leads: Studer et al. "Channel Charting"; ESPARGOS
channel-charting repos. Overlap: not 02, 16.

Australian English. All claims labelled **ESTABLISHED / HYPOTHESIS / SPECULATION**.
Hardware tier stated per claim. Sources are primaries (papers, upstream repos,
datasheets).

---

## Question

Given only CSI passively collected at one or more receivers, can a map of the
radio environment be learned *without labels*, and is that map the true geometry
or merely a topology-preserving distortion? What does Aquila's ESP32-S3 fleet
provide, and what is missing?

---

## Findings

### ESTABLISHED

**1. CC is a defined, reproduced method.** Studer et al. (IEEE Access 2018)
introduced CC: learn a mapping `g_θ` from high-dimensional CSI to a
low-dimensional **channel chart** such that points close in the chart are close
in physical space. The defining property is *local* geometric preservation, not
exact distances. CC is self-supervised: training uses CSI alone, with **no
ground-truth position labels** (`arXiv:1807.05247`; `channelcharting.github.io`).

**2. The canonical pipeline (three stages).** (`arXiv:1807.05247`,
`arXiv:2304.08095` "Theory, Practice and Applications" — Ferrand, Guillaud,
Studer, Tirkkonen; DICHASUS tutorial):
- **(i) CSI collection** — passively observe complex-valued per-subcarrier,
  per-antenna channel coefficients from many transmit locations over time.
- **(ii) Feature extraction** — reduce to *large-scale fading* features, made
  resilient to hardware impairments. In practice one feeds **sample correlations
  between channel coefficients of pairs of antennas** (real/imag split), *not*
  raw complex CSI, so that a global phase rotation `e^{jφ}` from unsynchronised
  oscillators cancels. This is the "feature engineering layer":
  `tf.einsum("damt,dbnt->dtabmn", csi, conj(csi))` (DICHASUS tutorial).
  Dissimilarity metrics include CIR amplitude (CIRA), angle-delay profile (ADP),
  and timestamp-based distances (`arXiv:2308.09539`).
- **(iii) Dimensionality reduction / manifold learning** — classical (PCA,
  Sammon's mapping / metric MDS) or deep (autoencoders, **Siamese** and
  **triplet** networks trained on the dissimilarity matrix via geodesic
  distances). Parametric DR learns a reusable forward charting function (FCF);
  non-parametric DR emits coordinates directly. A second "out-of-sample
  extension" phase maps new samples in real time (`arXiv:2304.08095`;
  `IIP-Group/ChannelCharting`; `IIP-Group/ChannelCharting_StreamingCSI`).

**3. What CC recovers by default is *relative* geometry only — affine-ambiguous.**
The chart's coordinate frame is arbitrary: it can be **translated, rotated,
scaled and reflected** relative to physical space. The ESPARGOS triplet-network
result is explicit: "the channel chart coordinates can be arbitrarily translated
/ rotated / scaled / flipped (affine transformation) compared to the true
physical coordinates", hence MAE is quoted *after optimal affine transform*
(`Jeija/ESPARGOS-WiFi-ChannelCharting`; `arXiv:2304.08095`). Local topology is
reliable; the global warp is the cost of dropping labels.

**4. Distortion is measured with topology/geometry metrics, not just position error.**
- **Continuity (CT)**, **Trustworthiness (TW)** ∈ [0,1] — neighbourhood
  preservation (physical→chart and chart→physical; "false neighbours" penalise
  TW). Both in scikit-learn.
- **Kruskal's Stress (KS)** ∈ [0,1], 0 best — global distance preservation.
- Position error (MAE, DRMS, CEP, R95) only meaningful **after** the optimal
  affine transform (or once an anchor fixes the frame).
This separation *is* the metric-vs-geographic distinction: CT/TW/KS are the
geometric-fidelity metrics; MAE/CEP are geographic and require alignment.
(`arXiv:2304.08095`; DICHASUS tutorial.)

**5. Real, labelled ground truth exists in public datasets — as more antennas,
not more users.** CC has been demonstrated on:
- **DICHASUS** (Stuttgart) — a distributed massive-MIMO channel sounder, e.g.
  `dichasus-cf0x`: **32 receive antennas** (4 arrays × 8), **1024 subcarriers**,
  50 MHz, with OTA frequency/time/phase synchronisation and a reference
  transmitter. `dichasus-cf0x` = 46.8 GB, `dichasus-ca0x` = 39.1 GB. Licence via
  DARUS (`doi:10.18419/DARUS-2854`).
- **ESPARGOS** (Stuttgart) — the directly relevant one: **ESP32-based** WiFi CSI
  channel sounder. `espargos-0001` (4 distributed 4×2 arrays, dominant LoS),
  `espargos-0002` (one 8×4 array, metal-wall NLoS), `espargos-0007` (passive,
  4 arrays, robot/human target). 40 MHz bandwidth, CSI from L-LTF/HT-LTF. Ground
  truth positions included (`espargos.net/datasets/`; `arXiv:2502.09405`).
- **Published results.** `espargos-0001`: CT 0.99, TW 0.99, KS 0.10, MAE 0.13 m,
  CEP 0.12 m (AoA-augmented Siamese). `espargos-0002`: CT 0.96, TW 0.96,
  KS 0.20, MAE 0.44 m after affine (`Jeija/ESPARGOS-WiFi-ChannelCharting`).
- **DICHASUS** commercial-4G example: 32 dual-polarised antennas, 288
  subcarriers at 2.5 GHz, hours of pedestrian tracks across LoS/NLoS; charts show
  the "inherent uncertainty about rotations and mirroring" (`arXiv:2304.08095`).

**6. The absolute-alignment question has three published answers.**
- **(a) Semi-supervised CC** — provide a *subset* of known positions; the chart
  is anchored to global coordinates. More labels → better global geometry.
- **(b) Known BS antenna positions** — the "Channel Charting in Real-World
  Coordinates" line (Taner, Palhares, Studer, `arXiv:2406.13722`, IEEE TWC 2025)
  adds a **bilateration loss** and an **LoS bounding-box loss** so the FCF outputs
  real-world coordinates with **32–80 phase/time-synchronised** BS antennas.
- **(c) Doppler-based CC** (Euchner, Stephan, ten Brink, `arXiv:2404.09620`) —
  a loss built on *differences* of unwrapped uplink phase across
  **frequency-synchronised** receivers. CFO cancels in the across-antenna
  difference, so it needs **only frequency synchronisation, not phase or time
  sync**, works with **just four single-antenna BS sites**, and produces
  coordinates in the **global frame with no affine transform**. It does assume
  roughly LoS-like propagation and an initially moving transmitter (then serves
  static ones too).

**7. Passive (device-free) CC now exists — the closest match to Aquila's thesis.**
"Passive Channel Charting" (`arXiv:2504.09924`, Apr 2025): static transmitters
spam beacons, static receivers, a **mobile passive target** (robot in foil, or a
human). The same Siamese/dimensionality-reduction machinery is reused; PCC beats
classical triangulation on `espargos-0007`. Cost: it **overfits to a particular
target type** — a limitation active-transmitter CC does not have. Training is
self-supervised; ground-truth positions are held out for evaluation.
(`Jeija/ESPARGOS-Passive-ChannelCharting`.)

**8. What the ESP32 hardware tier actually provides.**
- **Vanilla ESP32-S3 CSI**: single spatial stream, `wifi_csi_info_t` estimated
  from **L-LTF** (and HT-LTF if present); **64 subcarriers, 52 non-zero, 128
  bytes** for L-LTF at 20 MHz (HT-LTF: 56/114 at 20/40 MHz); signed 8-bit I/Q;
  `rx_ctrl` carries RSSI, noise floor, antenna, RX timestamp. ESP-IDF Wi-Fi
  driver docs. Chip CSI quality ranking (Espressif): **C5 > C6 > C3 ≈ S3 > ESP32**.
- **Cross-node phase is *not* free.** Espressif's own `esp-crab` achieves
  coherent phase only by **common-crystalling two chips through a clock buffer**
  ("single-transmission, dual-reception" mode) — i.e. the *shared/conducted
  reference* escape. The ordinary multi-node case is explicitly non-coherent.
- **ESPARGOS** is *eight ESP32s on one PCB*, phase-coherent because it
  distributes a **40 MHz reference clock plus a WiFi-based phase reference
  signal** via a resistive power splitter, plus per-array path-delay
  calibration. So the ESP32 *silicon* demonstrably supports coherent arrays — the
  enabler is the **shared clock/phase distribution and calibration**, not the
  chip. (`arXiv:2502.09405`.)

### HYPOTHESIS

**H1. Aquila can reproduce CC on ESP32-S3 — but only after manufacturing a
coherent aperture.** The ESPARGOS results are a proof-of-concept *on ESP32
silicon*, so the input format (L-LTF/HT-LTF complex CSI) and the NN pipelines are
directly portable. The blocker is phase reference: independently-clocked S3 nodes
give, at best, frequency-synced CFO that cancels across-antenna only if the
across-antenna differences are formed *within a synchronised aperture* or via the
Doppler loss. A **soldered-PCB polyhedron with one shared 40 MHz clock** is the
Aquila analogue of ESPARGOS's board and should yield a coherent 3D aperture.

**H2. Doppler-based CC (`arXiv:2404.09620`) is the most plausible escape for
genuinely distributed, independent-LO Aquila nodes**, because it needs only
frequency sync. Whether a handful of S3 nodes can hold frequency sync well enough
(unlike ESPARGOS's hardwired clock) is untested; the paper uses DICHASUS hardware
with deliberate desynchronisation as an ablation.

**H3. Passive CC maps onto Aquila's multi-node + `esp_wifi_80211_tx` scheme
directly**: static S3 transmitters spamming frames, static S3 receivers, and a
moving person/object as the passive target. The overfitting-to-target-type cost
is the open risk.

**H4. Temporal continuity is a real assumption, not free.** The timestamp-based
dissimilarity metric carries the trajectory assumption (points close in time are
close in space); CSI-based metrics carry the converse failure (LoS↔NLoS transitions,
revisits). The literature fuses both (`D_ADP` + `D_time`) precisely because either
alone breaks. Aquila must therefore stream CSI with a usable per-frame timestamp
(S3 has a microsecond RX timestamp) and a known traversal.

### SPECULATION

**S1.** A polyhedron aperture with a shared clock might make **DoA (MUSIC) and CC
mutually reinforcing** — CC-initialised DoA, DoA-augmented CC — the
`ToA-AoA-Augmented-ChannelCharting` idea, but on a volumetric aperture giving 3D
AoA. Untested.

**S2.** CC could serve as an **unlabelled pre-training** step that makes Aquila's
later fingerprinting/SLAM cheaper (chart as a warm start), but no source
demonstrates this transfer on ESP32-class hardware.

---

## ESP32-S3 feasibility

| Aspect | Verdict | Basis |
|---|---|---|
| Complex CSI input (L-LTF/HT-LTF) | **Available now** on S3 | ESP-IDF Wi-Fi CSI docs; `micropython-esp32-csi` exposes LLTF/HTLTF |
| Single antenna per node | **Limiting** | S3 is single-stream; ESP32 can switch up to 16 antennas via external switch but 1–2 active at once |
| 64 subcarriers (52 useful) | Workable but coarse vs 1024 (DICHASUS) / ESPARGOS | ESP-IDF; Wi-ESP |
| Cross-node phase coherence | **Not available on vanilla independent nodes** | esp-csi/esp-crab need a clock buffer; ESPARGOS needs a distributed 40 MHz clock + phase reference |
| Shared-clock polyhedron | **Plausible** — ESPARGOS is proof on ESP32 silicon | `arXiv:2502.09405` |
| Frequency-sync-only path (Doppler CC) | **Untested on S3** | method itself on DICHASUS (`arXiv:2404.09620`) |
| Timestamp for temporal metric | **Available** (µs RX timestamp) | BRIEF assumption; ESP-IDF `rx_ctrl` |
| Compute | **Host-side only**; NN training needs a GPU | ESPARGOS notebooks used RTX 4080; inference lighter |

The decisive hardware gap is **not the chip** (ESPARGOS reuses ESP32) — it is
**phase/time reference distribution and calibration**. A single-PCB polyhedron
with a shared clock is the shortest path; distributed independent-LO nodes must
rely on the Doppler loss or an OTA/common-TX reference.

**Gap quantified (vs ESPARGOS):** 8 coherent antennas on one board with 40 MHz
shared clock and per-array calibration → 1 antenna per node, independent clocks,
no path-delay calibration, 64 vs 1024 subcarriers. That is a large but bounded
gap, addressable by hardware design (shared clock) rather than new silicon.

---

## TX needed?

**Yes, in some form — but not for the CSI itself.** CC is *passively* learned; the
receiver only listens. However something must radiate:
- **Active-transmitter CC** needs a transmitter (the moving UE / a node) emitting
  frames — Aquila's `esp_wifi_80211_tx` beacons/pings from one or many nodes
  supply this.
- **Passive CC** (`arXiv:2504.09924`) needs **static transmitters** spamming
  frames plus only a *passive* moving target; the target radiates nothing.

So Aquila's TX capability is an enabler (illumination) rather than a requirement
for the geometry-learning algorithm. The **round-trip ping-and-unique-reply
scheme** is directly useful: replies give N distinguishable CSI streams plus
timestamps for the timestamp-based dissimilarity metric and for Doppler
unwrapping. Its failure modes (multi-user reply collisions, reply-timing jitter,
and above all the fact that it does **not** create phase coherence) limit it to
feeding temporal/Doppler features, not to fixing absolute phase.

---

## Evidence strength

**Method existing, published, reproduced: 9/10** — CC is a well-cited,
independently reproduced framework with open code and labelled public datasets
(`arXiv:1807.05247`; `arXiv:2304.08095`; the ESPARGOS and DICHASUS repos).

**Doppler/frequency-sync-only CC: 7/10** — one credible primary, global-frame
result with 4 antennas, but a single group and single environment
(`arXiv:2404.09620`).

**Passive CC: 6/10** — single 2025 paper + repo, one dataset, acknowledged
target-type overfitting (`arXiv:2504.09924`).

**CC on vanilla, independent-LO ESP32-S3, as Aquila would deploy: 3/10** — the
only ESP32-based result (ESPARGOS) *manufactures* coherence with a shared clock
and phase reference; nothing published shows CC from non-coherent, independently
clocked S3 nodes. This is the honest Aquila-specific confidence.

**Overall for this note: 7/10** — the *method* answers the question confidently
and the *hardware prerequisite* is now precisely identifiable, but the specific
Aquila configuration is unproven.

---

## Primary sources

- C. Studer, S. Medjkouh, E. Gönültaş, T. Goldstein, O. Tirkkonen, "Channel
  Charting: Locating Users within the Radio Environment using Channel State
  Information," *IEEE Access*, vol. 6, 2018. arXiv:1807.05247.
- P. Ferrand, M. Guillaud, C. Studer, O. Tirkkonen, "Wireless Channel Charting:
  Theory, Practice, and Applications." arXiv:2304.08095.
- S. Taner, V. Palhares, C. Studer, "Channel Charting in Real-World Coordinates
  with Distributed MIMO," *IEEE TWC*, 2025. arXiv:2406.13722.
- F. Euchner, P. Stephan, S. ten Brink, "Leveraging the Doppler Effect for
  Channel Charting." arXiv:2404.09620.
- F. Euchner, P. Stephan, S. ten Brink, "Uncertainty-Aware Dimensionality
  Reduction for Channel Charting with Geodesic Loss." arXiv:2412.01715.
- P. Stephan, F. Euchner, S. ten Brink, "Angle-Delay Profile-Based and
  Timestamp-Aided Dissimilarity Metrics for Channel Charting," *IEEE Trans.
  Commun.*, 2024. arXiv:2308.09539.
- F. Euchner et al., "Passive Channel Charting: Locating Passive Targets using
  Wi-Fi Channel State Information." arXiv:2504.09924.
- M. Gauger, F. Euchner, S. ten Brink, "ESPARGOS: An Ultra Low-Cost,
  Realtime-Capable Multi-Antenna WiFi Channel Sounder." arXiv:2502.09405.
- ESPARGOS datasets: https://espargos.net/datasets/ (espargos-0001/0002/0007).
- DICHASUS datasets: https://dichasus.inue.uni-stuttgart.de/datasets/ ;
  `dichasus-cf0x`, `doi:10.18419/DARUS-2854` (46.8 GB).
- Code: `IIP-Group/ChannelCharting` (MATLAB PCA+Sammon); `IIP-Group/ChannelCharting_StreamingCSI`;
  `Jeija/ESPARGOS-WiFi-ChannelCharting` (Siamese/triplet, TF); `Jeija/ESPARGOS-Passive-ChannelCharting`;
  `Jeija/ToA-AoA-Augmented-ChannelCharting`; `Jeija/Geodesic-Uncertainty-Loss-ChannelCharting`.
- Espressif: ESP-IDF Wi-Fi driver CSI docs (ESP32-S3); ESP-CSI Solution /
  `esp-crab` common-crystal phase sync (ESP Techpedia).
- `micropython-esp32-csi` (LLTF 52 sc, HTLTF 56/114 sc); Wi-ESP (JCDE 2020,
  52-subcarrier DFWS tool); `StevenMHernandez/ESP32-CSI-Tool`.

---

## Residual unknowns

1. Does CC work on **independently-clocked ESP32-S3** nodes at all, or does it
   require a shared clock/OTA reference in every case? (No primary.)
2. Practical **CSI frame rate and temporal coverage** needed to satisfy the
   temporal-continuity assumption on S3.
3. **CFO drift magnitude** on independently clocked S3 nodes over the minutes a
   Doppler-based chart needs — is frequency sync holdable without hardware?
4. Licence terms of the **DICHASUS** datasets (DARUS) and ESPARGOS datasets —
   must be checked before reuse; not resolved here.
5. Whether a **soldered polyhedron** shared-clock aperture yields enough angular
   diversity with only 64 subcarriers (ESPARGOS uses 40 MHz / more subcarriers).
6. **Passive CC** generalisation across target types — the paper itself flags
   overfitting.
7. Whether the **round-trip ping-and-unique-reply** timestamps are precise enough
   to substitute for the DICHASUS OTA synchronisation in the temporal metric.

---

## (A) Chaining

- **Cross-node phase coherence → CC.** Single-antenna CC dissimilarity metrics
  (ADP/AoA-style) are *blocked* without a phase reference. Shared/conducted
  reference (one PCB, shared clock — the ESPARGOS model and `esp-crab`) or a
  common-transmitter reference *enables* multi-antenna CC.
- **Frequency sync alone → Doppler CC** (`arXiv:2404.09620`), which then
  *enables* global-frame coordinates **without** affine alignment — a distinct
  capability from classical CC.
- **Continuous CSI streaming → temporal-continuity metric**, which *enables* the
  fused dissimilarity metric and makes CC robust to LoS/NLoS transitions.
- **`esp_wifi_80211_tx` → illumination** for both active CC (mobile node) and
  passive CC (static TX + mobile target).
- **CC → absolute localisation → RF-SLAM (prompt 16)**, and **CC → passive target
  charting (prompt 02)**. CC is the *unlabelled self-supervised backbone*; SLAM
  and fingerprinting are the supervised refinements that consume its output.
- **Highest-leverage enabling capability:** a **shared/conducted phase reference
  across a co-located aperture** (the polyhedron). It unlocks coherent multi-antenna
  CC *and* DoA/MUSIC *and* the AoA-augmented chart — one hardware decision with
  three downstream unlocks. Without it, the only wireless-only escape left is OTA
  reciprocity / common-TX reference, both unproven on S3.
- **Capability that unlocks a further one:** frequency synchronisation unlocks the
  Doppler loss, which unlocks *static-transmitter* localisation (the method, once
  trained on movers, serves static targets).

## (B) Existing models and data

- **Models/implementations:** Studer MATLAB PCA+Sammon simulator; Siamese and
  triplet NN CC; representation-constrained autoencoders (`arXiv:1908.02878`);
  geodesic/uncertainty-aware DR; bilateration+bounding-box real-coordinate CC;
  Doppler-loss CC; passive CC. All open-source (permissive licences on the
  GitHub repos — verify per repo) and Python/TensorFlow or MATLAB.
- **Datasets:** DICHASUS (32 antennas, 1024 subcarriers, phase-coherent, ground
  truth; `doi:10.18419/DARUS-2854`) and ESPARGOS (ESP32-based, 40 MHz, ground
  truth; `espargos.net`). ESPARGOS is the **most reusable for Aquila** because the
  input is literally ESP32 L-LTF/HT-LTF CSI — same silicon family, same CSI
  format — but *with phase coherence Aquila must supply itself*.
- **Can Aquila use them?** Yes for *method and code*: the pipelines take complex
  CSI matrices and are hardware-agnostic given the phase reference. Input-format
  compatibility with S3 is high for amplitude-based metrics (CIRA) and for the
  Doppler loss (phase *differences*); it is **low** for any metric assuming a
  coherent multi-antenna aperture unless Aquila builds that aperture. Licence and
  size (tens of GB) are practical frictions.

## (C) Own-data / new technology

**Could a purpose-built Aquila dataset unlock something not demonstrated?**
Demonstrated today: CC with **phase-coherent 8–80 antennas** (ESPARGOS/DICHASUS)
and Doppler CC with **4 frequency-synced, non-phase-coherent single-antenna**
sites. **Not** demonstrated: CC from **independent-LO, single-antenna ESP32-S3
nodes without phase synchronisation** — exactly Aquila's vanilla deployment.

**Minimal experiment that would prove it.**
1. Four or more ESP32-S3 receivers. Two configurations: **(a)** independent clocks;
   **(b)** a shared clock buffer (the `esp-crab`/ESPARGOS trick). 
2. One transmitter (a fifth S3) traverses a **known ground-truth trajectory**
   (marked floor, or a measured track) through a room with LoS and NLoS segments,
   transmitting standard frames.
3. Log per-frame CSI + the S3 microsecond RX timestamp for several thousand
   positions (the DICHASUS tutorials use ~20 k datapoints).
4. Run (i) the CIRA/ADP dissimilarity-metric CC pipeline, (ii) the timestamp-fused
   pipeline, and (iii) the Doppler-loss pipeline. Evaluate **CT, TW, KS** and,
   after optimal affine alignment, **MAE/CEP**; compare against RSSI
   fingerprinting and (where applicable) triangulation.
5. Stratify by configuration (a) vs (b) and by time-since-sync.

**What would falsify the claim "Aquila can channel-chart":**
- CT/TW indistinguishable from chance, or no better than a random neighbour graph;
- MAE after affine alignment no better than RSSI fingerprinting;
- with a shared clock, still no neighbourhood preservation (would implicate the
  64-subcarrier single-antenna CSI, not synchronisation);
- on independent clocks, **both** the dissimilarity and Doppler pipelines fail —
  which would establish that Aquila's vanilla independent-LO configuration
  *cannot* channel-chart and that a shared/OTA reference is mandatory.

A **positive** result in configuration (a) would be a genuine new contribution:
CC on the cheapest phase-incoherent COTS tier, which no primary currently shows.
