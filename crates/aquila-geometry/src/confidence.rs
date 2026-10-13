//! Confidence-scored outputs and the confidence algebra.
//!
//! Geometry in Aquila is **approximate and confidence-scored**: there is no
//! cross-node phase coherence, so no cross-node angle-of-arrival, no
//! ambiguity-free time-of-flight and no coherent array gain (see
//! `geometry-calibration.md` §0). Every geometric output therefore carries a
//! [`Confidence`] and, where the evidence is not decisive, an `ambiguous` flag.
//! Nothing downstream may treat an *advisory* position as a hard input.
//!
//! The algebra is the three operations named by `geometry-calibration.md` §2:
//!
//! - [`chain`] — composing transforms multiplies confidences;
//! - [`conjunction`] — independent requirements take the minimum;
//! - [`independent`] — independent evidence combines by a noisy-OR.
//!
//! Monotonicity (more evidence never lowers confidence) is a property test.

use aquila_types::Confidence;
use serde::{Deserialize, Serialize};

/// A value together with the confidence the producer has in it, and whether the
/// evidence was ambiguous.
///
/// `value == None` with `confidence == 0.0` is the honest answer for anything
/// not known: an unknown node, an unsurveyed position, or an observation that
/// touches no node in the topology. `ambiguous == true` records that two
/// candidate answers were within the ambiguity margin, so the value is *not* a
/// confident single answer (`geometry-calibration.md` §4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scored<T> {
    /// The value, or `None` when nothing could be attributed.
    pub value: Option<T>,
    /// The confidence in `value`, independent of [`aquila_types::Witness`].
    pub confidence: Confidence,
    /// Whether the top two candidates were within the ambiguity margin.
    pub ambiguous: bool,
}

impl<T> Scored<T> {
    /// The honest "nothing known" answer: no value, zero confidence, not
    /// ambiguous.
    #[must_use]
    pub const fn unknown() -> Self {
        Self {
            value: None,
            confidence: Confidence::ZERO,
            ambiguous: false,
        }
    }

    /// A fully confident, unambiguous answer.
    #[must_use]
    pub const fn certain(value: T) -> Self {
        Self {
            value: Some(value),
            confidence: Confidence::ONE,
            ambiguous: false,
        }
    }

    /// Construct from its parts.
    #[must_use]
    pub const fn new(value: Option<T>, confidence: Confidence, ambiguous: bool) -> Self {
        Self {
            value,
            confidence,
            ambiguous,
        }
    }

    /// Whether a value is present.
    #[must_use]
    pub const fn is_known(&self) -> bool {
        self.value.is_some()
    }

    /// Map the inner value, preserving confidence and ambiguity.
    #[must_use]
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Scored<U> {
        Scored {
            value: self.value.map(f),
            confidence: self.confidence,
            ambiguous: self.ambiguous,
        }
    }
}

/// Build a [`Confidence`] from a finite unit-interval value, clamping and
/// rejecting non-finite input to [`Confidence::ZERO`].
///
/// Geometry arithmetic can drift marginally outside `[0, 1]`; clamping keeps a
/// numerically safe value without inventing certainty.
#[must_use]
pub fn unit_confidence(value: f32) -> Confidence {
    if !value.is_finite() {
        return Confidence::ZERO;
    }
    Confidence::new(value.clamp(0.0, 1.0)).unwrap_or(Confidence::ZERO)
}

/// Compose two confidences: a transform through an unsure placement is itself
/// unsure, so confidences multiply.
#[must_use]
pub fn chain(a: Confidence, b: Confidence) -> Confidence {
    unit_confidence(a.get() * b.get())
}

/// Conjunction: a claim requiring several independent facts is only as
/// confident as the weakest, so confidences take the minimum.
#[must_use]
pub fn conjunction(a: Confidence, b: Confidence) -> Confidence {
    if a.get() <= b.get() {
        a
    } else {
        b
    }
}

/// Combine independent evidence by a noisy-OR, `1 - (1 - a)(1 - b)`.
///
/// Monotone: the result is never below either input, which is the
/// "more evidence never lowers confidence" invariant.
#[must_use]
pub fn independent(a: Confidence, b: Confidence) -> Confidence {
    unit_confidence(1.0 - (1.0 - a.get()) * (1.0 - b.get()))
}
