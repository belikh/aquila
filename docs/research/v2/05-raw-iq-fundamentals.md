# 05 — Raw I/Q fundamentals (esp-sdr / ESPARGOS)

**Question.** What does raw baseband I/Q observe that CSI does not, and what do
experimental ESP32-class I/Q receivers actually deliver? Sub-questions: (a) I/Q vs
CSI information content; (b) ESPARGOS/esp-sdr architecture, bandwidth, sample rate,
duty/gaps; (c) calibration status; (d) demonstrated capabilities and limits.

Australian English. Sources are primary: the ESPARGOS project site and its upstream
GPL firmware, the two named arXiv papers, the ESPARGOS One datasheet, and one
peer-reviewed-style paper directly comparing CSI and I/Q. Claims are separated
ESTABLISHED / HYPOTHESIS / SPECULATION and hardware tier is stated throughout.

---

## 1. Findings

### 1.1 What raw I/Q is, and what CSI is (information content)

**ESTABLISHED — the two are different objects.**

- **CSI** is a *frequency-domain* per-subcarrier complex channel estimate
  `H(f_j) = |H(f_j)| e^{j∠H(f_j)}`, produced by the receiver's OFDM pipeline from
  the *preamble* training fields (L-LTF, HT-LTF, HE-LTF) via FFT and channel
  estimation. On ESP32 the Wi-Fi driver exposes CSI derived from L-LTF and
  HT-LTF (ESPARGOS hardware paper, §III). ESPARGOS One reports **L-LTF 12-bit;
  HT-LTF/HE-LTF 8-bit** I and Q per subcarrier, at **20 MHz (40 MHz only via
  802.11n channel bonding)** (ESPARGOS One datasheet).
- **Raw I/Q** is the *time-domain complex baseband* stream straight off the
  modem's ADC, bypassing the Wi-Fi modem entirely. It is not tied to any frame
  format or to the standard's pilot structure.

**ESTABLISHED — CSI is a lossy, coarse projection of the I/Q that produced it.**

The clearest primary quantification is the CSI²Q paper (arXiv 2408.06848): for a
2.4 GHz / 20 MHz Wi-Fi packet, "the preamble signal consists of **320 IQ samples**
but a CSI measurement has only **52 channel estimates** of subcarriers." CSI is
"the outcome of a series of processes, such as FFT" on the preamble IQ —
i.e. a downselection plus quantisation. That paper's whole thesis is that CSI's
"coarse granularity and information loss" degrade RF fingerprinting, and that
transforming CSI back into a time-domain feature space sharing I/Q's basis lifts
device-recognition accuracy from **67 % → 82 %** on a real dataset and **76 % →
91 %** on a synthetic one. Raw I/Q is the *more* information-rich domain; CSI is
what survives the OFDM modem.

What I/Q therefore observes that CSI does not:

- **Arbitrary, non-Wi-Fi signals in-band** — Bluetooth/BLE, Zigbee, any 2.4 GHz
  emitter, plus Wi-Fi formats CSI cannot capture (802.11b, multi-spatial-stream).
  ESPARGOS states this explicitly as a benefit of the IQ path (espargos.net/espsdr).
- **The full instantaneous band at once.** An ESP32-S3 at 80 MSa/s with 10-bit
  I/Q samples ~80 MHz of instantaneous bandwidth — "nearly the whole 2.4 GHz ISM
  band at once" (independent ESP32-S3 80 MSa/s report).
- **Fine time-domain structure** (per-sample), not per-subcarrier aggregates; the
  raw waveform supports pulse compression, matched filtering, cyclostationary
  analysis — i.e. waveforms "more suitable for specific applications, e.g. pulse
  compression radar" (ESPARGOS).
- **Element-level array data** (when the array is phase-coherent) enabling
  arbitrary array processing, not just per-frame channel ratios.

### 1.2 esp-sdr / ESPARGOS architecture (bandwidth, sample rate, duty/gaps)

