# Deep Research — Datasets and v1→v2 Spec Comparison for Aquila

**Status:** research (out-of-band, not v1/v2 scope). Informed by primary sources where possible.
**Last updated:** 2026-10-17. **Sources:** see `../reference/sources.json` and primary documents cited inline.
**Conventions:** Australian English spelling throughout. Every external claim cites its source.

---

## 1. v1-spec vs v2-spec — what changed

Both directories (`aquila/` and `aquila-v2-spec/`) share the same AGENTS.md structure, CONTEXT.md vocabulary, docs/ layout, and `belikh/aquila` issue tracker. v2-spec is not a fork that discards v1; it is a **redirection of the delivery route** informed by a 20-agent deep-research cycle (`docs/research/v2/`). The table below summarises the divergence.

| Dimension | v1-spec (`aquila/`) | v2-spec (`aquila-v2-spec/`) |
|-----------|---------------------|-----------------------------|
| **Destination** | Shipped v1: real ESP32-S3 nodes → HA add-on; presence/occupancy/count/pose/vitals; three gated bars evidenced by commit-keyed ledger | Learned, placement-structured, commodity-node system: per-zone occupancy (gated claim) + advisory 3D position; no phase-coherent aperture; no runtime camera |
| **Spatial scope** | Room = zone; v1 emits one zone per room; cross-floor staged | ≥3 nodes per floor, perimeter, maximally spaced, core-only; placement is **structured**, not random |
| **Cross-node phase** | Feature-level fusion only; no cross-node phase; no AoA/ToF/coherent array | `Coherence::Uncoherent` only; amplitude, motion/Doppler, temporal CSI ratios, inter-node amplitude/co-motion; geometric AoA/coherent ToA demoted to Track 3 |
| **Still people** | Not explicitly addressed (gated bars are motion/vitals) | Learned from static fingerprint against empty-home baseline; not from Doppler |
| **Camera** | Ground truth research-only, never runtime input | Camera is a **training-time teacher only**, never runtime input; walk-through adaptation is camera-free |
| **Data improvement** | Fixed models; no in-service data contribution | Per-home adaptation; users contribute self-verifying `.aqs` Sessions (CSI + camera-derived labels) as git PRs; video never leaves the home |
| **Data licence** | CC BY-NC usable (non-commercial project); NC artefacts out of GPLv3 history | CC-BY-NC in a **separate data repository**; continuous improvement via pretrain→reference corpus→per-home adaptation→hosted corpus (DOI)→periodic retrain→add-on/HACS update channel |
| **Sites** | Reference deployment (~9+ nodes, 3 floors) | Two live sites: Site A (3-storey apartment ≈ v1 reference); Site B (2-storey house, many rooms) = cross-configuration holdout |
| **Nanosecond RX timestamp** | v1 stance: "no PHY timestamp" (`fusion-sync.md` §1.1) | **Exists on S3** (`WIFI_PKT_RX_TIMESTAMP_NSEC`, latched ~20.8 µs after sampling start on 80 MHz / 12.5 ns grid) — supersedes v1 stance; upgrades trigger/epoch design |
| **FTM on S3** | Not specified | **2.4 GHz HT20 only**, coarse; metre-level accuracy (802.11mc KPI 4 m @40 MHz; ESP32-S2 indoor mean 3.52 m) |
| **Active TX** | Dropped; ESP-NOW, MQTT, etc. all "never-used" | `raw esp_wifi_80211_tx` defaults to 1 Mbps 11b (no OFDM preamble, no CSI); CSI "ping" must be **ESP-NOW HT40 MCS0** (as esp-csi itself uses). Potential glossary conflict: `CONTEXT.md` bans ESP-NOW |
| **Hardware homogeneity** | ESP32-S3 reference | Required — every node same chip (ESP32-S3); C3/S3 mix produces different CSI shape, incomparable for training/pooling |
| **Firmware-in-the-loop** | Hardware-only | Espressif QEMU fork (`idf.py qemu`) for CI-testable node firmware without hardware; QEMU models **no RF**, so RF-validated behaviour stays `Hardware-gated` |
| **Gated metrics** | Presence F1 ≥ 0.90; torso-PCK@20 ≥ 65%; breathing MAE ≤ 2 brpm | Per-zone occupancy is the gated claim; advisory 3D position has no honest metric-3D claim on $6 radios; vitals/pose deferred from v2 destination |
| **ADRs** | 0001–0006 | 0001–0004 (v2 adds its own ADRs and extends owning specs) |
| **Spec files** | 19 topic files | 18 topic files (same set minus one) |
| **Witness grades** | Gated / Measured / Claimed / DataGated / HardwareGated (five) | Measured / Claimed / Gated (three — simplified set) |

