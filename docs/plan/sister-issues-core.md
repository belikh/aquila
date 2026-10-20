# Sister-issues audit — Aquila `crates/` (Rust)

- **Trigger:** finding #36 in [`orchestrator-omissions.md`](orchestrator-omissions.md) — the
  canonical CSI representation was `f32` amplitude/phase although the hardware emits
  `int8` I/Q; the node faked `f32` from `int8`; the wire was ~4× too big; the simulator
  never exercised the derivation. Remediated by
  [`docs/adr/0005-canonical-csi-representation.md`](../adr/0005-canonical-csi-representation.md).
- **Scope:** everything under `crates/` (Rust), read-only. The firmware C tree was read
  only to establish the hardware-native form. No repository content changed except this
  file.
- **Method:** `rg`/`grep`/`read` over the whole workspace, cross-checked against
  `docs/spec/*` and `docs/adr/*`. Findings are defects of the same *class* as #36, not
  repeats of #36 (which is already fixed).
- **Classes:** 1 representation mismatch · 2 derivation in the wrong place ·
  3 duplicate concepts · 4 fake/stub/placeholder as real · 5 untested critical path ·
  6 fabricated/ungraded numbers · 7 silent spec divergence · 8 hardcoded magic ·
  9 honesty gaps.

Each finding gives a stable id, its primary class (cross-class noted), `file:line`, a
verbatim quote, why it is a defect, a severity, and a concrete fix. Findings marked
**[definite]** are demonstrable from the code; **[decision]** need a call.

---

## Findings

### S1 — Two mutually-contradictory ESP32 I/Q byte orders, both frozen by tests — **class 1 (also 7, 9)** — `critical` — [definite]

- `crates/aquila-signal/src/decode.rs:279-282` — “The input `raw` is interleaved `int8 I/Q` pairs, imaginary byte first … each pair `[im, re]` becomes one `Complex32`”; code at `:320-322` reads `im = raw[offset]; re = raw[offset+1]`.
- `crates/aquila-signal/src/tests.rs:85-88` — `raw[0] = 5; // imaginary first` … `assert_eq!(frame.bins[0], Complex32::new(-6.0, 5.0));`
- The **canonical** path is the opposite: `crates/aquila-types/src/csi.rs:58-63` (“interleaved `I, Q` … `[I0, Q0, I1, Q1, …]`”), `i_at`/`q_at` at `:84-99`, the wire at `crates/aquila-wire/src/datagram.rs:30,110` and `crates/aquila-wire/src/constants.rs:32-34`, and `decode_csi` at `decode.rs:250` all treat byte 0 as **I (real)**.
- Firmware `firmware/esp32-s3-csi-node/components/csi_node/csi_node.c:145` copies `info->buf` **verbatim** into that payload, so exactly one of the two conventions is wrong for every live frame.
- The hardware order is an acknowledged open question: `docs/spec/signal.md:637-639` (“Exact ESP32-S3 CSI byte order (imaginary-first vs. real-first) … must be confirmed against a real capture before `decode_csi` is frozen”) and `firmware/.../README.md:125-128`.

**Why it is a defect.** This is #36’s exact shape: a canonical representation frozen
without checking the hardware’s native form. Two decoders of the *same* ESP32 buffer
assume opposite orders (`decode_record` vs `decode_csi`/wire/firmware), and both
behaviours are pinned by tests, so a fix silently breaks a test rather than failing
loudly. A wrong order swaps I/Q on every subcarrier of every live capture — corrupting
phase (`atan2(Q,I)` becomes `atan2(I,Q)`) and the motion/vitals DSP — with no error.

**Fix.** Confirm the real esp-csi `wifi_csi_info_t.buf` order from the pinned ESP-IDF
version; pick one convention; make the other decoder delegate to the chosen one; add an
ADR recording the hardware-native order; update `signal.md` §4.1/§7 and both tests.

---

### S2 — `first_word_invalid` covers 4 bytes in the firmware spec, 2 bytes in the core — **class 1 (also 7)** — `high` — [definite]

