# Sister-issues audit — Aquila specification set

Scope: `docs/spec/` (all), `docs/adr/` (all), `docs/reference/`, `CONTEXT.md`, and the
README/AGENTS docs. Read-only; nothing was changed except this file.

Reference finding: **#36** (`docs/plan/orchestrator-omissions.md`) — a representation
was frozen in `data-contracts.md` §9.1 and ADR-0004 that contradicted the hardware
(esp-csi emits int8 I/Q), later remediated by ADR-0005. This audit hunts the same
classes of defect elsewhere.

Method: every spec, ADR, the register, the glossary and the index/READMEs were read;
cross-references were checked against each other, against `docs/reference/sources.json`,
and against the actual crate tree (`crates/`, `Cargo.toml`, `tools/dag.toml`,
`xtask/`, `rust-toolchain.toml`).

## Summary of counts

| Class | Count | IDs |
| --- | --- | --- |
| 1. Cross-spec contradiction | 23 | D1–D23 |
| 2. Contract-vs-hardware mismatch | 5 | D24–D28 |
| 3. Stale content | 8 | D29–D36 |
| 4. Unsupported claim | 4 | D37–D40 |
| 5. Missing specification | 5 | D41–D45 |
| 6. Vocabulary drift | 5 | D46–D50 |
| 7. Register/reality mismatch | 5 | D51–D55 |
| 8. Ambiguity forcing a guess | 5 | D56–D60 |
| **Total** | **60** | |

---

## Class 1 — Cross-spec contradiction

### D1 — The `.aqs` session container is defined twice, incompatibly
- **Files:** `docs/spec/data-contracts.md:903` vs `docs/spec/capture-and-firmware.md:509`
- **Quote (data-contracts §9.2):** `stream.bin  # repeated: u32 LE len + postcard(CsiEnvelope)`; also `:243` “postcard-encoded `CsiEnvelope` inside a session archive”.
- **Quote (capture-and-firmware §8.1):** `"AQS" magic | u8 version | u32 metadata_len | metadata … "AQE" trailer | u64 record_count | sha256(record stream)` and `CSI frame | u64 host_arrival_us + the exact on-wire AQCI datagram bytes`.
- **Why it is a defect:** two mutually exclusive containers for the same artefact. `data-contracts.md` claims to be the single source of truth (“tie-breaker”), yet the implemented crate (`crates/aquila-session/src/container.rs`) and `capture-and-firmware.md` follow the AQS/AQE exact-bytes form. A builder cannot satisfy both.
- **Severity:** high
- **Fix:** make `data-contracts.md` §9.2 and §2 describe the AQS/AQE container (delegate detail to `capture-and-firmware.md` §8); delete the `stream.bin`/postcard wording, or open an ADR that chooses one.

### D2 — `Witness` has three grades in `CONTEXT.md` and five everywhere else
- **Files:** `CONTEXT.md:130` vs `docs/spec/data-contracts.md:134`, `docs/spec/evaluation.md:266`, `docs/spec/inference-models.md:23`
- **Quote (CONTEXT):** “Every sensing claim carries exactly one grade: - **Measured** … - **Claimed** … - **Gated**”.
- **Quote (data-contracts):** “There are exactly **five** grades … this is the *single* authoritative set (there is no three-grade set …)”.
- **Why it is a defect:** this is precisely the audit #10 contradiction, now reversed but not reconciled. `CONTEXT.md` is declared canonical and read first; it still defines three. `data-contracts.md` asserts the three-grade set does not exist while `CONTEXT.md` defines it.
- **Severity:** high
- **Fix:** update `CONTEXT.md`’s **Witness** entry to the five grades (keeping “Gated” first-class), or add an explicit ADR reconciling glossary and contracts.

### D3 — `NodeId` is declared three different ways
- **Files:** `docs/spec/signal.md:57` (`pub struct NodeId(pub u32);`), `docs/spec/capture-and-firmware.md:94` (`pub type NodeId = MacAddr; // [u8; 6]`), `docs/spec/data-contracts.md:85` (`pub struct NodeId(pub u16);`)
- **Why it is a defect:** the register says `NodeId` is declared exactly once in `aquila-types` (`interface-register.md:33`); two specs declare conflicting identities (u32 vs 6-byte MAC vs dense u16). The wire header is u16 and the MAC is translated at the seam (ADR-0003 §3), so both sibling declarations are wrong.
- **Severity:** high
- **Fix:** strike the `NodeId` re-declarations in `signal.md` §2 and `capture-and-firmware.md` §3.1; reference `aquila_types::NodeId` (u16) and note `MacAddr` is seam-local.

### D4 — `NodePair` is ordered in one spec and unordered in another
- **Files:** `docs/spec/data-contracts.md:389` vs `docs/spec/geometry-calibration.md:64`
- **Quote (data-contracts):** “An **ordered** node-pair … `tx` transmits, `rx` measures … Direction is significant (CSI is directional).”
- **Quote (geometry-calibration):** “A **NodePair** (link) is the unit of channel observation: the unordered pair `{a, b}` with a canonical id (`a-b`, lexicographic).”
- **Why it is a defect:** two incompatible identity models for the same frozen concept; affects pair enumeration, dedup and calibration keys.
- **Severity:** high
- **Fix:** geometry-calibration pivots on `tx`/`rx` order (or add an ADR choosing unordered and a second directional field).

