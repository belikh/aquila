# Aquila v2 — wayfinder map

Status: draft. This map plans; the spec and ADRs decide. Nothing here overrides
`CONTEXT.md`, the spec, or an ADR.

## Destination

A **learned, placement-structured, commodity-node** sensing system on the v1
fleet: a home of cheap ESP32-S3 nodes (≥3 per floor, on the perimeter, maximally
spaced, core-only) streams CSI to the core, and a **permutation-invariant,
node-set model** estimates **per-zone occupancy** (the gated claim) plus an
**advisory 3D position**. It handles **still and moving** people, needs **no
phase-coherent aperture and no runtime camera**, and **improves over successive
model releases** from camera-supervised Sessions contributed by users as git
pull requests.

## Notes

- Read `CONTEXT.md` first; it fixes the vocabulary. Use it exactly.
- Normative: `docs/spec/` (index at `docs/spec/README.md`),
  `docs/adr/0001`–`0004`, and `docs/spec/interface-register.md`. v2 adds its own
  ADRs and extends the owning specs.
- Advisory: `docs/research/` — it never defines v2 scope.
- External claims cite `docs/reference/sources.json` by `id`.
- Ordering principle: **spikes and tracer bullets**, cheapest-decisive first.
  Every claim is measured from its first commit and recorded in the commit-keyed
  ledger.
- Honest claims only. A per-home-adapted claim and a cross-home claim are
  different claims; the second is reported-but-ungated until it is evidenced.

## Decisions so far

Settled in the v2 design conversation; each points at the deciding reason.

- **Learned route, geometry demoted.** Passive geometric AoA/coherent ToA across
  independent ESP32-S3 radios is not evidenced and, on wireless-only independent
  nodes, is a research lead at best. Coherent apertures and geometric AoA/ToA
  move to **Track 3 research**, not v2 delivery. Two independent literature
  syntheses support this; neither is citable as-is — the load-bearing primaries
  must be verified first (see Fog).
- **Placement is structured, not random.** ≥3 nodes per floor, on the outer
  sides, spaced as far apart as possible, **core-only**. Onboarding enforces the
  layout; the accuracy depends on it. This is what makes per-home adaptation
  tractable — a predictable layout, not an arbitrary one.
- **Outputs are zone occupancy (the claim) and an advisory 3D position.** The
  gated output is per-zone occupancy; the coordinate is advisory, consistent with
  `geometry-calibration.md` §0. There is no honest metric-3D claim on $6 radios.
- **Features are phase-free across nodes.** Amplitude, motion/Doppler, temporal
  CSI ratios, and inter-node amplitude/co-motion. `Coherence::Uncoherent` only —
  the learned route does not need, and does not claim, cross-node phase.
- **Still people are learned from the static fingerprint**, against an empty-home
  baseline; not from Doppler.
- **Camera bootstraps, then leaves.** The camera is a **training-time teacher**
  only. It is never a runtime input (`CONTEXT.md`, `evaluation.md`,
  `inference-models.md` §7). The end state is **camera-free** walk-through
  adaptation.
- **Long setup is acceptable** (for now). A guided per-home session — perimeter
  placement → empty-home baseline → walk-through → local adaptation — is
  permitted, and is what de-risks cross-configuration collapse.
- **Data contribution is a git PR.** A user runs a capture tool that builds a
  self-verifying `.aqs` Session bundle (CSI + camera-derived labels + a manifest
  entry), and opens a pull request. **Video never leaves the home; only derived
  labels and CSI are contributed.** No runtime egress, no federated server, no
  relaxation of the private runtime surface.
- **Data licence is CC-BY-NC, in a separate data repository**, keeping NC
  artefacts out of the GPLv3 code history (`inference-models.md` §6).
- **Continuous improvement has a concrete mechanism:** pretrain on the
  deterministic simulator (labelled, free) → reference corpus → per-home
  adaptation → hosted corpus (dataset host with DOI) → periodic retrain → model
  published and delivered through the existing add-on/HACS update channel.
  Active learning (deployed models flag uncertain windows; users are invited to
  contribute those) is the highest-value data path.
- **Two live training sites.** *Site A* — a 3-storey apartment (≈ v1's ~9+ node
  / three-floor reference). *Site B* — a large 2-storey house, many rooms. Both
  live for the v2 development window. **Site B is the cross-configuration
  holdout**: train/reference on Site A, measure transfer to Site B.
