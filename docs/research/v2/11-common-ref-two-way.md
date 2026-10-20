# 11 — Common-transmitter reference and two-way transfer

Prompt: `11-common-ref-two-way`. Australian English. Primary sources only.
Status key: **ESTABLISHED** (demonstrated, multiple independent primaries),
**HYPOTHESIS** (supported but not yet demonstrated in Aquila's regime),
**SPECULATION** (plausible, unverified).

---

## Question

How do a common-transmitter reference and two-way time/frequency/phase transfer
recover a shared reference across independent LOs? Specifically: (a) the
common-TX principle and geometry; (b) two-way time transfer and phase transfer;
(c) required SNR/precision; (d) whether a single anchor suffices; (e) the
**ping-and-unique-reply** round-trip scheme — what it measures and where it
breaks.

Context (assumed, not re-litigated): independent-LO, receive-only, no-common-
reference cross-node phase is unobservable; the three published escapes are a
common-transmitter reference, OTA reciprocity (`Ĥ_AB·Ĥ_BA⁻¹`), and a
shared/conducted reference; none of the wireless-only escapes is yet proven on
ESP32-S3; FTM is on S3 but coarse; the MAC exposes a microsecond RX timestamp.

---

## Findings

### (a) Common-transmitter principle and geometry — ESTABLISHED

- **Principle.** Every otherwise-independent receiver that listens to the
  *same* transmitted signal shares that signal's carrier phase at the point of
  departure. The *difference* between what two receivers measure therefore
  contains a common transmitted-phase term that cancels, leaving only the
  geometry-dependent path difference and each receiver's own hardware phase
  offset. Calibrate the hardware offsets once and the receivers become a
  phase-coherent array. This is the "common transmitter" (a.k.a. reference
  beacon / master-slave reference) escape from independent-LO unobservability.
- **Geometry.** Two variants:
  1. **Conducted / distributed reference** — one oscillator (and optionally one
     RF phase-reference signal) physically distributed to every node. ESPARGOS
     does exactly this: a daisy-chained **40 MHz reference clock** for frequency,
     plus a **WiFi-based phase-reference signal** delivered over a resistive
     divider + microstrip network and switched in via an RF switch, to remove
     the per-PLL/analog phase offset that the shared clock alone cannot
     (`arxiv.org/html/2502.09405v1`, §II-A, §II-B). It states plainly that
     "reference clock distribution is insufficient for achieving phase
     coherence" and that the PLL/analog initial phase changes on every reset or
     channel change.
  2. **Over-the-air / wireless common TX** — one node broadcasts a known
     reference frame; all slave receivers measure their phase relative to it.
     This *replaces* the conductor with the air path, and therefore requires the
     anchor→receiver channels (or their differences) to be known or static.
- **Consumer-grade WiFi primaries.** `Phaser` (ACM MobiCom 2014,
  `dl.acm.org/doi/10.1145/2639108.2639139`) shows COTS 802.11 NICs can be
  calibrated/synchronised to a **20° median phase error**, "enabling inexpensive
  deployment of numerous phase-difference based spectral analysis techniques
  previously only available on costly, special-purpose hardware." Receiver-side
  phase distortions are exactly the target of common-TX calibration
  (`arxiv.org/html/2605.26836v1`, "Same Signal, Different Story", uses a single
  common transmitter + power splitter to isolate receiver-induced gain/phase
  distortion across COTS receivers).

### (b) Two-way time transfer and phase transfer — ESTABLISHED

- **General two-way time transfer (TWTT).** Two sites exchange timing signals;
  each timestamps the other's arrival against its own clock; the two readings
  are differenced. If the two paths are reciprocal, **all propagation delay
  cancels** and the clock offset is half the difference of the interval
  counters — *without* knowing the distance (`tf.nist.gov/general/pdf/836.pdf`,
  Hanson, NIST; eqs. 1–3). The classic caveat: the participants must transmit
  and receive (near-)simultaneously.
- **Two-way satellite time & frequency transfer (TWSTFT).** Operational for
  decades; **< 0.5 ns precision at all times**, **300 ps short-term noise in a
  300 s average**, calibratable to ~1 ns with a portable earth station; most of
  the propagation delay cancels by path symmetry
  (`nist.gov/.../two-way-satellite-time-and-frequency-transfer`;
  `tf.nist.gov/general/pdf/863.pdf`). Non-reciprocal residuals that do **not**
  cancel: equipment (TX/RX chain) delay differences, uplink/downlink and
  transponder delay differences, and the **Sagnac** term from Earth rotation
  (calculable from coarse position).
- **PTP / White Rabbit.** IEEE 1588-2008/2019 PTP gives sub-microsecond time
  with minimal resources and "sub-nanosecond time transfer accuracy … in a
  properly designed network"; it explicitly supports correcting **path
  asymmetry** where known (`standards.ieee.org/standard/1588-2008.html`,
  `ieeexplore.ieee.org/document/9120376`). **White Rabbit** (CERN) adds SyncE
  physical-layer syntonisation + precise phase measurement + PTP to reach
  **sub-nanosecond accuracy and picosecond jitter over fibre**
  (`indico.cern.ch/event/143656/.../1031-WHITE.pdf`). Both are *wired*
  techniques — the comparison matters, because the wireless channel, not the
  protocol, is the limit.
- **Two-way *phase* transfer (distributed arrays).** Brown & Poor's
  **time-slotted round-trip carrier synchronization** exploits the equality of
  clockwise vs counter-clockwise round-trip phase around a source loop when
  channels are reciprocal; needs `2N−1` sync slots and periodic repetition
  (`10.1109/TSP.2008.927073`; survey `wcsl.ece.ucsb.edu/.../dist_beamforming_commag.pdf`).
  **Master-slave** is the alternative: a master broadcasts a reference and
  slaves PLL-lock; the destination/receiver can serve as the implicit master.

### (c) Required SNR / precision — ESTABLISHED (SDR-grade), HYPOTHESIS (ESP32)

- **Picosecond wireless time transfer.** Mghabghab & Nanzer use a **spectrally
  sparse two-tone** two-way waveform: **2.26 ps precision at 36 dB SNR over a
  90 cm 5.8 GHz link**, approaching the CRLB (figure of merit ≈ 90), and
  explicitly compare to PTP (≈1 µs) and White Rabbit (carrier-phase refinement)
  (`arxiv.org/pdf/2206.08457`). Prior art: 11.3 ps at 31.2 dB over ~500 m LoS,
  50 MHz LFM. **This is SDR hardware, not WiFi.**
- **Precision-vs-SNR intuition.** Time-delay estimation variance scales as
  `1/(SNR·B²rms)` (CRB); more bandwidth and higher SNR directly buy precision.
  WiFi's ≤40 MHz occupied channel is far below the tens-to-hundreds of MHz and
  GHz carriers used in the ps-class work.
- **One-way timing primaries.** RBS can reach ~1 µs with low complexity
  (survey); that only supports low-rate beamforming. For phase coherent sensing,
  the *carrier* phase (not just symbol timing) must be aligned.

### (d) Does a single anchor suffice? — ESTABLISHED for a star; scales via anchor graph

- **Yes for a "star".** One common transmitter (or one master clock) is
  sufficient to give *relative* phase/frequency coherence to every node that can
  hear it, because the common transmitted phase cancels and only known/static
  path differences and calibrated hardware offsets remain. ESPARGOS and
  esp-ppb are both single-anchor designs in this sense.
- **No for a large/sparse network.** Rogalin et al. show a single master with
  high SNR to *all* APs is "clearly impossible in large networks"; they define a
  **connected graph of anchor APs** that exchange orthogonal sync pilots, with
  non-anchors doing local master-slave to a nearby anchor, all resolved by a
  central weighted least-squares (`arxiv.org/html/1310.7001v4`, §I-A, §III,
  §IV). Residual frequency error accumulates as relative phasor drift, so
  **calibration must be repeated at the same rate as synchronisation**.

### (e) The ping-and-unique-reply round-trip scheme

Scheme as posed: node A pings; **every** other node replies with a **unique**
packet; A times the round trips. Findings:

**What it can measure — ESTABLISHED (as two-way ranging/timing):**
- **Range / RTT per responder** — from round-trip time, *provided* the
  responder's processing (turnaround) delay is known/calibrated; classical
  SS-TWR formula `T̂prop = ½(Tround − Treply)` (Qorvo "mathematics of two-way
  ranging"). **DS-TWR** (3 messages) cancels relative clock drift
  (`arxiv.org/html/2204.08996v3`).
- **Clock offset / skew** per responder — from the same exchange (TWTT logic);
  RBIS/OpenWiFiSync estimate offset `θ` and skew `γ` from timestamp pairs
  (`arxiv.org/html/2511.14457v1`).
- **Identity** — the *unique* reply packet (source MAC / sequence / payload tag)
  identifies which node replied, and lets A disambiguate and de-collide replies.
  It does not by itself prove physical identity beyond the claimed MAC.
- **Channel of the reply leg** — A, acting as a **common receiver**, measures the
  CSI of each unique reply. All replies are demodulated by A's *one* LO, so
  their relative phases at A share A's receiver reference: this is the
  **common-receiver** dual of the common-TX escape. It yields A-centric
  *relative* channel phase across responders — but each responder still
  transmitted with its *own* LO, so a responder's unknown TX phase enters every
  reply and is **not** removed. Absolute or cross-responder *transmit* phase
  therefore remains unobservable from this exchange alone.
- **Doppler (moving only)** — successive pings give RTT change (coarse radial
  rate) and/or a temporal CSI-ratio at A for each reply link. Blind to static.

**Where it breaks:**
1. **Processing/turnaround delay is the dominant error.** SS-TWR error grows with
   reply time and clock offset: sub-100 ps requires reply delays below ~5 µs,
   impossible for frame-length messages (`forum.qorvo.com/...` TWR note). Any
   software/hardware lag changes round-trip latency and breaks the estimate
   (round-trip beamforming is explicitly "sensitive to software or hardware
   lags", `wcsl.ece.ucsb.edu/.../quitin_Globecom2012.pdf`).
2. **Path asymmetry.** TWTT assumes reciprocal delays; frame-length asymmetry,
   different MCS/size per reply, and differing AGC/filters break it. PTP
   "specifies how corrections for path asymmetry are made, if … known"
   (`1588-2008`) — i.e. asymmetry is a named hazard, not a first-order canceller.
3. **No common LO across transmitters.** The unique replies originate from
   independent LOs; nothing in the round trip removes their phase offset. This
   is the same wall as elsewhere — a ping/reply *protocol* is not a
   synchronisation *signal*.
4. **Timestamp granularity.** A **microsecond** MAC RX timestamp gives ~300 m
   one-way / ~150 m round-trip resolution — useless for fine ranging without
   interpolation. FTM fields are finer (`rtt` in pSec, `rtt_raw` ns, `dist_est`
   cm) but the ESP-IDF note warns "Distance measurement using RTT is not
   accurate" (`docs.espressif.com/.../esp32s3/api-guides/wifi.html`).
5. **Multipath bias & half-duplex turnaround** — indoor multipath biases RTT;
   a single radio cannot TX and RX the same channel simultaneously, so reply
   slots must be scheduled (TDM), costing duty and adding the turnaround term.
6. **Collisions/scaling** — all responders replying to one ping needs
   orthogonalisation (time/sequence), so "every other node replies" in practice
   becomes TDM or CDM; unknown responders can't be ranged.

---

## ESP32-S3 feasibility

| Element | ESP32-S3 status | Gap vs SDR-grade |
|---|---|---|
| Common-TX **conducted** reference | **HYPOTHESIS, closest to demo.** ESPARGOS (ESP32 + 40 MHz + WiFi phase ref) is a published low-cost design; esp-ppb is an **open-source ESP32-C3** implementation (GPL-3.0) claiming sub-PPB / <1° phase via FTM + VCTCXO | Wiring/distribution is a build problem, not a radio problem; S3 lacks the external-clock injection *documented* as on C3 boards — must be verified per board |
| Common-TX **wireless** reference | **HYPOTHESIS.** esp-ppb's "AP sends a reference frame 1 ms after each external CSI frame" is exactly this, and uses `esp_wifi_80211_tx`; results are **self-reported, unreviewed** | Needs the anchor→receiver geometry to be known/static |
| Two-way **time** transfer | **HYPOTHESIS.** FTM is on S3 (initiator + responder, 802.11mc); `rtt_raw`/`dist_est` exposed. OpenWiFiSync/RBIS on **ESP32-S3** reaches **±30 µs** — a time-sync result, not phase | µs time sync vs ns TWSTFT vs ps SDR: 4–7 orders of magnitude |
| Two-way **phase** transfer | **Not demonstrated.** No published ESP32 deployment of round-trip carrier sync / two-way phase transfer found | Requires carrier-phase coherent TX+RX and sub-µs turnaround control the ESP32 stack does not expose |
| Ping-and-unique-reply | **HYPOTHESIS.** Mechanically trivial with `esp_wifi_80211_tx` + unique non-QoS frames; FTM provides the timing fields | Still coarse timing, asymmetric, no cross-node phase; only a *measurement scaffold* |

`esp_wifi_80211_tx` supports **beacon / probe req / probe resp / action / non-QoS
data** (not encrypted or QoS) — enough for the reference frame and the unique
replies (`docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/wifi-driver/wifi-vendor-features.html`).

**Hardware honesty.** Everything ps/ns-class is **SDR-grade** (two-tone / LFM at
5.8 GHz, USRP-class hardware, often with a cabled 10 MHz + PPS first). Wired
sub-ns is **White Rabbit/PTP over fibre/Ethernet**. The only *demonstrated*
ESP32-family phase coherence is the **conducted ESPARGOS/esp-ppb** family and
Espressif's own **esp-crab** (ESP32-C5, single-TX/dual-RX RF phase sync). No
wireless-only escape is proven on S3. **Quantified gap:** ESP32-S3 has ≤40 MHz
occupied bandwidth, a 2.4 GHz-only WiFi chain, a µs-class exposed RX timestamp
and no exposed LO/clock discipline path, against tens–hundreds of MHz and
ps-class timing on SDR testbeds.

---

## TX needed?

**Yes — decisively.** Both escapes under study require transmission:
- Common-TX reference: one node must **transmit** the reference frame (OTA
  variant) — `esp_wifi_80211_tx`.
- Two-way transfer: both nodes must **transmit** (ping + reply); FTM already
  transmits action frames.
- The ping-and-unique-reply scheme is all TX + RX.

Even the *conducted* reference still needs a transceiver to *emit* the WiFi
phase-reference signal (ESPARGOS's generator "must simply produce valid WiFi
frames"). Receive-only I/Q mode cannot do any of this. This capability therefore
**depends on** prompt 07 (legal primitives) and 08/09 (TX geometry/scheduling).

---

## Evidence strength: **7 / 10**

**Justification.** The *theory* is very strong and long-established: common-
transmitter phase cancellation, TWTT/TWSTFT (<0.5 ns), PTP/White Rabbit
(sub-ns), and round-trip carrier synchronisation are all multiple-primary,
peer-reviewed, operationally deployed results. The *round-trip/ping-reply*
scheme's arithmetic and failure modes (processing delay, asymmetry) are textbook
and primary-sourced. The score is held below 9 because **the specific claim that
matters to Aquila — a wireless-only common reference or two-way phase transfer on
ESP32-S3 — is not demonstrated in any peer-reviewed primary**; the closest
ESP32 evidence (ESPARGOS, esp-ppb, esp-crab, OpenWiFiSync) is either conducted,
self-reported/unreviewed, on a different ESP32 variant (C3/C5), or only does
*time* synchronisation (µs), not phase. All high-precision numbers are SDR-grade.

---

## Primary sources (DOIs / URLs)

1. Hansen, D.W., "Fundamentals of two-way time transfers by satellite", 43rd
   Ann. Symp. Frequency Control, 1989 — `tf.nist.gov/general/pdf/836.pdf`.
2. NIST, "Two-Way Satellite Time and Frequency Transfer (TWSTFT)" —
   `nist.gov/pml/time-and-frequency-division/time-distribution/two-way-satellite-time-and-frequency-transfer`.
3. "Satellite Two-Way Time Transfer: Fundamentals and Recent Progress", NIST —
   `tf.nist.gov/general/pdf/863.pdf`.
4. IEEE Std 1588-2008 / 1588-2019, Precision Time Protocol —
   `standards.ieee.org/standard/1588-2008.html`;
   `ieeexplore.ieee.org/document/9120376`.
5. White Rabbit, "Sub-Nanosecond Timing over Ethernet" (CERN) —
   `indico.cern.ch/event/143656/papers/1378076/files/1031-WHITE.pdf`.
6. Rogalin, Bursalioglu, Papadopoulos, Caire et al., "Scalable Synchronization
   and Reciprocity Calibration for Distributed Multiuser MIMO", IEEE TWC 2014 —
   arXiv:1310.7001.
7. Brown III, D.R. & Poor, H.V., "Time-Slotted Round-Trip Carrier Synchronization
   for Distributed Beamforming", IEEE TSP 56(11), 2008 —
   `doi.org/10.1109/TSP.2008.927073`.
8. Mudumbai et al., "Distributed Transmit Beamforming: Challenges and Recent
   Progress", IEEE Comm. Mag. 2009 —
   `wcsl.ece.ucsb.edu/sites/default/files/publications/dist_beamforming_commag.pdf`.
9. Mghabghab & Nanzer, "Wireless Picosecond Time Synchronization for Distributed
   Antenna Arrays" — arXiv:2206.08457.
10. "Time Difference of Arrival Extraction from Two-Way Ranging" —
    arXiv:2204.08996.
11. Qorvo, "The mathematics of two-way ranging" —
    `forum.qorvo.com/uploads/short-url/toATZSWYOalHWElTUi8x4nUkw0m.pdf`.
12. Gundall, Herbst, Müller, Schotten, "Benchmarking OpenWiFiSync on ESP32:
    Towards Cost-Effective Wireless Time Synchronization" — arXiv:2511.14457.
13. ESPARGOS, "An Ultra Low-Cost, Realtime-Capable Multi-Antenna WiFi Channel
    Sounder" — arXiv:2502.09405.
14. esp-ppb (open-source ESP32-C3 phase-coherent CSI sync, GPL-3.0) —
    `github.com/jonathanmuller/esp-ppb`.
15. Espressif, esp-crab single-TX/dual-RX RF phase sync (ESP32-C5) —
    `docs.espressif.com/projects/esp-techpedia/en/latest/esp-friends/solution-introduction/esp-csi/esp-csi-solution.html`.
16. "Fine Time Measurement for the Internet of Things: A Practical Approach Using
    ESP32" — arXiv:2401.16517.
17. ESP-IDF Wi-Fi Driver (FTM, ESP32-S3) —
    `docs.espressif.com/projects/esp-idf/en/v5.5/esp32/api-guides/wifi.html`.
18. ESP-IDF `esp_wifi_80211_tx` vendor features —
    `docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/wifi-driver/wifi-vendor-features.html`;
    sample `github.com/Jeija/esp32-80211-tx`.
19. "Optimal Preprocessing of WiFi CSI for Sensing Applications", IEEE 2024 —
    `ieeexplore.ieee.org/document/10476327`.
20. Phaser (COTS phased-array calibration, 20° median) — ACM MobiCom 2014,
    `dl.acm.org/doi/10.1145/2639108.2639139`.
21. "Same Signal, Different Story: Demystifying Receiver Effects in Wi-Fi CSI",
    2026 — arXiv:2605.26836.
22. "Distributed MIMO Measurements for Integrated Communication and Sensing in an
    Industrial Environment" (Rb-clock synchronised) — PMC10935044.

---

## Residual unknowns

- Which ESP32-S3 dev boards (if any) expose an external **clock/LDO injection**
  path equivalent to the C3 boards esp-ppb relies on — board-specific, unverified.
- The **measured** wireless common-TX phase residual on stock ESP32-S3 (no
  peer-reviewed figure found; ESPARGOS uses a *conducted* reference).
- Whether FTM's `rtt` picosecond fields can be harvested on S3 for fine RTT
  independent of the "not accurate" driver estimate.
- Effect of ESP32 **AGC and per-channel PLL re-lock** on phase continuity across
  the ping/reply exchange.
- Legal/duty constraints on transmitting a continuous phase-reference signal
  (overlaps prompts 07/08) — not resolved here.
- Whether a **single OTA anchor** can calibrate receivers with unknown geometry,
  or only with known/static geometry.

---

## (A) Chaining

- **Common-TX reference ⇒ cross-node phase coherence ⇒ AoA / distributed
  virtual array / coherent combining.** Without it: "distributed beamforming is
  impossible without distributed carrier synchronization" (survey, §"Phase of the
  beamforming coefficient is essentially random").
- **Compliant 802.11 TX (`esp_wifi_80211_tx`) ⇒ both escapes.** The reference
  frame (common-TX) and the ping/reply (two-way) are *transmissions*. **Both are
  blocked without TX.** This is the highest-leverage dependency: `prompt 07
  (legal primitives) → prompt 11 (both schemes) → prompt 08/09 (geometry/TDM)
  → coherent multistatic sensing.`
- **Two-way time transfer ⇒ relative clock offset/skew ⇒ (with reciprocity,
  prompt 10) a shared reference.** But this is a *frequency/time* enabler, not a
  *phase* enabler by itself.
- **Conducted reference (ESPARGOS path) ⇒ a further capability: absolute (not
  just relative) cross-node phase**, because the reference network geometry is
  fixed and known. That, in turn, unlocks channel charting (03) with true
  phase and through-wall coherent processing (04).
- **Highest-leverage enabling capability** = a **common-transmitter reference**;
  it is the only escape that yields phase coherence without a reciprocal round
  trip, and the only one with a *published ESP32-family* realisation.

## (B) Existing models and data

- **ESPARGOS** (arXiv:2502.09405, and 2408.16377) — hardware + calibration
  method, plus public channel-charting datasets; low-cost ESP32-based
  multi-antenna sounder. Licence/code status must be checked per repo.
- **esp-ppb** (`github.com/jonathanmuller/esp-ppb`, **GPL-3.0**) — directly
  reusable ESP32-C3 firmware for wireless phase-coherent CSI (FTM + VCTCXO +
  common reference frame). GPL-3.0 is the licence to respect.
- **OpenWiFiSync** (arXiv:2511.14457) — RBIS on ESP32-S3; source-available
  reference for the two-way/RBIS protocol and its ±30 µs result.
- **esp-crab** (Espressif) — vendor RF-phase-sync solution (ESP32-C5).
- **Distributed-MIMO synchronization/calibration algorithms** (Rogalin et al.
  arXiv:1310.7001; Brown & Poor 10.1109/TSP.2008.927073) — algorithmically
  transferable, but validated on SDR/multi-antenna APs, not ESP32.
- **CSI-sanitisation models** (Phaser; "Optimal Preprocessing" IEEE 2024;
  "Same Signal, Different Story") — directly relevant to removing receiver
  phase offsets that a common-TX reference must otherwise absorb.
- **Can Aquila use them?** Yes in principle: GPL-3.0 (esp-ppb), published
  algorithms, and open Espressif components are usable subject to licence and
  the ESP32-variant/hardware-injection caveats. Datasets are re-usable for
  benchmarking but were captured on the originators' hardware.

## (C) Own-data / new technology

- **What a purpose-built Aquila experiment could unlock:** the first
  *peer-reviewable demonstration of a wireless-only common-transmitter phase
  reference on stock ESP32-S3*, and a quantified phase residual.
- **Minimal experiment (falsifiable):** three S3 nodes — one **common
  transmitter** (via `esp_wifi_80211_tx` reference frames) and two receivers at
  *fixed, measured* geometry. Simultaneously capture CSI at both receivers for
  the same reference frames. Compute the inter-receiver phase difference over
  minutes. **Claim:** it stays constant to within a stated bound after a one-time
  calibration (ESPARGOS reports exactly this kind of long-term stability on a
  *conducted* reference). **Falsifier:** drift exceeding the bound, or
  reset/channel-change phase jumps that do not reproduce — which would show the
  OTA reference is no better than no reference.
- **Second minimal dataset:** a small **ping-and-unique-reply** corpus with
  known node identities and surveyed distances, to quantify ESP32-S3 SS-TWR /
  DS-TWR error vs reply length, RN and multipath — the missing ESP32-S3
  ranging-error characterisation.
- **What would be genuinely new:** a wireless phase-transfer result on ESP32-S3
  (not SDR), and a **common-receiver** (single ping, many unique replies)
  phase-consistency result. If BOTH an OTA common-TX reference *and* a
  common-receiver exchange are shown to hold phase constant under fixed
  geometry, Aquila would have a cheap, cable-free route to a coherent array —
  something not currently demonstrated on this hardware class.

---

*End of note.*
