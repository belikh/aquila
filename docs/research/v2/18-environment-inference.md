# 18 — Environmental inference beyond humans

**Prompt:** What non-human states can CSI/I/Q infer — objects/furniture, doors/windows,
materials, water/plants, machinery — and with what confidence? Must contain (a)
object/furniture change detection; (b) door/window open-closed inference; (c) material and
moisture/liquid sensing; (d) human vs animal vs object classification; (e) confounders.

**Scope cross-cut:** all three radio capabilities — `esp-csi` CSI mode; experimental raw-I/Q
(ESPARGOS/esp-sdr); compliant 802.11 TX (`esp_wifi_80211_tx`), including sender/receiver
clusters and volumetric (polyhedron) apertures.

Australian English. Reference hardware: ESP32-S3. Primary sources only.
Status tags: **ESTABLISHED / HYPOTHESIS / SPECULATION**. Hardware tag: **[S3]** ESP32 family,
**[COTS]** other commodity Wi-Fi (Intel 5300/Atheros/Broadcom/Nexmon), **[SDR]** SDR- or
radar-grade.

---

## Question

Wi-Fi sensing is overwhelmingly built for *people*. This note asks the orthogonal question:
what can the same physical channel observations infer about the **non-human** environment —
the layout of furniture, the state of a door or window, the material or moisture content of a
target, the presence/identity of a machine — and how much of that survives on an ESP32-S3?

The physics is favourable in one sense: CSI is "very sensitive to environmental changes …
large movements such as people or animals … also subtle actions in a static environment"
[[26]](#s26). It is unfavourable in another: because every source of change (a person, a
chair, a door, temperature, a machine) is superposed on the *same* few tens of subcarriers,
the non-human signal is the *confounder* in most of the literature, not the target. The
question is therefore as much about **separating** an environmental change from the others as
about detecting it at all.

---

## Findings

### ESTABLISHED

