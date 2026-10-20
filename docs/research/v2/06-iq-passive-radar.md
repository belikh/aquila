# 06 — I/Q passive/bistatic radar and pulse-Doppler

Author: deep-research agent 06. Australian English. Primary sources only.
Scope: the Aquila v2 three radio capabilities (esp-csi CSI; experimental raw I/Q
receive; compliant 802.11 TX incl. clustered/volumetric TX), plus the
ping-and-unique-reply round-trip scheme.

## Question

Can gapped, uncalibrated ESP32 raw I/Q support passive/bistatic radar and
pulse-Doppler detection, and what are the range/velocity limits? This note must
cover (a) pulse-Doppler processing and resolution; (b) the effect of gaps/duty on
unambiguous range/velocity; (c) passive (illuminator-of-opportunity) vs bistatic
geometry; (d) required integration gain.

## Findings

### ESTABLISHED

**Pulse-Doppler fundamentals (textbook).** Range resolution is set solely by
bandwidth, ΔR = c/2B; Doppler resolution by coherent processing time, Δv =
λ/(2·T_int) with T_int = M·T the CPI (M pulses at interval T). Maximum
unambiguous range is R_max = c/(2·PRF) = c·PRI/2; maximum unambiguous velocity is
v_max = ±λ·PRF/4. The two are rigidly coupled — R_max·v_max ≈ c·λ/8 — so raising
PRF buys velocity at the cost of range and vice versa; low-PRF gives unambiguous
range but folded velocity, high-PRF the reverse, and medium-PRF is ambiguous in
both and must be resolved by multiple staggered PRFs. PRF also sets *blind*
ranges (eclipsing) and blind velocities. Coherent integration of N pulses adds
20·log10(N)/2 = 10·log10(N) dB to SNR (2 pulses → 3 dB, 10 → 10 dB, 20 → 13 dB);
non-coherent (post-detector) integration gains less than N, is a function of
P_d/P_fa, and loses phase (hence Doppler sign). (MathWorks PHASED; LL-MIT
"MTI and Pulse Doppler Processing" lecture 5; Sciencedirect range-ambiguity
overview; "A Study of Pulse-Doppler Radar PRF".)

**OFDM is a near-ideal illuminator of opportunity.** The OFDM ambiguity function
is *thumbtack-shaped*: delay resolution 1/B from the broadband property, Doppler
resolution 1/T_int from the symbol-integration time. CP-OFDM adds two sidelobes
at ±N_ST symbol shift (power [N_CP/(N_ST+N_CP)]²) but otherwise behaves like
plain OFDM; FBMC is similar with no CP sidelobes. (Cao et al., IET RSN 2016,
doi:10.1049/iet-rsn.2015.0414; Searle & Palmer 2014; Colone et al. 2012.)

**Passive WiFi bistatic radar is demonstrated — on SDR.** Colone et al. (IEEE
TAES 48(2):1061–1079, 2012) used a WiFi router as IoO to detect and localise
moving targets and form inverse-SAR cross-range profiles, with LS adaptive
clutter cancellation, MTI and taper windows. Chetty, Smith & Woodbridge (IEEE
TGRS 50(4):1218–1226, 2012) produced the first *through-the-wall* detections of
moving personnel using passive WiFi radar at standoff distances; performance was
signal-to-**interference**-ratio limited (not SNR), and a CLEAN-based canceller
improved SIR by ≈19 dB. Both used SDR receive hardware, not COTS Wi-Fi chips.

**Range resolution, not Doppler, is the historical weak point.** Early 802.11
(20/40 MHz) gives only ≈7.5–15 m range resolution — "almost useless" indoors — so
passive WiFi radar is *Doppler-primary*, with Doppler resolution tunable through
integration time. Only with 802.11ax/160 MHz does range resolution approach
≈1 m. (arXiv:2201.03005 "Using Wi-Fi Signal as Sensing Medium"; IET RSN 2021
"Super resolution passive radars based on 802.11ax".)

