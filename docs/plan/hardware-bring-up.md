# Hardware bring-up — ESP32-S3 CSI nodes

Status: runbook, prepared while the reference boards are in transit. Prose in
Australian English.

This is the operational plan for the day the ESP32-S3 boards arrive. It is
deliberately blunt about what is *not* ready: the boards make the v1 claims
testable, but they do not make them true on their own, and several pieces of the
production wiring are still absent. Read §1 before you open the box.

Related documents: `docs/INSTALL.md` §3–§5 (the operator-facing install path),
`docs/spec/capture-and-firmware.md` §5–§6 (the wire), `docs/spec/firmware-ota.md`
(OTA), `docs/spec/evaluation.md` (how a claim becomes measured),
`firmware/esp32-s3-csi-node/README.md` (the firmware itself).

---

## 0. What the boards unlock

Every claim in `docs/README.md` is graded `target` because none of it has been
measured on hardware. The boards unlock three distinct things, in increasing
order of effort:

1. **Wire correctness.** The C half of the one wire layout has never been
   flashed (`firmware/esp32-s3-csi-node/README.md` "What is untested",
   §"What is untested"). The imaginary-first byte order, the
   `first_word_invalid` handling, the PHY-flag decode, the HT20/HT40 null-bin map
   and the subcarrier offset are **assumptions** until a real capture confirms
   them (`docs/spec/signal.md:652`; `csi_node.c:224`).
2. **End-to-end sensing.** Presence/occupancy, person count, coarse pose and
   vitals on real multipath, and the three gated v1 bars
   (`docs/spec/evaluation.md:351-375`):
   - in-domain torso PCK@20 ≥ 65%,
   - in-domain presence F1 ≥ 0.90 (per zone, macro),
   - in-domain breathing MAE ≤ 2 brpm (vs a respiratory belt).
   Heart-rate MAE is reported with a baseline-fixed bar (no fixed number).
3. **OTA.** A real A/B flash, the pending-verify health window, activation and
   automatic rollback (`docs/spec/firmware-ota.md` §6).

Whole-home claims (presence F1 and count MAE at ~9+ nodes) need the full
deployment, not one or two boards (`docs/spec/roadmap.md:247-264`).

---

## 1. Honest preconditions — what is not ready yet

These are **software gaps**, not hardware ones. They were hardware-gated when
tickets #8, #13–#15 were closed, so the code compiles and the hardware-free
tests pass, but the production path that a real node needs is not wired. In
severity order:

### P1 — the add-on never ingests CSI by default

`aquila_core/config.yaml:19-20` publishes UDP `9100` (CSI) and `9101` (control),
and `aquila_core/Dockerfile:99-100` exposes them, but the composition root binds
only `8099`. It binds `9100` **only** when `AQUILA_LIVE_ADDR` is set
(`crates/aquila-addon/src/main.rs:452-468`), and nothing sets that variable — the
image's `ENV` is only `AQUILA_COMMIT`, `AQUILA_DASHBOARD_DIR`, `RUST_LOG`
(`aquila_core/Dockerfile:105-107`). So a shipped add-on repeats the synthetic
demo scene and never opens the CSI socket.

- **Blocks:** everything. No board can reach the core until this is fixed.
- **Fix (small):** default the live bind to `0.0.0.0:9100` when no
  `AQUILA_LIVE_ADDR` is given, keeping the simulator as the fallback when no node
  is present (or set `AQUILA_LIVE_ADDR=0.0.0.0:9100` in the image/run script).

### P2 — there is no core-side node control server (UDP 9101)

`aquila_sources::NodeControl` is a **client** (core → node)
(`crates/aquila-sources/src/control.rs:1-8,107-170`). The only things that
*serve* the AQCC channel are the test fixture `VirtualNode`
(`crates/aquila-testkit/src/virtual_node.rs`) and the firmware. There is no
production listener that answers `JOIN_REQUEST` with `JOIN_ACK`, accepts
`STATUS` heartbeats, or drives `SYNC_PING` → `SYNC_ECHO`. There is also **no**
`POST /nodes/pairing` endpoint (grep of `crates/aquila-api` finds none), so the
pairing flow of `capture-and-firmware.md` §6.3 has no core half.

- **Blocks:** pairing, node config (`GET_CONFIG`/`SET_CONFIG`/`SET_CSI`),
  `IDENTIFY`, `SYNC_PING`, and the OTA transport (P3).