- `docs/spec/capture-and-firmware.md:116-117` — “first four CSI bytes as unusable.”
- `docs/spec/signal.md:235-236` — “the first I/Q pair is zeroed and `flags.first_word_zeroed = true`.”
- `firmware/.../csi_node.c:220-226` — comment says “the first **4 bytes** are invalid … the first subcarrier is nulled”, but the code zeros only `csi[0]` and `csi[1]` (2 bytes).
- `crates/aquila-signal/src/decode.rs:331-336` and `crates/aquila-simulator/src/source.rs:321-328` zero exactly one I/Q pair.

**Why it is a defect.** The hardware quirk is documented as invalidating the first
**word** (4 bytes, i.e. two I/Q pairs); three places implement it as 2 bytes, in
disagreement with the capture spec, and no ADR records the choice. If the driver really
invalidates 4 bytes, the core silently keeps one corrupt subcarrier (and the simulator
cannot reveal it).

**Fix.** Pin the exact impairment width from the ESP-IDF version; make spec, firmware,
decoder and simulator agree; record it in an ADR.

---

### S3 — The canonical frame and wire drop the `first_word_invalid` hardware flag — **class 9 (also 1)** — `high` — [definite]

- `firmware/.../csi_node.c:220-222` — “The first 4 bytes are invalid when the driver says so; the frozen `CsiFrame` has no such flag, so the first subcarrier is nulled.”
- `crates/aquila-signal/src/decode.rs:266` — `first_word_invalid: false,` is **hardcoded** on the contract path.
- The wire header has no such bit (`crates/aquila-wire/src/datagram.rs:8-31`; `node_wire.h`).

**Why it is a defect.** A real hardware condition is encoded by destructive nulling:
after decoding, the core cannot distinguish “impaired capture” from “genuinely zero
sample”, and `RxMeta.first_word_invalid` is always `false` on the only path the pipeline
uses. This is the honesty-gap class (plausible output from absent information).

**Fix.** Carry the flag in `CsiFrame` and on the wire (a reserved bit/byte is already
available at datagram offset 15), propagate it through `decode_csi`, and let downstream
thresholds exclude the flagged subcarrier.

---

### S4 — Coarse pose is keyed to the simulator’s synthetic dither frequencies — **class 6 (also 5, 2)** — `critical` — [definite] — **the second #36**

- `crates/aquila-sensing/src/pose.rs:56-60` — “The default per-scatterer pose-dither frequencies … **This must match the synthetic pose signature the simulator injects** (`aquila_simulator`’s `TORSO_DITHER_HZ`)” → `[3.0, 5.0, 7.0, 9.0]`.
- `crates/aquila-simulator/src/scene.rs:100-111` — the simulator injects exactly `[3.0, 5.0, 7.0, 9.0]` per torso scatterer.
- `crates/aquila-sensing/src/pose.rs:170-195` — the estimator runs a Goertzel **at those exact frequencies** to recover ranges.
- `crates/aquila-testkit/src/tests.rs:1020-1053` — `coarse_pose_meets_the_torso_pck20_bar` asserts `score.pck >= 0.65`, the v1 gated bar (`CONTEXT.md` “Pose … torso-PCK@20 ≥ 65%”), against ground truth produced by the *same* simulator.
- `crates/aquila-sensing/src/pose.rs:309-316` — the resulting pose is emitted `witness: Witness::Measured`.

**Why it is a defect.** The estimator searches for precisely the signature the simulator
injects, so the test is circular: it can only ever pass. A real human has no 3/5/7/9 Hz
“torso keypoint” signature, so the gated 65 % claim is fabricated evidence for real
hardware, and the simulator is constitutionally unable to detect the failure. This is
#36 generalised: **the canonical algorithm was frozen against a synthetic implementation
detail, and the simulator shares the constant, so nothing tests the derivation.**
`Witness::Measured` presents it as evidence-backed.

**Fix.** Replace the dither-frequency surrogate with a physics-derived ranging estimator
(or a learned model behind the `PoseModel` seam); until then mark pose outputs
`Witness::HardwareGated`/`Claimed` and stop asserting the v1 gated bar on synthetic
input. If the surrogate is retained as a test-only asset, it must not be the estimator’s
production default.

---

### S5 — v1 gated metrics asserted on synthetic-only fixtures — **class 6 (also 5)** — `critical` — [definite]

