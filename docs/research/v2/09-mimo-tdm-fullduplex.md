# 09 — MIMO multiplexing, TDM necessity, full-duplex

**Question.** Across sender/receiver clusters, where is TDM mandatory vs FDM/CDM
viable, and is single-channel full-duplex feasible on ESP32? Must contain:
(a) TDM/FDM/CDM trade-offs; (b) half-duplex self-interference and leakage limits;
(c) why TDM is required under a shared LO; (d) simultaneous-transmission
legality/co-channel interference; (e) whether a co-located **polyhedron** array
with a shared clock removes the TDM requirement for AoA.

Reference hardware: ESP32-S3. Australian English. Verdict discipline:
ESTABLISHED / HYPOTHESIS / SPECULATION. Primary sources only.

---

## Bottom line

**TDM is mandatory for Aquila's multi-transmitter sensing on ESP32-S3.** Every
escape from it (FDM, CDM, full-duplex) fails at the ESP32-S3 PHY/MAC or radio
front end:

- **FDM** fails because an ESP32 has **one** 2.4 GHz radio and **one** channel at
  a time; a receiver cannot listen on channels 1, 6 and 11 simultaneously, so
  separating senders by channel merely moves the TDM to the receiver's tuning
  loop.
- **CDM** fails because `esp_wifi_80211_tx` transmits **whole 802.11 frames**;
  the LTF/pilot structure that carries the channel estimate is fixed by the
  PHY, so an application cannot impress orthogonal Zadoff–Chu/Walsh pilots on
  the air. Orthogonal-pilot channel sounding is an LTE/5G-NR baseband feature,
  not an ESP-IDF primitive.
- **Single-channel full-duplex** fails because ESP32 is a half-duplex,
  single-radio device with no circulator, no separate TX/RX chains, and no
  exposed baseband for the digital self-interference cancellation that even
  bespoke hardware needs (~110 dB at Wi-Fi power).

A co-located **polyhedron array with a shared clock and a hub-face switch does
NOT remove TDM.** It removes a *different* problem — cross-node phase
calibration — by giving all faces one common LO/clock. Faces are then still
sampled **sequentially** through the switch (element-level TDM), and on the
uplink every transmitter must still be time-separated. What a shared-LO switch
buys is *cheap, drift-free relative phase* once you switch; what it cannot buy
is *simultaneity*, which would require ~20 parallel coherent RX chains — a
different hardware class entirely.

**TX needed?** Yes for monostatic radar, round-trip ranging, and active
illumination. Passive multi-face AoA is possible receive-only against an
external illuminator (bistatic), but still TDM in the switching sense.

**Evidence strength: 7/10.** Full-duplex requirements, Argos shared-clock
calibration, switched-array AoA, and the ESP-IDF `esp_wifi_80211_tx`/CSI
constraints are each well sourced by strong primaries. The gap is that no
public work demonstrates a **shared-LO ESP32-S3 co-located coherent array**;
the polyhedron result is a defensible extrapolation, not a measured fact.

---

## (a) TDM / FDM / CDM trade-offs

### ESTABLISHED

- **CSI is per-frame.** On ESP32, channel state information "is estimated when
  packets are received from the transmitter"; the callback fires for each
  received packet and carries the source MAC. The frequency response is derived
  from the preamble's LTF symbols (up to 3 LTFs), which are fixed by the PHY
  (ESP-IDF Wi-Fi API / Wi-Fi Driver docs). A single 802.11 receiver cannot, from
  one composite received waveform, separate the channel of *transmitter A* from
  *transmitter B* if both transmit on the same 20 MHz channel at the same time —
  they sum in the medium and collide in the LTFs.
- **Orthogonal pilots are the CDM escape in standards-based systems.** In
  LTE/5G-NR, uplink sounding reference signals from multiple UEs are made
  orthogonal by cyclic shifts of a Zadoff–Chu base sequence; the number of
  orthogonal sequences is bounded by the sequence length (MDPI *Electronics*
  2019, "Channel Sounding for Multi-User Massive MIMO in Distributed Antenna
  System Environment"; Andrews, *A Primer on Zadoff–Chu Sequences*, arXiv
  2211.05702). This requires **baseband control of the pilot symbols**, which
  `esp_wifi_80211_tx` does not expose.