### Key implication: the v2 boundary on cross-node coherence

The v2 research synthesis (`docs/research/v2/00-synthesis.md` §1) formalises the observability boundary:

- With independent local oscillators, CSI takes the form `ĥ_ij = g_ij·a_i·b_j·e^{j(θ_i−ψ_j)}·e^{−j2πf_kτ}`, and only `θ_i−ψ_j` is observable.
- On a connected measurement graph, node phases are fixed **only up to one global additive constant** — a `U(1)` gauge (plus time/frequency gauges).
- **Invariants that survive the gauge** (usable with no shared reference): amplitude; per-link subcarrier differential phase (delay/ToF); per-receiver inter-antenna differential phase (AoA **within one receiver**); two-way products `ĥ_AB·ĥ_BA` (cancels LO, exposes hardware asymmetry); temporal differentials (Doppler).
- **Not observable without an added reference:** absolute cross-node phase, absolute phase-of-flight range, absolute synchronisation. **AoA across independent receivers is unobservable; AoA within a receiver is observable.**
- **Minimal reference:** one extra constraint per connected component fixes the gauge — a single common-transmitter broadcast, one reciprocal round trip per link (star is CRB-optimal), or a shared/conducted clock+tone. Cost O(N).

This boundary is the load-bearing constraint that shapes both v1 and v2, and it is directly relevant to the Jennison closure-phase invention (see §5 below).

---

## 2. Dataset deep research

### 2.1 EHUNAM — WiFi CSI dataset for human and machine sensing

| Attribute | Value |
|-----------|-------|
| **Source** | Springer Nature *Scientific Data*, 2025 Dec 17; 12:1950. DOI: 10.1038/s41597-025-06238-4. PMCID: PMC12711874. |
| **Repository** | Springer Nature Figshare (dataset ID 28541225); GitHub: GuillermoDiazSM/EHUNAM-dataset |
| **Size** | 72.39 GB (single .zip; unzipped contains per-measurement `.mat` files) |
| **Duration** | ~38 hours of recordings over **23 days** |
| **Environments** | **8 distinct environments**: basement room, classroom, office, storage room, laboratory, office 2, meeting room, industrial laboratory |
| **Subjects** | 21 people (17 with demographic data: 14 M / 3 F reported) |
| **Machines** | 9 machines (4 horizontal lathes, 1 manual milling cutter, 4 static) |
| **Measurements** | 2,401 total: MC1 = 1,618 (27 hours), MC2 = 105 (~22 min), MC3 = 678 (11 hours) |
| **CSI per 60 s measurement** | 1K–100K (median 58K); equispaced MC1 sets: 0.6K–1.2K |
| **Subcarriers** | 56 (ACT/20 MHz), 64 (NCE/20 MHz), 208/241 (80 MHz, campaign-dependent) |
| **Bandwidth** | 20 and 80 MHz; bands 2.4 GHz and 5 GHz; channels 11, 13, 40 |
| **NICs / tools** | Broadcom BCM43455C0 (Nexmon CSI Extractor, NCE); Atheros QCWB335 / AR5822 (Atheros CSI Tool, ACT) |
| **Traffic** | FTP, Iperf3, HT-sounding packets |
| **Receivers** | Up to 4 (MC1), 1 (MC2), 3 (MC3) |
| **Applications** | People counting (PC), human activity recognition (HAR), machine recognition (MR), machine activity recognition (MAR), person identification (PI); multiclass + multilabel |
| **Licence** | Paper: CC BY-NC-ND 4.0. Dataset on Figshare — licence to re-verify before redistribution. |
| **Access** | Figshare download; single .zip with `.mat` files |

#### Novelty relative to Aquila

EHUNAM is the **first public CSI dataset to include machine activity recognition (MAR)** and machine recognition (MR) alongside human sensing, including an **industrial environment** (UNAM mechanical design laboratory with lathes, milling machines, drill presses, cutting saws). It also provides **multilabel PC+MAR** (human + machine simultaneously). This is directly relevant to Aquila's v1/v2 scope because:

