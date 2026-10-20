# Sister-issues audit — firmware, HACS component, add-on packaging

Defects of the same **class** as finding #36
(`docs/plan/orchestrator-omissions.md` addendum): a canonical representation
contradicted the hardware, so the node faked `f32` from `int8`; the wire was
wrong; the derivation was untested.

- **Scope audited:** `firmware/` (ESP-IDF C), `custom_components/aquila/`
  (Python HACS component), `aquila_core/` (add-on packaging), `repository.yaml`,
  `hacs.json`, and the specs `docs/spec/firmware-ota.md`,
  `docs/spec/home-assistant.md`, `docs/spec/capture-and-firmware.md` §9,
  `docs/spec/privacy-security.md`; cross-checked against `data-contracts.md`,
  `signal.md`, `core-api.md`, `interface-register.md`, ADR-0004/0005, and the
  Rust core where a contract crosses the boundary.
- **Method:** read-only. `rg`/`read` for all sources; `nix develop .#ha --
  python3 -m py_compile custom_components/aquila/*.py` for the Python (passes —
  syntax valid, runtime never exercised). Nothing was modified except this file.
- **Class taxonomy:** (1) code contradicts spec; (2) on-node/wrong-layer work;
  (3) representation/units mismatch; (4) security/privacy violation of G1–G16;
  (5) fake/stub presented as real; (6) untested claims; (7) spec gaps.
- **Labels:** **[DEFECT]** = definitely a defect, decidable from the sources.
  **[GATED]** = genuinely hardware-gated (cannot be closed without a board),
  acceptable only if the limitation stays labelled. A finding may carry both:
  e.g. a contract contradiction that is *decidable now* but whose final symptom
  only appears on hardware.

The single most important result is **F17**, a second instance of the #36 class.

---

## Summary

### Counts per class

A finding is counted under its primary class; many also touch a secondary class
noted inline.

| Class | Count | Findings |
| --- | --- | --- |
| 1 — Code contradicts its spec | 14 | F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, F15, F16 |
| 2 — On-node / wrong-layer work | 2 | F13, F14 |
| 3 — Representation / units mismatch | 4 | F17, F18, F19, F20 |
| 4 — Security / privacy (G1–G16) | 6 | F21, F22, F23, F24, F25, F26 |
| 5 — Fake / stub presented as real | 7 | F27, F28, F29, F30, F31, F32, F33 |
| 6 — Untested claims | 4 | F34, F35, F36, F37 |
| 7 — Spec gaps | 2 | F38, F39 |

(`F17` is the #36-class finding; `F3` is the highest-impact cross-boundary
defect.)

### Top 10 by severity

| Rank | ID | Severity | One line |
| --- | --- | --- | --- |
| 1 | F17 | **critical** | Canonical I/Q byte order contradicts the hardware: contract freezes `I,Q`; esp-csi emits `imag,real`; firmware copies verbatim — the #36 class again. |
| 2 | F3 | **critical** | HA component is built on `/api/v1/nodes`, `/api/v1/firmware*` and a working WS stream that the core does not serve (WS returns 501; everything else 404). |
| 3 | F21 | **critical** | Node control channel is unauthenticated: the pairing token is stored but never checked; any LAN host can `SET_CONFIG`/`REBOOT`/`FACTORY_RESET`. |
| 4 | F22 | **critical** | CSI ingest is unsealed and unauthenticated, contradicting G3/G7's per-node AEAD guarantee. |
| 5 | F29/F39 | **critical** | OTA is specified in detail but entirely unimplemented: no opcodes, no `esp_ota_*`, no A/B partition table, no Ed25519 verify, no anti-rollback. |
| 6 | F4 | **high** | `node_control.c` reads a non-existent struct field (`config->csi.heartbeat_ms`) — the firmware cannot compile, proving it was never built. |
| 7 | F1/F23 | **high** | WiFi credentials come from Kconfig and are baked into the image, contradicting the NVS-provisioning spec and G13. |
| 8 | F24 | **high** | Add-on ships `retention_days: 30` — an imposed default retention, contradicting G9 ("the default is to retain"). |
| 9 | F18 | **high** | `boot` is a random `u32` on the control plane but truncated to `u16` on the wire; control and data planes disagree on node identity. |
| 10 | F27 | **high** | The add-on Dockerfile is an unrunnable placeholder with no `ENTRYPOINT`; the "add-on" cannot start. |

### Looks like a second #36?

