# Evaluation and accuracy — specification

Status: draft for review (v1).
Owner: evaluation and accuracy author.
Scope: how Aquila's sensing claims are measured, graded and reported — the
`GroundTruth` seam and its adapters, the evaluation protocols, the metric
definitions, the honest-claims grading scheme, and the v1 target bars. The
sensing pipeline, calibration algorithm, models and add-on are specified
elsewhere; this document references them but does not define them.

Vocabulary is fixed by `CONTEXT.md`. This document uses its terms — **Ground
truth**, **Baseline**, **Session**, **Node**, **Node-pair**, **Zone**,
**Presence**, **Pose**, **Vitals**, **Breathing rate**, **Heart rate**,
**Calibration**, **Witness** — and introduces no synonyms.

---

## 1. Purpose

Aquila's whole premise is that a home sensor is worth trusting only if every
number it states is **measured, reproducible and honestly graded**. This
specification makes that operational:

- **Measurement is first-class.** Every sensing claim is a value in a
  commit-keyed ledger (the `MetricSink`), never a sentence in a README.
- **Replay and simulation first.** Any ledger row is reproducible from recorded
  fixtures without hardware (roadmap, "What v1 shipped means").
- **Honest claims only.** A claim may carry a number only if the ledger backs
  it, at a stated coverage, with a reproducer command. Everything else is
  graded and labelled, not silently asserted.

### Non-goals (v1)

- No CI gating on metric thresholds. The workflow is **record-and-diff**; a
  regression is visible, not build-breaking.
- No claim of out-of-domain accuracy, cross-environment generalisation, or
  cross-room accuracy as a headline.
- No runtime dependency on ground truth. `GroundTruth` is an offline evaluation
  seam; it is never an input to presence, pose or vitals at runtime
  (`CONTEXT.md`). Camera ground truth and the phase-coherent RF reference are
  research-only and never ship.
- No person identity, recognition or re-identification. Subject and room
  identifiers exist only to partition data for evaluation; they are never
  modelled.

---

## 2. The `GroundTruth` seam

`GroundTruth` is the only source of reference labels for scoring. It is a deep
module: a tiny interface that hides every adapter's capture format, clock
alignment and calibration. It is owned by `aquila-evaluation` (the register's
seam list). A second adapter is required for the seam to exist — a fixture labeller
and at least one physical reference (chest strap or MM-Fi) are the first two.

### 2.1 Interface

```rust
/// An offline reference measurement for scoring a sensing estimate.
/// Never a runtime input.
trait GroundTruth {
    /// What this source can label.
    fn kind(&self) -> ReferenceKind;   // Presence | Count | Pose | Breathing | Heart

    /// The reference record for a time window of a session, if one exists.
    fn label(&self, session: &SessionId, window: TimeWindow) -> Option<ReferenceRecord>;

    /// Which part of the declared scope this source actually covers.
    fn coverage(&self, scope: &EvalScope) -> Coverage;

    /// Provenance: source, calibration and digest, for the ledger.
    fn provenance(&self) -> ReferenceProvenance;
}
```

Rules the seam enforces:

- **Aligned, not assumed.** `label` returns records already mapped onto the
  session's common clock grid (software time sync + per-node drift estimation,
  `aquila-sensing`). An adapter that cannot align returns `None` with a coverage
  gap, never an interpolated guess.
- **Reference is calibrated.** Each adapter records its own calibration
  (time offset, gain, unit) in `ReferenceProvenance`; an uncalibrated reference
  is graded, not used silently.
- **Paired, on demand.** The evaluator pairs each engine estimate with the
  reference record for the *same* window and the *same* scope cell (zone,
  subject, room, configuration). Pairing is by key, never by nearest-neighbour
  interpolation.

### 2.2 Adapters

