# 15 — Node self-localisation without infrastructure

**Prompt.** What accuracy can nodes self-localise from FTM/RTT, RSSI and CSI
delay, and what must be measured? Must contain: (a) FTM/RTT precision and
multipath bias; (b) RSSI vs CSI-delay/time-of-flight; (c) AoA feasibility given
ESP32 antenna count; (d) achievable 2D accuracy and error drivers.

Reference hardware: **ESP32-S3**. Australian English. Primary sources only;
ESTABLISHED / HYPOTHESIS / SPECULATION separated.

---

## Question

Given a loose cluster of ESP32-family nodes with **no** fixed infrastructure
(no APs at surveyed positions, no shared clock), how well can the nodes determine
their **relative** geometry — a self-localised map — from round-trip timing and
CSI, and what is fundamentally required to do so?

The concrete operational scheme to evaluate is the **ping-and-unique-reply round
trip**: one node pings; every other node replies with a **unique** packet; the
round trips are timed. What can this measure (offset, range, Doppler, identity,
channel), and where does it break? (BRIEF §"round-trip scheme".)

---

## Findings

### (a) FTM/RTT precision and multipath bias

**ESTABLISHED**

- IEEE 802.11mc (rollup: IEEE 802.11-2016) defines *fine timing measurement*
  (FTM): an initiator/responder exchange of timestamped Action frames and ACKs
  from which RTT is averaged over a burst; distance = `RTT·c/2`
  (IEEE SA; Android Developers, Wi-Fi RTT; ESP-IDF Wi-Fi Driver docs).
  Commercial precision is **1–2 m** (IEEE SA press note; ACM WiNTECH
  benchmarking paper: "typically with a precision of one to two meters").
- FTM error is dominated by the **first-arrival (first-path) detection** in
  multipath, **hardware timestamp resolution**, **MAC-to-PHY timestamp delay**,
  and **per-device offset/bias**, plus device heterogeneity. Reported outcomes:
  - LOS/low-multipath: meter-level consistent; indoor NLOS: ~5 m
    (Rajendran et al., 802.11mc vs az).
  - 90% CDF ranging tolerances by bandwidth: **80 MHz ≈ 2 m, 40 MHz ≈ 4 m,
    20 MHz ≈ 8 m** (Rodríguez et al., Sensors 2022, PMC9322114, citing Wi-Fi
    RTT docs).
  - Ranging error roughly **halves when bandwidth doubles** (20/40/80/160 MHz);
    2×2 MIMO ≈ 30% better; a 1 ns error ⇒ 30 cm (WiNTECH benchmarking, Table 1).
  - Auto-calibration of per-device offsets recovers a **median error of ~2 m**
    from the "up to 20 m" uncalibrated heterogeneous-device failure
    (Kwon et al., "Is Wi-Fi 802.11mc FTM ready for prime-time localization?",
    ACM WiNTECH 2022).
- **Multipath bias is always positive** in the raw estimate: if the true first
  path is missed, a later bounce is used and the range is *longer* than truth
  (Barral et al. ESP32 FTM, §VIII).

**ESTABLISHED (ESP32 family specifically)**

- The **only COTS IoT-class chips with FTM** are the **ESP32-XX family**
  (Barral et al., arXiv 2401.16517). ESP32-S3 supports FTM as **initiator (STA)**
  and **responder (AP)** (ESP-IDF S3 docs; S3 datasheet lists "802.11mc FTM").
- ESP32 FTM is **2.4 GHz only, HT20 (20 MHz)** — no 5 GHz, no VHT80 — which
  directly caps the achievable resolution (Barral et al.; WiNTECH
  benchmarking, Config 2). ESP-IDF FTM report exposes `rtt_raw`, `rtt_est`,
  `dist_est` (cm) and per-frame t1–t4 in **picoseconds**, plus
  `esp_wifi_ftm_resp_set_offset(offset_cm)` for a fixed responder correction.
- **Measured ESP32-S2 behaviour** (Barral et al., arXiv 2401.16517):
  `dist_est` is markedly better than `rtt_raw`; error is small outdoors at
  longer range but **poor at short indoor range** — ≈1 m error at a true 0.5 m,
  2–3 m error at 1.0–1.5 m. Espressif's own guidance: "Distance measurement
  using RTT is not accurate", and results are best **between two ESP32 chips**
  (self-calibrated high-resolution) rather than to a third-party AP.
