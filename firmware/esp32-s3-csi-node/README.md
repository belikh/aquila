# Aquila ESP32-S3 CSI node firmware

A reference ESP-IDF (C) firmware for a single fixed **ESP32-S3** node that
streams raw WiFi CSI to the Aquila core.

> **This firmware is not flashed or tested on hardware in this repository.**
> It is a reference source tree: it has never been built with a real ESP-IDF
> toolchain, never flashed to a board and never observed streaming. Treat it as
> a starting point, not as verified code. The Rust side of the wire is tested;
> the C side is not.

It implements the node end of `docs/spec/capture-and-firmware.md` §5 (CSI data
plane), §6 (node control channel) and §9 (firmware responsibilities and
MUST-NOTs), under the wire layout of ADR-0004 and `ADR-0003` §7/§9.4.

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
├── sdkconfig.defaults             target + CSI-enabled defaults
├── main/
│   ├── CMakeLists.txt
│   ├── Kconfig.projbuild          WiFi, core endpoint, node id, pairing code
│   └── main.c                     network join + orchestration
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
    └── node_control/              AQCC control channel (§6)
        ├── CMakeLists.txt
        ├── node_control.c
        └── include/node_control.h
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
committed header equals `aquila_wire::generate_c_header()`, so the two languages
cannot silently drift (`capture-and-firmware.md` §9.4).

## Build and flash

Install ESP-IDF v5.x and export its environment:

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
`main/Kconfig.projbuild`:

| Option | Meaning |
| --- | --- |
| `AQUILA_WIFI_SSID` / `AQUILA_WIFI_PASSWORD` | the home network to join |
| `AQUILA_CORE_HOST` | the add-on's IPv4 literal (the node talks only to it) |
| `AQUILA_CORE_DATA_PORT` | CSI data plane, default `9100` |
| `AQUILA_CORE_CONTROL_PORT` | node control plane, default `9101` |
| `AQUILA_NODE_ID` | dense node id, provisioned by the operator |
| `AQUILA_PAIRING_CODE` | the short-lived pairing code, `0` to skip join |

The `aquila-capture provision` subcommand writes the equivalent provisioning
payload (SSID, password, core endpoint) as JSON for an operator to flash into
NVS; pairing itself happens on the control channel.

## Behaviour

- **Capture** (`csi_node`) copies the raw int8 CSI buffer and `rx_ctrl`
  metadata in the receive callback — no allocation, no logging, no floating
  point — into a fixed-capacity single-producer/single-consumer ring. On
  overflow the frame is dropped and counted.
- **Data plane** (sender task) converts the int8 I/Q pairs to the f32
  amplitude/phase the AQCI payload carries (ADR-0004) and unicasts one datagram
  per frame, subject to `max_tx_rate_hz`; frames over the cap are dropped, not
  queued. The conversion is a wire-representation change performed outside the
  callback, not DSP.
- **Control plane** (`node_control`) serves `JOIN_REQUEST`/`JOIN_ACK`,
  `GET_CONFIG`/`SET_CONFIG`/`SET_CSI`, `STATUS` heartbeats, `SYNC_PING` →
  `SYNC_ECHO`, `REBOOT`, `IDENTIFY` and `FACTORY_RESET`, persisting the pairing
  token and config version in NVS.
- **MUST NOTs** (§9.2): the node performs no DSP, inference, fusion,
  calibration, filtering or FFT; never disciplines its clock; never transmits
  elicitation waveforms; and uses no ESP-NOW, MQTT, Matter, Zigbee, BLE or mesh.

## What is untested

Everything in C under this directory. In particular:

- the `esp-csi` raw buffer layout assumed by the int8 → f32 conversion
  (`[imag, real]` per subcarrier, `first_word_invalid` nulling the first
  subcarrier) — verify against the ESP-IDF version on the board;
- `wifi_pkt_rx_ctrl_t` field names used for the PHY flags (`sig_mode`, `cwb`,
  `stbc`, `fec_coding`, `sgi`) — these vary across ESP-IDF versions;
- the NVS provisioning path and `IDENTIFY` LED (currently disabled: set
  `IDENTIFY_GPIO` in `node_control.c` to a valid GPIO);
- on-hardware streaming, loss, rate and timing.

The Rust core, wire codec and the virtual-node tracer (`aquila-testkit`) are the
tested side of the same protocol.