- **Fix (large):** a control-plane server in `aquila-addon` that owns the
  per-node `NodeControl` endpoints, a pairing/token store under `/data`, and the
  `POST /nodes/pairing` API. Ticket #8's "enrolment gate" and "control gate"
  were hardware-gated; this is the missing production half.

### P3 — the firmware image store and OTA manager are never constructed

The signed image store, the transfer manager and the `0x0E`–`0x14` opcodes exist
and are tested (`crates/aquila-api/src/ota/`, ticket #13), the firmware agent
exists (`firmware/esp32-s3-csi-node/components/node_ota/`, ticket #14) and the HA
`update` entity exists (`custom_components/aquila/update.py`, ticket #15). But
the composition root never builds a store or registers a per-node
`OtaTransport`, so `handle.ota().has_store()` is false and every install returns
`503 ota_unavailable` (`crates/aquila-api/src/server.rs:432-443`). OTA is dead by
construction, not by omission in the specs.

- **Blocks:** OTA end to end (Step 8).
- **Fix:** wire the store (`FirmwareStore` under `/data/aquila/firmware/`) and a
  UDP `OtaTransport` per paired node into the add-on; depends on P2.

### P4 — the topology is hard-coded to the demo scene

The composition root builds its topology from `demo_scene()`
(`crates/aquila-addon/src/main.rs:183,731-788`): nodes 1 and 2 in
`kitchen`/`lounge`. It never loads an operator-defined topology from `/data`, and
the API has no topology-write route. Real nodes with other ids, rooms and floors
have no mapping, and calibration is keyed to a topology digest, so a wrong
topology invalidates it.

- **Blocks:** node→room mapping, the entity model, calibration, all sensing.
- **Fix:** load a topology document from `/data`, add the node-registration and
  room-assignment routes the dashboard and pairing flow need.

### P5 — the physical reference adapters are unimplemented

`aquila-evaluation` ships only `FixtureGroundTruth`
(`crates/aquila-evaluation/src/ground_truth.rs:8-10`). The two physical adapters
the bars need — `ReferenceVitalsGroundTruth` (respiratory belt for breathing, a
chest strap for ECG-derived heart rate) and `MmFiLabelGroundTruth` (pose) — are
specified (`docs/spec/evaluation.md:92-97`) but not implemented. Without them the
`Harness` cannot pair hardware estimates to references, and the bars stay
`target` no matter how good the sensing is.

- **Blocks:** the three v1 bars.
- **Needs:** the physical reference devices (§2), plus the adapters and the
  alignment/calibration they carry.

### P6 — smaller honesty and ergonomics items

- `IDENTIFY`'s LED blink is compiled out (`IDENTIFY_GPIO (-1)`,
  `firmware/.../node_control.c:46,453-458`), so a node cannot be physically
  identified during install (F33).
- `config_hash` is carried on `SET_CONFIG` but not verified (no canonical
  algorithm); a documented v1 limitation (`capture-and-firmware.md:455-459`,
  F7).
- The HT20/HT40 null-bin map and `subcarrier_offset = -(n/2)` are marked
  **PROVISIONAL** in code and spec and need a real capture (S11/F19).
- `docs/spec/firmware-ota.md:5-15` and `capture-and-firmware.md:449-453` still
  say OTA is "not yet implemented — see ticket #14", but #14 shipped the firmware
  agent; the status banners are stale and should be corrected.

---

## 2. Bench inventory

Gather these **before** the boards land so day 0 is unblocked:

- **Boards.** Reference: ESP32-S3-DevKitC-1. Any ESP32-S3 module with WiFi works.
  The dual-OTA partition table needs **≥ 4 MB flash**
  (`sdkconfig.defaults:14`, `partitions.csv`); a DevKitC-1 N8 (8 MB) works but
  only 4 MB is used unless `sdkconfig.defaults` is changed. Order a **spare**.
- **Cables.** USB **data** cables (many bundled cables are charge-only); a
  powered hub. The DevKitC-1 has two USB-C ports: **UART** (silicon-lab bridge,
  `/dev/ttyUSB0`) for flashing/console, and **USB** (native USB-OTG,
  `/dev/ttyACM0`). Use the UART port by default.
- **WiFi.** A 2.4 GHz AP on the same LAN as the HA host. **Fix the channel and
  width** (v1 defaults: channel 6, HT20, `demo_scene()` at
  `crates/aquila-addon/src/main.rs:683-686`). The node's sensing channel must
  match the AP it is associated with, and every node must use the same channel or
  the CSI is not comparable across nodes.
- **The HA host's LAN IPv4.** Nodes talk to `<host-LAN-IP>:9100`, **not** the
  add-on's `hassio` hostname — they cannot resolve Supervisor DNS and the CSI
  port is published on the host (`docs/spec/home-assistant.md:46-55`).
- **Reference instrumentation (for the bars, P5):** a respiratory belt, a chest
  strap with ECG-derived heart rate, an oximeter (redundant pulse cross-check
  only — no SpO₂, no HRV) and a pose reference (MM-Fi labels, or the research
  camera rig). These are a separate acquisition and gate §10.
- **A build host** with the repo and Nix (`nix develop .#esp`).

---

## 3. Day 0 — flash the first board

The firmware compiles in the repo dev shell; flashing needs the board.

1. **Sanity the board.** Plug into the UART port; confirm it enumerates
   (`dmesg | tail`, expect `/dev/ttyUSB0`). If it does not, it is the cable or
   the port — try the other USB-C port and a known-good data cable.

2. **Erase before the first flash.** The custom dual-OTA partition table
   (`partitions.csv`) differs from the ESP-IDF default, so clear the flash:
   ```sh
   nix develop .#esp --command bash -c \
     'cd firmware/esp32-s3-csi-node && idf.py -p /dev/ttyUSB0 erase-flash'
   ```

3. **Configure the factory defaults** (these are only defaults; the real
   provisioning path is NVS, §5):
   ```sh
   idf.py -p /dev/ttyUSB0 menuconfig   # "Aquila CSI node"
   ```
   Set: `AQUILA_WIFI_SSID` / `AQUILA_WIFI_PASSWORD` (the 2.4 GHz AP),
   `AQUILA_CORE_HOST` = the **HA host's LAN IPv4**, `AQUILA_CORE_DATA_PORT` 9100,
   `AQUILA_CORE_CONTROL_PORT` 9101, a unique `AQUILA_NODE_ID`, and a **non-zero**
   `AQUILA_PAIRING_CODE` (`0` disables join attempts,
   `main/Kconfig.projbuild:51-56`).

4. **Build and flash:**
   ```sh
   idf.py set-target esp32s3 && idf.py build
   idf.py -p /dev/ttyUSB0 flash monitor
   ```
   Expect `build/aquila_esp32s3_csi_node.bin` (~0.7 MB) plus the bootloader,
   partition table and `ota_data_initial.bin` (firmware README §"Build and
   flash"). `Ctrl+]` leaves the monitor. If flashing stalls, hold **BOOT**, tap
   **RESET**, release **BOOT** to enter download mode.

5. **Read the boot log.** Confirm the target, the join to WiFi, and the node
   starting to stream. Note the node id and the boot epoch it prints.

> **Do not distribute an image built with a baked-in WiFi password.** For the
> bench, menuconfig defaults are acceptable; for anything real, provision via
> NVS (`aquila-capture provision`, §5) so the credential is not in the image
> (G13, `capture-and-firmware.md:150-155`).

---

## 4. Wire verification — the first CSI at the core

This is the first real test of the assumptions in §0.1.

1. **Fix P1 first**, or nothing arrives. With the add-on ingesting on
   `0.0.0.0:9100`, the node's unicast datagrams land on the core.

2. **Capture a real session.** Use the dev tool (not the runtime) so the exact
   bytes are stored:
   ```sh
   nix develop --command cargo run -p aquila-capture -- record \
     --out /tmp/opencode/bench.aqs --bind 0.0.0.0 --port 9100 --frames 2000
   ```
   `record` stores **one exact received `AQCI` datagram per record**
   (`crates/aquila-capture/src/main.rs:10-16`), so replay runs the real decoder
   on what the node sent. Then `verify` the container.

3. **Confirm decode.** Check the source counters (`LiveStats`: `datagrams`,
   `decoded`, `bad_version`, `bad_frame`, `crates/aquila-sources/src/live.rs`).
   `decoded` climbing and `bad_frame` flat is the pass condition.

4. **Confirm the byte order and `first_word_invalid`** against the ESP-IDF on
   the board. This is the single most important hardware check: ADR-0006 fixes
   imaginary-first `[Q0,I0,…]`, but the `esp-csi` raw buffer layout has never
   been observed (`firmware/README.md` §"What is untested"). Feed a known tone
   or a hand-wave and check the derived phase sign.

5. **Freeze the null-bin map** (S11/F19). From this capture, derive the
   per-bandwidth subcarrier offset and nulled bins, and replace the provisional
   `-(n/2)` in `csi_node.c:224` and `aquila-simulator/src/source.rs:412` with a
   documented, frozen map (`signal.md:652`).

---

## 5. Pair and assign the node to a room

**Requires P2 and P4.** This is the least-built part of the path.

1. **Provision NVS** (the real credential path) or rely on menuconfig defaults
   for the bench. `aquila-capture provision` writes the JSON payload an operator
   flashes into the `aquila` NVS namespace (`wifi_ssid`, `wifi_pass`,
   `core_host`, `core_data_port`, `core_control_port`, `node_id`,
   `pairing_code`). The exact NVS write step is not yet pinned in the README
   (`capture-and-firmware.md:150-155`) — settle it here (a `parttool.py` /
   `nvs_partition_gen.py` CSV step), and record it.

2. **Pair.** The node sends `JOIN_REQUEST {node, boot, pairing_code, fw_version}`
   to the core; the core marks it pending, and on operator approval replies
   `JOIN_ACK {node, token, config}` (`capture-and-firmware.md:464-474`). The node
   stores the token in NVS. **The core half does not exist yet (P2)** — until it
   does, the node's control channel fails closed and only CSI flows.

3. **Assign the node to a room** in the topology (P4). Node ids must match the
   firmware `AQUILA_NODE_ID`, and the topology must be the one calibration is
   keyed to.

4. **Identify the board** physically (P6/F33): assign `IDENTIFY_GPIO` in
   `node_control.c:46` to a real GPIO so `IDENTIFY` blinks, and label the board.

---

## 6. First calibration

1. Open the dashboard (add-on **Open Web UI**, or the sidebar entry).
2. Define home → floor → room → node (P4).
3. Trigger a **global, whole-home calibration** pass: an empty-home baseline
   plus a labelled walk-through per room. Calibration is a versioned,
   commit-keyed artefact (`docs/INSTALL.md` §4).
4. Confirm the artefact is visible and versioned, and that
   `sensor.aquila_calibration_state` leaves `uncalibrated`. Until calibration
   has run, presence and vitals are `unavailable`/`unknown`, never a fabricated
   zero (`home-assistant.md` §6.1).

Note: the add-on's startup calibration is currently **synthetic**
(`calibrate_on_startup`, `crates/aquila-addon/src/main.rs:385-443`). Real
calibration from live captures is part of P1/P4.

---

## 7. Live validation — the first real sensing

With the node streaming, paired and calibrated, confirm the entities:

- `presence` per zone; `person count`; `breathing rate`; `heart rate`; node
  health (`nodes_online`/`nodes_total`).
- Sanity checks: wave an arm (motion), sit still and breathe (vitals), leave the
  room (presence drops). Watch for the honest `null` → `unavailable` behaviour
  when a reading is not available.
- Watch the logs for `bad_frame`, clock-drift warnings and dropped frames.

This is the point at which "a real node streams CSI and presence appears as an
HA entity" — ticket #8's end-to-end gate — is finally met.

---

## 8. OTA end to end

**Requires P3 and P2.** Once the store and a per-node transport are wired:

1. Sign an image with the project Ed25519 firmware key and import it into the
   store (`FirmwareStore`, `/data/aquila/firmware/`); the core verifies SHA-256
   + Ed25519 + target chip + compatibility before offering it.
2. Provision the node's trust anchor (NVS `ota`/`fw_pubkey`, or the compiled-in
   `AQUILA_FW_PUBKEY_HEX`). **With neither set the node fails every OTA closed**
   (firmware README).
3. Install from `update.aquila_<node>_firmware` and watch the chunks, the
   cumulative ACK, `OTA_END`, activation and reboot.
4. **Test the rollback deliberately:** push an image that never confirms, and
   confirm the bootloader rolls back and reports `OTA_RESULT rollback`.
5. Confirm anti-rollback rejects a downgrade below the security-version floor.

---

## 9. Measure the v1 bars

**Requires P5 and the reference devices.** This is the whole point.

- **Protocols** (`docs/spec/evaluation.md` §3): in-domain is the only scoped
  domain with a headline claim; LOSO, LORO, cross-configuration and
  leave-one-session-out are reported but ungated.
- **References:** breathing vs the respiratory belt; heart vs the chest strap;
  pose vs MM-Fi labels (or the research rig). Pair on the common clock, by key,
  never by nearest-neighbour.
- **Metrics:** torso PCK@20 (≥ 65%), presence F1 (≥ 0.90, per zone macro),
  breathing MAE (≤ 2 brpm), plus heart-rate MAE with its baseline-fixed bar and
  Bland-Altman for both vitals.
- **Ledger:** every result is a commit-keyed ledger row
  (`accuracy.torso_pck_at_20`, `accuracy.presence_f1`,
  `accuracy.breathing.mae_brpm`, `accuracy.heart_rate.mae_bpm`), with coverage
  and paired sample counts. Zero paired samples is *not measured*.
- **Then and only then** does a claim move off `target` in `docs/README.md` and
  `claims.toml`, and only to the grade the ledger supports.

---

## 10. Scale to the reference deployment

The whole-home claims (`presence.f1.home`, `count.mae.home`) need the reference
topology: **~9+ ESP32-S3 nodes over three floors** (`roadmap.md:247-264`,
`evaluation.md:287`). Per-node steps 3–7 repeat; then re-run calibration across
the full topology and collect LORO / cross-configuration splits. Budget for
spares and for channel discipline (one channel per floor or per home, held
constant for the whole corpus).

---

## 11. Definition of done (bring-up)

- [ ] P1–P4 closed; the add-on ingests live CSI, serves the control channel,
      constructs the OTA store, and loads a real topology.
- [ ] A node flashes, joins, streams, and decodes at the core with a flat
      `bad_frame` count.
- [ ] Byte order and `first_word_invalid` confirmed against the board; the
      null-bin map frozen from a real capture.
- [ ] The node pairs, is assigned a room, and calibrates; `calibration_state`
      leaves `uncalibrated`.
- [ ] Presence, count, breathing and heart rate appear as HA entities on real
      data.
- [ ] OTA installs, confirms after the health window, and rolls back on a bad
      image.
- [ ] P5 closed and the reference devices acquired; the three bars are measured
      and their ledger rows are written.

---

## 12. Risks and decision gates

| Risk | Why it matters | Mitigation |
| --- | --- | --- |
| **P1–P4 are more work than the boards** | The hardware is ready before the software is | Start P1/P4 now (hardware-free, testable via `VirtualNode`); treat them as the real critical path |
| **Board flash size** | Dual-OTA needs ≥ 4 MB | Check the module part number on arrival; adjust `sdkconfig.defaults` if it is an N4/N8 |
| **Channel/width drift** | CSI is only comparable on one channel | Pin the AP channel; record it per session in the ledger |
| **Byte-order surprise** | ADR-0006 is unverified on hardware | Verify first (§4.4); if the board differs, that is an ADR-level finding, not a patch |
| **Reference-device lead time** | Gates the bars (P5) | Order the belt/strap/oximeter now, in parallel with the boards |
| **UDP loss under load** | Hidden loss must never carry a gated result | Watch `LiveStats`; a lossy session is graded, not gated (G7) |
| **Doc staleness** | OTA banners still say "not implemented" | Correct `firmware-ota.md:5-15` and `capture-and-firmware.md:449-453` (P6) |

**Decision gates.** If the null-bin map cannot be confirmed from the first
capture, keep the PROVISIONAL label visible and do not gate any claim on it. If
the byte order differs from ADR-0006, stop and write an ADR superseding it before
any measurement. If the reference devices are not available, measure only
presence/count against your own observation and mark pose/vitals *hardware-gated*
— never quote an unmeasured number.

---

## Appendix — quick command card

```sh
# Compile (no hardware)
nix develop .#esp --command bash -c \
  'cd firmware/esp32-s3-csi-node && idf.py set-target esp32s3 && idf.py build'

# Flash and monitor (needs a board on /dev/ttyUSB0)
nix develop .#esp --command bash -c \
  'cd firmware/esp32-s3-csi-node && idf.py -p /dev/ttyUSB0 erase-flash flash monitor'

# Capture a real CSI session to a .aqs fixture
nix develop --command cargo run -p aquila-capture -- record \
  --out /tmp/opencode/bench.aqs --port 9100 --frames 2000

# Regenerate the firmware wire header after a wire change
nix-shell --run 'cargo test -p aquila-capture emit_node_wire_header -- --ignored'

# Everything (hardware-free) still green
nix develop --command cargo test --workspace
nix develop .#ha --command bash -c 'cd custom_components/aquila && python3 -m pytest -q'
```
