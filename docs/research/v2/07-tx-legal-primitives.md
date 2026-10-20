# 07 — Legal envelope and `esp_wifi_80211_tx` primitives

**Question.** What 802.11 frames can an ESP32-S3 lawfully transmit under ACMA LIPD
and FCC Part 15, and what primitives does `esp_wifi_80211_tx` actually expose?
Required content: (a) the API — frame types, ACK/block-ACK, injection, rate and
bandwidth knobs; (b) ACMA LIPD class licence EIRP and duty at 2.4 GHz; (c) FCC
15.247 dwell and PSD; (d) disallowed frames.

**Scope note.** Reference hardware is the ESP32-S3. Two distinct questions are
often conflated and must be kept apart:

1. **What is lawful in the air?** — a *spectrum* question (EIRP, PSD, duty,
   out-of-band). The regulator constrains emissions, not frame *contents*.
2. **What will Espressif's driver let you emit?** — an *API* question. This is
   narrower than the law and is where "disallowed frames" really bites.

---

## Findings

### (a) The API: `esp_wifi_80211_tx`

**ESTABLISHED** (Espressif ESP-IDF API reference and Wi-Fi Vendor Features guide):

- Signature: `esp_err_t esp_wifi_80211_tx(wifi_interface_t ifx, const void *buffer, int len, bool en_sys_seq)`.
- Documented permitted frames: **beacon, probe request, probe response, action
  frame, and non-QoS data frame**. It **cannot send encrypted or QoS frames**.
- Buffer length: `24 <= len <= 1500` bytes.
- `en_sys_seq`: if `false`, the sequence-control field in the raw buffer is kept;
  if `true`, the driver overwrites it with the system sequence number. Once a
  Wi-Fi connection is established, `en_sys_seq` **must** be `true`, otherwise
  `ESP_ERR_INVALID_ARG`.
- Preconditions: Wi-Fi mode must be station, AP, or station/AP, and the hardware
  must be initialised by `esp_wifi_set_promiscuous(true)` and/or
  `esp_wifi_start()` returning `ESP_OK`.
- **Rate:** default data rate is **1 Mbps**; `esp_wifi_config_80211_tx_rate()`
  sets the PHY rate; `esp_wifi_set_bandwidth()` sets bandwidth (2.4 GHz: 20 or
  40 MHz). *Caveat:* the rate API is documented as "call after
  `esp_wifi_init()` and before `esp_wifi_start()`", i.e. a static configuration,
  and the internal `esp_wifi_internal_set_fix_rate()` explicitly **does not
  support frames sent via `esp_wifi_80211_tx`**. Per-packet rate selection is
  therefore not supported on current silicon (an open feature request exists for
  the C5).
- **Power:** `esp_wifi_set_max_tx_power()`, unit 0.25 dBm, range 2–20 dBm on the
  ESP32 family. (Values are also mapped into `max_tx_power` in `wifi_country_t`.)
- **Side-effects (driver-level rejection):** for data frames, ToDS/FromDS bits
  must match the direction (STA→AP: ToDS=1, FromDS=0; AP→STA: ToDS=0, FromDS=1),
  and the Power Management, More Data and Retransmission bits must be 0, else the
  frame is discarded and `ESP_ERR_INVALID_ARG` is returned.
- Any destination MAC, source MAC, BSSID or frame subtype can in principle be
  placed in the buffer — the documented restriction is enforced by a driver check
  (`ieee80211_raw_frame_sanity_check`), and can be bypassed at link time with
  `-Wl,wrap=ieee80211_raw_frame_sanity_check`. This is the basis of the
  `esp32free80211` / `esp32-80211-tx` projects and is **unsupported and
  version-fragile**.

**ACK / block-ACK.**

- **Cannot be transmitted** as control frames through `esp_wifi_80211_tx` (only
  management and non-QoS data are allowed). Control-frame injection requires the
  link-wrap bypass.
- **Can be received**: promiscuous-mode filter masks exist for ACK, Block Ack,
  Block Ack Request, RTS/CTS, PS-Poll, CF-END, Control Wrapper
  (`WIFI_PROMIS_CTRL_FILTER_MASK_ACK/BA/BAR/RTS/...`). This matters for the
  round-trip timing scheme (a µs RX timestamp is available on the received ACK).