- **FDM across the three non-overlapping 2.4 GHz channels is standards-legal**
  but ESP32 has **one** Wi-Fi radio and is "limited to only one channel" at a
  time (ESP-IDF API notes; RF Coexistence doc: "ESP32 has only one 2.4 GHz ISM
  band RF module… uses the time-division multiplexing method"). A single ESP32
  receiver is therefore deaf to two of the three channels at any instant.
- **802.11 is half-duplex by design** (IEEE 802.11 family is "half-duplex
  over-the-air"). CSMA/CA requires a station to sense the medium clear before
  transmitting (clear-channel assessment, preamble detect ≈ 4 dB above noise).

### HYPOTHESIS

- For Aquila's clusters, the practical ranking is:
  1. **TDM (mandatory baseline).** One sender at a time on one channel; unique
     frames; receivers log CSI per frame. Costs a time-slot per sender.
  2. **FDM (conditional).** Split senders across channels 1/6/11 *and* give each
     receiver node a radio pinned to one channel, or accept receiver retuning.
     This gives concurrency only if receiver hardware is replicated per channel
     — which multiplies nodes rather than removing TDM.
  3. **CDM (not viable on S3).** Would need orthogonal LTF/pilot injection the
     vendor PHY does not permit. Raw-I/Q modes (esp-sdr/ESPARGOS) might in
     principle expose a wider band, but they are receive-only and gapped
     (see prompts 05/06), so they cannot become a transmitter-side CDM scheme.

---

## (b) Half-duplex self-interference and leakage limits

### ESTABLISHED

- The canonical full-duplex Wi-Fi result (Bharadia, McMilin, Katti, *Full
  Duplex Radios*, SIGCOMM 2013, DOI 10.1145/2486001.2486033): at 20 dBm average
  Wi-Fi power against a ≈ −90 dBm noise floor, **110 dB of self-interference
  cancellation** is needed; broken down as **≈60 dB analog**, 80 dB non-linear
  harmonic, and 50 dB transmitter-noise suppression. The best prior designs
  reached ≈85 dB and left ≈25 dB residual SI, which is why full-duplex only
  pays off when half-duplex link SNR is high (>40–45 dB).
- Analog cancellation is limited to ≈25 dB without antenna separation because it
  needs programmable delays with ~10 ps resolution to track the SI channel;
  adding two antennas 20 cm apart adds ~30 dB of isolation. The SI channel's
  near-field coherence time forces analog retuning on the order of every
  ~100 ms (SIGCOMM 2013). Follow-on work confirms the fundamental analog SIC
  ceiling is "rather difficult to achieve more than 60 dB… in the analog domain"
  (arXiv 2603.21561), with digital SIC required for the residue.
- These numbers are for **bespoke SIC boards with circulators and dedicated
  TX/RX chains**, demonstrated on WARP and on a purpose-built 80 MHz board —
  not on commodity radios, and emphatically not on ESP32.

### HYPOTHESIS

- ESP32-S3 has **no** circulator, **no** separate in-band TX/RX front ends, and
  **no** exposed DAC/ADC baseband to run digital cancellation. Its Wi-Fi/BT/
  802.15.4 blocks already share one RF module and coexist by TDM. The realistic
  on-S3 SI isolation is whatever antenna-to-antenna coupling and packet-level
  blanking give — tens of dB of *isolation*, not the 110 dB of *cancellation*
  full-duplex requires. Therefore **single-channel in-band full-duplex is not
  feasible on ESP32-S3**; any same-radio round trip must be TDM at packet
  granularity.
- Note the ASIC-class exceptions (RF-domain SIC giving 54 dB extra suppression,
  Zhou et al., IEEE JSSC/paper 7302529; 60 GHz FD transceivers with 54–70 dB SI
  suppression) are chip-level research, not accessible to Aquila.

---

## (c) Why TDM is required under a shared LO

### ESTABLISHED

- **Independent LOs break phase coherence, and cross-node phase synchronisation
  requires measurements that break the standard TDD flow** (arXiv 2504.11411v2,
  "Breaking the TDD Flow for Over-the-Air Phase Synchronization in Distributed
  Antenna Systems"). Two APs with unlocked LOs need periodic inter-AP phase
  estimates and re-estimation whenever LOs drift.
- **A shared 10 MHz reference is not enough for phase coherence.** Two
  acquisition channels sharing only a reference clock derive their LOs
  independently; PLL noise makes their phase skew drift. Sharing the **LO
  itself** across downconverter chains gives tight phase alignment
  (Microwave Journal, "Addressing the Challenges of Testing Multichannel
  Phase-Coherent Systems", 2017-09-15). This is the precise distinction that
  matters for the polyhedron: shared *clock* ≠ shared *LO*.
- **Argos shows what sharing clocks buys.** In the 64-antenna MU-MIMO base
  station (Shepard et al., MobiCom 2012, DOI 10.1145/2348543.2348553), all
  antennas **share clocks**, so internal calibration coefficients `b_{m→1}` are
  "stable over long periods of time" — measured <2.6% mean angle deviation
  (max 6.7%) over **4 hours**. Argos still uses **TDD** and time-slots: `K`
  orthogonal uplink pilots from `K` terminals (time/frequency-separated), and
  the reference antenna "sends a pilot while other antennas listen, then each of
  those antennas transmits, in turn, while the reference antenna listens" — an
  explicitly **sequential** internal-calibration protocol. Shared clocks made
  calibration *stable*, not *simultaneous*.
- In OFDM massive MIMO the standard separation of simultaneous users is
  orthogonal pilots (CDM), but the number of orthogonal sequences is bounded by
  sequence length, and this is a baseband feature (MDPI Electronics 2019).

### HYPOTHESIS — the crux for a shared-LO array

There are two distinct things people call "TDM under a shared LO":

1. **Per-transmitter separation.** Even with a perfectly shared LO across all
   receivers, if two transmitters emit on the same channel at once, one
   receiver's LTF sees a *sum* of two channels. Without orthogonal pilots (CDM)
   or distinct channels (FDM), the only way to estimate each channel
   individually is to give each transmitter its own **time slot** — TDM. This is
   why Argos sends `K` orthogonal pilots from `K` terminals and why Aquila's
   unique-packet round-trip is TDM by construction.
2. **TX/RX separation.** If the array is monostatic (same node/array transmits
   and listens), the shared LO does not stop the transmitter from deafening its
   own receiver; you still need full-duplex SIC (infeasible) or TDM between the
   ping and its echo/reply.

So under a shared LO, TDM is still required for both separation jobs. The shared
LO removes *calibration drift*, not the need to schedule.

---

## (d) Simultaneous-transmission legality / co-channel interference

### ESTABLISHED

- 802.11 stations must perform clear-channel assessment and defer while the
  medium is busy; concurrent same-channel transmissions are collisions, and
  spatial-reuse research is precisely about tuning the CCA threshold to allow
  *some* concurrency (Managing Channel Bonding with CCA in 802.11 networks).
- `esp_wifi_80211_tx` sends raw frames but is not a license to ignore the PHY:
  len is bounded 24–1500 bytes, only non-QoS/unencrypted frame types are
  accepted, and calling it in a tight loop can wedge the driver
  (ESP-IDF docs; esp-idf issue #6369). Its side effects include confusing other
  stations/APs (e.g. forged beacons).
- The legality layer is covered fully in prompt 07; the relevant point here is
  that ACMA LIPD / FCC 15.247 permit the *transmission* of compliant frames, but
  **do not** create a protocol licence to collide. Aggregating many concurrent
  co-channel senders raises total in-band power and mutual interference, and
  hidden-node transmitters (rooms that cannot hear each other) will fire
  simultaneously — producing exactly the LTF collisions CSI cannot separate.

### SPECULATION

- A cluster could, in principle, coordinate a *deterministic* schedule over a
  side channel (pre-agreed slots) so simultaneous co-channel transmission is
  avoided by design. This is still TDM; it just makes the TDM disciplined.
  Whether ACMA duty-cycle/EIRP headroom (prompt 07) allows a high slot rate for
  a large polyhedron is an open legal-design question.

---

## (e) Does a co-located polyhedron array with a shared clock remove TDM for AoA?

### ESTABLISHED — switched arrays give AoA *via element-level TDM*

- **Switched single-channel arrays work for AoA.** "Single Packet, Single
  Channel, Switched Antenna Array for RF Localization" (ACM IMWUT 2023,
  DOI 10.1145/3596263) sequentially connects antennas through a high-speed RF
  mux to a **single 2 MHz receiver** and uses **sub-packet switching** to obtain
  AoA from individual packets, without mux/receiver synchronisation — mean AoA
  error **3.4°**, 2D localisation **36.4 cm**, even on moving targets.
- **Pseudo-coherent TDM with a reference element.** "Direction Finding for SDRs
  with Switched Uniform Circular Arrays" (arXiv 2502.08592) uses a two-channel
  SDR and an 8+1 UCA: one port on a **central reference antenna**, the other
  switched across eight faces, recovering coherence from the reference. Switch
  phase error ≈ ±3° at 2.4 GHz; USRP B210, 1 MSa/s.
- **TDM is in fact used in classical digital beamforming** — "It is necessary
  for DOA estimation to sample signals received by array antenna[s]" via a TDM
  receiver (Maruyama et al., ISAP 2002).

### HYPOTHESIS — the polyhedron verdict

A soldered-PCB **icosahedron (20 triangular faces = 20 nodes)** with a **shared
clock** and a **hub-face switch** to a single shared transceiver:

- **Removes:** cross-node/face *phase-calibration* drift. With one LO and one
  receiver behind a switch, all faces share an inherently common phase
  reference; there is no independent-LO gauge to calibrate (contrast the COTS
  USRP array problem, Chen et al., MILCOM 2012, where each modular SDR has an
  independent LO and needs a reference transmitter / PDOA trick). A reference
  face (as in the 8+1 UCA) or known switch phase then makes relative phase
  recoverable.
- **Does NOT remove:** TDM. The hub-face switch samples faces **one at a time**;
  that *is* element-level TDM. On the uplink, each **transmitter** must still be
  time-separated (per-transmitter LTF separation, above). And per-face switching
  introduces its own phase jitter and transient settling (the UCA work discards
  a 4 kSa margin per switch event), which bounds how fast you can cycle faces
  relative to channel coherence.
- **Would remove face-TDM only if** every face had its **own coherent receiver
  chain** fed from **one shared LO**, so all 20 snapshots are captured
  simultaneously. That is not "a switch"; it is 20 parallel RF chains and a
  clock/LO distribution tree — a distinct, much harder hardware class. Whether a
  soldered polyhedron can hold 20 chains phase-matched to a fraction of a
  wavelength is unproven.

So the strongest correct statement is: **a shared-LO hub-switched polyhedron
makes phase-coherent 3D AoA *achievable and drift-free*, via sequential face
sampling plus a reference, but it does not eliminate TDM — it relocates TDM from
the network layer to the antenna switch, and it does not enable simultaneous
multi-transmitter channel estimation without orthogonal signalling ESP32 lacks.**

---

## RX / TX needed? and ESP32-S3 feasibility

- **TDM sensing:** fully feasible today on ESP32-S3 (`esp_wifi_80211_tx` +
  CSI RX callback; one sender per slot). **TX needed: yes.**
- **Full-duplex:** **not feasible** on ESP32-S3 (no SIC, single radio).
  Round trips must be half-duplex at packet granularity. **TX needed: yes.**
- **FDM:** only by pinning each receiver to a different channel = more nodes;
  a single ESP32 cannot monitor multiple channels. Partially feasible.
- **CDM:** **not feasible** via `esp_wifi_80211_tx` (no pilot injection).
- **Shared-LO hub-switched polyhedron AoA:** feasible in principle, and
  demonstrated on **SDR-grade** hardware (USRP), not yet on ESP32-S3. The gap is
  hardware: a shared LO/clock distribution tree, a low-phase-error RF switch,
  and a reference face — plus, on S3, sub-frame switching is limited because CSI
  is per-frame and the raw-I/Q path is receive-only and gapped.
- **Quantified gap:** full-duplex → ~110 dB cancellation of bespoke hardware vs
  ~0 dB exposed on ESP32; simultaneous coherent array → N parallel SDR chains
  vs one S3 radio behind a switch.

---

## The round-trip ping-and-unique-reply scheme

- **What it measures.** One node pings; every other node replies with a
  **unique** packet; round trips are timed. This is **TDM by construction** and
  therefore *exactly* the scheme ESP32-S3 can execute without full-duplex.
  - **Offset/clock alignment:** round-trip timing with the microsecond RX
    timestamp estimates relative clock offset (ping/reply asymmetry is the
    classic two-way time-transfer error).
  - **Range:** `RTT·c/2`, resolution bounded by µs timestamp and driver/PHY
    processing latency, not by the RF clock.
  - **Identity:** unique packet / source MAC identifies the responder (ESP-IDF
    CSI callback exposes `mac[6]`).
  - **Doppler:** temporal CSI ratios across successive replies give
    *moving-only* Doppler (blind to static scatter, per shared assumptions).
  - **Channel:** each reply's CSI gives that node's channel — but only because
    replies are time-separated.
- **Where it breaks.** (i) Replies that overlap in time collide in the LTFs (no
  orthogonal codes) → garbage CSI; (ii) under motion the channel changes between
  ping and reply, corrupting the common-phase assumption and Doppler; (iii) any
  attempt to make it *simultaneous* for speed reintroduces the LTF-collision
  problem; (iv) range accuracy is dominated by turnaround jitter, so offset and
  range are coupled; (v) it inherits TDM's slot-rate ceiling, which competes
  with channel coherence time.

---

## Strategic framing

### (A) Chaining

- **X enables Y:** Shared LO/clock + hub-face switch **enables** drift-free
  relative phase across polyhedron faces **→** single-shot-ish **3D AoA**
  (sequential faces + reference) **→** spatial filtering / multipath
  separation **→** cleaner CSI for channel charting (prompt 03) and
  through-wall work (prompt 04).
- **Y blocked without Z:** *Simultaneous* N-transmitter channel estimation is
  **blocked** without orthogonal pilots (CDM) or multi-channel receivers (FDM) —
  neither available on ESP32-S3 — so it collapses back to **TDM**.
- **Highest-leverage enabling capability:** a **disciplined TDM scheduler over a
  shared channel** (unique-packet round trip). It unlocks ranging, offset,
  identity, channel and (moving) Doppler from the *same* primitive, and it is the
  only multiplexing scheme the S3 PHY actually supports. The shared-LO
  polyhedron is the second-leverage capability: it unlocks coherent AoA, which
  TDM alone cannot give across independent-LO nodes.
- **Capability that unlocks a further one:** coherent shared-LO AoA unlocks
  distributed/MIMO beamforming and reciprocal self-calibration (prompts 08, 10).

### (B) Existing models/data/implementations

- **Argos** (MobiCom 2012, DOI 10.1145/2348543.2348553): shared-clock,
  internal-calibration, TDD many-antenna base station — architecture template,
  not directly reusable on S3 (WARP, 20/40 MHz, `AD9523` clock distribution).
- **esp-csi** (Espressif, github.com/espressif/esp-csi) and the ESP-CSI solution
  docs: the S3-side CSI capture + an ESP32-C3 companion transmitter
  ("self-transmission and reception mode"; "single-transmission dual-reception
  mode" using esp-crab) — directly usable, exactly Aquila's transmit/receive
  primitive set.
- **Switched-array AoA:** ACM IMWUT 2023 (DOI 10.1145/3596263); arXiv 2502.08592
  (8+1 UCA, reference antenna, MUSIC, ±3° switch error) — algorithms and
  calibration recipes transferable; hardware is SDR.
- **Full-duplex:** SIGCOMM 2013 (DOI 10.1145/2486001.2486033) and the SIC
  surveys — define the bar Aquila cannot meet on S3.
- **Channel sounding/CDM:** MDPI *Electronics* 2019 (Zadoff–Chu multi-user SRS);
  Andrews arXiv 2211.05702 — the standard CDM route, closed to S3.
- **Round-trip/two-way:** two-way time transfer / IEEE 1588 background applies
  (prompt 11).

### (C) Own-data / new technology

- **Not currently demonstrated:** a **shared-LO, hub-switched ESP32-S3
  polyhedron array** achieving coherent 3D AoA with sequential face sampling and
  a reference element. This would be new on ESP32-class hardware.
- **Minimal experiment.** Build a 3–4 face switched array on **one** ESP32-S3
  transceiver (single LO) with one face as reference; have a second ESP32-S3
  transmit known frames from known angles; measure closed-loop AoA error as a
  function of face-switch rate and settling margin, comparing (i) per-frame face
  selection against (ii) sub-frame/raw-I/Q switching. Also run the
  ping-and-unique-reply TDM scheme with N≥3 responders and measure CSI
  orthogonality loss vs reply spacing.
- **Falsifies it if:** per-face switching phase jitter (or per-frame CSI
  granularity) exceeds the phase resolution needed for the target angular
  accuracy — e.g. if measured AoA error is dominated by switch/LO jitter rather
  than by path geometry, the shared-LO-switch approach buys no accuracy over
  plain RSS. A second falsifier: replies that cannot be spaced within channel
  coherence time without LTF collisions, which would cap TDM slot rate below the
  motion rate (Doppler).

---

## Residual unknowns

1. Measured phase noise of a cheap RF switch + single ESP32-S3 LO across 20
   faces at 2.4 GHz — unquantified.
2. Whether ESP32-S3 raw-I/Q (esp-sdr/ESPARGOS) can be used for sub-frame face
   switching at all (receive-only, gapped, uncalibrated) — unresolved.
3. Actual CCA behaviour of `esp_wifi_80211_tx` in promiscuous mode (does it
   defer?), and the real collision rate for concurrent hidden-node senders —
   undocumented.
4. Whether a soldered PCB can distribute one LO/clock to 20 chains with
   sub-wavelength phase match — mechanical/RF-design unknown.
5. ACMA/FCC duty-cycle headroom for a high-rate TDM slot schedule across a large
   polyhedron — legal-design (prompt 07).
6. Number of usable orthogonal CSI snapshots per channel-coherence interval on
   S3 — needs measurement.

---

## Primary sources

1. Bharadia, McMilin, Katti. "Full Duplex Radios." SIGCOMM 2013.
   DOI 10.1145/2486001.2486033. (110 dB cancellation; 60 dB analog; 25 dB
   residual of prior art.)
2. Shepard, Yu, Anand, Li, Marzetta, Yang, Zhong. "Argos: Practical
   Many-Antenna Base Stations." MobiCom 2012. DOI 10.1145/2348543.2348553.
   (Shared clocks, stable internal calibration <2.6%/4 h, sequential TDD
   calibration.)
3. "Breaking the TDD Flow for Over-the-Air Phase Synchronization in Distributed
   Antenna Systems." arXiv 2504.11411v2. (Independent-LO phase sync breaks TDD.)
4. "Direction Finding for SDRs with Switched Uniform Circular Arrays."
   arXiv 2502.08592. (8+1 UCA, reference antenna, pseudo-coherent TDM, ±3°.)
5. "Single Packet, Single Channel, Switched Antenna Array for RF Localization."
   Proc. ACM IMWUT 2023. DOI 10.1145/3596263. (Sub-packet switching, 3.4° AoA.)
6. Chen, Lin, Kung, Lin, Gwon. "Determining RF AoA Using COTS Antenna Arrays."
   MILCOM 2012. (Independent-LO COTS array; PDOA/reference-transmitter fix.)
7. "Addressing the Challenges of Testing Multichannel Phase-Coherent Systems."
   Microwave Journal, 2017-09-15. (Shared clock ≠ shared LO.)
8. Zhou et al. "Integrated Wideband Self-Interference Cancellation in the RF
   Domain for FDD and Full-Duplex Wireless." 2015. IEEE 7302529. (RF SIC 54 dB.)
9. "Digital Self-Interference Cancellation in Full-Duplex Radios: A Fundamental
   Limit Perspective." arXiv 2603.21561. (Analog SIC ceiling ~60 dB.)
10. "Channel Sounding for Multi-User Massive MIMO in Distributed Antenna System
    Environment." MDPI Electronics 2019, 8(1):36. (Zadoff–Chu orthogonal SRS.)
11. Andrews. "A Primer on Zadoff–Chu Sequences." arXiv 2211.05702.
12. Espressif ESP-IDF: esp_wifi CSI docs; `esp_wifi_80211_tx` API; Wi-Fi Driver
    (80211 packet send); RF Coexistence (`coexist.html`) — one RF module, TDM.
    https://docs.espressif.com/projects/esp-idf/
13. Espressif ESP-CSI Solution / esp-csi (github.com/espressif/esp-csi):
    self-transmission-and-reception; single-transmission dual-reception.
14. Maruyama et al. "Digital Beamforming Equipment by Time Division Multiplex."
    ISAP 2002. (TDM sampling for DOA.)