**E1. Doors and windows can be inferred without attached sensors — on commodity Wi-Fi.**
The canonical work detects both the *events* and the *states* of everyday objects (doors,
windows) from CSI using a commodity AP and a commodity Wi-Fi receiver. It separates the mixed
contributions of multiple objects with **independent component analysis**, classifies with a
CNN+RNN, and then corrects the estimates with a **hidden Markov model** over legal state
transitions ("the *open* event of a door occurs only when the door is in a *closed* state")
[[1]](#s1). Hardware: **[COTS]**. This is the strongest single result for a non-human target,
and its HMM trick is a general recipe: environmental states are *structured sequences*, not
independent frames. Corroboration on COTS at the activity level: a device-free CSI system
recognised "opening and closing the refrigerator door" at 93.5 % and "opening and closing
closet door" at 100 % in a home feasibility study [[38]](#s38); door-opening/closing *motion*
is also rich enough that it carries a person-identity signature [[37]](#s37).

**E2. Static objects *can* be classified, but the signal is small and setup-bound.**
A 2025 study classifies static objects from AP CSI and finds that "traditional deep CNN
approaches prove futile … due to the relatively smaller amplitude in the CSI stream", while a
**k-NN instance-based** learner reaches up to **100 % under line-of-sight** [[3]](#s3). The
negative result is the important one: the static return is near the noise floor, so the answer
is dominated by the *geometry*, not by the object. A companion bistatic mobile result shows a
robot-mounted CSI sniffer deciding whether a **closed container contains an object**, with
accuracy dependent on robot movement and velocity [[5]](#s5) — motion of the *observer* buys
back the aperture that a static link lacks.

**E3. Materials and liquids are identifiable, but mostly on research NICs or with contact.**
Wi-Fi CSI amplitude-ratio and phase-difference features identified **10 common liquids at
98.8 %** and similar liquids (salt concentrations) at >95 % [[6]](#s6). A COTS liquid-level
system reached **97 % continuous / F-score 0.968 discrete**, robust across container materials
(metal/glass/ceramic), angles, distances and NLoS [[7]](#s7) — *but it attaches a low-cost
transducer to the container and reads its mechanical resonance*, so it is not purely passive.
The survey of the field is unambiguous about the hardware gap: for material identification,
Wi-Fi has the lowest cost but the **worst precision** — roughly **decimetre-level, bandwidth-
limited (~160 MHz)**, needing hardware/software modification — behind UWB, RFID and mmWave,
which reach cm-level and higher class counts [[8]](#s8). Physics-based *penetration* methods
(infer permittivity from phase/ToF through the material) generalise better than reflection
kits, because they need devices only on one side [[8]](#s8).

**E4. Soil moisture — the clearest "plant/water" result — is demonstrated, but on SDR.**
Strobe maps multi-antenna relative time-of-flight and amplitude through soil to permittivity
and electrical conductivity, i.e. moisture and salinity, using Wi-Fi bands; implementation on
**USRP and WARP (SDR) plus commodity cards**, with antennas **buried in the soil** [[9]](#s9).
CoMEt goes further and is *contactless*: phase across a receive array plus ToF jointly
estimates each soil layer's depth and in-soil wavelength, giving moisture for **up to three
layers with 1.1 % median error**, implemented on an **SDR + Raspberry Pi** [[10]](#s10). A
much simpler COTS result reports strong *linear* RSSI↔water-content relationships (R²
0.92–0.99) in lab and field [[11]](#s11) — cheap, but RSSI-only, so a coarse and easily
confounded proxy. Hardware: **[SDR]** for the precise results, **[COTS]** for the coarse one.

**E5. Atmospheric state (temperature, humidity) is inferable from CSI, and this is the key
confounder.** Over 74 hours in one office, CSI collected at night (no people) still drifted
night-to-night; classifiers recovered the **hour of night (11 classes)** from CSI alone, and
regression recovered **temperature (RMSE 0.4 °C)** and **relative humidity (RMSE 1.75 %)** |
[[2]](#s2). A dedicated commodity testbed (Raspberry Pi + Nexmon, 5.18 GHz/80 MHz) raised
humidity classification to **97 %** at 5 % resolution against WiHumidity's 79 % [[12]](#s12).
Rainfall intensity is classifiable from CSI (**>90 % LoS, >85 % NLoS**, beating RSS-based
methods "by approximately 40 %") [[13]](#s13). Hardware: **[COTS]**.

**E6. Machines are distinguishable from humans and from one another.**
Industrial machinery emits **impulsive noise** whose burst shape (peak amplitude, spacing,
prominence, width) fingerprints the individual device. Deep models identify single and
simultaneous sources with **>85–99 %** accuracy in-domain, and transfer across location/day at
**~80 %** for a five-class problem with IN-features — far better than raw |CSI| (33–65 %)
[[14]](#s14). The **EHUNAM** dataset confirms the split is fundamental: "machine activity
detection differs fundamentally from human activity detection, as the CSI frames exhibit …
intrinsic characteristics, such as the presence of impulsive noise", with machine recognition
>97 % and combined human-presence + machine-activity >93 % [[15]](#s15)[[16]](#s16).
Hardware: **[COTS]** (IEEE 802.11n/ac NICs).

**E7. Human-vs-animal is solved only on radar-grade hardware, and only via physiology.**
Cats and dogs share the human respiratory-rate band, so rate alone cannot separate them; on
**IR-UWB radar** a *respiratory-to-heartbeat energy ratio* (RHER) plus an energy index does,
because the in-body structure differs [[17]](#s17). Earlier work on outdoor sensor networks
also tackles human-vs-animal classification with radar/UWB-grade hardware [[18]](#s18). On
Wi-Fi, the demonstrated animal results are *vital-sign extraction*, not classification: pig
respiration at 99.87 % from CSI [[19]](#s19), a preliminary pets-respiration study
[[20]](#s20), and the CSI-Bench "motion source recognition" task that labels humans, **pets,
Roomba robots and fans** in the same pipeline [[24]](#s24). Hardware: **[SDR/radar]** for the
classification result, **[COTS]** for the vital-sign/coexistence results.

### HYPOTHESIS

**H1. On ESP32-S3 specifically, non-human *change detection* (not fine classification) is
feasible, dominated by amplitude.** The only directly relevant ESP32 result found is
"StaticSense", which detects and classifies static objects inside shielded enclosures using
**low-cost ESP32 microcontrollers in a single-link** setup [[4]](#s4) — but the abstract does
not state accuracy, object count, or phase usage, so on the current evidence it is a
feasibility claim, not a benchmark. Supporting: ESP32 CSI is amplitude-usable out of the box
(52–56 subcarriers, HT20/40) with stable temporal structure in an *empty* room [[30]](#s30),
and non-human-adjacent tasks work on ESP32 at 100 Hz (see feasibility below). Confidence:
medium.

**H2. On ESP32-S3, material/soil/liquid *classification* is blocked** without either a wider
bandwidth front-end (raw I/Q, prompt 05), a second antenna/reference, or deliberate contact
(attached transducer, buried probe). Rationale: every precise material/soil result above used
**[SDR]**, a **[COTS]** research NIC with 80–160 MHz and 3 antennas, or contact; the survey
ranks Wi-Fi last on precision *before* degrading further to a single-antenna, 20–40 MHz
part [[8]](#s8). Confidence: medium-high.

**H3. A shared-reference phase (co-crystal/common-clock or OTA reciprocity) is the specific
enabler that turns ESP32 from "amplitude change detector" into a fine environmental sensor.**
Espressif's own motion algorithm only *begins* to use phase once the two chips are tied by a
clock buffer to "eliminate the relative frequency offset" [[27]](#s27); without it, ESP32 CSI
phase has an uncorrected slope/CFO that is not tied to path length [[28]](#s28). Since
furniture/object inference at cm-scale (E2, E5) lives in exactly the small-amplitude/slope
features, the reference is plausibly decisive. Untested for environmental targets.

### SPECULATION

**S1. The ping-and-unique-reply round-trip scheme adds little to *static* environmental
inference, and may even mislead.** Round-trip timing measures offset/range/Doppler **plus
channel** (prompt 11). For a *static* object or a closed door there is no Doppler, and the
MAC's microsecond RX timestamp gives at best ~150 m of range resolution per microsecond
[prompt 17] — useless for a chair moved 3 cm. So the round-trip's value here is (i) a shared
*time base* that stabilises the channel fingerprint, and (ii) liveness/motion gating before an
environmental query. Speculation: a round-trip is best used as the *reference plane* against
which a *channel* change is attributed, not as the environmental measurement itself. No
primary source tested this framing.

**S2. Volumetric (polyhedron) apertures would make *object localisation* — not just
detection — a model-based problem on ESP32-class hardware.** With many short baselines and a
shared clock, static-scatter geometry becomes an AoA/tomography problem (prompts 02/08), and
a static object stops being a near-noise-floor scalar and becomes a resolvable point. No
published result exists on a 3D soldered-PCB array for non-human targets; purely speculative.

---

## ESP32-S3 feasibility

**What the hardware gives (established):** every ESP32 series supports CSI, including
ESP32-S3; ESP-IDF exposes amplitude, phase, RSSI, noise floor and the RX `rx_ctrl`
(antenna) field [[26]](#s26). Espressif's own ranking places ESP32-C5 > C6 > C3 ≈ **S3** >
ESP32 [[27]](#s27). Typical ESP32 CSI is 52–56 subcarriers (802.11n HT20); newer chips expose
64/128/192/256, at the cost of UART throughput [[29]](#s29)[[30]](#s30). CSI packet rates
reported: **20 Hz** (community mesh), **100 Hz** (ESP32 person-ID) [[30]](#s30). ESP32 has
one RF chain (1–2 antennas), HT20/HT40, no 80 MHz — versus the 80 MHz / 3-antenna research
NICs used by the precise material, humidity and liquid results.

**Feasibility verdict — by target:**

| Target | On ESP32-S3 | Basis | Confidence |
|---|---|---|---|
| Door/window open-closed (coarse, fixed geometry) | Plausible amplitude-only | E1 is COTS but amplitude-driven; HMM structure helps | Medium |
| Furniture moved / object present (coarse) | Plausible | E2 low-SNR; ESP32 amplitude-only; StaticSense is ESP32 | Medium |
| Static object *class* (many classes) | Doubtful without reference/aperture | E2, H1, H2 | Low–medium |
| Machine on/off & type (impulsive noise) | Plausible | E6 uses commodity NICs; IN is an amplitude/burst feature | Medium-high |
| Human vs pet vs robot *coexistence* | Presence-level only | E7; classification needs radar/RHER or contacts | Low |
| Humidity / soil / liquid moisture (coarse change) | Marginal | E4/E5; SDR or 80 MHz for precision | Low |
| Liquid/material *identification* | Blocked (no contact) | E3 precision is bandwidth/antenna-bound | Low–very low |

**The honest gap.** Every precise non-human result (material 98 %+, liquid 97 %, soil 1.1 %,
rainfall, RHER animal classification) sits on **[SDR]**, an **80–160 MHz / 3-antenna [COTS]**
NIC, or radar hardware. The *only* ESP32-family non-human demonstration found is StaticSense
[[4]](#s4); the ESP32 two-antenna/one-chain, 20–40 MHz, uncorrected-phase configuration is the
weakest of all four hardware classes surveyed. Quantitatively, the field's own survey places
Wi-Fi material precision at **decimetre-level** even with full research hardware [[8]](#s8) —
so the ESP32 gap is large and must not be papered over.

## TX needed?

**For detection and change: no.** All of E1–E7 are passive: a transmitter exists only as an
illuminator (an AP, or a peer node's ordinary traffic). Aquila's *receive-only* mode is
sufficient for the feasible targets in the table above.

**TX becomes useful in three specific ways:**
1. **Controlled illumination / packet rate.** A dedicated `esp_wifi_80211_tx` sender (prompt
   07) sets a stable frame train; environmental inference depends on *rate stability*, and
   ESP32 CSI packet loss appears as rate rises [[29]](#s29). Sender-node TX buys reliable,
   configurable samples.
2. **Bistatic geometry for static objects.** The mobile bicstatic result [[5]](#s5) shows the
   static-object problem is partly an *aperture* problem; a cluster of senders/receivers
   (prompt 08) supplies baseline diversity.
3. **The round-trip reference plane (S1).** One node pings, unique replies timed → a common
   time/phase reference across independent LOs (prompt 11), which H3 argues is the missing
   ingredient for fine environmental sensing.

**Not needed / not sufficient:** transmitting cannot manufacture bandwidth; it does not
convert a 20 MHz ESP32 into an 80 MHz NIC. Material permittivity estimation remains
bandwidth-limited regardless of TX cleverness [[8]](#s8).

---

## Evidence strength

**7 / 10** for the question as asked (which non-human states are inferable, and with what
confidence).

**For it:** a decade of peer-reviewed primaries with high reported accuracy across *every*
sub-part — door/window state [[1]](#s1), static objects [[3]](#s3)[[4]](#s4), materials and
liquids [[6]](#s6)[[7]](#s7)[[8]](#s8), soil moisture [[9]](#s9)[[10]](#s10), humidity and
rainfall [[12]](#s12)[[13]](#s13), machinery [[14]](#s14)[[15]](#s15), human-vs-animal
[[17]](#s17) — including a purpose-built machine+human dataset [[15]](#s15).

**Against (why not 8–9):**
1. **Hardware transfer is unproven.** Almost nothing above is demonstrated on ESP32-S3; the
   one ESP32 citation [[4]](#s4) withholds accuracy.
2. **"Confidence" is in-distribution.** Reported percentages are same-setup, same-day
   accuracies; the very same papers' own robustness analyses show the model *breaks*
   (Anti-Fall: "furniture movement has quite a big impact … requests classification model
   re-construction" [[36]](#s36); transfer 33–65 % for raw CSI [[14]](#s14)).
3. **Confounders are severe and under-modelled.** cm-level furniture shifts and
   temperature/humidity drift alone move CSI enough to alter ML inference [[2]](#s2), and
   long-term concept drift degrades accuracy over a week in a *static* room [[21]](#s21).
4. **No calibrated uncertainty.** No cited work reports the confidence of a *negative*
   environmental judgment (e.g. "the door is closed, with 95 % confidence") — the actual
   sensing requirement.

Deducting for the hardware gap and the confounder/cliff nature of the task gives 7.

---

## Primary sources

<a id="s1"></a>[1] *Detecting State Changes of Indoor Everyday Objects using Wi-Fi Channel State Information.* Proc. ACM IMWUT 1(3), 2017. DOI 10.1145/3131898. https://dl.acm.org/doi/10.1145/3131898

<a id="s2"></a>[2] Turetta, Kindt, Masrur, Chakraborty, Pravadelli, Demrozi. *Environmental Microchanges in WiFi Sensing.* DATE 2024. https://past.date-conference.com/proceedings-archive/2024/DATA/10005_pdf_upload.pdf

<a id="s3"></a>[3] Zneit et al. *Static Object Classification Using WiFi Signals.* IEEE LCN 2025, 1–7. https://www.computer.org/csdl/proceedings-article/lcn/2025/11146325/2a1SPQXk6Yg

<a id="s4"></a>[4] *StaticSense: Static Object Detection using Wi-Fi Channel State Information.* IEEE, 2025 (ESP32 microcontrollers, single-link). https://ieeexplore.ieee.org/document/11485253/

<a id="s5"></a>[5] Behzad et al. *Mobile WiFi CSI-Based Object Detection: A Bistatic Approach for Enhanced Sensing in Enclosed Environment.* IEEE PerCom Workshops 2025, 507–512. DOI 10.1109/PerComWorkshops65533.2025.00116

<a id="s6"></a>[6] Li C., Li F., Du W., et al. *A Material Identification Approach Based on Wi-Fi Signal.* Computers, Materials & Continua, 2021. DOI 10.32604/cmc.2021.020765. https://www.sciencedirect.com/org/science/article/pii/S1546221821012479

<a id="s7"></a>[7] Ren Y., Tan S., Zhang L., Wang Z., Wang Z., Yang J. *Liquid Sensing Using WiFi Signals (LiquidSense).* arXiv:2106.10356 (IMWUT). https://arxiv.org/pdf/2106.10356

<a id="s8"></a>[8] Chen et al. *Wireless Sensing for Material Identification: A Survey.* IEEE COMST, 2024. https://tns.thss.tsinghua.edu.cn/sun/publications/2024.Material_Identification_COMST.pdf

<a id="s9"></a>[9] Ding J., Chandra R. *Towards Low Cost Soil Sensing Using Wi-Fi (Strobe).* ACM MobiCom 2019. DOI 10.1145/3300061.3345440. https://www.microsoft.com/en-us/research/wp-content/uploads/2019/08/Strobe_camera_ready-1.pdf

<a id="s10"></a>[10] Khan U. M., Shahzad M. *Estimating soil moisture using RF signals (CoMEt).* ACM MobiCom 2022, 242–254. DOI 10.1145/3495243.3517025

<a id="s11"></a>[11] Salman A. K., et al. *Wi-Fi signal for soil moisture sensing.* Environ. Monit. Assess. 197(1):109, 2024. DOI 10.1007/s10661-024-13587-x

<a id="s12"></a>[12] *Smart CSI Processing For Accurate Commodity WiFi-based Humidity Sensing.* arXiv:2409.07857. https://arxiv.org/html/2409.07857v1

<a id="s13"></a>[13] *RainGaugeNet: CSI-Based Sub-6 GHz Rainfall Attenuation Measurement and Classification for ISAC Applications.* arXiv:2501.02175. https://arxiv.org/html/2501.02175v1

<a id="s14"></a>[14] Landa I., Díaz G., Sobrón I., Eizmendi I., Vélez M. *Machinery detection by impulsive noise recognition using WiFi sensing.* Pervasive and Mobile Computing, 2025. DOI 10.1016/j.pmcj.2025.102018

<a id="s15"></a>[15] de Armas E., et al. *EHUNAM, a WiFi CSI-based dataset for human and machine sensing.* Scientific Data 12:1950, 2025. https://www.nature.com/articles/s41597-025-06238-4

<a id="s16"></a>[16] de Armas E., Díaz G., Sobrón I., Matías J. M., Eizmendi I. *WiFi-CSI Sensing for Human Presence and Machine Activity in Industrial Environments.* IEEE WFCS 2025. https://ieeexplore.ieee.org/document/11140205

<a id="s17"></a>[17] *Method for Distinguishing Humans and Animals in Vital Signs Monitoring Using IR-UWB Radar.* Int. J. Environ. Res. Public Health 16(22):4462, 2019. DOI 10.3390/ijerph16224462

<a id="s18"></a>[18] Zhong Y., et al. *Classification of animals and people based on radio-sensor network.* IEEE, 2016. https://ieeexplore.ieee.org/document/7751603/

<a id="s19"></a>[19] *An Animal Respiration Monitoring System Based on Channel State Information of Wi-Fi Network.* 2020. DOI 10.1145/3377170.3377196

<a id="s20"></a>[20] *Wi-Fi CSI for Animal Vital Sign Estimation: A Preliminary Investigation Using Pets in Indoor Settings.* 2025. https://www.researchgate.net/publication/398285716

<a id="s21"></a>[21] Kayano K., Ohta S., Nishio T., Yoda D., Taniguchi K., Nabetani T. *Experimental Evaluation of Long-Term Concept Drift and Its Mitigation in WiFi CSI Sensing.* IEEE CCNC 2025. DOI 10.1109/CCNC54725.2025.10975920

<a id="s22"></a>[22] *Cross-Domain WiFi Sensing with Channel State Information: A Survey.* ACM Computing Surveys, 2023. DOI 10.1145/3570325

<a id="s23"></a>[23] Yang J., et al. *MM-Fi: Multi-Modal Non-Intrusive 4D Human Dataset for Versatile Wireless Sensing.* NeurIPS 2023 Datasets & Benchmarks. https://proceedings.neurips.cc/paper_files/paper/2023/file/3baf7a39d07e9f4f1e258a412df94521-Paper-Datasets_and_Benchmarks.pdf

<a id="s24"></a>[24] Zhu G., Hu Y., Gao W., Wang W., Wang B., Liu K. *CSI-Bench: A Large-Scale In-the-Wild Dataset for Multi-task WiFi Sensing.* arXiv:2505.21866. https://arxiv.org/html/2505.21866v1

<a id="s25"></a>[25] Zheng Y., et al. *Widar3.0: Zero-effort cross-domain gesture recognition with Wi-Fi.* IEEE TPAMI 44(11):8671–8688, 2022. Dataset: https://ieee-dataport.org/open-access/widar-30-wifi-based-activity-recognition-dataset

<a id="s26"></a>[26] Espressif. *esp-csi.* https://github.com/espressif/esp-csi

<a id="s27"></a>[27] Espressif. *Introduction to ESP-CSI Solution* (chip ranking; co-crystal clock-buffer board). https://docs.espressif.com/projects/esp-techpedia/en/latest/esp-friends/solution-introduction/esp-csi/esp-csi-solution.html

<a id="s28"></a>[28] espressif/esp-csi issue #121. *Phase offsets in ESP32 CSI (AEGHB-250).* https://github.com/espressif/esp-csi/issues/121

<a id="s29"></a>[29] espressif/esp-csi issue #249. *Binary Data Output Support for Wi-Fi CSI (256 subcarriers / 100+ Hz throughput).* https://github.com/espressif/esp-csi/issues/249

<a id="s30"></a>[30] *Transformer-Based Person Identification via Wi-Fi CSI Amplitude and Phase Perturbations* (ESP32 TX/RX, 100 Hz, 52 subcarriers). arXiv:2507.12854. https://arxiv.org/html/2507.12854v1

<a id="s31"></a>[31] Banerjee... / *Wi-ESP — A tool for CSI-based Device-Free Wi-Fi Sensing (DFWS).* J. Comput. Des. Eng. 7(5):644, 2020. https://academic.oup.com/jcde/article/7/5/644/5837600

<a id="s32"></a>[32] *A Survey on Wi-Fi Sensing Generalizability: Taxonomy, Techniques, Datasets.* arXiv:2503.08008. https://arxiv.org/html/2503.08008v3

<a id="s33"></a>[33] *DATTA: Domain-Adversarial Test-Time Adaptation for Cross-Domain WiFi-Based HAR.* arXiv:2411.13284. https://arxiv.org/html/2411.13284v1

<a id="s34"></a>[34] *RF-Sensing: A New Way to Observe Surroundings.* IEEE, 2022. https://ieeexplore.ieee.org/document/9982449

<a id="s35"></a>[35] *Survey on Context-Aware Radio Frequency-Based Sensing.* Sensors 25(3):602, 2025. https://www.mdpi.com/1424-8220/25/3/602

<a id="s36"></a>[36] *Anti-Fall: A Non-intrusive and Real-time Fall Detector Leveraging CSI from Commodity WiFi Devices.* arXiv:1507.01057 (robustness-to-environment section). https://arxiv.org/pdf/1507.01057

<a id="s37"></a>[37] Iwase K., et al. *Person identification using Wi-Fi CSI focusing on door opening/closing motions.* IEEE, 2024. https://ieeexplore.ieee.org/document/10444203/

<a id="s38"></a>[38] *Using a Device-Free Wi-Fi Sensing System to Assess Daily Activities and Mobility in Low-Income Older Adults.* JMIR/PMC, 2024. https://pmc.ncbi.nlm.nih.gov/articles/PMC11599892/

<a id="s39"></a>[39] *VitalCSI: Contactless Respiratory Rate Estimation Using Consumer-Grade Wi-Fi CSI* (ESP32 comparison row). PMC, 2025. https://pmc.ncbi.nlm.nih.gov/articles/PMC12788229/

---

## Residual unknowns

1. **ESP32-S3 non-human benchmarks.** Apart from StaticSense [[4]](#s4), no published
   accuracy exists for door/furniture/object classification on ESP32-S3; whether ESP32
   amplitude-only CSI matches the COTS results (E1, E2) is unmeasured.
2. **StaticSense detail.** Accuracy, class count, whether phase or multi-link were used, and
   the enclosure geometry are all unstated in the available abstract [[4]](#s4).
3. **Confounder separability.** No primary source isolates an *environmental* target
   (door/furniture) from *atmospheric* drift and *human* activity simultaneously; the
   decomposition in [[1]](#s1) (ICA) is per-session, not per-weather.
4. **Calibrated confidence.** No cited work reports the false-alarm/confidence behaviour of an
   environmental state classifier over days, which is what a "sixth sense" product needs.
5. **Raw-I/Q for environment.** Whether gapped receive-only I/Q (prompt 05) improves
   material/soil inference over CSI on ESP32 hardware is untested; probably yes for
   permittivity (needs bandwidth), unknown for objects.
6. **Frame rate ceiling.** The maximum sustainable ESP32 CSI rate with stable, loss-free
   capture for a multi-node environmental experiment (where a centimetre-scale change may
   only appear in fine temporal structure) is unquantified; UART framing is the reported
   bottleneck [[29]](#s29).
7. **Dataset licences.** EHUNAM, MM-Fi, CSI-Bench and Widar3.0 redistribution/ commercial
   terms were not verified here and must be checked before Aquila reuses them.

---

## Chaining (A)

- **The passive CSI receive capability (#1) is the enabler for *all* feasible environmental
  targets** in the table. No TX, no I/Q, no reference is required for coarse change detection
  (door/window, large furniture, machine on/off). *"#1 enables non-human change detection."*
- **The co-crystal / shared-reference escape (prompts 10/11/12) is the highest-leverage
  enabler for *fine* environmental inference.** It is the difference between amplitude-only
  change detection and cm-scale object/material inference (H3). *"Fine environmental sensing
  is blocked without a shared phase reference; the reference enables it."*
- **Compliant TX + clusters (#3) enables a further capability:** controlled bistatic/
  multistatic illumination turns the static-object problem into an aperture problem (E2, H2,
  S2), which in turn feeds the spatial inference of prompts 02/08. *"#3 enables geometry; an
  aperture enables object localisation."*
- **Raw I/Q (#2) is a partial escape for material/soil only**, and only via bandwidth; it does
  not by itself enable anything for doors/furniture. *"#2 → permittivity/soil profiling, not
  object state."*
- **Dependency summary:** machines (E6) and doors (E1) need only #1; static object *classes*
  need #1 + reference or aperture; materials/soil need #2 or contact; volumetric 3D object
  localisation needs #3 (polyhedron) + shared clock.

## Existing models and data (B)

- **Datasets including non-human content:** **EHUNAM** (23 days, 8 environments incl. a
  factory; 21 people, **9 machines**; 802.11n/ac NICs; multiclass + multilabel) [[15]](#s15);
  **CSI-Bench** (26 in-the-wild environments, includes **pets, Roomba robots, fans** motion-
  source labels; Broadcom/Qualcomm/MediaTek devices) [[24]](#s24). Both are usable for
  machine/robustness work but are **not** ESP32-format and do not target doors/furniture.
- **Human-centric datasets** (adaptable, not environmental): MM-Fi (CSI + RGB-D + LiDAR +
  mmWave) [[23]](#s23); Widar3.0 [[25]](#s25); SignFi, WiAR, UT-HAR, XRF55 (via the
  generalizability survey) [[32]](#s32).
- **Models/methods:** ICA + CNN/RNN + HMM for object states [[1]](#s1); k-NN instance-based
  for static objects [[3]](#s3); physics-based permittivity models for materials/liquids
  [[7]](#s7)[[8]](#s8); relative-ToF permittivity for soil [[9]](#s9)[[10]](#s10); impulsive-
  noise feature extraction + ProtoNet/Siamese few-shot for machines [[14]](#s14); domain
  adaptation for the drift problem — DATTA, KNN-MMD, RF-EATS [[8]](#s8)[[22]](#s22)[[33]](#s33).
- **Input-format caveat.** Nearly every dataset/model assumes Intel 5300/Atheros/Nexmon
  (30–114 subcarriers, 3 antennas, up to 80–160 MHz). An ESP32-S3 front-end must
  resample/align — a domain-adaptation problem, not a plug-in [[32]](#s32).
- **Licences unverified** (see residual unknown 7) — do not assume commercial reuse.

## Own-data / new technology (C)

**Claim a purpose-built Aquila dataset could prove:** that an ESP32-S3 link, with a shared
phase reference and a fixed geometry, can report **door/window state and furniture layout**
with calibrated confidence *while explicitly regressing out temperature and humidity* — i.e.
that the confounder in E5 can be measured and removed rather than ignored. No public dataset
isolates an environmental target from atmospheric drift on ESP32 hardware.

**Minimal experiment (one room, two ESP32-S3 nodes + optional co-crystal pair):**
1. Log CSI (amplitude; phase if co-crystal) at a stable rate against a fixed AP/peer.
2. Script a labelled set of non-human changes, each repeated across ≥5 days:
   door open/closed; window open/closed; a wheeled chair at N marked positions (cm apart, per
   E2/E5); a water container filled in steps; a machine switched on/off.
3. Record ground truth with independent cheap sensors: reed switches, a BME280
   (temperature/humidity), a load cell, an ultrasonic level sensor.
4. Train (a) an amplitude-only classifier, (b) the same with temperature/humidity as
   nuisance regressors, (c) a shared-reference phase variant. Compare per-day and cross-day.

**Falsified if:** after nuisance regression, cross-day accuracy for door/furniture/machine
state is at chance, *or* the shared-reference phase variant shows no gain over amplitude-only,
*or* temperature/humidity alone separate the classes as well as the target variable does
(i.e. the target was never observable). Any of these kills the ESP32 environmental-sensing
goal at that geometry.

**What would be genuinely new:** the first **ESP32-S3 non-human state benchmark with
atmospheric nuisance control and a co-crystal phase arm**, plus a per-day drift curve. Nothing
in the literature does this for anything but humans.

**Second, orthogonal experiment (round-trip, S1):** run the ping-and-unique-reply scheme
alongside the environmental log and test whether the round-trip reference *raises* cross-day
classification accuracy by stabilising the channel fingerprint. Falsified if the reference
plane gives no accuracy or stability gain over ordinary CSI correlation.
