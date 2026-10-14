//! CSI decoding and the core conditioning types.
//!
//! This module owns the signal pipeline's **internal** conditioning
//! representation, [`ComplexFrame`] — the per-subcarrier [`Complex32`] bins the
//! DSP consumes. It is deliberately not a contract type: the one canonical
//! `CsiFrame` lives in `aquila-types` (`data-contracts.md` §2). [`decode_csi`]
//! converts that contract frame into a [`ComplexFrame`], and [`decode_record`]
//! decodes a raw ESP32 CSI record for the record-level path. Both are total
//! functions of their input: they allocate but perform no IO and hold no
//! globals (`signal.md` §4.1).

use aquila_types::{Band, CsiFrame as ContractFrame, NodeId};

use crate::error::DecodeError;
use crate::math::{cos_approx, sin_approx};

/// Channel width; determines the expected subcarrier count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bandwidth {
    /// 20 MHz: 64 subcarriers.
    Ht20,
    /// 40 MHz: 128 subcarriers.
    Ht40,
}

impl Bandwidth {
    /// The expected subcarrier count for this bandwidth.
    #[must_use]
    pub fn expected_bins(self) -> usize {
        match self {
            Bandwidth::Ht20 => 64,
            Bandwidth::Ht40 => 128,
        }
    }
}

/// Sample-accurate complex channel coefficient, normalised to CN0-ish units.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Complex32 {
    /// The real (in-phase) component.
    pub re: f32,
    /// The imaginary (quadrature) component.
    pub im: f32,
}

impl Complex32 {
    /// The additive identity.
    pub const ZERO: Complex32 = Complex32 { re: 0.0, im: 0.0 };
    /// The multiplicative identity.
    pub const ONE: Complex32 = Complex32 { re: 1.0, im: 0.0 };

    /// Construct a coefficient.
    #[must_use]
    pub const fn new(re: f32, im: f32) -> Self {
        Complex32 { re, im }
    }

    /// The squared magnitude, `re² + im²`.
    #[must_use]
    pub fn magnitude_sq(self) -> f32 {
        self.re * self.re + self.im * self.im
    }

    /// The magnitude, `√(re² + im²)`.
    #[must_use]
    pub fn magnitude(self) -> f32 {
        self.magnitude_sq().sqrt()
    }

    /// The complex conjugate.
    #[must_use]
    pub fn conj(self) -> Self {
        Complex32::new(self.re, -self.im)
    }

    /// Whether both components are finite.
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.re.is_finite() && self.im.is_finite()
    }
}

impl core::ops::Mul for Complex32 {
    type Output = Complex32;

    /// The complex product, evaluated in a fixed order:
    /// `(a + bi)(c + di) = (ac − bd) + (ad + bc)i`.
    fn mul(self, other: Self) -> Self {
        Complex32::new(
            self.re * other.re - self.im * other.im,
            self.re * other.im + self.im * other.re,
        )
    }
}

/// Receive metadata carried alongside the CSI record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RxMeta {
    /// The sending node.
    pub node: NodeId,
    /// The frame sequence number, monotonic within a node stream modulo loss.
    pub seq: u32,
    /// Raw device clock in microseconds; **not** drift-corrected here.
    pub t_device_us: u64,
    /// The centre frequency, in hertz.
    pub centre_freq_hz: u32,
    /// The channel width.
    pub bandwidth: Bandwidth,
    /// Whole-frame received signal strength, in dBm.
    pub rssi_dbm: i8,
    /// Whole-frame noise floor, in dBm.
    pub noise_floor_dbm: i8,
    /// ESP32 quirk: the leading `I/Q` pair is unreliable.
    pub first_word_invalid: bool,
}

/// Per-frame flags recorded by decode; anomalies are *recorded*, not enforced
/// destructively (`signal.md` D4/D5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameFlags {
    /// The first `I/Q` pair was zeroed because `first_word_invalid` was set.
    pub first_word_zeroed: bool,
    /// The reported RSSI lay outside the plausible range.
    pub rssi_anomaly: bool,
    /// The reported noise floor lay outside the plausible range.
    pub noise_floor_anomaly: bool,
}