### D5 — `PairScope` variant names disagree
- **Files:** `docs/spec/data-contracts.md:402` (`Room, SameFloor, CrossFloor`) vs `docs/spec/geometry-calibration.md:455` (`intra_room | inter_room | inter_floor`)
- **Why it is a defect:** the same enum serialised two ways across contract and calibration artefact.
- **Severity:** medium
- **Fix:** use the frozen `PairScope` discriminants in geometry-calibration’s schema.

### D6 — The `Clock` trait has two different signatures
- **Files:** `docs/spec/capture-and-firmware.md:230` (`fn now_us(&self) -> u64;`) vs `docs/spec/fusion-sync.md:50` (`fn now(&self) -> CoreTime;`)
- **Why it is a defect:** the one seam every adapter injects has two shapes. The actual code (`crates/aquila-types/src/time.rs:70`) matches fusion-sync; capture-and-firmware is stale, but a builder reading the register’s seam list plus capture-and-firmware gets `now_us`.
- **Severity:** medium
- **Fix:** correct `capture-and-firmware.md` §4 to `fn now(&self) -> CoreTime` and delete the `SystemClock/FakeClock` `now_us` sketch.

### D7 — Device clock units: microseconds or nanoseconds?
- **Files:** `docs/spec/capture-and-firmware.md:107` (`device_ts_us: u64` “microseconds”), `:238` (`esp_timer_get_time`, microseconds) vs `docs/spec/fusion-sync.md:30` (“nanoseconds since node boot”) and `:179` (`DeviceTime` ns)
- **Why it is a defect:** the wire field is `device_time_us` (µs, `data-contracts.md:201`), but fusion-sync models the device clock in ns. Without a stated conversion, offset/drift maths is ambiguous.
- **Severity:** medium
- **Fix:** state the µs↔ns bridge explicitly in `fusion-sync.md` §1.1 and `data-contracts.md` §10.

### D8 — `Calibration` is three different shapes
- **Files:** `docs/spec/data-contracts.md:946` (frozen struct: `home, contract, topology_hash, node_offsets_db, pair_offsets, validity…`) vs `docs/spec/geometry-calibration.md:419` (JSON artefact: `schema_version, calibration_id, nodes[], links[], zones[]…`) vs `docs/spec/signal.md:96` (`Calibration { per_node: Vec<Complex32>, state }`)
- **Why it is a defect:** `data-contracts.md` §8 lists `Calibration` on the never-redefine list, yet two specs define materially different `Calibration`s (one runtime, one on-disk, one per-node multiplier), and the field names do not line up (`node_positions_m`/`node_offsets_db` vs `nodes[].placement`/`clock` vs `per_node`).
- **Severity:** high
- **Fix:** mark signal’s as an internal type with a non-colliding name (e.g. `NodeCorrection`); declare which of data-contracts vs geometry is the canonical artefact and make the other cite it.

### D9 — `Band` name collision (RF band vs filter band)
- **Files:** `docs/spec/signal.md:113` (`pub struct Band { pub lo_hz, pub hi_hz }`) vs `docs/spec/data-contracts.md:126` (`pub enum Band { B2G4 = 0, B5G = 1, Unknown }`)
- **Why it is a defect:** the frozen contract `Band` is the 2.4/5 GHz enum; signal reuses the name for a frequency interval. `data-contracts.md` §8 freezes `Band`; `interface-register.md` names only `ComplexFrame` as signal’s internal type.
- **Severity:** medium
- **Fix:** rename signal’s to `FreqBand`, or mark it internal and note the collision is deliberate.

### D10 — The ledger row and the accuracy metric names disagree across four specs
- **Files:** `docs/spec/data-contracts.md:764` (`LedgerRecord`), `docs/spec/measurement.md:50` (`Measurement`), `:303` (`accuracy.torso_pck_at_20`, `accuracy.presence_f1`, `accuracy.breathing.mae_brpm`), `docs/spec/evaluation.md:351` (`pose.pck20.torso`, `presence.f1`, `breathing.mae`), `docs/spec/core-api.md:542` (`presence.f1`, `torso.pck20`, `vitals.breathing.mae`), `docs/spec/roadmap.md:60` (`presence.count.mae`)
- **Why it is a defect:** `LedgerRecord` and `Measurement` are two incompatible row types for one ledger; the metric identifiers differ four ways for the same three bars, so a gate/citation cannot be traced.
- **Severity:** high
- **Fix:** pick one row type (`LedgerRecord` is on the §8 frozen list) and one metric-id vocabulary; have `measurement.md`, `evaluation.md`, `core-api.md`, `roadmap.md` all cite it.

### D11 — Ledger path: `measure/` or `.aquila/metrics/`?
- **Files:** `docs/spec/measurement.md:190` (`.aquila/metrics/ledger.jsonl`) vs `docs/spec/data-contracts.md:895` (`measure/ledger.jsonl`)
- **Why it is a defect:** `repository-tooling.md` §13.2 admits the divergence but nothing has been corrected at the source; two specs still disagree on where the ledger lives.
- **Severity:** medium
- **Fix:** update `data-contracts.md` §9.2 to `.aquila/metrics/ledger.jsonl` (or bless one in an ADR) and remove the §13 footnote dependence.

