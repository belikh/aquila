# 02 — Multi-node passive localisation and fingerprinting

**Prompt.** `02-multinode-passive-localisation`. With several passive receivers,
how do CSI/RSSI fingerprints localise uncooperative people, and what accuracy do
device-free methods achieve? Must contain: (a) radio tomographic imaging (RTI)
formulation; (b) fingerprinting vs model-based; (c) node count/placement effects;
(d) NLoS and multi-person ambiguity; (e) calibration burden. Leads: Wilson &
Patwari (RTI), E-eyes, CARML.

Australian English. All claims labelled **ESTABLISHED / HYPOTHESIS / SPECULATION**.
Hardware tier stated per claim. Sources are primaries (papers, upstream repos,
datasheets).

> Note on the brief's lead "CARML": I could find no primary source by that name.
> The intended reference is almost certainly **CARM** — "CSI-based human Activity
> Recognition and Monitoring" (Wang et al., MobiCom 2015, DOI
> `10.1145/2789168.2790093`) — a *model-based* activity-recognition system whose
> two models (CSI-speed and CSI-activity) are the closest match. I treat CARM as
> the lead and flag the naming uncertainty below.

---

## Question

Given a set of fixed passive receivers that observe ordinary 802.11 traffic, how
much can we infer about the position of one or more people who carry no device?
What does *multi-node* add over the single-node case (prompt 01), which deployment
parameters dominate accuracy, and what does Aquila's ESP32-S3 fleet let us do that
a PC-class network card does not — and vice versa?

---

## Findings

### ESTABLISHED

**1. The RTI linear model (the canonical formulation).** Wilson & Patwari
(IEEE TMC 2010, `10.1109/TMC.2009.174`; preprint `span.ece.utah.edu/uploads/RTI.pdf`)
express the shadowing loss on each link as a sum of attenuations over spatial
voxels:

```
y_i(t) = Σ_j w_ij · x_j(t) + n_i(t)        (single link)
y = W x + n                                (all links, matrix form)
Δy = W Δx + n                              (differenced against a baseline)
```

- `y` — vector of `M` RSS measurements (in **dB**), one per link.
- `x` — the attenuation image, `N` voxels to estimate.
- `W` — `M×N` weighting matrix: each **column** is a voxel, each **row** is a link.
- `w_ij` — voxel `j`'s weight on link `i`. The paper's model sets
  `w_ij = 1/√d_i` when voxel `j` lies inside a **normalised ellipse** with foci at
  the two nodes, and `0` otherwise. (The normalisation and the inverse-square-root
  of link distance are the two empirical choices the paper validates.)
- Differencing against a submitted baseline (`Δy = y − ȳ`) removes the static
  multipath and leaves motion-induced change.

**2. RTI reconstruction is an ill-posed inverse problem solved by regularisation.**
Least squares (`x̂ = (WᵀW)⁻¹Wᵀy`) is unusable because `WᵀW` is near-singular.
Wilson & Patwari derive a Gaussian-noise MAP estimator, equivalent to **Tikhonov
H1 (smoothness) regularisation**:

```
x̂ = (WᵀW + α(DᵀₓDₓ + DᵀᵧDᵧ))⁻¹ Wᵀy
```

where `Dₓ, Dᵧ` are horizontal/vertical difference operators and `α` is the
regularisation weight. The solution is a **linear transform of the measurements**,
so reconstruction is cheap. Complexity grows **linearly in voxels, quadratically
in nodes** (since `M ≈ K(K−1)/2` links for `K` nodes).

**3. Node density and geometry dominate accuracy — and this is quantified.**
The same paper derives a **Cramér-Rao-type lower bound on image mean-squared
error** as a function of node geometry, and plots it against node density. Key
established results:
- Average MSE falls **rapidly** with increasing node density, then saturates.
- A **square perimeter** deployment beats front-back and random deployments of the
  same node count: surrounding the area ensures few voxels are crossed by no links.
- **Rail edges and corners** (few crossing links) are the least accurate regions;
  well-crossed central voxels are most accurate.
- Their 28-node experiment over a 441 ft² area gave single-person squared error
  0.021 and two-person 0.036 (on the cylindrical human model).

