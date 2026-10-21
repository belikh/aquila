# The ESP32-S3's ROM, and its PHY interface

The ESP32-S3's WiFi MAC, PHY and baseband are not in the node's flash: they
are the chip's mask-programmed ROM. That ROM is where the channel estimator
lives, and the channel estimator is where the per-frame sign ambiguity of
§4 of the research note comes from. This directory is the extraction that
establishes what the ROM exposes and where its boundaries are.

## Why the ROM is not in the flash

The node's partition table declares five regions — `nvs`, `otadata`,
`phy_init`, `ota_0` and `ota_1` — and `idf.py flash` writes four of them: the
bootloader, the partition table, the application and the OTA state. The
`phy_init` region at 0x11000 is declared and never written.

That is not a fault. The ESP32-S3's PHY has no code blob: the application
reaches the ROM's PHY through a function table, and the only flash-side
artefact is the PHY's *calibration* data — `phy_multiple_init_data.bin`, 1072
bytes of RF tables with a `PHYINIT` header, most of it unpopulated. The
application does not even read the partition on this target:
`esp_phy_get_init_data` returns the built-in `phy_init_data` struct directly.
The partition is used only by the country-code update path.

So the "binary blobs in use" are:

| blob | where | what |
| --- | --- | --- |
| the ROM | the chip | the WiFi MAC, PHY and baseband, including the channel estimator |
| the PHY init data | the `phy_init` partition, or the built-in struct | RF calibration tables, 1072 bytes |
| the application | `ota_0` / `ota_1` | this project's code, plus the IDF's closed WiFi driver objects |
| the provisioning and the OTA trust anchor | `nvs` | |

## The ROM's PHY function table

The application reaches the ROM's PHY through a dispatch table.
`phy_get_romfunc_addr` in the driver calls the ROM's `phy_get_romfuncs`, which
returns `rom_phyFuns[0]`, and then **patches nine of the ROM's entries with
the application's own RAM implementations** — the temperature-to-power
conversion, the PLL volume calibration, the WiFi and Bluetooth transmit gains,
the I²C host id, the transmit-power tracking and the WiFi digital gain.

The table lives in the ROM's RAM and is filled at boot, so the published ELF
cannot show it. The only place it exists is a running node. `extract_rom.py`
and `resolve_rom_phy_table.py` in this directory are the extraction: the
firmware dumps the table at boot, and the dump is resolved against the ROM's
symbol index.

`rom_phy_table_resolved.json` is the result: 175 entries, of which **135 are
the ROM's own named functions** and 30 are the application's replacements
patched over them.

## What the table says about the sign ambiguity

The table is the complete list of hardware controls the chip exposes, and it
is worth reading for what it does *not* contain. It has the AGC
(`rom_enable_agc`, `rom_disable_agc`), the noise floor
(`rom_phy_get_noisefloor`, `rom_read_hw_noisefloor`), the calibration
(`rom_dc_iq_est`, `rom_iq_corr_enable`, `rom_iq_est_disable`, `rom_set_rx_comp`,
`rom_set_cal_rxdc`), the gain control, the I²C and the power control. Every
one of them is a *configuration* or a *calibration* function.

**None of them is a per-frame phase function.** The channel estimate is
produced per frame by the baseband's hardware, and the ROM's table exposes no
control over it. That is the boundary: the per-frame sign is decided in
hardware that the application cannot reach and cannot configure.

One control was tested directly. The application calls
`rom_iq_est_disable` through the table — the DC and I/Q-imbalance correction
is exactly the kind of thing that would negate a whole channel vector if its
sign were taken from a noisy estimate. The register it clears was read before
and after the call: the bits were **already clear**, so the call was a no-op
and the flip is present with that switch in that state. The switch does not
gate the flip.

## Reproducing it

```
# the ROM ELF comes from the Espressif toolchain
python3 extract_rom.py ~/.espressif/tools/esp-rom-elfs/20241011/esp32s3_rev0_rom.elf out/
# the table comes from a running node: the firmware's ROMPHY dump
python3 resolve_rom_phy_table.py
```

The ROM's bytes are not copied into this repository — they are Espressif's.
`rom_manifest.json` records the source ELF's SHA-256 so the extraction can be
checked against the same ROM.