**COTS Wi-Fi monostatic range-Doppler is now demonstrated.** Sanson et al.
(arXiv:2508.02799, 2025) extract simultaneous range *and* Doppler from
commercial CSI on an Intel Wi-Fi AX211 NIC, at 160 MHz: range resolution 0.94 m,
velocity resolution 0.03 m/s (0.025 s frame interval), median errors 0.05 m
(range) and 0.03 m/s (velocity), max unambiguous range 480 m, max velocity
±0.5 m/s. Two problems are named and solved: hardware asynchronisation (time-
offset cancellation + phase-alignment correction) and Tx/Rx antenna coupling
that makes the *first range cell* (≤0.93 m) unusable without self-interference
cancellation. This is a commercial NIC, **not** ESP32.

**OTA self-referencing realises phase-coherent bistatic radar on unmodified
COTS Wi-Fi.** LoSRef (arXiv:2602.05344, 2026) exploits the Tx–Rx line-of-sight
path — the earliest, temporally stable arrival — as both a delay reference and a
phase reference, then suppresses it as clutter. This recovers a phase-coherent
time-variant CIR from a *single-antenna, single-port* receiver, preserves the
sign of the Doppler frequency (approaching vs receding), and is demonstrated for
human gait and respiration at 160 MHz with ~3.9 Hz Doppler resolution. This is
the over-the-air escape arm of the boundary problem, realised without wired
reference or dedicated reference antennas. It is COTS Wi-Fi (not stated as ESP32).

**ESP32 raw I/Q (esp-sdr) — what the hardware actually delivers.** ESPARGOS
found an *undocumented* modem debug path in ESP32-family silicon that bypasses
the fixed Wi-Fi modem and dumps raw baseband I/Q into SRAM. Supported on the
original ESP32, C3, C5, C6, C61, S2, **S3** and S31. Stated capability:
10-bit signed I and Q per 32-bit word (plus RX-gain/AGC bits); tuning 2.2–2.7 GHz
(5 GHz only on C5); analog RX bandwidth ≈13–54 MHz; sample rate up to 80 MSa/s;
gain and power **uncalibrated**; captures are **gapped**; receive-only. The
binding limit is *egress*: the modem can write 2560 Mbit/s into SRAM but UART
manages ~3 Mbit/s, SPI ~13.3 Mbit/s, and only the ESP32-S31 offers USB-HS
480 Mbit/s / GbE 1000 Mbit/s — so "our firmware streams at a very low duty
cycle", and packets between capture windows are missed. esp-sdr explicitly lists
"pulse compression radar" as an intended use and notes on-chip preprocessing can
offload the throughput bottleneck — but **no radar demonstration is claimed**.
(espargos.net/espsdr; espargos.net/news/iq-sampling-and-sdr-mode; ESPARGOS One
8-channel phase-coherent I/Q.)

**ESPARGOS phase-coherent CSI already embodies the shared/conducted-reference
escape.** Eight antenna-coupled microcontrollers (ESP32-S2 in the 2024 datasets)
share one crystal oscillator and receive a distributed *phase reference signal*
(≈2.4–2.5 GHz WiFi frames over a resistive-divider/microstrip network, chosen
against a per-board RF switch) so PLL-induced phase offsets are calibrated out;
multiple boards are combined over a coax carrying FMUX'd 40 MHz clock + phase
reference. The authors state plainly that ESPARGOS CSI is "considerably more
noisy and may exhibit additional impairments" than SDR channel sounders.
(arXiv:2502.09405; arXiv:2408.16377.)

### HYPOTHESIS

**(H1) Gapped I/Q degrades the ambiguity function in a specific, bounded way.**
If the capture gaps are quasi-periodic and their pattern is *known*, the
effective PRF is the *slow* aggregate and integration gain is scaled by the duty
cycle; if the gaps are irregular/unknown, the non-uniform sampling raises
range–Doppler sidelobes and can alias Doppler, eroding the thumbtack. This is
standard non-uniform-sampling theory applied to radar, but I found no primary
source that measures the effect on esp-sdr's specific gap pattern — it must be
characterised on hardware.

