# Firmware update (OTA) — specification

Status: draft for review (v1.1). Prose in Australian English.

Scope: how an ESP32-S3 node's firmware is updated over the private LAN from the
Aquila core, triggered and observed from Home Assistant, with no cloud. Subordinate
to `CONTEXT.md` and to `docs/adr/0003-interface-reconciliation.md`; interfaces
become binding when ratified by an ADR (see §10).

## 1. Goals and non-goals

Goals:

- Update node firmware from the core over the existing private LAN. **No cloud, no
  internet, no vendor update server.**
- Observable and triggerable from Home Assistant: one `update` entity per node.
- Signed and verified images (Ed25519), SHA-256 digest, and an anti-rollback
  version floor.
- Dual-partition A/B with **automatic rollback** when a new image does not prove
  healthy.
- Power-loss safe and resumable.
- Respect the privacy posture: no new published surface. OTA rides the existing
  node control channel, not a node-facing HTTP endpoint (see §2).

Non-goals for v1:

- No cloud/remote update service and no node-initiated internet fetch.
- No delta/patch updates (full images only; deltas are a future optimisation).
- No update of the HA add-on or the HACS component here — those update through HA
  and HACS (see `home-assistant.md` §9).

## 2. Why the control channel, not node HTTP

`privacy-security.md` G4 makes the management HTTP+WS API loopback/ingress only,
and the node is a dumb sensor that must not run an HTTP server
(`capture-and-firmware.md` §9.2). OTA is therefore a **chunked transfer over the
existing AQCC control channel** (UDP `9101`), which already carries authenticated
request/response. UDP is unreliable, so the transfer adds explicit reliability:
per-chunk CRC, a sliding window with cumulative ACK, retransmission, and a
resumable offset.

## 3. Components

| Component | Lives in | Role |
| --- | --- | --- |
| Image store | core `/data/aquila/firmware/` | signed images + manifest (version, sha256, ed25519 sig, target chip, compatibility, notes) |
| OTA manager | core (`aquila-api` / `aquila-addon`) | orchestrates per-node updates, concurrency caps, retries, progress, ledger rows |
| Firmware OTA agent | ESP32-S3 firmware | receives chunks, writes the inactive partition, verifies, activates, health-checks, rolls back |
| HA update entity | HACS component (`update.py`) | one `update` entity per node; state + install action |

The add-on bundles the firmware image(s) matching its own version; a signed image
may also be imported by the operator through the dashboard. No image is ever
fetched from the internet.

## 4. Signing and trust

- Images are signed with the project's **Ed25519 firmware key** (distinct from a
  node's pairing token). The public key is a trust anchor provisioned into the node
  at flash time (embedded in the app or NVS).
- The node verifies the Ed25519 signature over the image digest **and** the SHA-256
  digest before activation; it never activates an unverified image.
- **Anti-rollback:** a monotonic firmware security version stored in NVS/eFuse
  rejects downgrades below a configured floor unless an explicit factory-authorised
  force flag is given.
- The core verifies an image (sha256 + Ed25519 + target chip + core compatibility
  range) before offering it to any node.