**ESTABLISHED — the mechanism.** ESP32-family chips contain an **undocumented
debug path** that connects the modem's ADC/DAC to the CPU, bypassing the
fixed-function Wi-Fi modem, intended for Espressif's internal modem testing.
ESPARGOS "figured out" this path and wrote firmware to dump raw IQ into reserved
internal SRAM. The capture mechanism differs per chip: e.g. on the ESP32-C61 the
SRAM is organised into 64 KiB banks used as a two-bank ring, with the modem
writing one bank while the CPU copies the other for transfer (espargos.net/espsdr).
The path exists all the way back to the **original ESP32** (news post, Sep 2026).

**ESTABLISHED — supported silicon and formats.** esp-sdr (GPL-3.0) supports the
**original ESP32, ESP32-S2, ESP32-S3, ESP32-C2, ESP32-C3, ESP32-C5, ESP32-C6,
ESP32-C61, ESP32-H2 and ESP32-S31** (S31 streaming experimental). IQ word format
on the reference C61: bits **19–10 = I, 9–0 = Q** (signed 10-bit, −512…+511);
bits 27–20 = RX gain table index; bits 31–28 = inferred AGC FSM state. Precision
per component is configurable (signed 8-bit `CAP16`, packed 10-bit `CAP20`, or
raw 32-bit words).

