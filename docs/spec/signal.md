# Spec: `aquila-signal` — CSI signal conditioning

Status: draft for review. Scope: the signal-conditioning module only.

This spec defines one deep, pure module: `aquila-signal`. It turns a stream of
recorded CSI frames into per-node signal features (`presence` energy, motion
residuals, spectrograms, breathing/heart-rate candidates). Everything in this
crate is a pure function over owned data. It performs no IO, reads no clocks,
and holds no globals. The transport (`CsiSource`), time base (`Clock`), models
(`PoseModel`/`VitalsModel`), ground truth (`GroundTruth`), and sinks (`Sink`,
`MetricSink`) are seams *outside* this module.

The DSP design borrows the genuinely sound parts of RuView — phase
sanitisation, bandpass/resonator filtering, autocorrelation-style periodicity —
but rebuilds them as a small, testable, deterministic pipeline instead of a
crate sprawl.

## 1. Boundaries and non-goals

In scope: CSI decoding and validation; subcarrier layout projection; hardware
(normalisation) coefficients; phase unwrapping and sanitisation; bandpass /
resonator filtering; motion and residual features; spectrogram and
ballistocardiogram-style (BVP) extraction; subcarrier selection.

Out of scope, and explicitly *not* to be smuggled in:

- **No IO, no globals.** No file, socket, clock, environment, thread, `static mut`,
  or lazily-initialised state. Autocorrelation lags, filter coefficients, STFT
  windows, and twiddle tables are `const` values or passed-in config, never
  mutable statics.
- **No cross-node phase in v1.** Nodes are not phase-coherent (no RF reference).
  Inter-node features use amplitude magnitude and event timing only. Absolute
  inter-node phase, conjugate cross-node products, and angle-of-arrival are
  future work behind the phase-coherent-RF-reference decision.
- **No model inference.** This crate emits features; the `PoseModel` /
  `VitalsModel` seams consume them.
- **No time-sync estimation.** The `Clock` seam owns drift estimation. The signal
  module only applies a caller-supplied linear time map and deterministic
  interpolation (see §4.8), and flags non-uniformity.
- **No network framing, CRC, or node addressing.** Those belong to transport.
  `decode_csi` validates a CSI *record*, not a packet.
- **Single reference pipeline.** One frozen configuration path; no adaptive
  filters, no auto-tuned parameters in v1.

Language: Australian English. `crate aquila-signal`; no other aquila crate may
be imported except `aquila-types`'s plain identifier types if needed (enforced by
the workspace dependency DAG).

## 2. Public surface

Small interface, no traits. All items are pure.

