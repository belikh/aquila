# Aquila ESP32-S3 CSI node firmware

A reference ESP-IDF (C) firmware for a single fixed **ESP32-S3** node that
streams raw WiFi CSI to the Aquila core and updates its own firmware over the
node control channel.

> **Compile status.** This firmware is **compile-verified with ESP-IDF v5.5.2**
> via the repository dev shell `nix develop .#esp` (see *Build and flash* below).
> It is **not flashed and not run on hardware in this repository**: it has never
> been flashed to a board, never observed streaming and never executed an OTA, so
> its on-device behaviour is unverified. The Rust side of the wire is tested; the
> C side is compile-verified only.

It implements the node end of `docs/spec/capture-and-firmware.md` §5 (CSI data
plane), §6 (node control channel, including the per-node token authentication
of §6.3) and §9 (firmware responsibilities and MUST-NOTs), the node-side firmware
OTA agent of `docs/spec/firmware-ota.md` §5, §6 and §8, under the wire layout
of ADR-0004 (magic and framing), ADR-0005 (the raw int8 I/Q payload) and
ADR-0006 (the imaginary-first byte order), and `ADR-0003` §7/§9.4.

## Target

- **Chip:** ESP32-S3.
- **Framework:** ESP-IDF v5.x (the CSI API `esp_wifi_set_csi*` lives in
  `esp_wifi`; the [`esp-csi`](https://github.com/espressif/esp-csi) repository
  has worked examples of the same API).
- **Board:** any ESP32-S3 module with WiFi, for example an
  ESP32-S3-DevKitC-1. Add a USB-UART bridge for flashing if the board lacks one.
- **Host:** Linux (build and flash), matching the Slice-7 Linux-only scope.

## Layout

```
esp32-s3-csi-node/
├── CMakeLists.txt                 ESP-IDF project file
├── partitions.csv                 dual-OTA (A/B) layout: two app slots + otadata
├── sdkconfig.defaults             target, CSI, 4 MB flash + dual-OTA + rollback
├── main/
│   ├── CMakeLists.txt
│   ├── Kconfig.projbuild          WiFi, core endpoint, node id, pairing code
│   └── main.c                     network join + orchestration + OTA health
└── components/
    ├── node_wire/                 framing: the C half of the one wire layout
    │   ├── CMakeLists.txt
    │   ├── node_wire_util.c
    │   └── include/
    │       ├── node_wire.h        GENERATED from aquila-wire — do not edit
    │       └── node_wire_util.h
    ├── csi_node/                  capture + CSI data plane (§5, §9.1)
    │   ├── CMakeLists.txt
    │   ├── csi_node.c
    │   └── include/csi_node.h
    ├── node_control/              AQCC control channel (§6)
    │   ├── CMakeLists.txt
    │   ├── node_control.c
    │   └── include/node_control.h
    └── node_ota/                  firmware OTA agent (firmware-ota.md §5, §6, §8)
        ├── CMakeLists.txt
        ├── Kconfig                health window, security floor, trust anchor
        ├── node_ota.c
        ├── ed25519/               vendored Ed25519 verification (public domain)
        │   ├── ed25519.c
        │   └── ed25519.h
        └── include/node_ota.h
```

### One wire layout, two languages

`components/node_wire/include/node_wire.h` is **generated**, not hand-written.
It is emitted from the Rust codec:

```sh
nix-shell --run 'cargo test -p aquila-capture emit_node_wire_header -- --ignored'
# writes /tmp/opencode/node_wire.h; set NODE_WIRE_OUT to change the path
```

Copy that file to `components/node_wire/include/node_wire.h`. A CI test
(`aquila-capture`, `committed_node_wire_header_is_current`) asserts the
committed header equals `aquila_wire::generate_c_header()` — it compares the
**header text** only. It does not compile or run the C framing, so
`node_wire_util.c` could still drift from the layout the header declares; a
shared golden-vector test that feeds fixed inputs through the C framing and
compares bytes with the Rust encoder is planned, not yet present (F37,
`capture-and-firmware.md` §9.4).

## Build and flash

### Compile in the repository dev shell (ESP-IDF v5.5.2)

The repository pins ESP-IDF v5.5.2 and the `xtensa-esp32s3` toolchain in the
`esp` dev shell, so no host-wide ESP-IDF install is needed to **compile**:

```sh
nix develop .#esp --command bash -c \
  'cd firmware/esp32-s3-csi-node && idf.py set-target esp32s3 && idf.py build'
```

This is the exact path by which the firmware is compile-verified, and it emits
`build/bootloader/bootloader.bin`, `build/partition_table/partition-table.bin`,
`build/ota_data_initial.bin` and `build/aquila_esp32s3_csi_node.bin`. The image
is ~750 KB (`0xb7810` bytes) against a ~1.98 MB app slot, so both A/B slots fit
comfortably in 4 MB. Compiling needs no hardware; **flashing does**, and flashing
is not possible in this repository (there is no board).

The build uses the custom dual-OTA partition table in `partitions.csv`
(`ota_0` at `0x20000`, `ota_1` at `0x210000`, `otadata` at `0xf000`), selected
by `CONFIG_PARTITION_TABLE_CUSTOM` in `sdkconfig.defaults`. `sdkconfig` itself
is generated and not committed.

### Or install ESP-IDF v5.x yourself

```sh
git clone --recursive https://github.com/espressif/esp-idf.git ~/esp/esp-idf
. ~/esp/esp-idf/export.sh
```

Then, from this directory:

```sh
idf.py set-target esp32s3
idf.py menuconfig        # "Aquila CSI node": set SSID, password, core host/ports
idf.py build
idf.py -p /dev/ttyUSB0 flash monitor
```

`idf.py menuconfig` exposes the node settings declared in
`main/Kconfig.projbuild`. Every value is a **factory default only**: the node
loads its real provisioning from NVS first and falls back to these when the
corresponding key is absent (`capture-and-firmware.md` §8.3, §9.1.8, §9.3).

| Option | Meaning |
| --- | --- |
| `AQUILA_WIFI_SSID` / `AQUILA_WIFI_PASSWORD` | factory-default network credentials (provision the real ones into NVS) |
| `AQUILA_CORE_HOST` | the add-on's IPv4 literal (the node talks only to it) |
| `AQUILA_CORE_DATA_PORT` | CSI data plane, default `9100` |
| `AQUILA_CORE_CONTROL_PORT` | node control plane, default `9101` |
| `AQUILA_NODE_ID` | dense node id, provisioned by the operator |
| `AQUILA_PAIRING_CODE` | the short-lived pairing code, `0` to skip join |

The **Aquila firmware OTA** menu (`components/node_ota/Kconfig`) carries the OTA
tuning:

| Option | Meaning |
| --- | --- |
| `AQUILA_OTA_HEALTH_WINDOW_S` | pending-verify health window before an image confirms itself, default `60` |
| `AQUILA_FW_SECURITY_VERSION` | compiled-in anti-rollback floor |
| `AQUILA_FW_ALLOW_DOWNGRADE` | explicit factory-authorised downgrade flag |
| `AQUILA_FW_PUBKEY_HEX` | compiled-in Ed25519 trust anchor (64 hex chars); NVS `ota`/`fw_pubkey` wins |

The `aquila-capture provision` subcommand writes the provisioning payload
(SSID, password, core endpoint) as JSON for an operator to flash into NVS under
the `aquila` namespace (`wifi_ssid`, `wifi_pass`, `core_host`,
`core_data_port`, `core_control_port`, `node_id`, `pairing_code`); pairing itself
happens on the control channel. Because the credentials live in NVS, a
distributed image does not carry the network password (G13).

## Behaviour

- **Capture** (`csi_node`) copies the raw int8 CSI buffer and `rx_ctrl`
  metadata in the receive callback — no allocation, no logging, no floating
  point — into a fixed-capacity single-producer/single-consumer ring. On
  overflow the frame is dropped and counted.
- **Data plane** (sender task) copies the raw signed int8 I/Q buffer into the
  AQCI payload verbatim (ADR-0005) and unicasts one datagram per frame, subject
  to `max_tx_rate_hz`; frames over the cap are dropped, not queued. The node
  performs no floating-point conversion: the core derives amplitude and phase.
- **Control plane** (`node_control`) serves `JOIN_REQUEST`/`JOIN_ACK`,
  `GET_CONFIG`/`SET_CONFIG`/`SET_CSI`, `STATUS` heartbeats, `SYNC_PING` →
  `SYNC_ECHO`, `REBOOT`, `IDENTIFY` and `FACTORY_RESET`, persisting the pairing
  token and config version in NVS.
- **Firmware OTA** (`node_ota`, `firmware-ota.md` §5–§8). The agent rides the
  authenticated control channel: `OTA_BEGIN` validates the target/slot and the
  version, applies the anti-rollback floor and opens the inactive partition with
  `esp_ota_begin`; `OTA_CHUNK` checks a CRC-32 per chunk, writes it with
  `esp_ota_write` and emits a cumulative `OTA_ACK`; `OTA_END` finalises the
  SHA-256 and verifies an **Ed25519** signature over that digest against the
  provisioned trust anchor, then `esp_ota_set_boot_partition` and reboots;
  `OTA_ABORT` discards the transfer. Failures are reported as `OTA_RESULT`. The
  agent is implemented and compile-verified (ESP-IDF v5.5.2) but hardware-untested;
  a trust anchor (`CONFIG_AQUILA_FW_PUBKEY_HEX` or NVS `ota`/`fw_pubkey`) must be
  provisioned, or OTA fails closed.
- **Pending-verify and rollback** (`firmware-ota.md` §6). On the first boot of a
  new image the node enters a pending-verify state and starts a health task; if
  the node stays healthy (associated + streaming + control responsive) for the
  health window (default 60 s) it calls
  `esp_ota_mark_app_valid_cancel_rollback()`, persists the flag in NVS and moves
  the security-version floor up; otherwise the bootloader rolls back.
- **Anti-rollback** (`firmware-ota.md` §4). A monotonic security version is
  persisted in NVS and only rises after a confirmed update; an image whose
  version encodes a lower security version is rejected, unless the
  factory-authorised `CONFIG_AQUILA_FW_ALLOW_DOWNGRADE` flag is set.
- **Trust anchor** (`firmware-ota.md` §4). The 32-byte Ed25519 public key is
  read from NVS (`ota`/`fw_pubkey`) or falls back to the compiled-in
  `CONFIG_AQUILA_FW_PUBKEY_HEX`. With neither set the node has no anchor and
  fails every verification closed. ESP-IDF's bundled mbedTLS has no Ed25519
  signature primitive, so the verifier is a small vendored public-domain
  implementation (`node_ota/ed25519/`); SHA-256 and CRC-32 use mbedTLS and the
  ESP ROM respectively.
- **Control authentication** (`capture-and-firmware.md` §6.3, F21). After
  pairing, every inbound command must carry the 32-byte HMAC-SHA256 trailer
  keyed by the pairing token, must come from the pinned core endpoint, and must
  present a `request_id` not seen in the recent-replay window. `JOIN_ACK` is the
  one message accepted before pairing, because it delivers the token. Until the
  core attaches the same trailer (the wire codec provides
  `encode_control_authenticated`), the node **fails closed**: it refuses every
  command. Node→core replies and `STATUS` are not yet authenticated, so the
  control channel is authenticated core→node only in v1.
- **MUST NOTs** (§9.2): the node performs no DSP, inference, fusion,
  calibration, filtering or FFT; never disciplines its clock; never transmits
  elicitation waveforms; and uses no ESP-NOW, MQTT, Matter, Zigbee, BLE or mesh.

## What is untested

The C under this directory **compiles** (ESP-IDF v5.5.2, `nix develop .#esp`),
but none of it is exercised on hardware. In particular:

- the `esp-csi` raw buffer layout copied verbatim as the int8 I/Q payload
  (`[imag, real]` versus `[real, imag]` per subcarrier, and `first_word_invalid`
  nulling the first subcarrier) — verify the byte order against the ESP-IDF
  version on the board;
- `wifi_pkt_rx_ctrl_t` field names used for the PHY flags (`sig_mode`, `cwb`,
  `stbc`, `fec_coding`, `sgi`, `not_sounding`) — these vary across ESP-IDF
  versions. `sig_mode` 1 sets the HT bit and 3 sets VHT; the ESP32-S3 radio is
  11n, so VHT and the reserved HE bit never occur on this target, but the
  vocabulary no longer conflates 11ac/11ax with 11n (F10);
- the NVS provisioning path;
- the OTA agent end to end: the `esp_ota_*` A/B write, activation and reboot;
  the pending-verify health window and `esp_ota_mark_app_valid_cancel_rollback`
  / `esp_ota_mark_app_invalid_rollback_and_reboot` rollback path; and the
  whether-the-image-fits and partition-selection behaviour against a real
  4 MB flash — hardware-gated; none of this is exercised on hardware here;
- the vendored Ed25519 verifier (`node_ota/ed25519/`) against a real signed
  image — hardware-gated: it is the published TweetNaCl reference, but only the
  Rust core signs and verifies images in this repository, so the C verifier has
  not been round-tripped against a core-produced signature;
- the anti-rollback floor and trust-anchor provisioning in NVS (a fresh node
  with no `fw_pubkey` fails every OTA closed by design);
- the control-channel HMAC-SHA256 verification and replay window
  (`mbedtls_md_hmac`), and the matching core-side trailer;
- the internal temperature-sensor read used by `STATUS` (the sensor may be
  unavailable, in which case the field reads `0`);
- the `IDENTIFY` LED (currently disabled: set `IDENTIFY_GPIO` in
  `node_control.c` to a valid GPIO);
- on-hardware streaming, loss, rate and timing.

The Rust core, wire codec and the virtual-node tracer (`aquila-testkit`) are the
tested side of the same protocol.