**4. Variance-based (motion) RTI and through-wall tracking are demonstrated.**
Wilson & Patwari (IEEE TMC 2011, `10.1109/TMC.2010.175`; arXiv:0909.5417) replace
mean attenuation with **motion-induced RSS variance** on each link, build a motion
image, and run a **Kalman filter** to track a moving target through a wall — a
**34-node, 780 ft²** deployment. This is the origin of the "amplitude/Doppler,
blind to static scatter" theme in prompt 01, applied across a network.

**5. Fingerprinting is the dominant *device-free* CSI method and reaches
sub-metre accuracy.** A CSI fingerprint maps a high-dimensional per-subcarrier
(and per-antenna, per-link) measurement to a labelled grid location; online it is
matched by kNN / SVM / CNN. Representative primaries:
- **E-eyes** (Wang et al., MobiCom 2014, `10.1145/2639108.2639143`): device-free
  *activity/location* identification with **CSI amplitude signatures from a single
  AP**; >96% TPR, <1% FPR across two apartments. It distinguishes *in-place* vs
  *walking* activities, i.e. coarse location + activity, not continuous tracking.
- **SVM + CSI fingerprints** (Zhou et al., IEEE Sensors J. 2017,
  `10.1109/JSEN.2017.2762428`): >97% presence detection; localisation error
  **1.22 m / 1.39 m** in two test areas.
- **ESP32-specific**: a 2026 preprint (DOI `10.20944/preprints202601.2378.v1`)
  deploys **three ESP32 transceivers** in a 6 m × 4 m room, 100 Hz CSI, 35 grid
  points, >1.2 M labelled snapshots, and reports **median error 0.45 m, 90th
  percentile <0.9 m**, ~93% grid-cell classification, sub-10 ms inference on a
  Raspberry Pi. This is the closest published ESP32 analogue of what Aquila wants.

**6. Model-based (calibration-light) DFL is a real alternative.** Instead of a
radio map, geometric propagation models relate the measured perturbation directly
to target position, needing **no offline training**:
- **LiFS** (Wang et al., IEEE TMC 2018; `10.1109/TMC.2017.2774296`) localises with
  fine-grained CSI **without offline training** by selecting subcarriers that are
  *not* corrupted by multipath, and using a geometric (Fresnel-zone) relationship.
- **Fresnel-zone model** (FZM) localisation (e.g. MFDL, UbiComp 2017 demo,
  `10.1145/3123024.3123172`) estimates which Fresnel ellipsoid a target occupies
  from CSI phase; MFDL claims **decimetre** accuracy.
- **CSI-ratio model** (Zeng et al., "FarSense", IMWUT 3(3), 2019,
  `10.1145/3351279`; survey `10.1007/s42486-021-00077-z`) divides the CSI of two
  closely-spaced receive antennas to cancel common phase/amplitude noise — the
  standard trick for making phase usable on COTS hardware.
- **Exponential-Rayleigh model** for RSS-based DFL/tracking (Zheng et al., 2015)
  extends the first-Fresnel-zone attenuation model and needs less training.

**7. Multi-person is the hard wall, and the difficulty is fundamental, not just
engineering.** A dedicated survey (ACM Computing Surveys 2020, `10.1145/3396302`)
states that multiple co-present people cause **ambiguity in both the measurement
and target identity**. Concrete primaries:
- **RSS linear-superposition fails.** Most methods assume multi-body shadowing
  sums linearly with the number of people; the EM model of multiple bodies
  (Sensors 2021, 21(5):1728) shows the **mutual EM interaction** between bodies
  breaks that assumption.
- **Ambiguous heat maps.** UWB channel-impulse-response DFL (Sensors 2020, PMC9416598)
  shows multiple local maxima; the highest peak is not always the true position,
  and combining links can place the estimate at the *wrong* ambiguity.
- **RTT multi-person** (TimeSense, arXiv:2409.00030) reaches median **1.57 m /
  2.65 m** in two testbeds (better than RSSI baselines by 49–103%), but accuracy
  degrades with each added person and with fewer LoS links (blind spots).
