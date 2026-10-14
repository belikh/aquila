# ADR-0006: The CSI sample byte order is imaginary-first

- Date: 2026-10-14
- Status: Accepted
- Supersedes: the sample-order wording of
  [ADR-0005](0005-canonical-csi-representation.md) ("interleaved `I, Q`",
  `[I0, Q0, I1, Q1, …]`). ADR-0005's decision to carry the raw signed int8
  sample stands; only the byte order it printed is corrected here.
- Related: `docs/spec/data-contracts.md` §2, §9.1;
  `docs/spec/capture-and-firmware.md` §5.1, §7.4, §9.1; `docs/spec/signal.md`
  §4.1, §7; `docs/spec/interface-register.md`;
  `docs/adr/0003-interface-reconciliation.md` §3
- Implemented by: `crates/aquila-types`, `crates/aquila-wire`,
  `crates/aquila-signal`, `crates/aquila-simulator`,
  `firmware/esp32-s3-csi-node`
- Reference: `docs/reference/sources.json` id `esp-csi-csi`

## Context

The ESP32 `esp-csi` capture delivers the channel state information as signed int8
pairs, one complex sample per subcarrier, read straight out of the receive buffer
(`wifi_csi_info_t.buf`). Espressif documents that buffer explicitly: "For each
subcarrier, the imaginary part is stored first, followed by the real part"
(`esp-csi-csi`). The hardware-native byte order is therefore
`[Q0, I0, Q1, I1, …]` — the imaginary (quadrature) byte first.

ADR-0005 froze the canonical sample as "interleaved `I, Q` per subcarrier in
ascending index order (`[I0, Q0, I1, Q1, …]`)" — the real byte first. That is the
conjugate order of the hardware. Because the payload is copied verbatim from the
receive buffer, every live frame was read with `I` and `Q` swapped, and every
derived phase (`atan2(Q, I)`) was conjugated. The record-level decoder
(`decode_record`) already assumed imaginary-first, so the two decoders of the
same buffer disagreed and both behaviours were pinned by tests.

The canonical wire/contract representation must be the hardware's native form;
no conversion belongs on the node.

## Decision

Pin the CSI sample byte order to **imaginary-first**, exactly as esp-csi emits it.

1. `aquila_types::CsiFrame::i_q` is the raw signed int8 sample, interleaved
   `Q, I` (**imaginary first**) per subcarrier in ascending index order:
   `[Q0, I0, Q1, I1, …]`, so `n_subcarriers == i_q.len() / 2`. The derived
   accessors (`i_at`, `q_at`, `amplitude`, `phase`) read the in-phase byte as the
   *second* byte of the pair and the quadrature byte as the *first*, so
   `amplitude` is `√(I² + Q²)` and `phase` is `atan2(Q, I)` on the hardware's
   true axes.
2. `aquila-wire` copies that sample verbatim under the retained `AQCI` magic and
   16-byte header / 28-byte fixed body. The header `sc_format` byte stays
   `INT8_IQ` (`1`); the payload is `2 × n_subcarriers` bytes, imaginary first.
3. `aquila-signal::decode_csi` and `decode_record` both derive each `Complex32`
   bin from the imaginary-first pair, dividing by the nominal
   `IQ_COUNTS_PER_UNIT`. This is where the old on-node amplitude/phase
   derivation now lives.
4. `aquila-simulator` quantises to the same imaginary-first order, so the
   hardware order is exercised end to end. A phase-sign end-to-end test from a
   hardware-shaped capture pins it.
5. The node still does no floating point: it copies the raw int8 receive buffer
   verbatim, except for the documented first-complex-sample impairment.

ADR-0005's payload decision — raw signed int8, one node-side `memcpy`, derived
amplitude/phase, `IQ_COUNTS_PER_UNIT` — is unchanged. Only its printed byte order
is corrected.

## Alternatives considered

1. **Keep real-first and conjugate in the core.** Rejected: the wire would carry
   the hardware's bytes in a non-hardware order, every consumer would need a
   documented swap, and the phase-sign error is silent — exactly the failure
   this ADR removes.
2. **Reorder the bytes to real-first on the node.** Rejected: it is node-side
   signal processing against "dumb sensor", touches the receive buffer, and
   duplicates work the core can do once, under test, at decode.
3. **Carry a per-frame order flag on the wire.** Rejected: there is one hardware
   order; a flag invents a second wire form and a mode to test for no gain.
4. **Leave the two decoders disagreeing and defer to a real capture.** Rejected:
   the vendor documents the order now (`esp-csi-csi`); deferring keeps a
   known-wrong phase in the frozen path.

## Consequences

- Positive: derived phase has the hardware's sign; the motion and vitals DSP is
  no longer fed a conjugated channel.
- Positive: `decode_csi` and `decode_record` agree on one order, and one
  phase-sign test proves the end-to-end path from a hardware-shaped capture.
- Positive: the node remains a pure `memcpy`; the imaginary-first order is the
  receive buffer's own.
- Accepted cost: ADR-0005's printed `[I0, Q0, …]` is wrong and is corrected
  here; any text that copied it must be updated (the sweep is
  `docs/spec/data-contracts.md`, `docs/spec/capture-and-firmware.md`,
  `docs/spec/signal.md` and `docs/spec/interface-register.md`).
- The `AQCI` magic, the 16-byte header, the 28-byte fixed body, the `INT8_IQ`
  format byte and the control plane are unchanged, so ADR-0004's remaining
  decisions stand.