**Yes — F17.** It is the precise shape of #36: a *canonical representation was
frozen without checking it against what the hardware produces*, the wire form
follows the frozen (wrong) choice, and the derivation is untested because the C
side is never run and the simulator emits the frozen order.

- `data-contracts.md:206` and `ADR-0005:41-46` freeze `i_q` as
  **"interleaved `I, Q`"** (`[I0, Q0, I1, Q1, …]`, real first).
- `signal.md:221-224` states the opposite about the *same hardware bytes*:
  **"interleaved `int8` I/Q pairs, imaginary byte first per Espressif
  `wifi_csi_info_t`"**, and `signal.md:637-639` flags the byte order as an
  **open, unresolved** question ("must be confirmed against a real capture before
  `decode_csi` is frozen").
- The firmware copies the esp-csi buffer **verbatim** (`csi_node.c:145`,
  `node_wire_util.c:119`), so on the wire the bytes are **imag-first** (per
  `signal.md:221`), i.e. **`[Q0, I0, …]`** — the opposite of the frozen
  contract.
- The simulator emits **real first** (`source.rs:317-318` pushes `re` then
  `im`), so the core's `decode_csi` (`signal.md:216`, "interleaved `I, Q`") is
  correct for the simulator and **wrong (phase-conjugated) for live CSI**:
  `phase = atan2(Q, I)` becomes `atan2(I, Q) = -phase`.

Amplitude is unaffected (`I²+Q²` is symmetric), which is exactly why the bug is
invisible in an amplitude-only test — the same way #36's `sqrtf` was invisible
until the phase mattered. Any phase-based vital (breathing/heart via phase) is
wrong on hardware, and nothing exercises it. `decode_record` vs `decode_csi` in
`signal.md` currently assume **opposite byte orders for the same bytes**: an
internal contradiction in one file.

---

## Findings by class

### Class 1 — Code contradicts its spec

#### F1 — WiFi credentials are compile-time Kconfig constants, not NVS — **[DEFECT]** (also class 4, G13)
`firmware/esp32-s3-csi-node/main/main.c:68-71`
```c
strncpy((char *)wifi_config.sta.ssid, CONFIG_AQUILA_WIFI_SSID, ...);
strncpy((char *)wifi_config.sta.password, CONFIG_AQUILA_WIFI_PASSWORD, ...);
```
`capture-and-firmware.md` §9.1.8 ("Persist config, token and credentials in
NVS; load and apply at boot") and §8.3 (`provision` writes NVS network
credentials) require provisioning through NVS. The firmware instead reads the
SSID/password from `Kconfig.projbuild` at compile time and bakes the WiFi
password into the image — contradicting the provisioning design and G13
("Secrets … never baked into images"). This is **[GATED]** only in that a real
provisioning path needs hardware to test; the *design* contradiction is
decidable now.
**Fix:** read network credentials from NVS (written by `aquila-capture
provision`); drop the password from Kconfig.

#### F2 — The control opcode table omits the OTA extension — **[DEFECT]** (also class 5/7)
`firmware/esp32-s3-csi-node/components/node_wire/include/node_wire.h:38-51`
defines opcodes `0x01`–`0x0D` only. `capture-and-firmware.md` §6.2 and
`firmware-ota.md` §5 add `0x0E`–`0x14` (`OTA_BEGIN … OTA_RESULT`), and
`capture-and-firmware.md` §9.5 says the node implements them. The generated
header (from `aquila-wire::generate_c_header()`) has no OTA opcodes, so the
firmware cannot speak OTA even if handlers existed.
**Fix:** extend the opcode table in `aquila-wire` and regenerate
`node_wire.h`; then implement handlers (see F29/F39).

#### F3 — The HA component's contract is not served by the core — **[DEFECT]**
`crates/aquila-api/src/server.rs:152-203` serves only `GET /api/v1/live`,
`/api/v1/health`, `/api/v1/state`, and `/api/v1/stream`, and the WS upgrade
returns **501** (`server.rs:169-180`, "WebSocket upgrade is not implemented").
The component depends on a **working** stream (`manifest.json` `iot_class:
local_push`; `stream.py:125-153`) and calls `GET /api/v1/nodes`
(`api.py:196-198`), `GET /api/v1/schema` (`api.py:188-190`), `GET
/api/v1/firmware` (`api.py:200-202`) and the per-node firmware routes
(`api.py:204-231`) — all **404** in the core. Consequences: the WS client
reconnect-loops forever against a 501; `update.aquila_<node>_firmware` never
gets node data; `aquila.install_firmware` cannot work. `home-assistant.md` §5.2
is the binding contract for the component, and the core does not implement it.
**Fix:** implement the listed routes + the RFC 6455 `aquila.v1` stream in
`aquila-api`, or narrow `home-assistant.md` §5.2 and the component to the
routes the core actually serves (and drop `local_push`).

#### F4 — `node_control.c` reads a field that does not exist (compile error) — **[DEFECT]** (also class 5)
`firmware/esp32-s3-csi-node/components/node_control/node_control.c:348`
```c
s_heartbeat_ms = config->csi.heartbeat_ms > 0u ? config->csi.heartbeat_ms : 5000u;
```
`csi_node_config_t` (`csi_node.h:20-32`) has no `heartbeat_ms` member, and
`node_control_config_t` (`node_control.h:22-30`) does not add one. `main.c`
never sets it. The file cannot compile. The spec puts `heartbeat_ms` in both the
SET_CONFIG body (`capture-and-firmware.md:412-413`) and the NVS key set
(`:623`), so the field is specified but absent from the struct.
**Fix:** add `uint32_t heartbeat_ms` to `csi_node_config_t` (or
`node_control_config_t`) and set it in `main.c`/`csi_node_default_config`.

#### F5 — `secondary` is accepted and stored but never applied — **[DEFECT]**
`firmware/esp32-s3-csi-node/components/csi_node/csi_node.c:319-321`
```c
if (config->channel != 0u) {
    esp_wifi_set_channel(config->channel, WIFI_SECOND_CHAN_NONE);
}
```
`secondary` is a config field (`csi_node.h:25`), arrives in `SET_CONFIG`
(`node_control.c:217`) and is spec'd (`capture-and-firmware.md:412`), but the
channel is always set with `WIFI_SECOND_CHAN_NONE`. A config value is silently
ignored.
**Fix:** map `secondary` (0/1/2) to `WIFI_SECOND_CHAN_NONE`/`_ABOVE`/`_BELOW`
and pass it to `esp_wifi_set_channel`.

#### F6 — `GET_CONFIG` returns a version string, not the configuration — **[DEFECT]** (also class 5)
`firmware/esp32-s3-csi-node/components/node_control/node_control.c:262-267`
replies to `GET_CONFIG` with `send_result(..., "config_version=%u")`.
`capture-and-firmware.md` §6.2 defines `GET_CONFIG` as "read current config";
only `config_version` is returned. The caller cannot read channel/bandwidth/
caps.
**Fix:** serialise the `csi_node_config_t` fields into the RESULT body.

#### F7 — `config_hash` is carried but never verified — **[DEFECT]** (also class 5/7)
`node_control.c:200-234` reads `version u32 | hash[32] | …` but never uses the
32-byte hash. `capture-and-firmware.md:409-411`: "`SET_CONFIG` carries a
`config_version` and a `config_hash`; the node validates". A corrupted or
spoofed config is applied unchecked.
**Fix:** recompute the config hash and reject on mismatch before applying.

#### F8 — Control request deduplication is required but absent — **[DEFECT]** (also class 7)
`capture-and-firmware.md:377-379`: "Every request is idempotent and keyed by
`(node, request_id)`; a retransmit is deduplicated by the node within a bounded
window." `node_control.c:236-310` has no `request_id` cache; duplicate
`SET_CONFIG`/`SET_CSI` are applied repeatedly.
**Fix:** keep a bounded ring of recent `request_id`s and drop duplicates.

#### F9 — NVS persistence is a subset of what §9.3 requires — **[DEFECT]** (also class 7)
`capture-and-firmware.md:623` lists NVS keys "network credentials, core
host/ports, pairing token, CSI config, `max_tx_rate_hz`, `heartbeat_ms`".
`node_control.c:56-93` persists only `paired`, `token`, `config_version`,
`heartbeat_ms`. Network credentials, core endpoint, and the CSI config are not
persisted; a reboot loses them (and `main.c` re-seeds CSI from defaults).
**Fix:** persist and reload the full provisioning set; reconcile with F1.

#### F10 — `phy_flags` maps HE/VHT to the "HT" bit and never sets the NDP bit — **[DEFECT]** (low)
`csi_node.c:82-101`: `if (rx->sig_mode >= 1u) flags |= 1u<<0; /* HT */` labels
VHT/HE frames as HT, and bit 5 "sounding/NDP" (`data-contracts.md:218-219`) is
never set.
**Fix:** decode `sig_mode` into the correct PHY family; set bit 5 when
applicable.

#### F11 — ADR-0004's payload consequence text still contradicts ADR-0005 — **[DEFECT]** (low)
`docs/adr/0004-on-air-csi-layout.md:58`: "the firmware must emit `f32`
amplitude/phase, not int8 I/Q." The file has a prominent "superseded in part"
banner, so an expert is not misled, but the stale line is the exact #36
instruction. ADRs are immutable-once-accepted; the banner is the mitigation.
**Fix:** leave ADR-0004 immutable, but ensure the capture spec's §5.1 revision
note and the register both point at ADR-0005 (they do) so no reader lands on
the stale line alone.

#### F12 — `capture-and-firmware.md` §5.1 and the control spec are internally consistent post-0005, but the firmware README over-claims — **[DEFECT]** (low)
`firmware/esp32-s3-csi-node/README.md:56-67` claims the committed header is
checked by CI "so the two languages cannot silently drift". The CI test
(`crates/aquila-capture/src/main.rs:182-191`) compares the **header text** only;
it does not compile or run the C framing, so `node_wire_util.c` can drift from
the generated layout unchecked (see F37).
**Fix:** add a shared golden-vector test that runs the C framing (host build or
QEMU) against the Rust encoder, or soften the claim.

---

#### F15 — Captured metadata fields are dead and misdescribed — **[DEFECT]** (low)
`firmware/esp32-s3-csi-node/components/csi_node/csi_node.c:138-139`
```c
slot->bandwidth_mhz = 20u; /* refined from config below at send time */
slot->secondary = info->rx_ctrl.secondary_channel;
```
The comment lies: the sender never refines or reads `slot.bandwidth_mhz` (it
uses `cfg.bandwidth_mhz` at `:230`), and `slot.secondary` is captured but the
AQCI layout (`data-contracts.md` §9.1) has no `secondary` field, so the config's
`secondary` never leaves the node. Dead fields plus a misleading comment.
**Fix:** delete the unused fields, or transmit `secondary` if the contract is
meant to carry it (see F5).

#### F16 — `csi_len_cap` units are ambiguous (raw bytes vs AQCI payload bytes) — **[DEFECT]**
`csi_node.h:30` documents `csi_len_cap` as "maximum CSI payload in bytes";
`csi_node_default_config` seeds it with `AQ_CSI_MAX_DATAGRAM_LEN` (1472,
`csi_node.c:253`), which is the *whole-datagram* cap, while the callback
compares it against the **raw receive buffer length** (`csi_node.c:118`, `len`),
not the AQCI payload (`28 + 2n`). The default is then silently clamped to 1428
at `:267-268`. The field means three different things (raw bytes, payload
bytes, datagram bytes) in three places, and the default value is inconsistent
with the documented unit.
**Fix:** define one unit (raw int8 bytes), derive the datagram/payload bounds
from it explicitly, and set the default consistently.

---

### Class 2 — On-node processing / wrong-layer work

#### F13 — The component decides "latest firmware version" — **[DEFECT]** (low)
`custom_components/aquila/update.py:141-153` `_manifest_latest()` takes
`versions[-1]` from the manifest as the newest. `home-assistant.md` §5.2 /
`firmware-ota.md` §7 make the core the owner of `latest_version` and the store
manifest; picking "latest" by list order in the presentation layer is a
policy decision the core owns, and it is order-dependent (not version-sorted).
**Fix:** serve `latest_version` per node from the core; the component should
present, not rank.

#### F14 — (Positive) On-node DSP/FP is now removed — no defect, recorded for completeness
`csi_node.c:145` copies the receive buffer; `node_wire_util.c:114-120` copies
`2n` bytes with no `math.h`. The `sqrtf`/`atan2f` of #36 are gone; amplitude/
phase derivation lives in `aquila-types`/`aquila-signal`. The "dumb sensor"
rule §9.2 is satisfied **in intent** — but see F17, where the *order* of those
bytes is still contradicted by the contract.

---

### Class 3 — Representation / units mismatch

#### F17 — **Canonical I/Q byte order contradicts the hardware — the #36 class** — **[DEFECT] + [GATED] for confirmation**
See the "second #36" section above for the full argument. Concretely:
- `docs/spec/data-contracts.md:206`, `docs/adr/0005-...:41-46` — `i_q` is
  "interleaved `I, Q`" (`[I0, Q0, …]`).
- `docs/spec/signal.md:221-224` — the hardware is "imaginary byte first per
  Espressif"; `signal.md:637-639` lists the order as an **unresolved open
  question**.
- `firmware/.../csi_node.c:145` + `node_wire_util.c:119` — the firmware copies
  the buffer **verbatim**, so it emits `[Q0, I0, …]`.
- `crates/aquila-simulator/src/source.rs:317-318` — the simulator emits
  `[I0, Q0, …]`.
- `docs/spec/signal.md:216` (`decode_csi`, "interleaved `I, Q`") vs
  `signal.md:221` (`decode_record`, "imaginary byte first") — **opposite
  assumptions about the same bytes in one file**.

**Why it is a defect:** the frozen contract (and the simulator) pick one order
while the hardware and the firmware's verbatim copy pick the other. On live
hardware, phase is conjugated relative to the pipeline; amplitude tests never
notice. This is exactly #36's mechanism — a representation frozen without
checking the hardware, wire and derivation untested.
**Severity:** critical.
**Fix (decidable now):** choose one order as canonical, state it once in
`data-contracts.md` §2, and make **all four** agree: the contract, the
simulator, `decode_csi`/`decode_record`, and the firmware's byte-emission (the
firmware may reorder the pair cheaply, or the core may swap on decode if the
contract keeps I-first). Add a fixture captured from a real ESP32-S3 that pins
the order, and a phase-based end-to-end test so an order flip cannot pass on
amplitude alone. Until then, keep `signal.md` §7's open question visible and
mark the live path untested. **[GATED]** only for the final confirmation that
the ESP-IDF version on the board is imag-first.