```rust
// aquila-signal

/// One ESP32-S3 node. Identity only; no behaviour.
pub struct NodeId(pub u32);

/// Channel width; determines the expected subcarrier count.
pub enum Bandwidth { Ht20, Ht40 }

/// Sample-accurate complex channel coefficient, normalised to CN0-ish units.
#[derive(Clone, Copy, PartialEq)]
pub struct Complex32 { pub re: f32, pub im: f32 }

/// Receive metadata carried alongside the CSI record.
pub struct RxMeta {
    pub node: NodeId,
    pub seq: u32,
    pub t_device_us: u64,          // raw device clock; NOT drift-corrected here
    pub centre_freq_hz: u32,
    pub bandwidth: Bandwidth,
    pub rssi_dbm: i8,
    pub noise_floor_dbm: i8,
    pub first_word_invalid: bool,  // ESP32 quirk: leading I/Q pair unreliable
}

/// The subcarrier grid: how many bins, which are usable, and their index order.
pub struct SubcarrierLayout {
    pub expected_bins: usize,      // 64 (HT20) | 128 (HT40)
    pub usable: Vec<u16>,          // ascending subcarrier indices, nulls removed
}

/// The pipeline's internal decoded frame. NOT the contract frame: the one
/// canonical on-air/on-disk frame is `aquila_types::CsiFrame` (raw signed int8
/// I/Q, `data-contracts.md` §2, ADR-0005). `decode_csi` is the single
/// conversion, deriving the complex bins from that sample.
pub struct ComplexFrame {
    pub meta: RxMeta,
    pub bins: Vec<Complex32>,      // len == layout.expected_bins
    pub flags: FrameFlags,
}

/// Per-node (and, later, per-pair) hardware normalisation, precomputed.
/// Runtime hot path is a complex multiply: no `sin`/`cos` at runtime.
pub struct Calibration {
    pub per_node: Vec<Complex32>,  // length == expected_bins; identity if empty
    pub state: CalibrationState,   // Calibrated | Uncalibrated | Partial
}

/// The single frozen reference configuration.
pub struct Config {
    pub layout: SubcarrierLayout,
    pub band_breathing: Band,      // default 0.10..0.60 Hz
    pub band_heart: Band,          // default 0.80..2.50 Hz
    pub band_motion: Band,         // default 0.10..5.00 Hz
    pub uniform_fs_hz: f32,        // resample grid for STFT
    pub stft_window: usize,
    pub stft_hop: usize,
    pub select_k: usize,
}

pub struct Band { pub lo_hz: f32, pub hi_hz: f32 }

// --- stage functions (each pure) -------------------------------------------

/// Contract frame -> internal conditioning frame (the one conversion).
pub fn decode_csi(frame: &aquila_types::CsiFrame)
    -> Result<ComplexFrame, DecodeError>;

/// Raw ESP32 CSI record -> internal conditioning frame (record-level path).
pub fn decode_record(raw: &[u8], meta: &RxMeta, layout: &SubcarrierLayout)
    -> Result<ComplexFrame, DecodeError>;

pub fn validate_sequence(frames: &[ComplexFrame]) -> Vec<SeqAnomaly>; // gaps/reorder/regression

pub fn normalise(frame: &ComplexFrame, cal: &Calibration) -> NormalisedFrame;

pub fn unwrap_phase(wrapped: &[f32]) -> Vec<f32>;

pub fn sanitise_phase(unwrapped: &[f32], layout: &SubcarrierLayout) -> SanitisedPhase;

pub fn bandpass(x: &[f32], coeffs: &BiquadCoeffs) -> Vec<f32>;
pub fn resonator(x: &[f32], at_hz: f32, fs_hz: f32) -> Complex32; // sliding Goertzel

pub fn motion_features(x: &[f32], window: usize) -> Vec<f32>;

pub fn stft(x: &[f32], cfg: &StftConfig) -> Spectrogram;
pub fn bvp_track(spec: &Spectrogram, band: Band) -> Vec<BvpSample>;

pub fn resample_uniform(x: &[f32], t_us: &[u64], fs_hz: f32)
    -> Result<Vec<f32>, ResampleError>;

pub fn select_subcarriers(features: &SubcarrierFeatureMatrix, k: usize)
    -> SubcarrierMask;

// Top-level reference composition (thin; calls the stages above in order).
pub fn process(chunk: &[ComplexFrame], cal: &Calibration, cfg: &Config)
    -> Result<Features, SignalError>;
```

`Features` is an owned, plain-data struct (per node, per frame index) with no
methods that do IO. `SignalError` is a closed enum. No function takes a closure
that could capture IO; no function spawns a task.

### 2.1 Packed feature vector (`FeatureVector`)

`NodeFeatures::feature_vector()` packs one node's conditioning output into the
canonical `aquila_types::FeatureVector` (`data-contracts.md` §1) consumed by
`aquila-inference`: every usable-subcarrier amplitude RMS value followed by
every usable-subcarrier motion energy, both in ascending usable-subcarrier
order. `Features::feature_vector(node)` is the per-node lookup. The layout is
owned here; the model seam never invents, resizes or pads it.

## 3. Determinism contract

Bit-exactness is a first-class requirement for the reference pipeline and is
pinned as follows.

1. **Reference target.** `x86_64-unknown-linux-gnu`, pinned Rust toolchain,
   pinned `-C target-cpu` (a fixed baseline, never `native`), fixed LTO/codegen
   flags. Bit-exact means: identical feature buffers byte-for-byte for identical
   input bytes on this build.
