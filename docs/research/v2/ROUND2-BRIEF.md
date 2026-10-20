# Aquila v2 — Round-2 brief

Status: synthesis of round 1, written by the round-1 synthesis agent. Australian
English. This file is read by every round-2 agent, together with `BRIEF.md`,
`00-synthesis.md`, `EXPERIMENT-ENV.md`, and the `22-invention-*.md` notes.
Research is out-of-band: it never defines v1 scope.

---

## 1. Provenance

**Ingested in full (no sampling, no excerpts):** the complete transcripts of all
**20 primary "Invention discovery" sessions a–t** — user messages, assistant
messages, tool calls, tool results, and reasoning — plus **10 retry/interrupted
sessions** (c, d, e, f, f‑2, j, j‑2, k, l, l‑2), read directly from the OpenCode
session store (`~/.local/share/opencode/opencode.db`, opened read-only).

- **Combined size:** ≈ 16.3 MB raw / ≈ 14.2 MB rendered text, split into 375
  line-aligned chunks of ≤ 40 KB for reading.
- **Session IDs (primary a–t):** `ses_ec1ad3909ffet8j25LWtdMUk4B` (a),
  `ses_ec1ad13edffemFmP6r6pqf4cwb` (b), `ses_ec1acc58effeMMlFqCD15rUBL2` (c),
  `ses_ec1acb2b0ffeEdiWeoT5MOWNV1` (d), `ses_ec1aca038ffee8xboPPFO1do0u` (e),
  `ses_ec1ac8f2effeT2Ma4lKDI8qFKK` (f), `ses_ec1abff56ffeGGR0wBZfH27Bqy` (g),
  `ses_ec1abe6f9ffehgYPR4d5b1rJfd` (h), `ses_ec1abc8fefferYRf2ivCJ1wU0t` (i),
  `ses_ec1ab39d2ffetlxkRjMfmOfCyF` (j), `ses_ec1ab2bbeffekN7NPe60PybYjJ` (k),
  `ses_ec1ab1dfeffeLoikryVJtnmcwI` (l), `ses_ec1aae30dffeQpeL3EIhUYZ0FW` (m),
  `ses_ec1aa5cc6ffeAfJRucpnXpP0da` (n), `ses_ec1aa24d1ffeAFzp0zCsOcvr57` (o),
  `ses_ec1a9ce4cffeR3c5n3p1re7FKo` (p), `ses_ec1a9bbdfffeqcEkT0oPyqB1DU` (q),
  `ses_ec1a9a743ffelZkBcrfcEEETU7` (r), `ses_ec1a9920effepPOvn0jVKIeMVA` (s),
  `ses_ec1a92bf0ffelm90HZHR16l1iR` (t). Retry session IDs are recorded in the
  round-1 working notes; retries exist for c, d, e, f (×2), j (×2), k, l (×2).
- **Missing/unreadable:** none. `22-invention-a.md` … `22-invention-t.md` exist
  for **17 of 20 letters**; **f, j, l produced no note** (their sessions were
  interrupted mid-derivation — their reasoning survives only in the transcripts
  and is summarised below). ~36 transcript "patch" events (workspace snapshots
  naming `Cargo.lock` and `crates/aquila-api/*` in the `aquila` repo) are
  recorder artefacts, not invention content; they were noted and ignored.

---

## 2. Five synthesis findings

### Finding 1 — Convergence: one architecture emerged independently, ~a dozen times

Round 1's surviving notes converge on a single architecture:

> **A conducted-coherent polyhedral cell** (shared 40 MHz + conducted 2.4 GHz
> phase reference — the ESPARGOS pattern) as the only cheap, *proven* local
> gauge fix; **wireless gauge-fixing between cells**; the **static scene**
> (static multipath, LoS paths, ambient frames) as the ongoing reference;
> **two-way products/ratios** as the fiducial (ranges, hardware fingerprints,
> divergence tests); and **the residual after subtracting the static field** as
> the sensing signal.

Named instances: Coherent Weave (a), LIGHTHOUSE / Coherent Home (b), WireLock
(c), Clockless Anchoring (d), Lighthouse Aperture (e), Reciprocal Coherence
Fabric (g), Closure Weave (h), Coherent Polyhedral Mesh (i), Reciprocal Strobe
(k), Ambient-Coherent Aperture (m), Gauge Lattice (n), Closure Lattice (o),
coherence-locality (p), ApertureLock (q), Clutter Clock (r), Reciprocal Gauge
Mesh (s), Coherent Star (t). Every session's decisive experiment is a member of
**one family**: few boards, a conducted ground truth, and a phase-residual versus
cadence/holdover curve. Round 1 did not measure it anywhere.

