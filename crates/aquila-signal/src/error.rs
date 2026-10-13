//! The typed error surface for `aquila-signal`.
//!
//! Every stage returns a typed error rather than panicking on malformed input.
//! [`DecodeError`] covers [`decode_csi`](crate::decode_csi),
//! [`ResampleError`] covers [`resample_uniform`](crate::resample_uniform),
//! [`ConfigError`] covers configuration validation, and [`SignalError`] is the
//! closed top-level enum returned by [`process`](crate::process).
//!
//! Errors are plain data: they carry the offending quantity so a caller can log
//! or count it without re-deriving it. Australian English throughout.

use thiserror::Error;

/// Everything `decode_csi` can reject.
///
/// The decoder is a total function of `(raw, meta, layout)`: it accepts a
/// structurally valid CSI record and otherwise names the structural fault.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DecodeError {
    /// The record was empty. A CSI record must carry at least one `I/Q` pair.
    #[error("CSI record is empty")]
    EmptyRecord,
    /// The raw length in bytes is odd, so it cannot be whole `I/Q` pairs.
    #[error("CSI record length {0} is odd")]
    OddLength(usize),
    /// The raw length exceeds the 612-byte on-air maximum.
    #[error("CSI record length {0} exceeds the 612-byte maximum")]
    TooLarge(usize),
    /// The number of decoded bins does not match the layout.
    #[error("CSI record carries {found} bins, expected {expected}")]
    WrongBinCount {
        /// The bin count implied by the raw length.
        found: usize,
        /// The bin count the layout requires.
        expected: usize,
    },
    /// The layout's expected bin count is not one this decoder supports.
    #[error("unsupported bandwidth for a layout of {0} bins")]
    UnsupportedBandwidth(usize),
}

/// Everything `resample_uniform` can reject.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ResampleError {
    /// The series and its timestamps have different lengths.
    #[error("series length {series} does not match timestamp length {times}")]
    LengthMismatch {
        /// The number of samples in the series.
        series: usize,
        /// The number of timestamps.
        times: usize,
    },
    /// Fewer than two samples cannot define a grid.
    #[error("need at least two samples to resample, found {0}")]
    TooFewSamples(usize),
    /// The target sampling rate must be positive and finite.
    #[error("target sampling rate must be positive and finite")]
    BadRate,
    /// Timestamps must be non-decreasing.
    #[error("timestamps are not non-decreasing at index {0}")]
    NonMonotone(usize),
    /// A gap between samples exceeds the irregularity budget.
    #[error("timestamp gap {gap_us} us at index {index} exceeds the irregularity budget")]
    TooIrregular {
        /// The index of the sample after the offending gap.
        index: usize,
        /// The offending gap, in microseconds.
        gap_us: u64,
    },
    /// A timestamp span too short to place even two grid slots.
    #[error("timestamp span is too short for the target rate")]
    SpanTooShort,
}

/// Errors raised while validating a [`Config`](crate::Config) or layout.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ConfigError {
    /// The layout declares zero expected bins.
    #[error("layout has zero expected bins")]
    EmptyLayout,
    /// A usable bin index lies outside the expected bin range.
    #[error("usable bin {index} is out of range for {expected_bins} bins")]
    UsableOutOfRange {
        /// The offending bin index.
        index: u16,
        /// The declared bin count.
        expected_bins: usize,
    },
    /// The usable bins are not strictly ascending.
    #[error("usable bins are not strictly ascending at position {0}")]
    UsableNotAscending(usize),
    /// A frequency band is degenerate (`lo >= hi`) or out of the Nyquist range.
    #[error("band {lo}..{hi} Hz is invalid at {fs_hz} Hz")]
    BadBand {
        /// The lower band edge, in hertz.
        lo: f32,
        /// The upper band edge, in hertz.
        hi: f32,
        /// The sampling rate the band is checked against.
        fs_hz: f32,
    },
    /// The uniform sampling rate is not positive and finite.
    #[error("uniform sampling rate must be positive and finite")]
    BadRate,
    /// The STFT window and hop must both be non-zero.
    #[error("stft window and hop must both be non-zero")]
    BadStftShape,
    /// A vitals-estimation parameter is invalid: a zero sample floor, a
    /// non-positive confidence scale, or a non-finite SNR floor.
    #[error("vitals-estimation parameters are invalid")]
    BadVitalsParams,
    /// A section's poles are not strictly inside the unit circle.
    #[error("filter poles are not strictly inside the unit circle")]
    UnstableFilter,
    /// A filter coefficient is not finite.
    #[error("filter coefficients are not finite")]
    NonFiniteFilter,
}

/// The closed top-level error returned by [`process`](crate::process).
#[derive(Debug, Clone, PartialEq, Error)]
pub enum SignalError {
    /// A CSI record failed to decode.
    #[error(transparent)]
    Decode(#[from] DecodeError),
    /// Resampling failed.
    #[error(transparent)]
    Resample(#[from] ResampleError),
    /// Configuration is invalid.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// The calibration array does not match the layout's expected bin count.
    #[error("calibration shape {actual} does not match expected bins {expected}")]
    CalibrationShape {
        /// The length of the supplied calibration array.
        actual: usize,
        /// The bin count the layout requires.
        expected: usize,
    },
    /// A phase sample is not finite.
    #[error("phase input is not finite at index {0}")]
    NonFinitePhase(usize),
    /// A sample is not finite.
    #[error("sample input is not finite at index {0}")]
    NonFiniteSample(usize),
    /// A requested band lies outside `[0, fs/2]`.
    #[error("band {lo}..{hi} Hz is outside [0, {nyquist}] Hz")]
    BandOutOfRange {
        /// The lower band edge, in hertz.
        lo: f32,
        /// The upper band edge, in hertz.
        hi: f32,
        /// The Nyquist frequency, in hertz.
        nyquist: f32,
    },
    /// The input chunk was empty.
    #[error("input chunk is empty")]
    EmptyChunk,
}
