# Aquila research tracks

Three out-of-band research tracks. None is v1 scope. Each is written to be
picked up later as its own **wayfinder map** — a shared set of decision
tickets on the GitHub tracker — rather than folded into the v1 delivery.

Shared framing: v1 is the sensing core, global whole-home calibration, the HACS
custom component and the ingress dashboard. It proves the in-domain bars with
recorded fixtures, replay and simulation. These tracks exist because three
capabilities that v1 deliberately defers — camera labels, active waveform
control, and phase-coherent multi-node RF — are the plausible next cliffs in
accuracy and in what "sensing" even means. Each is cheap to *scout* and dear to
*build*, so each deserves its own map and its own kill criteria.

Related: [esp32-as-sdr.md](./esp32-as-sdr.md) is the detailed primary-source
note underpinning Track 3.

---

## Track 1 — Camera-based ground truth and cross-modal training

**Question.** Can commodity cameras cheaply and automatically label CSI, so that
presence, count and coarse pose models are trained against dense ground truth
instead of sparse hand-labelling?

**Why it matters to Aquila.** The v1 measurement ledger is only as honest as its
ground truth, and hand-labelling thousands of captures does not scale. The v1
bars (torso-PCK@20 ≥65%, presence F1 ≥0.9) are in-domain and fixture-backed, but
every model improvement beyond v1 needs more, better-labelled data. Camera
labels are the obvious lever: a two-node rig with a camera overhead produces
per-frame pose, count and presence automatically, synchronised against the CSI
stream. Because Aquila is non-commercial, non-commercial (NC) research datasets
are usable for training, which removes the licensing objection that would
otherwise block this track.

**What success looks like.**

- A rig spec that other people can reproduce: fixed camera-to-floor geometry,
  a software time-sync method that ties CSI frames to video frames within a
  stated tolerance, and a calibration procedure.
- An auto-labelling pipeline that emits `GroundTruth` records in the same
  vocabulary the v1 `GroundTruth` seam already uses (presence, count, pose
  keypoints, breathing reference), so a camera capture is just another ground-
  truth adapter.
- A measured label-quality report: label noise estimated against a small hand-
  labelled subset, and the cross-modal transfer gain quantified — models trained
  on camera-labelled data evaluated on held-out CSI with a *different* reference
  (e.g. the reference breath belt), not on camera data again.
- A privacy posture note: what is captured, where it is stored, who can see it,
  and why the rig is research-only and never ships in the add-on.

**Rough cost.** A few hundred AUD of camera plus mounts; one node-pair rig; a
week or two of engineering for sync and auto-labelling; ongoing storage for
video. The dominant cost is the sync work and the label-noise analysis, not
hardware.

**Integration if it succeeds.** The auto-labeller becomes a new `GroundTruth`
adapter behind the existing seam — no core change. The produced datasets become
recorded fixtures for model fine-tuning, extending the MM-Fi seed corpus with
own-capture data. Nothing about the v1 runtime changes; this track feeds the
training loop, not the inference loop.

**Deferred by design.** Camera-GT is research-only in the brief. This map should
end with a rig recipe and a dataset, not with a shipped feature.

---

## Track 2 — Active sensing and reinforcement learning

**Question.** Can nodes act — not just listen — to improve sensing? Concretely:
can an agent choose per-node actions (which subcarriers to emphasise, how to shape
the transmit pattern, where to notch) to maximise a camera-derived reward, within
the compliant limits of class-licensed Wi-Fi?

**Why it matters to Aquila.** v1 nodes are dumb sensors; the add-on does all the
thinking, and every node streams passively. That is the right v1 posture, but it
fixes the waveform and the geometry. Many failure modes — deep multipath, an
obstructing wall, a still subject — are not fixable by better DSP on a fixed
waveform. Active sensing is the lever: let the system perturb the channel and
observe the response. The reward signal comes from Track 1's camera labels, which
is why these two tracks interlock.

**What success looks like.**

- A precise statement of the **action space that is legal**: only compliant
  802.11 frames (`esp_wifi_80211_tx()` sends beacons, probe requests/responses,
  action frames and non-QoS data — not encrypted or QoS frames), only within
  Australia's LIPD class licence (shared frequencies, power limits, no
  interference protection). Anything off-standard needs an apparatus licence and
  is out of scope.
- A simulation-first result: an RL agent in the replay/sim harness (v1 already
  commits to replay+sim first) that beats a passive baseline on a defined
  metric — e.g. presence F1 or count accuracy under hard multipath — with a
  measured reward-correlation check against camera labels.
- A hardware spike on a small rig proving the chosen actions are actually
  expressible through `esp_wifi_80211_tx()` and observable in CSI, plus an
  interference and duty-cycle budget.
- An explicit negative result is a success too: if the compliant action space is
  too small to matter, say so and close the track.

**Rough cost.** Most of the cost is simulation and engineering, not hardware: an
RL loop in the existing harness, a reward adapter off Track 1, then a modest rig
spike (the same node-pair rig as Track 1 can be reused). Weeks-to-months of one
person's time; the legal analysis is cheap and must come first.

