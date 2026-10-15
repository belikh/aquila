# Remediation verification — sister-issue audits

Read-only verification of waves R1–R5 against the **current tree** (HEAD
`0a6fac5`). Every finding in the three audits is checked independently of the
fix commits; a finding is **resolved** only when the current source or a passing
test demonstrates it, **residual** when it is still present, and **new** when the
remediation introduced a fresh problem.

- Sources audited: [`sister-issues-core.md`](sister-issues-core.md) (S1–S20),
  [`sister-issues-firmware-ha.md`](sister-issues-firmware-ha.md) (F1–F39),
  [`sister-issues-specs.md`](sister-issues-specs.md) (D1–D60).
- Nothing was changed except this file.

## Tally

| Report | Resolved | Residual | New |
| --- | ---: | ---: | ---: |
| core (S) | 14 | 6 | — |
| firmware/HA (F) | 26 | 13 | — |
| specs (D) | 60 | 0 | — |
| introduced by remediation | — | — | 3 |
| **Total** | **100** | **19** | **3** |

The two criticals — S1/F17 (I/Q byte order) and S4 (circular pose) — are
genuinely fixed, not merely relabelled. S4 now carries a falsifiability control
(`pose_search_band_is_not_keyed_to_the_simulator`, `crates/aquila-testkit/src/tests.rs:1179`)
and emits `Witness::Claimed` (`crates/aquila-sensing/src/pose.rs:460`).

## 1. Finding-by-finding status

Legend: **R** resolved · **X** residual · **N** new.

### core — S1–S20

| id | area | status | evidence |
| --- | --- | --- | --- |
| S1 | I/Q order | R | ADR-0006 imag-first; `csi.rs:107-140` (`i_at` reads `2k+1`, `q_at` reads `2k`); `decode.rs:263,339-341`; `datagram.rs:116-118`; phase-sign test in `aquila-wire/src/tests.rs` |
| S2 | `first_word_invalid` width | R | `signal.md:249-250`, `data-contracts.md:211`, `csi_node.c:216` all say "leading complex sample (two bytes)"; `decode.rs:286-291` |
| S3 | flag dropped | R | `CsiFrame.first_word_invalid` (`csi.rs:126`); wire `flags` bit 0 (`datagram.rs:21,98,221`); `decode_csi` records `first_word_zeroed` (`decode.rs:286`) |
| S4 | circular pose | R | band-search estimator (`pose.rs:69-100,209-260`); per-seed simulator dither (`scene.rs:170-190`); control test `tests.rs:1179`; `Witness::Claimed` `pose.rs:460` |
| S5 | gated bars on synthetic | R | ledger rows graded `Authority::Synthetic` (`tests.rs:316-343`); `claims.toml` lists all three as `target`; `testing.md:238` |
| S6 | zero noise power | R | real `residual_noise_power` (`process.rs:440-444,510-516`); `SubcarrierMask` (`selection.rs:53`) |
| S7 | duplicated constants | R | one declaration in `aquila-types/src/pose.rs:19,31,37`; pin test `tests.rs:1206-1219` |
| S8 | pose in core | R | `DspPoseModel` implements `PoseModel`; engine calls the seam (`engine.rs:21,213-220,1132`) |
| S9 | synthetic threshold default | R | `presence_motion_threshold: Option<f64>` default `None` (`config.rs:71,164`); uncalibrated core refuses (`engine.rs:456-460,473`) |
| S10 | saturating sim / fake RSSI | R | modelled fixed gain `SIM_GAIN` (`source.rs:44-72`), `saturated` flag (`source.rs:319,365,399-400`), RSSI from link budget (`source.rs:382-393`) |
| S11 | `subcarrier_offset`, null map | **X** | `source.rs:412` and `csi_node.c:224` still `-(n/2)`; `signal.md:652` still "must be confirmed against a real capture" |
| S12 | channel→freq map | R | one `channel_centre_freq_mhz` in `aquila-types/src/csi.rs:47-67` (ch 14 = 2484); used by `decode.rs:228-229` and `source.rs:468` |
| S13 | two max-length constants | R | `MAX_RECORD_BYTES = 1428` derived from `MAX_SUBCARRIERS` 714 (`decode.rs:17-22`); boundary at `tests.rs:66` |
| S14 | all-zero digests | R | `config_hash: None` (`record.rs:128`, `source.rs:649`), `sha256: None` (`source.rs:654`, `generator.rs:109,189`, `addon/main.rs:190`) |
| S15 | null-commit ledger default | **X** | `MeasurementConfig::default().commit == "0".repeat(40)` (`config.rs:36`), validation accepts it (`config.rs:292-300`), composition root uses the default (`aquila-addon/src/main.rs:125`) |
| S16 | silent fallbacks | **X** | `process.rs:432-435` `unwrap_or_else(|_| …clone())` unflagged; `features.rs:54-56` → `(0.0,false)`; `calibration.rs:1179-1182` `map_or(0, …)` |
| S17 | recorded bytes re-encoded | **X** | `record.rs:4-9,97` still `encode_datagram`; only the doc was softened |
| S18 | stale f32/ADR refs | **X** | `bridge.rs` fixed, but `aquila-wire/src/constants.rs:11` still asserts `data-contracts.md` §9.1 spells the body `AQCS` (see N2) |
| S19 | duplicate concepts | **X** | `MetricKind` (`ledger.rs:19`) and `Grade` (`measurement.rs:51`) both survive; `Confidence`/`Quality`/`NodeQuality` (`primitives.rs:138,171,222`) unfused |
| S20 | `decode_record` dead path | R | both decoders now imag-first and flag-consistent (`decode.rs:253,309`); labelled "record-level path" |