### D12 — Two competing `CsiFrame` shapes for the one seam type
- **Files:** `docs/spec/capture-and-firmware.md:103` (struct with `node/boot/sequence/device_ts_us/host_arrival_us/tsf_us/phy/first_word_invalid/source_mac/csi: CsiBuffer`) vs `docs/spec/data-contracts.md:250` (frozen `CsiFrame` with `node_id/boot_epoch/sequence/device_time_us/band/channel/…/i_q`)
- **Why it is a defect:** the register says `CsiFrame` is declared exactly once (`interface-register.md:23`); capture-and-firmware still prints a second, incompatible struct in its seam section. This is the same class as audit #11 (the old signal `CsiFrame`), only the surviving copy moved.
- **Severity:** high
- **Fix:** replace `capture-and-firmware.md` §3.1’s `CsiFrame` with a reference to `aquila_types::CsiFrame` and move host-arrival metadata into the `CsiEnvelope` note.

### D13 — The private HTTP+WS control plane has three different endpoint sets
- **Files:** `docs/spec/core-api.md:174` (`/node-pairs`, `/calibration`, `/calibration/{sessionId}/steps/...`, `/metrics/snapshot`, `/capabilities`) vs `docs/spec/dashboard.md:148` (`/pairs`, `/calibrations`, `/calibrations/{id}/actions`, `/measurements`, `/system/health`) vs `docs/spec/home-assistant.md:271` (`/schema`, `/state`, `/api/v1/firmware…`)
- **Why it is a defect:** three specs claim to be the contract for the same private plane and none agrees on route names (`node-pairs` vs `pairs`, `calibration` vs `calibrations`, `metrics/snapshot` vs `measurements`, `health` vs `system/health`). The dashboard and the HACS component both depend on it; a builder cannot implement it without guessing.
- **Severity:** critical
- **Fix:** make `core-api.md` the single endpoint contract; rewrite `dashboard.md` §5 and `home-assistant.md` §5.2 to consume it; remove duplicate tables.

### D14 — The control plane requires auth in one spec and forbids-by-default in another
- **Files:** `docs/spec/core-api.md:55` vs `docs/spec/home-assistant.md:295`
- **Quote (core-api §3):** “Every request MUST be authenticated. Exactly two trust modes exist; there is no anonymous mode except the liveness probe in §4.”
- **Quote (home-assistant §5.2):** “since 8099 is not published to the LAN, the default is trusted local access with **no token**.”
- **Why it is a defect:** the security model of the one private API is directly contradicted. `privacy-security.md` G5 requires ingress identity validation on every request; home-assistant’s default-off auth violates G5.
- **Severity:** critical
- **Fix:** adopt core-api §3 as the contract; state in home-assistant that ingress headers (or the loopback token) are mandatory, and drop “no token”.