- For action frames sent with `esp_wifi_action_tx_req()`, the request struct
  exposes a `no_ack` field — the only per-frame ACK control the public API
  surfaces.

**CRITICAL nuance — raw TX does not excite CSI by default.** **ESTABLISHED
(upstream code):** CSI in the ESP32 is estimated from the **OFDM preamble** of a
received frame. A frame sent by `esp_wifi_80211_tx` defaults to **1 Mbps (11b
DSSS/CCK)**, which carries **no OFDM preamble**, so a passive receiver extracts
**no CSI**. Espressif's own CSI sender (`esp-csi` `examples/get-started/csi_send`)
does **not** use `esp_wifi_80211_tx`: it broadcasts with **ESP-NOW** at
**HT40, MCS0-LGI (6.5 Mbps)**. The two "send" primitives are therefore different:
*ESP-NOW / connected-STA data* is the CSI-excitation primitive; *raw
`esp_wifi_80211_tx`* is the arbitrary-frame primitive.

### (b) ACMA LIPD class licence — 2.4 GHz

The relevant instrument is the **Radiocommunications (Low Interference Potential
Devices) Class Licence**. The 2015 licence was **remade as the LIPD Class Licence
2025**, in force **1 October 2025**; the 2.4 GHz digital-modulation limits are
unchanged.

**ESTABLISHED** (legislation.gov.au, Schedule 1):

- Wi-Fi at 2.4 GHz is classified as a **digital modulation transmitter**.
  - LIPD 2015 item 59: 2400–2483.5 MHz, **maximum EIRP 4 W (36 dBm)**, with the
    **radiated peak power spectral density in any 3 kHz not exceeding 25 mW per
    3 kHz**.
  - LIPD 2025 item 7 (+ clause 43): identical — **4 W EIRP**, radiated peak PSD
    **≤ 25 mW per 3 kHz**.
- **No duty-cycle limit** attaches to the 2.4 GHz digital-modulation item. (The
  duty-cycle clauses in the licence apply to telemetry/telecommand and medical
  items, not to this one.)
- **No indoor-only restriction** for the 2.4 GHz digital-modulation item (unlike
  the 5 GHz RLAN items, which require indoor use and/or DFS and CSMA).
- For reference: the general "all transmitters" item is 2400–2483.5 MHz at
  **10 mW** — the 4 W digital-modulation item is the one a Wi-Fi device uses.
- The class licence authorises **emissions**, not protocol behaviour. Frame
  subtype/MAC contents are irrelevant to it; what must comply is EIRP, PSD,
  spurious/out-of-band emission and the equipment standard (AS/NZS 4268 / EN 300
  328 style).

### (c) FCC 47 CFR 15.247 — dwell and PSD

**ESTABLISHED** (eCFR §15.247):

- Operates in 2400–2483.5 MHz under two regimes: **frequency-hopping (FHSS)**
  and **digitally modulated (DTS)**. **Wi-Fi is a DTS**, not FHSS.
- DTS requirements:
  - minimum **6 dB bandwidth ≥ 500 kHz**;
  - maximum **conducted output power 1 W (30 dBm)**;
  - **PSD ≤ 8 dBm in any 3 kHz** (conducted), measured during continuous
    transmission;
  - out-of-band attenuation ≥ 20 dB (peak) or ≥ 30 dB (RMS-average);
  - antennas ≤ 6 dBi without reduction; above 6 dBi in the 2.4 GHz band, for
    fixed point-to-point use only, conducted power reduces 1 dB per 3 dB.
