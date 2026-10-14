# Risk register and open questions

**Status:** settled brief, living register. **Spelling:** Australian English.
**Licence:** GPL-3.0-only (assumed and itself at risk — see §3). **Claims:** honest only.

This document exists because Aquila's hardest problems are not in the code that
is already planned — they are in what the physics, the licensing and the tooling
will actually permit. It is deliberately blunt. A risk that is written down is a
risk someone owns; a risk left implicit is a v1 bar that quietly fails.

It is a **register**, not a plan: it names likelihood, impact, mitigation and
owner, and it collects the questions the tickets must answer before the affected
slices can exit honestly.

---

## Owner legend

There is no team yet. Owners are **roles**, so that responsibility survives
whichever person fills it.

| Alias | Owns |
| --- | --- |
| `lead` | project lead; break-glass decisions and honest-claim wording |
| `core` | sensing core / DSP / signal conditioning |
| `rf` | hardware, ESP32 firmware, RF and capture |
| `data` | datasets, training, model provenance |
| `legal` | licensing, compliance, ACMA/RF legality |
| `packaging` | add-on, HACS component, Home Assistant integration |
| `measurement` | ledger, metrics, honesty enforcement |

## Scoring

- **Likelihood:** Low (<20 %), Medium (20–60 %), High (>60 %) — over the v1
  window, given the settled brief.
- **Impact:** Low (slows one slice), Medium (delays/reshapes a slice), High
  (moves a v1 bar, a licence, or the reference hardware).
- **Severity** = the pair; there is no numeric product, because a high-likelihood
  medium risk (data feeds the model) matters differently from a low-likelihood
  high risk (a legal trap).

---

## 1. Hardest risks, stated bluntly

These four are the reasons v1 may miss its own bars. Everything else is
management.

1. **Cross-room and cross-floor accuracy will be much worse than in-domain
   numbers, and the brief already concedes it.** WiFi CSI is dominated by
   multipath and geometry; a model that scores on in-domain fixtures in one room
   does not automatically survive a different room, a different furniture layout,
   or a different floor. The v1 bars are *in-domain* precisely because nobody can
   honestly promise more. The risk is not that cross-room is weak — it is that
   Slice 5/6 exit criteria and the "whole-home" framing imply more than the bar
   supports.

2. **There is no phase-coherent RF reference, and phase is where pose and
   breathing live.** The brief defers coherence to "future-only" and keeps
   feature-level fusion. That is a defensible scope cut, but it means the richest
   signal (cross-node phase) is unavailable, and any absolute-phase feature is
   unreliable per-frame. The realistic v1 feature set is amplitude/motion-based;
   whether that clears torso-PCK@20 ≥ 65 % is genuinely open.

3. **ESP32 CSI tooling is immature and under-documented.** CSI comes from a
   proprietary modem blob, through `esp-csi`, with changing subcarrier counts,
   LTF configuration and timing across silicon revisions and ESP-IDF versions.
   "Raw CSI over UDP" is a promise the firmware may only partly keep, and dropped
   or malformed frames directly poison vitals.

4. **Model provenance is unresolved and may be a legal dead end.** "Seed from
   ruvnet NC weights, fine-tune on MM-Fi" combines at least two sources whose
   licences may forbid exactly what Aquila wants to do (ship GPL-3.0-only
   derivatives, non-commercially). If those terms are incompatible or
   non-redistributable, there is no seed and no fine-tune as written.

---

## 2. Technical and accuracy risks

| ID | Risk | L | I | Mitigation | Owner |
| --- | --- | --- | --- | --- | --- |
| R-01 | Cross-room / cross-floor accuracy collapses versus in-domain; whole-home bars are not evidenced by per-room fixtures. | High | High | Report in-domain and cross-room separately; make Slice 5/6 exit criteria honest about degradation; keep v1 zone scope room-level and label cross-floor staged. | `core` |
| R-02 | A single global whole-home calibration cannot remove per-room geometry effects; presence variance across sessions stays high. | Med | High | Measure F1 variance cross-session (Slice 3 ledger row) before claiming success; if it fails, escalate whether per-room staging must move earlier. | `core` |
| R-03 | Absence of phase coherence caps pose/vitals; amplitude-only features miss the PCK@20 bar. | High | High | Baseline amplitude/motion features first; treat phase as unreliable per-frame; decide from the fixture result whether pose is v1 or staged. | `core` |
| R-04 | Person count has no v1 bar and is weakly separable from presence; a "count" entity overclaims. | Med | Med | Keep Count as recorded/statistical, never gated, until a defensible band exists; report count MAE rather than asserting it. | `measurement` |
| R-05 | Heart rate has no fixed bar ("baseline-fixed"); the honest claim is undefined and could drift. | High | Med | Fix the bar from the Slice 2 baseline and freeze it in the ledger; keep HR statistical and non-gated in v1. | `measurement` |
| R-06 | Replay/sim-first overfits to fixtures; in-domain bars do not transfer to live nodes (Slice 7). | Med | High | Make live-vs-replay parity an explicit Slice 7 exit ("drift matches replay-of-capture within tolerance"); record live metrics under a distinct authority. | `core` |
| R-07 | Ground truth used for development leaks into runtime or inflates fixtures. | Low | High | Keep GroundTruth research-only and never a runtime input; label every derived number `Declared`; separate eval split from training split. | `measurement` |