- **Counting is easier than locating.** Crowd-counting from CSI with a DNN
  (Zhou et al., Wireless Networks 2020, `10.1007/s11276-020-02274-7`) is within
  ±1 person for crowd sizes 6–34 (97.7% of cases at size 34); Wi-CaL (Choi et al.
  2022) reports **0.35 MAE counting and 91.4% localisation with five people**.
  Count ≈ known; precise per-person coordinates ≈ unresolved.

### HYPOTHESIS

**8. Multi-node *amplitude* RTI/fingerprinting is the unblocked ESP32-S3 path;
multi-node *phase* (coherent AoA/imaging) is blocked without a shared reference.**
The established results above (RTI, E-eyes, ESP32 DFL, crowd counting) all use
**amplitude or RSS-variance** and survive independent local oscillators, because
each receiver only needs its own (arbitrary) phase reference and amplitude is
LO-independent. Any method needing the *phase relationship between nodes*
(CSI-ratio across a distributed array, coherent DoA, multi-static imaging) needs a
common reference (BRIEF assumption; prompts 10–12, 17). I therefore predict:
multi-node amplitude DFL on a 6–20-node S3 fleet will land in the published
0.4–1.5 m band and improve roughly as the RTI MSE bound predicts, whereas
volumetric 3D DoA from a soldered polyhedron will **not** work on S3 without one
of the three escapes. The 2026 three-link ESP32 result (0.45 m median) is a data
point consistent with this; it has not yet been replicated at higher node counts.

**9. A compact volumetric aperture (polyhedron) improves *geometry diversity* for
amplitude imaging even without phase coherence.** A 20-face icosahedron packed
with single-antenna nodes places links on many non-coplanar orientations; for
RTI's ellipse-weighted model this adds crossing links in the vertical dimension,
which a flat perimeter cannot (Wilson & Patwari's bound is 2D; the 3D extension
would reduce the "few-crossing-voxel" regions). This is *hypothesis*, not
demonstrated for ESP32, but follows directly from the established density/geometry
result. It does **not** require phase coherence because RTI is amplitude-based.

**10. Calibration burden can be shifted, not eliminated.** Fingerprinting needs a
site survey; RTI needs a vacant baseline; model-based needs geometry knowledge.
Transfer learning reduces the survey: few-shot GNN transfer reaches 85–87%
(10-shot) vs 95.8% with 400 labelled samples/location — a **>40× label
reduction** (arXiv:2201.12656); unsupervised domain adaptation across time periods
and environments is demonstrated (arXiv:2502.07246; Sensors 2021, 21(3), TCA-based).
Zero-calibration participatory sensing (RadioGrapher, arXiv:1909.03551) builds the
device-free radio map from crowd-sourced device-based positions using Fresnel
zones. I hypothesise that an ESP32 fleet's **stable, fixed node positions** make
RTI's differenced model (*relative* change) lower-calibration than fingerprinting,
at the cost of requiring a vacant baseline per deployment.

**11. The round-trip ping-and-unique-reply scheme measures range, identity,
channel and coarse offset — but not sub-metre range on the bare MAC timestamp.**
One node pings; every other node replies with a unique packet; round trips are
timed. What each observation gives:
- **Identity / liveness**: *which* node replied (unique payload, MAC) — trivial
  and robust.
- **Channel**: the CSI of each reply on each link — the raw material for
  amplitude RTI/fingerprinting.
- **Doppler**: temporal CSI change (moving only) — prompt 01.
- **Range / clock-offset**: RTT → distance via `d = c·RTT/2`. Two-way exchange
  cancels the *offset* between the two clocks, which one-way ranging cannot.
- **Where it breaks**: (i) the ESP32 MAC exposes only a **microsecond RX
  timestamp** (BRIEF); 1 µs of round-trip time is ~300 m of path, so bare
  timestamping gives essentially no ranging resolution. Sub-metre needs ~6.7 ns,
  which is why FTM uses dedicated fine-timing hardware. (ii) **Processing turn-
  around delay** adds to RTT and must be calibrated out. (iii) **Multipath bias**
  inflates range (over-estimate, not over/under). (iv) With many nodes replying,
  **contention/collisions** and **half-duplex** serialise the round trips. (v) No
  common phase reference means the *phase* of replies is not comparable across
  nodes. So the scheme is a strong identity+channel+Doppler primitive and a weak
  coarse-ranger; it is the natural way to *gather* CSI for localisation, not to
  localise on its own.

