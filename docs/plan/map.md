# Aquila v1 — wayfinder map

## Destination

**v1 shipped**, exactly as `docs/spec/roadmap.md` §"What v1 shipped means"
defines it: real ESP32-S3 nodes streaming raw CSI over WiFi UDP to the HA add-on;
Home Assistant surfacing presence/occupancy, person count, coarse pose, breathing
and heart rate; the HACS component and the ingress dashboard install from the
published repository; the three in-domain bars (torso PCK@20 ≥ 65 %, presence
F1 ≥ 0.90, breathing MAE ≤ 2 brpm) evidenced by commit-keyed ledger rows; global
whole-home calibration first-class and versioned by commit; node firmware
updates over the LAN by signed OTA surfaced in Home Assistant; and the whole
pipeline reproducible from recorded fixtures without hardware.

## Notes

- Read `CONTEXT.md` first; it fixes the vocabulary. Use it exactly.
- Normative: `docs/spec/` (index at `docs/spec/README.md`),
  `docs/adr/0001`–`0004`, and `docs/spec/interface-register.md`.
- Advisory: `docs/research/` — it never defines v1 scope.
- External claims cite `docs/reference/sources.json` by `id`.
- Ordering principle: **tracer bullets** (`roadmap.md`). Every ticket cuts the
  whole pipeline end to end — a CSI source, the DSP and inference in between, an
  output, and a measurement record — and is measured from its first commit. We
  do not build horizontal layers and hope they meet.
- Honest claims only. A slice is not done until its ledger rows exist at the
  target bar and a diff against the previous commit is available; nothing gates
  CI in v1 (record and diff).

## Decisions so far

The settled design, in gists that point at the deciding document:

- **One crate graph, one port pair** — the interface register and ADR-0003 fix
  fourteen product crates plus four tooling crates, the six seams, the
  dependency edges, and the ports (UDP `9100` data / `9101` control; TCP `8099`
  HTTP+WS+ingress). `tools/dag.toml` enforces the graph.
- **`aquila-sensing` is the single stateful core**; `Clock`, calibration, time
  sync and fusion are internal modules with internal seams, not crates. See
  ADR-0001, `architecture.md`, ADR-0003 §5–6.
- **Nodes are dumb sensors**: no DSP, inference, fusion or calibration on the
  node; one CSI frame per UDP datagram; loss surfaced, never recovered.
  `capture-and-firmware.md`.
- **Replay and simulation first**: any ledger row is reproducible from recorded
  fixtures without hardware. `testing.md`, `capture-and-firmware.md` §7–8.
- **One versioned contract crate**: `aquila-types` owns every shared type;
  `CsiFrame` (wire), `CsiEnvelope` (arrival), `SensingFrame` (time-aligned
  window). `data-contracts.md`, ADR-0003 §9.
- **Feature-level fusion only, no cross-node phase**: no AoA, no ToF, no
  coherent array in v1; a phase-coherent RF reference is future-only.
  `fusion-sync.md` §3, `risks.md` R-02/R-21.
- **Measurement is first-class**: every claim is a commit-keyed `LedgerRecord`
  behind `MetricSink`; `Witness` is `Gated`/`Measured`/`Claimed`. `measurement.md`,
  `evaluation.md`.
- **Private surface only**: no public REST/WS/MCP, no SDK, no plugins, no MQTT;
  the control plane is loopback/ingress and a direct node control channel.
  `core-api.md`, `home-assistant.md` §5.1, `privacy-security.md` G8.
- **Global whole-home calibration** is one artefact, bound to a topology digest,
  versioned by commit, invalidated honestly. `geometry-calibration.md` §7–10.
- **Model pipeline**: PyTorch → ONNX, seeded from the ruvnet NC weights,
  fine-tuned on MM-Fi and own captures, run in Rust via `ort` (CPU), admitted
  only with a provenance model card. `inference-models.md`.
- **Toolchain and repo**: one GPLv3-only monorepo, pinned toolchain, enforced
  DAG, no committed agent scaffolding, conventional non-squashed history.
  ADR-0002, `repository-tooling.md`.
- **Firmware OTA over the control channel**: signed images (Ed25519 + SHA-256),
  dual-partition A/B, automatic rollback, an anti-rollback version floor; driven
  and observed from Home Assistant as one `update` entity per node; no cloud and
  no node HTTP server. `firmware-ota.md`, `interface-register.md` (Firmware OTA),
  ADR-0004.