- **Machine activity recognition** is explicitly out of v1 scope (`CONTEXT.md`: inference beyond scope lists fall detection, etc.; MAR not mentioned but is beyond the human-sensing bounded context).
- **Industrial environment** is outside Aquila's "home" bounded context (one calibrated dwelling).
- **Broadcom/Atheros NICs + Raspberry Pi / laptops** — not ESP32-S3. Domain adaptation would be mandatory (v2 synthesis §5: "these are L-CSI from Intel/Atheros NICs — domain adaptation is mandatory; none is ESP32-S3 CSI").
- **CC BY-NC-ND** — the ND (no-derivatives) clause is stricter than Aquila's CC BY-NC usage; Aquila could use it as an isolated research benchmark but could not ship derived models under GPLv3-compatible terms without re-derivation.

#### Relevance score for Aquila: LOW-MEDIUM (research benchmark only; not ESP32-S3; not home context; ND licence restricts derivative use)

---

### 2.2 CSI-Bench — large-scale in-the-wild WiFi sensing benchmark

| Attribute | Value |
|-----------|-------|
| **Source** | arXiv:2505.21866 (v2, 20 Nov 2025); accepted at NeurIPS 2025 (39th conference). Authors: Zhu, Hu, Gao, Wang, Wang, Liu. DOI: 10.48550/arXiv.2505.21866. |
| **Repository** | GitHub: Jenny-Zhu/CSI-Bench-Real-WiFi-Sensing-Benchmark; project page: ai-iot-sensing.github.io |
| **Size** | Not explicitly stated in GB; **461 hours** of effective CSI data |
| **Users** | 35 unique users |
| **Environments** | 26 diverse indoor environments |
| **Devices** | 16 device types (commercial WiFi edge devices: routers and edge devices, e.g. Apple HomePod, Google Nest) |
| **Tasks** | Fall detection, breathing monitoring, localisation, motion source recognition; co-labeled multitask dataset (joint annotations: user identity, activity, proximity) |
| **Features** | **Amplitude-only CSI** (phase instability across platforms limits exploration) |
| **Licence** | Paper: CC BY-NC-ND 4.0. Repo: MIT. Data licence: unverified. |
| **Access** | GitHub repo (code); data access to verify |

#### Novelty relative to Aquila

CSI-Bench is the most directly comparable large-scale benchmark to what Aquila needs:

- **In-the-wild, continuous daily activity** — matches Aquila's "home" context far better than lab datasets.
- **Breathing monitoring** — directly relevant to Aquila's vitals bar (breathing MAE ≤ 2 brpm).
- **Fall detection** — explicitly out of v1 scope (`CONTEXT.md`), but a v2/Track consideration.
- **Localisation** — relevant to v2's advisory 3D position goal.
- **Commercial edge devices** — not ESP32-S3; domain adaptation mandatory.
- **Amplitude-only features** — aligns with v2's `Coherence::Uncoherent` route (no cross-node phase), but Aquila's v1 signal pipeline does use phase (per-node, within-link). The phase-instability finding corroborates Aquila's decision to avoid cross-node phase coherence.
- **CC BY-NC-ND paper / MIT repo / unverified data** — data licence unverified; Aquila must re-verify before any use.

#### Relevance score for Aquila: HIGH (best structural match to v2's learned, placement-structured, in-the-wild route; but device mismatch and licence unverified)

---

### 2.3 CSI-BFI-HAR — CSI + Beamforming Feedback Information dataset

| Attribute | Value |
|-----------|-------|
| **Source** | IEEE DataPort. DOI: 10.21227/nbp8-7r47. Author: Khandaker Foysal Haque (Northeastern University). Created 2025-05-01; updated 2026-05-28. |
| **Repository** | IEEE DataPort (login required); GitHub: kfoysalhaque/CSI-BFI-HAR, BeamSense, SiMWiSense; HuggingFace mirror: foysalhaque/CSI-BFI-HAR-Dataset |
| **Size** | ~500 GB CSI + ~700 GB BFI ≈ **1.2 TB total** (12 tar files: HAR-1.tar 25.24 GB through HAR-12.tar 21.76 GB) |
| **Subjects** | Single-subject: 6 subjects, 20 activities, 6 environmental setups. Multi-subject: 3 individuals performing distinct actions concurrently. |
| **Environments** | LoS and NLoS scenarios, across multiple days |
| **Devices** | Commercial IEEE 802.11ac devices |
| **Data types** | **CSI + BFI** (first public HAR dataset with both); BFI = compressed representation of beamforming feedback, easily collectible without firmware modification |
| **Licence** | To verify (IEEE DataPort terms) |
| **Access** | Login required to download dataset files |