/// The subcarrier grid: how many bins, which are usable, and their index order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubcarrierLayout {
    /// The expected bin count: 64 (HT20) or 128 (HT40).
    pub expected_bins: usize,
    /// Ascending subcarrier indices with nulls removed.
    pub usable: Vec<u16>,
}

impl SubcarrierLayout {
    /// The provisional HT20 layout: 64 bins, index 0 (DC) removed.
    ///
    /// The exact null-bin map is a research-verify item (`signal.md` §7) and is
    /// configuration, not assumption; this constructor is the frozen default
    /// until a real capture pins it.
    #[must_use]
    pub fn ht20() -> Self {
        SubcarrierLayout {
            expected_bins: 64,
            usable: (1..64).collect(),
        }
    }

    /// The provisional HT40 layout: 128 bins, indices 0 and 64 removed.
    #[must_use]
    pub fn ht40() -> Self {
        SubcarrierLayout {
            expected_bins: 128,
            usable: (1..128).filter(|i| *i != 64).collect(),
        }
    }

    /// Validate `L1`: `expected_bins` is non-zero and `usable` is strictly
    /// ascending and in range.
    pub fn validate(&self) -> Result<(), crate::error::ConfigError> {
        use crate::error::ConfigError;
        if self.expected_bins == 0 {
            return Err(ConfigError::EmptyLayout);
        }
        let mut previous: Option<u16> = None;
        for (position, &index) in self.usable.iter().enumerate() {
            if usize::from(index) >= self.expected_bins {
                return Err(ConfigError::UsableOutOfRange {
                    index,
                    expected_bins: self.expected_bins,
                });
            }
            if let Some(prev) = previous {
                if index <= prev {
                    return Err(ConfigError::UsableNotAscending(position));
                }
            }
            previous = Some(index);
        }
        Ok(())
    }

    /// Project full bins onto the usable subset, preserving ascending order
    /// (`L2`). Indices beyond the supplied slice are skipped.
    #[must_use]
    pub fn project(&self, bins: &[Complex32]) -> Vec<Complex32> {
        self.usable
            .iter()
            .filter_map(|&i| bins.get(usize::from(i)).copied())
            .collect()
    }
}

/// The signal pipeline's internal, decoded frame.
///
/// This is the conditioning representation — per-subcarrier [`Complex32`] bins
/// with receive metadata and decode flags — **not** the on-air/on-disk contract
/// frame. The canonical contract frame is `aquila_types::CsiFrame`
/// (`data-contracts.md` §2); [`decode_csi`] is the one conversion between them.
#[derive(Debug, Clone, PartialEq)]
pub struct ComplexFrame {
    /// The receive metadata.
    pub meta: RxMeta,
    /// The complex subcarrier coefficients; `len == layout.expected_bins`.
    pub bins: Vec<Complex32>,
    /// The decode flags.
    pub flags: FrameFlags,
}

/// The nominal centre frequency of a channel, in hertz, for diagnostics only.
///
/// The contract frame carries the band and channel, not the frequency; this
/// derives the same nominal label the transport seam always used. It is never
/// read by the DSP (`signal.md` §2).
fn centre_freq_hz(band: Band, channel: u8) -> u32 {
    match band {
        Band::B2G4 => 2_407 + u32::from(channel) * 5,
        Band::B5G => 5_000 + u32::from(channel) * 5,
        Band::Unknown => 0,
    }
}