### Finding 2 — Divergence: coherence-first versus geometry-first, and the bridge

Two coherent camps, disagreeing on what is *required*:

- **Coherence-first** (a, b, e, g, k, m, n, q, r, t): lock the fleet via
  common-transmitter reference frames, two-way reciprocity, steerable
  references, face-averaged phase loops and scene/clutter-locked tracking; then
  image coherently. Bound: σ_φ ≲ 0.3–0.5 rad at the working cadence.
- **Gauge-free / geometry-first** (d, h, o, p, s): closure phases, invariants-only
  fusion, self-survey and "manufactured coherence" from measured channels;
  sensing that never needs cross-node carrier phase. Bound: the space–bandwidth
  product and ambiguity resolution, not coherence.
- **The bridge** (s, t): measure each link's own propagation phase with the
  gauge-free two-way product; then any one-way frame on that link transfers
  phase algebraically ("coherence is derived from geometry"). If true, the
  coherence-first versus geometry-first split collapses into one hierarchy —
  and the whole programme hinges on *one number plus one trick* (Finding 4).

### Finding 3 — New facts (2024–2026) that change the round-1 picture

- **ESP-PPB** (GitHub `jonathanmuller/esp-ppb`, Feb 2026, GPL-3.0, **unrefereed**):
  FTM-disciplined VCTCXO (~0.1–1 ppb), a reference frame ~1 ms after every
  external CSI frame; **< 1° best / < 10° typical single-frame phase** on
  ESP32-C3; $50–80/node. The first *wireless* phase-coherent CSI claim on the
  ESP32 family — it directly challenges the round-1 assumption that wireless
  escapes are unproven (verification outstanding; none on S3).
- **ESP-SDR**: undocumented raw I/Q on the ESP32 family, phase-coherent across
  ESPARGOS One's eight antennas; ESP32-S3 has a continuous decimated-I/Q mode.
- **ESPARGOS One** is a product (€350/8 dual-pol faces; multi-array aperture
  kits) — the conducted pattern is commoditised; the volumetric/3D instance is
  not.
- **WiDRa** (JSAC 2024): sum-carrier-phase *differential* ranging < 1 mm on
  off-the-shelf Wi-Fi at 20 MHz; direction-finding explicitly deferred.
- **RTCSI / ML-Track** (WCNC 2023 / TMC 2025): round-trip CSI cancels
  asynchrony (0.96 → 0.04 rad); four Raspberry Pi CM4s, multi-link passive
  tracking at 0.23 m median. Round-trip products are validated prior art.
- **WiCAL** (2025): cabled collaborative antenna arrays, 15.6 cm median 3D.
- **LoSRef** (arXiv 2602.05344): the LoS path as an over-the-air delay+phase
  reference on commodity 802.11ax; dynamics to 20 dB below static clutter.
- **IEEE 802.11bf-2025** (WLAN sensing) standardises the procedure, not
  cross-node phase coherence.

**Still open after all 30 sessions:** no volumetric ESP32 array exists; no
wireless cross-node carrier coherence demonstrated on S3; no closure-phase /
interferometric treatment of a Wi-Fi sensing network; no measured M2.

### Finding 4 — The decisive quantities (all unmeasured on ESP32-S3)

The programme gates on a short list of numbers, none of which round 1 measured:

1. **σ_φ (M2):** per-packet cross-node phase residual after CFO removal at MAC
   cadence. Targets: ≤ 0.3–0.5 rad (λ/16–λ/36). Raw LO phase wraps every
   ~5–10 µs; after CFO removal the residual (10–100 Hz) gives ms–tens-of-ms
   coherence; a reference frame ~1 ms before/after a sensing frame turns this
   into the governing quantity (3.6° per 1 ms at 10 Hz residual; 36° at
   100 Hz).
2. **TX-instant jitter × CFO:** no TX hardware timestamp on ESP32; a 1 µs
   reply-interval uncertainty costs 0.3–0.6 rad at 48–96 kHz CFO. Reference
   datum: in-chip reply standard deviation < 1 µs (WCNC 2023, Raspberry Pi);
   unmeasured on S3.
3. **CSI capture-rate ceiling** (tracking bandwidth) and **face/packet
   averaging law** (does per-face phase noise average as √N?).