#### Novelty relative to Aquila

CSI-BFI-HAR is notable for including **Beamforming Feedback Information (BFI)** alongside CSI:

- BFI is a compressed surrogate for CFR/CSI that can be retrieved from commodity Wi-Fi without firmware modification (see Wi-BFI tool, Haque 2023, DOI 10.1109/... cited in research). This is relevant to Aquila because:
  - Aquila's nodes are ESP32-S3 using `esp-csi` (raw CSI). BFI would require different NICs (Intel/Atheros with beamforming feedback support) — **not ESP32-S3**.
  - The v2 synthesis notes BFI as a "newly explored and easily collectible sensing primitive" but does not integrate it into the v2 route.
- **First public CSI+BFI dataset** — valuable for research comparing CSI vs BFI as sensing primitives, but outside Aquila's hardware bounded context.
- Multi-subject simultaneous activities (3 people, distinct actions) — relevant to person count and multi-person ambiguity, which v1 flags as an open question (Q-05 / R-04).

#### Relevance score for Aquila: LOW (BFI not on ESP32-S3; different NICs; but useful as a research comparison point for CSI-vs-BFI)

---

### 2.4 Widar 3.0 — WiFi-based gesture recognition dataset

| Attribute | Value |
|-----------|-------|
| **Source** | ACM MobiSys 2019 ("Zero-Effort Cross-Domain Gesture Recognition with Wi-Fi", Zheng et al., 717 citations). IEEE DataPort open access. DOI: 10.21227/7znf-qp86. Homepage: tns.thss.tsinghua.edu.cn/widar3.0/ |
| **Size** | **258,000 hand-gesture instances**; 8,620 minutes total (~143.7 hours) |
| **Users** | 16 users |
| **Environments** | **75 domains** |
| **Gestures** | 9 hand gestures |
| **Features** | RSSI + CSI from commodity WiFi NICs; Doppler Frequency Shift (DFS); Body-coordinate Velocity Profile (BVP) |
| **Licence** | IEEE DataPort open access (free, login required) |
| **Access** | IEEE DataPort open access; homepage |

#### Novelty relative to Aquila

Widar 3.0 is the largest public WiFi gesture dataset and the canonical cross-domain benchmark:

- **Cross-domain (75 environments)** — directly relevant to Aquila's cross-room/cross-home degradation questions (v1 Q-01 / R-01; v2 cross-site transfer Site A→Site B).
- **Gesture recognition** — out of v1 scope (CONTEXT.md: gesture not in inferred outputs; pose is coarse keypoints only). Relevant to v2 only if gesture were ever added (it is not in the v2 destination).
- **DFS and BVP features** — Doppler and body-coordinate velocity profile are signal-processing features Aquila's pipeline does not currently compute (v1 signal.md does spectrogram/BVP-style tracking for vitals, but not gesture-specific DFS/BVP).
- **Commodity WiFi NICs** — not ESP32-S3; domain adaptation mandatory.
- **Open access** — most accessible of the IEEE DataPort datasets.

#### Relevance score for Aquila: MEDIUM (cross-domain benchmark value; gesture out of scope; device mismatch)

---

### 2.5 Awesome-WiFi-CSI-Sensing — curated repository collection

| Attribute | Value |
|-----------|-------|
| **Source** | GitHub: NTUMARS/Awesome-WiFi-CSI-Sensing (~1.1k stars); topic: channel-state-information |
| **Contents** | Curated collection of open-source datasets, extraction tools (Nexmon CSI Extractor, ESP32 CSI tools, Atheros CSI Tool), and deep-learning libraries |
| **Key tools indexed** | Nexmon CSI Extractor (seemoo-lab/nexmon_csi — up to 256 subcarriers, 80 MHz, Broadcom/Raspberry Pi), ESP32-CSI-Tool, Atheros CSI Tool, RuView, Wi-Fi sensing surveys |
| **Key datasets indexed** | MM-Fi (NeurIPS 2023 D&B, CC BY-NC 4.0), and others |
| **Licence** | Repository-level; individual items carry their own licences |