### SPECULATION

**12. A purpose-built ESP32-S3 multi-node dataset could probe the node-count/
placement scaling curve that no public dataset covers for this hardware.**
Every published ESP32 device-free result I found uses **three links** or a single
room. No public ESP32 dataset varies node count, geometry (planar vs volumetric)
and occupancy together with ground truth. A 12–20-node S3 deployment could yield
the first ESP32-specific `accuracy vs node count vs geometry` surface, directly
testing Wilson & Patwari's bound on low-cost hardware. Speculative because it may
only reproduce known COTS behaviour; its value is the *hardware-specific* curve
and calibration-transfer cost, not new theory.

---

## Must-contain checklist

| Requirement | Where | Status |
|---|---|---|
| (a) RTI formulation | Findings §1–2 | ESTABLISHED |
| (b) Fingerprinting vs model-based | §5 vs §6 | ESTABLISHED |
| (c) Node count / placement effects | §3, §9 | ESTABLISHED / HYPOTHESIS |
| (d) NLoS and multi-person ambiguity | §4 (through-wall), §7 | ESTABLISHED |
| (e) Calibration burden | §10 | HYPOTHESIS |

---

## ESP32-S3 feasibility

**What is demonstrated on ESP32/ESP32-S3 specifically.**
- **CSI is available on the whole ESP32 family** including S3 (Espressif `esp-csi`;
  `esp-csi-rs` lists S3). Espressif's own solution page ranks CSI quality
  `ESP32-C5 > ESP32-C6 > ESP32-C3 ≈ ESP32-S3 > ESP32` — **S3 is mid-pack**, and
  notably no better than C3.
- **Amplitude CSI device-free localisation has been shown on ESP32** (3-link,
  0.45 m median; preprint `10.20944/preprints202601.2378.v1`). ESPectre
  (GPL-3.0) runs CSI motion sensing on S3 in the field.
- **ESP32-CSI-Tool** (Hernandez, MIT) provides active (STA/AP) and passive CSI
  capture; **Wi-ESP** (JCDE 7(5):644-656) discusses ESP32 CSI, noting **up to 16
  antennas via an external antenna switch** — relevant to building a switching
  array on a polyhedron.
- Espressif's **multi-antenna "co-crystal" board** connects two chips through a
  **clock buffer to remove relative frequency offset**, explicitly so the S3 can
  use CSI *phase*; and a **single-TX/dual-RX** mode (one chip TX, two "esp-crab"
  chips RX) extends spatial coverage. This is Espressif's own acknowledgement that
  cross-chip phase needs a shared clock.

**What is NOT demonstrated on ESP32-S3.**
- No published ESP32 study gives **continuous multi-person localisation**; the
  three-link result is single/two-occupant grid classification. Multi-person
  accuracy figures (1.5–2.7 m) come from other COTS/RTT hardware.
- No published ESP32 result demonstrates **coherent multi-node phase localisation**
  (DSF/AoA) on a distributed array.

**Other COTS vs SDR-grade.**
- **Other COTS (Intel 5300 NIC)**: E-eyes, LiFS, CSI-ratio, few-shot transfer,
  Widar all use it — **3 antennas × 30 subcarriers × 2×2 MIMO = 120-dim CSI**.
  ESP32-S3 gives **1 antenna, ~64 subcarriers, 20/40 MHz**, i.e. **no on-node
  spatial diversity**; spatial diversity must come from *many nodes*.
- **SDR-grade (USRP X310/2953, ADALM-PLUTO)**: used for Fresnel-zone/CSI-ratio
  experiments and two-way-CSI localisation (`10.1145/3686490.3686495`; SDR
  receiver in the two-way-CSI paper). These give wider bandwidth, full I/Q and
  multi-antenna phase coherence that ESP32 lacks.
- **Quantified gap**: ESP32-S3 trades ~4× fewer spatial streams and no cross-node
  phase for ~100× cost/ubiquity. Multi-node *amplitude* methods close much of the
  spatial-diversity gap by deploying more nodes; *phase* methods do not close
  without a reference.