**ESTABLISHED — capture parameters (revised from the datasheet-level "How it
works" page).**

| Parameter | Value | Note |
|---|---|---|
| Sample rate | up to **80 MSa/s** into SRAM | rates 80/40/20/10/8/4/16 MS/s |
| Analog RX bandwidth | ~**13–54 MHz** (chip-dependent; S3 **13–69 MHz**, C3 14–62, S2 15–60, C2 12–20, H2 4–11) | approximate noise width; not alias-free at every rate |
| Tuning | software attempts **100–6000 MHz** in 1 MHz steps; nominal **2.2–2.7 GHz** and 5 GHz (C5 only; 4.8–6.0 GHz) | extended low band ~1.84 GHz (5/6 LO mode) on some chips; C61/S31 ~2.15–2.18 GHz; C61 reaches 2.7 GHz |
| Gain | **automatic (HW AGC) or manual** (PHY table index) | |
| Transport | UART ~3 Mbit/s, SPI ~13.3 Mbit/s, S3 native USB, S31 HS-USB 480 Mbit/s / GbE 1000 Mbit/s | |

**ESTABLISHED — the duty-cycle/gap limit, and its cause.** At 80 MSa/s × 32 bit
the modem writes **2 560 Mbit/s** into SRAM; that input pipe is far faster than
any output link. ESPARGOS: "the bottleneck is getting the samples off the chip …
our firmware streams the IQ samples at a **very low duty cycle** … Packets that
arrive between capture windows may be missed." Captures therefore **have gaps**;
nominal sample rates exceed sustained serial throughput. This is the defining
limitation of the stock path.

**ESTABLISHED — S3-specific continuous mode.** The standard `esp32s3` firmware
has an **IQS** mode: gapless, *decimated* complex baseband over **native USB
Serial/JTAG only**. The second core filters and decimates a 16 MSa/s ring by
powers of two (64–1024), yielding **250 kSa/s down to 15.625 kSa/s**, 4/8/16 bits
per component. Frames carry a sample index, a gap flag (bit 0) and a dropped-frame
flag (bit 1), and a CRC32, so the host can measure gaps exactly. Gaps occur only
when the host stops reading or the worker abandons a bank. S3 also does on-chip
FFT spectra (256–2048 bins at 16/40/80 MS/s) over native USB.

**ESTABLISHED — the "SDR-grade" ceiling.** A community contributor first got
**80 MSa/s sustained** out of an ESP32-S3 by bit-banging 16 GPIOs to an external
**Artix-7 FPGA** acting as a USB3 controller (FT600), with the FPGA feeding the
ESP the 40 MHz reference. ESPARGOS credits this pre-announcement disclosure. So
*gapless full-rate* IQ on S3 as yet requires **external FPGA glue**; the stock
route is burst-or-decimated.

### 1.3 Calibration status

**ESTABLISHED — the raw path is uncalibrated.** esp-sdr's own docs: "**Gain and
power are uncalibrated.** Extended tuning does not guarantee PLL lock or
reception." Manual gain mirrors a PHY table entry and disables RF-saturation
intervention; AGC behaviour is inferred, not documented.

**ESTABLISHED — cross-antenna phase coherence is *not* a property of the chip; it
is engineered by ESPARGOS.** Independent ESP32 receivers are frequency- but not
phase-coherent: the PLL "may exhibit phase uncertainty," and the initial phase
changes after reset and after every LO retune, plus receiver-specific analog
phase shifts. ESPARGOS achieves coherence via:

1. a daisy-chained **40 MHz reference clock** (frequency synchronisation), and
2. a **2.4 GHz Wi-Fi-based phase reference** distributed by a resistive-divider +
   microstrip network to an RF switch at each antenna, with periodic calibration:
   `ĥ_CAL,m = (ĥ_OTA,m / ĥ_REF,m)·e^{−jφ_path,m}`.

Multiple boards are combined master/slave over the same frequency-multiplexed
coaxial reference (40 MHz + 2.4 GHz). Coherence is thus a **conducted/common
reference**, exactly the escape class the brief assumes; there is no evidence the
wireless-only escapes are needed or proven here.

**ESTABLISHED (announced, docs pending).** With the latest firmware, *ESPARGOS
One* "can now **phase-coherently** capture raw IQ samples" across its 8 channels,
turning it into an 8-channel SDR "with low duty cycle." ESPARGOS explicitly says
the raw-IQ capabilities "are not yet fully covered in the ESPARGOS library
documentation, demos, manual or datasheet." The ESPARGOS One firmware remains
**closed source for now**; esp-sdr (the open firmware) is a *different, single-chip*
codebase whose cross-channel coherence is not claimed.

### 1.4 Demonstrated capabilities and limits

**ESTABLISHED — demonstrated.**

- Single-chip raw IQ burst capture and on-chip spectrum, on many ESP32 variants,
  with CRC-checked captures and documented bandwidth mappings (esp-sdr validation
  docs; e.g. ESP32 validated across all 701 whole-MHz settings).
- ESP32-S3 continuous decimated IQ at 15.625–250 kSa/s over native USB, feeding
  GNU Radio/gqrx/SDR# via `esp-sdr-bridge` / SpyServer / rtl_tcp.
- ESP32-S31 experimental streaming: **8-bit I + 8-bit Q at up to 20 MSa/s over
  USB and 40 MSa/s over GbE**, e.g. a continuously-sampled LTE signal at 2.63 GHz.
- 80 MSa/s sustained S3 (external FPGA).
- ESPARGOS *CSI* system (not raw IQ) demonstrated: phase-coherent 8-antenna CSI,
  MUSIC AoA, channel charting — CT 0.96 / TW 0.96 / KS 0.20 / MAE 0.44 m / CEP
  0.42 m on `espargos-0002`. Datasets published on DaRUS.
- The undocumented path is not a backdoor/vulnerability but an undocumented modem
  capability (ESPARGOS FAQ).

**HYPOTHESIS — plausible but not yet published as a demonstrated result.**

- Raw IQ enables passive/bistatic radar, pulse-Doppler, arbitrary-signal
  localisation, and coherent array processing on ESP32-class hardware. ESPARGOS
  states pulse compression radar as a target application and localisation of
  arbitrary signals; no primary paper yet reports radar/Doppler results from
  ESP32 raw IQ specifically.
- Raw IQ phase-coherence across ESPARGOS One's 8 channels holds well enough for
  array gain — announced but uncharacterised publicly.

**SPECULATION.**

- CSI and raw IQ could be jointly exploited (CSI for per-frame channel, IQ for
  full-band arbitrary signals) for higher-resolution sounding; not demonstrated.
- Stock ESP32-S3 could reach gapless multi-MSa/s IQ without FPGA via clever DMA /
  USB scheduling; current evidence says no.

---

## 2. ESP32-S3 feasibility (hardware honesty)

- **Receive-only raw IQ: demonstrated on ESP32-S3 specifically.** esp-sdr lists
  `esp32s3` as supported (✅): raw burst **80/40/16 MS/s**, **13–69 MHz** analog
  bandwidth, manual/AGC gain, and a native-USB **continuous decimated IQ** mode
  (15.625–250 kSa/s, 4/8/16 bit). On-chip FFT spectra 256–2048 bins.
- **Full-rate sustained (≥ tens of MSa/s) on a bare S3: not demonstrated** — needs
  an external FPGA (16 GPIO → Artix-7 → FT600 USB3). 80 MSa/s is the ADC/dump
  ceiling, not the delivery ceiling.
- **Cross-node phase coherence on independent S3s: not demonstrated.** As per the
  brief's standing assumption, independent-LO phase is unobservable; the S3 raw
  path does not change this. Coherence requires the ESPARGOS-style **conducted**
  40 MHz + 2.4 GHz reference (ESPARGOS One firmware, closed), not the open esp-sdr.
- **Calibration:** gain/power uncalibrated; no factory phase calibration; outdoor
  act after reset or retune.
- **Comparison to other COTS Wi-Fi:** Intel/Atheros CSI toolkits give *processed
  CSI only*, no raw IQ. **SDR-grade** (USRP B210 etc.: 56 MHz real-time, 16-bit,
  calibrated, gapless, µs-accurate timing) is a different class — ESP32 raw IQ is
  **8–10 bit**, **gapped**, **uncalibrated**, single-digit-MHz sustained without
  FPGA. The gap to SDR-grade is large on precision and duty cycle, but the cost
  gap (~US$5 vs ~US$1000+) is the whole point.
- **Not ESP32:** the CSI²Q comparison uses PicoScenes/USRP and the WiSig dataset
  (USRP IQ) — useful evidence about I/Q-vs-CSI information content in general, but
  not an ESP32 result.

---

## 3. TX needed?

- **Passive raw-IQ reception needs no TX.** ESP-SDR is receive-only as shipped;
  it listens to whatever is in-band.
- **Active sounding / radar with a known waveform needs TX.** ESPARGOS FAQ: the
  chip *can* transmit arbitrary (and even phase-coherent) waveforms, but no
  transmitter is released "due to the potential for misuse." So Aquila's compliant
  TX route is `esp_wifi_80211_tx` (Prompt 07), and raw IQ then provides the
  receive side.
- **Cross-node coherence needs a shared reference**, whether conducted (ESPARGOS)
  or, unproven, wireless (reciprocity / common transmitter — Prompts 10/11).

---

## 4. Evidence strength

**8 / 10.**

Justification: strong, mostly primary sources — the upstream GPL firmware repo
with explicit validation docs, the official project site, the ESPARGOS One
datasheet, and two named arXiv papers, plus one independent peer-style paper on
I/Q-vs-CSI. Deductions: (i) the raw-IQ capability and ESPARGOS One coherence are
**recent (Sep 2026) project self-reports**, with docs the project itself calls
incomplete; (ii) no independent IEEE-style paper yet characterises the raw-IQ
path or its coherence/gaps; (iii) duty-cycle and ENoB numbers are not published
quantitatively. The core architectural and information-content claims are solid;
the demonstrated-capability claims are the weaker part.

---

## 5. Primary sources

1. ESPARGOS project — *ESP-SDR: Raw IQ Capture with Espressif's ESP32 Chips*.
   https://espargos.net/espsdr (accessed 2026-10-15).
2. ESPARGOS project — *IQ Sampling and SDR Mode* (news, 2026-09-28).
   https://espargos.net/news/iq-sampling-and-sdr-mode/
3. ESPARGOS — *esp-sdr* firmware repository (GPL-3.0-or-later).
   https://github.com/ESPARGOS/esp-sdr ; docs `rx-controls.md`, `iq-stream.md`,
   `spectrum.md`, `s31-streaming.md`.
4. F. Euchner, S. ten Brink — *ESPARGOS: An Ultra Low-Cost, Realtime-Capable
   Multi-Antenna WiFi Channel Sounder*, arXiv:2502.09405 (2025-02-13).
   https://doi.org/10.48550/arXiv.2502.09405
5. F. Euchner, S. ten Brink — *ESPARGOS: Phase-Coherent WiFi CSI Datasets for
   Wireless Sensing Research*, arXiv:2408.16377 (2024-08-29).
   https://doi.org/10.48550/arXiv.2408.16377
6. ESPARGOS — *ESPARGOS One datasheet* (2026-10-05).
   https://espargos.net/espargos-one/espargos-one-datasheet.pdf
7. ESPARGOS — *Phase-Coherent Multi-Array Systems*.
   https://espargos.net/docs/combined-arrays.html
8. *Improving WiFi CSI Fingerprinting with IQ Samples* (CSI²Q), arXiv:2408.06848.
   (320 preamble IQ vs 52 CSI; 67 %→82 % real, 76 %→91 % synthetic.)
9. Channel-charting code/results for ESPARGOS datasets:
   https://github.com/Jeija/ESPARGOS-WiFi-ChannelCharting
10. Independent disclosure — h0m3us3r, "Got raw I/Q streaming out of an
    ESP32-S3 at 80 MS/s" (Reddit r/esp32), cited by ESPARGOS.
    https://www.reddit.com/r/esp32/comments/1wq37xz/
11. SoapyESPSDR (receive-only SoapySDR driver, S31, GbE/GNU Radio/gqrx).
    https://github.com/ESPARGOS/SoapyESPSDR
12. DICHASUS distributed massive-MIMO channel sounder (higher-quality SDR-based
    CSI comparison). https://dichasus.inue.uni-stuttgart.de/

---

## 6. Residual unknowns

1. **Quantified raw-IQ phase-coherence** across ESPARGOS One's 8 channels
   (residual phase error, drift, calibration cadence) — announced, docs pending.
2. **Duty-cycle / gap statistics** for burst mode (mean gap length, distribution)
   — not published; S3 continuous mode's exact garble behaviour at high kSa/s.
3. **Effective number of bits (ENoB)** of the 8/10-bit capture path, and AGC
   behaviour frame-to-frame (its FSM bits are only "inferred").
4. **Single-node IQ phase stability** across retunes without the phase reference.
5. Whether any **non-Wi-Fi decoding** (802.11b, BLE) is implemented in software,
   or only the capability is present.
6. Whether the **S3 continuous decimated path** can be made phase-coherent with
   other nodes at all (it is single-chip, native-USB, no reference input exposed).
7. **Licence of ESPARGOS datasets** (DaRUS entries) for ML use — not verified here.

---

## 7. Chaining / Existing models and data / Own data

### (A) Chaining — which capabilities enable which

- **CSI (`esp-csi`) is the baseline**: per-frame, per-subcarrier, 20/40 MHz,
  works on the whole ESP32 family today. It enables single-link motion/Doppler and
  multi-antenna CSI. It is *insufficient* for arbitrary-signal observation and for
  element-level full-band processing.
- **Raw I/Q requires the chip; it enables** arbitrary-signal reception, full-band
  instantaneous capture, fine time-domain structure, and array processing. But raw
  I/Q **by itself does not escape the independent-LO phase problem** — the open
  esp-sdr path is per-chip and uncalibrated.
- **The common/conducted reference (ESPARGOS-style 40 MHz clock + 2.4 GHz phase
  reference) enables** cross-antenna phase coherence, which is what actually
  unlocks **AoA / 3D DoA / localisation** from either CSI or raw IQ. This is the
  **highest-leverage enabling capability**: without it, raw IQ is amplitude and
  spectrum only; with it, raw IQ becomes a coherent multi-channel array receiver.
- **`esp_wifi_80211_tx` (compliant TX) enables** active known-waveform sounding,
  which combined with raw-IQ RX unlocks bistatic/passive-radar-style experiments
  (Prompt 06) and deterministic channel sounding. It **also** opens the *wireless*
  coherence escapes (common-transmitter, OTA reciprocity — Prompts 10/11), which
  are the only routes that remove the conducted-cabling constraint.
- Dependency statements: *Coherent AoA requires a shared reference (X enables Y).
  Gapless full-band IQ requires FPGA glue (Y blocked without Z). Passive radar
  requires an illuminator and coherent RX (blocked without either).*
- One capability unlocking a further one: **the common reference** unlocks raw-IQ
  array coherence, which in turn unlocks 3D DoA and localisation from arbitrary
  signals.

### (B) Existing models/data

- **ESPARGOS datasets** (`espargos-0001…0005+`, DaRUS): phase-coherent MIMO CSI,
  labelled with reference positions; directly usable for channel charting and
  localisation. `pyespargos` (open) and `ESPARGOS-WiFi-ChannelCharting` (open) are
  usable; verify dataset licence before ML use. Hardware: ESP32-S2/C61 arrays.
- **DICHASUS**: higher-quality distributed massive-MIMO CSI (SDR-based); a
  quality benchmark but not ESP32.
- **CSI²Q (arXiv 2408.06848)**: method to transfer I/Q-domain learning onto CSI;
  directly relevant if Aquila has only CSI from many nodes and wants I/Q-like
  features. Hardware/input: CSI matrix + auxiliary IQ (USRP).
- **WiSig / RF-fingerprinting IQ datasets**: USRP IQ; relevant to IQ-based
  identity but not ESP32-formatted.
- **esp-sdr / SoapyESPSDR** (GPL-3.0 / open): the enabling software; Aquila can use
  and extend it (copyleft — check compatibility with Aquila's licence).
- **GNU Radio / gqrx** via `esp-sdr-bridge` / SpyServer / rtl_tcp: usable host
  DSP chain for ESP32-S3 IQ.
- None of these is a *raw-IQ* ESP32 dataset with the gapped/uncalibrated
  characteristics; that gap is Aquila's opportunity.

### (C) Own data / new technology

- **Could a purpose-built Aquila dataset unlock something not demonstrated?** Yes,
  plausibly: **the first public, characterised ESP32 raw-IQ dataset** (gapped,
  AGC-varying, with ground-truth reference), paired with a same-scene esp-csi
  capture, would let the community quantify exactly what IQ buys over CSI on
  commodity hardware — a result currently asserted (ESPARGOS) but not measured
  in the open.
- **Minimal experiment to prove it.** Two (or more) ESP32-S3 nodes and one
  compliant transmitter; capture simultaneously:
  (i) raw IQ via esp-sdr on each node, and (ii) esp-csi on the same nodes, while a
  subject moves through a scripted path. Score, on the *same* scene:
  **Doppler/velocity resolution, range resolution, detectability (ROC), and (with
  the S3 continuous mode) coherence across a shared reference where available** —
  IQ vs CSI, at matched bandwidth and duration. Include a retune test to measure
  phase reset and AGC steps.
- **What would falsify it.** Raw IQ (gapped, 8-bit, uncalibrated gain) shows **no
  measurable improvement** in Doppler/range/detectability over CSI on ESP32-S3;
  **or** coherence collapses unless identical cabling is used (i.e. the IQ
  advantage is unreachable without the conducted reference, making it
  operationally equivalent to ESPARGOS and not a general escape).
- **Round-trip scheme tie-in.** One node pings; every other node replies with a
  *unique* packet; round trips are timed on the µs RX timestamp. What it can
  measure: coarse **range/offset** (one-way × c/2, bounded by µs timing ≈ 300 m
  granularity — hence coarse), **identity** (unique MAC/sequence), and the
  **channel** from each reply's CSI/IQ. It **cannot** measure cross-node carrier
  phase or fine Doppler without a common reference; multipath and timestamp
  jitter limit ranging. Minimal probe: timestamped ping + unique replies with known
  sequence, comparing measured round-trip delay against FTM ground truth and
  against known static geometry. Falsifier: per-reply delay variance exceeds the
  information the round-trip adds over FTM alone.

---

*Note: research is advisory and out-of-band; it does not define scope and does not
override any ADR. Where a source conflicts with the brief's standing assumptions
(independent-LO phase unobservable; the three escapes; none wireless-only proven on
ESP32-S3), the brief is retained — no source contradicts it here.*