| Adapter | `ReferenceKind` | Basis | Availability |
| --- | --- | --- | --- |
| `FixtureLabelGroundTruth` | Presence, Count | Labels committed with the recorded fixture set; the v1 default. | v1 |
| `ReferenceVitalsGroundTruth` | Breathing, Heart | Chest strap (ECG-derived heart rate) and respiratory belt for breathing; oximeter as a redundant pulse cross-check only (no SpO2, no HRV — out of scope). | v1 eval rig |
| `MmFiLabelGroundTruth` | Pose | MM-Fi multimodal labels: WiFi CSI plus camera-derived 3D skeletons. Training seed and cross-dataset pose evaluation; non-commercial use only. | v1 eval / training |
| `CameraRigGroundTruth` | Presence, Count, Pose, Breathing (weak) | A synchronised research rig that auto-labels CSI; research-only, behind the same seam. | research (Track 1) |

Notes.

- **Vitals reference.** Breathing rate is scored against a respiratory belt (the
  direct mechanical reference); the chest strap supplies an independent heart
  rate. An oximeter is accepted only as a second pulse-rate source. Because the
  belt and strap are worn and the CSI nodes are fixed, the reference is an
  *independent* measurement of the same physiological signal, not derived from
  the nodes.
- **MM-Fi.** MM-Fi is a multimodal research corpus (WiFi CSI and mmWave with
  camera-derived pose labels across many subjects, actions and scenes). It is
  used to seed and fine-tune the pose model and to evaluate pose as a
  **cross-dataset** result. Dataset-specific metadata (subject and scene counts,
  label schema) is recorded from the dataset's own documentation when used, not
  asserted here.
- **Camera rig.** The rig spec, synchronisation tolerance and label-noise
  analysis live in research Track 1; the seam contract above is all the core
  needs. The rig never runs in the add-on.

### 2.3 Reference honesty

A reference is itself a claim. Its own accuracy, synchronisation error and
label noise are recorded in `ReferenceProvenance` and reported alongside the
metric. Scored metrics inherit the reference's `Witness` grade as an upper
bound: an engine claim cannot be stronger than the reference it was scored
against.

---

## 3. Evaluation protocols

Every protocol is a named, deterministic partition of the fixture set (or the
captured corpus), with a fixed seed recorded in the ledger. No protocol may
reuse a session, subject, room, node-pair or configuration across train and
test.

| Protocol | Holdout unit | Measures | Role in v1 |
| --- | --- | --- | --- |
| **In-domain** | Held-out sessions of the same subjects, rooms and configuration | Best-case capability on familiar geometry | Gated bars |
| **LOSO** (leave-one-subject-out) | One subject | Cross-subject generalisation | Reported, ungated |
| **LORO** (leave-one-room-out) | One room (all subjects) | Cross-room / multipath generalisation | Reported, ungated |
| **Cross-configuration** | One whole home (node layout, furniture, channel width) | Cross-environment generalisation | Reported, ungated |
| **Leave-one-session-out** | One session | Session-to-session stability | Reported |

Definitions.

- **In-domain.** Train and test drawn from the same subjects, rooms and
  node/node-pair configuration; only the held-out sessions differ. This is the
  only scoped domain in which v1 makes a headline claim.
- **LOSO.** For each of the *S* subjects, train on all sessions of the other
  *S−1* subjects and test on the held-out subject. Report the mean and standard
  deviation of the metric across the *S* folds, plus the per-fold values — never
  a single pooled number. Subjects are labelled only for partitioning.
- **LORO.** For each room, hold the entire room out (geometry and multipath
  change); all subjects remain. Report per-room and mean±SD across rooms. LORO
  is the honest proxy for "a room the system has never seen".
- **Cross-configuration.** Hold out an entire configuration (a whole home:
  node count, positions, orientations, channel width, furniture). This is the
  hardest split and the one most likely to be near-chance for pose; it is
  reported to bound the claim, not to set a bar.
