# 13 — ESP32-S3 firmware OTA agent

- Type: task
- Blocked by: 07, 12
- Status: open

## Goal

Implement the node-side OTA agent: receive chunks over the control channel, write
the inactive OTA partition, verify Ed25519 + SHA-256, activate, health-check, and
roll back automatically if the new image does not prove healthy.

## Implements

- `docs/spec/firmware-ota.md` §5, §6, §8.
- `docs/spec/capture-and-firmware.md` §9 (firmware responsibilities).
- New firmware component `firmware/esp32-s3-csi-node/components/node_ota/`.

## Relies on

- `docs/reference/sources.json`: `esp-idf-ota`, `esp-idf-ota-rollback`.

## Acceptance criteria

- The firmware handles `OTA_BEGIN`/`OTA_CHUNK`/`OTA_ACK`/`OTA_END`, writing chunks
  sequentially to the inactive partition via `esp_ota_*`, with CRC32 per chunk and
  SHA-256 + Ed25519 over the whole image before activation.
- On success: `esp_ota_set_boot_partition`, reboot, pending-verify health window,
  then `esp_ota_mark_app_valid_cancel_rollback`; on failure: automatic rollback
  and an `OTA_RESULT`.
- Anti-rollback version floor enforced.
- `node_wire.h` regenerated from `aquila-wire` to include the OTA opcodes.
- The firmware README states the untested-on-hardware status explicitly; any
  host-testable framing/verification logic is unit-tested, the rest is flagged.

## First test-first step

Regenerate `node_wire.h` with the OTA opcodes and add a `node_ota` component
skeleton that acknowledges `OTA_BEGIN`.