2. **Single-threaded core.** No parallelism in the reference path. Any future
   parallel reduction must be a fixed-shape reduction tree; until then it is
   forbidden.
3. **No floating-point contraction.** The reference relies on Rust/LLVM *not*
   contracting `a*b + c` into an FMA. `mul_add` is banned in the reference
   numeric path except where a stage explicitly documents it as part of the
   contract. No `-ffast-math`-style flags.
4. **Owned transcendentals.** `f32::sqrt` (IEEE-754 correctly rounded) is
   permitted. `sin`, `cos`, `atan2`, `exp`, `ln`, `powf` are *not* called at
   runtime in the reference path; where a stage needs them it uses a pinned
   `const` table (Hann window, twiddle factors) or the in-crate `atan2`
   approximation with documented coefficients and operation order. Calibration
   stores complex multipliers, so phase rotation is a multiply.
5. **Fixed accumulation order.** All sums iterate in ascending index order.
   Length-sensitive sums accumulate in `f64` and narrow to `f32` at one stated
   point per stage. Variance uses a stable two-pass form (§4.6).
6. **No FP-environment tricks.** No FTZ/DAZ changes, no rounding-mode changes,
   no `unsafe` reinterpretation of floats. Denormal handling follows the target
   default and is deterministic per target.
7. **Deterministic inputs.** Recorded fixtures are committed byte-exact; decode
   is a total function of those bytes. There is no random seeding anywhere in
   the reference path.

Cross-target (e.g. core vs. the Rust→WASM dashboard build) equality is *not*
claimed, except on the integer and pinned-table paths; the dashboard build is a
non-reference target and its numbers are advisory.

Every stage ships a golden-fixture test asserting a SHA-256 over its output
buffer, plus an in-process reproducibility test.

## 4. Stage specifications

Each stage lists: purpose; contract; invariants; failure modes; property-based
tests (proptest-style). "P-" identifiers are the consolidated invariants in §6.

### 4.1 Decode and validate (`decode_csi`, `decode_record`)

**Purpose.** Produce the pipeline's internal [`ComplexFrame`] from the canonical
contract frame, and reject structurally invalid input before any DSP touches it.

**Contract.** `decode_csi(frame: &aquila_types::CsiFrame) -> Result<ComplexFrame,
DecodeError>` derives the one contract frame's per-subcarrier `Complex32` bins
from its raw signed int8 I/Q sample (`i_q`, interleaved `I, Q`;
`data-contracts.md` §2, ADR-0005), dividing each count by the nominal
`IQ_COUNTS_PER_UNIT`. This is where the amplitude/phase derivation the firmware
used to do on-node now lives. `decode_record(raw, meta, layout) ->
Result<ComplexFrame, DecodeError>` decodes a raw ESP32 CSI record: `raw` is
interleaved `int8` I/Q pairs, imaginary byte first per Espressif
`wifi_csi_info_t`, and `bins.len() == layout.expected_bins`. Both are total
functions; they allocate but do not touch IO. The exact I/Q byte interleaving
and the HT20/HT40 null-bin map are **research-verify** items flagged in §7, not
assumed silently.

**Invariants.**
- `D1` Structural: `decode_record` accepts iff `raw.len()` is even,
  `raw.len()/2 == layout.expected_bins`, and `raw.len() <= 612`;
  `decode_csi` accepts iff `i_q.len()` is even and non-empty. Every accepted
  frame satisfies `bins.len() == expected_bins` (`decode_record`) or
  `i_q.len() / 2` (`decode_csi`).
- `D2` Determinism: identical input yields bit-identical `ComplexFrame`.
- `D3` Exactness: `int8 → f32` conversion is exact; no clamping or NaN.
- `D4` `first_word_invalid` policy: the first I/Q pair is zeroed and
  `flags.first_word_zeroed = true`; the frame is otherwise accepted.
