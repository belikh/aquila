# 19 — Model-based vs learning-based routes and supervision

**Prompt:** For each capability, which of model-based inference or learned models is
appropriate, and what supervision does each need? Must contain (a) capability-by-capability
route; (b) label requirements (none/synthetic/weak/full); (c) generalisation across
rooms/devices; (d) ESP32 edge vs host compute.

**Scope cross-cut:** all three radio capabilities — `esp-csi` CSI mode; experimental raw-I/Q
(ESPARGOS/esp-sdr); compliant 802.11 TX (`esp_wifi_80211_tx`), including sender/receiver
clusters and volumetric (polyhedron) apertures.

Australian English. Reference hardware: ESP32-S3. Primary sources only.

---

## Question

Wi-Fi sensing splits into two paradigms that are frequently conflated:

1. **Model-based** (a.k.a. *modeling-based*): exploit a physical/mathematical model
   (Fresnel-zone diffraction, CSI-ratio, time-of-flight, angle-of-arrival, Rician fading)
   to map CSI perturbations to a physical quantity. Inference is largely analytic; learning,
   if any, is a thin classifier or estimator on top.
2. **Learning-based** (a.k.a. *pattern-based* / data-driven): learn the CSI→label mapping
   directly from data with shallow ML (SVM, random forest) or deep nets (CNN, RNN/GRU/LSTM,
   Transformer/ViT).

The question for Aquila: **for each intended capability, which route is appropriate, and what
supervision does each demand** — measured in labels (none / synthetic / weak / full), compute
(ESP32-S3 edge vs host), and robustness across new rooms and devices?

---

## Findings

### ESTABLISHED