**(H2) ESP32-S3 can do passive bistatic detection *if* a reference is provided.**
A passive receiver needs no TX, and an I/Q front end spanning ≈13–54 MHz captures
the whole illuminator (a nearby AP, or a dedicated Aquila TX node). Range
resolution would be genuinely good (B = 20–80 MHz → 1.9–7.5 m, or better on the
C5's wider front end). The unresolved blocker is phase coherence across packets:
without a reference, only magnitude/Doppler-sign-blind processing survives. The
two candidate fixes are exactly the published escapes — a shared/conducted clock
(as ESPARGOS uses) or OTA LoS/reciprocal referencing (LoSRef). Neither is yet
demonstrated on ESP32 raw I/Q.

**(H3) ESP32 monostatic pulse-Doppler is doubly blocked.** Besides the reference
problem, monostatic operation on a co-located node hits the same Tx/Rx coupling
that Sanson et al. had to cancel, and ESP32-family radios are not full-duplex —
so the first range cells are lost and half-duplex turnaround adds a delay dead
zone. Aquila's `esp_wifi_80211_tx` could provide a *known* waveform/PRF, which
would let pulse-Doppler use a matched filter and known priors, but the
self-interference and gapped-capture problems remain.

### SPECULATION

- A soldered-PCB polyhedron (icosahedron, 20 faces/nodes) with a **shared LO**
  is a compact volumetric passive-bistatic aperture: because the LO is shared,
  cross-node phase *is* observable, giving 3D direction-of-arrival plus
  multistatic range/Doppler from one illuminator. Physically plausible; wholly
  undemonstrated on ESP32 and not addressed by any primary source I found.
- Chip-level matched filtering of a pulse-compressed waveform (esp-sdr's own
  suggestion) could partially bypass the egress bottleneck by reducing what must
  leave the chip — but it trades away the raw I/Q that makes the mode valuable.

## ESP32-S3 feasibility

- **Pulse-Doppler detection:** HYPOTHESIS only. Not demonstrated on any
  ESP32-family part. Blocked by (i) gapped, uncalibrated I/Q, (ii) no
  demonstrated phase reference on the I/Q path, (iii) S3 has no USB-HS/GbE
  egress (that is the S31), so duty cycle is the worst case.
- **Passive/bistatic radar:** demonstrated on **SDR** (Colone, Chetty) and on
  **COTS Wi-Fi NICs** (Sanson; LoSRef). On **ESP32-S3 specifically**: not
  demonstrated. Quantitative gap vs SDR/NIC: egress throughput (SDR/NIC stream
  continuously; ESP32-S3 ≈13.3 Mbit/s SPI → very low duty) and calibration
  (NIC/SDR phase alignment solved; ESP32 I/Q uncalibrated).
- **TX needed?** Passive illumination: no TX required (uses an existing AP or
  another node). Active pulse-Doppler with known PRF: yes — `esp_wifi_80211_tx`.
  Round-trip timing/identity: yes (one node pings, others reply).
- **Round-trip ping-and-unique-reply:** measures coarse range (FTM-class
  microsecond timestamps → ~300 m per µs, i.e. metres-to-tens-of-metres, not
  cm), identity (unique reply payload), per-round channel (CSI), clock offset/skew
  (from the timing series), and *moving-only* Doppler across successive rounds.
  Breaks on: independent-LO phase unobservability (no absolute phase/delay across
  nodes), microsecond-timestamp coarseness, multipath bias, half-duplex turnaround
  uncertainty, and reply collisions unless replies are TDM'd or CDM'd with unique
  codes.

## Evidence strength

**7/10.** The *physics and signal processing* are textbook-established
(resolution, ambiguity, integration gain — 10/10) and passive/bistatic WiFi radar
is demonstrated repeatedly on SDR and, for phase-coherent bistatic delay-Doppler,
very recently on unmodified COTS Wi-Fi (LoSRef, 2026). The deduction is therefore
strong. The score is held below 8–9 solely because **no radar or pulse-Doppler
demonstration exists on ESP32-family hardware**, and the two decisive ESP32
parameters — the real duty/gap structure of esp-sdr and whether a reference can
be made coherent on the I/Q path — are unmeasured. Confidence in the negative
(ESP32-S3 alone, no reference, cannot do coherent pulse-Doppler) is higher than in
the positive.

## Primary sources

- Colone, Falcone, Bongioanni, Lombardo, "WiFi-Based Passive Bistatic Radar:
  Data Processing Schemes and Experimental Results," *IEEE TAES* 48(2):1061–1079,
  Apr 2012. https://ieeexplore.ieee.org/document/6172975
- Chetty, Smith, Woodbridge, "Through-the-Wall Sensing of Personnel Using
  Passive Bistatic WiFi Radar at Standoff Distances," *IEEE TGRS*
  50(4):1218–1226, Apr 2012. doi:10.1109/TGRS.2011.2164411
- Sanson, Shah, Pinaroc, Frascolla, "Extracting Range-Doppler Information of
  Moving Targets from Wi-Fi Channel State Information," arXiv:2508.02799, 2025.
  https://arxiv.org/abs/2508.02799
- "Wi-Fi Radar via Over-the-Air Referencing: Bridging Wi-Fi Sensing and Bistatic
  Radar" (LoSRef), arXiv:2602.05344, 2026. https://arxiv.org/html/2602.05344v3
- Cao, Wei et al., "Feasibility of multi-carrier modulation signals as new
  illuminators of opportunity… OFDM vs FBMC," *IET RSN*, 2016.
  doi:10.1049/iet-rsn.2015.0414
- Searle & Palmer, "Evaluation of the ambiguity function for passive radar with
  OFDM transmissions," IEEE Radar Conf., 2014.
- Euchner, Schneider, Gauger, ten Brink, "ESPARGOS: An Ultra Low-Cost,
  Realtime-Capable Multi-Antenna WiFi Channel Sounder," arXiv:2502.09405, 2025.
- Euchner & ten Brink, "ESPARGOS: Phase-Coherent WiFi CSI Datasets for Wireless
  Sensing Research," arXiv:2408.16377, 2024. Dataset doi:10.18419/darus-4456.
- ESPARGOS, "ESP-SDR: Raw IQ Capture with Espressif's ESP32 Chips."
  https://espargos.net/espsdr ; https://espargos.net/news/iq-sampling-and-sdr-mode
- ESPARGOS/esp-sdr (GitHub); ESPARGOS/SoapyESPSDR; ESPARGOS/pyespargos.
- MathWorks PHASED ambiguity-function documentation; LL-MIT Lincoln Laboratory
  "MTI and Pulse Doppler Processing" lecture notes (Skolnik lineage).

## Residual unknowns

1. The exact gap/duty structure of esp-sdr capture on ESP32-S3, and whether
   S3's 80 MSa/s claim is sustainable without an FPGA (the 80 MSa/s figure was
   reported on S3 via Reddit with an external FPGA).