- **Fold hygiene.** One session belongs to exactly one fold. A subject's
  sessions never straddle train and test. A node-pair, its calibration artefact
  and its baseline are all held in the same fold as the session. Leakage of any
  of these invalidates the run and is a ledger failure, not a low score.

### 3.1 Scope and coverage

An `EvalScope` declares the population a metric claims: the set of sessions,
subjects, rooms, node-pairs and configurations. Every reported metric carries:

- **coverage** — the fraction of each scope dimension actually scored
  (e.g. 12 of 14 sessions, 5 of 5 rooms);
- **paired sample count** at the metric's unit (windows, decisions, frames);
- **exclusions** — every dropped session, dropped window and the reason.

A metric on a partial scope is labelled with its coverage. A metric with zero
paired samples is **not measured** (see §6); it is data-gated or
hardware-gated.

---

## 4. Metrics

All metrics are computed only over paired `(estimate, reference)` samples on the
common clock. Units are fixed: brpm, bpm, millimetres, dimensionless fractions.

### 4.1 Pose

**Torso-PCK@20.** The v1 pose bar. Percent of correct keypoints among the
**torso keypoint set** — neck, left/right shoulder, left/right hip, pelvis —
whose estimated position lies within **20% of the torso span** of the reference.
The torso span is the distance between the neck and the pelvis midpoint in the
reference pose; it is the per-sample normaliser, so PCK@20 is scale-invariant
and comparable across subjects and rooms.

- Report torso-PCK@20 as the headline pose number, with per-keypoint breakdown.
- Because CSI pose is low-fidelity by design (`CONTEXT.md`), a coarse keypoint
  schema is used; the schema is fixed and versioned, and both reference and
  estimate are reduced to it before scoring.

**MPJPE.** Mean per-joint position error in millimetres, after aligning the
reference and estimate to a common root. Reported as a secondary absolute
error.

**PA-MPJPE.** Procrustes-aligned MPJPE: the estimate is aligned to the reference
by the similarity transform (translation, rotation, uniform scale) that
minimises error before measuring, removing global pose ambiguity. This is the
**primary cross-configuration pose number** because it is invariant to the
global frame, which a fixed-node CSI system cannot recover reliably.

Report MPJPE and PA-MPJPE together; never MPJPE alone.

### 4.2 Presence and count

**Presence F1.** Per-zone binary presence, evaluated at a fixed decision window
(e.g. 1 s) and rate. Precision, recall and F1 are reported per zone; the
headline is the **macro-F1 across zones**, with a whole-home micro-F1 for the
home-level claim. The positive class is "at least one person present".

- The reference is the fixture label or, on the eval rig, the camera or a
  human annotator; the source is named with every score.
- Presence is scored at the same window and clock as vitals (roadmap, Slice 2),
  not on a parallel path.

**Count MAE.** Mean absolute error of the per-zone and whole-home person count,
reported with its own coverage. Count is reported, not gated in v1 beyond the
`count.mae.home` ledger row.

### 4.3 Vitals

**Breathing MAE.** Mean absolute error of estimated breathing rate (brpm)
against the respiratory-belt reference, per window, paired on the common clock.

**Bland-Altman.** For any paired vital (breathing and heart), report the
agreement analysis, not just error:

- differences `d = estimate − reference` per paired window;
- **bias** = mean of `d`;
- **limits of agreement** = bias ± 1.96 × SD(`d`);
- the paired sample count and coverage;
- a Bland-Altman plot retained with the ledger entry.

MAE alone hides systematic bias; Bland-Altman exposes it. Both are always
reported. Pearson `r` and Lin's concordance correlation are reported as
secondary.

**Heart MAE.** Reported with a bar fixed from its own measured baseline
(roadmap, Slice 2) — no invented target. Heart rate behind a worn strap is the
harder reference; expected error is larger than breathing and is stated as
such.

### 4.4 Calibration and drift

