# 17 — Time and frequency precision on ESP32-S3

*Aquila v2 deep-research note. Reference hardware: ESP32-S3. Australian English.
All claims tagged ESTABLISHED / HYPOTHESIS / SPECULATION. Primary sources listed at
the end with URLs.*

---

## Question

What precision do ESP32-S3 **FTM (802.11mc)**, the **microsecond RX timestamp** and
**TSF** actually provide, and what coherence does that imply for Aquila's three radio
capabilities? Specifically:

- (a) RX-timestamp resolution and accuracy;
- (b) TSF behaviour and limits;
- (c) FTM ranging precision;
- (d) resulting relative-LO coherence time and phase error.

Overlap guard: not `10-ota-reciprocity`, not `12-consensus-oscillator`,
not `15-node-self-localisation` (this note stops at *what the hardware clock gives
you*; those notes use it).

---

## Summary answer

| Mechanism | Nominal resolution | Honest accuracy | Cross-node use |
|---|---|---|---|
| System clock (`esp_timer_get_time`) | 1 µs (80 MHz APB) | ±10 ppm clock; ~10 ppm relative | Scheduling only |
| TSF (`esp_wifi_get_tsf_time`) | 1 µs (64-bit, 1 MHz tick) | µs-level within a BSS; ~10 ppm relative drift | Coarse time only |
| RX packet timestamp (`rx_ctrl.timestamp`) | µs in public struct; **ns** via `WIFI_PKT_RX_TIMESTAMP_NSEC` | latched **20.8 µs after sampling start**, residual in **80 MHz cycles (12.5 ns)** | TDoA/alignment, needs common clock |
| MAC clock counter (`esp_wifi_internal_get_mac_clock_time`) | 1 µs, 32-bit | overflows ~71 min | Same clock as RX timestamp |
| FTM timestamps (t1–t4) | picoseconds (64-bit) | metre-level ranging after calibration | Ranging + ppm estimate |
| FTM reported `ppm` | 1 ppm (int16) | relative clock offset local vs peer | LO-offset seed |

The single most important correction to Aquila's working assumption is that the
MAC does **not** only expose a microsecond RX timestamp: a **nanosecond-resolution
RX timestamp** (`WIFI_PKT_RX_TIMESTAMP_NSEC`) has been exposed in ESP-IDF, and the
underlying quantisation is the **80 MHz baseband cycle (12.5 ns)** with a fixed
**20.8 µs latch latency**. This is proven on ESP32 (S2) by the ESPARGOS array and
is the basis of Aquila's cross-node time-alignment work.

---

## (a) RX-timestamp resolution and accuracy

### ESTABLISHED

- **Public field.** `wifi_pkt_rx_ctrl_t.timestamp` is documented as "the local
  time when this packet is received. It is precise only if modem sleep or light
  sleep is not enabled. unit: microsecond." — i.e. **µs resolution, unreliable
  under power save** (ESP-IDF `esp_wifi` API reference, v5.0–v6.1).