**Experimental phase-coherent ESP32 exists but is research-grade.** ESP-PPB
(2026, GPL-3.0) phase-locks multiple **ESP32-C3** nodes over the air using
**Wi-Fi FTM + a 0.5 ppm VCTCXO**, reporting **<1° single-frame phase accuracy** at
<0.1–1 PPB clock alignment, 1 master + 2–4 slaves. Caveats it states itself:
**single antenna per node**, and the **C3 chip is not FCC/CE certified** — lab/
research only. So cross-node phase on ESP32 is *reachable* but not commodity and
not on S3 out of the box.

---

## TX needed?

**No for the receive-only core.** RTI, fingerprinting and crowd counting are all
*passive*: they consume CSI from existing traffic. Aquila's ambient/AP traffic or
the `esp-csi` router-ping mode suffices. This is the same posture as prompt 01.

**Yes, but optionally, for three things:**
1. **Controlled illumination / rate.** A designated TX (one node or a router)
   raises and stabilises the frame rate so RTI variance is well-sampled. Espressif
   documents a "packet-sending device continuously switching channels" mode as the
   highest-accuracy configuration; `esp_wifi_80211_tx` would let one node own that
   role (prompt 07 legality).
2. **The round-trip scheme** (Finding §11), which requires `esp_wifi_80211_tx` and
   a responder role — this is TX-dependent by construction, and turns a passive
   fleet into an active identity/channel/coarse-range probe.
3. **Common-transmitter reference** (prompts 10–11): if *one* node transmits and
   all others receive, that is precisely the "common-transmitter reference" escape
   that makes cross-node phase observable — the enabler for coherent AoA/imaging.
   A single pinging node is the cheapest possible movement toward phase coherence.

So: multi-node passive localisation is **fully achievable RX-only**; TX buys
rate control, ranging/identity, and the only wireless route to cross-node phase.

---

## Evidence strength

**Overall: 8 / 10.**

Breakdown by sub-claim:
- RTI formulation and the `y = Wx + n` model: **9/10** — single canonical primary
  (Wilson & Patwari 2010, >900 citations), reproduced widely, closed-form bound.
- Fingerprinting sub-metre accuracy on COTS: **8/10** — many independent
  primaries across hardware; but claimed accuracies vary wildly (0.4–1.5 m).
- Multi-person ambiguity and counting-vs-locating gap: **7/10** — well-attested,
  but the *best achievable* accuracy remains hardware- and scene-dependent.
- Node count/placement scaling law: **7/10** — the MSE bound is rigorous; the
  empirical curve beyond ~24 nodes is thin.
- **ESP32-S3-specific multi-node performance: 5/10** — only a single three-link
  preprint; no replication, no multi-person, no volumetric array.
- Cross-node phase on ESP32: **4/10** — one research platform (ESP-PPB, C3 + TCXO,
  uncertified), no S3 result.

Justification: the *field* is mature and theory-grounded, so the general question
scores high; the *ESP32-S3 answer* is thinly evidenced, so the hardware-specific
score is lower. I refuse to average those into one number without the breakdown.

---

## Primary sources

RTI and radar tomography
- Wilson & Patwari, "Radio Tomographic Imaging with Wireless Networks," *IEEE
  Trans. Mobile Computing* 9(5):621–632, 2010. DOI `10.1109/TMC.2009.174`.
  Preprint: `https://span.ece.utah.edu/uploads/RTI.pdf` / `.../RTI_version_3.pdf`.
- Wilson & Patwari, "See-Through Walls: Motion Tracking Using Variance-Based Radio
  Tomography Networks," *IEEE TMC* 10(5):612–621, 2011. DOI `10.1109/TMC.2010.175`;
  arXiv:0909.5417.
- Patwari & Wilson, "RF Sensor Networks for Device-Free Localization: Measurements,
  Models, and Algorithms," *Proc. IEEE* 98(11):1961–1973, 2010.
- Zhao, Patwari & Phillips, "Radio tomographic imaging and tracking of stationary
  and moving people via kernel distance," IPSN 2013. DOI `10.1145/2461381.2461410`.
- dRTI: directional RTI, arXiv:1402.2744.
- Robust RTI under sensor-location uncertainty, *Digital Signal Processing*, 2023.
  `https://www.sciencedirect.com/science/article/abs/pii/S1051200423001252`.