- `D5` Metadata sanity is *recorded*, not enforced destructively: out-of-range
  `rssi`/`noise_floor` set a flag rather than failing the decode.

**Failure modes.** `OddLength`, `WrongBinCount { found, expected }`,
`TooLarge`, `UnsupportedBandwidth`, `EmptyRecord`, and (for `decode_csi`)
`EmptyFrame`, `OddIqLength`.

**Property tests.**
- `D1`: for arbitrary byte lengths, `decode_record` is `Ok` iff length ∈
  `{2·expected_bins}`; the error variant is exactly determined. For arbitrary
  contract frames, `decode_csi` is `Ok` iff the I/Q sample is even-length and
  non-empty.
- `D2/Golden`: `sha256(bins_as_le_bytes)` equals the committed fixture digest.
- `D4`: setting `first_word_invalid` yields `bins[0] == 0` and no other change.
- `D5`: arbitrary `rssi`/`noise_floor` never change `bins`, only `flags`.

### 4.2 Sequence validation (`validate_sequence`)

**Purpose.** Surface transport reality (UDP loss, reordering, node reboot) as
data, without altering frames.

**Contract.** Input: a per-node ordered slice. Output: `Vec<SeqAnomaly>` where
`SeqAnomaly ∈ { Gap { from, to }, Reorder { .. }, Regression { .. }, Duplicate }`.
Never returns an error and never drops frames.

**Invariants.**
- `S1` No mutation: output is a pure function of `seq` fields.
- `S2` A monotone, contiguous run produces an empty anomaly list.
- `S3` Gaps are not failures; they are expected and downstream stages must
  tolerate them.
- `S4` Determinism.

**Failure modes.** Empty input → empty list (not an error).

**Property tests.**
- `S2/P-S`: strictly increasing by +1 ⇒ `[]`.
- `S1`: re-running on the same slice gives an identical list.
- `S3`: removing a middle frame introduces exactly one `Gap` and changes nothing
  else.

### 4.3 Subcarrier layout projection (`SubcarrierLayout::usable`)

**Purpose.** Define once which physical bins carry usable channel response, so
every downstream stage shares one canonical order.

**Contract.** `usable` is an ascending `Vec<u16>` of in-range, non-null bin
indices for the configured bandwidth. Projection selects `bins[i] for i in
usable`.

**Invariants.**
- `L1` `usable` is strictly ascending and all `< expected_bins`.
- `L2` Projection preserves order; no reordering by magnitude.
- `L3` Layout is `const`/config, never computed from signal content.

**Failure modes.** `expected_bins == 0`, or `usable` out of range → rejected at
config construction (`ConfigError`).

**Property tests.** `L1` holds for every constructed layout; projection is
order-preserving.

### 4.4 Hardware normalisation (`normalise`)

**Purpose.** Remove per-node, per-subcarrier gain and phase response from the
global calibration, so nodes are comparable.

**Contract.** `H_norm[i] = H[i] * C[i]` where `C[i]` is a precomputed
`Complex32` multiplier from `Calibration`. Pure complex multiply; no runtime
trig. Time-invariant.

**Invariants.**
- `N1` Identity: empty `per_node` (or all `1+0i`) ⇒ `H_norm == H` bit-exactly.
- `N2` Multiplicativity: `|H_norm| == |H|·|C|` (up to one rounding).
- `N3` Invertibility within ε when the inverse multiplier is supplied.
- `N4` Determinism (complex multiply is a fixed operation sequence).
- `N5` `CalibrationState::Uncalibrated` is carried on the output frame so
  downstream metrics can gate on calibration status (per the ledger).

**Failure modes.** Length mismatch between `per_node` and `expected_bins` →
`SignalError::CalibrationShape`. Non-finite multiplier → that bin flagged,
treated as identity (never propagates NaN).

**Property tests.**
- `N1`: arbitrary complex input with identity calibration is unchanged
  byte-for-byte.
- `N2`: `abs(re²+im²)` scales monotonically with `|C|` for fixed `H`.
- `N4`: golden digest.

### 4.5 Phase unwrapping (`unwrap_phase`)

