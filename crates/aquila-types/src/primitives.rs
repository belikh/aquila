//! Shared primitive vocabulary: bands, PHY flags, the evidence grade and the
//! bounded confidence/quality scalars.

use serde::{Deserialize, Serialize};

use crate::error::{require_finite_f32, require_unit_interval, ValidationError};

/// The radio band of a CSI frame: 2.4 GHz or 5 GHz.
///
/// `Unknown` is a first-class value, not an error. Any unrecognised variant
/// deserialises to `Unknown`; a reader never reinterprets it as a known band.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Band {
    /// The 2.4 GHz band.
    B2G4 = 0,
    /// The 5 GHz band.
    B5G = 1,
    /// An unrecognised or unset band.
    #[serde(other)]
    Unknown = 255,
}

/// A striped OFDM secondary channel, as reported by the ESP receiver.
///
/// `None` means the 20 MHz primary channel only. `Unknown` is a first-class
/// fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecondaryChannel {
    /// No secondary channel; 20 MHz primary only.
    None = 0,
    /// The secondary channel is above the primary.
    Above,
    /// The secondary channel is below the primary.
    Below,
    /// An unrecognised or unset secondary channel.
    #[serde(other)]
    Unknown = 255,
}

/// A bitflag-like newtype over the PHY status word from the receiver.
///
/// Bit 0 is HT (11n), bit 1 secondary present, bit 2 STBC, bit 3 LDPC, bit 4
/// short GI, bit 5 sounding/NDP. Higher bits are reserved and preserved
/// verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PhyFlags(pub u16);

impl PhyFlags {
    /// HT (802.11n) was used.
    pub const HT: u16 = 1 << 0;
    /// A secondary channel was present.
    pub const SECONDARY_PRESENT: u16 = 1 << 1;
    /// Space-time block coding was used.
    pub const STBC: u16 = 1 << 2;
    /// Low-density parity-check coding was used.
    pub const LDPC: u16 = 1 << 3;
    /// A short guard interval was used.
    pub const SHORT_GI: u16 = 1 << 4;
    /// The frame was a sounding/NDP frame.
    pub const SOUNDING_NDP: u16 = 1 << 5;

    /// No flags set.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// The raw flag word.
    #[must_use]
    pub const fn bits(&self) -> u16 {
        self.0
    }

    /// Whether every bit in `mask` is set.
    #[must_use]
    pub const fn contains(&self, mask: u16) -> bool {
        self.0 & mask == mask
    }
}

/// The evidence grade every sensing claim carries, exactly one.
///
/// This is the `CONTEXT.md` vocabulary. Do not invent synonyms: there are no
/// grades named "validated", "verified", "proven" or "trusted".
///
/// The variants are ordered strongest first. `Gated` is the terminal state of a
/// `Measured` claim; `DataGated` and `HardwareGated` are the two blocked
/// sub-grades of `Claimed` (`evaluation.md` §5), naming the missing input or
/// the missing hardware respectively.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Witness {
    /// Measured and passing a defined v1 bar. The only grade a headline claim
    /// may carry. Derived from a `Gated` ledger record with `passed == true`.
    Gated = 0,
    /// Computed from real, replayed or simulated input and recorded in the
    /// commit-keyed ledger. True by construction for any recorded result.
    Measured = 1,
    /// Asserted without a recorded measurement. Never emitted to Home
    /// Assistant.
    Claimed = 2,
    /// Blocked on missing input: no fixture, no labels, no reference recording,
    /// or insufficient paired samples. A named, pending measurement.
    DataGated = 3,
    /// Blocked on missing hardware: a reference device, chest strap or belt,
    /// camera rig, or the reference-scale node deployment. A named, pending
    /// measurement.
    HardwareGated = 4,
}

impl Witness {
    /// Whether a claim at this grade may carry a number.
    ///
    /// Only `Measured` and `Gated` may; every other grade is a pending or
    /// unbacked claim.
    #[must_use]
    pub const fn may_carry_value(&self) -> bool {
        matches!(self, Witness::Measured | Witness::Gated)
    }
}

/// A confidence value in `[0, 1]`.
///
/// Used where a model reports its own certainty. Confidence is independent of
/// [`Witness`]: a claim can be confidently wrong, or correctly asserted with
/// weak evidence.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Confidence(f32);

impl Confidence {
    /// Zero confidence.
    pub const ZERO: Confidence = Confidence(0.0);
    /// Full confidence.
    pub const ONE: Confidence = Confidence(1.0);

    /// Construct a confidence, rejecting values outside `[0, 1]` or non-finite
    /// values.
    pub fn new(value: f32) -> Result<Self, ValidationError> {
        require_unit_interval("confidence", value)?;
        Ok(Self(value))
    }

    /// The inner value.
    #[must_use]
    pub const fn get(&self) -> f32 {
        self.0
    }

    /// Validate the `[0, 1]` invariant.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_unit_interval("confidence", self.0)
    }
}

/// A calibration-quality score in `[0, 1]`.
///
/// Fusion derives a node's weight from its calibration quality
/// (`fusion-sync.md` §2.3): a node with quality `0` contributes nothing.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Quality(f32);

impl Quality {
    /// Zero quality; the node contributes nothing.
    pub const ZERO: Quality = Quality(0.0);
    /// Full quality.
    pub const ONE: Quality = Quality(1.0);

    /// Construct a quality score, rejecting values outside `[0, 1]` or
    /// non-finite values.
    pub fn new(value: f32) -> Result<Self, ValidationError> {
        require_finite_f32("quality", value)?;
        if (0.0..=1.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ValidationError::Range {
                field: "quality",
                detail: format!("{value} is not in [0, 1]"),
            })
        }
    }

    /// The inner value.
    #[must_use]
    pub const fn get(&self) -> f32 {
        self.0
    }

    /// Validate the `[0, 1]` invariant.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_finite_f32("quality", self.0)?;
        if (0.0..=1.0).contains(&self.0) {
            Ok(())
        } else {
            Err(ValidationError::Range {
                field: "quality",
                detail: format!("{} is not in [0, 1]", self.0),
            })
        }
    }
}