- **ESP-community replication (ESP32-S3, forum t=46773):** "poor results even
  with clear indoor line-of-sight." This is consistent with the 20 MHz HT20
  resolution limit; treat any ESP32-S3 ranging claim with caution.

### (b) RSSI vs CSI-delay / time-of-flight

**ESTABLISHED**

- **RSSI ranging** is cheap and passive but non-linear, orientation-sensitive
  and multipath-corrupted; median accuracy ~**2–4 m** (SpotFi §2, and the survey
  of IEEE 802.11mc/az/bk). On ESP32-class 2.4 GHz hardware, Bullmann et al.
  found **calibrated FTM 3.52 m vs RSSI 4.47 m** mean error — i.e. RSSI is
  *competitive with uncalibrated/short-range FTM*, and FTM only wins once
  calibrated.
- **CSI-derived ToF (fine delay):** estimating absolute time-of-flight from CSI
  phase is **blocked by hardware asynchronisation** — sampling time offset (STO)
  and sampling frequency offset (SFO) add a large common delay/phase ramp per
  packet. SpotFi succeeds precisely because it **does not use ToF for range**:
  it jointly estimates AoA and *relative* ToF and uses a 2D-MUSIC virtual array
  from 3 antennas × subcarriers, then localises by matching AoA+RSSI against
  **known AP positions** (median 40 cm, LoS; §3.2.2, §4.3).
- Narrow Wi-Fi bandwidth fundamentally limits delay resolution: the timestamping
  resolution bound equals the baseband sampling period = 1/BW (20 MHz ⇒ 50 ns ⇒
  ~15 m of raw resolution before super-resolution/curve-fitting); COTS clock
  rates as low as tens of MHz correspond to metre-scale resolution
  (IMDEA thesis; SIGCOMM'15 "Sub-Nanosecond Time of Flight" notes clocks
  "limiting resolution to tens of nanoseconds").

**ESTABLISHED (carrier-phase escape)**

- **WiDRa** (Ratnam et al., IEEE TWC 2024, arXiv 2405.12168) recovers
  **differential** range from **sum-carrier-phase** on *unsynchronised* COTS
  Wi-Fi, achieving **< 1 mm RMSE** differential range — *not* bandwidth-limited —
  but only for **range change between adjacent measurements**, requires a
  strong LoS (Rician K ≥ 7), and fails (cycle slips) above ~250 mm/s at 25 ms
  sampling. Useful for tracking a node's motion, not for a static absolute map.

### (c) AoA feasibility given ESP32 antenna count

**ESTABLISHED**

- AoA by subspace methods (MUSIC/SpotFi) needs a **phase-coherent multi-antenna
  array**; SpotFi's headline result is exactly that it works with **3 antennas**
  and *no* hardware change (SpotFi, SIGCOMM 2015; median AoA error < 5° LoS,
  < 10° NLoS). ArrayTrack-style systems need 6–8 antennas.
