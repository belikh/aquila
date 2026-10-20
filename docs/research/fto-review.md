# Patent Landscape / Freedom-to-Operate (FTO) Review — Aquila WiFi CSI Sensing

**Project:** Aquila — WiFi CSI sensing on ESP32-S3 nodes (github.com/belikh/aquila)
**Scope:** v1 spec — feature-level fusion, no cross-node phase coherence, global whole-home calibration, GPLv3-only code, non-commercial
**Prepared:** 2026-10-18
**Classification:** Advisory — not legal advice

---

## Disclaimer

This report is a preliminary freedom-to-operate and patent landscape review prepared for internal project planning. **It is not legal advice.** It does not constitute a clearance opinion, infringement analysis, or validity assessment of any identified patent. Patent statuses (granted / published / expired / abandoned) are as reported by public sources at time of writing and may be inaccurate or stale. A qualified patent attorney, preferably with wireless-sensing and standard-essential-patent (SEP) experience, must be consulted before any filing, product release, or reliance on design-around strategies. Jurisdictions of particular relevance: Australia (project jurisdiction), United States (ESP32/Espressif and many sensing patentees), Canada (Origin/Aerial/Cognitive Systems), Europe (EP filings), and China (numerous CSI-sensing filings).

---

## 1. Executive summary

Aquila operates in a crowded, fast-moving patent space. WiFi CSI sensing has been patented aggressively since roughly 2013–2016, and the filing rate accelerated after the IEEE 802.11bf WLAN sensing PAR was approved (2020) and the standard published (IEEE Std 802.11bf-2025, 26 September 2025). The space is dominated by a small number of well-funded assignees — Origin Wireless (acquired by ADT in Feb 2026; 220+ filed patents), Cognitive Systems Corp. (450+ awarded patents across RF sensing), Aerial Technologies / nami.ai (73 filed/pending patents after the nami–Aerial strategic combination), Mitsubishi Electric Research Laboratories, and patent-pool administrators Sisvel and Via Licensing — alongside academic assignees (e.g. Singapore University of Technology and Design) and Chinese filers.

Aquila's deliberate architectural choices materially reduce its FTO exposure:

- **No cross-node phase coherence** — independent local oscillators make absolute cross-node phase unobservable; only gauge-invariant combinations (closure phase, two-way product/ratio, ambient cross-receiver ratio) are used. This avoids a large family of phase-coherent array / AoA / ToF / distributed-beamforming patents.
- **Feature-level fusion only** — no coherent complex combining across nodes; magnitudes and event timing only.
- **Passive / illuminator-of-opportunity reception** — Aquila nodes receive existing WiFi traffic; they do not steer, transmit dedicated sensing waveforms, or perform active probing beyond compliant 802.11 operation.
- **ESP32-S3 commodity nodes** — uses Espressif's published ESP-CSI capture path (Apache-2.0 repo), not raw I/Q or an ESP32-as-SDR.

The areas of **greatest FTO concern** are (i) multi-node / mesh sensing topology, where nami holds a patented mesh approach and others (Aerial, Origin, A. Zandifar / Aerial) claim multi-transceiver sensing areas; (ii) passive / ambient occupancy detection, where Aerial Technologies holds a family (US12044789B2, priority 2016) covering CSI-based occupancy with multiple transceivers; and (iii) the 802.11bf SEP thicket, where declared and undeclared SEPs exist but are primarily a risk for *implementers of the standard*, which Aquila is not (it uses legacy 802.11n/ac/ax CSI capture, not 11bf sensing procedures).

The area of **greatest opportunity / novelty** is closure-phase / gauge-invariant multi-node sensing across directed loops with independent LOs. No WiFi closure-phase patent was found in this review; the project's drafted closure-phase application ("Wi-Fi Sensing Mesh for Three-Dimensional Scatterer Tracking Using Jennison Closure Phase Across Directed Multi-Node Packet Loops") targets a genuinely open niche — provided its claims are carefully drafted to avoid read-on to generic multi-link CSI fusion.

GPLv3-only, non-commercial code shapes strategy strongly: GPLv3 §11 grants an implicit patent license to downstream recipients and contains a patent retaliation clause (conveying a covered work while asserting a patent against it terminates the license). This *discourages* Aquila from asserting patents against downstream users and *encourages* defensive publication over offensive patenting. Non-commercial use also reduces the commercial enforcement incentive of patentees, though it does not eliminate infringement risk.

---

## 2. Patent landscape by area

### 2.1 WiFi CSI-based sensing (presence, occupancy, gesture, vital signs)

This is the core and most densely patented area. Assignees and representative records:

| Assignee | Representative record(s) | Type / status | Notes |
|---|---|---|---|
| Origin Wireless Inc. (acquired by ADT, Feb 2026) | US10833912B2 (priority 2012-12-05); US10374863B2 (priority 2012-12-05); WO2016011433A2; US11444710; US11448728 | Grants + applications | Founded by K. J. Ray Liu (Univ. of Maryland); "AI Sensing"; 220+ filed patents; driving force behind 802.11bf. Covers presence, fall detection, vital signs, occupancy. |
| Aerial Technologies Inc. | US12044789B2 (priority 2016-04-22, granted 2024-07-23, expires 2038-06-21); US10705179B2; US10779127B2; US11043094B2; US11348428B2; US11774542B2 | Grants (family) | Inventor Azar Zandifar et al. CSI-based occupancy detection using multiple transceivers, device-free, LSTM/sequence models, auxiliary info. Directly overlaps Aquila presence/occupancy claims. |
| Cognitive Systems Corp. | 450+ awarded patents (RF sensing, motion network intelligence, AI); 17+ issued/allowed US patents (per company, ca. 2019) | Grants | Aura WiFi Motion. Strong patent portfolio per founder statement. |
| nami.ai / Aerial.ai | nami mesh topology patent (per nami.ai); combined portfolio ~73 filed/pending (RF sensing, AI sensing algorithms, mesh architectures) | Filed/pending | nami took strategic shareholding in Aerial.ai in 2025. |
| Mitsubishi Electric Research Labs (MERL) | US11902811B2 / US20220286885A1 (priority 2021-03-08, granted 2024-02-13, expires 2041-03-31); EP4197216B1; WO2022190721A1; CN117044264A; JP7562021B2 | Granted (US/EP/JP) + applications | Multi-band Wi-Fi fusion for WLAN sensing (CSI sub-7 GHz + beam SNR at 60 GHz), autoencoder fusion, occupancy/people counting/localization. Inventors incl. Philip Orlik, Pu Wang, Jianyuan Yu, Toshiaki Koike-Akino, Ye Wang. |
| Chinese academic/industry filers | CN110149604B (people detection via WiFi CSI variance; active); EP4546671A1 (published 2025-04-30, priority CN 2022-07-25, PCT/CN2023/096573) | Granted / published | CN110149604B uses static/dynamic CSI correlation and adaptive thresholding — low-level feature patent. EP4546671A1 is a WLAN sensing method/system. |
| Singapore University of Technology and Design | US20150358696A1 / WO2014109710A1 (distributed wireless sensing) | Application | Early distributed sensing work. |
| (Generic / academic) | — | Prior art | The William & Mary CSI survey (Ma, Zhou, Wang, ACM CSUR 2019) and RuView blog document extensive pre-2019 prior art (RSSI/CSI variance, EMA thresholds, per-link motion detection). |

**Landscape assessment:** Presence/occupancy via CSI thresholding and variance is well-patented (Origin, Aerial, CN filers). However, much of the earliest academic work (2012–2016) is now public prior art and some early patents may be approaching expiry or have lapsed for non-payment of fees. Aquila's v1 (global calibration, feature-level fusion, per-zone gated presence) overlaps conceptually with Aerial's family but differs in architecture (no multi-transceiver "active sensing area" marketing; dumb nodes; local-first; no cloud). Infringement analysis by counsel is required per claim.

### 2.2 Multi-node / mesh WiFi sensing systems

This area is directly relevant to Aquila's multi-node ESP32-S3 deployment.