Calibration is measured, not assumed: per-node drift, cross-session presence
variance and post-calibration vital MAE are ledger rows (`calibration.digest`,
`calibration.drift.per_node`, `presence.f1.cross_session`). Re-running
calibration on the same fixtures must reproduce the same artefact digest.

### 4.5 Uncertainty

Every statistical metric is reported with a confidence interval (bootstrap over
the pairing unit) and the per-fold spread for the leave-one-X protocols. A
point estimate without an interval is not a sufficient ledger row.

---

## 5. The honest-claims grading scheme

Every sensing claim carries exactly one of **five** grades — the single
authoritative set, also declared by `aquila_types::Witness` and
`data-contracts.md` §1: **Gated**, **Measured**, **Claimed**, **Data-gated** and
**Hardware-gated**. The grades refine the `Witness` vocabulary in `CONTEXT.md`:
**Measured**, **Claimed** and **Gated** are kept verbatim, and **Claimed** is
split into the two blocked sub-grades **Data-gated** and **Hardware-gated** so a
pending measurement names its blocker. This is an extension of the glossary, not
a contradiction of it.

| Grade | Meaning | May carry a number? |
| --- | --- | --- |
| **Measured** | Computed from real, replayed or simulated input, recorded in the commit-keyed ledger with non-zero paired samples, stated coverage and a reproducer command. | Yes, with its coverage and sample count |
| **Claimed** | Asserted without a recorded measurement. | No |
| **Data-gated** | Blocked on missing input: no fixture, no labels, no reference recording, or insufficient paired samples. A named, pending measurement. | No |
| **Hardware-gated** | Blocked on missing hardware: reference device, chest strap or belt, camera rig, or the ~9+ node deployment. A named, pending measurement. | No |
| **Gated** | Measured **and** passing a defined v1 bar. The only grade a headline claim may carry (`CONTEXT.md`). | Yes |

**Gated is the terminal state of a Measured claim**, not a fifth kind of
evidence: it is Measured plus a passed bar.

### 5.1 How a claim becomes Measured (the three gates)

A claim is promoted from Claimed (or a blocked sub-grade) to **Measured** only
when all three gates pass:

1. **Non-zero paired samples.** At least the metric's minimum paired sample
   count is available — estimates and references paired by key on the common
   clock. Floors: ≥ 30 paired windows for a rate metric (breathing, heart);
   ≥ 200 decisions and ≥ 3 sessions for presence; ≥ 500 reference frames and
   ≥ 3 subjects for pose. Zero paired samples is never Measured.
2. **Coverage.** The scored set covers the claim's declared scope, and the
   coverage fraction is recorded. A metric on a partial scope is Measured **for
   that coverage only**, and the claim states it. Coverage below the scope's
   floor is Data-gated for the uncovered part.
3. **Reproducer.** One command replays the recorded fixtures, recomputes the
   metric and writes the ledger row, with no hardware. The command, the fixture
   digest and the split seed are stored in the row.

The ledger row is the unit of proof. It records: `commit`, `protocol`,
`metric`, `value`, `ci`, `paired_samples`, `coverage`, `scope`,
`fixture_digest`, `model_digest`, `calibration_digest`, `reference_adapter`,
`reference_provenance`, `seed`, and `reproducer`.

### 5.2 Lifecycle and downgrade

```
Claimed ──(inputs missing)──▶ Data-gated
   │        ──(rig missing)──▶ Hardware-gated
   │
   └─(three gates pass)─▶ Measured ──(passes v1 bar)─▶ Gated
```

- A headline claim (in a README, release note or issue) may be **Gated** only.
- Quoting a number the ledger does not back is a defect; the claim reverts to
  Claimed.
- If coverage or paired samples regress below a floor, the claim **downgrades**
  from Measured to the appropriate blocked grade — it does not keep its old
  number.
- An engine result is never stronger than its reference: a metric scored
  against an uncalibrated or noisy reference inherits that limitation.