- **Stock ESP32-S3 boards are single-antenna.** S3 has an antenna-diversity
  switch supporting at most ~2 antennas on WROOM-DA-class modules (ESP-IDF S3
  "Multiple Antennas" note). Espressif's own AoA answer: the ESP32-C5 WROOM-1
  has **one** antenna and "you will require an **external antenna array**" for
  direction finding (ESP-IDF issue #15953).
- Phase-coherent ESP32 arrays exist only with **added hardware reference**:
  - **esp-crab / ESP-CSI common-crystal board:** 3 switched directional antennas
    on an ESP32-C3 + an ESP32-S3 reference, with a **clock buffer** to remove
    relative frequency offset (Espressif ESP-CSI techpedia).
  - **ESPARGOS** (arXiv 2502.09405): multiple ESP32 receivers fed by a
    **conducted phase-reference distribution network**; without this,
    multi-board CSI "would be unusable for many JCaS algorithms, including AoA".
- Therefore **AoA is blocked on a bare ESP32-S3** and requires either a
  switched array with a shared clock (small baseline, 2–3 elements) or a
  conducted/OTA reference (ESPARGOS-style) — i.e. it is an **owned-hardware
  problem, not a firmware setting**.

### (d) Achievable 2D accuracy and error drivers

**ESTABLISHED**

- Multilateration from ≥3 FTM ranges gives **1–2 m** typical 2D accuracy in good
  geometry (Android Developers; IEEE SA). Indoor/tomographic testbeds:
  WiFi-FTM indoor horizontal mean error ~**3.5 m** (90% ≈ 11 m) in cluttered
  space, even with dense reference points (Rodríguez et al., Sensors 2022) —
  geometry and multipath dominate.
- **Anchors are the absolute-reference cost.** From *pairwise ranges alone* the
  best a node set can recover is a **relative map** (MDS-MAP, dwMDS;
  Costa/Jain/Globerson 2006; Shang et al. MDS-MAP), unique only up to
  translation, rotation and reflection; **anchors** (or a shared reference)
  fix the gauge. Wi-Fi-RTT-aware self-calibration from pairwise ranges exists
  (Batstone et al., "Robust time-of-arrival self-calibration … Wi-Fi RTT",
  IEEE ICC 2016; Wendeberg et al. calibration-free TDOA).

**Error drivers, ranked:** (1) missing/biased first path (multipath, NLoS);
(2) bandwidth (20 MHz on ESP32 ⇒ coarse); (3) per-device hardware offset;
(4) geometry (GDOP); (5) clock drift over the burst; (6) device heterogeneity.

### The ping-and-unique-reply round-trip scheme

**ESTABLISHED / HYPOTHESIS (assessment)**

- **Identity:** ESTABLISHED and robust — each node's MAC/packet serial is unique,
  so the ping-and-reply round identifies *which* node answered, and sequential
  unique replies give a clean TDM schedule (matches BRIEF scope-3 sender
  clusters).
- **Range / offset:** HYPOTHESIS. To get range you must timestamp the *physical*
  departure/arrival, not the software round trip. The MAC `rx_ctrl->timestamp`
  is **microsecond** resolution (ESP32 forum t=35796) ⇒ ~150 m quantisation at
  best (1 µs)·c — *useless* for ToF directly. But the FTM burst machinery
  *does* expose **picosecond** t1–t4, so a **FTM-style** ping-and-reply (not a
  bare ESP-NOW ping) is the only route to sub-metre range. Bare ESP-NOW round
  trips measured at ~6 ms, reducible to ~689 µs with AMPDU disabled and fixed
  rate (forum t=9965) — that is software/airtime latency, **not** propagation
  delay, and cannot be deconvolved without per-frame hardware timestamps.
- **Doppler:** HYPOTHESIS — only via CSI phase change *between* successive
  replies; blind to static geometry (consistent with BRIEF assumption).
- **Channel:** ESTABLISHED — the reply frames carry CSI (amplitude/phase per
  subcarrier), so each round trip also samples the channel.
- **Where it breaks:** without a common-transmitter reference, OTA reciprocity,
  or a conducted/shared reference, cross-node **phase** is unobservable
  (BRIEF assumption) — so the round trip yields **coarse ranges and identity**,
  not a coherent phase field, unless FTM hardware timestamps are used and
  offsets calibrated.

---

## ESP32-S3 feasibility

| Measurement | On ESP32-S3 today | Gap |
|---|---|---|
| FTM/RTT range | Yes — S3 initiator (STA) & responder (AP), 2.4 GHz HT20/HT40; native self-calibration best ESP↔ESP | 20 MHz only ⇒ ~1–3 m short-range indoor error; needs per-node offset calibration |
| RSSI range | Yes | 2–4 m; orientation-dependent |
| CSI delay/ToF | Raw CSI available | Absolute ToF blocked by STO/SFO; needs sync or Wi-Fi carrier-phase (WiDRa-style) |
| AoA | **No on bare board** (1–2 antennas) | Requires switched array + shared clock (esp-crab) or conducted reference (ESPARGOS) |
| Differential range (motion) | Plausible with CSI sum-carrier-phase | Unproven on S3; needs LoS K≥7 |
| Relative self-localisation (MDS) | Plausible from pairwise FTM | Needs ≥3–4 nodes, good geometry, offset calibration; anchors for absolute frame |

**Honesty on hardware class:** FTM/RTT results are **demonstrated on the ESP32
family** (S2/S3) and on other COTS Wi-Fi (Intel AX200/8260, Broadcom/Pixel).
SpotFi's 40 cm and WiDRa's <1 mm are **other COTS Wi-Fi (Intel 5300 / AX-class,
multi-antenna, 5 GHz, 80–160 MHz)** — Aquila's S3 has one antenna and 20 MHz, so
the realistic S3 target is **metre-level relative geometry**, not decimetre
absolute localisation. AoA and mm-level ranging are **SDR-grade / purpose-built
(aperture + reference)** on ESP32.

---

## TX needed?

**Yes.** Every route here requires transmission:

- **FTM is inherently two-way** — the node must transmit Action frames and ACKs;
  a receive-only node cannot range itself.
- **CSI-delay / carrier-phase** needs a transmitter (self or peer) as illuminator.
- **Ping-and-unique-reply** needs each node to transmit a unique reply.
- `esp_wifi_80211_tx` raw injection is **not strictly required** for FTM (standard
  FTM frames/prompts suffice), but is required if Aquila wants bespoke
  ping-and-reply timing frames or custom CSI-sounding packets. Legal envelope is
  Prompt 07's remit; FTM exchange is ordinary compliant 802.11.

A **shared-clock / common-transmitter reference** is the enabling *hardware*
addition for phase and AoA; TX alone does not cross the phase boundary
(BRIEF assumption; Prompt 10/11).

---

## Chaining — (A)

- **FTM/RTT (capability 3, TX) enables** coarse pairwise range ⇒ **relative
  self-localisation** (MDS/multilateration). No fixed APs required.
- **CSI (capability 1) enables** differential range/Doppler *and* channel
  fingerprinting, but **absolute geometry is blocked** without time sync.
- **Doppler (CSI ratios) requires** a moving target and a transmitter; it is
  blind to static nodes (BRIEF assumption).
- **AoA is blocked** without either (i) a switched antenna array with a shared
  clock (esp-crab) or (ii) a conducted/OTA phase reference (ESPARGOS). AoA is
  the highest-leverage *unlock*: AoA + FTM ranges ⇒ 2D/3D self-localisation
  without dense anchors.
- **Highest-leverage enabling capability for self-localisation:** a **shared
  clock / common reference across nodes** (Prompts 10/11/12). It simultaneously
  unlocks AoA, coherent CSI phase, and sub-metre ranging.
- **Volumetric polyhedron (20-node icosahedron):** a co-located shared-clock
  aperture would turn the cluster into a **single distributed antenna array**,
  giving 3D DoA without an external reference — but this chains through Prompt
  08/09 (TDM necessity under a shared LO).

## Existing models and data — (B)

- **Wi-Fi RTT/FTM (802.11mc):** standardised, implemented in Android
  (`WifiRttManager`) and **ESP-IDF** (`examples/wifi/ftm`), Espressif's open
  source (Apache-2.0). Directly usable on S3.
- **SpotFi** (SIGCOMM 2015; code not officially released; method can be
  re-implemented) — expects Intel 5300 3-antenna CSI; **not** directly
  transferable to single-antenna S3 but the 2D-MUSIC/Joint-AoA-ToF method is.
- **WiDRa** (arXiv 2405.12168; IEEE TWC) — differential carrier-phase ranging;
  algorithm usable; validated on Intel, unproven on S3.
- **Self-localisation algorithms:** **MDS-MAP**, **dwMDS** (Costa et al., ACM
  TOSN 2006), **Wendeberg et al.** calibration-free TDOA, **Batstone et al.**
  Wi-Fi-RTT ToA self-calibration — all open literature, directly usable.
- **ESPARGOS** (arXiv 2502.09405, 2408.16377) and **esp-crab** — open designs
  for phase-coherent ESP32 arrays; licence/board availability to verify.
- **Datasets:** ESP32 FTM measurement set (Barral et al., IEEE DataPort
  `10.21227/2pv8-ze59`); MM-Fi / ESPARGOS datasets (Prompt 19 territory).

## Own-data / new technology — (C)

**Not currently demonstrated:** a **self-localising ESP32-S3 mesh map from
pairwise FTM alone with no surveyed anchors**, plus a quantified S3-specific
FTM error model. Espressif documents capability but not S3 2D accuracy; the
WiNTECH paper measured S2, not S3.

- **Minimal experiment:** 6–8 ESP32-S3 nodes on a known 2D grid in a single
  room, all pairs running FTM bursts (HT20), logging `rtt_raw`, `rtt_est`,
  `dist_est` and t1–t4. Fit (i) a per-device offset, (ii) a multipath-bias
  model, then run MDS + Procrustes — **without** using the surveyed positions as
  anchors — and score the recovered relative map against ground truth after
  optimal rigid alignment.
- **What it would prove:** whether S3 FTM reaches **metre-level relative
  geometry** and whether per-node self-calibration closes the residual.
- **Falsifier:** if post-alignment RMS error exceeds ~2–3 m, or varies by more
  than ~50% across rooms/repeats, then S3 FTM self-localisation is
  room-fragile and needs an added reference.
- **Second experiment (higher risk):** add the esp-crab/ESPARGOS shared clock
  and test whether AoA + FTM yields decimetre 2D — this is the path that would
  genuinely differentiate S3 from "another RSSI system".

---

## Evidence strength

**8 / 10.**

- FTM/RTT ranging theory, the 1–2 m commercial figure, bandwidth dependence and
  multipath bias are **established by a ratified standard and multiple
  independent measurement campaigns** (IEEE, Android, ACM WiNTECH, arXiv
  2401.16517, arXiv 2511.17935, survey arXiv 2509.03901).
- ESP32-S3 *specific* 2D self-localisation accuracy is **not** directly
  published (S2 measured, S3 documented only as capable), and AoA/CSI-phase on
  ESP32 is **hardware-gated** — hence 8, not 9–10.

## Primary sources

- IEEE Std 802.11-2016 (802.11mc FTM); IEEE SA "802.11az" note.
- Android Developers, *Wi-Fi location: ranging with RTT* (1–2 m multilateration).
- Espressif ESP-IDF, ESP32-S3 Wi-Fi Driver — FTM section (initiator STA /
  responder AP); `esp_wifi_ftm_report`, `esp_wifi_ftm_resp_set_offset`.
- ESP32-S3 Series Datasheet v2.2 (802.11mc FTM, hardware TSF).
- Barral, Escudero, García-Naya, *Fine Time Measurement for the IoT: A Practical
  Approach Using ESP32*, arXiv:2401.16517 (dataset DOI 10.21227/2pv8-ze59).
- Rajendran et al., *Performance comparison of 802.11mc and 802.11az Wi-Fi FTM*,
  arXiv:2511.17935 (IEEE 2026).
- Kosek-Szott et al., *Indoor Positioning with Wi-Fi Location: A Survey of
  IEEE 802.11mc/az/bk FTM Research*, arXiv:2509.03901; Computer Communications
  10.1016/j.comcom.2025.108400.
- *Benchmarking and Security Considerations of Wi-Fi FTM for Ranging in IoT
  Devices*, ACM WiNTECH/WiSec 2023, 10.1145/3576914.3587520.
- Kwon et al., *Is Wi-Fi 802.11mc FTM ready for prime-time localization?*, ACM
  WiNTECH 2022, 10.1145/3556564.3558234.
- Rodríguez et al., *WiFi FTM and UWB Characterization for Localization in
  Construction Sites*, Sensors 22(14):5373, 2022 (PMC9322114).
- Ratnam et al., *WiDRa … Carrier Phase*, IEEE TWC 2024, arXiv:2405.12168
  (10.1109/TWC.2024.3400000-class DOI: IEEE doc 10556775).
- Kotaru et al., *SpotFi: Decimeter Level Localization Using WiFi*, SIGCOMM
  2015, 10.1145/2829988.2787487.
- Shang et al., *MDS-MAP*; Costa, Patwari, Hero, *dwMDS*, ACM TOSN 2(1), 2006,
  10.1145/1138127.1138129.
- Batstone et al., *Robust time-of-arrival self-calibration … Wi-Fi RTT*,
  IEEE ICC 2016.
- Wendeberg et al., *Calibration-free TDOA self-localisation*, J. Locat. Based
  Serv. 7, 2013, 10.1080/17489725.2013.796410.
- Ratnam et al. / Espressif ESP-CSI techpedia — esp-crab common-crystal array;
  ESPARGOS arXiv:2502.09405, arXiv:2408.16377.
- ESP-IDF issue #15953 (ESP32-C5 single antenna; external array required for
  AoA); ESP32 forum t=46773 (S3 poor indoor range), t=35796 (µs timestamp
  inadequate for ToF), t=9965 (ESP-NOW RTT latency).

## Residual unknowns

- No published **ESP32-S3 2D self-localisation** accuracy with anchors removed.
- Magnitude and stability of **S3 FTM per-device offset** across units and
  thermal conditions.
- Whether **esp-crab / ESPARGOS** shared-clock arrays reach decimetre AoA+FTM on
  S3 (designs target C5 in places).
- Fourier/CRB limit of S3 **20 MHz** FTM ranging under realistic indoor
  multipath — not measured.
- Whether ESP32 **carrier-phase differential ranging** (WiDRa method) is
  implementable at all given S3 CSI phase impairments without a conducted
  reference.
- FTM **burst length/duty** and its effect on multi-node TDM scheduling
  (overlaps Prompt 09).