- Node density effect RTI (exponential weighting), AIP Conf. Proc. 3232:020007.
  `https://pubs.aip.org/aip/acp/article/3232/1/020007/3316588/`.

Fingerprinting and device-free CSI
- Wang et al., "E-eyes: device-free location-oriented activity identification
  using fine-grained WiFi signatures," MobiCom 2014. DOI `10.1145/2639108.2639143`.
- Zhou et al., "Device-Free Presence Detection and Localization With SVM and CSI
  Fingerprinting," *IEEE Sensors J.*, 2017. DOI `10.1109/JSEN.2017.2762428`.
- Device-Free Indoor Localization with ESP32 Wi-Fi CSI Fingerprints, preprint 2026.
  DOI `10.20944/preprints202601.2378.v1` (Sciety/researchgate mirrors).
- Few-Shot Transfer Learning for Device-Free Fingerprinting Indoor Localization,
  arXiv:2201.12656.
- Zero-Calibration Device-free Localization (RadioGrapher), arXiv:1909.03551.
- Robust Indoor Localization in Dynamic Environments (UDA), arXiv:2502.07246.
- CSI Fingerprinting Localization With Low Human Efforts, *IEEE/ACM ToN*, 2021.
  DOI `10.1109/TNET.2020.3035210`.

Model-based DFL
- Wang et al., "LiFS: Low Human-Effort, Device-Free Localization with Fine-Grained
  Subcarrier Information," *IEEE TMC*, 2018. DOI `10.1109/TMC.2017.2774296`.
  PDF: `https://oar.princeton.edu/bitstream/88435/pr17p0q/1/LiFS.pdf`.
- Fresnel-zone model localisation (MFDL demo), UbiComp 2017.
  DOI `10.1145/3123024.3123172`.
- Zeng et al., "FarSense" (CSI-ratio model), *IMWUT* 3(3), 2019.
  DOI `10.1145/3351279`.
- Wu, Zeng et al., "WiFi CSI-based device-free sensing: from Fresnel zone model to
  CSI-ratio model," *CCF Trans. PCI* 4:88–102, 2022. DOI `10.1007/s42486-021-00077-z`.
- Zheng et al., "An Exponential-Rayleigh Model for RSS-Based Device-Free
  Localization and Tracking," *IEEE TMC*, 2015.
- "Device-Free WiFi Human Sensing: From Pattern-Based to Model-Based Approaches,"
  *IEEE Commun. Mag.* 55(10):91–97, 2017.

Multi-person / counting / NLoS
- Survey: "A Survey on Device-free Indoor Localization and Tracking in the
  Multi-resident Environment," *ACM Computing Surveys*, 2020. DOI `10.1145/3396302`.
- Zhou et al., "Device-free crowd counting with WiFi CSI and deep neural networks,"
  *Wireless Networks*, 2020. DOI `10.1007/s11276-020-02274-7`.
- Device-Free Crowd Counting Using Multi-Link Wi-Fi CSI (Doppler), *Electronics*
  10(3):315, 2021.
- Choi et al., "Wi-CaL," 2022. `https://ieeexplore.ieee.org/document/9724220/`.
- EM model-based DFL of multiple bodies, *Sensors* 21(5):1728, 2021.
- UWB CIR device-free localisation (ambiguities), *Sensors* 2020, PMC9416598.
- TimeSense (multi-person RTT), arXiv:2409.00030.

ESP32 hardware / platform
- Espressif `esp-csi`: `https://github.com/espressif/esp-csi`.
- Espressif ESP-CSI solution page (chip ranking, co-crystal board, single-TX/
  dual-RX): `https://docs.espressif.com/projects/esp-techpedia/en/latest/esp-friends/solution-introduction/esp-csi/esp-csi-solution.html`.
- `esp-csi-rs` crate: `https://docs.rs/esp-csi-rs`.
- Hernandez, ESP32-CSI-Tool (MIT): `https://github.com/StevenMHernandez/ESP32-CSI-Tool`.
- Atif et al., "Wi-ESP — A tool for CSI-based Device-Free Wi-Fi Sensing (DFWS),"
  *J. Comput. Design Eng.* 7(5):644–656, 2020.