#### Novelty relative to Aquila

This is a **discovery index**, not a dataset itself. Its value to Aquila:

- **Nexmon CSI Extractor** — the same tool family EHUNAM used (MC1/MC3 with Broadcom BCM43455C0 on Raspberry Pi). Aquila uses `esp-csi` on ESP32-S3; Nexmon is a different capture path (Broadcom on Linux/Raspberry Pi).
- **ESP32-CSI-Tool** — directly relevant to Aquila's ESP32-S3 node firmware (capture-and-firmware.md §9: esp-csi via `wifi_csi_config_t`, `esp_wifi_set_csi_rx_cb`).
- **RuView** — already in Aquila's sources.json (ruvnet/RuView, MIT); the signal.md spec "borrows the genuinely sound parts of RuView — phase sanitisation, bandpass/resonator filtering, autocorrelation-style periodicity."
- **MM-Fi** — already in Aquila's sources.json (CC BY-NC 4.0); used for pose fine-tuning in v1 inference pipeline.

#### Relevance score for Aquila: MEDIUM (discovery value; confirms tool/dataset landscape; no new primary data)

---

### 2.6 Wi-Fi CSI Object Detection — Kaggle dataset

| Attribute | Value |
|-----------|-------|
| **Source** | Kaggle: armandokeller/wifi-csi-object-detection |
| **Contents** | Raw and normalized CSI amplitude and phase columns for indoor object detection |
| **Licence** | MIT |
| **Access** | Kaggle (requires account) |

#### Novelty relative to Aquila

- **MIT licence** — permissive, compatible with Aquila's GPLv3 (code and weights licensed separately; GPLv3 covers source, weights carry their own terms).
- **Object detection** — out of Aquila's scope (CONTEXT.md: inference beyond scope includes identity/recognition/gait/re-identification/fall detection; object detection not listed but is beyond human-sensing bounded context).
- **Amplitude and phase columns** — relevant to Aquila's signal conditioning discussions (phase handling, normalization).
- Small-scale / single-source — not a large benchmark; limited generalisation value.

#### Relevance score for Aquila: LOW (MIT licence is a plus; object detection out of scope; small scale)

---

### 2.7 Wi-Fi CSI Human Activities — HuggingFace Master's thesis dataset

| Attribute | Value |
|-----------|-------|
| **Source** | HuggingFace: aselya9185/wifi-csi-human-activities |
| **Contents** | CSI measurements from indoor office environments; complete signal processing and machine learning pipeline |
| **Context** | Master's thesis dataset |
| **Access** | HuggingFace |

#### Novelty relative to Aquila

- **Indoor office environments** — matches Aquila's "home" / indoor context reasonably well.
- **Complete processing pipeline** — potentially useful as a reference implementation for DSP + ML, but thesis-scale (limited subjects/environments).
- Licence and scale to verify on HuggingFace.

#### Relevance score for Aquila: LOW-MEDIUM (pipeline reference value; small scale; licence to verify)

---

## 3. Comparative summary table

| Dataset | Size | Duration | Users/Subjects | Environments | Key tasks | Hardware | Licence | Access | Aquila relevance |
|---------|------|----------|----------------|--------------|-----------|----------|---------|--------|------------------|
| EHUNAM | 72.39 GB | ~38 h / 23 days | 21 people, 9 machines | 8 (incl. industrial) | PC, HAR, MR, MAR, PI | Broadcom/Atheros NICs, Raspberry Pi | CC BY-NC-ND (paper); Figshare (verify) | Figshare download | LOW-MEDIUM (MAR novel but industrial, not ESP32-S3, ND licence) |
| CSI-Bench | — | 461 h | 35 users | 26 | Fall, breathing, localisation, motion source | 16 commercial edge devices | CC BY-NC-ND (paper); MIT (repo); data unverified | GitHub | HIGH (best match to v2 learned/in-the-wild route; device mismatch) |
| CSI-BFI-HAR | ~1.2 TB | — | 6 (single) / 3 (multi) | Multiple (LoS/NLoS) | HAR (20 activities) | Commercial 802.11ac | To verify | IEEE DataPort (login) | LOW (BFI not on ESP32-S3) |
| Widar 3.0 | — | ~143.7 h | 16 users | 75 domains | Gesture (9) | Commodity WiFi NICs | IEEE DataPort open | IEEE DataPort | MEDIUM (cross-domain benchmark; gesture out of scope) |
| Awesome-WiFi-CSI-Sensing | — | — | — | — | — | Various | Mixed | GitHub | MEDIUM (discovery/index value) |
| Wi-Fi CSI Object Detection | — | — | — | — | Object detection | — | MIT | Kaggle | LOW (out of scope) |
| Wi-Fi CSI Human Activities | — | — | — | Office | HAR | — | To verify | HuggingFace | LOW-MEDIUM (pipeline reference) |