**Purpose.** Make per-subcarrier phase continuous so a linear trend can be
estimated, correcting the `2π` wraps imposed by `atan2`.

**Contract.** Input: physical phase values in `(-π, π]` over the usable
subcarrier sequence (ascending bin index). Output: an unwrapped series such that
adjacent differences lie in `(-π, π]`.

```
u[0] = φ[0]
u[k] = φ[k] + 2π · m[k],  m[k] chosen so u[k] − u[k−1] ∈ (−π, π]
```

**Invariants.**
- `U1` Congruence: `u[k] ≡ φ[k] (mod 2π)` for all `k`.
- `U2` Idempotence: if all adjacent wrapped differences already lie in
  `(−π, π]`, `unwrap_phase(u) == u`.
- `U3` First element preserved: `u[0] == φ[0]`.
- `U4` Determinism: only comparisons and integer multiples of `2π`.
- `U5` Boundary honesty: a corrected jump whose *pre-correction* magnitude
  exceeds `π` (possible across a null-bin gap) sets an `UnwrapBoundary` flag;
  the wrap correction is not silently trusted.

**Failure modes.** Empty input → empty output. Non-finite input → rejected by
the caller before this stage (`SignalError::NonFinitePhase`).

**Property tests.**
- `U1`: `∀ φ, wrap(unwrap(φ)) == wrap(φ)`.
- `U2`: for generated series with max adjacent jump `< π`, `unwrap` is identity.
- `U3`: `unwrap(φ)[0] == φ[0]` for all `φ`.
- `U4`: golden digest; repeated runs identical.

### 4.6 Phase sanitisation (`sanitise_phase`)

**Purpose.** Remove the sampling-time-offset / packet-detection-delay linear
phase slope and the constant offset across subcarriers (the classic CSI phase
sanitisation), leaving the residual phase that carries motion and vital signs.

**Contract.** Input: unwrapped phase `u` over ascending usable bins `k_i`.
Output: `SanitisedPhase { residual: Vec<f32>, slope: f32, intercept: f32,
fit_ok: bool }`. Least-squares line via a fixed, documented accumulation order:

```
κ̄ = Σk_i / n,  φ̄ = Σu_i / n
a = Σ(k_i − κ̄)(u_i − φ̄) / Σ(k_i − κ̄)²
b = φ̄ − a·κ̄
residual[i] = u_i − (a·k_i + b)
```

`n` is the usable-bin count. Accumulate in `f64`, narrow once.

**Invariants.**
- `P1` Affine equivariance: if `u_i = α·k_i + β` exactly, then `residual ≡ 0`
  and `slope == α` (both within a stated ε).
- `P2` Residual mean is zero (‖Σ residual‖ ≤ ε) and residual is orthogonal to
  the affine subspace spanned by `{1, k}` (the defining property).
- `P3` `n ≥ 2` required to fit; `n < 2` sets `fit_ok = false` and returns `u`
  unchanged (never a division by zero).
- `P4` Determinism; fixed order, `f64` accumulation.
- `P5` Shift covariance: adding a constant to all `u_i` does not change
  `residual`.
- `P6` Guarded: non-finite input yields `fit_ok = false`, not NaN.

**Failure modes.** `n < 2`; collinear/zero-spread bin indices (only if a
degenerate layout is supplied) → `fit_ok = false`.

**Property tests.**
- `P1`: generated affine phases sanitise to ~0 residual, recovering `α`.
- `P2`: `|Σ residual| ≤ ε` and `Σ residual·k_i ≤ ε` for arbitrary input.
- `P5`: `sanitise(u + c) == sanitise(u)` within ε.
- `P4`: golden digest.

*Alternative for future work:* a phase-difference slope estimator that is
wrap-robust without unwrapping. Not in the reference pipeline (determinism and
one frozen path take priority).

### 4.7 Bandpass / resonator filtering (`bandpass`, `resonator`)

**Purpose.** Isolate physiological and motion bands and reject DC, drift, and
out-of-band noise.