## 3. Data and model-provenance risks

| ID | Risk | L | I | Mitigation | Owner |
| --- | --- | --- | --- | --- | --- |
| R-08 | The ruvnet NC weights' licence is incompatible with shipping a GPL-3.0-only derived model. | High | High | Establish exact licence terms before any use; if non-redistributable, drop it as a seed and record the decision as an ADR. | `legal` |
| R-09 | MM-Fi (or any NC dataset) cannot legally fine-tune weights that ship in a non-commercial GPL project. | High | High | Audit the dataset licence; seek a permissive substitute or restrict that arm to eval-only, never shipped. | `legal` |
| R-10 | Own-capture data (3 floors, 9+ nodes, multiple people, sessions) is far short of what training and evidencing the bars requires. | Med | High | Define a capture plan with volume targets before Slices 4–6 exit; treat missing data as a blocked slice, not a rounding error. | `data` |
| R-11 | Reference labels (camera-GT, breath belt) are unavailable, unconsented, or unlawful to collect at home. | Med | Med | Scope reference capture narrowly, with consent and local-only storage; treat camera-GT as research-only and never runtime. | `data` |
| R-12 | Dataset splits are ambiguous or leak between train/eval, making gated numbers meaningless. | Med | Med | Pin the eval-split digest in `tools`; version the split; a split change makes results `uncomparable`. | `measurement` |

## 4. Legal and licensing risks

| ID | Risk | L | I | Mitigation | Owner |
| --- | --- | --- | --- | --- | --- |
| R-13 | "Non-commercial" is a project intent, not an enforceable term: GPL-3.0 grants commercial use explicitly. | High | Med | Decide how intent is expressed (README/`CONTRIBUTING`, not the licence); do not claim the licence restricts commerce. | `legal` |
| R-14 | Adding an NC clause to a GPLv3 licence is not a valid combined licence for the ecosystem. | Med | Med | Do not graft NC onto GPL; if non-commercial must be legally binding, choose a different, compatible licence deliberately. | `legal` |
| R-15 | GPL-3.0-only component/add-on conflicts with Home Assistant's Apache-2.0 ecosystem expectations. | Low | Med | Confirm the HACS/Apache interaction; place licence files correctly; document the boundary. | `packaging` |
| R-16 | Any active-sensing or transmit experiment crosses ACMA LIPD limits. | Low | High | Keep transmit out of v1; confine it to a research note; require legal review before any Tx code. | `legal` |

## 5. Hardware, CSI-tooling and RF risks

| ID | Risk | L | I | Mitigation | Owner |
| --- | --- | --- | --- | --- | --- |
| R-17 | ESP32-S3 CSI is unstable/under-documented: subcarrier count, LTF config and per-frame timing vary with IDF version and silicon rev. | High | High | Pin one firmware + IDF + silicon rev as the reference; capture recorded fixtures from it; document the CSI shape as a contract. | `rf` |
| R-18 | The S3 CSI ceiling (subcarriers, snr, frame rate at 20/40 MHz HT) cannot support pose. | Med | High | Measure the ceiling from recordings before committing pose to v1; keep C6/C5/array as a documented fallback, not a v1 assumption. | `rf` |
| R-19 | Reference device churn (IDF releases, silicon revisions, EOL) breaks replay/live parity mid-project. | Med | Med | Freeze the reference toolchain; treat any upgrade as an ADR with a re-baseline of fixtures and ledger rows. | `rf` |
| R-20 | "Raw CSI over UDP to the core" proves impractical (payload size, rate, buffering) on real S3 nodes. | Med | High | Prototype the live transport on real nodes early (spike before Slice 7); measure sustained rate and loss in the ledger. | `rf` |

## 6. Multi-node timing and streaming risks

