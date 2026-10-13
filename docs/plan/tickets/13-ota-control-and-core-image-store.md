# 13 — OTA control channel and core image store

- Type: task
- Blocked by: #8
- Status: open

## Goal

Add the firmware OTA transfer to the AQCC control channel and the core-side
orchestration and signed image store, so the core can push a signed image to a
node over the LAN and track the job — no cloud, no node HTTP server.

## Implements

- `docs/spec/firmware-ota.md` §3, §4, §5, §7.
- `docs/spec/capture-and-firmware.md` §6 (opcodes `0x0E`–`0x14`).
- `docs/spec/interface-register.md` (Firmware OTA).
- `docs/spec/core-api.md`, `docs/spec/home-assistant.md` §5 (endpoints).

## Relies on

- `docs/reference/sources.json`: `esp-idf-ota`, `ha-update`.

## Acceptance criteria (test-first)

- `aquila-wire` encodes/decodes the new control opcodes with golden vectors:
  `OTA_BEGIN`, `OTA_CHUNK`, `OTA_ACK`, `OTA_STATUS`, `OTA_END`, `OTA_ABORT`,
  `OTA_RESULT`.
- The core OTA manager chunks an image with a sliding window, cumulative ACK,
  per-chunk CRC32, retransmission and a resumable offset; a fake transport
  receives a full image and reports success.
- A signed image store under a configurable data directory verifies SHA-256 +
  Ed25519 and the target-chip/compatibility range, writes a manifest, and rejects
  a bad signature (never offers it).
- The `/api/v1/firmware*` endpoints report job progress; a `Measurement` row is
  recorded.
- Deterministic tracer over a fake transport (no hardware), matching the
  record-and-diff ledger policy.

## First test-first step

A red test in `aquila-wire` that round-trips `OTA_BEGIN`/`OTA_CHUNK`/`OTA_ACK`,
then a `aquila-testkit` fake node that accepts a full image.
