# 07 — Live nodes and node control

- Type: task
- Blocked by: 06
- Status: open

## Goal

Real ESP32-S3 nodes stream raw CSI over WiFi UDP into the core running as an HA
add-on, the core drives nodes over the direct control channel, and live drift
estimation matches replay-of-capture within tolerance. The replay and live
adapters now both satisfy `CsiSource`, making the seam real.

## Implements

- `docs/spec/roadmap.md` Slice 7.
- `docs/spec/capture-and-firmware.md` §5 (data-plane wire, ports, loss), §6
  (control channel, pairing, heartbeat), §9 (firmware responsibilities and
  MUST-NOTs), §9.4 (one wire layout, two languages).
- `docs/spec/core-api.md` (private HTTP+WS control plane, node commands,
  loopback/ingress binding).
- `docs/spec/fusion-sync.md` §1.4–1.6 (correcting live stamps, sync health).
- `docs/spec/privacy-security.md` G4–G7 (loopback/ingress, enrolment,
  authenticated replay-resistant ingest).
- `docs/spec/interface-register.md` (ports `9100`/`9101`); ADR-0003 §7.

## References

`esp-csi` (CSI capture), `espectre` (unicast requirement), `espargos` (coherence
boundary), `acma-lipd`/`fcc-part15` (compliance boundary), `ha-addons`.

## Acceptance criteria

- **End-to-end gate:** a real node streams CSI over UDP; presence appears as an
  HA entity with no MQTT and no ESP-NOW.
- **Parity gate:** live drift estimation matches replay-of-capture within the
  `fusion-sync.md` §1.5 tolerance (risks.md R-06); live metrics carry a distinct
  authority.
- **Integrity gate:** UDP loss and reordering are detected and recorded; a
  session with hidden loss never carries a `Measured` gated result
  (privacy-security.md G7, risks.md R-20/R-23).
- **Enrolment gate:** only enrolled nodes enter the pipeline; unenrolled packets
  are dropped and counted (privacy-security.md G6).
- **Control gate:** commands and config are transactional and idempotent; the
  firmware performs no DSP, inference or clock discipline.
- Ledger rows `live.presence.f1`, `live.clock.drift`, `host.streams.count`.

## Test-first first step

Write a failing adapter-conformance test that replays a recorded UDP capture
through `UdpLiveSource` and asserts it satisfies the same `CsiSource` contract
suite as `ReplaySource`.
