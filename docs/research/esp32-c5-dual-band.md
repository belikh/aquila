# Research — ESP32-C5 dual-band vs ESP32-S3 for CSI

**Status:** research. Decision taken: **ESP32-S3 is the reference capture device** (C6 noted as a
future option); ESP32-C5 rejected for this use case.
**Last updated:** 2026-10-13. **Sources:** see `../reference/sources.json`.

## Question

Does the ESP32-C5's native dual-band 2.4/5 GHz radio make it a better reference device than the
ESP32-S3 for home CSI sensing across three floors?

## Findings

- **CSI support:** `esp-csi` lists the C5, and Espressif confirmed 5 GHz CSI in Oct 2025, but it is
  young — on C5 v1.0 / IDF 5.5.4 the 5 GHz IQ buffer was frozen (fixed only via an undocumented
  register in Apr 2026), `esp-radar` console was incompatible, and UDP streaming bugs persist.
  The S3 is the most-exercised target (examples, ESPectre, most published work).
- **Does 5 GHz help?** Penetration is **worse** — higher frequency attenuates more, punishing
  cross-floor links. Subcarrier **count** comes from Wi-Fi 6 HE-LTF (available on the cheaper,
  mature **C6**), not from the 5 GHz band. 5 GHz has cleaner phase but lower SNR through floors.
- **Small displacement:** 5 GHz phase changes ~2× per displacement (shorter wavelength) — the only
  genuine advantage, relevant to respiration, but roughly cancelled by the SNR loss.
- **Coherence:** cross-node phase coherence is a **clock** problem, not a band problem; dual-band is
  silent on it. Espressif's only coherence solution (`esp-crab`) shares one crystal across two C5s —
  two nodes, not three floors, and its hardware files are unpublished.
- **Availability:** C5 GA in 2026 (ESP-IDF v5.5+/v6.x), ~$15 devkits.

## Implication for Aquila

Dual-band does not genuinely help. **ESP32-S3 is the reference** (deepest tooling, best floor/wall
penetration). If more subcarriers ever prove necessary, evaluate the 2.4 GHz-only **C6**, which
already has Wi-Fi 6 HE-LTF. Prototype multi-node synchronisation first; no band choice solves it.