4. **Holdover / coherence budget** B = Δν_res·T_obs; update-rate optimum ~20 ms;
   oscillator drift versus re-calibration cadence.
5. **Sharpness arithmetic (resolution, not precision):** range resolution
   c/2B = 7.5 m at HT20; ~1.8 m stitching 13 × 20 MHz; ~0.3 m only with
   5–6 GHz wideband or UWB. Angular resolution λ/D ≈ 24° per 30 cm puck;
   1–3° at home scale; near-field curvature gives cm-class range within
   ~1.4 m of a puck, and a house-scale *coherent* fleet is everywhere
   near-field (Rayleigh distance 0.4–1.6 km) — mm–cm 3D if σ_φ ≤ ~10°.

Every round-1 decisive experiment (E1 / M2′ / LOCK-1 / Closure Gate / Two-Puck
Room Test / Phase-Star) is a way of measuring (1) ± (2) against a conducted
ground truth.

### Finding 5 — Gaps and errors to correct

- **Round 1 was literature-only.** No experiment or simulation environment
  existed; all quantitative claims are estimates. Round 2 must be
  simulation-heavy (see `EXPERIMENT-ENV.md` and the prompt below).
- **f, j, l are missing** (no notes; sessions interrupted). Their directions —
  phase-swept beacon / RIPS-style hub protocols (f); reciprocal reference
  holography (j); closure-phase mesh with a self-shadowed monostatic puck (l) —
  must be picked up in round 2.
- **Superseded claims:** "none of the wireless-only escapes is yet proven on
  the ESP32 family" is now challenged (ESP-PPB, unrefereed) and "no phase-
  coherent raw IQ" is superseded (ESP-SDR). On **S3 specifically**, the
  original statement stands.
- **Vocabulary conflict:** `CONTEXT.md` bans "phase-coherent RF reference",
  ESP-NOW, Doppler, raw I/Q — while the Track-3 invention space requires them.
  Flagged throughout round 1; any graduating invention needs a glossary change
  and an ADR.
- **No common notation or shared error budget** across notes. Round 2 should
  write to one ledger: σ_φ, coherence budget B, the ambiguity cascade
  (FTM → stitched delay → carrier phase), airtime, and target-coherence time
  (λ/2v ≈ 62 ms at 1 m/s).

---

## 3. ROUND-2 PROMPT (verbatim — the orchestrator appends one trailing character a…t, alone on the final line; that character is `<c>`)

```text
You are an inventor-researcher taking part in Aquila round 2
(github.com/belikh/aquila — WiFi CSI sensing for the home; reference hardware
ESP32-S3). Round 1 produced: docs/research/v2/BRIEF.md, twenty research notes
01–20, the synthesis 00-synthesis.md, and twenty invention sessions whose notes
are docs/research/v2/22-invention-*.md (a–t; f, j, l are missing — those
sessions were interrupted). A synthesis brief covering all thirty round-1
sessions is at docs/research/v2/ROUND2-BRIEF.md.

READ FIRST, IN FULL: ROUND2-BRIEF.md; EXPERIMENT-ENV.md; BRIEF.md;
00-synthesis.md; and at least three 22-invention-*.md notes nearest your track.
Use primary sources only (papers, standards, upstream code, datasheets); cite
DOIs/URLs.

MISSION: advance beyond round 1 on ONE front, in depth. Round 1 found the same
architecture in many guises and left one family of unmeasured numbers standing
between "coherent" and "not coherent". Round 2 must (a) RESOLVE the discrete
divergences in ROUND2-BRIEF.md (coherence-first vs geometry-first; what is
required vs optional), (b) ATTACK the decisive measurements — design, simulate
and numerically evaluate the gating experiments instead of merely specifying
them, (c) DEVELOP one leading invention into something concrete: protocol,
estimator, error budget and falsifier, and (d) EXPLORE what round 1 missed,
including the three missing directions — phase-swept beacon / RIPS-style hub
protocols; reciprocal reference holography; closure-phase mesh with a
self-shadowed monostatic puck.

EXPERIMENT ENVIRONMENT: the environment is specified in EXPERIMENT-ENV.md.
Python numerics are provided by the wrapper
`/home/matt/projects/aquila-v2-spec/sim-python` (a pip virtualenv on Python
3.12; it must be used instead of `.venv/bin/python`): NumPy, SciPy, SymPy,
pandas, matplotlib, scikit-learn, scikit-rf, h5py, zarr, networkx, trimesh,
pint, uncertainties. `nix develop .#sim` gives the Rust toolchain, jq, gnuplot
and graphviz (tooling only); the repo's deterministic CSI simulator is
`cargo run -p aquila-capture -- synth`. GNU Radio, openems, octave and PyTorch
are NOT provisioned (they build from source here and the disk is tight) — if a
claim needs one, say so and compute an analytic bound instead. Wolfram is
reached through the Wolfram MCP tools. GROUND YOUR PROPOSALS IN SIMULATION where
useful: produce the numbers — phase residuals, CRLB curves, ambiguity-resolution
success rates, coherence budgets, airtime — rather than asserting them. A
simulation is not a measurement (label accordingly, per EXPERIMENT-ENV.md);
state parameters used and keep scripts reproducible; put figures, tables and
code snippets in your note.