| ID | Risk | L | I | Mitigation | Owner |
| --- | --- | --- | --- | --- | --- |
| R-21 | Software time sync over consumer WiFi is too coarse for coherent node-pair fusion. | Med | High | Quantify achievable sync precision early; if inadequate, restrict fusion to loose feature-level alignment and say so. | `core` |
| R-22 | Per-node drift cannot be estimated without a phase-coherent reference; residual jitter breaks breathing MAE ≤ 2 brpm. | Med | High | Estimate drift from timing/amplitude statistics, not phase; set the vitals window from measured jitter, not assumed. | `core` |
| R-23 | UDP loss, reordering and bursts silently corrupt vitals and pose. | High | Med | Detect and record loss in the session; a session with hidden loss must not carry a `Measured` gated result. | `core` |
| R-24 | Clock-grid alignment across 9+ nodes is O(n²) and does not scale to the reference topology. | Med | Med | Prove the alignment algorithm at reference scale in sim before live; budget latency explicitly. | `core` |

## 7. Packaging and Home Assistant risks

| ID | Risk | L | I | Mitigation | Owner |
| --- | --- | --- | --- | --- | --- |
| R-25 | HA add-on constraints (ingress, host networking for UDP, local-only access, no extra ports) reshape the host design late. | Med | Med | Validate ingress + UDP on a clean HA OS install in a spike before Slice 8; treat networking as an ADR. | `packaging` |
| R-26 | Rust → WASM dashboard stack is heavy or does not fit the add-on image/size budget. | Med | Med | Prototype the WASM build and size early; `system.wasm.size.dashboard` is a recorded metric from day one. | `packaging` |
| R-27 | Reproducible add-on/HACS install drifts (base images, dependencies, secrets baked into the image). | Med | Med | Pin base images and toolchains; `release.build.digest`; never bake `${HOME}` or secrets into committed files. | `packaging` |
| R-28 | HACS component and dashboard are tempted to own sensing logic, breaking the "component speaks only to the host" rule. | Med | Med | Enforce the boundary in review; the component is presentation and transport only. | `packaging` |

## 8. Project and honesty risks

| ID | Risk | L | I | Mitigation | Owner |
| --- | --- | --- | --- | --- | --- |
| R-29 | Research tracks (ESP32-as-SDR, coherent arrays, active sensing, camera GT) leak into v1 branches. | Med | Med | Hard-exclude them from v1; keep each behind its own research note and issue labels; no v1 slice depends on one. | `lead` |
| R-30 | An in-domain ledger row is paraphrased into a general accuracy claim in docs, UI or release notes. | Med | High | Enforce the Witness grade; release checklist requires every headline claim to cite a gated ledger row on a named split and seed. | `measurement` |
| R-31 | Dependency-DAG enforcement or the crate map becomes theatre: declared but not checked, or so strict it blocks legitimate splits. | Low | Med | `structure.dag.violations` checks a committed allowlist; allowlist changes are explicit, reviewed edits. | `measurement` |
| R-32 | Slice exit criteria drift from the ledger's evidence, so "done" means "merged", not "measured". | Med | High | A slice is not done until its ledger rows exist at the target bar and a diff is available; no exceptions. | `lead` |

---

## 9. Open questions the tickets must resolve

Each question below blocks a concrete decision. A ticket that touches the
affected area must resolve the question (or explicitly defer it with an ADR)
before its slice can exit.


**Accuracy and cross-domain**

- **Q-01:** What is the measured cross-room and cross-floor degradation versus in-domain, and at what point does Aquila stop attributing a result to a room?
  *Blocks:* Slice 5/6 exit criteria; whether zone attribution is a v1 claim.
- **Q-02:** Can one global whole-home calibration meet the v1 bars across heterogeneous rooms, or does the brief's "global only" force a room-agnostic model that misses them?
  *Blocks:* Slice 3 design; whether per-room staging must move earlier than planned.
- **Q-03:** With no phase-coherent RF reference, which features survive per-frame and across node-pairs, and can any pose or breathing bar be met without phase alignment?
  *Blocks:* the `aquila-signal` feature set, fusion design, and whether coarse pose is realistically in v1.
- **Q-04:** What does "in-domain" mean precisely — one room, one home, one furniture layout, one device set — and does the reference deployment stay inside it?
  *Blocks:* the honest wording of every gated claim and the v1 release statement.
- **Q-05:** Is multi-person count even in v1's honest claim set, and what count-MAE band is defensible given no bar in the brief?
  *Blocks:* whether Count ships as a claimed output; presence/count separation.
- **Q-06:** What reference and split fix the heart-rate bar, given the brief defers it to "baseline-fixed"?
  *Blocks:* Slice 2 exit, vitals model choice, gated vs statistical classification.