### D15 — Firmware endpoints exist in three specs and are denied in the API spec
- **Files:** `docs/spec/interface-register.md:89` and `docs/spec/home-assistant.md:277` and `docs/spec/firmware-ota.md:109` add `/api/v1/firmware*`; `docs/spec/core-api.md:215` says “No other routes exist.”
- **Why it is a defect:** the register and two specs rely on firmware routes the authoritative API table forbids. A builder following core-api would omit an endpoint the OTA ticket (#15) requires.
- **Severity:** high
- **Fix:** add the OTA routes to `core-api.md` §6 (or explicitly defer them there with a pointer to `firmware-ota.md`).

### D16 — Three WebSocket protocol shapes
- **Files:** `docs/spec/core-api.md:332` (`{v,type,seq,ts,id,data}` envelope, `subscribe` with `kinds`+`scope`) vs `docs/spec/dashboard.md:186` (`{type,id,topics}` + `StreamEvent{snapshot/update}`) vs `docs/spec/home-assistant.md:286` (`{type,seq,ts,zone_id,measurements}`)
- **Why it is a defect:** the stream contract is three different protocols; the dashboard and component cannot both be right.
- **Severity:** high
- **Fix:** adopt core-api §10 as the stream contract; rewrite dashboard §5 and home-assistant §5.2 against it.

### D17 — The crate graph and levels in three docs are the pre-ADR-0003 world
- **Files:** `docs/spec/architecture.md:38` (six layers, 11 crates, no `aquila-wire/session/simulator`), `docs/spec/repository-tooling.md:211` (“eleven `aquila-*` modules”), `docs/spec/repository-tooling.md:286` (dag levels: `sources=1, sensing=2, ha=3, api=4, dashboard=5`) vs `docs/spec/interface-register.md:31` (14 product crates; `wire=1 … sources=3, simulator=4, sensing=5, evaluation=6, ha=6, api=7, dashboard=8`) and `tools/dag.toml`.
- **Why it is a defect:** ADR-0003 §1 added `aquila-wire`, `aquila-session`, `aquila-simulator` and `aquila-capture`; two specs still present the superseded graph and a superseded `tools/dag.toml` body. This is the exact “ADR-0003 not applied everywhere” the brief warns about.
- **Severity:** high
- **Fix:** rewrite `architecture.md` §2/§3/§5 and `repository-tooling.md` §3 (including the embedded `dag.toml`) from the register; or mark both sections non-normative with a pointer.

### D18 — Seam adapter names disagree across five specs
- **Files:** `docs/spec/architecture.md:199` (`UdpCsiSource`, `ReplayCsiSource`, `LedgerMetricSink`) vs `docs/spec/interface-register.md:63` (`UdpLiveSource`, `ReplaySource`, `JsonLedgerSink`) vs `docs/spec/capture-and-firmware.md:170` (`ReplaySource`, `UdpLiveSource`) vs `docs/spec/geometry-calibration.md:216` (`FakeCsiSource`)
- **Why it is a defect:** the register is authoritative on seams/adapters; architecture and geometry use names that never appear in it, and no `FakeCsiSource` appears in the register at all.
- **Severity:** medium
- **Fix:** converge on register names; replace `FakeCsiSource` with the register’s `ReplaySource`/testkit naming.

### D19 — `CsiSource` method is `next` in one spec and `recv` in others
- **Files:** `docs/spec/architecture.md:58` (`CsiSource::next`) and `:231` vs `docs/spec/interface-register.md:40` (`CsiSource::recv/shutdown`) and `docs/spec/capture-and-firmware.md:144`
- **Why it is a defect:** the one pull method is named differently in the architecture doc.
- **Severity:** medium
- **Fix:** correct architecture.md §3/§7 to `recv`/`shutdown`.

### D20 — `GroundTruth`/`MetricSink` adapter names disagree
- **Files:** `docs/spec/evaluation.md:94` (`FixtureLabelGroundTruth`, `CameraRigGroundTruth`) vs `docs/spec/interface-register.md:66` (`FixtureGroundTruth`, `CameraGroundTruth`) vs `docs/spec/architecture.md:202` (`FixtureGroundTruth`, `CameraGroundTruth`); `docs/spec/measurement.md:180` (`MemorySink`) vs register `InMemoryMetricSink`
- **Why it is a defect:** adapter identities used in the ledger and tests differ from the register.
- **Severity:** medium
- **Fix:** use register names throughout.

### D21 — `architecture.md` speaks a superseded type vocabulary
- **Files:** `docs/spec/architecture.md:54` (`RawCsiFrame`, `CleanCsi`, `FeatureVector`, `Correction`, `Observation`, `GroundTruthSample`, `MetricRecord`) and `:55` (`clean(raw, &Correction, &SignalConfig) -> CleanCsi`)
- **Why it is a defect:** none of `RawCsiFrame`/`CleanCsi`/`Correction`/`Observation`/`MetricRecord` exist in the register or in `aquila-types`; the canonical interfaces are `CsiFrame`, `decode_csi`, `FeatureVector`, `Detection`/`Event`, `LedgerRecord`.
- **Severity:** medium
- **Fix:** replace the §3 table’s interface column with the register’s public-interface summaries.

### D22 — `measurement.md` says nothing depends on `aquila-metrics`; the register says the core does
- **Files:** `docs/spec/measurement.md:35` (“nothing in the DSP, inference, fusion or calibration graph depends on it”) and `:442` vs `docs/spec/interface-register.md:105` (`aquila-sensing -> … aquila-metrics`) and `tools/dag.toml`
- **Why it is a defect:** the measurement spec’s placement claim is contradicted by the authoritative edges it is supposed to sit under.
- **Severity:** medium
- **Fix:** update measurement.md §1/§8 to match the register (the core records per-step `Measurement` rows).

### D23 — `MemorySink` vs `InMemoryMetricSink` (and `RecordingMetricSink`)
- **Files:** `docs/spec/measurement.md:180` and `docs/spec/inference-models.md:502`
- **Why it is a defect:** minor, but three names circulate for the same fake sink.
- **Severity:** low
- **Fix:** standardise on the register’s `InMemoryMetricSink`.

---

## Class 2 — Contract-vs-hardware mismatch

### D24 — `capture-and-firmware.md` §3.1 still freezes `NodeId = MacAddr` on the wire
- **File:** `docs/spec/capture-and-firmware.md:93`
- **Quote:** “Stable node identity: the ESP32-S3 factory base MAC (six bytes) … it is the only node id on the wire. `pub type NodeId = MacAddr;`”
- **Why it is a defect:** ADR-0003 §3 settled the opposite — “the dense `NodeId(u16)` in `data-contracts.md` is the canonical *contract* identity and `aquila-wire` maps between them at the seam” — and the §5.1 header (`node_id u16`) already implements that. §3.1 was left behind when §5.1 was fixed, so the seam section contradicts both the wire it introduces and the hardware mapping. **This is the closest sibling of #36: a representation frozen in a spec that contradicts the reconciled hardware/contract.**
- **Severity:** high
- **Fix:** delete the `MacAddr` type alias; describe the u16 dense id and the MAC→u16 `NodeIdentityMap` at the seam.

### D25 — `capture-and-firmware.md` §3.1’s `CsiFrame` carries fields the wire and envelope model forbid
- **File:** `docs/spec/capture-and-firmware.md:103`
- **Quote:** `pub tsf_us: Option<u32>` … `pub host_arrival_us: u64` … `pub csi: CsiBuffer`.
- **Why it is a defect:** §5.3 (`:330`) states “The wire carries no TSF (ADR-0004 replaced the earlier 56-byte header and its optional `tsf_us`)”, and `data-contracts.md` §2 places host arrival on `CsiEnvelope`, not on the frame. The seam struct still assumes the superseded 56-byte layout.
- **Severity:** high
- **Fix:** align §3.1 with `aquila_types::CsiFrame`/`CsiEnvelope`.

### D26 — `torso-PCK@20` uses keypoints that are not in the frozen COCO-17 schema
- **Files:** `docs/spec/evaluation.md:187` vs `docs/spec/data-contracts.md:442`
- **Quote (evaluation):** “the **torso keypoint set** — neck, left/right shoulder, left/right hip, pelvis”.
- **Why it is a defect:** `data-contracts.md` freezes `KeypointId` to COCO-17 (nose, eyes, ears, shoulders, elbows, wrists, hips, knees, ankles) — there is **no** neck and **no** pelvis keypoint. The metric cannot be computed on the frozen schema; the same doc then admits the schema is “to be fixed by ADR” (`:412`).
- **Severity:** high
- **Fix:** define the torso set from COCO-17 (e.g. shoulders + hips, with the neck/pelvis derived midpoints stated as derived), and freeze it with an ADR.

### D27 — `signal.md` decode cap (`raw.len() <= 612`) contradicts ADR-0005’s subcarrier ceiling
- **Files:** `docs/spec/signal.md:229` vs `docs/adr/0005-canonical-csi-representation.md:89`
- **Quote (signal D1):** “`raw.len() <= 612`”; **Quote (ADR-0005):** “`MAX_SUBCARRIERS` rises from 178 to 714 for the same 1472-byte datagram budget”.
- **Why it is a defect:** the ADR moved the canonical int8 ceiling to 714 subcarriers (1472-byte frame); signal.md still hard-caps the decode at 306 subcarriers (612 bytes), a leftover from the f32 era.
- **Severity:** medium
- **Fix:** set the cap from the canonical `MAX_SUBCARRIERS` and the datagram budget in one place.

### D28 — `capture-and-firmware.md` §11 keeps “Node identity = MAC on the wire” an open decision
- **File:** `docs/spec/capture-and-firmware.md:668`
- **Quote:** “**Node identity.** The wire and home model use the factory base MAC as the stable node id”.
- **Why it is a defect:** ADR-0003 §3 already decided this; the “open decision” section re-opens a settled interface and contradicts the header in the same document.
- **Severity:** high
- **Fix:** remove §11.1 (and §11.4’s port restatement where superseded) or mark them “settled by ADR-0003”.

---

## Class 3 — Stale content

### D29 — `repository-tooling.md` §3 is the pre-ADR-0003 crate graph and DAG
- **File:** `docs/spec/repository-tooling.md:211`
- **Quote:** “**eleven `aquila-*` modules** in six topological levels”; the embedded `tools/dag.toml` (`:276`) lists `aquila-sources = 1`, `aquila-sensing = 2`, `aquila-ha = 3`, `aquila-api = 4`, `aquila-dashboard = 5` and omits `aquila-wire/session/simulator`.
- **Why it is a defect:** the register says fourteen product crates and the real `tools/dag.toml` has different levels; this document presents the superseded data as authoritative.
- **Severity:** high
- **Fix:** regenerate §3 from `interface-register.md`/`tools/dag.toml` (see D17).

### D30 — `repository-tooling.md` pins Rust 1.99.0; the repo pins 1.98.1
- **Files:** `docs/spec/repository-tooling.md:135` and `:171` (`rust-version = "1.99.0"`, `channel = "1.99.0"`) vs `rust-toolchain.toml` (`1.98.1`), `Cargo.toml` (`rust-version = "1.98"`), `AGENTS.md:12`, `flake.nix` (`1.98.1`)
- **Why it is a defect:** a builder following the tooling spec installs a compiler the repo does not pin; the reference-pipeline determinism argument depends on the pin.
- **Severity:** high
- **Fix:** set both to the actual pinned toolchain (1.98.1).

### D31 — `docs/spec/README.md` says “Nine tracer-bullet slices”; the roadmap has ten
- **File:** `docs/spec/README.md:27` vs `docs/spec/roadmap.md:225`
- **Quote:** “`roadmap.md` | Nine tracer-bullet slices and the v1 exit criteria.”
- **Why it is a defect:** Slice 10 (Firmware OTA) was added; the index count is stale.
- **Severity:** medium
- **Fix:** change to “ten tracer-bullet slices (1–10)”.

### D32 — `docs/README.md` file and ADR counts are stale
- **File:** `docs/README.md:40`
- **Quote:** “Specs: 18 files, including `firmware-ota.md`” and `:42` “ADRs: `0001`–`0004`.”
- **Why it is a defect:** there are 19 spec topic files (20 including the index) and five ADRs (`0005` exists).
- **Severity:** medium
- **Fix:** update to “19 spec files” and “ADRs `0001`–`0005`”.

### D33 — `measurement.md` cites the allow-list as `aquila-dag.toml`
- **File:** `docs/spec/measurement.md:259` (and `:8` “Crate: `aquila-metrics`”)
- **Quote:** “`cargo metadata` vs committed allowlist `aquila-dag.toml`”
- **Why it is a defect:** the file is `tools/dag.toml` (register, ADR-0002, repository-tooling).
- **Severity:** medium
- **Fix:** correct the path.

### D34 — `interface-register.md` cites a superseded port in `core-api.md`
- **File:** `docs/spec/interface-register.md:81`
- **Quote:** “`8765` (`core-api.md` §2) … superseded”
- **Why it is a defect:** `core-api.md` §2 now specifies 8099; the parenthetical is a stale pointer.
- **Severity:** low
- **Fix:** drop the `8765 (core-api.md §2)` note or reword to “formerly `8765`”.

### D35 — The HA tree in `repository-tooling.md` does not match the repo or `home-assistant.md`
- **Files:** `docs/spec/repository-tooling.md:50` (`ha/aquila_core/`, `ha/custom_components/aquila/`) vs `docs/spec/home-assistant.md:190` (`aquila_core/`, `custom_components/aquila/` at root) vs the actual repo root (`aquila_core/`, `custom_components/`)
- **Why it is a defect:** repository-tooling claims to have reconciled the layout to `ha/`, but neither `home-assistant.md` nor the tree reflects it; the reconciliation is unilateral and unlanded.
- **Severity:** medium
- **Fix:** either land `ha/` in the repo and update home-assistant.md, or update repository-tooling §1/§1.2 to the actual root layout.

### D36 — `data-contracts.md` still says Session frames are postcard on disk
- **File:** `docs/spec/data-contracts.md:243`
- **Quote:** “On disk: postcard-encoded `CsiEnvelope` inside a session archive (§9).”
- **Why it is a defect:** same as D1; the durable representation is the AQS exact-wire container, not postcard.
- **Severity:** medium
- **Fix:** fold into D1’s correction.

---

## Class 4 — Unsupported claim

### D37 — `evaluation.md` §7 external pose bands contradict the only cited source
- **File:** `docs/spec/evaluation.md:374`
- **Quote:** “roughly **65–75%** torso-PCK@20 … Cross-subject (LOSO): roughly **60–65%** … Zero-shot cross-room … roughly **15–20%**”.
- **Why it is a defect:** no `sources.json` id is cited; the reference file actually records `hf-mmfi-pose` at “Torso-PCK@20 82.686 in-domain” (`docs/reference/resources.md:21`). The in-domain band is therefore contradicted by the project’s own reference and the rest is uncited.
- **Severity:** medium
- **Fix:** cite the source id and reconcile the band (or mark the whole §7 as `Claimed` with an explicit unpinned external origin).

### D38 — `fusion-sync.md` §1.5 accuracy budget has no reference, ledger row, or test
- **File:** `docs/spec/fusion-sync.md:150`
- **Quote:** “Single-node offset error (RMS) | ≤ 2 ms … Extrapolation error over a 10 s gap | ≤ 40 µs”.
- **Why it is a defect:** the spec’s own rules require every numeric claim to cite `docs/reference/` or a ledger row; this table cites neither, yet §2.7 turns some into required tests only implicitly. A builder cannot know if ≤2 ms is derived or aspirational.
- **Severity:** medium
- **Fix:** mark these as target/gate values with `claims.toml` ids and the ledger rows that measure them (or label `Claimed`).

### D39 — Simulator modulation constants are uncited
- **File:** `docs/spec/capture-and-firmware.md:471`
- **Quote:** “`A_b ≈ 3–8 mm`, `f_b = 0.1–0.5 Hz` … `A_h ≈ 0.05–0.2 mm`”.
- **Why it is a defect:** physical amplitude/rate ranges are stated as fact with no reference id or fixture; the simulator’s determinism test pins behaviour but not physical correctness.
- **Severity:** low
- **Fix:** cite a source id or label as modelling assumptions.

### D40 — Inference tensor dimension defaults are uncited and then re-opened
- **File:** `docs/spec/inference-models.md:160`
- **Quote:** “`L` | contributing links … | ≤ 3 per node-pair, ≤ 24 per zone”.
- **Why it is a defect:** asserted as v1 defaults in §1.4, then §8.2 says they “depend on firmware CSI rate … confirm against the ESP32-S3 capture path before freezing”.
- **Severity:** low
- **Fix:** label the table as provisional with the owning open question, or move it to §8.

---

## Class 5 — Missing specification

### D41 — `xtask` subcommands the specs require do not exist
- **Files:** `docs/spec/repository-tooling.md:793` (`cargo xtask coverage-floor`), `:797` (`cargo xtask claims-verify`), `:840` (`cargo xtask ledger-comment`), `docs/spec/evaluation.md:396` (`cargo xtask eval`), `docs/adr/0002-toolchain-and-repo.md:135`
- **Why it is a defect:** `xtask/src/main.rs` implements only `dag-check` and `check-scaffolding`. CI YAML in the spec would fail on the other three, and the evaluation reproducer is undefined. Audit #21/#33 are “resolved” for `dag-check` only.
- **Severity:** high
- **Fix:** implement or strike the missing subcommands; if struck, remove them from `repository-tooling.md` §10 and `evaluation.md`.

### D42 — `aquila-dashboard` is in the register and DAG but does not exist
- **Files:** `docs/spec/interface-register.md:46`, `tools/dag.toml` (`"aquila-dashboard" = 8`) vs `crates/` and `Cargo.toml`
- **Why it is a defect:** every other named crate exists; `aquila-dashboard` has a spec, a DAG entry and a ticket (#10) but no `crates/aquila-dashboard/` and no workspace member. This is the register/reality trap the orchestration policy §5 warns about, and it is the same omission class as audit #2 (originally `aquila-api`).
- **Severity:** high
- **Fix:** scaffold the crate (or strike it and defer dashboard until the ticket lands), and ensure `dag-check` fails on a registered-but-absent member.

### D43 — `claims.toml` is required by testing.md but absent
- **Files:** `docs/spec/testing.md:193`, `docs/spec/repository-tooling.md:45` (`claims.toml  # honest-claims registry`), `docs/spec/repository-tooling.md:797`
- **Why it is a defect:** the honest-claims bijection gate (G21) parses a registry file that does not exist; no claim ids are specified anywhere.
- **Severity:** medium
- **Fix:** add `claims.toml` (or make its creation part of ticket #11) and define `CLM-` ids in the specs that carry bars.

### D44 — The `NodeControl` trait is named but never specified
- **Files:** `docs/spec/interface-register.md:40` (`NodeControl::send`) and `:70` vs `docs/spec/capture-and-firmware.md:357` (§6 describes wire bodies, not the trait)
- **Why it is a defect:** the register exposes a `NodeControl::send` interface; no spec defines its signature, error type or lifecycle (only the datagram layout).
- **Severity:** low
- **Fix:** add the trait sketch to `capture-and-firmware.md` §6.

### D45 — The evaluation reproducer command is unspecified
- **File:** `docs/spec/evaluation.md:393`
- **Quote:** “Proposed shape of a reproducer (tooling TBD by ADR): `cargo xtask eval …`”
- **Why it is a defect:** a required ledger field (“reproducer”, §5.1) points at a command no spec or tool defines.
- **Severity:** medium
- **Fix:** specify the command and its outputs, or mark it out of v1.

---

## Class 6 — Vocabulary drift

### D46 — `geometry-calibration.md` calls a node-pair a “link” throughout
- **File:** `docs/spec/geometry-calibration.md:56` (and `link`/`link_id`/`contributing_links` passim)
- **Quote:** “└── NodePair (link)      unordered node pair = one CSI link” and `:453` “`link_id string canonical unordered "a-b"`”.
- **Why it is a defect:** `CONTEXT.md:69` defines **Node-pair** and explicitly lists `link`, `edge`, `mesh link` under “_Avoid_”. The calibration artefact thereby stores a forbidden synonym as a schema field name.
- **Severity:** medium
- **Fix:** rename `link`/`link_id` to `node_pair`/`pair_id` in the schema and prose.

### D47 — `geometry-calibration.md` treats Zone as a sub-room volume, against the v1 glossary
- **File:** `docs/spec/geometry-calibration.md:56`
- **Quote:** “└── Zone (sofa, bed, doorway)    named sub-volume in room frame”.
- **Why it is a defect:** `CONTEXT.md:57` says “v1 emits one zone per room”, and `data-contracts.md:96` makes `ZoneId = RoomId`. Geometry’s zone model is a future capability presented as the v1 model, and `zone_of` returning sub-room zones (`:246`) contradicts the contract.
- **Severity:** medium
- **Fix:** mark sub-room zones explicitly staged/future and have v1 `zone_of` return the room.

### D48 — `core-api.md` uses “storey” for floor
- **Files:** `docs/spec/core-api.md:176` (“storey count”), `:221` (“storey order”)
- **Why it is a defect:** `CONTEXT.md:48` defines **Floor** and lists `level`, `storey` under “_Avoid_”.
- **Severity:** low
- **Fix:** use “floor”.

### D49 — `measurement.md`’s `Authority` collides with the `Witness` vocabulary
- **File:** `docs/spec/measurement.md:92`
- **Quote:** `enum Authority { Measured, Replayed, Derived, Imported, Declared }`.
- **Why it is a defect:** `Measured`/`Replayed`/`Declared` overlap the canonical `Witness` grades but are a parallel enum on the ledger row, so two “evidence strength” vocabularies coexist; `CONTEXT.md:138` also forbids `ground truth`/`validated` synonyms that this area skirts.
- **Severity:** medium
- **Fix:** fold `Authority` into the `Witness`/`MetricKind` model or state the mapping explicitly and reconcile with D10.

### D50 — `docs/README.md`’s evidence-grade list omits `Gated`
- **File:** `docs/README.md:31`
- **Quote:** “numbers carry an evidence grade (measured / claimed / data-gated / hardware-gated).”
- **Why it is a defect:** the headline grade `Gated` is missing, compounding the three-vs-five confusion of D2.
- **Severity:** low
- **Fix:** list all five grades, strongest first.

---

## Class 7 — Register/reality mismatch

### D51 — Registered crate `aquila-dashboard` has no directory or workspace member
- Cross-reference: D42. Register and `tools/dag.toml` name it; `crates/` and `Cargo.toml` do not.
- **Severity:** high (counted once under D42; recorded here as the class-7 instance).

### D52 — `tools/dag.toml` will not detect a registered-but-absent crate
- **File:** `docs/spec/repository-tooling.md:418` (rule “`B` must be a registered member”) vs `xtask/src/dag.rs`
- **Why it is a defect:** the check validates members against the register, not the register against members; `aquila-dashboard` passes while absent. The guardrail’s stated purpose (“Every crate … must exist or be struck”, `orchestration.md:30`) is not enforced.
- **Severity:** medium
- **Fix:** add a rule that every name in `[levels]` is an actual member (or exempt it).

### D53 — `repository-tooling.md`’s `ha/` tree does not exist
- Cross-reference: D35. The spec’s §1 tree and §1.2 reconciliation are not reflected on disk.
- **Severity:** medium.

### D54 — `architecture.md`’s `addon/` composition root and root `custom_components/` do not match the workspace
- **File:** `docs/spec/architecture.md:261`
- **Quote:** `addon/  # composition root …` and `custom_components/aquila/`.
- **Why it is a defect:** the composition root is `crates/aquila-addon` (`Cargo.toml`), and the repo has `aquila_core/` plus `custom_components/` at root, not `addon/`.
- **Severity:** medium
- **Fix:** fold into D17/D35’s reconciliation.

### D55 — Register claims `tools/dag.toml` + `xtask` exist and run on every check; the `xtask` surface is only two subcommands
- Cross-reference: D41.
- **Severity:** medium.

---

## Class 8 — Ambiguity that would force a guess

### D56 — Evaluation reproducer is explicitly “TBD by ADR”
- Cross-reference: D45. `evaluation.md:393` leaves a required ledger field to be invented.
- **Severity:** medium.

### D57 — The torso keypoint schema is simultaneously asserted and “to be fixed by ADR”
- Files: `docs/spec/evaluation.md:187` vs `:412`
- **Quote:** “The exact torso keypoint schema and torso-span normaliser, to be fixed by ADR”.
- **Why it is a defect:** §4.1 states a concrete torso set and normaliser, then §9 says it is undecided. A builder must guess.
- **Severity:** medium
- **Fix:** resolve with the ADR and make §4.1 cite it.

### D58 — `stale_after_days: 90` is a placeholder in a live schema default
- **File:** `docs/spec/geometry-calibration.md:595` (and `:504` in the schema, `:580` example)
- **Quote:** “`stale_after_days`: 90 is a placeholder; needs a real drift study on S3s.”
- **Why it is a defect:** a placeholder is written into the calibration artefact default (`§13`), so a builder will ship 90 as if decided.
- **Severity:** low
- **Fix:** mark the field explicitly provisional and required to be set at runtime, not defaulted.

### D59 — ADR-0001/0002/0003 are still “Proposed” while the whole repo treats them as settled
- Files: `docs/adr/0001-module-architecture.md:4`, `docs/adr/0002-toolchain-and-repo.md:4`, `docs/adr/0003-interface-reconciliation.md:4` (all `Status: Proposed`), while `docs/plan/README.md:34` says “ADRs win over specs” and specs say “settled by ADR-0003”.
- **Why it is a defect:** an immutable-once-accepted ADR cannot be a tie-breaker while marked Proposed; a builder cannot tell which decisions are binding. Only ADR-0005 is `Accepted`.
- **Severity:** medium
- **Fix:** accept the ADRs (or mark the superseded parts) so the register’s authority is real.

### D60 — Dashboard ledger-view scope is an open question that changes the UI contract
- **File:** `docs/spec/dashboard.md:328`
- **Quote:** “Whether the ledger view should expose diffing between arbitrary commits in v1 or only the latest commit’s diff.”
- **Why it is a defect:** `core-api.md` §6 exposes only `GET /metrics/snapshot` (current commit), so the dashboard cannot resolve this without either inventing endpoints or dropping the feature; `dashboard.md` §5 already lists `GET /api/v1/measurements/{commit}`.
- **Severity:** low
- **Fix:** settle the ledger read contract once (in core-api) and point dashboard at it.

---

## Answers to the brief’s summary asks

### Counts per class

| Class | Count |
| --- | --- |
| 1. Cross-spec contradiction | 23 |
| 2. Contract-vs-hardware mismatch | 5 |
| 3. Stale content | 8 |
| 4. Unsupported claim | 4 |
| 5. Missing specification | 5 |
| 6. Vocabulary drift | 5 |
| 7. Register/reality mismatch | 5 |
| 8. Ambiguity forcing a guess | 5 |

### Top 10 by severity (all critical/high first)

1. **D13** — three incompatible private-API endpoint sets (critical).
2. **D14** — control-plane auth required vs default-off (critical).
3. **D2** — `Witness` three vs five grades (high).
4. **D1/D36** — two incompatible `.aqs` containers, one declared canonical (high).
5. **D24** — `NodeId = MacAddr` frozen at the seam, contradicts ADR-0003 and the wire (high; #36 sibling).
6. **D12/D25** — a second, superseded `CsiFrame` at the seam (high).
7. **D17/D29** — the pre-ADR-0003 crate graph and DAG still presented as authoritative (high).
8. **D8/D10** — `Calibration` and the ledger row have multiple incompatible shapes/names (high).
9. **D26** — torso-PCK@20 defined on keypoints absent from the frozen COCO-17 schema (high).
10. **D41/D42** — required `xtask` subcommands and the `aquila-dashboard` crate are absent (high).

### What looks like a second #36

**D24 (with D25) — `docs/spec/capture-and-firmware.md` §3.1.** When ADR-0005 fixed the wire (`§5.1`), the *seam* section above it was not touched: it still freezes `pub type NodeId = MacAddr` “the only node id on the wire”, a `boot: u32`, an optional `tsf_us`, and a node-side `csi: CsiBuffer`. That is the same failure mode as #36 — a representation frozen in a spec that contradicts (a) the reconciliation ADR (dense `NodeId(u16)`), (b) the frozen contract (`data-contracts.md` §2), (c) the hardware-derived wire (§5.1 has no TSF, 16-byte header), and (d) the actual `aquila_types::CsiFrame`. It is currently untested because the simulator and wire paths bypass this struct.

**Runner-up: D1 — `data-contracts.md` §9.2.** The self-declared “single source of truth” freezes the `.aqs` session container as a directory of `postcard(CsiEnvelope)` records, while `capture-and-firmware.md` §8 and the implemented `aquila-session` crate use the AQS/AQE exact-wire-bytes container. The canonical contract representation contradicts the shipped one — the same “frozen representation vs reality” shape as #36.

### Systemic read

The same root cause as #36 recurs: **a reconciliation (ADR-0003/0004/0005) landed in one file and was not propagated to the siblings.** The register, `tools/dag.toml`, `data-contracts.md` §9.1, `capture-and-firmware.md` §5.1 and `aquila-types` were updated; `architecture.md`, `repository-tooling.md`, `capture-and-firmware.md` §3.1/§11, the ledger path, the crate counts and the three API specs were not. A short follow-up sweep — “grep the superseded spellings and the old crate list and correct every hit” — closes D2, D6, D7, D9, D12, D17–D23, D24–D31 and D35 at once.