**Contract.** `bandpass` is a cascade of second-order sections (RBJ-style
bandpass) applied to a `f32` series with zero initial state; coefficients are
precomputed (offline) and supplied as `BiquadCoeffs`. `resonator` is a sliding
Goertzel returning the complex band power at one frequency. Both are pure
batch transforms; the per-stream state variant mutates only an explicit
caller-owned `FilterState`.

**Invariants.**
- `F1` Stability: every section has poles strictly inside the unit circle;
  checked once at `Config` construction (`ConfigError::UnstableFilter`).
- `F2` Linearity: `bandpass(αx + βy) == α·bandpass(x) + β·bandpass(y)` within ε.
- `F3` DC rejection: `bandpass(1,1,…) ` settles to ~0 and `resonator` at
  `at_hz > 0` on a constant is ~0.
- `F4` Zero-in/zero-out: all-zero input with zero state yields all-zero output.
- `F5` Time-invariance and determinism; fixed direct-form operation order.
- `F6` Monotone response: for a pure tone at the band centre, output amplitude
  is monotone non-decreasing in input amplitude.

**Failure modes.** Unstable or non-finite coefficients → rejected at config
time. Warm-up transient: output before `group_delay` samples is flagged via a
`settled_after` count, not silently used.

**Property tests.**
- `F2` linearity on generated signals.
- `F4` zero preservation.
- `F1` pole-radius check over random stable bands.
- `F6` amplitude monotonicity for centre-frequency tones.
- `F5` golden digest.

### 4.8 Uniform resampling precondition (`resample_uniform`)

**Purpose.** CSI arrivals are event-driven and jittered; the STFT needs a
uniform grid. This stage maps device timestamps to a uniform grid.

**Contract.** Input: series `x`, device timestamps `t_us`, target `fs_hz`.
Output: uniform series resampled by deterministic linear interpolation. The
device→common time base map is supplied by the `Clock` seam as a caller-owned
linear correction; this function does *not* estimate drift.

**Invariants.**
- `R1` Determinism: fixed grid origin (the first timestamp), fixed interpolation
  arithmetic.
- `R2` Identity on an already-uniform grid.
- `R3` Non-uniformity is reported: if timestamp jitter or gaps exceed a
  configured ratio, return `ResampleError::TooIrregular` rather than inventing
  samples for a long gap.
- `R4` Monotone time map: timestamps must be non-decreasing; violation is an
  error (the caller may sort first via `validate_sequence`).

**Failure modes.** `< 2` samples, non-monotone times, gaps beyond budget.

**Property tests.** `R2` identity; `R1` golden digest; `R3` a long gap yields
`TooIrregular`.

### 4.9 Motion / residual features (`motion_features`)

**Purpose.** Extract clutter-removed motion energy and periodicity proxies that
feed presence and coarse activity.

**Contract.** Input: a series `x` (amplitude or sanitised phase) and window
`W`. Clutter estimate per position is the window mean; residual is the
mean-removed window. Output includes per-window energy and a spectral-flux term.
Two-pass, stable variance:

```
μ = (Σ_{j∈W} x_j) / W
E = Σ_{j∈W} (x_j − μ)²
```

**Invariants.**
- `M1` Translation invariance: `x → x + c` leaves the residual and `E`
  unchanged (bit-exact for constant `c` only in exact arithmetic; ε otherwise).
- `M2` Homogeneity: scaling the residual by `c > 0` scales `E` by `c²`.
- `M3` Non-negativity: `E ≥ 0`, with `E == 0` iff the window is constant.
- `M4` Determinism; two-pass variance; fixed order.
- `M5` Window-boundary honesty: partial windows at the edges are flagged, not
  padded with a value that implies data.

**Failure modes.** `W == 0` → config error; `W` greater than series length →
all windows flagged partial.

**Property tests.**
- `M1`, `M2`, `M3` on generated signals.
- `M4` golden digest.

### 4.10 Spectrogram and BVP (`stft`, `bvp_track`)