- **Dwell time**: the **0.4 s average time of occupancy** rule belongs to the
  **FHSS** regime (and hybrid systems' hopping half). A **pure DTS (Wi-Fi) has no
  dwell-time requirement under §15.247** — there is no FCC duty-cycle limit for
  digital modulation in this band. The brief's "dwell" is an FHSS artefact, not a
  Wi-Fi constraint.

### (d) Disallowed frames — regulatory vs API

- **Regulatory:** neither ACMA nor FCC restricts 802.11 *frame types*; both
  constrain emissions only. So there is no spectrum-law category of "disallowed
  frame".
- **API:** `esp_wifi_80211_tx` disallows **encrypted frames, QoS data frames, and
  all control frames** (ACK, Block Ack, Block Ack Request, RTS/CTS, PS-Poll,
  CF-END). Arbitrary subtypes require the unsupported link-wrap bypass.
- **Real legal exposure is elsewhere:** spoofing another's BSSID/MAC can disrupt
  third parties; deauthentication/disassociation floods and jamming are criminal
  misuse offences (and a class licence gives *no* protection from interference).
  A device claiming class-licence operation must still meet the equipment
  standard, and its antenna gain must be accounted for in EIRP.

### (e) ETSI EN 300 328 V2.2.2 (EU comparator, lead)

**ESTABLISHED** (ETSI): non-FHSS equipment — RF output power **≤ 20 dBm e.i.r.p.**;
PSD **≤ 10 dBm/MHz e.i.r.p.**; for **non-adaptive** equipment above 10 dBm e.i.r.p.,
duty cycle / Tx-sequence (≤ 10 ms) / Tx-gap (≥ the preceding Tx-sequence, minimum
3.5 ms) and a **Medium Utilisation ≤ 10%** apply; adaptive equipment (LBT/DAA) is
exempt. Wi-Fi implements CSMA/CA, so it is treated as adaptive. EN 300 328 is
substantially **stricter than ACMA LIPD** (20 dBm vs 36 dBm EIRP; PSD in 1 MHz vs
3 kHz).

---

## ESP32-S3 feasibility

**ESTABLISHED:** `esp_wifi_80211_tx` is documented for `esp32s3` (ESP-IDF S3 Wi-Fi
guide), with the same preconditions, allowed frame set and 24–1500 byte limits as
the generic ESP32. Max conducted TX power is on the order of **+20 dBm**, and a
typical PCB antenna contributes ~0–2 dBi, so **EIRP ≈ 20–22 dBm**. That is
**14–16 dB below** the ACMA 4 W (36 dBm) ceiling and **8–10 dB below** the FCC
1 W (30 dBm) conducted limit — a 20-node polyhedron needs **no power reduction**
to comply. The binding constraint is **PSD**: ACMA's 25 mW/3 kHz (≈ 14 dBm/3 kHz)
and FCC's 8 dBm/3 kHz are both comfortably met by a standard 20 MHz OFDM channel
at ~20 dBm total.

**HYPOTHESIS:** the practical blocker for Aquila is **not** legal but technical —
excitation. If the only supported way to emit OFDM (CSI-bearing) frames is
ESP-NOW or an established connection (rate >6 Mbps), then a "ping" transmitter
must be an ESP-NOW broadcaster or a connected STA/AP, and the raw-injection path
is relegated to non-CSI control/beacon traffic. If the link-wrap bypass is used
to raise the raw rate to OFDM, it is unsupported and firmware-version-fragile.

**SPECULATION:** whether `esp_wifi_config_80211_tx_rate()` can be used *after*
`esp_wifi_start()` on current S3 IDF to raise raw-frame rate to 6/9/12 Mbps
without a connection is disputed in community reports and not stated in the
public API docs; it must be measured, not assumed.

---

## TX needed?

**Yes — unconditionally.** Every active capability in Aquila's scope requires a
transmitter: on-demand CSI needs a source all receivers can observe; a
controlled ping is what converts an opportunistic passive system into a
repeatable one; the round-trip ping-and-unique-reply scheme is TX+RX by
definition (and needs the µs RX timestamp plus a way to TX a unique reply). The
legal envelope is wide enough (ACMA 4 W EIRP, unlimited duty; FCC 1 W DTS, no
dwell) that compliance is a design footnote, not a research risk.

---

## Evidence strength: **8 / 10**

- **High confidence (primary):** the API reference and vendor-features guide; the
  LIPD 2015 and LIPD 2025 Schedule 1 text; 47 CFR §15.247; ETSI EN 300 328
  V2.2.2. These are authoritative and mutually consistent.
- **Deduction from primary code:** that ESP-NOW (MCS0) — not `esp_wifi_80211_tx`
  — is Espressif's CSI sender is read directly from `esp-csi` source; high
  confidence for *that example*, lower for the general claim that *no* raw-TX
  configuration yields CSI.
- **Deduction from issue trackers, not docs:** runtime rate control for raw TX,
  and the link-wrap bypass, rest on GitHub issues and community posts rather
  than vendor documentation; these are the weak points. Hence 8, not 9–10.

---

## Primary sources

- Espressif, *Wi-Fi* API reference, `esp_wifi_80211_tx` —
  https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/network/esp_wifi.html
- Espressif, *Wi-Fi Vendor Features — Wi-Fi 80211 Packet Send* (incl. ESP32-S3
  variant) —
  https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/wifi-driver/wifi-vendor-features.html
- Espressif, *Wi-Fi Driver — Wi-Fi 80211 Packet Send* (ESP32-S3, v4.4.3) —
  https://docs.espressif.com/projects/esp-idf/en/v4.4.3/esp32s3/api-guides/wifi.html
- Espressif ESP-IDF issue #19015, *Per-packet PHY rate selection for
  `esp_wifi_80211_tx`* — https://github.com/espressif/esp-idf/issues/19015
- Espressif ESP-IDF issue #7078, *Rate limitation of `esp_wifi_80211_tx`* —
  https://github.com/espressif/esp-idf/issues/7078
- Espressif, `esp-csi` — `examples/get-started/csi_send/main/app_main.c`
  (ESP-NOW, HT40, MCS0-LGI) — https://github.com/espressif/esp-csi
- Jeija, `esp32-80211-tx` (raw-frame sample; sanity-check bypass) —
  https://github.com/Jeija/esp32-80211-tx ; obsolete predecessor
  https://github.com/Jeija/esp32free80211
- ACMA, *Radiocommunications (Low Interference Potential Devices) Class Licence
  2015*, Schedule 1 items 21/55/56/59 —
  https://www.legislation.gov.au/F2015L01438/2023-05-19/2023-05-19/text/original/epub/OEBPS/document_1/document_1.html
- ACMA, *LIPD Class Licence 2025* (in force 1 Oct 2025), item 7 + clause 43 —
  https://www.legislation.gov.au/F2025L01047/asmade/text
- ACMA, *LIPD class licence* landing page —
  https://www.acma.gov.au/licences/low-interference-potential-devices-lipd-class-licence
- 47 CFR §15.247, *Operation within the bands 902–928 MHz, 2400–2483.5 MHz, and
  5725–5850 MHz* — https://www.ecfr.gov/current/title-47/chapter-I/subchapter-A/part-15/subpart-C/subject-group-ECFR2f2e5828339709e/section-15.247
- ETSI EN 300 328 V2.2.2 (2019-07), *Wideband transmission systems; Data
  transmission equipment operating in the 2,4 GHz band* —
  https://www.etsi.org/deliver/etsi_en/300300_300399/300328/02.02.02_60/en_300328v020202p.pdf
- FCC, *Spread Spectrum Devices*, Second Report and Order (digital modulation =
  1 W, 8 dBm/3 kHz) —
  https://www.federalregister.gov/documents/2002/06/25/02-15951/spread-spectrum-devices

---

## Residual unknowns

1. **Runtime raw-TX rate.** Can `esp_wifi_config_80211_tx_rate()` be applied after
   `esp_wifi_start()` to lift `esp_wifi_80211_tx` above 1 Mbps on current S3
   IDF, and does an OFDM-rate raw frame actually produce CSI at a receiver? Not
   documented; must be bench-measured.
2. **CSI from injected frames.** Whether a 6–54 Mbps raw frame (via the link-wrap
   bypass) yields valid LLTF/HT-LTF CSI as reliably as ESP-NOW's MCS0 broadcast.
3. **Duty rounding under ACMA.** LIPD sets no duty cycle for the DTS item, but
   continuous 100% duty in shared spectrum raises interference-realism and
   equipment-standard questions (out-of-band, EME) that a *device* must still
   satisfy.
4. **Antenna/EIRP of a polyhedron.** Multi-face simultaneous TX (if ever used)
   raises aggregate-power and MIMO/beamforming questions under §15.247(c)(2)(ii)
   and the ACMA EIRP definition ("in any direction").
5. **Action-frame ACK semantics under `no_ack`**, and whether received ACK/BA
   frames carry a usable timestamp for round-trip timing.

---

## Chaining

- **TX (07) enables CSI sensing (01) and I/Q capture (05)** — without a
  transmitter there is no controllable excitation, only opportunistic traffic.
- **TX (07) enables every multi-node geometry (02, 08, 09, 15)** — senders and
  receivers are the two halves of the aperture.
- **The compliance envelope (07) constrains TDM/duty design (09) and any radar
  waveform (06, 08)** — although at 2.4 GHz ACMA/FCC impose no duty limit on
  Wi-Fi, the *shared-channel* reality and EN 300 328 (if EU-relevant) do.
- **Receivable ACK/BA + µs timestamps (07 + 17) enable the round-trip
  ping-and-unique-reply scheme (11)**.
- **Highest-leverage enabling capability:** the ability to emit **OFDM-bearing
  frames at a chosen rate on demand**. That single primitive is what makes CSI
  sensing *active*; the raw non-QoS injection primitive is secondary.
- **Capability that unlocks a further one:** the received ACK/BA timestamp is the
  keystone for the round-trip scheme (range/offset) and therefore for node
  self-localisation (15) and two-way transfer (11).

## Round-trip scheme (ping-and-unique-reply)

One node pings; every other node replies with a **unique** packet; round trips
are timed.

- **Measures:** one-way/round-trip **time offset and range** (via µs RX timestamp
  of the ACK/reply plus the peer's TX timestamp); **identity** (unique payloads
  disambiguate responders); **channel** (CSI on each reply); **Doppler** (temporal
  CSI ratios — moving targets only, blind to static).
- **Breaks where:** the shared channel forces **TDM/half-duplex**; a *receiver*
  that must reply is no longer passive (TX on the same radio); the µs timestamp
  is far too coarse for centimetre ranging (→ FTM/802.11mc, coarse, or a two-way
  exchange with the peer's timestamp); unique replies at the 1 Mbps raw default
  cap the reply rate; and all nodes transmitting must each independently satisfy
  EIRP/PSD. `no_ack`/ACK semantics complicate "did the reply land?" checks.

## Existing models and data

- **`esp-csi`** (Apache-2.0) and the `esp-radar` component — the reference
  send/receive stack; its `csi_send` example is the canonical CSI exciter
  (ESP-NOW, MCS0-LGI). Directly usable.
- **`esp32-80211-tx`** (Jeija) — raw arbitrary-frame sample; useful for
  control/beacon injection, not CSI excitation.
- **ESPARGOS / esp-sdr** — raw I/Q TX+RX research platform; relevant as the
  SDR-grade comparator.
- No published *machine-learning model* addresses the legal envelope; the
  envelope is a constraint on any model, not a data source.

## Own-data / new technology

A purpose-built dataset is not the lever here; a **bench characterisation** is.
**Minimal experiment:** two or more ESP32-S3 nodes on a fixed 2.4 GHz channel and
bandwidth; node A excites frames three ways — (i) `esp_wifi_80211_tx` at the
1 Mbps default, (ii) `esp_wifi_80211_tx` at a raised OFDM rate (configuration API
and, separately, the link-wrap bypass), (iii) ESP-NOW MCS0 — while nodes B…N run
CSI capture; log whether valid CSI appears and at what packet rate. In parallel,
measure conducted/radiated power and PSD of the worst case with a spectrum
analyser against the ACMA 25 mW/3 kHz and FCC 8 dBm/3 kHz limits.

- **Would prove:** which supported primitive generates CSI on demand — i.e.
  whether `esp_wifi_80211_tx` can be the Aquila "ping", or whether ESP-NOW /
  connected-STA framing is mandatory.
- **Would falsify:** the assumption that raw non-QoS injection at any rate yields
  usable OFDM CSI on independent-LO receivers. If it does not, the ping node must
  be an ESP-NOW broadcaster, and the topology and calibre of the round-trip
  scheme must be redesigned accordingly.