#### F18 — `boot` is `u32` on the control plane, truncated to `u16` on the wire — **[DEFECT]**
`main.c:98` sets `csi.boot = esp_random()` (a `u32`); `node_control.c:144,165`
send the full `u32` in `JOIN_REQUEST`/`STATUS`; `csi_node.c:231` casts
`(uint16_t)s_boot_epoch` into the AQCI frame. `data-contracts.md:199` says
`boot_epoch` is `u16` "incremented every boot"; `capture-and-firmware.md:591`
says "`boot` = random `u32` at boot". The core pairs/identifies a node by one
value and resets epoch state on another; the two planes disagree on the same
concept, and two random `u32`s can share a `u16` low half.
**Severity:** high.
**Fix:** one width and one rule. Either make `boot_epoch` a `u16` everywhere
(matching the wire) or widen the wire field; reconcile `data-contracts.md:199`
(random vs increment) with `capture-and-firmware.md:591`.

#### F19 — `subcarrier_offset = -(n/2)` is an unverified hardware assumption — **[GATED]**
`csi_node.c:228`: `int16_t subcarrier_offset = (int16_t)(-(int16_t)(n / 2u));`
`data-contracts.md:205` defines the field as the index of the first sample.
Whether `n` (from `info->len/2`) is a contiguous, symmetric bin set starting at
`-n/2` depends on the chip/bandwidth (guard/DC nulls). `signal.md:637-639`
flags the HT20/HT40 null-bin map as unresolved. The firmware asserts a
convention the project has not pinned.
**Fix:** derive `subcarrier_offset` from a documented per-bandwidth bin map;
verify on hardware. Acceptable as **[GATED]** only while labelled (the README
does not currently label this one).