- `crates/aquila-testkit/src/tests.rs:1049-1058` — `assert!(score.pck >= 0.65, "torso-PCK@20 … below the v1 bar")` and `assert!(presence.f1 >= 0.90, "fused presence F1 … below the v1 bar")`.
- `crates/aquila-signal/src/tests.rs:571-596` — `vitals_estimates_a_known_breathing_rate` asserts ≤2 brpm (the v1 MAE bar) on a synthetic tone.
- `docs/spec/testing.md:126-127` — “**synthetic** … May **never** back an accuracy claim.”

**Why it is a defect.** The three headline v1 bars (F1 ≥ 0.9, PCK@20 ≥ 65 %, MAE ≤ 2 brpm)
are asserted here purely from the deterministic generator, with no ledger `Gated` row
against a recorded fixture. `Witness::Gated` “is derived from a `Gated` ledger record with
`passed == true`” (`crates/aquila-types/src/primitives.rs:99`); these tests bypass the
ledger entirely.

**Fix.** Gate the headline numbers on `recorded` fixtures only; keep synthetic runs as
`Measured` sanity checks (or a separate, clearly-labelled `synthetic` tier).

---

### S6 — Subcarrier selection’s “noise power” is hardcoded to zero, so SNR selection is a no-op — **class 4 (also 7)** — `high` — [definite]

- `crates/aquila-signal/src/process.rs:437-442` — `SubcarrierFeatureMatrix::new(usable.clone(), motion_energy.clone(), vec![0.0; n_use])`.
- `crates/aquila-signal/src/selection.rs:36-48` — score is `band / noise`, falling back to `band` when `noise == 0.0`; with all-zero noise the “SNR” score degenerates to raw band power.
- Docs/spec say selection is by signal-to-noise and is **frozen into `Config`** at calibration time (`signal.md` §4.11; `selection.rs:4-6`), but `Config` (`process.rs:97-116`) has **no mask field** and `select_k: 8` is a runtime default (`:130`).

**Why it is a defect.** The documented mechanism (calibrated, SNR-ranked, frozen mask) is
not the one that runs (uncalibrated, raw-power-ranked, recomputed per chunk with a
zeroed noise term). It is both a fake (the noise input is present by name but always
zero) and a spec divergence.

**Fix.** Compute a real per-subcarrier noise power, or compute the selection at
calibration and carry a `SubcarrierMask` in `Config`; if the degenerate behaviour is
intended for now, remove the misleading noise parameter and record the divergence.

---

### S7 — Shared synthetic constants duplicated across crates because the DAG forbids the edge — **class 3 (also 2)** — `medium` — [definite]

- `crates/aquila-sensing/src/pose.rs:56-60` — “it is duplicated here because the sensing crate may not depend on the simulator in the dependency DAG.”
- `crates/aquila-simulator/src/scene.rs:108` vs `crates/aquila-sensing/src/pose.rs:60` — `TORSO_DITHER_HZ` duplicated verbatim.
- Torso geometry duplicated: `crates/aquila-simulator/src/scene.rs:146-148` (`shoulder_half_width_m: 0.20`, `torso_length_m: 0.50`) vs `crates/aquila-sensing/src/fusion.rs:205-211` (`TorsoGeometry::adult`).
- The DAG guardrail that forces this is real: `tools/dag.toml`, `xtask/src/dag.rs`.

**Why it is a defect.** Two copies of a constant that *must* stay bit-identical for the
pose test to pass, with no test asserting they match. Any edit to one silently changes
the meaning of the other (or, if they diverge, makes the only pose test fail for the
wrong reason).

**Fix.** Move the constant/geometry into a lower, non-test crate (e.g. a `feature` or
`testkit`-independent seam in `aquila-types`), or add a test that pins the two equal.

---

### S8 — Pose DSP and rigid-torso fusion live in the stateful core, bypassing the inference seam — **class 2** — `high` — [definite]

- `crates/aquila-sensing/src/pose.rs:142-262` — Goertzel range extraction and phase-slope ranging implemented directly in `aquila-sensing`.
- `crates/aquila-sensing/src/fusion.rs:290-458` — the rigid-torso weighted least-squares fit and interior prior.
- `crates/aquila-sensing/src/pose.rs:7-9` — “the core calls this estimator directly, because the seam’s `FeatureVector` layout is not yet pinned … this path needs the raw per-node CSI series.”