### 5.3 Reported but ungated

The following are reported as Measured (or labelled Data-/Hardware-gated) for
honesty and diffing, but are **never** promoted to Gated and never headline:

- every LOSO, LORO and cross-configuration result;
- cross-room and cross-floor/home results and count MAE;
- PA-MPJPE and MPJPE as absolute numbers;
- Bland-Altman limits of agreement;
- heart-rate MAE until its bar is fixed from baseline;
- any metric on a scope narrower than v1's reference deployment.

v1 makes **no** claim of out-of-domain accuracy or cross-environment
generalisation (roadmap). Cross-room is deliberately reported-but-ungated.

---

## 6. v1 target bars

The three gated bars, in-domain, evidenced by commit-keyed ledger rows
(roadmap, "What v1 shipped means"):

| Metric | Bar | Scope | Ledger row |
| --- | --- | --- | --- |
| In-domain torso PCK@20 | ≥ 65% | In-domain pose fixtures, single room | `pose.pck20.torso` |
| Presence F1 | ≥ 0.90 | In-domain presence fixtures, per zone (macro) | `presence.f1` |
| Breathing MAE | ≤ 2 brpm | In-domain, vs respiratory belt reference | `breathing.mae` |

Also required for v1, reported and diffed but with no fixed bar:

- heart-rate MAE (bar fixed from its own baseline);
- whole-home presence F1 and count MAE at ~9+ node scale (`presence.f1.home`,
  `count.mae.home`);
- PA-MPJPE and MPJPE;
- Bland-Altman bias and limits of agreement for breathing and heart;
- coverage and paired sample counts for every row.

No v1 bar is set on any out-of-domain protocol.

---

## 7. Published reference points (external, Claimed)

For calibration of expectations only. These are **external claims**, graded
Claimed until Aquila reproduces them; they are not Aquila measurements and must
never be quoted as such.

- **In-domain** (same subjects, room and configuration): roughly **65–75%**
  torso-PCK@20. Aquila's v1 bar (65%) sits at the conservative end of this band.
- **Cross-subject** (LOSO): roughly **60–65%** — a modest, not catastrophic,
  drop from in-domain.
- **Zero-shot cross-room** (LORO / cross-configuration): roughly **15–20%** —
  near-chance for pose, which is why cross-room is reported-but-ungated.
- **Presence** is the robust capability: in-domain F1 ≥ 0.9 is credible;
  cross-room presence degrades but is expected to remain usable.
- **Vitals**: breathing MAE of 1–2 brpm in-domain is the credible band; heart
  rate is materially harder and its error is expected to be several times the
  breathing error.

When Aquila measures any of these, the result is recorded in the ledger and the
external reference point is superseded for that scope.

---

## 8. Reproducer and ledger

Proposed shape of a reproducer (tooling TBD by ADR):

```
cargo xtask eval --protocol loso --metric torso_pck20 --commit <sha> \
    --fixtures <digest> --out ledger.json
```

Guarantees:

- Runs offline from recorded fixtures; no nodes, no camera, no strap.
- Deterministic: fixed seed, pinned model and calibration digests; identical
  input produces an identical ledger row.
- Writes one row per metric per protocol per commit; rows are diffable across
  commits and are the sole basis for every number Aquila states.

---

## 9. Open questions

- The exact torso keypoint schema and torso-span normaliser, to be fixed by ADR
  with the `aquila-inference` crate.
- Whether the vitals eval rig uses a respiratory belt, chest-strap-derived
  respiration, or both; the belt is the direct reference, the strap the
  independent cross-check.
- Whether MM-Fi evaluation is cross-dataset only, or also a training seed
  protocol with its own held-out split.
- The minimum-coverage floors per scope dimension, once the fixture corpus size
  is known.
- Whether the ledger view exposes arbitrary commit diffs or only the latest
  (shared with the dashboard spec).