#### F20 — Band mapping is a hardcoded channel boundary — **[GATED]** (low)
`csi_node.c:76-79`: `band_from_channel` returns 0 for `channel <= 14`, else 1.
`data-contracts.md`/`capture-and-firmware.md` also allow `band = 255`
(unknown), never produced. Minor; needs hardware to confirm 5 GHz channel ranges
in `rx_ctrl.channel`.
**Fix:** map explicitly per the driver's channel set; emit 255 when unknown.

---

### Class 4 — Security / privacy violations of G1–G16

#### F21 — The node's control channel is unauthenticated — **[DEFECT]**
`capture-and-firmware.md` §6.3: "The node stores the token in NVS and is
paired; subsequent control frames authenticate with it." §6.2 and
`privacy-security.md` G7 ("Every … control command is authenticated with the
per-node key … mutually authenticated, pinned"). The firmware stores the token
(`node_control.c:43,192`) and **never uses it**: `handle_message`
(`node_control.c:236-256`) checks only magic and version. There is no HMAC,
no key, no sequence/replay check, and `recvfrom` accepts datagrams from any
source. Any LAN host can `SET_CONFIG`, `SET_CSI`, `REBOOT` and `FACTORY_RESET`
a node.
**Severity:** critical.
**Fix:** authenticate every inbound control message with the per-node key
(HMAC/AEAD over the frame), pin the core's address, and reject unauthenticated
or replayed commands. Until implemented, the guarantee must be marked
unimplemented, not asserted.

#### F22 — CSI ingest is neither sealed nor authenticated — **[DEFECT]** (contract) / **[GATED]** (implementation)
`privacy-security.md` G3(c) ("the CSI payload framing is AEAD-sealed with the
per-node key") and G7 ("AEAD (sealed CSI payload) plus a monotonic per-node
sequence"). The firmware sends the raw int8 I/Q in **plaintext UDP**
(`csi_node.c:234-236`), with no key and no authentication; sequence numbers
exist but are unauthenticated (`data-contracts.md` §2.5). No AEAD appears
anywhere in `firmware/`, `custom_components/`, or `aquila_core/`. Note the spec
is itself inconsistent: G3/G7 assert sealing as *enforced*, while
`privacy-security.md:437` says "**If** AEAD sealing of CSI payloads is deferred
past v1 …" — a conditional that contradicts the guarantee's unconditional
statement.
**Severity:** critical.
**Fix:** either implement per-node AEAD sealing on the wire (node and core) or
honestly downgrade G3/G7 to "deferred; confidentiality relies on WLAN layer-2
encryption" and fix `privacy-security.md:437` to match. The current text claims
a protection that does not exist.

#### F23 — The WiFi password is baked into the firmware image — **[DEFECT]** (G13)
`main.c:70` copies `CONFIG_AQUILA_WIFI_PASSWORD` (a Kconfig string) into the
station config, so the network password is compiled into the binary. G13: "Per-
node keys and any HA token live in Supervisor-managed add-on options, never
baked into images." (See F1.)
**Severity:** high.
**Fix:** provision credentials into NVS at flash time; keep secrets out of
`Kconfig.projbuild` and the image.

#### F24 — The add-on ships an imposed default retention — **[DEFECT]** (G9)
`aquila_core/config.yaml:27,31` and `home-assistant.md:94,98`:
```yaml
options:
  retention_days: 30
schema:
  retention_days: int(1,365)
```
G9: "Aquila never deletes or expires user data on its own. There is no default
TTL … the default is to retain." A default of 30 days is an imposed retention
window — the exact behaviour G9 forbids. The spec itself embeds the
contradiction (`home-assistant.md` §3.1 vs `privacy-security.md` G9).
**Severity:** high.
**Fix:** remove the default (retain indefinitely unless the user opts in), or
change the default to "retain" and make the option explicitly opt-in; reconcile
the two specs.

#### F25 — G12 (CSP sandbox) and G11 (read-only rootfs) are not implemented anywhere — **[DEFECT]** (also class 5/7)
G12 requires the dashboard be served "under a strict Content Security Policy
… `default-src 'none'; …`". No CSP is configured in `aquila_core/` or the
component; the only server is the placeholder Dockerfile (F27). G11 requires a
"read-only root filesystem"; `config.yaml` does not (and cannot) express it, and
the Dockerfile is a placeholder. Neither guarantee has an implementation to test.
**Severity:** high.
**Fix:** implement the add-on server with the CSP header on dashboard
responses; document/enforce the least-privilege runtime flags in the real
Dockerfile; until then mark G11/G12 unimplemented.

#### F26 — Default-off auth on the private API — **[GATED/acceptable if labelled]**
`home-assistant.md:296-299` and `api.py:135-144` default to no token because
8099 is not host-published. This is honest and consistent with the publishing
model (config.yaml does not publish 8099; only 9100/9101 are mapped).
**No change required**, but it is only safe while the port stays unpublished;
if `ports` ever gains 8099, G4/G5 break. Record as a labelled dependency, not a
defect.

---

### Class 5 — Fake / stub presented as real

#### F27 — The add-on Dockerfile is an unrunnable placeholder — **[DEFECT]** (labelled)
`aquila_core/Dockerfile:3-11` labels itself "PLACEHOLDER — NOT A VERIFIED
BUILD"; every real step is a `RUN echo "TODO …"` (`:46,:60`), and `:84` states
"Intentionally left with no ENTRYPOINT: this placeholder is not runnable." The
add-on — the deliverable that "runs the Rust core natively and serves the
dashboard" (`home-assistant.md` §1) — cannot start. It is honestly labelled, so
this is **[GATED]** in the sense that a build needs the heavy Rust/ONNX
toolchain, but the *packaging deliverable is absent*, and it names crates
(`aquila-addon`, `aquila-api`, `aquila-dashboard`) that are partly missing from
the workspace (no `crates/aquila-dashboard`).
**Fix:** real multi-stage build with `ENTRYPOINT`; or mark the add-on
"unreleased" in `repository.yaml`/`config.yaml` and stop advertising a
`ghcr.io/belikh/aquila-core-*` image that does not exist.

#### F28 — OTA opcodes/endpoints exist in docs and the component but nowhere else — **[DEFECT]** (see F2, F3, F29)
`firmware-ota.md` §7 and `home-assistant.md` §5.2 define the `/api/v1/firmware*`
routes and the `update` entity; the component implements the client
(`api.py:200-231`, `update.py`) against a core that 404s them. The OTA
`update` entity and `aquila.install_firmware` service therefore present a
functional surface that cannot work.
**Fix:** implement the core routes + OTA manager, or remove the surface and mark
OTA unreleased.

#### F29 — No OTA implementation in firmware (opcodes, partition API, verification, rollback) — **[DEFECT]**
`rg -n "ota|esp_ota|partition" firmware/` finds **nothing** (the only hits are
the substring "total"). There is no `esp_ota_begin`/`esp_ota_write`/
`esp_ota_set_boot_partition`, no `esp_ota_mark_app_valid_cancel_rollback`, no
Ed25519/SHA-256 verification, no anti-rollback floor, and **no partition table**
(`find firmware -iname '*partition*'` is empty). `firmware-ota.md` §6 requires
all of these. The dual-partition A/B rollback the spec depends on is
impossible with the default single-factory partition table.
**Severity:** critical.
**Fix:** add `partitions.csv` with two OTA app slots + `otadata`; implement the
`0x0E`–`0x14` handlers with chunk CRC-32, SHA-256 + Ed25519 verify against a
provisioned public key, `esp_ota_set_boot_partition`, pending-verify and
rollback, and an NVS/eFuse anti-rollback floor.

#### F30 — Panel asset and brand icon are referenced but absent — **[DEFECT]**
`home-assistant.md` §3.2 lists `brand/icon.png`; the repo has only
`custom_components/aquila/brand/README.md`. `__init__.py:199-208` registers a
panel with `webcomponent_path="aquila-panel.js"`, which is not shipped anywhere.
The panel silently fails to render (caught and warned at
`__init__.py:209-211`).
**Fix:** ship both assets, or drop the panel registration and the spec-tree
entry until the dashboard work lands.

#### F31 — `STATUS` reports a hardcoded temperature of 0 — **[DEFECT]** (low)
`node_control.c:173`: `aq_put_i16_le(&body[offset], 0); /* temperature,
unimplemented here */`. `capture-and-firmware.md` §6.4 lists "temperature" as a
heartbeat field. It is a stub value masquerading as a reading.
**Fix:** read the on-die temperature sensor (`temperature_sensor` driver) or
reject/omit the field; do not report `0`.

#### F32 — `STATUS` reports a hardcoded last-RTT of 0 — **[DEFECT]** (low)
`node_control.c:45` declares `s_last_rtt_us`; it is initialised to 0
(`:351`) and never written, yet reported as "last measured core RTT"
(`:177`). `SYNC_PING` computes `t1/t2/t3` (`:280-291`) but never stores an RTT.
**Fix:** compute and store the RTT on `SYNC_PING`/`SYNC_ECHO`, or mark the
field unset.

#### F33 — `IDENTIFY` does not blink an LED — **[DEFECT]** (labelled)
`node_control.c:34` `#define IDENTIFY_GPIO (-1)`; the `IDENTIFY` handler
(`:298-300`) only replies. `capture-and-firmware.md` §9.1.9: "IDENTIFY blinks
an LED." Labelled in a comment and in the README's "untested" list.
**Fix:** drive a real GPIO (or an on-board LED) when configured.

---

### Class 6 — Untested claims

#### F34 — The entire HA component is untested — **[GATED]** (labelled)
`custom_components/aquila/*.py` passes `py_compile` (syntax valid) but there is
no test module and no HA runtime in-tree; config flow, coordinator, WS, entities
and OTA were never exercised (AGENTS.md's `nix develop .#ha` provides HA, but no
tests exist). The omissions audit already records this (#5). Delivery claims
like "the flow validates `/api/v1/health` and creates the entry"
(`home-assistant.md` §9) are unverified.
**Fix:** add `pytest`/`pytest-homeassistant-custom-component` tests for the
config flow, coordinator merge, seq handling and entity availability; run them
in the `.#ha` shell in CI.

#### F35 — The entire firmware is untested — **[GATED]** (labelled)
`firmware/esp32-s3-csi-node/README.md:6-10` states it plainly: "never been
built … never flashed … Treat it as a starting point." This is honest. The
defect is not the untested status but the places where untested code is
nonetheless written as if verified (F4 proves it cannot compile; F1/F21/F29 are
spec-contradicting).
**Fix:** add a host-build smoke check (compile the components with a stub ESP-IDF
shim) so at least compile errors like F4 are caught; keep the hardware-gated
label.

#### F36 — OTA has no test and no fixture — **[GATED]**
`firmware-ota.md` §9: "Deterministic from a fixture (a simulated node OTA)."
No such fixture, simulator node OTA, or harness exists.
**Fix:** extend the virtual live node (`aquila-testkit`) to implement the OTA
opcodes against an in-memory partition fake and assert a successful transfer, a
CRC failure, a bad signature and a rollback.

#### F37 — The "one layout, two languages" check covers only the header text — **[DEFECT]**
`crates/aquila-capture/src/main.rs:182-191`
(`committed_node_wire_header_is_current`) asserts the committed C header equals
`generate_c_header()`. It never compiles or runs `node_wire_util.c`, so the C
framing (`aq_frame_csi`, `aq_put_*`) can drift from the layout the header
declares. `README.md:64-67` and `capture-and-firmware.md` §9.4 claim the two
ends "cannot silently drift"; the check is narrower than the claim.
**Fix:** add a golden-vector test that feeds fixed inputs through the C framing
and compares bytes with the Rust encoder (a host build of the pure C helpers is
feasible without hardware).

---

### Class 7 — Spec gaps (required behaviour specified but unimplemented/unimplementable)

#### F38 — Anti-rollback, Ed25519 verify and A/B partitions are specified, unimplemented, and partly unimplementable as written — **[DEFECT]**
`firmware-ota.md` §4-6 and `capture-and-firmware.md` §9.5 require: an Ed25519
public key "provisioned into the node at flash time", a monotonic firmware
security version in "NVS/eFuse", and dual-partition A/B with rollback. None is
present (F29); there is no partition table, and no key provisioning path. The
"public key … provisioned at flash time" has no corresponding provisioning
mechanism in `aquila-capture provision` (which the spec says writes "NVS
(network credentials, core endpoint)", `capture-and-firmware.md:544`).
**Fix:** specify and implement key provisioning and the partition layout; or
mark OTA out of v1 scope and strike the guarantee.

#### F39 — G5 ingress-identity validation has no implementation in the management server — **[DEFECT]** (also class 4)
`privacy-security.md` G5: "Validate `X-Remote-User-Id` (and related ingress
headers) on every request; reject when absent … WebSocket upgrades are
same-origin and checked." The only management server in-tree
(`crates/aquila-api/src/server.rs`) performs **no** header validation and
serves `/api/v1/state` unauthenticated; the WS upgrade is a 501. On a host where
the port is reachable from another add-on, the "private" API is open. Combined
with F26 (no token by default), the management surface has no enforcing
control.
**Fix:** validate Supervisor ingress headers on every request and the WS
upgrade in the real add-on server; reject off-ingress requests.

---

## Notes / non-findings (to avoid overcounting)

- **F14** records that the #36 on-node floating-point defect is actually fixed
  by ADR-0005 — this is a *positive* result, not a finding.
- **F26** (default-off API token) and the honest firmware README/placeholder
  labels are examples of *good* labelling; they are recorded only where an
  underlying deliverable is absent.
- The omissions audit's #6 (`aquila-panel.js` missing) and #5 (HA untested) are
  re-encountered here as F30 and F34; they are counted once in the class totals
  and flagged as duplicates of known tickets.
- Cross-references to the sibling audit: F3/F28/F29 (core routes and OTA) and
  F38 overlap the "declared-but-absent artefacts" root cause in
  `orchestrator-omissions.md` §4; F17 is the standalone #36 twin.

## Highest-value fixes (ordered)

1. **F17** — pick and pin the I/Q byte order once, in `data-contracts.md` §2,
   and make the contract, simulator, decoders and firmware agree; add a
   hardware capture and a phase-based end-to-end test. This closes the second
   #36.
2. **F3** — make the core serve the component's contract (or trim both). Every
   HA-facing feature is dead until this lands.
3. **F21/F22** — implement per-node control authentication and CSI seal, or
   downgrade G3/G7 honestly. Do not ship a spec that claims AEAD that does not
   exist.
4. **F4/F1/F29** — make the firmware compile, provision secrets through NVS, and
   either implement OTA (with a partition table) or strike it from v1.
5. **F24** — remove the imposed 30-day retention default; reconcile
   `home-assistant.md` §3.1 with G9.
