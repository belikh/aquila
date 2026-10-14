# ADR-0005: The canonical CSI sample is raw signed int8 I/Q

- Date: 2026-10-14
- Status: Accepted; superseded in part by
  [ADR-0006](0006-csi-iq-byte-order.md) (the sample byte order only — see the
  banner below; the raw signed int8 payload decision stands)
- Supersedes: the payload decision of
  [`docs/adr/0004-on-air-csi-layout.md`](0004-on-air-csi-layout.md) (the `AQCI`
  magic and the header/body field set are retained)
- Related: `docs/spec/data-contracts.md` §2, §9.1; `docs/spec/capture-and-firmware.md`
  §5, §7, §9; `docs/spec/signal.md` §4.1; `docs/spec/interface-register.md`
- Implemented by: `crates/aquila-types`, `crates/aquila-wire`,
  `crates/aquila-signal`, `crates/aquila-simulator`

> **Superseded in part.** [ADR-0006](0006-csi-iq-byte-order.md) corrects the
> **sample byte order** printed below: the canonical sample is
> interleaved `Q, I` (**imaginary first**), `[Q0, I0, Q1, I1, …]`, as esp-csi
> emits it — not the `[I0, Q0, …]` real-first order written here. Every other
> decision below (raw signed int8, one node-side `memcpy`, derived
> amplitude/phase, `IQ_COUNTS_PER_UNIT`) is retained; do not follow this ADR's
> byte-order wording.

## Context

The ESP32 `esp-csi` capture delivers the channel state information as **signed
int8 I/Q samples**: one complex pair per subcarrier, read straight out of the
receive buffer. That is the form the radio modem produces and the only form the
node can obtain without doing arithmetic.

ADR-0004 froze a different representation as canonical: an `f32` amplitude and
`f32` phase per subcarrier, on the wire and in `aquila_types::CsiFrame`. That
decision contradicted the hardware. It forced three defects:

1. **On-node floating point.** To emit amplitude/phase the firmware had to run
   `sqrtf(re*re + im*im)` and `atan2f(im, re)` in the sender task, against the
   "dumb sensor; no on-node processing" rule (`capture-and-firmware.md` §9.2).
2. **A bloaty wire.** Two `f32` per subcarrier is 8 bytes; the raw sample is 2.
   The payload was ~4× larger than the hardware form for no information gain.
3. **An untested path.** The simulator synthesised amplitude/phase directly, so
   nothing ever exercised the int8 representation the hardware actually uses.

The canonical wire/contract representation must be the hardware's native form;
derivations belong downstream, in the core.

## Decision

Make the **raw signed int8 I/Q sample** the canonical CSI representation.

1. **`aquila_types::CsiFrame`** stores the sample verbatim as `i_q: Vec<i8>`,
   interleaved `I, Q` per subcarrier in ascending index order (`[I0, Q0, I1,
   Q1, …]`), so `n_subcarriers == i_q.len() / 2`. `subcarrier_offset`, `band`,
   `channel`, `bandwidth_mhz`, `boot_epoch`, `sequence`, `device_time_us`,
   `rssi_dbm`, `noise_floor_dbm` and `phy` are unchanged. `validate` requires a
   non-empty, even-length sample. Amplitude and phase are **derived**, not
   stored: `CsiFrame::amplitude`/`phase` and `amplitude_at`/`phase_at` compute
   `√(I²+Q²)` and `atan2(Q, I)`.
2. **`aquila-wire`** carries the same sample under the retained `AQCI` magic and
   16-byte header / 28-byte fixed body. The header `sc_format` byte becomes
   `INT8_IQ` (`1`); the payload is `2 × n_subcarriers` bytes, interleaved `I, Q`.
   The header/body field set, offsets and version rules are otherwise
   unchanged. The generated `node_wire.h`, golden vector and C header follow.
3. **`aquila-signal::decode_csi`** derives the internal `ComplexFrame` bins from
   the int8 sample: each pair `[I, Q]` becomes one `Complex32`, divided by the
   nominal `IQ_COUNTS_PER_UNIT`. This is where the old on-node arithmetic now
   lives.
4. **The node does no floating point.** The firmware copies the raw int8 receive
   buffer into the payload, unchanged except for the documented first-word
   impairment.
5. **`aquila-simulator`** synthesises the complex `H_k`, then **quantises** it to
   signed int8 — modelling the hardware — so the full pipeline, including the
   core's int8 → f32 derivation, is exercised by the test suite.

The canonical sample is the node's raw form; the arbitrary esp-csi gain is
absorbed by one documented constant (`IQ_COUNTS_PER_UNIT`) in the core.

## Alternatives considered

1. **Keep ADR-0004's `f32` amplitude/phase.** Rejected: it contradicts the
   hardware and requires on-node floating point and a needlessly large payload
   (this ADR exists to undo it).
2. **Store amplitude and phase as canonical and derive int8 on the wire, with a
   lossy quantisation at the node.** Rejected: it invents a representation the
   hardware never produces and still needs the on-node conversion.
3. **Normalise each frame to full int8 scale on the node (AGC).** Rejected: it
   is on-node signal processing, it destroys the absolute amplitude the core
   uses, and esp-csi's default is a fixed manual scale.
4. **Bump `wire_major` to force a clean break.** Rejected for now: the
   `sc_format` byte already distinguishes the payload, and `wire_major` is shared
   with the unchanged control plane (`CONTROL_VERSION == WIRE_MAJOR`). The
   format byte alone rejects a stale decoder and is counted at the seam.

## Consequences

- Positive: the node is a dumb sensor again — a pure `memcpy` of the receive
  buffer, no `math.h`; the wire is 2 bytes per subcarrier instead of 8; and the
  simulator's int8 path means the derivation is covered end to end.
- Positive: amplitude and phase are now unambiguously *derived*, so the one
  place they can drift is the core, under test, rather than the firmware.
- `MAX_SUBCARRIERS` rises from 178 to 714 for the same 1472-byte datagram
  budget; the old bound is not loosened in bytes, only in subcarriers.
- Accepted cost: unlike `f32`, the int8 sample carries amplitude in arbitrary
  counts; the core's working unit is fixed by `IQ_COUNTS_PER_UNIT`, and any
  downstream absolute threshold (for example the uncalibrated presence
  fallback) is expressed in that unit.
- Accepted cost: a capture that fills less of the int8 range carries coarser
  amplitude resolution; this is the hardware's resolution, and the honest one to
  model.
- The `AQCI` magic, the 16-byte header, the 28-byte fixed body and the control
  plane are unchanged, so ADR-0004's remaining decisions stand.