**Integration if it succeeds.** A node action becomes a new field on the
existing direct node control channel — the core already owns node control. The
policy runs add-on-side (nodes stay dumb; the add-on does inference), so the
seam is a new action-emitting controller behind the core's control plane, plus a
reward adapter behind the `GroundTruth` seam. v1's dumb-sensor posture is
preserved: nodes receive actions, they do not decide them.

**Ethical/legal guardrail.** Active emission changes the RF environment for
neighbours. Any success must carry a compliance and duty-cycle budget, and a
default-off posture.

---

## Track 3 — ESP32 as an SDR and the phase-coherent CSI array

*This track is summarised from [esp32-as-sdr.md](./esp32-as-sdr.md), which holds
the primary sources; read that note for the underlying evidence.*

**Question.** Two coupled questions: (a) is the ESP32 a usable software-defined
radio, and (b) can Aquila build a **phase-coherent array** of ESP32 nodes sharing
an RF reference, so that cross-node *raw phase* becomes fusable?

**Why it matters to Aquila.** v1 is explicitly **feature-level fusion** with a
**software time sync and per-node drift estimation**; a phase-coherent RF
reference is future-only. Cross-node raw phase is the thing that unlocks
angle-of-arrival, delay and RF-SLAM — a materially richer view than per-node
amplitudes and features. If it works, the whole-node-pair model changes from
"fuse features" to "fuse a coherent aperture".

**What the note establishes (summary of esp32-as-sdr.md).**

- Standard Espressif parts are **not** general SDRs: the PHY is fixed undocumented
  hardware, the MAC is largely a proprietary blob, and public API does not expose
  the RF ADC/DAC.
- **Change (2025–26):** `ESPARGOS/esp-sdr` found an undocumented debug path that
  bypasses the fixed-function modem and exposes **raw receiver I/Q**. It is
  **receive-only**, tunable only as *software attempts* (100–6000 MHz, 1 MHz
  steps, **not a guaranteed range**), **uncalibrated** in gain and power, and
  **gapped**. ~80 MSa/s bursts on most parts; S31 streams continuously to
  20–40 MSa/s (experimental). Parts of the firmware are AI-generated. P4 has no
  radio.
- Verdict: a **partial, receive-only SDR** — real raw I/Q, but narrow-duty,
  uncalibrated, PHY-bypassing, and chip-subset. It is **not** competitive with a
  ~US$30 RTL-SDR (RX) or a ~US$200–300 PlutoSDR/HackRF (wider tune, calibrated,
  half/full-duplex Tx).
- For Aquila, esp-sdr's raw IQ is **not automatically better than CSI**. CSI is
  already the OFDM complex channel response; the binding constraints are phase
  coherence, calibration and multipath — exactly where esp-sdr is weakest.
- The larger near-term win is the **array**: ESPARGOS uses ordinary CSI across
  **phase-synchronised ESP32s** for angle-of-arrival and delay (ESPARGOS One is a
  4×2 dual-polarised 2.4 GHz array at €350). Tx is constrained to compliant
  802.11 shapes; arbitrary Tx is a legal boundary, not just a technical one.
- Extra notes: 5 GHz is native only on the C5/C61 family; no variant has native
  sub-GHz. A receive-only SDR-ish path also makes passive eavesdropping and
  red-team attack demos easier.

**What success looks like.**

- A reproducibility check: repeat the esp-sdr raw-I/Q claim on the reference
  ESP32-S3 and state, in Aquila's own measurements, the real usable bandwidth,
  calibration state and duty cycle.
- For the array: a demonstrated shared RF reference across N nodes (ESPARGOS-
  style) with a measured cross-node phase-stability number, and an
  angle-of-arrival or delay estimate from ordinary CSI that beats v1's
  feature-level fusion on a defined metric.
- A clear verdict on whether coherent raw-phase fusion is worth moving from
  "future-only" into a roadmap, or whether the calibration and legality burden
  keeps it out.

**Rough cost.** Highest of the three. An ESPARGOS-style coherent array is
low-hundreds of euro in parts but the real cost is calibration and reference-
distribution engineering — weeks-to-months. The esp-sdr reproducibility spike
alone is a small, cheap side-quest (a couple of S3 boards and a weekend) and can
run independently of the array work.

**Integration if it succeeds.** A coherent array would change the core's fusion
stage from feature-level to coherent-phase, which touches the `CsiSource` and
fusion seams — this is precisely why it is **future-only** and a separate map,
not v1. The esp-sdr reproducibility spike has no v1 integration at all; it is
evidence gathering. Note that "raw SDR" and "the array" are separable: the array
uses ordinary CSI and may succeed even if raw I/Q does not.

---

## How the tracks relate

- Tracks 1 and 2 interlock: Track 1 supplies the camera labels that Track 2 needs
  as reward. Track 2 can be scouted in simulation before Track 1 hardware exists.
- Track 3 is orthogonal to both and gated on calibration and legality.
- All three assume the v1 replay/sim harness, recorded fixtures and the existing
  seams (`CsiSource`, `Clock`, `PoseModel`/`VitalsModel`, `GroundTruth`, `Sink`,
  `MetricSink`). Each should extend those seams via adapters rather than fork the
  core.
- Each track should end with a verdict ticket — including a documented *negative*
  result — and only then a decision on whether to open a wayfinder map for
  delivery.