**Purpose.** Produce a time–frequency view over physiological bands and track
the dominant breathing/heart-rate frequency over time (the BVP-style series).

**Contract.** `stft` computes, for frame `m` and bin `k`,
`X[m,k] = Σ_{n<W} w[n]·x[mH+n]·e^{−i2πkn/W}`, using a `const` Hann window and
`const` twiddle/tables; bins map to `f_k = k·fs/W`. `bvp_track` band-limits
`|X|` to a `Band`, picks the dominant bin per frame by deterministic argmax
(lowest bin on ties), optionally refines by a pinned parabolic formula, and
returns `BvpSample { t, freq_hz, snr_db }` (brpm/bpm conversion is left to the
model seam).

**Invariants.**
- `B1` Linearity of the STFT; Parseval within a fixed window-energy constant.
- `B2` Frequency mapping: bin `k` ↔ `k·fs/W`; DC and negative-frequency layout
  are fixed and documented.
- `B3` Determinism: pinned window/twiddle tables; fixed summing order.
- `B4` Shift equivariance: an integer-hop shift of a periodic input shifts the
  spectrogram by exactly that many frames; sub-hop shifts are quantised.
- `B5` Monotone response: increasing the amplitude of a tone at bin `k₀`
  monotonically increases `|X[·,k₀]|`; the argmax honours this once it is the
  largest bin.
- `B6` Short-signal policy: window longer than the signal zero-pads
  deterministically (never errors inside a window); the padded region is
  flagged.

**Failure modes.** `W == 0` or `H == 0` → config error; `band` outside
`[0, fs/2]` → `SignalError::BandOutOfRange`; non-finite input → error before the
STFT.

**Property tests.**
- `B1` Parseval on random signals.
- `B4` integer-hop equivariance.
- `B5` tone-amplitude monotonicity at the injected bin.
- `B2` a synthetic tone at a known frequency lands in the predicted bin.
- `B3` golden digest.

### 4.11 Subcarrier selection (`select_subcarriers`)

**Purpose.** Choose the subcarriers that actually carry motion/vitals signal,
rather than the ones dominated by noise or hardware ripple.

**Contract.** Input: a per-subcarrier feature matrix (band power vs. noise
power). Output: a `SubcarrierMask` of the top-`k` bins. Selection runs during
calibration and is then **frozen** into `Config`; the runtime pipeline consumes
the frozen mask, so selection does not read live signal in the hot path.

**Invariants.**
- `K1` Subset: the mask is a subset of `layout.usable`.
- `K2` Order: selected indices are ascending.
- `K3` Cardinality: `len ≤ k`; if fewer than `k` candidates exist, all are
  returned.
- `K4` Determinism and documented tie-break: sort by score descending, index
  ascending; equal scores resolve to the lower index.
- `K5` Rank consistency (monotonicity): if a bin's score rises strictly above
  another's, their relative selection order swaps accordingly.
- `K6` Permutation invariance: the result depends only on `(index, score)`
  pairs, not on the input's storage order.

**Failure modes.** All-zero scores → deterministic lowest-index selection.
Non-finite score → that bin is excluded and flagged, never treated as `+∞`.

**Property tests.**
- `K1`–`K3` over random score matrices.
- `K4` ties resolve to lowest index; repeated runs identical.
- `K6` shuffling input rows does not change the mask.

## 5. Reference pipeline composition

`process(chunk, cal, cfg)` runs, in order:

1. The chunk arrives already decoded: each contract `CsiFrame` became a
   `ComplexFrame` at the transport seam via `decode_csi`; `process` never sees
   contract frames.
2. `validate_sequence` per node → anomalies attached to `Features`.
3. layout projection (ascending usable bins).
4. `normalise` with the frozen `Calibration`.
5. amplitude path and phase path:
   - amplitude magnitude series per bin;
   - `unwrap_phase` → `sanitise_phase` per frame.
6. `resample_uniform` per bin onto `uniform_fs_hz`.
7. `bandpass` into `band_breathing`, `band_heart`, `band_motion`.
8. `motion_features` over `band_motion`.
9. `stft` → `bvp_track` per physiological band, restricted to the frozen
   subcarrier mask.