| Assignee | Record | Type / status | Relevance to Aquila |
|---|---|---|---|
| nami.ai | Mesh/multi-node topology patent (per nami.ai "Wi-Fi Mesh Sensing" page); joint Aerial–nami portfolio ~73 filed/pending | Filed/pending | nami claims its multi-node mesh topology (each device = sensing node + network participant; nodes transmit NDPs and extract CSI from peers' NDPs) is patented independently of CSI engine. **High relevance** — Aquila is multi-node but uses passive reception of ambient WiFi, not NDP exchange between sensing nodes. Design-around: receive-only, no sounding-frame transmission between Aquila nodes. |
| Aerial Technologies | Multi-transceiver sensing-area family (US12044789B2 et al.; "at least two transceivers"; sensing area extended by number/location of devices) | Granted | Claims a sensing area created by multiple transceivers, motion quantification, aggregation (PCA example), sequence-based learner. Aquila's multi-node fusion is feature-level (log-odds pooling, PMF mixture), not coherent combining; but the "multiple transceivers → occupancy" concept is close. |
| MERL | US11902811B2 (multi-band fusion across radio devices; "sensing receivers… coordinated or uncoordinated") | Granted | Covers sharing CSI/beam attributes across multiple sensing receivers. Aquila shares per-node features, not multi-band CSI/beam SNR. |
| Origin Wireless | Client-to-client / peer-to-peer sensing mesh; Soft-AP sub-network | Product (patented per website) | Origin markets peer-to-peer CSI exchange between IoT devices. Aquila nodes stream to a core; they do not peer-exchange CSI. |

**Landscape assessment:** Multi-node mesh sensing is a live patent zone. nami's patent and Aerial's multi-transceiver family are the closest potential blockers for a multi-node ESP32 system. Aquila's key distinctions (dumb receive-only nodes; no NDP/sounding exchange between nodes; feature-level not coherent fusion; no cross-node phase) are design-around candidates that counsel should map to claim language.

### 2.3 Closure phase / interferometric sensing applied to WiFi or radio

**No WiFi-specific closure-phase patent was found.** This is Aquila's principal open niche.

- Radio-astronomy closure phase (Jennison 1958; Wilkinson et al.) is foundational and long in the public domain. It is not patentable as such and is prior art against any closure-phase claim.
- Optical/fibre interferometric sensing patents exist (e.g. US6671055B1 Luna Innovations; various Apple self-mixing interference patents; Australian National University optical interferometric sensing AU2022341028A1) but are optical-domain, not WiFi CSI.
- No record was found of a patent claiming closure phase, two-way CSI product/ratio, or ambient cross-receiver ratio applied to *commodity WiFi CSI with independent LOs*.

**Assessment:** This supports the project's drafted closure-phase application. Novelty hinges on (a) applying Jennison closure phase to directed multi-node WiFi packet loops, (b) doing so with *independent* node LOs (no phase coherence), and (c) using only gauge-invariant combinations. Counsel should run a novelty search in G01S (radio direction-finding / ranging) and H04B (transmission) classes before filing, specifically for "closure phase", "bispectrum", "triple product", "two-way channel reciprocity", and "directed loop" combined with WiFi/CSI/WLAN.

### 2.4 Two-way reciprocal CSI / channel estimation for sensing

| Assignee | Record | Type / status | Relevance |
|---|---|---|---|
| InterDigital Patent Holdings | US20130039349A1 (CSI feedback in CoMP); US20090075686A1 (two-way training for multi-user MIMO) | Applications (2009–2011) | Channel reciprocity and two-way training — communication-oriented, not sensing. |
| Research In Motion / Antonio Forenza | US20090067402A1 (distributed input-distributed output) | Application (2007) | DIDO — communication, not sensing. |
| US10547358B2 / US11146313B2 / US11581924B2 | RF calibration exploiting channel reciprocity in distributed wireless | Granted | Reciprocity-based calibration — comms-oriented. Various national family members (AU, IL, TW, CN, KR). |
| Samsung Research America | "Optimal Preprocessing of WiFi CSI for Sensing" (gain/phase error model) | Paper (not patent) | Prior art / technical reference; not a patent. |

**Assessment:** Reciprocity/calibration patents are predominantly communication-standard-oriented (MIMO calibration, beamforming reciprocity) rather than WiFi-sensing-oriented. Aquila's per-node calibration (clock offset/skew, amplitude/phase drift basis) and its *rejection* of cross-node phase coherence keep it outside the core of these families. However, if Aquila ever adopts two-way reciprocal CSI between nodes for ranging/calibration, counsel should check these families.

### 2.5 Passive WiFi sensing using ambient traffic / illuminator-of-opportunity

| Assignee / source | Record | Type / status | Notes |
|---|---|---|---|
| B. Tan, K. Woodbridge, K. Chetty | P102447GB Passive Wi-Fi Sensing | UK patent application | Cited on "Passive WiFi Sensing" site; through-wall detection, Doppler, SDR architecture, covert/undetectable. |
| US9696421B2 | Through-wall sensing system using WiFi/cellular radar | Granted | Colone et al. passive bistatic WiFi radar; references Chetty et al. (2012) through-the-wall personnel sensing. |
| UCL (Li & Bocus) | "A Taxonomy of WiFi Sensing: CSI vs Passive WiFi Radar" (IEEE TMC 2017) | Paper | Distinguishes CSI (comms extension) from PWR (radar principles). |
| Origin / Linksys | "Linksys Aware" WiFi motion service | Product | Commercial passive sensing on existing WiFi. |

**Assessment:** Passive WiFi radar / illuminator-of-opportunity is patented (Tan/Woodbridge/Chetty; US9696421B2). Aquila's reception of ambient WiFi traffic is conceptually passive sensing, but Aquila uses CSI from ESP32 reception of its own network's frames (not an external illuminator radar architecture) and does not claim through-wall radar or Doppler radar processing. The distinction matters at claim level.

### 2.6 IEEE 802.11bf WLAN sensing standard-related patents

- **IEEE Std 802.11bf-2025** published 26 September 2025 (confirmed via IEEE SA). TGbf formed October 2020; PAR approved September 2020. TGbf leadership included Huawei, LG Electronics, Qualcomm, Ericsson, Meta Platforms (per ABI Research).
- **SEP landscape:** IEEE LOA (Letter of Assurance) disclosures are typically blanket; actual essential patents are rarely disclosed. Sisvel runs a Wi-Fi Multimode pool (Wi-Fi 6 + Wi-Fi 7 SEPs; 11 patent owners; $0.50/unit consumer rate for Wi-Fi 6 pool; broader for multimode). Via Licensing administers pools. IPlytics/Greyb analysis suggests ~4,000 Wi-Fi 7 patent families, ~1,700 active, ~47% of sampled active families truly essential; ~90% undeclared to IEEE.
- **11bf-specific:** WO2023212082A1 ("Methods for sensing in a WLAN", published 2023) references 802.11bf and describes sensing session/measurement setup; assignee not clearly identified in results. LG, Intel, Meta, OPPO contributed to 11bf draft text.
- **Aquila relevance:** Aquila does *not* implement 802.11bf sensing procedures (NDP sounding, sensing measurement beams, 11bf MAC/PHY). It captures CSI on legacy 802.11n/ac/ax hardware via ESP-CSI. Therefore Aquila is not a 11bf implementer and 11bf SEPs are not a direct infringement risk — *unless* Aquila later adopts 11bf sensing features, at which point SEP licensing (RAND, likely via pools) becomes relevant. The bigger risk is that 11bf products incorporate sensing and that 11bf SEP holders' broader WiFi SEP portfolios may read on Aquila's WiFi usage — but WiFi SEP pools already cover compliant WiFi operation, so Aquila's use of standard WiFi is presumably licensed via the chip/device vendor (Espressif) or pool.

### 2.7 Wi-Fi Alliance sensing certifications / patent pools

- **Wi-Fi Alliance:** No public "Wi-Fi CERTIFIED Sensing" program was identified in results. Wi-Fi Alliance runs interoperability certification; it does not itself license patents. Any 11bf certification program would sit alongside (not replace) SEP licensing.
- **Patent pools:** Sisvel Wi-Fi Multimode (Wi-Fi 6 + 7 SEPs, 11 owners, RAND); Via Licensing (various pools); historically Sisvel 802.11 joint licensing (Ericsson, ETRI, Hera Wireless, Nokia, Sanyo). None of these is a "WiFi sensing pool" per se, but sensing implementations on WiFi chips may touch the underlying WiFi SEP portfolio.
- **Assessment:** For Aquila (non-commercial, uses commodity ESP32-S3 WiFi), WiFi SEP pool exposure is indirect and likely exhausted via Espressif's chip licensing. This is not a primary FTO concern unless Aquila builds/sells its own WiFi silicon.

### 2.8 ESP32 / ESP-IDF CSI capture patents (Espressif)

- **No Espressif CSI patent was found.** Espressif publishes ESP-CSI under Apache-2.0 (github.com/espressif/esp-csi) and documents it in ESP-IDF / ESP-Techpedia. ESP-CSI supports ESP32/S2/S3/C3/C5/C6/C61; self-transmission/reception mode exists on ESP32-C5 (mm-level phase sensing).
- Espressif patents (if any) relating to CSI capture were not identifiable from public web sources. Espressif's IP model appears to be open reference software + hardware sales, not patent enforcement on CSI capture.
- **Assessment:** Low FTO concern from Espressif specifically. The main hardware risk is general WiFi SEPs on the ESP32-S3 radio, which Espressif (as the chip vendor) would carry for compliant WiFi operation. Aquila's software use of ESP-CSI is Apache-2.0 and GPLv3-compatible.

---

## 3. FTO analysis per Aquila invention area

The task references "inventions a–t" and a closure-phase patent. These invention labels and the `docs/patent/` directory do not yet exist in the repository; the closure-phase application is described as "already drafted" but is not present in the repo. Below I analyze the *invention areas* embodied in the v1 spec, mapping each to FTO risk. The project should assign invention IDs (a–t) and create `docs/patent/` as part of next steps.

| # | Invention area (per v1 spec) | FTO risk | Rationale / nearest patents | Design-around |
|---|---|---|---|---|
| A | Raw CSI capture from ESP32-S3 via ESP-CSI over WiFi UDP | Low | No Espressif CSI patent found; ESP-CSI is Apache-2.0; WiFi SEPs likely exhausted at chip level. | Keep using published ESP-CSI path; avoid modifying radio firmware to emit non-standard waveforms. |
| B | Per-node CSI DSP: phase sanitisation, unwrap, bandpass, STFT, BVP/vitals extraction | Low–moderate | Well-known DSP (per William & Mary survey, RuView); RuView-style prior art is public. Patents on specific filtering may exist but are narrow. | Pipeline is deterministic, frozen-config, published in spec; relies on known techniques. |
| C | Per-node presence / motion from baseline-subtracted CSI | Moderate | Covered conceptually by CN110149604B (CSI variance/threshold), Origin, Aerial. Baseline + threshold is old prior art but patented in specific embodiments. | Use adaptive, per-zone, calibration-derived thresholds (already spec'd); avoid literal "static vs dynamic CSI variance" claim language. |
| D | Per-zone occupancy / presence with gated claim (F1 ≥ 0.9) | Moderate–high | Aerial US12044789B2 et al. (CSI occupancy, multiple transceivers, sequence/LSTM learner, auxiliary info, post-hoc correction). Direct conceptual overlap. | Distinguish: dumb receive-only nodes; no active sensing area; no cloud; local-first; per-home adaptation via git PR. Counsel claim comparison required. |
| E | Person count (mixture of per-node PMFs; no double-counting within room) | Moderate | Counting via WiFi CSI is an established application (survey Table 9); specific mixture/convolution methods may be patented. | Keep count advisory, quality-weighted, uncertainty-published; avoid claiming exact count as a hard output. |
| F | Coarse per-node pose (torso PCK@20) | Moderate | Pose from WiFi CSI is patent-adjacent (CMU DensePose-from-WiFi; ruvnet weights; various academic work). "Coarse, view-relative, no identity/gait" limits scope. | Keep pose coarse, per-node, view-relative; no world-frame triangulation; no identity. |
| G | Breathing / heart rate from CSI (bandpass + resonator/Goertzel + STFT) | Low–moderate | Vital-sign CSI estimation is widely published (2012–2019 prior art); patents exist but the DSP is standard. | Use published bands (breathing 0.10–0.60 Hz; heart 0.80–2.50 Hz); publish reproducible pipeline. |
| H | Feature-level multi-node fusion (log-odds pooling, PMF mixture, convolution across rooms/floors) | Moderate | Cross-device CSI fusion is claimed by MERL US11902811B2 and Aerial aggregation. Feature-level (not coherent) fusion reduces but does not eliminate overlap. | Explicitly uncoherent; no cross-node phase; magnitudes/timing only; `Coherence::Uncoherent` enforced in code. |
| I | Software time sync (NTP-style 4-timestamp, drift fit, RTT gating) | Low | Standard NTP/clock-offset techniques; not WiFi-sensing-specific; unlikely to be sensibly patented for this use. | Keep software-only, millisecond budget; no hardware RF reference. |
| J | Global whole-home calibration artefact (versioned, content-addressed, invalidation) | Low | Calibration artefacts/versioning are systems-engineering, not core sensing IP. Low patent density. | Already local-first and commit-keyed; no novel claim needed. |
| K | Proximity fingerprinting + bounded RSSI trilateration with 95% error ellipse, advisory demotion | Low–moderate | Indoor RSSI trilateration is old; error-ellipse demotion is a honesty feature, unlikely patented. | Keep advisory-only, never a hard input to presence/pose/vitals. |
| L | No cross-node phase coherence; independent LOs; gauge-invariant only | **Defensive strength** | Avoids phase-coherent array / AoA / ToF / distributed beamforming patents (a large category). | Preserve as an explicit architectural invariant; do not add coherent combining. |
| M | Closure phase / two-way product/ratio across directed multi-node packet loops (Jennison) | **Novelty opportunity** (patentable) | No WiFi closure-phase patent found. Radio-astronomy closure phase is public-domain prior art (supports novelty, not infringement). | Draft claims carefully to cover *application* to WiFi CSI with independent LOs, not the math itself. |
| N | Passive reception / illuminator-of-opportunity (ambient WiFi traffic) | Moderate | Passive WiFi radar patents (Tan/Woodbridge/Chetty P102447GB; US9696421B2). | Aquila uses own-network CSI frames, not external illuminator radar; no through-wall/Doppler radar claims. |
| O | Camera-free runtime; no camera ground truth as runtime input | Low | Not an invention; a scope exclusion. Privacy feature. | Maintain. |
| P | Per-home adaptation via git PR (not over-the-air learning) | Low | Process/IP-management choice, not a technical invention. | Maintain. |
| Q | Local-first; private control plane; no public REST/WS/MCP/SDK | Low | Architectural/privacy choice. | Maintain (also reduces attack surface and third-party IP exposure). |
| R | Firmware OTA (signed, A/B, rollback, anti-rollback) via HA update entity | Low | OTA is well-known; Espressif ESP-IDF OTA documented. Signed A/B rollback is common. | No novel claim needed; use standard ESP-IDF. |
| S | Witness evidence grading (Gated/Measured/Claimed/Data-gated/Hardware-gated) | Low | Process/claims-integrity mechanism; not a sensing invention. | Maintain. |
| T | GPLv3-only code + CC-BY-NC data/weights separation | Strategic (licence, not patent) | GPLv3 §11 patent license + retaliation. | Keep code GPLv3-only; keep NC data/weights out of GPL history (per inference-models.md §6). |

### Overall FTO verdict

- **Highest risk:** Areas D (occupancy) and H (multi-node fusion), due to Aerial's granted family and nami's mesh patent. MERL's multi-band fusion is a secondary concern but less on-point (Aquila is single-band, sub-7 GHz CSI only).
- **Moderate risk:** Areas C, E, F, G, J — established application areas with scattered patents; narrow design-arounds available.
- **Low risk:** A, B, I, K, L, O, P, Q, R, S — either well-known prior art, standard techniques, or architectural exclusions.
- **Open novelty:** Area M (closure phase across directed loops with independent LOs) — supports a patent application; also the area most likely to *avoid* existing blockers.

---

## 4. Key patent holders / assignees

| Holder | Location | Portfolio relevance | Notes |
|---|---|---|---|
| Origin Wireless (acquired by ADT Inc., Feb 2026) | USA (College Park, MD) | 220+ filed patents; presence, fall, vitals, occupancy; driving force behind 802.11bf | Founder K. J. Ray Liu; now part of ADT. Largest dedicated WiFi-sensing patent portfolio. |
| Cognitive Systems Corp. | Canada (Ottawa) | 450+ awarded patents (RF sensing, motion network intelligence, AI); 17+ US issued/allowed (ca. 2019) | "Aura WiFi Motion"; aggressive early patenting (founder statement). |
| Aerial Technologies Inc. | Canada (Montreal) | US12044789B2 family (occupancy, multiple transceivers); ~73 filed/pending combined with nami | Acquirer/partner of nami (2025). Direct overlap with Aquila occupancy/multi-node. |
| nami.ai | Singapore / global | Mesh multi-node topology patent; joint portfolio with Aerial | Claims multi-node mesh sensing independent of CSI engine. |
| Mitsubishi Electric Research Labs (MERL) | USA | US11902811B2 / multi-band CSI+beam-SNR fusion; EP4197216B1; WO2022190721A1 | Multi-band fusion, occupancy/counting/localization; granted US/EP/JP. |
| Espressif Systems | China | No CSI patent found; ESP-CSI Apache-2.0 | Chip vendor; WiFi SEP exposure carried at silicon level. |
| Sisvel | Luxembourg | Wi-Fi Multimode pool (Wi-Fi 6 + 7 SEPs; 11 owners); historic 802.11 pool | SEP pool administrator; $0.50/unit Wi-Fi 6 consumer rate. Indirect relevance only. |
| Via Licensing Alliance | USA | Patent pool administrator | Indirect relevance. |
| CSIRO | Australia | WLAN patents (expired/lapsed; US '069 granted 1996, 18 counterpart countries) | Foundational WiFi IP; largely expired. Not a sensing player. |
| Academic / research | Singapore (SUTD), William & Mary (US), Tsinghua (UCL/UK), etc. | Early distributed/CSI sensing filings; much is public prior art | Prior-art sources rather than active enforcers. |

---

## 5. Relevant patent / publication numbers (where found)

All numbers below were observed in public search results; statuses are as reported by Google Patents / assignee sites and **must be re-verified with counsel** (Google Patents legal-status lines are explicitly not legal conclusions).

| Number | Kind | Assignee | Title / topic | Status (as reported) |
|---|---|---|---|---|
| US12044789B2 | Grant | Aerial Technologies | Occupancy detection using WiFi sensing (multiple transceivers, CSI, sequence model) | Active; expires 2038-06-21 |
| US20210333351A1 | Application | Aerial / Zandifar et al. | Same family (priority 2016-04-22) | Published |
| US10705179B2, US10779127B2, US11043094B2, US11348428B2, US11774542B2 | Grants | Aerial / Zandifar et al. | Occupancy / sensing family continuations | Granted |
| US11902811B2 | Grant | MERL | Multi-band Wi-Fi fusion for WLAN sensing | Granted 2024-02-13; expires 2041-03-31 |
| US20220286885A1 | Application | MERL | Same family (priority 2021-03-08) | Published; granted as above |
| EP4197216B1, WO2022190721A1, CN117044264A, JP7562021B2 | Various | MERL | Same multi-band fusion family | Granted/published |
| CN110149604B | Grant | Chinese inventors | People detection via WiFi CSI variance + adaptive threshold | Active |
| US9696421B2 | Grant | — | Through-wall sensing using WiFi/cellular radar (passive bistatic) | Granted |
| P102447GB | Application | Tan / Woodbridge / Chetty | Passive Wi-Fi sensing | UK application |
| US10547358B2 / US11146313B2 / US11581924B2 | Grants | — | RF calibration exploiting channel reciprocity (distributed wireless) | Granted; multiple national family members |
| US10833912B2 | Grant | Origin Wireless | Wireless IoT methods/apparatus/systems | Granted (priority 2012-12-05) |
| US10374863B2 | Grant | Origin Wireless | Event recognition based on wireless signal | Granted (priority 2012-12-05) |
| WO2016011433A2 | Application | Origin Wireless | Wireless positioning systems | Published |
| US11444710, US11448728 | Grants | Origin Wireless | Wireless sensing methods/apparatus/systems | Granted (filed 2020-2021) |
| US20150358696A1 / WO2014109710A1 | Application | SUTD | Distributed wireless sensing | Application |
| EP4546671A1 | EP application | (CN priority 2022-07-25; PCT/CN2023/096573) | WLAN sensing method and system | Published 2025-04-30 |
| WO2023212082A1 | WO application | — | Methods for sensing in a WLAN (802.11bf-related) | Published 2023 |
| IEEE Std 802.11bf-2025 | Standard | IEEE | WLAN sensing amendment | Published 2025-09-26 |

**Not found (searched, no record):** any patent combining "closure phase" / "Jennison" / "bispectrum" / "triple product" with WiFi CSI sensing; any Espressif CSI-capture patent; any Wi-Fi Alliance sensing patent pool.

---

## 6. Strategic recommendations

### 6.1 Should Aquila file its own patent application? When?

**Yes — but selectively and defensively.** The strongest candidate is the closure-phase / gauge-invariant multi-node invention (area M), which is (a) apparently novel in the WiFi domain, (b) central to Aquila's differentiator (multi-node sensing without phase coherence), and (c) the area least likely to read on existing blockers.

**Timing:** File (provisional or PCT) *before* any public disclosure that would jeopardize novelty — including before publishing the closure-phase algorithm, demoing multi-node closure-phase sensing at 10-device validation, or posting detailed technical write-ups. Given 10 ESP32-S3 devices arriving soon for hardware validation, file before those validation results are published or demoed publicly. A provisional now, followed by a PCT/national-phase decision within 12 months, is the standard prudent path.

**Caveat:** GPLv3-only code undercuts the value of *asserting* a patent (GPLv3 §11 grants downstream recipients an implicit patent license and the retaliation clause terminates the license if Aquila sues a downstream user). The patent's value is therefore primarily **defensive** (deterrence, freedom to operate, blocking competitors, acquisition/partnership leverage) rather than **offensive** (licensing revenue). This is consistent with a non-commercial project.

### 6.2 What goes in the patent vs. trade secret

| In patent application | Kept as trade secret / not disclosed |
|---|---|
| Closure-phase formulation across directed multi-node packet loops with independent LOs; gauge-invariant combinations (two-way product/ratio, ambient cross-receiver ratio); the specific loop construction and phase-cancellation algebra as applied to WiFi CSI. | Exact ESP32-S3 CSI byte layout and driver quirks (until verified/frozen); the specific trained model weights and fine-tuning data (NC, separate repo); per-home calibration thresholds and signature values (customer-specific); implementation-level DSP constants and optimisation details; the "git PR per-home adaptation" workflow. |
| The architectural invariant "no cross-node phase coherence" as a claim feature (defensive, to preempt others patenting coherent-array approaches that Aquila might later want to use). | |

Trade secret is only viable while details remain unpublished. Since Aquila is open-source (GPLv3), the *code* cannot be a trade secret; the patent should protect the *inventive concept*, not the implementation.

### 6.3 Defensive publication strategy

Defensive publication is well-suited to Aquila because:
- GPLv3 + non-commercial makes offensive enforcement unattractive/inconsistent.
- Publishing establishes prior art, blocking others from patenting the same subject matter.
- It is low-cost and immediate (no prosecution fees, no delay).

**What to defensively publish:** The full v1 signal pipeline (signal.md), feature-level fusion rules (fusion-sync.md), calibration artefact and invalidation policy (geometry-calibration.md), the time-sync method (fusion-sync.md §1), and the explicit "no cross-node phase coherence" design rule. These should be published in a dated, citable form — the repo itself (with git timestamps), ideally supplemented by a formal defensive publication (e.g., IP.com, or a technical report with DOI) for the core inventions not being patented.

**What NOT to defensively publish:** Anything Aquila intends to patent (closure phase) — publication before filing destroys novelty in most jurisdictions (Australia and most of the world are absolute-novelty; the US has a 1-year grace period but it is risky to rely on it).

### 6.4 Open-source defensive disclosure

Aquila's existing model is already a strong open-source defensive disclosure:
- GPLv3-only code (public, dated, via GitHub).
- Public spec and docs (this repo).
- Commit-keyed measurement ledger (provides dated evidence of conception/reduction to practice).

**Recommendation:** Continue and formalise. Add a `PATENTS.md` or `docs/patent/IP-policy.md` stating: (a) Aquila's patent stance (defensive only; no assertion against non-commercial users or downstream GPLv3 recipients); (b) an express, irrevocable patent non-assertion covenant for non-commercial and GPLv3-compliant use (to be drafted by counsel); (c) encouragement of defensive publication of improvements. This both deters opportunistic patenting by others and aligns with the project's non-commercial ethos.

### 6.5 Design-around summary

If counsel identifies a specific blocking patent, Aquila's design-around levers (already embedded in v1) are:

1. **Never transmit sensing frames between Aquila nodes** — receive only; avoids nami mesh patent and active-sounding claims.
2. **Never combine raw CSI phase across nodes** — magnitudes, timing, and gauge-invariant combinations only; avoids coherent-array / AoA / ToF patents.
3. **Single-band sub-7 GHz CSI only** — avoids MERL multi-band (CSI + 60 GHz beam SNR) claims.
4. **No dedicated illuminator / radar waveform** — uses ambient WiFi traffic; avoids passive-bistatic-radar claims where distinguishable.
5. **Advisory, uncertainty-published outputs** — count/pose/vitals are estimates with published uncertainty, not hard claims; narrows claim coverage.
6. **Per-home, git-PR adaptation** — not a per-customer trained model; reduces any data- or model-related IP exposure.

---

## 7. Immediate next steps

1. **Create `docs/patent/`** and move/ink the drafted closure-phase application ("Wi-Fi Sensing Mesh for Three-Dimensional Scatterer Tracking Using Jennison Closure Phase Across Directed Multi-Node Packet Loops") into it; assign invention IDs (a–t) per the task; record each invention's status (drafted / filed / published / granted / trade-secret).
2. **Engage a qualified patent attorney** (wireless sensing + SEP experience) for: (a) claim-language comparison of Aquila's v1 against Aerial US12044789B2 family, nami mesh patent, and MERL US11902811B2; (b) novelty/freedom-to-operate search on closure-phase + WiFi CSI; (c) GPLv3 §11 implications of any filing.
3. **File the closure-phase provisional/PCT** before publishing validation results from the 10-device ESP32-S3 hardware validation.
4. **Defensively publish** the non-patented core pipeline (signal processing, fusion, time sync, calibration) in dated, citable form.
5. **Add a patent/IP policy file** (`docs/patent/IP-policy.md` or `PATENTS.md`) stating defensive-only stance and non-assertion for non-commercial/GPLv3-compliant use (counsel-drafted).
6. **Re-verify every patent number/status** in §5 before relying on it (Google Patents legal status is not a legal conclusion; check national registers, term adjustments, and fee payments).
7. **Monitor 802.11bf SEP disclosures** if Aquila ever adopts 11bf sensing features; WiFi SEP pools (Sisvel/Via) are the relevant licensing channel at that point.
8. **Record invention conception dates** (git commits, dated specs) for all invention areas to support prior-art and derivation arguments.

---

## 8. References

- Aquila spec: `docs/spec/` (fusion-sync.md, geometry-calibration.md, signal.md, architecture.md, inference-models.md, privacy-security.md); `CONTEXT.md`; `docs/reference/sources.json` — github.com/belikh/aquila
- IEEE Std 802.11bf-2025 (WLAN sensing), published 26 Sep 2025 — https://standards.ieee.org/ieee/802.11bf/11574
- IEEE 802.11 working group recent approvals — https://www.ieee802.org/11
- Origin Wireless patents — https://patents.justia.com/assignee/origin-wireless-inc ; https://www.originwirelessai.com
- US12044789B2 (Aerial occupancy) — https://patents.google.com/patent/US12044789B2/en
- US11902811B2 / US20220286885A1 (MERL multi-band fusion) — https://patents.google.com/patent/US20220286885A1/en ; https://patents.google.com/patent/US11902811B2/en
- CN110149604B (CSI people detection) — https://patents.google.com/patent/CN110149604B/en
- US9696421B2 (through-wall WiFi/cellular radar) — https://patents.google.com/patent/US9696421B2/en
- US10547358B2 (reciprocity calibration) — https://patents.google.com/patent/US10547358B2/en
- nami Wi-Fi Mesh Sensing — https://nami.ai/wifi-sensing
- Aerial–nami ~73 patents portfolio — https://finance.yahoo.com/news/aerial-nami-announce-strategic-cooperation-150300830.html
- Cognitive Systems patent portfolio (450+; 17 US) — https://www.cognitivesystems.com ; https://henry.law/case-study/cognitive-systems-corp
- EP4546671A1 (WLAN sensing, 2025) — https://data.epo.org/publication-server/rest/v1.2/publication-dates/2025-04-30/patents/EP4546671NWA1/document.pdf
- WO2023212082A1 (sensing in WLAN) — https://patents.google.com/patent/WO2023212082A1/en
- Sisvel Wi-Fi Multimode pool — https://www.sisvel.com/licensing-programmes/Wi-Fi/wi-fi-mm ; https://www.wifipatentpool.com/licensing-info
- Sisvel Wi-Fi 6 pool ($0.50/unit) — https://ipwatchdog.com/2024/11/05/managing-wi-fi-6-sep-licensing-litigation-challenges-10-billion-royalty-market ; https://www.prnewswire.com/news-releases/sisvel-launches-80211-wi-fi-joint-licensing-program-222937951.html
- Via Licensing Alliance — https://www.via-la.com
- William & Mary CSI survey (Ma, Zhou, Wang, ACM CSUR 2019) — https://gzhou.pages.wm.edu/wp-content/blogs.dir/5736/files/sites/13/2017/12/WiFiSenseSurvey_CSUR19.pdf
- Espressif ESP-CSI (Apache-2.0) — https://github.com/espressif/esp-csi ; https://docs.espressif.com/projects/esp-techpedia/en/latest/esp-friends/solution-introduction/esp-csi/esp-csi-solution.html
- RuView (MIT) CSI sensing — https://github.com/ruvnet/RuView ; https://ruview.blog
- Passive WiFi Sensing (P102447GB; Tan/Woodbridge/Chetty) — https://sites.google.com/view/passivesensing
- ABI Research 802.11bf roadmap — https://go.abiresearch.com/hubfs/Marketing/Whitepapers/The%20Commercial%20Market_Value_for_WiFi_Sensing/...
- Wi-Fi Alliance / WBA industry report — https://wballiance.com/wp-content/uploads/2022/10/WBA_2023-Industry-Report_Final-Oct_01-3.pdf
- GPLv3 §11 (patent license / retaliation) — https://www.gnu.org/licenses/gpl-3.0.en.html
- K. J. Ray Liu / Origin Wireless history — https://kjrayliu.org ; https://en.wikipedia.org/wiki/K._J._Ray_Liu