- **Nanosecond field.** A patch authored by the ESPARGOS developer (Florian
  Euchner, "Jeija") exposes `WIFI_PKT_RX_TIMESTAMP_NSEC` for a "packet Rx timestamp
  of **nano-second resolution**" (esp-idf issue #9843, Nov 2024). The field is
  "under internal review" in the same thread and has since appeared in releases
  (it is referenced again in issue #15348).
- **MAC clock source.** Espressif staff confirm "this `rx_ctrl.timestamp` is
  obtained from the **internal MAC clock**" (issue #15348, Feb 2025). The raw
  counter for ESP32-S3 is the peripheral register at **`0x60035000`**, a **32-bit,
  1 µs** counter that "will overflow and reset to 0 after approximately **71
  minutes**"; it was later exposed as `esp_wifi_internal_get_mac_clock_time()`
  (32-bit, µs; issue #15348, Nov 2025).
- **True quantisation and latch latency (the key primary evidence).**
  `pyespargos` documentation for `espargos.csi_packet` states verbatim: *"The
  microsecond timestamp is latched **20.8 microseconds after the sampling start**.
  `rxstart_time_cyc` supplies the remaining offset in **80 MHz clock cycles**. CSI-derived
  fractional timing is deliberately not part of this hardware timestamp."* Since
  one 80 MHz cycle is 12.5 ns, the hardware timestamp grid is **12.5 ns**, plus a
  fixed 20.8 µs pipeline latency, plus a fractional term recoverable from CSI phase
  slope.
- **Demonstrated cross-node use.** In issue #9843 the ESPARGOS author reports
  collecting **nanosecond receive timestamps from eight ESP32-S2s all connected to
  the same 40 MHz clock source**, calibrated to a reference packet, and walking a
  transmitter between two arrays: "the time differences of arrival split into two
  groups … the range of the values (around **30–40 ns**) more or less matches the
  TDoA values one would expect for a distance of 5 m (2·5 m / 3·10⁸ m/s = **33.3 ns**)".
  So sub-µs TDoA is demonstrably resolvable **when the receivers share a clock**.
- **CSI timestamps are *not* carrier phase.** `pyespargos` is explicit that the
  timestamp is "only the sampling start time" and excludes the CSI-derived offset;
  and that "CSI-derived fractional timing is deliberately not part of this hardware
  timestamp". Timestamping aligns the **baseband sampling grid (STO)**, not the LO
  carrier phase — the two are calibrated separately (see (d)).

### HYPOTHESIS

- **Latch-path latency is stable to ~ns.** `pyespargos.sensor_calibration` states
  per-sensor timing offsets "serve as the sensor's clock offset … differing only by
  **nanosecond-scale latch-path latencies**". Stable-to-ns is asserted by that
  implementation, not by an Espressif datasheet; it should be re-measured on S3.
- **The 12.5 ns grid is averaged down to sub-ns.** ESPARGOS resolved ~33 ns TDoA
  differences against geometry; the sub-grid fractional term comes from CSI phase
  slope, not the raw timestamp.

### SPECULATION / NOT VERIFIED

- A 2026 secondary source (industrialmonitordirect.com) claims "all four timestamps
  are **10 ns units** (modulo 2³²)" and sub-50 ns FTM-disciplined sync on ESP32-S3.
  This **conflicts with primary evidence** (hardware timestamp is 80 MHz-cycle / 12.5 ns
  with a 20.8 µs latch; FTM report fields are documented in picoseconds) and appears
  to be AI-generated vendor content. Do not rely on it.
- The exact MAC-clock frequency before the 80 MHz-cycle digitiser is **not in the
  public ESP32-S3 TRM**; the TRM chapter on the Wi-Fi MAC timestamp register was not
  located in open sources.

---

## (b) TSF behaviour and limits

### ESTABLISHED

- **What it is.** Every 802.11 station maintains a **64-bit TSF counter with a
  1 MHz tick** (1 µs increments, modulus 2⁶⁴); this is the standard's timing function
  (IEEE 802.11; restated by OpenWiFiSync, 2024). The ESP32-S3 datasheet lists
  "Automatic beacon monitoring (**hardware TSF**)".
- **API.** `int64_t esp_wifi_get_tsf_time(wifi_interface_t)`. It "will return 0"
  for a station that is not connected or has not received a beacon after connecting;
  "Enabling power save may cause the return value inaccurate, except WiFi modem
  sleep" (ESP-IDF `esp_wifi` reference).
- **Accuracy inside a BSS.** The standard keeps TSF synchronised across associated
  STAs to within roughly a few µs plus propagation delay; community and vendor
  figures range from "<1 µs" to "~25 µs", and a beacon-based implementation reports
  ~10 µs average (Mahmood et al. 2016; OpenWiFiSync survey of prior work).
- **Measured relative drift.** OpenWiFiSync measures a **constant skew of ≈3.96 ppm**
  between master and slave TSF over 3000 beacons (~5 min), with deterministic
  multiples on dropped beacons. Independently, an ESP32-S3 user reports TSF
  "drifts ~10 µs/s relative to `rx_ctrl.timestamp`" (≈10 ppm) on their setup
  (issue #15348).

### Limits

- TSF is **µs-resolution**: 1 µs = **300 m** of propagation, so TSF alone is useless
  for fine ranging or coherent phase; it is a *coarse scheduling / holdover* clock.
- It is **beacon-driven**: only sent in beacons/probe responses, so it is
  unavailable against a non-responsive peer (a user notes a Linux AP gives no TSF
  access) and drifts between beacons.
- It is **not a shared clock**. Two nodes tracking the *same* AP reduce their
  relative offset, but TSF never carries carrier phase, so it cannot make CSI phase
  coherent.

---

## (c) FTM (802.11mc) ranging precision

### ESTABLISHED

- **S3 support and ceiling.** ESP32-S3 supports FTM as **Initiator in station mode**
  and **Responder in AP mode** (ESP-IDF Wi-Fi Driver). It is a **2.4 GHz-only**
  part (802.11b/g/n, 20/40 MHz), so **maximum ranging bandwidth is 40 MHz**
  (ESP-FAQ: "ESP32-S3 FTM supports maximum ranging bandwidth up to 40 MHz").
  The original ESP32 has **no FTM hardware**.
- **Field formats.** The FTM report carries `rtt` in **picoseconds** and `t1–t4` in
  **picoseconds** (64-bit), `rtt_raw`/`rtt_est`/`dist_est` in **nanoseconds/metres**,
  and an `int16_t ppm` "clock frequency offset … between local and peer" (ESP-IDF
  `esp_wifi` reference). The `ppm` field is directly useful to Aquila as a
  relative-LO estimate.
- **Standard KPI (90 % CDF error):** **2 m at 80 MHz, 4 m at 40 MHz, 8 m at 20 MHz**
  (802.11mc, quoted in the ESP32 forum and by Espressif support). ESP32-S3 is capped
  at 40 MHz → **≈4 m KPI**.
- **Measured accuracy (ESP32 family):**
  - ESP-IDF own example: `Avg raw RTT: 49.218 nSec … Estimated RTT - 33 nSec,
    Estimated Distance - 5.07 meters` for a true ~5 m link.
  - Barral et al. 2022 (ESP32-S2, outdoor, 90 % within): **<2 m at 40 MHz**, ~2.5 m
    at 20 MHz; **indoor** only ~40 % of samples within 2 m (20 MHz), ~75 % within
    5 m, with tails to 20 m; mean calibrated indoor error **3.52 m**.
  - An Espressif forum test at ~0.5 m reported **22.9 m** estimated distance — FTM
    is poor at short range / without per-device calibration.
  - 2025 cross-implementation study (802.11mc vs .az): a **1 ns RTT error ≈ 0.3 m**
    range error; uncalibrated responders bias distance to tens of metres
    (e.g. 71.9 m at 20 MHz uncalibrated → 11.0 m calibrated).
  - Wikipedia/Android consensus: Wi-Fi RTT positions to **1–2 m**.

### Interpretation

The **timestamps are picosecond counters but the accuracy is metre-level**, because
error is dominated by **MAC-to-PHY transmit/receive delay calibration**, not counter
resolution. FTM is therefore: (i) a coarse rangefinder, and (ii) a convenient
**relative-clock-offset (ppm) sensor** — not a fine timing reference.

---

## (d) Resulting relative-LO coherence time and phase error

### ESTABLISHED — the physical model

Two independent LOs at carrier `f_c` whose crystals differ by a relative offset
`δ = ppm·10⁻⁶` beat at `Δf = δ·f_c`. For 2.4 GHz, **1 ppm = 2.4 kHz**. The CSI
phase of a dynamic path rotates at `2πΔf` (carrier-frequency offset, CFO), with a
subcarrier-slope term from sampling-frequency offset (SFO) and a packet-detection
delay (PDD) intercept; all are well documented (Diaz et al. 2023; "Perceiving
accurate CSI phases", 2014; B2B-calibration paper, 2018).

Define the raw phase-coherence time as the time for the LO phase to drift by π/2:
`τ_{π/2} = 1/(4Δf)`.

| Relative offset | Δf at 2.4 GHz | `τ_{π/2}` (raw) | `τ_{π/8}` (raw) |
|---|---|---|---|
| 2 ppm (trimmed BOM) | 4.8 kHz | 52 µs | 26 µs |
| 10 ppm (measured, ESP32-S3 setup) | 24 kHz | 10.4 µs | 5.2 µs |
| 20 ppm (two worst-case ±10 ppm crystals) | 48 kHz | 5.2 µs | 2.6 µs |

- **Crystal basis (ESTABLISHED).** The ESP32-S3 hardware design guidelines require
  the **40 MHz crystal to be within ±10 ppm**; the APB_CLK high-resolution timer is
  "typically 80 MHz … frequency deviation **<±10 ppm**", 1 µs resolution (ESP-IDF
  System Time). Relative offset between two independent S3s is therefore
  **0–20 ppm**.
- **Residual after correction (ESTABLISHED).** The NIC applies CFO/SFO correction
  but "**residual CFO remains in CSI**" and appears as a slow drift in the phase
  intercept (esp-csi issue #121; Diaz et al.). "Clock offsets are relatively stable
  within a short time (e.g., **in the order of minutes**)", so the rotation is nearly
  constant per lock and removable by normalisation against a reference channel —
  after which RMSE of phase against ground truth was **≈0.117 rad** (B2B method,
  2018). **Calibration, not raw drift, is the practical limit.**

### ESPARGOS two-scalar calibration (ESTABLISHED)

The open, working answer to cross-node phase is a **per-sensor (timing offset,
phase offset) pair**, measured by a calibration run, valid until the receivers
re-lock:

- `sensor_calibration`: "every sensor … is described by two scalars: a **timing
  offset** (its sampling instant relative to the reference sensor) and a **phase
  offset** (its LO phase relative to the reference sensor …). **Both re-roll
  whenever the receivers re-lock**, which is why they must be measured by a
  calibration run."
- `iq_pool.sync()` "sends a **broadband reference signal to all receivers through
  the board's (or multi-board) reference network**. It uses this common signal to
  align the samples in time and measure the phase and delay differences between
  antennas." Run again after any frequency/sample-rate change.
- `estimate_phase_time_offsets()` anchors timing offsets to hardware timestamps and
  estimates constant per-antenna phase offsets separately, in the slope-free domain.

This is the **shared/conducted reference** escape named in the Aquila brief (and its
sibling: the oss-csi "co-crystal" board ties an ESP32-C3 sender and ESP32-S3
receiver "through a clock buffer to eliminate the relative frequency offset").

### Coherence implication for Aquila (HYPOTHESIS)

- **Without a common clock/reference**, cross-node CSI **carrier phase** is
  unobservable (independent-LO gauge — matches Aquila's stated assumption). Raw
  relative-LO phase coherence is only **~5–10 µs**, so per-packet phase is
  unpredictable across nodes and no coherent integration is possible.
- **With a shared 40 MHz clock + conducted phase reference** (ESPARGOS-style), the
  LO phase offset becomes a **static per-lock scalar** (calibratable to ~0.1 rad),
  and the residual limit is re-lock drift and the **12.5 ns sampling-grid + 20.8 µs
  latch** alignment, not LO phase noise. This is **demonstrated on ESP32-S2**, and
  the S3 is the same Wi-Fi IP.
- **Timing implication.** 1 µs (TSF/public timestamp) = 300 m → useless for coherent
  alignment. 12.5 ns (80 MHz cycle) = **3.75 m** per grid step → coarse; CSI-phase
  fractional timing + averaging gives the sub-ns alignment ESPARGOS demonstrates
  (~33 ns TDoA over 5 m resolved).

---

## ESP32-S3 feasibility

| Requirement | S3 status | Demonstrated on |
|---|---|---|
| µs RX timestamp (`rx_ctrl.timestamp`) | ✅ available | ESP32 family (docs) |
| ns RX timestamp (`WIFI_PKT_RX_TIMESTAMP_NSEC`) | ✅ exposed in ESP-IDF (≥5.x); S3 in family | ESP32-S2 (ESPARGOS, #9843) |
| MAC-clock counter (32-bit µs) | ✅ `esp_wifi_internal_get_mac_clock_time()`, 71 min rollover | ESP32-S3 (Espressif staff, #15348) |
| TSF (64-bit µs) | ✅ `esp_wifi_get_tsf_time`, beacon-driven | ESP32-S3 (datasheet; users) |
| FTM initiator/responder | ✅ S3 both roles, **max 40 MHz**, ~4 m KPI | S3 (docs); accuracy measured on S2/C3 |
| Cross-node ns TDoA | ✅ **only with shared clock** | ESP32-S2 ×8, common 40 MHz (ESPARGOS) |
| Cross-node carrier phase | ✅ **only with shared/conducted reference** | ESP32-S2 (ESPARGOS sync) |

**Hardware honesty / gap.** The ns-timestamp TDoA demonstration is **ESP32-S2**
(not S3); the S3 is stated by Espressif to carry the same RX-timestamp field and
the same hardware TSF/FTM IP, but a **published S3-specific ns-TDoA or FTM-accuracy
measurement was not found**. FTM accuracy numbers come from **ESP32-S2/C3 and other
COTS Wi-Fi**; the S3's 40 MHz ceiling places it at the 4 m KPI tier. This is **not**
SDR-grade timing: an ESPARGOS-style common clock is **conducted** (distributed
coax/PCB reference), so it is a *hardware-modification* escape, not a wireless one.
The three wireless-only escapes (common transmitter, OTA reciprocity, shared
reference over the air) remain **unproven on S3** — consistent with the brief.

## TX needed?

- **No TX** for: reading RX timestamps, TSF, or operating S3 as an FTM
  initiator/responder (FTM supplies its own frames).
- **TX needed** for: using the timestamps to measure a deliberate round trip
  (ping-and-unique-reply), supplying a **common-transmitter reference** to make
  cross-node phase observable, or distributing a wireless reference. ESP32 has
  **no published per-frame TX hardware timestamp** for arbitrary frames
  (`esp_wifi_80211_tx`) — issue #9843 explicitly requests it; only FTM's t1/t3
  provide TX timestamps, and only inside an FTM session. So the generic
  ping-and-unique-reply scheme times **arrivals precisely but departures only in
  software** (µs–ms scheduling jitter), unless implemented as an FTM exchange.

---

## Evidence strength: **8 / 10**

- **For (strong):** primary, concordant sources — ESP-IDF API reference, ESP32-S3
  datasheet and hardware-design guidelines, the `pyespargos` API/CDN docs (which
  state the 20.8 µs latch and 80 MHz-cycle residual exactly), Espressif engineer
  statements in issues #9843/#15348, the ESPARGOS peer-reviewed/published work, and
  independent FTM measurement papers (Barral 2022; 2511.17935; 2303.03766).
- **Against (deductions):** no public TRM entry for the MAC timestamp clock before
  the digitiser; the ns field and cross-clock TDoA are demonstrated on **S2**, not
  S3; FTM accuracy not measured on S3 specifically; the exact latch-latency
  stability is asserted by pyespargos, not a datasheet. The one secondary source
  making stronger claims is low-trust.

---

## Primary sources

- ESP-IDF, *Wi-Fi (esp_wifi) API Reference — ESP32-S3* (TSF, FTM report fields,
  `wifi_pkt_rx_ctrl_t.timestamp`) — v6.1 (stable) and v5.0/v5.3:
  https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/api-reference/network/esp_wifi.html
- ESP-IDF, *Wi-Fi Driver — ESP32-S3* (FTM roles; CSI): https://docs.espressif.com/projects/esp-idf/en/v5.2/esp32s3/api-guides/wifi.html
- ESP-IDF, *System Time — ESP32-S3* (APB_CLK 80 MHz, ±10 ppm, 1 µs; RTC 6.6667 µs):
  https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/api-reference/system/system_time.html
- ESP-IDF example, *FTM* (`Avg raw RTT 49.218 nSec … 5.07 meters`):
  https://github.com/espressif/esp-idf/blob/master/examples/wifi/ftm/README.md
- ESP-FAQ, *Wi-Fi* ("ESP32-S3 FTM … up to 40 MHz"; "ESP32 doesn't have FTM hardware"):
  https://docs.espressif.com/projects/esp-faq/en/latest/software-framework/wifi.html
- Espressif, *ESP32-S3 Series Datasheet* (802.11b/g/n, 20/40 MHz, 1T1R, 802.11mc FTM,
  hardware TSF): https://www.espressif.com/sites/default/files/documentation/esp32-s3_datasheet_en.pdf
- Espressif, *ESP32-S3 Hardware Design Guidelines* ("40 MHz crystal … accuracy … within
  ±10 ppm"): https://docs.espressif.com/projects/esp-hardware-design-guidelines/en/latest/esp32s3/
- esp-idf issue #9843, *Let users access timestamp of reception … (802.11mc FTM)*
  (RX timestamp already in `wifi_pkt_rx_ctrl_t`; `WIFI_PKT_RX_TIMESTAMP_NSEC`; ESPARGOS
  ns-TDoA over 8 ESP32-S2 on a shared 40 MHz clock; FTM error ~2 m at 80 MHz):
  https://github.com/espressif/esp-idf/issues/9843
- esp-idf issue #15348, *Expose access to `wifi_pkt_rx_ctrl_t.timestamp` source*
  (MAC clock at `0x60035000`, 32-bit µs, 71 min rollover;
  `esp_wifi_internal_get_mac_clock_time()`; TSF ~10 µs/s drift):
  https://github.com/espressif/esp-idf/issues/15348
- esp-idf issue #2706, *Incomplete documentation about WiFi RX timestamp*:
  https://github.com/espressif/esp-idf/issues/2706
- pyespargos API reference, `sensor_calibration`, `csi_packet` (20.8 µs latch; 80 MHz
  cycle residual; per-lock timing/phase offsets), `csi_calibration`, `csi_processing`,
  `csi_cluster`, `iq_pool.sync()`:
  https://espargos.net/docs/source/modules.html
- ESPARGOS, *IQ sampling* (broadband reference through shared reference network;
  re-sync after freq/rate change): https://espargos.net/docs/iq-sampling.html
- Gundall & Schotten, *OpenWiFiSync* (TSF 64-bit 1 MHz tick; measured 3.96 ppm skew),
  arXiv:2410.08742: https://arxiv.org/abs/2410.08742
- Barral et al., *Fine Time Measurement for the Internet of Things: A Practical
  Approach Using ESP32* (ESP32-S2 FTM: outdoor <2 m @40 MHz; indoor mean 3.52 m;
  t1–t4 in ps, rtt in ns), arXiv:2401.16517: https://arxiv.org/abs/2401.16517
- *Performance comparison of 802.11mc and 802.11az FTM* (1 ns ≈ 0.3 m; uncalibrated
  bias), arXiv:2511.17935: https://arxiv.org/abs/2511.17935
- *Benchmarking and Security Considerations of Wi-Fi FTM* (timestamp resolution bound
  = baseband sampling period; 1 ns → 30 cm), arXiv:2303.03766:
  https://arxiv.org/pdf/2303.03766
- Mahmood et al., *Clock Synchronization Over IEEE 802.11* (TSF resolution limited to
  1 µs), IEEE TII 2016: https://ieeexplore.ieee.org/document/7782431
- Diaz et al., *Channel phase processing in wireless networks for human activity
  recognition* (CFO/SFO/PDD model), 2023:
  https://www.sciencedirect.com/science/article/pii/S2542660523002834
- *Mitigation of CSI Temporal Phase Rotation with B2B Calibration* (residual rotation
  after NIC correction; RMSE ≈0.117 rad), Sensors 2018:
  https://pmc.ncbi.nlm.nih.gov/articles/PMC6263436/
- *Perceiving accurate CSI phases with commodity WiFi devices* (CFO/SFO/PDD; offsets
  stable over minutes), INFOCOM 2014: https://www.researchgate.net/publication/320250377
- Espressif Techpedia, *ESP-CSI Solution* (co-crystal board: ESP32-C3 + ESP32-S3
  "connected through a clock buffer to eliminate the relative frequency offset"):
  https://docs.espressif.com/projects/esp-techpedia/en/latest/esp-friends/solution-introduction/esp-csi/esp-csi-solution.html
- ESPARGOS dataset paper, *Phase-Coherent WiFi CSI Datasets …*, arXiv:2408.16377:
  https://arxiv.org/pdf/2408.16377v1
- ESPARGOS, *An Ultra Low-Cost, Realtime-Capable Multi-Antenna WiFi Channel Sounder*,
  arXiv:2502.09405: https://arxiv.org/abs/2502.09405

*(Low-trust, flagged: industrialmonitordirect.com "ESP32-S3 Wi-Fi FTM Time Sync:
Sub-µs Disciplined Timers" — claims 10 ns FTM timestamp units and <50 ns sync that
contradict primary evidence; not used as a basis for any finding.)*

---

## Residual unknowns

1. **MAC-clock pre-digitiser frequency and true quantisation.** The public ESP32-S3
   TRM does not document the Wi-Fi MAC timestamp counter. Is the underlying tick
   really 80 MHz (12.5 ns), or coarser with digitiser interpolation? Unverified.
2. **S3-specific ns-TDoA / phase-coherence measurement.** The demonstration is S2.
   Whether the S3 exhibits the same 20.8 µs latch and residual behaviour is assumed,
   not shown.
3. **Latch-latency stability across re-lock and temperature.** pyespargos asserts
   ns-scale latch-path latencies; no long-horizon drift figure for S3.
4. **Whether `WIFI_PKT_RX_TIMESTAMP_NSEC` is in the default release or still a
   patch.** Threads show a user-supplied patch; confirm the shipped IDF version and
   whether it survives without a custom build.
5. **FTM `ppm` field accuracy.** Is the reported relative offset good enough to seed
   LO tracking (sub-ppm over minutes)? No characterisation found.
6. **No TX hardware timestamp for `esp_wifi_80211_tx` frames.** Blocks precise
   round-trip timing outside FTM; unresolved by Espressif.
7. **Temperature-induced ppm.** ±10 ppm is a trim spec; no S3 onboard-temperature
   vs ppm curve found.

---

## (A) Chaining — which capability enables which

- **Shared/conducted clock + reference ⇒ phase coherence ⇒ OTA-AoA, distributed
  beamforming, channel charting.** The **highest-leverage enabler** is the
  *reference distribution network* (one 40 MHz clock line + a conducted calibration
  signal), exactly as ESPARGOS/esp-csi co-crystal boards do. Without it, cross-node
  carrier phase is unobservable and everything phase-based is blocked.
- **RX ns timestamp ⇒ TDoA / array time alignment.** Enabled by the timestamp field,
  but **only realisable with the shared clock**. It is the *measurement* that makes
  geometry recoverable; the shared clock is what makes it *meaningful*.
- **FTM ⇒ coarse ranging + a relative-ppm seed.** Enables rough node self-geometry
  and seeds LO-offset tracking; **blocked for fine ranging** by MAC-PHY delay
  calibration. Feeds `15`/`11`.
- **TSF ⇒ coarse time/scheduling (sub-µs–tens of µs).** Enables TDM slot alignment
  and multi-second holdover; **cannot support phase** and is unavailable against
  silent APs.
- **Doppler/CSI-ratio sensing (non-coherent temporal) ⇒ needs no shared clock and no
  fine timestamp** — it rides on relative phase *within* one receiver over time, so
  it is the one capability that is *not* gated by cross-node coherence.

Dependency sentences: "ns RX timestamp enables TDoA; TDoA blocked without a shared
clock." "Shared reference enables phase-coherent arrays; arrays blocked without it."
"FTM enables a ppm seed; fine FTM ranging blocked without responder calibration."

## (B) Existing models / data / open-source

- **ESPARGOS datasets** (arXiv:2408.16377) — phase-coherent multi-antenna WiFi CSI
  with timestamps and transmitter ground-truth positions; **directly usable** as a
  format/calibration reference (check dataset licence — hosted via espargos.net /
  IEEE DataPort). Hardware: ESP32-S2 arrays sharing a clock.
- **`pyespargos`** — open Python stack implementing the two-scalar (timing, phase)
  calibration and ns timestamps; **reusable methodology** for Aquila's S3 port.
- **Espressif `esp-csi`** — CSI collection firmware (Apache-2.0); documents the
  co-crystal clock-buffer approach.
- **OpenWiFiSync** (arXiv:2410.08742) — open TSF-based clock sync; a baseline for
  coarse Aquila time distribution.
- **B2B calibration method** (Sensors 2018) — reference-channel normalisation that
  preserves CSI phase; **applicable** as Aquila's phase-rotation removal model.
- **FTM measurement corpora/code** (Barral's `esp32s2-ftm-*`) — empirical FTM bias
  models; usable to correct/interpret S3 FTM.

Licence/hardware caveats: the strongest coherent-array results **require the
conducted reference network** (custom hardware), an input format change relative to
plain `esp-csi`, and are demonstrated on S2 — not drop-in for stock S3.

## (C) Own-data / new technology

**Claim to test:** *Two stock-representative ESP32-S3s, given only one shared 40 MHz
clock (and optionally one conducted calibration tone), can be made phase- and
time-coherent to sub-nanosecond sampling alignment and ~0.1 rad LO phase, with the
calibration holding across a re-lock and over minutes.*

**Minimal experiment / dataset.**
1. Two (then N) ESP32-S3 boards sharing a single 40 MHz clock; one common
   transmitter at surveyed positions; capture `WIFI_PKT_RX_TIMESTAMP_NSEC` + CSI per
   frame.
2. **Timestamp characterisation:** inject a known-length coax delay; regress measured
   timestamp difference against true delay to recover the **20.8 µs latch** and the
   **12.5 ns grid**, and measure residual jitter (target: stable to a few ns).
3. **Phase characterisation:** run the ESPARGOS-style sync (broadband reference),
   estimate per-sensor (timing, phase) offsets, and measure (i) residual phase RMSE
   against geometry and (ii) repeatability after forced re-locks and over 10 min.
4. **Cross-check the LO estimate** against FTM's reported `ppm`.

**What would falsify it.**
- Timestamp differences that do **not** reduce to sub-grid after CSI-phase
  interpolation, or a **latch latency that varies > ~1 ns** between boards/re-locks →
  cross-node time coherence is not achievable on S3 without extra hardware.
- Residual LO phase that fails to stay ~0.1 rad over minutes after calibration, or
  re-rolls unpredictably within a lock → phase coherence is only transient and the
  coherent-array premise fails on S3.
- A measured TSF/MAC relative offset inconsistent with FTM `ppm` by orders of
  magnitude → the FTM ppm field is not a usable LO seed.

If it succeeds, Aquila would have a **published S3-specific ns-timestamp + phase
calibration characterisation** that currently exists only for the S2 — a
purpose-built dataset that de-risks every phase-based capability (AoA, channel
charting, distributed coherent Tx) on the reference hardware.