- **Partial camera coverage is expected.** Roughly **2 rooms per site** carry a
  camera for the training period. Cameras supervise *position*; **occupancy is
  labelled more cheaply** (guided walk-throughs, and optionally an mmWave sensor
  as a second `GroundTruth` adapter). So the position head is supervised in few
  rooms and stays advisory; occupancy is supervised everywhere.
- **Non-human motion must be rejected.** A domestic cat is present at Site A, so
  the corpus must include explicit "animal present, no person" Sessions, or the
  system will report false presence.
- **Hardware homogeneity is required.** Every node must be the same chip (ESP32-S3,
  v1 reference). A C3/S3 mix produces a different CSI shape and is incomparable
  for both training and pooling.
- **Firmware-in-the-loop via Espressif's QEMU fork.** `idf.py qemu`
  (espressif/qemu, official) runs real ESP-IDF firmware headlessly — CPU, memory,
  several digital peripherals, eFuse emulation and secure-boot v2 — so the node
  control channel, pairing, the OTA agent, the ring buffer and the sync state
  machine become testable in CI **without hardware**. The `.#esp` shell provides
  the host prerequisites and wires activation plus the one-time
  `idf_tools.py install qemu-xtensa`; ESP-IDF itself is installed by Espressif's
  own installer (it is not source-buildable here). Chip simulators model **no
  RF**: any behaviour discovered or validated only in QEMU stays
  **`Hardware-gated`** (`evaluation.md` §5). Secondary use: QEMU-vs-silicon
  differential testing is a discovery technique — where real silicon diverges
  from the emulator (frozen documented behaviour), something undocumented is
  happening, and the espressif/qemu device models plus IDF
  `soc/s3/register/s3_*.h` headers are themselves diffable against the public
  TRM for undocumented registers.

## Not yet specified (fog)

In-scope questions the frontier has not sharpened into tickets. Each must
graduate into a ticket, an ADR, or a `CONTEXT.md` change — never a silent
assumption.

- **Adaptation method and budget.** Few-shot probe vs full fine-tune vs
  self-supervised embedding + light classifier; how much labelled data per home;
  how long setup may take before it stops being acceptable.
- **Position head honesty.** Metric regression vs zone classification; the
  coverage below which the coordinate is withheld; when (if ever) position can be
  gated rather than advisory.
- **Representation learning.** How much the camera-free rooms' unlabelled CSI
  contributes via contrastive/self-supervised pretraining (channel-charting-like),
  and whether it transfers across the two sites.
- **Cross-site transfer.** The measured Site-A→Site-B degradation with and
  without adaptation — the number that decides whether "better over time" is real.
- **Contribution governance.** Retention of raw CSI vs derived-only; active-learning
  opt-in and the one suggestion that leaves the device; contributor attestation
  (DCO/CLA) and the consent text in the UI.
- **Hosting.** When and where the corpus moves off git (dataset host choice, DOI,
  cost) and how the repo carries manifests + golden clips.
- **Model release and measurement.** How an improvement is *proven* (pinned
  splits, held-out homes) rather than asserted; staged rollout via the add-on.
- **Trigger beacon.** Whether the controlled-cadence / epoch beacon is needed at
  all, and its compliance posture if it is.
- **mmWave as a `GroundTruth` adapter.** Whether an mmWave sensor is a useful
  presence/position reference where cameras are absent, and how it aligns.
- **Citable sources.** Verify WiCAL, Larsson & Vieira, the ESPARGOS hardware
  paper, and the reciprocity/OTA papers against primaries before any enter
  `sources.json`; the two AI syntheses in hand are not themselves citable.
- **Firmware-in-the-loop coverage.** What QEMU can prove before silicon: which
  node-firmware behaviours (control channel, pairing, OTA agent, ring buffer)
  pass headlessly; whether the espressif fork emulates any Wi-Fi at all; and
  whether QEMU-vs-silicon differential testing exposes undocumented S3 behaviour
  worth chasing.

## Out of scope

Beyond the destination; never graduates into this map.

- **Phase-coherent apertures, cross-node AoA and coherent ToA** — Track 3
  research (`research-tracks.md`, `esp32-as-sdr.md`); demoted from delivery.
- **3D pose** — deferred; the bar is occupancy, not keypoints.
- **Vitals** (breathing, heart rate) — deferred from the v2 destination.
- **Active sensing / transmit / steering** — except an optional trigger beacon,
  and then only within the ACMA LIPD / FCC Part 15 boundary (`risks.md` R-16).
- **Camera as a runtime input** — research-only, training-only.
- **A federated/cloud aggregation service** — contribution is an offline git PR;
  the runtime surface stays private.