**E1. The taxonomy is standard and widely used.** Recent surveys split CSI sensing into
model-based and learning-based (pattern-based) methods, and add a third *hybrid* class
[[1]](#s1)[[2]](#s2)[[3]](#s3). The canonical framing paper is Wu, Yang, Zhou & Wang,
*Device-Free WiFi Human Sensing: From Pattern-Based to Model-Based Approaches* (IEEE Comm.
Mag., 2017) [[4]](#s4); the model side is consolidated in Wu & Zeng's *from Fresnel zone
model to CSI-ratio model* (2021) [[5]](#s5).

**E2. Model-based approaches trade ceiling for robustness and interpretability.**
Model-based methods "analyze and recognise human behaviours by constructing mathematical
relationships among WiFi devices, environment, human position/posture, and received CSI";
they are valued for *interpretable physical meaning* and *the ability to guide system design*
[[5]](#s5). The literature states the Fresnel-zone model "performs better than traditional
deep learning in generalization ability, computational efficiency, robustness" [[6]](#s6) —
a strong, repeated claim, though usually demonstrated on one capability (respiration /
displacement) rather than as a head-to-head benchmark.

**E3. Learning-based approaches dominate accuracy on fixed benchmarks but generalise poorly.**
Deep nets (CNN/LSTM/GRU/Transformer) "commonly achieve accuracies exceeding 95%" [[7]](#s7),
and SenseFi reports state-of-the-art models at high in-domain accuracy across four public
datasets [[8]](#s8). But "even minor environmental changes — such as alterations in room
layout or the presence of new objects — [lead] to substantial performance degradation or
complete failure … especially serious for learning-based methods" [[9]](#s9)[[10]](#s10).
This is not a marginal effect; it is the field's central open problem.

**E4. Supervision comes in a well-defined ladder.**
- **None (self-supervised):** AutoFi learns CSI representations with no labels via geometric
  self-supervision [[11]](#s11); the SenseFi library ships an unsupervised track (train on
  NTU-Fi HAR, test on NTU-Fi HumanID) [[8]](#s8).
- **Weak:** channel charting is learned from partial geometry — self-supervised triplet loss
  (no labels at all), or *weakly-supervised* bilateration/LoS-bounding-box losses that need
  only AP positions and approximate LoS areas [[12]](#s12)[[13]](#s13).
- **Synthetic pre-training:** parameterised ray-tracing/geometric-primitive CSI generation
  is used to pre-train models, then fine-tune on a little real data [[14]](#s14)[[15]](#s15).
- **Full:** the classical supervised HAR/gesture/identity benchmarks (UT-HAR, NTU-Fi,
  Widar3.0, SignFi) [[8]](#s8).

**E5. Physics-based models need *geometry*, not labels.** Fresnel/CSI-ratio models require the
Tx–Rx positions and the assumption of a dominant reflection/diffraction term; they then predict
signals analytically (e.g. dynamic Fresnel-zone model predicts RF variation vs human position
and orientation) [[16]](#s16). RTI (radio tomographic imaging) is the analogous model-based
device-free-localisation route: a linear shadowing model over many links, no per-position
training in the ideal case [[17]](#s17)[[18]](#s18).

**E6. Edge inference is feasible but demonstrated mainly off-CSI.** ESP-DL/ESP-NN provide
INT8 quantised NN inference for ESP32-S3, exploiting the ~128-bit SIMD "PIE" vector
instructions (~4.5×–6.25× acceleration over ESP32; ~18× over unoptimised C) [[19]](#s19)[[20]](#s20)[[21]](#s21)[[22]](#s22).
TFLite-Micro is also supported via esp-nn [[23]](#s23). **However**, Espressif's own
demos are vision (face detection, ESP-WHO), not CSI. Wi-Fi-CSI networks are generally trained
and run on host CPUs/GPUs; "DL approaches typically require GPU acceleration … which may not
always be available, especially on resource-constrained edge devices" [[1]](#s1). Model
compression (0.0578 M vs 252.10 M parameters for a lightweight GRU HAR model [[24]](#s24))
shows the edge path exists, and a full ESP32-collected CSI + LSTM-Attention pipeline has been
run end-to-end on low-cost S3-class MCUs (0.95 macro-F1, own dataset) [[25]](#s25), plus a
CNN HAR on ESP32 modules (94.7%) [[26]](#s26) — but inference is offloaded or server-side in
most such systems.

**E7. Datasets exist, with heterogeneous licences and hardware.** MM-Fi: 40 subjects, 4
environments, 27 actions, 320k+ synchronised frames across RGB/depth/LiDAR/mmWave/WiFi-CSI
(Atheros CSI Tool, TP-Link N750, 114 subcarriers, 5 GHz) [[27]](#s27)[[28]](#s28). ESPARGOS:
phase-coherent many-antenna CSI datasets on ESP32-S2, openly published via DaRUS
[[29]](#s29)[[30]](#s30). CSI-Bench: in-the-wild, 16 devices, 26 environments, 35 users,
~461 h, CC BY-NC-ND 4.0 [[31]](#s31)[[32]](#s32). SenseFi benchmark code is MIT-licensed
[[8]](#s8).

### HYPOTHESIS

**H1. A capability-by-capability route split is the right framing** (see table below). The
evidence supports a division: capabilities whose physical quantity is well-modelled (range,
respiration rate, gross displacement, Doppler, count) are best served by model-based/analytic
front-ends; capabilities with a high-dimensional, poorly-parameterised label space (identity,
fine gesture, pose, activity class) currently require learning, and their generalisation problem
is unsolved.

**H2. Synthetic pre-training closes most of the data gap on static set-ups.** A
parameterised synthetic-CSI Sim2Real study reports 20 % of real labels reaching 82.1 %
macro-F1 = 98.6 % of full-dataset performance, cutting collection cost ~80 % [[14]](#s14);
WiFi-JEPA pre-trains purely on ray-traced geometry and improves cross-environment pose
[[15]](#s15). This suggests synthetic data is a viable substitute for *bulk* labels, but
fine-tuning still needs a little real, task-specific supervision.

**H3. Weak supervision beats full labels on the geometry task.** Weakly/hybridly-supervised
channel charting matches a semi-supervised baseline that needed ~400 ground-truth-labelled
CSI samples, *without* any UE ground-truth positions [[12]](#s12). If this transfers to an
ESP32-scale array, absolute coordinates may be recoverable from AP positions alone.

**H4. Cross-modal supervision is the strongest lever for learning-based generalisation.**
Vision/radar teachers (knowledge distillation, semantic anchors) transfer robustness into the
weaker Wi-Fi modality [[9]](#s9)[[33]](#s33)[[34]](#s34); Wi-Fi↔radar translation is
demonstrated [[9]](#s9). This is a *label-manufacturing* trick: synthetic/weak labels are
mined from co-located synchronised sensors.

### SPECULATION

**S1. Model-based front-ends will dominate on ESP32-S3**, because they (i) run without a GPU,
(ii) need no labels, and (iii) survive the device/room change that defeats learned models —
matching E2/E6. Learning would be reserved for host-side or lightly-quantised classification
of a *model-derived* feature (e.g. classify activity from a CSI-ratio phase history).

**S2. An Aquila polyhedron aperture + model-based AoA could produce labels cheaply for a
learned downstream task** — the array gives multi-link geometry (model-based), and the
geometry becomes weak supervision for a learner (H3). This is speculative: no published
ESP32-S3 volumetric-array result exists.

**S3. The round-trip scheme is intrinsically model-based for its physical quantities**
(the reply timing yields range/offset by geometry, Doppler by phase-rate), and intrinsically
learning-based only for identity (a classification over reply-fingerprints). No primary source
demonstrates the *unique-reply round-trip* identity channel; the claim is ours.

---

## Capability-by-capability route (prompt item a & d)

| Capability | Recommended route | Supervision | Where it runs |
|---|---|---|---|
| Presence / motion detection (01) | **Model-based** — amplitude/Doppler variance, CSI-ratio; threshold or tiny classifier | **None/near-none** | ESP32-S3 edge feasible |
| Respiration / vital signs | **Model-based** — Fresnel-zone / CSI-ratio phase model [[5]](#s5)[[16]](#s16) | **None** | Edge or host |
| Gross displacement / velocity / direction | **Model-based** — dynamic Fresnel / WiDir-phase [[16]](#s16) | **None** | Edge |
| Doppler (moving only) | **Model-based** — temporal CSI-ratio phase rate [[5]](#s5) | **None** | Edge |
| Activity recognition (HAR) | **Hybrid** — model features + learned classifier; or synthetic-pretrained DL | **Synthetic + few real**, or **full** | Host training; edge inference via ESP-DL |
| Gesture / fine-grained | **Hybrid/learning** — domain-adaptation DL (Widar3.0, AutoFi) [[8]](#s8)[[35]](#s35) | **Full or weak+DA** | Host (GPU) |
| Identity / gait | **Learning** — few-shot/self-supervised (CAUTION, IdentiFi) [[36]](#s36)[[37]](#s37) | **Full / few-shot / self-sup** | Host |
| Device-free localisation | **Model-based** (RTI, Fresnel-localisation) *or* **weakly-supervised** charting | **None (RTI)** / **weak (charting)** | Host; edge for RTI |
| Relative geometry / channel charting | **Self/weakly-supervised** [[12]](#s12) | **None/weak** (AP positions + LoS boxes) | Host |
| Material / through-wall inference | **Model-based** (attenuation vs frequency) + light regression | **None/few** | Host |
| Human vs object vs animal (18) | **Learning** (classification) once models exclude known physics | **Full** | Host |
| Range / clock offset (round-trip, 11) | **Model-based** — geometric timing | **None** | Edge |
| Node self-localisation / FTM (15) | **Model-based** — FTM/RTT geometry, multipath-corrected | **None/self-cal** | Edge |
| RF-SLAM / occupancy (16) | **Hybrid** — physics motion model + learned observation model | **Weak/self** | Host |

**Generalisation (item c).** Model-based routes are *room-parametrised* (they adapt by
re-measuring geometry), so they degrade gracefully and transparently; learning-based routes are
*room-fitted* and fail under layout change [[9]](#s9). Device heterogeneity is a distinct axis:
CSI from Intel 5300 vs Atheros vs ESP32 differs in subcarrier count (30 vs 114 vs ~64), AGC and
phase calibration; domain adaptation/alignment (MMD, adversarial, AirFi) is the standard remedy
[[9]](#s9)[[38]](#s38). No source shows a *single* learned model transferring cleanly across
these platforms without adaptation.

---

## ESP32-S3 feasibility

- **CSI-mode, model-based inference: HIGH.** Amplitude/CSI-ratio features are cheap; runs on
  S3 today. esp-csi supports the whole ESP32 family [[39]](#s39).
- **CSI-mode, learned inference on-device: MEDIUM.** ESP-DL/ESP-NN/INT8 SIMD exist and a
  lightweight GRU (~0.06 M params) is plausible [[22]](#s22)[[24]](#s24), but no published
  CSI-model-on-S3-with-ESP-DL result was found. Latency/memory for CSI tensors (3×114×500 in
  benchmarks [[8]](#s8)) are large for MCU SRAM; windowing/compression (EfficientFi) needed
  [[40]](#s40).
- **Raw-I/Q (isolation #2), learned: LOW/unproven.** Gapped, uncalibrated receive-only I/Q
  has no published learned-sensing pipeline; the ESPARGOS work is *CSI* on phase-locked S2
  arrays, not raw I/Q [[29]](#s29).
- **TX clusters / polyhedron (capability #3): no ML precedent.** Volumetric aperture ML
  (DoA/pose) is SDR- or ray-tracing-grade only; the ESP32-S3 20-node icosahedron is untested.

**Quantified hardware gap.** Benchmarks and datasets are overwhelmingly **COTS Wi-Fi NIC**
(Intel 5300 / Atheros, `L-CSI`, 20–40 MHz, 30–114 subcarriers, often immutable firmware)
[[1]](#s1)[[27]](#s27). ESP32 CSI gives cheaper, more flexible access (all-access MAC, TX
control) but coarser/uncertain phase and fewer antennas than the 8×4 ESPARGOS S2 array
[[29]](#s29) — itself still below the *phase-coherent* arrays assumed by channel charting and
distributed-MIMO literature [[12]](#s12). Aquila sits between commodity NICs and SDR-grade
arrays: better than a single NIC, short of a synchronised sounder.

**TX needed?** Model-based localisation-via-round-trip, RTI and channel charting all need
*illumination* — i.e. capability #3 (compliant TX) is the enabler. Passive-only (receive-only
from ambient APs, capability #1) suffices for presence/respiration/Doppler but cannot create
the controlled multi-link geometry that model-based localisation and charting need. See Chaining.

---

## Evidence strength

**7 / 10.**

Justification: the *paradigm taxonomy*, the *supervision ladder*, the *generalisation failure of
learning*, and the *existence of models/datasets* are all firmly established across multiple
independent primary surveys and benchmarks (E1–E7). The strength is capped by: (i) most
"model-based generalises better" claims are single-capability demonstrations, not head-to-head
benchmarks; (ii) synthetic-pretraining and weak-supervision results are recent and few
(H2/H3, [[14]](#s14)[[12]](#s12)); (iii) **zero** results map these routes onto ESP32-S3 CSI
specifically or onto volumetric TX apertures; (iv) reproducibility in the field is itself poor
[[1]](#s1). Strong on the general principle, weak on Aquila-specific transfer.

---

## Primary sources

<a id="s1"></a>[1] Guarino, Carra, Cominelli, Gringoli, Lo Cigno. *A survey on CSI-based Wi-Fi sensing datasets and models with a focus on reproducibility.* Computer Communications, 2026. https://www.sciencedirect.com/science/article/pii/S0140366426000216

<a id="s2"></a>[2] Ma, Xi, et al. *Machine Learning Techniques for Wi-Fi CSI-based Recognition and Sensing: A Comprehensive Review.* 2025. https://www.researchgate.net/publication/399940461 (Semantic Scholar: https://www.semanticscholar.org/paper/bc345e6653bec267769d20c06ed019ccfd49a21b)

<a id="s3"></a>[3] *A Comprehensive Survey on Wi-Fi Sensing for Human Identity Recognition.* Electronics (MDPI) 12(23):4858, 2023. https://www.mdpi.com/2079-9292/12/23/4858

<a id="s4"></a>[4] Wang, Zhou, et al. *Device-Free WiFi Human Sensing: From Pattern-Based to Model-Based Approaches.* IEEE Communications Magazine 55(10):91–97, 2017. https://ieeexplore.ieee.org/abstract/document/8067692

<a id="s5"></a>[5] Wu, Zeng. *WiFi CSI-based device-free sensing: from Fresnel zone model to CSI-ratio model.* CCF Transactions on Pervasive Computing and Interaction, 2021. DOI 10.1007/s42486-021-00077-z

<a id="s6"></a>[6] *Application of Fresnel Zone Model in Wireless Sensing Field.* 2017/2023. https://pdfs.semanticscholar.org/05b4/584cacd158351c7c673cf4421a451b9b4ac1.pdf

<a id="s7"></a>[7] *Machine Learning Techniques for Wi-Fi CSI-based Recognition and Sensing* (as [2]), §classical vs deep models.

<a id="s8"></a>[8] Yang, Chen, Zou, et al. *SenseFi: A library and benchmark on deep-learning-empowered WiFi human sensing.* Patterns 4(3):100703, 2023. arXiv:2207.07859. Code (MIT): https://github.com/xyanchen/wifi-csi-sensing-benchmark

<a id="s9"></a>[9] *A Survey on Wi-Fi Sensing Generalizability: Taxonomy, Techniques, Datasets, and Future Research Prospects.* arXiv:2503.08008 (v2/v3). https://arxiv.org/html/2503.08008v3

<a id="s10"></a>[10] *A Short Overview of Multi-Modal Wi-Fi Sensing.* arXiv:2505.06682, 2025. https://arxiv.org/html/2505.06682v1

<a id="s11"></a>[11] Yang, Chen, Zou, Wang, Xie. *AutoFi: Towards Automatic WiFi Human Sensing via Geometric Self-Supervised Learning.* arXiv:2205.01629, 2022.

<a id="s12"></a>[12] *Channel Charting in Real-World Coordinates with Distributed MIMO.* arXiv:2406.13722 (v2), 2025. https://arxiv.org/html/2406.13722v2

<a id="s13"></a>[13] Euchner, Kellner, Stephan, ten Brink. *ESPARGOS: Phase-Coherent WiFi CSI Datasets for Wireless Sensing Research.* arXiv:2408.16377, 2024. Channel-charting demo: https://github.com/Jeija/ESPARGOS-WiFi-ChannelCharting

<a id="s14"></a>[14] *Parameterized synthetic WiFi CSI data generation for trustworthy human activity recognition: A Sim2Real approach with edge deployment analysis.* Internet of Things (Elsevier), 2026. https://www.sciencedirect.com/science/article/abs/pii/S2542660526000405

<a id="s15"></a>[15] *WiFi-JEPA: Self-supervised Learning for WiFi-CSI 3D Human Pose Estimation.* arXiv:2607.11064. https://arxiv.org/html/2607.11064v1

<a id="s16"></a>[16] *Towards a Dynamic Fresnel Zone Model to WiFi-based Human Activity Recognition.* Proc. ACM IMWUT 7(2), 2023. DOI 10.1145/3596270

<a id="s17"></a>[17] Wilson, Patwari. *Radio Tomographic Imaging with Wireless Networks.* IEEE Trans. Mobile Computing, 2010. (892 citations.)

<a id="s18"></a>[18] *Device-Free Localization Using Multi-Link MIMO Channels in Distributed Antenna Networks.* arXiv:2505.04085, 2025. https://arxiv.org/html/2505.04085v1

<a id="s19"></a>[19] Espressif. *ESP-DL: deep-learning library.* https://github.com/espressif/esp-dl

<a id="s20"></a>[20] Espressif. *ESP-DL User Guide — ESP32-S3.* https://docs.espressif.com/projects/esp-dl/en/release-v1.1/esp32s3/introduction.html

<a id="s21"></a>[21] Espressif. *ESP-DL benchmarks (ESP32-S3, ESP32-P4).* https://github.com/espressif/esp-dl/blob/master/benchmark_report.md

<a id="s22"></a>[22] Espressif. *Edge-AI with ESP32-S3 Workshop: Introduction.* https://developer.espressif.com/workshops/edge-ai-with-esp32-s3/introduction (128-bit SIMD, ~18× INT8 speed-up)

<a id="s23"></a>[23] Espressif. *esp-nn: optimised NN functions for TFLite Micro.* https://github.com/espressif/esp-nn

<a id="s24"></a>[24] *Human Activity Recognition Through Augmented WiFi CSI Signals by Lightweight Attention-GRU.* Sensors 25(5):1547, 2025. https://www.mdpi.com/1424-8220/25/5/1547

<a id="s25"></a>[25] Abdurrahman, Irawan. *Stable Human Activity Recognition Using ESP32-Collected CSI Data with LSTM-Attention.* IEEE ICIC, 2025. https://ieeexplore.ieee.org/document/11309497

<a id="s26"></a>[26] *CSI-Based Human Activity Recognition Using Low-Cost Wi-Fi Modules and Deep Neural Networks.* IEEE, 2025. https://ieeexplore.ieee.org/document/11231490

<a id="s27"></a>[27] Yang, Huang, Zhou, et al. *MM-Fi: Multi-Modal Non-Intrusive 4D Human Dataset for Versatile Wireless Sensing.* NeurIPS 2023 Datasets & Benchmarks. arXiv:2305.10345. DOI 10.52202/075280-0822

<a id="s28"></a>[28] MM-Fi toolbox. https://github.com/ybhbingo/MMFi_dataset (raw images withheld for privacy; 17 body keypoints supplied instead)

<a id="s29"></a>[29] ESPARGOS dataset espargos-0002 (DaRUS). https://espargos.net/datasets/data/espargos-0002

<a id="s30"></a>[30] ESPARGOS dataset espargos-0005 (DaRUS), DOI 10.18419/DARUS-4754; espargos-0007, DOI 10.18419/DARUS-4973.

<a id="s31"></a>[31] *CSI-Bench: A Large-Scale In-the-Wild Dataset for Multi-task WiFi Sensing.* arXiv:2505.21866, 2025. https://arxiv.org/html/2505.21866v1

<a id="s32"></a>[32] CSI-Bench repository (CC BY-NC-ND 4.0). https://github.com/Jenny-Zhu/CSI-Bench-Real-WiFi-Sensing-Benchmark

<a id="s33"></a>[33] *CM-NET: Cross-Modal Learning Network for CSI-Based Indoor People Counting.* Electronics 11(24):4113, 2022. https://www.mdpi.com/2079-9292/11/24/4113

<a id="s34"></a>[34] *Research on Cross-Scene Human Activity Recognition Based on Radar and Wi-Fi Multimodal Fusion.* 2025. https://www.researchgate.net/publication/390651951

<a id="s35"></a>[35] Zheng, et al. *Widar3.0: Zero-effort cross-domain gesture recognition with Wi-Fi.* IEEE TPAMI 44(11):8671–8688, 2022.

<a id="s36"></a>[36] *CAUTION: A Robust WiFi-based Human Authentication System via Few-shot Open-set Gait Recognition.* IEEE IoT-J, 2022. https://ieeexplore.ieee.org/abstract/document/9726794

<a id="s37"></a>[37] *Self-Supervised WiFi-Based Identity Recognition in Multi-User Smart Environments (IdentiFi).* Sensors/PMC, 2025. https://pmc.ncbi.nlm.nih.gov/articles/PMC12115556/

<a id="s38"></a>[38] Wang, Yang, Cui, Xie, Sun. *AirFi: Empowering WiFi-based passive human gesture recognition to unseen environment via domain generalization.* IEEE TMC, 2024.

<a id="s39"></a>[39] Espressif. *esp-csi.* https://github.com/espressif/esp-csi (all ESP32 series support CSI; dual-core 240 MHz with AI instruction set)

<a id="s40"></a>[40] Yang, et al. *EfficientFi: Towards large-scale lightweight WiFi sensing via CSI compression.* IEEE IoT-J, 2022.

---

## Residual unknowns

1. **ESP32-S3 CSI tensors + on-device inference**: no published latency/energy numbers for a
   *CSI* model on ESP-DL/ESP-NN (vision only). Unknown whether a HAR/gesture net fits SRAM
   after windowing.
2. **Head-to-head model-based vs learned benchmarks**: the "Fresnel generalises better" claim
   is asserted, not shown as a controlled comparison across identical rooms/devices.
3. **Dataset licences**: MM-Fi's exact redistribution terms (arXiv non-exclusive; images
   withheld) and the ESPARGOS DaRUS licence (likely CC BY, **unverified**) need confirmation
   before Aquila reuses them commercially.
4. **Cross-platform transfer**: no result transfers one model across Intel 5300 / Atheros /
   ESP32 CSI without adaptation; the size of the residual gap is unquantified.
5. **Volumetric (polyhedron) aperture ML**: no published learned or model-based result on a
   soldered-PCB 3D array; completely open.
6. **Raw-I/Q learned sensing**: ESPARGOS is phase-coherent *CSI*, not raw I/Q; whether gapped
   receive-only I/Q supports any learning target is undemonstrated.
7. **Weak-supervision transfer**: channel-charting weak-supervision gains were shown on
   synchronised arrays; behaviour under ESP32 drift/AGC is unknown.
8. **Round-trip unique-reply identity**: the mini-scheme in the brief has no primary source.

---

## Chaining (A)

- **Capability #3 (compliant TX / clusters) is the highest-leverage enabler.** It creates the
  *controlled, known-geometry illumination* that model-based localisation, RTI and channel
  charting all require; without it, sensing is limited to passive ambient reflection.
- **#1 (CSI) → model-based presence/respiration/Doppler** directly, with **no labels**. This is
  the lowest-cost, most robust Aquila product surface and the right first target.
- **#3 (TX) + #1 (CSI) → multi-link geometry → weak supervision** for a learned
  downstream task (localisation/identity). "TX enables weak labels for learning."
- **Volumetric polyhedron aperture** could supply multi-baseline geometry that becomes a
  *model-based* absolute-position estimator (AoA triangulation), which in turn labels a learner.
  Node count/geometry replaces labels. Untested.
- **Raw-I/Q (#2) is currently a dead end for learning**; it does not enable a further capability
  without a calibration/reference escape (blocked without the common-reference work in prompts
  10–12).
- **Dependency chain:** calibrated shared reference (10/11/12) → coherent multi-node phase →
  model-based distributed localisation; **blocked without** the reference. For *amplitude-only*
  model-based sensing (presence/respiration), no such dependency.

## Existing models and data (B)

- **Models / code Aquila can use:** SenseFi library + model zoo (MIT, PyTorch) [[8]](#s8);
  AutoFi self-supervised pretraining [[11]](#s11); Widar3.0 cross-domain gesture [[35]](#s35);
  AirFi domain generalisation [[38]](#s38); Fresnel/CSI-ratio models (analytic, no licence
  encumbrance) [[5]](#s5)[[16]](#s16); RTI linear model [[17]](#s17); ESP-DL/ESP-NN runtime
  (Espressif licence) [[19]](#s19)[[23]](#s23).
- **Datasets:** MM-Fi (multimodal, 40 subjects, 4 rooms) [[27]](#s27); ESPARGOS (phase-coherent
  ESP32-S2 arrays, matches Aquila's radio chip family closely) [[29]](#s29)[[13]](#s13);
  CSI-Bench (in-the-wild, non-commercial licence) [[31]](#s31); SenseFi's UT-HAR/NTU-Fi/Widar
  [[8]](#s8). **Input format caveat:** nearly all are `L-CSI` from Intel 5300/Atheros
  (30–114 subcarriers), so an ESP32 front-end must resample/align — a domain-adaptation problem,
  not a plug-in.
- **Hardware gap:** ESPARGOS is the closest match (ESP32-S2 CSI, many antennas) but requires a
  *wired common clock/phase reference* [[29]](#s29); Aquila's wireless-only goal is not met by
  any public dataset.

## Own-data / new technology (C)

**Claim a purpose-built Aquila dataset/experiment could prove:** that a **model-based front-end
trained on synthetic physics** and fine-tuned on a *tiny* real Aquila set matches
learning-from-scratch accuracy **while transferring to an unseen room without re-labelling** —
the central open problem (E3).

**Minimal experiment.** Two-room, one-node-ping/others-reply capture; collect CSI plus a
co-located camera/mmWave label source for a *small* anchor set (~20 % of sessions) and
**withhold all labels** from the second, unseen room. Compare (i) synthetic-physics-pretrained
+ few-shot model, (ii) pure supervised baseline, (iii) analytic Fresnel/CSI-ratio model. Metric:
cross-room macro-F1 without target labels [[14]](#s14)[[15]](#s15).

**Falsified if:** the no-label cross-room model performs no better than chance (or no better
than the pure-supervised baseline), *or* synthetic pretraining gives no data-efficiency gain —
i.e. the Sim2Real advantage (H2) does not survive the ESP32 device/room shift.

**Second, orthogonal experiment (Aquila-unique):** the round-trip unique-reply scheme, testing
whether reply-timing + reply-CSI fingerprint gives (a) model-based range/offset *and* (b) a
learned identity signature in *one* protocol. Falsified if identity signatures fail to persist
across sessions or range estimates are swamped by multipath bias.

**What would be genuinely new:** a *labelled* dataset from an ESP32-S3 **volumetric polyhedron**
with synchronised ground truth, paired with a **weakly-supervised** charting recipe — nothing
in the literature covers a 3D soldered-PCB aperture, and the strongest generalisation results
(E2/H3) come from exactly this model-based-plus-weak-label combination.