- ESPectre (GPL-3.0): `https://github.com/francescopace/espectre`.
- ESP-PPB (ESP32-C3 + VCTCXO + FTM phase lock, GPL-3.0):
  `https://github.com/jonathanmuller/esp-ppb`.
- `esp_wifi_sensing` component (presence, on-site calibration):
  `https://components.espressif.com/components/espressif/esp_wifi_sensing`.

Ranging / round trip
- FTM/RTT primaries: *Benchmarking Wi-Fi FTM for Ranging in IoT* (ACM WiSec 2023,
  DOI `10.1145/3576914.3587520`); 802.11mc vs 802.11az comparison, arXiv:2511.17935;
  passive RTT positioning, *Electronics* 9:1193, 2020.
- Device-free Wi-Fi Localization via Deep Learning on Two-Way CSI (SDR receiver),
  IEEE 2026. `https://ieeexplore.ieee.org/document/11339481`.

Datasets
- MM-Fi (multi-modal 4D human dataset), NeurIPS 2023 Datasets & Benchmarks.
  `https://proceedings.neurips.cc/paper_files/paper/2023/file/3baf7a39d07e9f4f1e258a412df94521-Paper-Datasets_and_Benchmarks.pdf`.

---

## Residual unknowns

1. **No ESP32-S3 multi-node scaling data.** The `accuracy vs node count vs
   geometry` curve is unmeasured for this hardware; published ESP32 DFL uses three
   links.
2. **No ESP32 multi-person localisation result.** Counting is shown; locating
   several people simultaneously is not.
3. **Cross-node phase on S3 is unproven.** ESP-PPB (C3 + VCTCXO + FTM) is an
   existence proof on different silicon and is uncertified; whether S3 can do it
   (or the three escapes in the BRIEF) is open.
4. **Volumetric-aperture benefit is unquantified.** No source measures a soldered
   polyhedron's RTI/DoA gain over a planar perimeter.
5. **Calibration transfer cost on ESP32.** Few-shot/UDA results are on Intel 5300;
   their label-reduction factors may not transfer to S3's 1-antenna CSI.
6. **Exact hardware used in the 2026 ESP32 preprint** (S2 vs S3, antenna config)
   is not confirmed from the abstract; flag before citing the 0.45 m number as
   S3-specific.
7. **Round-trip ranging resolution on ESP32** with the µs MAC timestamp is not
   published; the FTM path is documented but its ESP32 precision under multipath
   indoors is only coarsely characterised (1–2 m, needs wide channels).

---

## (A) Chaining

- **`01 single-node passive CSI` enables `02 multi-node passive localisation`.**
  RTI/fingerprinting is just prompt 01's per-frame amplitude/variance features
  aggregated across links. Without per-node amplitude CSI, 02 is impossible.
- **`02` is enabled by and enables the sweep, not the phase work.**
  - Amplitude multi-node DFL needs **no** cross-node phase → **unblocked** on S3.
  - Coherent AoA/coherent multi-static imaging (a natural extension of a
    polyhedron aperture, prompts 08/09) is **blocked without** a shared reference
    — one of: common-transmitter reference (11), OTA reciprocity (10), or a
    shared/conducted clock (12/17). So 02's *phase half* depends on 10/11/12/17.
- **Highest-leverage enabling capability: a common-transmitter reference.** It is
  the cheapest escape (one node TX, all RX), it simultaneously (i) supplies stable
  illumination for RTI, (ii) makes cross-node phase observable → coherent imaging/
  DoA, and (iii) is the same primitive the round-trip scheme uses. Aquila should
  treat "one node pings, many receive" as the keystone experiment.
- **A capability that unlocks a further one:** the **shared-clock polyhedron**
  (prompt 09) — if a co-located volumetric array shares a clock, it removes the
  TDM requirement *and* gives cross-node phase for 3D DoA, unlocking coherent
  volumetric imaging that plain 02 cannot reach.
- **`esp_wifi_80211_tx` (07) enables** the round-trip probe and the common-TX
  reference; it is a gating primitive for 02's active forms.
- **`13 static-multipath self-referencing` enables** longer-interval RTI
  baselines; **`14 map/location-aided calibration`** reduces 02's survey burden.

## (B) Existing models and data