---

## 4. Licence posture for Aquila (non-commercial, GPLv3)

Per `docs/README.md` §Conventions and `inference-models.md` §6:

- **Code and weights are licensed separately.** GPLv3 covers the source. NC datasets/weights carry their own NC terms; NC is a use restriction, not copyleft; it neither relicenses GPLv3 code nor is laundered off the weights by it.
- **Derivative weights stay NC.** A model seeded from NC weights and/or fine-tuned on NC data is a derivative and inherits NC: it cannot be offered under GPLv3 alone and cannot be used commercially.
- **Keep NC artefacts out of GPLv3 history.** Dataset bytes and NC weight files live in a separate artefact store, not committed to git. The repo commits code, configs, small owned fixtures, and model cards (which record NC provenance). NC-derived `.onnx` payloads ship under a `MODEL-LICENCE` file stating NC, alongside GPLv3 code.
- **The card carries the truth.** Every model card has `licence` (weights) and `commercial_use: false`; the loader refuses a card missing either.
- **Permissively licensed CSI+camera pose for commercial use: none found** (paired-csi-datasets.md §Findings).

**Dataset-specific licence assessment:**

| Dataset | Paper licence | Data licence | Verdict for Aquila |
|---------|--------------|--------------|-------------------|
| EHUNAM | CC BY-NC-ND 4.0 | Figshare (verify) | ND is stricter than Aquila's NC usage; research benchmark only; do not ship derived models |
| CSI-Bench | CC BY-NC-ND 4.0 | Unverified | Research benchmark only; verify data licence before any use |
| CSI-BFI-HAR | To verify | To verify (IEEE DataPort) | Do not use until licence verified |
| Widar 3.0 | Open access (IEEE DataPort) | Open access | Usable as research benchmark; gesture out of scope |
| Awesome-WiFi-CSI-Sensing | Mixed | Mixed | Index only; check each item |
| Wi-Fi CSI Object Detection | MIT | MIT | Usable; object detection out of scope |
| Wi-Fi CSI Human Activities | To verify | To verify | Pipeline reference only |

---

## 5. Connection to the invention work (Jennison closure phase / coherent mesh)

The invention work in `/tmp/opencode/inv/` (30 sessions, ~14 MB, inventions a–t) is highly relevant to both the patent application already drafted (`docs/patent/`) and the v2 spec direction. Key connections:

### 5.1 The observability boundary is the crux

The v2 synthesis (§1) and multiple invention notes converge on the same boundary:

- **U(1) gauge:** with independent LOs, absolute cross-node phase is unobservable; only relative phases within a gauge survive.
- **Three escapes:** (1) common-transmitter reference, (2) OTA reciprocity (`ĥ_AB·ĥ_BA⁻¹`), (3) shared/conducted clock+tone. None proven on ESP32-S3 wireless-only.
- **M2 (per-packet cross-node phase residual)** is the decisive measurement — the whole wireless-only question turns on it.

### 5.2 Closure-phase mesh inventions (most relevant to the patent)

The patent application (`docs/patent/jennison-closure-phase-wifi-mesh.md`) claims Jennison closure phase across directed multi-node packet loops for 3D scatterer tracking without cross-node phase coherence. The invention notes confirm this is **genuinely novel territory**:

- **Invention h ("The Closure Weave"):** Tier A = closure-phase sensing around directed triangles of mesh links. Each node appears once as TX and once as RX, so LO phases cancel. **No primary source found applying closure/bispectrum/self-cal to Wi-Fi/WLAN/5G** (invention o searched heavily). Nearest neighbours: VLBI closure/self-cal (Jennison 1958; Cornwell & Wilkinson 1981), incoherent Wi-Fi imaging, TDD reciprocity calibration, LoSRef.
- **Invention o ("The Closure Lattice"):** Run the mesh as a radio interferometer; every node TX+RX; hub correlates into cross-node visibilities; closure phases `Φ_ijk = arg(V_ij·V_jk·V_ki)` cancel every per-node LO/RF-chain phase term exactly at every subcarrier. Independent closures = (N−1)(N−2)/2 per frequency. Self-cal identifiable when N(N−1)/2 > 2N−1 (VLBI condition).
- **Invention s ("The Reciprocal Gauge Mesh"):** Mode 0 gauge-free (two-way product = self-contained coherent bistatic link; doubled phase/Doppler); Mode 1 geometry-fused; Mode 2 manufactured coherence.
- **Invention t ("The Coherent Star"):** Conducted-coherent polyhedron broadcasts reference frames; every node re-fixes LO phase per received frame (common-TX escape, O(1) airtime). Explicitly imports closure phases/phase referencing/self-cal from radio astronomy — "not found applied to Wi-Fi sensing."

### 5.3 Prior art relevant to novelty/obviousness

| Prior art | Relevance | Gap vs invention |
|-----------|-----------|------------------|
| Jennison 1958 (VLBI closure phase) | Foundation of closure phase | Radio astronomy, not Wi-Fi; coherent arrays observing common source |
| Cornwell & Wilkinson 1981 (self-cal) | Closure + self-calibration | Radio interferometry |
| WiDRa (JSAC 2024, arXiv:2405.12168) | Sum-carrier-phase differential ranging, <1 mm RMSE COTS | Round-trip phase, not closure phase across loops |
| RTCSI / ML-Track / Chronos | Two-way CSI products, CFO cancellation | Two-way products, not loop closure |
| LoSRef (arXiv:2602.05344) | LoS path as OTA delay+phase reference | Single link, not multi-node loops |
| ESP-PPB (esp-ppb, C3, GPL-3.0) | Wireless phase-coherent CSI via FTM+VCTCXO | Common reference frame, not closure phase |
| ESPARGOS | Conducted 40 MHz + Wi-Fi phase reference over coax | Conducted reference, not wireless closure |
| 802.11bf WLAN sensing | Standard (published 2025) | No phase-coherence mechanism; no conformant commodity silicon as of 2026 |

**Novelty assessment:** The combination of (i) directed packet loops in an incoherent Wi-Fi mesh, (ii) Jennison closure-phase cancellation of per-node clock/LO error, and (iii) 3D scatterer tracking from multi-loop closure-phase geometry has **no primary source found**. This supports the patent's novelty position.

### 5.4 Relevance to v2 route

The v2 map demotes coherent apertures, cross-node AoA, and coherent ToA to Track 3 research. The invention work explores exactly that Track 3 territory:

- **Tier A inventions (h, o, s):** Closure-phase and reciprocal-gauge mesh — receive-only or low-duty TX; no cross-node coherence needed; gauge-free observables. These **do not require** the cross-node coherence that v2 demotes. They could potentially enhance v2's `Coherence::Uncoherent` route with closure-based scene constraints.
- **Tier B/C inventions (i, t, n, q):** Conducted-coherent polyhedron hubs, coherent TX, multi-puck coherence — these **do require** the O(N) reference that v2 places in Track 3.
- **Invention m ("Ambient-Coherent Aperture"):** Coherence inherited per frame from incumbent (non-cooperative) OFDM traffic; no cable, no exchange, no disciplined oscillator; receive-only. This is the **most compatible with v2** — it works with ambient traffic and no cross-node phase coherence.

### 5.5 Decisive experiments relevant to both patent and v2

From the invention notes and v2 synthesis (§6), ranked cheapest-decisive first:

1. **Relative-LO coherence & reciprocity residual on two S3s** (vs conducted-reference ground truth). Gates every coherent method. Falsified if residual grows unbounded / is statistically random. (v2 Exp 1; invention h E1, i E1, k E-1)
2. **Closure-phase gate** (invention o): 4× ESP32-S3; one transmits ESP-NOW HT20; three receive; compute closure phases over 10 min. Tests: invariance across AGC/reboot/channel; predicted closure shift from moved metal plate; walking-person signature. Cost <$40, one day. **This directly tests the patent's core claim.**
3. **Nanosecond RX-timestamp characterisation** (v2 Exp 2): latch latency, 12.5 ns grid, jitter, forced re-lock phase RMSE.
4. **Multi-node fingerprinting/RTI accuracy vs node count vs geometry on S3** (v2 Exp 4; invention m E3).
5. **FTM self-survey** (v2 Exp 5): 6–8 nodes, all-pairs, MDS + Procrustes without anchors.
6. **Ambient ledger test** (invention m E1): 2 S3s CSI-capturing the same third-party frames; scored vs coax-splitter oracle. Pass σ(Δψ̂−oracle) ≤ 0.2 rad.