CHOOSE ONE TRACK (do not cover all):
(T1) The coherence gate — the few-board conducted-truth phase-residual
     experiment, simulated and specified to the register level (CFO tracking,
     TX-jitter budget, face/packet averaging).
(T2) The gauge-free track — geometry-first self-survey, closure-phase / RTI
     imaging with no cross-node coherence; quantify what "sharp" survives.
(T3) The cell — the volumetric ESP32 aperture: array manifold with mutual
     coupling and polarisation, near-field curvature; beamforming and
     super-resolution performance.
(T4) The protocol — one shared arbitration/scheduling design (ping + unique
     replies + broadcast reference frames) meeting the airtime, latency and
     holdover budget at home scale.
(T5) The synthesis risk — pick the single most likely failure that would kill
     the programme and try to falsify it computationally.

RULES: separate ESTABLISHED / HYPOTHESIS / SPECULATION; be honest about hardware
tier (ESP32-S3 / ESP32 family / other COTS Wi-Fi / SDR-grade); for any invention
state: what is genuinely new, the physical mechanism, what must be true, the
falsifying experiment, ESP32-S3 feasibility, cost, and whether transmit is
needed; if a quantity closes a capability, name it and the minimal reference
that crosses it. Use CONTEXT.md vocabulary where it applies and flag conflicts
(research is out-of-band).

DELIVERABLE: write your full note to
docs/research/v2/23-<slug>-<c>.md
(choose a short slug; `<c>` is the single final character of this prompt),
including your simulation code or a pointer to it and the numbers you produced.
Then return, as your only final message, a ≤ 400-word summary: exact file path,
the advance you made, the key numbers, the falsifier, and ESP32-S3 feasibility.
```

(The prompt ends with the DELIVERABLE paragraph; the orchestrator appends the
single trailing character — one of a…t — alone on the final line, exactly as in
round 1. Each round-2 agent must therefore write
`docs/research/v2/23-<slug>-<c>.md` and return a ≤ 400-word summary.)

---

## 4. Round-2 acceptance criteria (for the orchestrator)

1. Every track produces **numbers**, not literature summaries alone (a table of
   simulated/derived values with code provenance).
2. The **gauge question is settled to the extent simulation can settle it**:
   under the best-supported ESP32-S3 parameter ranges, does the per-frame
   cross-node phase residual stay ≤ 0.5 rad at the working cadence, and with
   what margin? State the killing parameter values explicitly.
3. At least one track fully specifies and computationally checks a **complete
   end-to-end chain** (calibration → coherence → localisation) with an error
   budget and a named falsifier.
4. The three missing directions (f, j, l) receive at least one dedicated
   treatment each.
5. All notes write to the shared ledger (σ_φ, coherence budget B, ambiguity
   cascade, airtime, target coherence time) so the next synthesis can compare
   like with like.
6. Simulations follow EXPERIMENT-ENV.md honesty rules (labelled as simulations,
   parameters reported, no guesses dressed as computations).

---

## 5. Operational constraints (orchestrator)

- **Disk is tight.** The host hit a 100 % full filesystem during round 1, which
  silently broke file writes mid-run. Keep every artefact small: no large arrays
  on disk, no multi-hundred-MB datasets, no full-resolution FDTD meshes. Clean
  up your own temporary files when done. Prefer computed scalars, small CSVs and
  figures over bulk output.
- **One shared shell.** Enter the environment with `nix develop .#sim` from the
  worktree root. The closure is pre-warmed for you; do **not** add new Nix
  inputs or packages — if a tool is missing, say so in your answer instead.
- **Wolfram** is reached through the Wolfram MCP tools
  (`WolframLanguageEvaluator`, `WolframContext`, `WolframAlpha`). Do not attempt
  a local Wolfram install.