**Why it is a defect.** The `PoseModel` seam (`crates/aquila-inference/src/seam.rs`) is
the declared boundary for pose; the DSP and fusion that belong to `aquila-signal`/the
model are implemented in the top stateful crate, which then cannot be swapped without
touching the core. This is the “work done in the wrong place” class, and it is the
proximate cause of S4 (the synthetic frequencies leaked into the core because the
algorithm was written there).

**Fix.** Move the estimator behind the seam (with a pinned feature layout), or record an
ADR that the in-core surrogate is deliberate and time-boxed.

---

### S9 — Presence threshold tuned to the simulator’s amplitude scale — **class 8 (also 6)** — `high` — [definite]

- `crates/aquila-sensing/src/config.rs:151-159` — “The motion threshold is **tuned to the deterministic simulator’s amplitude scale**; it is a starting point, not a claim … a moving occupant measures ~0.22 and an empty room ~0.03 …, so 0.10 sits between them.” Values `presence_motion_threshold: 0.10`, `presence_motion_scale: 0.03`.
- These are the **uncalibrated** defaults (`engine.rs:456`), i.e. what runs before/without a calibration artefact.

**Why it is a defect.** After ADR-0005 the amplitude unit is set by
`IQ_COUNTS_PER_UNIT` and the real esp-csi gain; the simulator’s post-quantisation mean
amplitude is a simulator artefact. On real hardware the absolute level differs, so a
0.10 threshold is meaningless — a class-6 threshold tuned to synthetic data (partly
mitigated by the comment, which is why this is high not critical).

**Fix.** Express the threshold as a per-node dB rise above a learned baseline (the
calibrated path already exists), make the uncalibrated default refuse to decide rather
than guess, and record the tuning in the ledger.

---

### S10 — Simulator quantisation saturates the int8 range and fabricates RSSI — **class 5 (also 6)** — `high` — [definite]

- `crates/aquila-simulator/src/source.rs:59-61` — `(value * 64.0).round().clamp(-128.0, 127.0)`.
- `crates/aquila-simulator/src/source.rs:206-224,302-318` — baseline `re ∈ [0.5, 1.0]` plus several 0.5–0.7 scatterers, ×64, routinely exceeds 127 and clamps.
- `crates/aquila-simulator/src/source.rs:340-341` — `rssi = (-90.0 + 15.0 * mean).clamp(...)`; `noise_floor = rssi - 30`.

**Why it is a defect.** After ADR-0005 the simulator is the *only* model of the hardware
form, but it models a capture that saturates a large fraction of the int8 range — which
destroys exactly the amplitude information `CsiFrame::amplitude()`/`IQ_COUNTS_PER_UNIT`
depend on — and it invents RSSI from a linear clamp. Both are presented as the hardware
model, so the pipeline is validated against a fiction (the #36 simulator archetype,
inverted: it exercises the int8 *path* but with unphysical occupancy).

**Fix.** Model the real esp-csi fixed-point gain (`manu_scale`/AGC) so saturation is
rare, flag saturated frames, and derive RSSI from a link budget rather than a tuned
clamp.

---

### S11 — Hardcoded `subcarrier_offset = -(n/2)` and provisional null-bin maps — **class 8** — `medium` — [definite]

- `firmware/.../csi_node.c:228` — `int16_t subcarrier_offset = (int16_t)(-(int16_t)(n / 2u));`
- `crates/aquila-simulator/src/source.rs:356` — `subcarrier_offset: -((n / 2) as i16)`.
- `crates/aquila-signal/src/decode.rs:137-158` — “provisional HT20/HT40 layout … The exact null-bin map is a research-verify item.”

**Why it is a defect.** The DC/null/guard-bin map and the subcarrier offset differ by
bandwidth and real hardware; they are hardcoded to a guess and never validated against a
capture, yet they set the frequency axis for every DSP stage.

**Fix.** Derive from a real capture and make both offset and null map configuration
(frozen into `Config`), per `signal.md` §7.