- **Licence**: GPL-3.0-only code; the project is non-commercial, so NC datasets
  and NC weights are usable but derivative artefacts stay NC and out of the GPL
  history. `inference-models.md` §6, `risks.md` R-13/R-14.

## Not yet specified (fog)

In-scope questions the frontier has not sharpened into tickets. Drawn from
`docs/spec/risks.md` §9; each blocks a slice's honest exit and must graduate into
a ticket, an ADR, or a `CONTEXT.md` change — never a silent assumption.

- **Q-03 / R-03 — features without phase coherence.** Which amplitude/motion
  features survive per-frame and across node-pairs, and can any pose or
  breathing bar be met without phase alignment? Settles the `aquila-signal`
  feature set and whether coarse pose is realistically v1. *Blocks* slices 2, 4.
- **Q-04 / R-01 — what "in-domain" means precisely.** One room, one home, one
  furniture layout, one device set; and does the reference deployment stay
  inside it? Fixes the wording of every gated claim. *Blocks* 1, 5, 6, 10.
- **Q-01 / R-01 — cross-room and cross-floor degradation.** The measured drop
  versus in-domain, and when Aquila stops attributing a result to a room.
  *Blocks* 5, 6.
- **Q-02 / R-02 — can one global calibration meet the bars** across
  heterogeneous rooms, or does "global only" force per-room staging earlier?
  *Blocks* 3, 5.
- **Q-05 / R-04 — is multi-person count in the honest claim set**, and what
  count-MAE band is defensible with no bar in the brief? *Blocks* 4, 6.
- **Q-06 / R-05 — what reference and split fix the heart-rate bar**, given
  "baseline-fixed"? *Blocks* 2.
- **Q-07 / Q-08 / R-08 / R-09 — model provenance and licence.** Exact ruvnet NC
  weight terms, and whether MM-Fi can legally fine-tune redistributed GPL
  weights. *Blocks* 2, 4, 10.
- **Q-09 / Q-10 / Q-11 / R-10 / R-11 / R-12 — own-capture data plan.** How much
  data across 3 floors / 9+ nodes / multiple people / sessions is needed; what
  consented reference is lawful; how the eval split is pinned. *Blocks* 4–6.
- **Q-15 / Q-17 / R-17 — the ESP32-S3 CSI shape contract.** Which IDF version,
  `esp-csi` revision and silicon rev produce stable, documented CSI; and the
  fallback when frames are dropped or malformed. *Blocks* 7.
- **Q-18 / Q-19 / Q-20 / R-21–R-24 — software time sync at scale.** Achievable
  precision across 9+ nodes; drift without a phase reference; loss/reorder
  detection. *Blocks* 3, 4, 7.
- **Q-21 — end-to-end latency** acceptable for HA automations at reference
  scale. *Blocks* 7, 9.
- **Q-22 / Q-25 / R-25 — HA OS add-on constraints** (ingress, host networking
  for UDP, WASM serving) that bind the host and dashboard. *Blocks* 7, 9.
- **Q-23 / R-31 — the DAG allow-list policy** kept honest without blocking
  legitimate splits. *Blocks* 11 and any new edge.
- **Q-24 / R-30 — stopping a passing in-domain row** from being paraphrased into
  an out-of-domain claim. *Blocks* 10.

## Out of scope

Beyond the destination; never graduates into this map. Each has its own research
note or is reserved in `CONTEXT.md`.

- **Camera ground truth** as a runtime input — research-only, Track 1
  (`research-tracks.md`, `evaluation.md` §5.3).
- **Active sensing / transmit / steering** — constrained to compliant 802.11
  and ACMA LIPD; Track 2, `privacy-security.md` G16, `risks.md` R-16.
- **ESP32-as-SDR raw I/Q and the phase-coherent array** — future-only; Track 3,
  `esp32-as-sdr.md`.
- **Any public API, SDK, MCP, or plugin surface** — v1 is in-tree only
  (`CONTEXT.md` "Explicitly not used").
- **Per-room or per-floor calibration**, cross-floor pose, identity/gait/fall
  detection, HRV/SpO2 — staged or out of scope (`CONTEXT.md`, `risks.md`).
- **PoE, ESP-NOW, MQTT, Matter, Zigbee, non-Linux hosts, armv7/armhf** —
  dropped by decision (`home-assistant.md` §3.1, research notes).