/// Decode the canonical contract frame into the conditioning representation.
///
/// This is the one conversion from the on-air/on-disk `aquila_types::CsiFrame`
/// (ordered `amplitude`/`phase` arrays, `data-contracts.md` §2) to a
/// [`ComplexFrame`] (per-subcarrier [`Complex32`] bins). The reconstruction
/// `re = a·cos(φ)`, `im = a·sin(φ)` inverts the magnitude and argument the wire
/// carries, using the crate's deterministic [`cos_approx`]/[`sin_approx`] so the
/// conversion is bit-identical across hosts.
///
/// # Errors
///
/// Returns [`DecodeError`] when the contract frame carries no subcarriers,
/// mismatched `amplitude`/`phase` lengths, or a non-finite sample. A valid
/// contract frame never fails.
pub fn decode_csi(frame: &ContractFrame) -> Result<ComplexFrame, DecodeError> {
    let n = frame.amplitude.len();
    if n == 0 {
        return Err(DecodeError::EmptyFrame);
    }
    if frame.phase.len() != n {
        return Err(DecodeError::LengthMismatch {
            amplitude: n,
            phase: frame.phase.len(),
        });
    }
    let mut bins = Vec::with_capacity(n);
    for i in 0..n {
        let amplitude = frame.amplitude[i];
        let phase = frame.phase[i];
        if !amplitude.is_finite() {
            return Err(DecodeError::NonFiniteSample {
                field: "amplitude",
                index: i,
            });
        }
        if !phase.is_finite() {
            return Err(DecodeError::NonFiniteSample {
                field: "phase",
                index: i,
            });
        }
        bins.push(Complex32::new(
            amplitude * cos_approx(phase),
            amplitude * sin_approx(phase),
        ));
    }

    let bandwidth = if frame.bandwidth_mhz >= 40 {
        Bandwidth::Ht40
    } else {
        Bandwidth::Ht20
    };
    let meta = RxMeta {
        node: frame.node_id,
        seq: frame.sequence.min(u32::MAX as u64) as u32,
        t_device_us: frame.device_time_us,
        centre_freq_hz: centre_freq_hz(frame.band, frame.channel),
        bandwidth,
        rssi_dbm: frame.rssi_dbm,
        noise_floor_dbm: frame.noise_floor_dbm,
        first_word_invalid: false,
    };
    let flags = FrameFlags {
        first_word_zeroed: false,
        rssi_anomaly: !(-100..=0).contains(&frame.rssi_dbm),
        noise_floor_anomaly: !(-120..=0).contains(&frame.noise_floor_dbm),
    };

    Ok(ComplexFrame { meta, bins, flags })
}

/// Decode a raw ESP32 CSI record into a typed [`ComplexFrame`].
///
/// The input `raw` is interleaved `int8 I/Q` pairs, imaginary byte first, per
/// the Espressif `wifi_csi_info_t` convention: each pair `[im, re]` becomes one
/// [`Complex32`]. Conversion `int8 → f32` is exact; no clamping or `NaN`
/// (`D3`). The output `bins.len()` equals `layout.expected_bins`. This is the
/// record-level path; [`decode_csi`] is the contract-frame path.
///
/// # Errors
///
/// Returns [`DecodeError`] for an empty, odd-length, oversized or
/// wrong-bin-count record. Metadata anomalies never fail the decode; they set
/// [`FrameFlags`] instead (`D5`).
pub fn decode_record(
    raw: &[u8],
    meta: &RxMeta,
    layout: &SubcarrierLayout,
) -> Result<ComplexFrame, DecodeError> {
    if raw.is_empty() {
        return Err(DecodeError::EmptyRecord);
    }
    if raw.len() > 612 {
        return Err(DecodeError::TooLarge(raw.len()));
    }
    if !raw.len().is_multiple_of(2) {
        return Err(DecodeError::OddLength(raw.len()));
    }
    let found = raw.len() / 2;
    if found != layout.expected_bins {
        return Err(DecodeError::WrongBinCount {
            found,
            expected: layout.expected_bins,
        });
    }
    if layout.expected_bins != Bandwidth::Ht20.expected_bins()
        && layout.expected_bins != Bandwidth::Ht40.expected_bins()
    {
        return Err(DecodeError::UnsupportedBandwidth(layout.expected_bins));
    }

    let mut bins = Vec::with_capacity(found);
    let mut offset = 0;
    while offset + 1 < raw.len() {
        let im = f32::from(raw[offset] as i8);
        let re = f32::from(raw[offset + 1] as i8);
        bins.push(Complex32::new(re, im));
        offset += 2;
    }

    let mut flags = FrameFlags {
        first_word_zeroed: false,
        rssi_anomaly: !(-100..=0).contains(&meta.rssi_dbm),
        noise_floor_anomaly: !(-120..=0).contains(&meta.noise_floor_dbm),
    };
    if meta.first_word_invalid {
        if let Some(first) = bins.first_mut() {
            *first = Complex32::ZERO;
        }
        flags.first_word_zeroed = true;
    }

    Ok(ComplexFrame {
        meta: *meta,
        bins,
        flags,
    })
}