- ESP32-S3 **secure boot** is recommended but optional in v1; the residual risk
  (a physical attacker with one node's flash key) is documented, not hidden.

## 5. Transfer protocol (AQCC OTA opcodes)

Extends the control channel opcode table (`capture-and-firmware.md` §6.2):

| Code | Name | Direction | Body |
| --- | --- | --- | --- |
| `0x0E` | `OTA_BEGIN` | core → node | `image_id, size, sha256[32], sig[64], version, chunk_size, window` |
| `0x0F` | `OTA_CHUNK` | core → node | `offset, len, crc32, bytes[len]` |
| `0x10` | `OTA_ACK` | node → core | `received_upto, status` (cumulative) |
| `0x11` | `OTA_STATUS` | node → core | `state, received_upto, size` (progress; also polled) |
| `0x12` | `OTA_END` | core → node | `image_id, sha256[32]` — finalise, verify, activate |
| `0x13` | `OTA_ABORT` | both | `reason` — discard the transfer |
| `0x14` | `OTA_RESULT` | node → core | `outcome (verified/activated/rollback/failed), reason` |

Rules:

- Default chunk payload 1024 B (configurable, ≤ the maximum datagram); sliding
  window default 8 with cumulative ACK; missing chunks retransmitted.
- The node writes chunks **sequentially** into the inactive OTA partition; a resume
  token (`image_id`, `received_upto`) lets the core resume after a lost connection.
- CRC32 per chunk; SHA-256 over the whole image; Ed25519 over the digest.
- One OTA at a time per node; a global concurrency cap protects the LAN.

## 6. Activation, health check, rollback

1. After `OTA_END` and successful verify, the node sets the inactive partition as
   the next boot target (`esp_ota_set_boot_partition`) and reboots.
2. On first boot of the new image it starts in a **pending-verify** state and must
   self-confirm after a health window (default 60 s: associated, streaming CSI,
   control responsive) via `esp_ota_mark_app_valid_cancel_rollback()`.
3. If it does not confirm (crash loop or health timeout), the bootloader **rolls
   back** to the previous image and the node reports `OTA_RESULT rollback`.
4. Both partitions are retained until confirmation.

## 7. Home Assistant surface

- Platform **`update`**: `update.aquila_<node>_firmware` per node, device class
  `firmware`, with `installed_version`, `latest_version`, `update_available`, an
  install action, `in_progress` and `update_percentage`.
- Core API additions (private, loopback/ingress):

  | Path | Purpose |
  | --- | --- |
  | `GET /api/v1/firmware` | store manifest: versions, sizes, sha256, target chip, notes |
  | `GET /api/v1/nodes/{node}/firmware` | installed version, boot partition, pending, last result |
  | `POST /api/v1/nodes/{node}/firmware/install` | begin OTA (`{image_id, force?}`) → job id |
  | `GET /api/v1/nodes/{node}/firmware/job` | progress/state |
  | `POST /api/v1/nodes/{node}/firmware/abort` | abort the transfer |

- WS: a `firmware` event (`node`, `state`, `progress`) folded into the health
  stream.
- The component adds `update.py` (an `UpdateEntity` over the coordinator snapshot)
  and an `aquila.install_firmware` service. The dashboard gains an OTA panel.

## 8. Failure modes and safety

- **Power loss mid-transfer:** chunks are written sequentially; on reboot the
  inactive partition is incomplete so the bootloader keeps running the current
  image; the transfer resumes from `received_upto`.
- **Power loss before confirm:** rollback (§6).
- **Corrupt chunk:** CRC fails → NACK → retransmit.
- **Bad signature/digest:** node rejects, discards, reports `failed`; never
  activates.
- **Version incompatibility:** core refuses to offer; node rejects below the floor.
- **Link loss:** the transfer pauses and is resumable; bounded retries with
  backoff; abandonable.

## 9. Measurement

Ledger rows (recorded, not gated, per `measurement.md` policy):
`firmware.ota.duration_ms`, `firmware.ota.retries`, `firmware.ota.result`
(success/rollback/failed), `firmware.version.installed` per node. Deterministic
from a fixture (a simulated node OTA); statistical on hardware.

## 10. Open questions

- Firmware key custody and rotation; whether to **require** secure boot in v1.
- Delta updates; dual-partition is required for rollback and is assumed.
- Whether the add-on auto-offers its bundled image or requires operator import.
- Throttling the transfer to protect the sensed channel/backhaul.

## 11. References

- `docs/spec/capture-and-firmware.md` §6 (control channel), §9 (firmware).
- `docs/spec/home-assistant.md` (§5 API, §6 entities, §9 upgrade flow).
- `docs/spec/privacy-security.md` (G4 — no published management surface).
- `docs/spec/measurement.md`.
- Sources: `esp-idf-ota`, `esp-idf-ota-rollback`, `ha-update` in
  `docs/reference/sources.json`.