### firmware / HA — F1–F39

| id | area | status | evidence |
| --- | --- | --- | --- |
| F1 | Kconfig WiFi creds | R | `Kconfig.projbuild` password `default ""`; `main.c:74-114` reads NVS first |
| F2 | OTA opcodes missing | **X** | `capture-and-firmware.md:440-446` lists 0x0E–0x14; `node_wire.h` ends at `AQ_OP_EVENT`; no handler in `node_control.c` |
| F3 | core doesn't serve HA contract | R | `server.rs:255-296` serves `/nodes`, `/schema`, `/firmware`, per-node firmware, `/metrics/snapshot`, `/capabilities`; manifest `iot_class` → `local_polling` (`manifest.json`) |
| F4 | non-existent field (no compile) | R | `csi_node.h:31` has `heartbeat_ms`; `idf.py build` succeeds |
| F5 | `secondary` ignored | R | `csi_node.c:242-253` maps to `WIFI_SECOND_CHAN_*` |
| F6 | GET_CONFIG returns version only | R | `node_control.c:388-402` returns all config fields in the RESULT body |
| F7 | `config_hash` unverified | **X** | `node_control.c:343-347` still applies config without checking the hash (documented "reserved until the algorithm is pinned") |
| F8 | no request dedup | R | bounded replay ring (`node_control.c:46,116-124,510-513`) |
| F9 | NVS subset | R | `load_state`/`save_state` persist full CSI config (`node_control.c:159-197`); `main.c:74-114` reads network/core provisioning |
| F10 | PHY flags | **X** | `phy_flags_from_rx` (`csi_node.c:78-97`) sets bits 0–4 but never bit 5 "sounding/NDP" (`data-contracts.md:223`) |
| F11 | ADR-0004 stale line | R | ADR left immutable with banner; capture spec points to ADR-0005/0006 |
| F12 | README over-claims | R | `README.md:64-71` now states the check is header-text only |
| F13 | component ranks firmware | R | `update.py:102-106` reads `latest_version` from the core |
| F14 | on-node FP gone (positive) | R | `csi_node.c` copies verbatim, no `math.h` |
| F15 | dead metadata fields | R | slot no longer carries `bandwidth_mhz`/`secondary` (`csi_node.c:44-53`) |
| F16 | `csi_len_cap` units | R | `csi_node.h:29` "maximum raw int8 CSI bytes"; default `CSI_RAW_MAX` 1428 (`csi_node.c:60,265`) |
| F17 | I/Q order (#36 twin) | R | ADR-0006; see S1 |
| F18 | boot width/rule | **X** | width unified u16 (`main.c:166`, `send_join_request` `node_control.c:251`), but `data-contracts.md:200` still says "Incremented every boot; persisted in node NVS" vs `capture-and-firmware.md:647-648` "random at boot" (firmware uses random, not persisted) |
| F19 | `subcarrier_offset` assumption | **X** | `csi_node.c:224` `-(n/2)`; same as S11 |
| F20 | band boundary | **X** | `csi_node.c:74-77` `channel <= 14 ? 0 : 1`; never emits `255` unknown |
| F21 | unauthenticated control | R | HMAC-SHA256 trailer, pinned source, replay window, fail-closed (`node_control.c:6-12,90-124,458-515`) |
| F22 | unsealed CSI | R | honestly downgraded: `privacy-security.md:59,115-120,467-473` (ingest auth deferred in v1) |
| F23 | password baked | R | see F1 |
| F24 | imposed retention | R | `aquila_core/config.yaml` `retention_days: 0`; `home-assistant.md:94-102` |
| F25 | G12 CSP / G11 rootfs | **X** | `privacy-security.md:245-267` marks both as v1 limitations (no CSP until ticket #10) |
| F26 | default-off token | R | no change required; port unpublished |
| F27 | unrunnable Dockerfile | R | real stages + `ENTRYPOINT` (`aquila_core/Dockerfile:88`), labelled UNVERIFIED |
| F28 | OTA surface dead | R | install/abort now fail closed with typed `503 ota_unavailable` (`server.rs:282-296`) |
| F29 | no OTA impl | **X** | no `esp_ota_*`, no partition table (same as F2) |
| F30 | missing panel assets | R | panel registration removed; `brand/icon.png` honestly documented as not shipped (`home-assistant.md:154`) |
| F31 | hardcoded temperature 0 | R | read from `temperature_sensor` (`node_control.c:266-281,297-300`) |
| F32 | hardcoded RTT 0 | **X** | field named `last_rtt_us` now carries echo turnaround `t3-t2`, not RTT (`node_control.c:304-306,426`) |
| F33 | IDENTIFY no LED | **X** | `IDENTIFY_GPIO (-1)` (`node_control.c:45`); blink is compiled out (`:436-444`) |
| F34 | HA untested | **X** | no test module under `custom_components/` (labelled) |
| F35 | firmware untested | R | now compile-verified (`idf.py build` passes); README labels hardware-gate |
| F36 | OTA no fixture | **X** | no OTA fixture/harness (same as F2/F29) |
| F37 | header-only drift check | R | `README.md:70-71` and `capture-and-firmware.md:686-688` now honestly say the golden-vector test is "planned, not yet present" |
| F38 | OTA key provisioning | **X** | no partition layout nor key-provisioning path (same as F2/F29) |
| F39 | G5 ingress identity | R | auth enforced on every request; `/live` is the only anonymous route (`server.rs:228-244`) |

### specs — D1–D60

All sixty are resolved. Representative evidence:

| id(s) | area | status | evidence |
| --- | --- | --- | --- |
| D1, D36 | `.aqs` container | R | `data-contracts.md:248,941` cite the exact-wire AQCI container |
| D2 | Witness 3 vs 5 | R | `CONTEXT.md:130-145` lists all five |
| D3, D24, D25, D28 | `NodeId`/seam `CsiFrame` | R | `data-contracts.md:85` `NodeId(u16)`; `capture-and-firmware.md:90-103` references `aquila-types::CsiFrame`, no `MacAddr`/`tsf_us`; §11.1 "settled by ADR-0003" |
| D4, D5, D46 | node-pair / PairScope / "link" | R | `geometry-calibration.md:58-74,66-70,473` ordered pair, frozen discriminants, `pair_id` |
| D6, D7 | Clock / device-time units | R | `capture-and-firmware.md:212-219`; `fusion-sync.md:42-44` states the µs↔ns bridge |
| D8, D9, D10, D11, D12 | Calibration, Band, ledger, path, seam frame | R | `data-contracts.md:962,986`; `signal.md:121` `FreqBand`; `measurement.md:49-51`; `data-contracts.md:740` `.aquila/metrics/ledger.jsonl`; `capture-and-firmware.md:90-103` |
| D13, D14, D15, D16 | API endpoints / auth / firmware routes / WS | R | `core-api.md` single router; `server.rs:228-313`; component `iot_class: local_polling` |
| D17, D18–D23, D29 | crate graph, adapters, vocab | R | `repository-tooling.md:216` "fourteen"; `architecture.md:208-213`; stale spellings flagged at `architecture.md:77-79` |
| D26, D27, D30–D36 | keypoints, decode cap, toolchain, counts | R | `evaluation.md:190-192`; `signal.md:240-245`; `repository-tooling.md:175` 1.98.1; `README.md:27`; `docs/README.md:43` |
| D37–D40 | unsupported claims | R | `evaluation.md:384-393`; `fusion-sync.md:155-165`; `capture-and-firmware.md:522-526`; `inference-models.md:156` |
| D41–D45 | missing specs / xtask | R | `xtask/src/main.rs:28-29` `coverage-floor`/`claims-verify`; `dag.rs:105-118`; `crates/aquila-dashboard/` stub; `claims.toml`; `evaluation.md:412` `eval` struck |
| D47–D50 | zone, storey, Authority, grades | R | `geometry-calibration.md:56,73`; no "storey" in specs; `measurement.md:126-132`; `docs/README.md:31` |
| D51–D60 | register/reality, ambiguity | R | cross-refs resolved; `geometry-calibration.md:615`; ADRs `Accepted`; `dashboard.md:292` |

## 2. Residuals and new issues, most severe first

### R-A (high) — OTA is specified as implemented but is entirely absent (F2, F29, F36, F38)

`capture-and-firmware.md:693-702` states in the present tense that "the node uses
ESP-IDF dual-partition A/B OTA with Ed25519 verification, an anti-rollback version
floor, and automatic rollback", and §6.2 lists opcodes `0x0E`–`0x14`. None exist:
`node_wire.h` stops at `AQ_OP_EVENT`, no `esp_ota_*` call or `partitions.csv`
exists under `firmware/`, and there is no key-provisioning path or OTA fixture.
The remediation plan declares OTA "ticketed, not a gap", but the spec still
asserts the node implements it.

**Fix:** either implement the opcodes/partition table/verify (tickets #13–#15), or
rewrite §6.2 and §9.5 (and `firmware-ota.md`) to say OTA is **specified but
unimplemented in v1**, with the status vocabulary already used elsewhere.

### R-B (medium) — ledger defaults to a null commit (S15)

`MeasurementConfig::default()` keys every row to `"0".repeat(40)`
(`crates/aquila-sensing/src/config.rs:36`), `validate` accepts it
(`config.rs:292-300`), and the composition root constructs the core with
`SensingConfig::default()` (`crates/aquila-addon/src/main.rs:125`). So production
ledger rows are attributed to no build — a fabricated provenance, unchanged.

**Fix:** make `commit` a required argument (no `Default`) or have `Sensing::new`
reject an all-zero commit.

### R-C (medium) — recorded sessions still re-encode the wire (S17)

`crates/aquila-capture/src/record.rs:4-9,97` still runs decoded frames through
`encode_datagram`, so a wire codec bug is invisible in a recording — only the
docstring was softened to admit it. `container.rs:16-18` still calls the stored
bytes "the exact on-wire `AQCI` bytes".

**Fix:** store the received datagram bytes verbatim in the `.aqs` record and
reserve re-encoding for the `synth` path; or state the lossy behaviour in
`container.rs` and the capture spec.

### R-D (medium) — silent "unknown → 0" fallbacks survive (S16)

`crates/aquila-signal/src/process.rs:432-435` substitutes the non-uniform series
via `unwrap_or_else(|_| …clone())` without flagging it; `features.rs:54-56`
collapses a failed transform to `(0.0, false)`; `calibration.rs:1179-1182` maps a
missing analysis to `channel=0, bandwidth=0, subcarriers=0` (band correctly
`Unknown`, the rest not).

**Fix:** propagate a typed `Partial`/`Flagged` state or set an explicit
unknown marker instead of a plausible zero; have every consumer check
`used_signal`.

### R-E (medium) — the honest-claims bijection is not actually enforced (new, N1)

`docs/spec/testing.md:187-210` requires a **bijection** between "the README's
claims table", test declarations and `claims.toml`, and says the verifier fails a
"README claim with no registry link". There is no root `README.md`, no claims
table anywhere, and `xtask/src/claims.rs` only validates the registry's internal
structure (it never parses test declarations or a README). `claims-verify` hence
reports OK while the described gate (G21) cannot fail.

**Fix:** add the README claims table and have `claims-verify` parse test/bench
`CLM-` declarations, or soften `testing.md` §7 to describe what is actually
enforced.

### R-F (low) — `subcarrier_offset` and the null-bin map are still guesses (S11, F19)

`firmware/.../csi_node.c:224` and `crates/aquila-simulator/src/source.rs:412` both
compute `-((n/2) as i16)`; `signal.md:652` still flags the HT20/HT40 null-bin map
as unconfirmed. Hardware-gated, but the firmware asserts a convention the project
has not pinned.

**Fix:** derive both from a documented per-bandwidth bin map and a real capture,
frozen into `Config`; keep the "unverified" label visible until then.

### R-G (low) — duplicate epistemic types not collapsed (S19)

`MetricKind` (`ledger.rs:19`) and `Grade` (`measurement.rs:51`) remain identical
variant sets; `Confidence`/`Quality`/`NodeQuality` (`primitives.rs:138,171,222`)
remain three separate `[0,1]` newtypes; `Authority` remains a second evidence
enum (partially mapped at `measurement.md:126-132`).

**Fix:** collapse `Grade` into `MetricKind` and the three confidences behind one
`UnitScalar` with named aliases, as the finding proposed.

### R-H (low) — boot-epoch rule still contradicts the firmware (F18)

`data-contracts.md:200` says `boot_epoch` is "Incremented every boot; persisted in
node NVS"; `capture-and-firmware.md:647-648` says "random at boot"; `main.c:166`
uses `(uint16_t)esp_random()` and never persists it. Width is unified; the rule is
not.

**Fix:** choose "random, not persisted" (matching the code) or "incremented and
persisted" and correct the other document.

### R-I (low) — PHY flag bit 5 never set (F10)

`csi_node.c:78-97` sets HT/secondary/STBC/LDPC/SGI but never the "sounding/NDP"
bit (`data-contracts.md:223`), and still maps VHT/HE onto the HT bit.

**Fix:** decode `sig_mode` into the correct PHY family and set bit 5 when the
frame is a sounding/NDP frame.

### R-J (low) — `last_rtt_us` reports a turnaround, not an RTT (F32)

`node_control.c:304-306` writes `s_echo_turnaround_us` (`t3-t2`, `:426`) into the
field the contract calls "last measured core RTT" (`capture-and-firmware.md:475`).

**Fix:** compute the true RTT at the core, or rename/re-document the field as the
node turnaround.

### R-K (low) — remaining labelled gaps

- `config_hash` not verified (F7, `node_control.c:343-347`).
- `band_from_channel` never emits `255` unknown (F20, `csi_node.c:74-77`).
- `IDENTIFY` blink disabled (`IDENTIFY_GPIO -1`, F33).
- G11/G12 unimplemented, labelled (F25, `privacy-security.md:245-267`).
- HA component and firmware have no tests (F34/F35, labelled).

### New issues

- **N1 (medium)** — the honest-claims bijection gap (R-E above).
- **N2 (low)** — `crates/aquila-wire/src/constants.rs:11` still says
  "`data-contracts.md` §9.1 spells the same canonical body `AQCS`", but
  `data-contracts.md` now uses `AQCI` (`:882,912`). A false cross-reference.
- **N3 (low)** — the imaginary-first order is attributed to **ADR-0005** in
  `constants.rs:33` and `capture-and-firmware.md:297`; the byte order is ADR-0006
  (ADR-0005 only carries the int8 decision). `constants.rs:60` also still prints
  "interleaved signed int8 `I, Q`" (real-first) in its formula comment.

## 3. Build and health checks

All three commands pass on the current tree (no edits):

- `nix develop --command cargo test --workspace` — **281 passed, 0 failed, 1
  ignored** (33 test binaries; the ignored test is
  `emit_node_wire_header`, an on-demand header regenerator).
- `nix develop --command cargo run -p xtask -- dag-check` — `dag-check: OK — 18
  members, 55 internal edges, rev 1`.
- `nix develop .#esp --command bash -c 'cd firmware/esp32-s3-csi-node && idf.py build'`
  — `Project build complete`; `aquila_esp32s3_csi_node.bin` 0xADA50 bytes, 32 %
  of the app partition free.
- (Bonus) `cargo run -p xtask -- claims-verify` — `OK — 3 claims validated`.

No test assertion was weakened to pass. The one changed assertion
(`decode_d4…`, `aquila-signal/src/tests.rs`) now checks that only the **first**
pair is zeroed rather than two, which is the corrected S2 behaviour, not a
relaxation.

## R7 status

Remediation round **R7** (this pass) closed every residual and new issue above
that could be closed without hardware, and re-labelled the rest honestly. The
tree is green; the checks are at the end of this section.

### Fixed

| id | fix | key files |
| --- | --- | --- |
| N1 (R-E) | The claims table now exists (`docs/README.md` **Claims**) and `claims-verify` enforces the registry↔table bijection, not just registry structure; `testing.md` §7 names what is actually enforced. | `docs/README.md`, `xtask/src/claims.rs`, `docs/spec/testing.md`, `claims.toml` |
| N2 (S18) | The false `AQCS` cross-reference is gone: the data-plane body is `AQCI` in `capture-and-firmware.md` §5.1, the register and `data-contracts.md` §9.1. | `crates/aquila-wire/src/constants.rs` |
| N3 | Imaginary-first is attributed to **ADR-0006** (int8 verbatim is ADR-0005); the `I, Q` prints are corrected to `Q, I`. | `constants.rs`, `c_header.rs`, `node_wire.h`, `lib.rs`, `capture-and-firmware.md` |
| S15 (R-B) | `MeasurementConfig::default()` carries an explicit **empty** commit; `SensingConfig::validate` refuses empty **and** all-zero; `with_commit` is the seam; the add-on requires `AQUILA_COMMIT` and refuses to start without it. A zero hash is never presented as a commit. | `aquila-sensing/src/config.rs`, `aquila-addon/src/main.rs`, tests |
| S16 (R-D) | Unknown is explicit, never `0`: `signal_motion_energy: Option<f64>`; `NodeFeatures.grid_is_uniform` flags a failed resample; `NodeAnalysis` channel fields are `Option` and `ChannelCalibration.known` marks placeholder channel blocks. | `features.rs`, `process.rs`, `calibration.rs`, `geometry-calibration.md` |
| S17 (R-C) | The live source **retains the raw datagram** and `record` stores those exact bytes in the `.aqs` Session, so replay runs the real decoder on what the node sent (code and doc now agree). | `aquila-sources/src/live.rs`, `aquila-capture/src/record.rs`, `main.rs` |
| S11 / F19 (R-F) | The `-(n/2)` subcarrier offset is explicitly marked **PROVISIONAL** in both the firmware and the simulator, pointing at `signal.md` §7. The value is still unconfirmed (hardware-gated). | `csi_node.c`, `aquila-simulator/src/source.rs` |
| F18 (R-H) | The boot-epoch rule is unified on **random, not persisted** across `data-contracts.md`, `csi.rs` and `capture-and-firmware.md`, matching `main.c`. | `data-contracts.md`, `aquila-types/src/csi.rs` |
| F10 (R-I) | `PhyFlags` gains **VHT** and **HE** bits; the firmware decodes `sig_mode` (1⇒HT, 3⇒VHT) and sets the sounding/NDP bit from `!not_sounding`, instead of collapsing every mode onto HT. | `aquila-types/src/primitives.rs`, `csi_node.c`, `data-contracts.md`, firmware `README.md` |
| F32 (R-J) | The stale "`last_rtt_us` in the contract" comment is corrected: the STATUS latency field is the node's echo turnaround, as `capture-and-firmware.md` §6.4 already states. | `node_control.c` |
| S19 (R-G) | `Grade` is now `aquila_types::MetricKind` (one enum); the three confidences are one `UnitScalar` with `Confidence`/`Quality`/`NodeQuality` aliases. | `aquila-metrics/src/measurement.rs`, `aquila-types/src/primitives.rs` |
| F2/F29/F36/F38 (R-A) | OTA is stated as **not yet implemented — see ticket #14** everywhere it is described, and the absent opcodes/endpoints are marked unimplemented in code. | `capture-and-firmware.md` §6.2/§9.5, `firmware-ota.md`, `docs/README.md`, `constants.rs`, `server.rs`, `custom_components/aquila/update.py` |
| F20 | `band_from_channel` now returns `255` (unknown) for a channel outside the contract's known sets instead of guessing 5 GHz. | `csi_node.c` |
| F33 | The spec now states that `IDENTIFY`'s blink is compiled out until a GPIO is assigned. | `capture-and-firmware.md` §9.1 |

### Left with reason

| id | reason |
| --- | --- |
| S11 / F19 | The HT20/HT40 null-bin map and subcarrier offset cannot be correct without a real capture. Marked provisional in code **and** spec; no pretence of correctness. Resolving it needs hardware (or a published ESP-IDF map). |
| R-A OTA | The feature is unimplemented, not mislabelled. Specs and code now say so; the work is tickets #13–#15. |
| F7 | `config_hash` is carried but not verified: v1 defines no canonical core-side hash algorithm to recompute against. Documented in `capture-and-firmware.md` §6.2. |
| F25 | G11 (rootfs hardening) / G12 (CSP) remain labelled v1 limitations in `privacy-security.md`. |
| F33 | Blink stays compiled out (`IDENTIFY_GPIO (-1)`); the node acknowledges but shows no light. Now stated in the spec. |
| F34 / F35 | The HA component and the firmware still have no automated tests; the firmware is compile-verified only. Labelled in the component and firmware READMEs. |

### Revised tally

Original R6 state: 100 resolved / 19 residual / 3 new. R7 closes **13** of the
residual/new findings outright, converts **3** (OTA group, S11/F19, F33) to
honest labels, and leaves **4** labelled gaps (F7, F25, F34/F35) that need
hardware, a canonical algorithm or a test-harness decision rather than an edit.

### Checks (all green after R7)

- `nix develop --command cargo test --workspace` — **284 passed, 0 failed, 1
  ignored** (the ignored test is still `emit_node_wire_header`; two focused R7
  tests were added: `a_zero_or_empty_commit_is_refused` and the exact-bytes
  assertion in `udp_live_source_counts_bad_frames_and_keeps_going`).
- `nix develop --command cargo run -p xtask -- dag-check` — `dag-check: OK — 18
  members, 55 internal edges, rev 1`.
- `nix develop --command cargo run -p xtask -- claims-verify` — `OK — 3 claims
  validated; registry and table are a bijection (claims.toml, docs/README.md)`.
- `nix develop .#esp --command bash -c 'cd firmware/esp32-s3-csi-node && idf.py
  build'` — `Project build complete`; `aquila_esp32s3_csi_node.bin` 0xADA90
  bytes, 32 % of the app partition free.
- `nix develop .#ha --command python3 -m py_compile custom_components/aquila/*.py`
  — OK.
- `cargo clippy --workspace --all-targets` — clean.

No test assertion was weakened. `#[forbid(unsafe_code)]` holds (no `unsafe`
added).

## R8 — accidental revert of the persistent add-on core (found live)

**Class:** `git add -A` sweeping stale working-tree files into an unrelated commit.

`4a6cd6e` ("attach the HACS zip without waiting on the multi-arch image") was
meant to change one line of `release.yml`, but `git add -A` also staged stale
pre-`1a39eb3` copies of `crates/aquila-addon/src/main.rs`, `aquila_core/Dockerfile`,
`aquila_core/config.yaml`, `crates/aquila-api/src/{assets,server}.rs`, and the
build artefact `aquila.zip`. Net effect: the persistent composition root added in
`1a39eb3` was reverted to the one-shot simulator run (bound `127.0.0.1`, no
dashboard, no calibration), so the released add-on image 0.1.2 exited immediately
— observed live as `aquila-addon: run complete — 0 events published`. That in
turn surfaced as `invalid_auth` in the HACS config flow, because the core the
component was validating against had already stopped.

Restored: `crates/aquila-addon/src/main.rs` and the `aquila_core/Dockerfile`
comment from `1a39eb3`. Retained: the `aquila-api` private-network auth change
(`peer_is_trusted`) and the `GET|HEAD` static-asset handling — both are present in
the current tree. Untracked and ignored: `aquila.zip`, `aquila-dashboard.zip`,
`SHA256SUMS`, `crates/aquila-dashboard/dist/`.

**Preventive rule:** never run `git add -A` where a subagent or an interrupted
session may have left a stale tree. Stage explicit paths and read
`git diff --staged` before committing.

Checks: workspace build + **324 passed, 0 failed**; `dag-check: OK — 18 members,
59 internal edges, rev 1`.

## R9 — health endpoint did not match the component contract (found live)

**Class:** two specs defined the same endpoint differently, and only one was
implemented.

`home-assistant.md` §5.2/§7 require `GET /api/v1/health` to carry `instance_id`,
`api_version`, `min_component_version` and the status fields the component reads
(`core_connected`, `self_test_ok`). `core-api.md` §4.1 defined health as
`{status, uptimeMs, api:{major,minor}, subsystems}` — and that is what the core
implemented, so `instance_id` was never returned. Live symptom: the HACS config
flow aborted with `unknown` (`api.py::async_validate` raises
`AquilaError("core health response is missing instance_id")`), after the earlier
`invalid_auth` was resolved by the add-on 0.1.3 persistence fix.

Fixed (issue #19): `core-api.md` §4.1 is now the one authoritative health schema;
the core returns `instance_id` (UUID, generated once and persisted under `/data`),
`min_component_version` (one constant shared with `/capabilities`),
`core_connected`, `self_test_ok`, `nodes_online`/`nodes_total`,
`calibration_state`, and honest `null`s for `clock_drift_ms`/`last_packet_ts`.
The component reads the API major from the canonical `api` object (`api_version`
kept as a tolerated alias). `home-assistant.md` §5.2 now references §4.1.

Checks: **327 passed, 0 failed** (core); **43 passed** (component); `dag-check`
OK; clippy clean. Shipped in add-on **0.1.4** / component **0.2.1** (`v0.1.4`).