| Method | Source | Input | Hardware shown on | Aquila usable? |
|---|---|---|---|---|
| RTI (mean + variance) | Wilson & Patwari 2010/2011 | RSS (dB), differenced | 2.4 GHz sensor net (COTS) | **Yes, now** — amplitude/RSS, LO-independent |
| E-eyes | Wang 2014 | CSI amplitude | 1 commodity AP | **Yes** — amplitude only |
| LiFS / Fresnel-zone | Wang 2018; MFDL 2017 | CSI phase/amplitude | Intel 5300 | Amplitude part yes; phase part needs per-node reference |
| CSI-ratio / FarSense | Zeng 2019 | CSI **ratio of 2 antennas** | Intel 5300 (co-located) | **No on single-antenna S3 node** — needs 2 antennas co-located; possible *within* a shared-clock cluster |
| CARM (not "CARML") | Wang 2015 MobiCom | CSI dynamics | COTS | Yes for activity, not localisation |
| Crowd counting DNN | Zhou 2020 | CSI variance | COTS | Yes — amplitude/variance |
| Few-shot/UDA transfer | arXiv:2201.12656 / 2502.07246 | CSI fingerprint | Intel 5300 | Method yes; calibrate for S3 CSI |
| esp-csi | Espressif (repo licence: check before bundling) | CSI | All ESP32 incl. S3 | **Yes** — the collection layer |
| ESP32-CSI-Tool | Hernandez, MIT | CSI, active/passive | ESP32 | **Yes** — MIT-licensed |
| ESPectre | GPL-3.0 | CSI variance | S3/C3/C6 | Yes but **GPL-3.0** (copyleft) |
| ESP-PPB | GPL-3.0 | phase-coherent CSI | ESP32-C3 + VCTCXO | Reference only; uncertified, C3 not S3 |
| MM-Fi | NeurIPS 2023 | mmWave/WiFi/LiDAR | various | Use for pretraining/architecture, not ESP32 |

Licence flags: prefer **MIT** (ESP32-CSI-Tool) and Espressif's own repo for
bundling; **GPL-3.0** (ESPectre, ESP-PPB) is copyleft and constrains how Aquila
can incorporate changes into a permissively-licensed product.

## (C) Own-data / new technology

**Claim to test:** *a compact multi-node ESP32-S3 fleet with controlled geometry
reaches sub-metre single-person device-free localisation and correct crowd counts,
and its node-count/placement scaling matches the RTI MSE bound — while remaining
fully receive-only.*

**Minimal experiment.** 8–16 ESP32-S3 nodes on a known layout (at least one
volumetric/polyhedron sub-array), one designated TX (router or one S3), CSI
streamed to a shared host with a common ingest timestamp. Ground-truth grid
(≈25 points) + occupancy ∈ {0,1,2,3}. Repeat in two rooms and both LoS and one
NLoS (through-wall) configuration. Record amplitude CSI, RSSI, and — where a TX
node exists — round-trip timing.

**What it proves that is not currently demonstrated:**
1. The ESP32-specific `accuracy vs node count` curve (3 → 8 → 16 nodes).
2. Whether a volumetric aperture beats a planar perimeter on S3 (Finding §9).
3. Multi-person localisation/counting on ESP32 (currently absent).
4. Calibration-transfer cost (few-shot on S3 CSI) — Finding §10.

**Falsifiers (any one kills the claim):**
- Amplitude CSI carries no more localisation information than RSSI at S3's
  1-antenna, ≤40 MHz resolution (prompt 01's null-sensitivity caveat).
- Accuracy saturates **above** ~1.5 m for single-person even at 16 nodes and
  dense grid, i.e. the RTI bound is not approached on this hardware.
- The volumetric array shows no accuracy gain over a planar perimeter with equal
  node count.
- Multi-person error grows faster than linearly in occupancy (ambiguity dominates).

**Why it is worth building now:** the dataset is cheap (S3 nodes, no licence),
purely receive-only (no 07 legal question), and directly fills the ESP32-specific
gap that every published primary leaves open — three links, one room, single
occupant.

---

*Trailing uncertainty: the "CARML" lead is unresolved (treated as CARM); the 2026
ESP32 preprint's exact silicon is unconfirmed; all ESP32 cross-node phase claims
rest on a single uncertified C3 research platform.*