**Data, model and provenance**

- **Q-07:** What are the exact licence terms of the ruvnet NC weights, and can GPL-3.0-only Aquila legally ship derivatives of them?
  *Blocks:* whether those weights may be a seed at all; `aquila-inference` licensing; packaging.
- **Q-08:** Can MM-Fi (or any NC dataset) legally fine-tune weights that ship and are redistributed, and are derived weights themselves redistributable?
  *Blocks:* the training pipeline, the eval split, and model distribution.
- **Q-09:** How much own-capture data, across 3 floors / 9+ nodes / multiple people / multiple sessions, is needed to train and to evidence the bars, and who captures it?
  *Blocks:* the data plan and the entry criteria for Slices 4–6.
- **Q-10:** What reference measurement (camera-GT, breath belt) is available, consented and lawful to collect, given camera-GT is research-only?
  *Blocks:* the `GroundTruth` adapter, fixture labelling, eval methodology.
- **Q-11:** What exactly separates the training split from the eval split, and how is leakage prevented when both come from the same small capture set?
  *Blocks:* the meaning of every gated number; the pinned eval-split digest.

**Legal and licensing**

- **Q-12:** What concrete mechanism expresses "non-commercial" given GPL-3.0 grants commercial use, and is an added NC clause even compatible with GPLv3?
  *Blocks:* the `LICENSE` file, contribution terms and repo policy.
- **Q-13:** Does shipping the HACS component and add-on under GPL-3.0-only conflict with Home Assistant's ecosystem expectations, and where must each licence file live?
  *Blocks:* packaging manifests and component licence headers.
- **Q-14:** Is any active-sensing or transmit capability a v1 concern at all, or is it permanently out of scope behind a research note?
  *Blocks:* scope containment and the ACMA/LIPD compliance posture.

**Hardware and CSI tooling**

- **Q-15:** Which exact ESP-IDF version, `esp-csi` revision and S3 silicon rev produce stable, documented CSI, and what is the CSI shape contract (subcarriers, LTF config, rate)?
  *Blocks:* the reference firmware choice, the fixture capture harness, and live-vs-replay parity.
- **Q-16:** Does the S3 CSI ceiling support pose, or does the reference device need C6/C5 or a node array?
  *Blocks:* the reference-hardware ADR and whether pose can be a v1 bar on S3.
- **Q-17:** What is the fallback when frames are dropped, malformed or gapped, and does any gated result tolerate it?
  *Blocks:* `CsiSource` live adapter behaviour and session-integrity semantics.

**Timing and streaming**

- **Q-18:** What software time-sync precision is achievable across 9+ nodes over consumer WiFi, and is it sufficient for node-pair fusion and breathing?
  *Blocks:* the `Clock` design, drift-estimation method and fusion viability.
- **Q-19:** How is per-node drift estimated without a phase-coherent reference, and what residual jitter is tolerable for breathing MAE ≤ 2 brpm?
  *Blocks:* the `aquila-sensing` drift-estimation algorithm and the vitals window length.
- **Q-20:** How are UDP loss and reordering detected and recorded so that no corrupted session carries a `Measured` gated result?
  *Blocks:* session integrity, ledger authority, and Slice 7 parity claims.
- **Q-21:** What end-to-end latency is acceptable for HA automations, and does batching for fusion meet it at reference scale?
  *Blocks:* windowing, the `Sink` adapter and host streaming design.

**Packaging, process and honesty**

- **Q-22:** What HA OS/add-on constraints (ingress, host networking for UDP, local-only access, WASM serving) bind `aquila-addon`/`aquila-api` and the dashboard?
  *Blocks:* the host/dashboard design, Slice 7/8 packaging and the security posture.
- **Q-23:** Which exact dependency-DAG edges are allowed, and how is the allowlist kept honest without blocking legitimate crate splits?
  *Blocks:* `structure.dag.violations` policy, the crate map and the DAG ADR.
- **Q-24:** What stops a passing in-domain ledger row from being paraphrased into an out-of-domain claim in docs, UI or release notes?
  *Blocks:* the release checklist, dashboard wording and Witness-grade enforcement.

---

## 10. How this register is used

- A slice may not exit while an open question that blocks it is unresolved.
- Resolving a question produces either an entry in this file's response, a
  `CONTEXT.md` glossary change, or an ADR — never a silent assumption.
- New risks are added with an owner; risks without an owner are the lead's.
- Severity is re-scored at each slice exit, because data, tooling and licensing
  facts will change what is possible.