2. Whether esp-sdr's raw path can be made phase-coherent across packets on one
   node, and across nodes, and by which escape.
3. Achievable self-interference cancellation on an ESP32 monostatic node.
4. Licence and redistribution terms for ESPARGOS datasets / pyespargos / esp-sdr
   (believed open-source; NOT verified in this pass).
5. Measured detection range and minimum detectable velocity for an ESP32 I/Q
   passive receiver at realistic duty cycle and 2.4 GHz.
6. Whether the polyhedron/shared-LO aperture's phase coherence survives at the
   required per-packet cadence.

## Chaining

- **CSI (cap 1) → enables per-frame Doppler, blind to static and to absolute
  delay.** Requires no reference; gives the fallback (magnitude/Doppler-only).
- **Raw I/Q (cap 2) → enables range/delay (via IFFT of the wideband capture) and
  arbitrary illuminators (non-WiFi IoO, pulse compression).** *Blocked without* a
  phase/delay reference. Requires calibration (uncalibrated today).
- **802.11 TX (cap 3) → enables a known waveform and PRF (active pulse-Doppler
  with matched filtering), multistatic geometry, and the round-trip scheme.**
  *Blocked without* legal/API primitives (prompt 07) and TDM (prompt 09).
- **Highest-leverage enabling capability: a common reference (shared/conducted
  clock, OTA LoS, or OTA reciprocity).** It is the single thing that converts
  caps 1 and 2 from magnitude-only to phase-coherent delay-Doppler, and it is the
  join between prompt 06 and prompts 10/11/13. LoSRef shows the OTA variant works
  on COTS Wi-Fi; ESPARGOS shows the conducted variant works on ESP32 *CSI* but
  not yet on the I/Q path.