---

### S12 — Two copies of the channel→frequency map, and a wrong 2.4 GHz channel 14 — **class 1 (also 3, 8)** — `medium` — [definite]

- `crates/aquila-signal/src/decode.rs:217-223` and `crates/aquila-simulator/src/source.rs:410-417` — both compute `2_407 + channel*5` / `5_000 + channel*5`.
- Channel 14 → 2477 MHz; the real 2.4 GHz channel 14 is 2484 MHz (`csi.rs:52` documents channels 1–14). 5 GHz is likewise not a linear `5000 + ch*5` plan.
- `decode.rs:217` claims it is “for diagnostics only”, but the simulator uses the same value as the **physics RF reference** in the scatterer phase (`source.rs:290-302`).

**Why it is a defect.** Two implementations of one physical map (a duplicate-concept
defect) that are also wrong at the band edge; and the value is not “diagnostics only”
for the simulator, where a 7 MHz error shifts the phase slope used for ranging.

**Fix.** One shared lookup with the real channel plan; add channel-14 and a 5 GHz
channel to the tests.

---

### S13 — Two different “maximum raw CSI length” constants — **class 3** — `medium` — [definite]

- `crates/aquila-signal/src/decode.rs:298-300` — `if raw.len() > 612 { TooLarge }`, `error.rs:26-27` “612-byte on-air maximum”.
- `crates/aquila-wire/src/constants.rs:52-58` — `MAX_SUBCARRIERS = 714` (1428 payload bytes), mirrored in `node_wire.h:27` and `csi_node.c:38` (`CSI_RAW_MAX = 1428`).

**Why it is a defect.** 612 bytes is a leftover from the superseded 56-byte-header layout
(ADR-0004/`capture-and-firmware.md` §5.1). The record-level decoder now rejects a legal
714-subcarrier frame that the wire and the contract decoder accept, and the boundary is
untested at 714.

**Fix.** Derive the record cap from `MAX_SUBCARRIERS` (or delete the unused
record-level path and keep one decoder).

---

### S14 — All-zero digests presented as content addresses — **class 9 (also 4)** — `medium` — [definite]

- `crates/aquila-capture/src/record.rs:128` — live-capture `Session { config_hash: [0u8; 32], seed: 0, … }`.
- `crates/aquila-simulator/src/source.rs:597,602` — `config_hash: [0u8; 32]` and `GroundTruthRef { sha256: [0u8; 32] }`.
- `crates/aquila-testkit/src/generator.rs:109-112,189-192` and `crates/aquila-addon/src/main.rs:190-193` — `FirmwareRef { sha256: [0u8; 32] }`.

**Why it is a defect.** These are content-addressed fields (`config_hash`, artefact
digest, firmware digest) carrying an all-zero value that passes validation and looks
real; a consumer that compares digests will treat every artefact as identical, and the
manifest’s provenance is silently unbacked.

**Fix.** Make the fields `Option` (absent when unknown) or compute the real digest;
reject an all-zero digest in `validate`.

---

### S15 — Measurement ledger is keyed to a null commit by default — **class 8 (also 6)** — `medium` — [definite]

- `crates/aquila-sensing/src/config.rs:33-41` — `MeasurementConfig::default()` → `commit: "0".repeat(40)`, `host: "aquila-sensing"`.
- `crates/aquila-sensing/src/config.rs:283-293` — validation accepts any 40 hex chars, so all-zeros passes.
- `crates/aquila-sensing/src/engine.rs:317,335` and the `record` calls throughout `process_batch` use `self.config.measurement.commit`.

**Why it is a defect.** The entire ledger contract is “commit-keyed” (`ledger.rs:1-7`),
but any measurement taken without the composition root overriding the default is keyed
to `0000…`, i.e. attributed to no build. That is a fabricated provenance.

**Fix.** Make `commit` obligatory (no default), or have the core refuse to record when it
is all-zero.

---

### S16 — `Option`→default-zero (and silent fallback) hides “unknown” in the pipeline — **class 9** — `medium` — [definite]