---

## 6. Recommendations for Aquila

### 6.1 Datasets

1. **CSI-Bench** is the highest-value external dataset for v2: in-the-wild, 461 hours, 35 users, 26 environments, breathing + localisation + fall tasks. **Action:** Verify data licence (currently unverified) before adding to `sources.json`; consider as a cross-domain evaluation holdout analogous to v2's Site B.
2. **Widar 3.0** is the best cross-domain gesture benchmark (75 domains) but gesture is out of scope. **Action:** Monitor for cross-domain transfer lessons only; do not add to scope.
3. **EHUNAM** is novel for machine activity recognition (MAR) but industrial context + Broadcom/Atheros NICs + CC BY-NC-ND put it outside Aquila's bounded context. **Action:** Research benchmark only; do not add to `sources.json` as a v1/v2 input.
4. **CSI-BFI-HAR** introduces BFI as a sensing primitive but requires non-ESP32-S3 NICs. **Action:** Track as a research comparison point (CSI vs BFI); not for v1/v2.
5. **Awesome-WiFi-CSI-Sensing** is a discovery index. **Action:** Use to track Nexmon/ESP32 CSI tool developments; no dataset ingestion.

### 6.2 Patent + invention integration

The Jennison closure-phase patent application (`docs/patent/`) is technically well-positioned:

- **Novelty:** No primary source found applying closure phase to Wi-Fi sensing mesh (invention o searched heavily).
- **Boundary-consistent:** The invention works within the U(1) gauge constraint (closure cancels per-node LO phases) — it does not claim unobservable absolute cross-node phase.
- **Hardware-compatible:** Claims 1–8 are receivable on stock ESP32-S3 (esp-csi, promiscuous HT20, ESP-NOW, ns RX timestamps) with no coherent TX needed — matching invention h's Tier A and invention o's receive-only mode.
- **Recommended next step:** Execute invention o's Closure Gate experiment (4× ESP32-S3, <$40, one day) to generate measured evidence before filing. This would upgrade the patent's "Claimed" evidence grade toward "Measured."

### 6.3 v2-spec alignment

The v2 route (learned, placement-structured, `Coherence::Uncoherent`, camera-free runtime, per-home adaptation via git PR) is **consistent with the research synthesis**. The datasets that best support it are CSI-Bench (in-the-wild benchmark) and Aquila's own captured Sessions (the git-PR contribution model). The invention work (closure phase, reciprocal gauge, ambient-coherent aperture) represents **Track 3 / enhancement territory** that could future-proof v2 but is not required for the v2 destination.

---

## 7. Sources consulted

All primary sources where accessible:

- EHUNAM paper: PMC12711874; Scientific Data DOI 10.1038/s41597-025-06238-4; Figshare 28541225
- CSI-Bench: arXiv 2505.21866; NeurIPS 2025 poster 121605
- CSI-BFI-HAR: IEEE DataPort DOI 10.21227/nbp8-7r47
- Widar 3.0: MobiSys 2019; IEEE DataPort DOI 10.21227/7znf-qp86
- Awesome-WiFi-CSI-Sensing: GitHub NTUMARS/Awesome-WiFi-CSI-Sensing
- Wi-Fi CSI Object Detection: Kaggle armandokeller/wifi-csi-object-detection
- Wi-Fi CSI Human Activities: HuggingFace aselya9185/wifi-csi-human-activities
- Aquila v1: docs/reference/sources.json (existing entries: mmfi, widar3, csibench, ruvnet, esp-csi, espectre, espargos, openmac, ruview, etc.)
- Aquila v2: docs/research/v2/00-synthesis.md, BRIEF.md, docs/plan/v2/map.md, docs/research/paired-csi-datasets.md
- Invention notes: /tmp/opencode/inv/NOTES.md, NOTES2.md (inventions a–t)