10. assemble `Features`.

Selection (`select_subcarriers`) is a **calibration-time** step that produces the
mask consumed at step 9; it is not re-run per frame. This keeps the runtime path
free of data-dependent branching that could vary between replays.

The frozen configuration and the calibration coefficients are inputs, not
globals. Replaying a recorded fixture with the same `Config` and `Calibration`
must reproduce `Features` bit-for-bit.

## 6. Consolidated invariants

Decode (D): `D1` structural accept rule; `D2` deterministic; `D3` exact `int8→f32`;
`D4` `first_word_invalid` zeroes only the first pair and flags it; `D5` metadata
anomalies are flagged, not destructive.

Sequence (S): `S1` pure; `S2` contiguous run ⇒ no anomalies; `S3` gaps tolerated;
`S4` deterministic.

Layout (L): `L1` usable is strictly ascending and in range; `L2` projection
order-preserving; `L3` layout is config, never signal-derived.

Normalisation (N): `N1` identity multiplier ⇒ bit-exact passthrough; `N2`
`|H_norm| = |H|·|C|`; `N3` invertible within ε; `N4` deterministic; `N5`
calibration state carried.

Unwrap (U): `U1` `u ≡ φ (mod 2π)`; `U2` idempotent below `π` jumps; `U3` first
element preserved; `U4` deterministic; `U5` flagged ambiguous boundaries.

Sanitise (P): `P1` affine inputs ⇒ zero residual and recovered slope; `P2`
residual orthogonal to `{1, k}`; `P3` `n < 2` ⇒ `fit_ok = false`, input returned;
`P4` deterministic `f64` accumulation; `P5` shift covariance; `P6` non-finite
guard.

Filter (F): `F1` stable poles; `F2` linear; `F3` DC rejected; `F4` zero-in/
zero-out; `F5` time-invariant and deterministic; `F6` monotone tone response;
warm-up flagged, not hidden.

Resample (R): `R1` deterministic grid; `R2` identity on uniform input; `R3`
irregularity reported, gaps not invented; `R4` non-decreasing time required.

Motion (M): `M1` translation invariance; `M2` `c²` homogeneity; `M3` `E ≥ 0` and
zero iff constant; `M4` deterministic two-pass variance; `M5` partial windows
flagged, not padded.

Spectrogram/BVP (B): `B1` linear and Parseval; `B2` fixed bin↔frequency map;
`B3` deterministic pinned tables; `B4` integer-hop shift equivariance; `B5`
monotone tone response; `B6` deterministic zero-pad with flag.

Selection (K): `K1` subset of usable; `K2` ascending; `K3` `len ≤ k`, return all
when short; `K4` deterministic with lower-index tie-break; `K5` rank consistency;
`K6` permutation invariance.

Global: `G1` no IO, no globals, no threads in the reference path; `G2` no
cross-node phase in v1; `G3` bit-exact on the pinned reference target; `G4` no
`atan2`/`sin`/`cos`/`exp`/`ln` at runtime outside pinned tables or the in-crate
`atan2`; `G5` same `Config`+`Calibration`+fixture ⇒ identical `Features`.

## 7. Open questions (honest unknowns)

- Exact ESP32-S3 CSI byte order (imaginary-first vs. real-first) and the HT20/
  HT40 null-bin map must be confirmed against a real capture before `decode_csi`
  is frozen; until then they are configuration, not assumptions.
- Whether `f32` is sufficient for the accumulation-heavy stages, or whether the
  residual/spine should stay in `f64` throughout, is to be settled by the
  measurement ledger (record + diff), not by taste.
- The phase-difference (wrap-robust) slope estimator is a candidate to replace
  unwrap-then-detrend; it must prove deterministic and beat the current path on
  recorded fixtures before it enters the reference pipeline.
- Cross-node amplitude-difference features are permitted in v1; any phase-based
  cross-node feature is blocked on the phase-coherent-RF-reference decision.
