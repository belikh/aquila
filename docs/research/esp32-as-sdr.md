# ESP32 as an SDR — research note

Scope: evaluate the claim "the ESP32 family is a software-defined radio" and what
it would mean for Aquila (WiFi CSI sensing: presence, pose, vitals; 2.4/5 GHz).
Facts are separated from hype; unverified items are flagged.

## 1. What an SDR requires, and where ESP32 qualifies

An SDR replaces analogue modulators/demodulators/tuners with software: a tunable
RF front-end feeding a high-rate ADC (RX) / DAC (TX), with DSP on a host or CPU
(definition: RTL-SDR, "What is Software Defined Radio" — https://www.rtl-sdr.com/about-rtl-sdr/).

Standard Espressif parts are **not** general SDRs. The reverse-engineering work
documents a **SoftMAC** design: the PHY (baseband + RF, e.g. baseband
`0x3ff5d000`, RF front-end `0x3ff45000`) is fixed undocumented hardware, and the
MAC is largely in the proprietary blob
(https://zeus.ugent.be/blog/23-24/open-source-esp32-wifi-mac/). Public API does
not expose the RF ADC/DAC.

**Change (2025–26):** `ESPARGOS/esp-sdr` discovered an **undocumented debug path
that bypasses the fixed-function Wi-Fi modem**, exposing raw receiver I/Q:
https://github.com/ESPARGOS/esp-sdr. Capabilities per that README:
- RX-only ("this firmware currently implements reception only").
- Tuning *attempts* 100–6000 MHz, 1 MHz steps. Documented as software attempts,
  **not a guaranteed range**; gains/power are **uncalibrated**; captures have
  **gaps**; CLI tests auto-extend ~1.84 GHz on many parts; C5 adds 5 GHz.
- ~80 MSa/s burst on ESP32/S2/S3/C3/C6/C61/S31 (varies); S31 streams continuously
  up to 20 MSa/s (USB) / 40 MSa/s (Ethernet), **experimental**; S3 streams
  decimated 15.625–250 kSa/s. "Parts of the firmware are AI-generated."
- P4 is unsupported: **no integrated radio**. H2 captures its Bluetooth PHY.

Verdict: the ESP32 family is now a **partial, receive-only SDR** — genuinely raw
I/Q, but narrow-duty, uncalibrated, PHY-bypassing, and not the standard product.

### Variants and radios
- ESP32: WiFi 4 (802.11 b/g/n) 2.4 GHz + BT 4.2/BLE. 2×8-bit DAC.
- ESP32-S2: WiFi 4, no BT; USB-OTG; 2×8-bit DAC.
- ESP32-S3: WiFi 4 + BLE 5; no DAC.
- ESP32-C3: WiFi 4 + BLE 5 (RISC-V).
- ESP32-C6: WiFi 6 (2.4 GHz) + BLE 5 + 802.15.4.
- ESP32-H2: 802.15.4 + BLE 5, **no Wi-Fi**.
- ESP32-P4: **no radio** (needs a companion C6 etc.).
- ESP32-C5 (not in the brief): **dual-band 2.4/5 GHz WiFi 6** + BLE + 802.15.4 —
  the only ESP with a native 5 GHz path (esp-sdr uses it above 3 GHz).
- ESP32-S31 / C61: newer; S31 adds high-speed streaming.
Sources: Espressif SoC index (https://www.espressif.com/en/products/socs) and the
esp-sdr chip table. No variant has native sub-GHz.

## 2. Proven capabilities

- **Promiscuous / monitor mode:** supported (`esp_wifi_set_promiscuous`); sniffer
  mode documented (https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-guides/wifi-driver/wifi-modes.html).
- **CSI:** all series (ESP32/S2/C3/S3/C5/C6/C61) via `esp-csi`
  (https://github.com/espressif/esp-csi). Per-LLTF/HT-LTF/STBC-HT-LTF complex
  subcarriers, up to 612 bytes; ~64 subcarriers at 20 MHz HT, ~128 at 40 MHz
  (https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-guides/wifi-driver/wifi-vendor-features.html).
  This is a modem *output*, not raw IQ.
- **Raw PHY/baseband access:** via reverse-engineering only; esp32-open-mac now
  sends/receives arbitrary 802.11 frames with blobs used only for hardware init
  (https://github.com/esp32-open-mac/esp32-open-mac).
- **Raw I/Q:** esp-sdr (above).
- **DAC arbitrary waveform generation:** original ESP32/S2 only, 2×8-bit; DMA,
  I2S or cosine generator; official examples emit sine/triangle/saw/square;
  "severe distortion" above ~2 MHz, cosine recommended <200 kHz
  (https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-reference/peripherals/dac.html).
  This is baseband/audio, not RF.
- **I2S / LCD_CAM parallel capture feeding an external RF front-end:** peripherals
  exist, but **no verified published project** was found doing this for RF; treat
  as unverified.
- **FM/spur transmit hacks:** not verified in this pass — flagged.

## 3. Transmit

- `esp_wifi_80211_tx()` sends only beacon / probe request / probe response /
  action frames and **non-QoS data**; it cannot send encrypted or QoS frames
  (vendor-features link above). So Tx is constrained to compliant 802.11 shapes.
- esp-sdr is **receive-only**; its hardware diagram shows a Tx path but none is
  implemented.
- Regulatory: Wi-Fi is a class-licensed LIPD device in Australia — shared
  frequencies, power limits, no interference protection, no protection from
  interference; off-standard/arbitrary emission would not meet the standard and
  needs an apparatus licence (ACMA LIPD: https://www.acma.gov.au/licences/low-interference-potential-devices-lipd-class-licence).
  FCC Part 15 is analogous. Arbitrary-RF Tx is a legal, not just technical,
  boundary.

## 4. SDR alternatives at similar cost

| SDR | Tune | BW / rate | Bits | TX | ~Cost |
| --- | --- | --- | --- | --- | --- |
| RTL-SDR | 24–1766 MHz | 2.56 MS/s stable | 8 | No | ~US$30 |
| HackRF One | 1–6000 MHz | 20 MS/s | 8 | Yes (half-duplex) | ~US$300 |
| USRP (1/B2x0 class) | DC–6 GHz | 40–64 MS/s | 12 | Yes | ~US$700–1000+ |
| PlutoSDR | 325–3800 MHz | 61.44 MSPS, 20 MHz BW | 12 | Yes (full-duplex) | ~US$200 |
(RTL/HackRF/USRP: RTL-SDR comparison table; Pluto: https://wiki.analog.com/university/tools/pluto.)
LimeSDR ~US$300+ (not re-verified here).

**When ESP32 + front-end beats buying one:** only for *massively multi-node,
phase-coherent* sensing where per-node cost dominates — e.g. ESPARGOS One, a 4×2
dual-polarised 2.4 GHz array built from phase-synchronised ESP32s at €350
(https://espargos.net/). For a single wideband/tunable radio, a dedicated SDR wins.

## 5. Implications for Aquila

- For presence/pose/vitals, ESP-SDR's raw IQ is **not automatically better** than
  CSI. CSI already yields the OFDM complex channel response; the binding
  constraints are phase coherence, calibration and multipath — exactly where
  esp-sdr is weakest (uncalibrated gain/phase, gapped captures).
- The larger near-term win for Aquila is the **array** approach (ESPARGOS uses
  ordinary CSI across phase-synchronised ESP32s for angle-of-arrival/delay), not
  raw SDR sampling.
- Wider instantaneous bandwidth could help delay resolution for RF-SLAM /
  through-wall, but esp-sdr's tens-of-MHz analogue bandwidth is comparable to a
  40/80 MHz Wi-Fi channel — incremental, not transformative.
- Active sensing / RL: limited — no arbitrary Tx; you can still craft compliant
  802.11 frames and steer with an array.
- Anti-sensing/privacy layer: an SDR-ish receiver makes passive eavesdropping and
  attack demonstration easier; useful for red-team work.
- Hard limits/risks: uncalibrated, gapped, receive-only, 2.4 GHz (5 GHz only on
  C5-C61 family), legality of any custom Tx, and cost/complexity of coherent arrays.

## 6. Bottom line

**Partial capability with narrow use — mostly hype as a general SDR.** The ESP32
family now has a real, documented, receive-only raw-I/Q path (esp-sdr), which is
worth a small Aquila research spike alongside the phase-coherent-array direction.
It is not competitive with a US$30 RTL-SDR (RX) or US$200–300 PlutoSDR/HackRF
(wider tune, calibrated, Tx). Treat "ESP32 is an SDR" as true only with the
qualifiers: receiver-only, narrow-duty, uncalibrated, undocumented, and
across-a-subset-of-chips.