- **Capability that unlocks a further one:** raw I/Q + reference unlocks 3D
  volumetric DoA on the polyhedron (shared LO), which in turn unlocks multistatic
  range–Doppler localisation — a chain not yet closed anywhere on ESP32.

## Existing models and data

| Resource | What it is | Applicable to Aquila? |
|---|---|---|
| Colone 2012 PBR processing (LS clutter cancel, MTI, taper, ISAR) | Algorithms + experimental scheme | Yes — algorithms hardware-agnostic; inputs are SDR reference+surveillance channels, which Aquila emulates with CSI/I/Q |
| Chetty 2012 TTW passive radar | Experimental method + CLEAN canceller | Yes — algorithm transferable; needs the reference channel Aquila lacks today |
| Sanson 2025 | COTS-NIC monostatic range-Doppler recipe (time-offset cancel, phase align, SI cancel) | Yes, directly — but validated on Intel AX211, not ESP32 |
| LoSRef 2026 | OTA LoS reference for phase-coherent bistatic delay-Doppler, single-port | Yes — the most directly transferable, and it embodies the OTA escape |
| Cao 2016 / Searle 2014 | OFDM ambiguity-function theory | Yes — theory for waveform design on `esp_wifi_80211_tx` |
| ESPARGOS datasets / pyespargos | Phase-coherent CSI + ML labels + code | Yes for CSI/ML; does **not** provide I/Q radar data |
| esp-sdr repo | Raw I/Q firmware | Yes — the enabling primitive; no radar processing included |
| KrakenSDR | COTS coherent SDR receiver for passive radar | Reference design, not a Wi-Fi chip |
| DICHASUS | SDR CSI datasets | Yes for modelling; SDR-grade, not ESP32 |

No public dataset pairs ESP32 raw I/Q with radar ground truth. That gap is the
opportunity below.

## Own-data / new technology

**Minimal decisive experiment.** Two ESP32-S3: node A transmits a known,
repetitive 802.11 sequence (via `esp_wifi_80211_tx`) or a nearby AP is used as
IoO; node B receives with esp-sdr raw I/Q. Provide a reference two ways in
separate runs: (i) a shared 40 MHz clock + phase reference wired as ESPARGOS
does; (ii) OTA-only, LoSRef-style (align on the direct path). A person walks
radially past the link. Record synchronised I/Q plus a ground-truth track (total
station or a reference radar). Compute a delay–Doppler (pulse-Doppler) map.

**Proves:** that a phase-coherent delay–Doppler peak with the correct Doppler
*sign* can be recovered from gapped ESP32 I/Q, and the achievable integration
gain at the real duty cycle.

**Falsifies:** if the gap-induced sidelobe floor swamps the target, if phase
coherence does not survive to the packet cadence, or if egress duty is so low
that the CPI cannot contain enough coherent samples — the claim fails.

**Also build the missing artifact:** a small **ESP32-S3 I/Q delay–Doppler
dataset** with radar/total-station ground truth (walking, gestures, respiration,
approaching vs receding). No such dataset exists publicly; it would let the
community study gapped-I/Q radar processing and would directly bootstrap Aquila's
own model-based and learning-based routes.

## Resolution of the round-trip question

The ping-and-unique-reply scheme is **coarse-ranging, not coherent**: it yields
identity (unique reply), per-round CSI, clock offset/skew from the timing series,
and *moving-only* Doppler, but the microsecond MAC timestamp and the independent
LOs block absolute phase, so it cannot supply the delay/phase reference that
pulse-Doppler needs. It is useful as a *scheduling and identity* fabric and as a
coarse FTM-class range, not as a radar reference. It breaks under collisions
(fixed by TDM/CDM) and multipath.