- `crates/aquila-signal/src/process.rs:426-429` — `resample_uniform(...).unwrap_or_else(|_| amp_series[b].clone())` (and the phase series) silently substitutes the **non-uniform** series on irregularity instead of surfacing it, contrary to `R3` (“gaps not invented”).
- `crates/aquila-sensing/src/features.rs:54-58` — a failed `aquila_signal::process` becomes `(0.0, false)`; the `false` flag is sometimes consulted (`engine.rs:571`) and sometimes not.
- `crates/aquila-sensing/src/calibration.rs:1179-1182` — `map_or(0, …)` for band/channel/subcarriers when no analysis exists.

**Why it is a defect.** A downstream reader sees a genuine-looking amplitude/motion
number where the transform actually failed. Several of these are flagged elsewhere
(`used_signal`), but the `unwrap_or_else` substitution is unflagged.

**Fix.** Propagate the error (or an explicit `Partial`/`Flagged` state) rather than
substituting the raw series; make the caller check `used_signal` where it currently does
not.

---

### S17 — Recorded sessions do not preserve the received bytes, contradicting the container contract — **class 5 (also 7)** — `medium` — [definite]

- `crates/aquila-session/src/container.rs:16-18` — “`datagram` is the **exact on-wire** `AQCI` bytes”.
- `crates/aquila-capture/src/record.rs:4-9,95-98` — “`record` **re-encodes** it through `encode_datagram` … the tool does not claim to preserve bytes a forward-compatible decoder ignored.”

**Why it is a defect.** A recorded `.aqs` re-serialises decoded frames, so a wire codec
bug (wrong field, dropped trailing bytes, clamping) is invisible in the recording —
precisely the kind of bug #36 was. The container’s own doc asserts the opposite of what
`record` does.

**Fix.** Store the received datagram bytes verbatim in the `.aqs` record; reserve
re-encoding for the `synth` path only.

---

### S18 — Stale references to the superseded f32/ADR-0004 representation — **class 7** — `low` — [definite]

- `crates/aquila-sensing/src/bridge.rs:4-24` — “ordered `amplitude`/`phase` arrays … mismatched `amplitude`/`phase` lengths, or a non-finite sample” — `CsiFrame` no longer stores amplitude/phase, and `decode_csi`’s errors are `EmptyFrame`/`OddIqLength`.
- `docs/spec/capture-and-firmware.md:266` — still cites “the `data-contracts.md` §9.1 layout carried under the `AQCI` magic (**ADR-0004**)” as the authority for the payload, although ADR-0004’s payload is superseded by ADR-0005.
- `crates/aquila-wire/src/constants.rs:11-13` — “`data-contracts.md` §9.1 spells the same canonical body `AQCS`”.

**Why it is a defect.** The #36/#16 remediation was applied to code and ADRs but not the
prose; the docs now describe the representation the code no longer uses, which is how the
next agent reintroduces the mistake. Not a runtime defect.

**Fix.** Update `bridge.rs` docs and the two spec references to ADR-0005 / `AQCI`.

---

### S19 — `MetricKind`/`Grade` and three confidence newtypes duplicate one concept each — **class 3** — `low` — [definite]

