# ADR-0004: One on-air CSI layout — CsiFrame-compatible payload under `AQCI`

- Date: 2026-10-14
- Status: Superseded in part by [ADR-0005](0005-canonical-csi-representation.md)
  (the `f32` amplitude/phase payload; the `AQCI` magic and the header/body field
  set are retained)
- Related: `docs/spec/capture-and-firmware.md` §5.1, `docs/spec/data-contracts.md` §9.1,
  `docs/spec/interface-register.md`, `docs/adr/0003-interface-reconciliation.md`
- Implemented by: `crates/aquila-wire`

> **Superseded in part.** [ADR-0005](0005-canonical-csi-representation.md)
> replaces the `f32` amplitude/phase payload decision below with a raw signed
> int8 I/Q payload. The `AQCI` magic and the header/body field set decided here
> are retained; do not follow this ADR's payload guidance.

## Context

`aquila-wire` is the single on-air codec (ADR-0003 §4). Two specs describe the CSI
data-plane datagram and they disagree:

- `capture-and-firmware.md` §5.1: magic `AQCI`, a 56-byte header, factory MAC plus
  an int8 interleaved I/Q payload.
- `data-contracts.md` §9.1: magic `AQCS`, a 16-byte header, an `f32` amplitude/phase
  payload, described as the "canonical on-air" form.

Only the `data-contracts.md` shape round-trips the frozen `CsiFrame` contract
(`data-contracts.md` §2) through the `encode_datagram`/`decode_datagram` signatures the
register requires. The two cannot both be the wire form without a lossy conversion that
would break the frozen contract and the replay determinism the tracer bullet depends on.

## Decision

Adopt **one** layout: the `data-contracts.md` §9.1 header offsets, version rules and
`f32` amplitude/phase payload, carried under the **`AQCI`** magic name that the capture
spec and the register use. Concretely:

1. One CSI frame per UDP datagram, little-endian, byte layout pinned by a golden vector
   in `aquila-wire`.
2. The wire carries `NodeId`; the factory `MacAddr` is translated at the seam by
   `aquila-wire`'s `NodeIdentityMap` (ADR-0003 §3).
3. The control plane keeps the separate `AQCC` framing (ADR-0003 §7 ports `9100`/`9101`).
4. `node_wire.h` is generated from `aquila-wire::generate_c_header()` so the firmware and
   the core cannot drift (ADR-0003 §4).

## Alternatives considered

1. **Adopt `capture-and-firmware.md` §5.1 (int8 I/Q, 56-byte header).** Rejected: it does
   not round-trip the frozen `CsiFrame`, forcing a lossy or lossless-but-spec-breaking
   conversion, and surrenders the amplitude/phase contract the signal pipeline consumes.
2. **Keep both layouts with a mode flag.** Rejected: two wire forms defeat the "one layout,
   two languages" purpose of `aquila-wire` and double the firmware surface.

## Consequences

- Positive: the wire and the frozen contract agree; the golden vector pins it; the firmware
  header is generated, not hand-maintained.
- Accepted cost: `capture-and-firmware.md` §5.1 (56-byte header, int8 I/Q) must be corrected
  to this ADR when next touched; the firmware must emit `f32` amplitude/phase, not int8 I/Q.
- The variable-length subcarrier member is a documented gap in the generated C header and is
  a firmware-side follow-up.