- `crates/aquila-types/src/ledger.rs:17-27` `MetricKind { Deterministic, Statistical, Gated }` vs `crates/aquila-metrics/src/measurement.rs:41-51` `Grade { Deterministic, Statistical, Gated }` — identical variant sets.
- `crates/aquila-types/src/primitives.rs:127-258` — `Confidence`, `Quality`, `NodeQuality` are three near-identical `[0,1]` newtypes (the third added for the per-node quality work in #19).
- `crates/aquila-metrics/src/measurement.rs:26-39` `Authority` overlaps `Witness`.

**Why it is a defect.** Two declarations of the epistemic class can drift; three
confidence types force conversions and admit inconsistent validation.

**Fix.** Collapse `Grade`/`MetricKind` to one type; unify the confidence newtypes behind
one `UnitScalar` with named aliases.

---

### S20 — The `decode_record`/`RxMeta` record path is unreachable from the pipeline — **class 5** — `low` — [decision]

- `crates/aquila-signal/src/decode.rs:277-343` `decode_record` and `RxMeta` are only exercised by unit tests (`tests.rs`); the live and contract paths go through `decode_csi` (`bridge.rs:26`). The firmware’s `first_word_invalid` handling in `decode_record` is therefore dead code, while the live path hardcodes the flag `false` (S3).

**Why it is a defect (candidate).** Two decoders exist for the same hardware buffer, one
unused; the unused one carries the only correct-looking `first_word_invalid` logic. Either
delete it and fix the live path, or route the live path through it.

**Fix.** Decide whether the record-level path is the intended ESP32 decoder; if not,
delete it and fold its flag handling into the contract path.

---

## Counts per class

A finding is counted under its primary class; cross-class tags are shown in the table.

| Class | Findings | Count |
| --- | --- | --- |
| 1 Representation mismatch | S1, S2, S12 | 3 |
| 2 Derivation in the wrong place | S7, S8 | 2 |
| 3 Duplicate concepts | S7, S13, S19 | 3 |
| 4 Fake/stub/placeholder as real | S6, S14 | 2 |
| 5 Untested critical path | S4, S5, S10, S17, S20 | 5 |
| 6 Fabricated/ungraded numbers | S4, S5, S9, S10, S15 | 5 |
| 7 Silent spec divergence | S2, S6, S18 | 3 |
| 8 Hardcoded magic values | S9, S11, S12, S15 | 4 |
| 9 Honesty gaps | S3, S10, S14, S16 | 4 |

Total distinct findings: **20** (several serve multiple classes). Highest-severity
overlaps: S4 is class 6 + 5 + 2 and is the second #36; S10 is class 5 + 6 + 9; S2 is
class 1 + 7.

## Top 10 by severity

| Rank | Id | Severity | One line |
| --- | --- | --- | --- |
| 1 | **S4** | critical | Coarse pose is keyed to the simulator’s synthetic 3/5/7/9 Hz dither; the gated PCK@20 ≥ 65 % test is circular — **second #36** |
| 2 | **S1** | critical | Two contradictory ESP32 I/Q byte orders, both frozen by tests; one is wrong for every live frame |
| 3 | **S5** | critical | The three v1 gated bars (F1 ≥ 0.9, PCK@20 ≥ 65 %, MAE ≤ 2 brpm) are asserted on synthetic-only fixtures |
| 4 | **S6** | high | Subcarrier selection’s noise power is hardcoded zero, so the documented SNR selection is a fake |
| 5 | **S8** | high | Pose DSP and torso fusion are implemented in the stateful core, bypassing the inference seam |
| 6 | **S9** | high | Presence threshold is tuned to the simulator’s amplitude scale yet is the uncalibrated production default |
| 7 | **S10** | high | Simulator int8 quantisation saturates the range and fabricates RSSI — the only model of the hardware form is unphysical |
| 8 | **S2** | high | `first_word_invalid` invalidates 4 bytes per the firmware spec but 2 bytes in every implementation |
| 9 | **S3** | high | `CsiFrame`/wire drop the `first_word_invalid` flag; the core cannot see impaired captures |
| 10 | **S14** | medium | All-zero digests (`config_hash`, firmware sha, ground-truth sha) presented as content addresses |

## Looks like a second #36?

**Yes — S4 is the second #36.** The pattern is identical in kind: a canonical algorithm
(the coarse-pose estimator) was frozen against a *synthetic* representation rather than
the hardware, the simulator was built to emit exactly that representation
(`TORSO_DITHER_HZ = [3.0, 5.0, 7.0, 9.0]`), and so the test suite can never falsify it —
exactly as the old simulator synthesised amp/phase directly and never exercised the int8
path. It additionally emits `Witness::Measured` and asserts the v1 gated torso-PCK@20 bar.

**S1 is a second #36 of the representation axis.** After ADR-0005 fixed the *sample
encoding* (int8 vs f32), the *byte order within* that sample is still frozen two
mutually-contradictory ways, with the hardware-native order unverified and both
behaviours pinned by tests — the same “frozen without checking the hardware’s native
form” root cause recorded for #16/#36.

The generalised lesson from #36 holds and these findings restate it: **the canonical
representation (and any algorithm keyed to it) must be the hardware’s native form, and
the simulator must be able to fail.** S4 and S10 are where it currently cannot.
