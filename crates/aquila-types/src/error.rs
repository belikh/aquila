//! Validation errors for the contract types.
//!
//! A contract value either satisfies every invariant its shape states, or it is
//! rejected with a [`ValidationError`]. Validation is pure and takes no clock,
//! no random source and no I/O, so a rejected value is rejected identically on
//! every host.

/// The error type returned by every `validate` method in this crate.
///
/// Variants name the offending field and the reason, so a caller can log or
/// surface the failure without re-deriving it. The error carries no
/// behaviour: it is data about a data shape.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    /// A numeric field lies outside its permitted range.
    #[error("field `{field}` is out of range: {detail}")]
    Range {
        /// The field that failed.
        field: &'static str,
        /// A human-readable description of the permitted range.
        detail: String,
    },
    /// A floating-point field is `NaN` or infinite where finiteness is required.
    #[error("field `{field}` must be finite")]
    NonFinite {
        /// The field that failed.
        field: &'static str,
    },
    /// A string identifier is empty where a non-empty identifier is required.
    #[error("field `{field}` must be a non-empty identifier")]
    EmptyId {
        /// The field that failed.
        field: &'static str,
    },
    /// Two related collections or arrays disagree in length.
    #[error("field `{field}` has length {actual}, expected {expected}")]
    Length {
        /// The field that failed.
        field: &'static str,
        /// The observed length.
        actual: usize,
        /// The required length.
        expected: usize,
    },
    /// A stated structural or referential invariant does not hold.
    #[error("invariant violated: {detail}")]
    Invariant {
        /// A human-readable description of the invariant that was broken.
        detail: String,
    },
}

/// Reject a non-finite `f32`.
pub(crate) fn require_finite_f32(field: &'static str, value: f32) -> Result<(), ValidationError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ValidationError::NonFinite { field })
    }
}

/// Reject a non-finite `f64`.
pub(crate) fn require_finite_f64(field: &'static str, value: f64) -> Result<(), ValidationError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ValidationError::NonFinite { field })
    }
}

/// Require an `f32` to be finite and within `[0, 1]`.
pub(crate) fn require_unit_interval(
    field: &'static str,
    value: f32,
) -> Result<(), ValidationError> {
    require_finite_f32(field, value)?;
    if (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(ValidationError::Range {
            field,
            detail: format!("{value} is not in [0, 1]"),
        })
    }
}

/// Require an arbitrary `f32` to be finite or `NaN`.
///
/// Used where the contract permits `NaN` to mean "no estimate" but never
/// permits an infinity.
pub(crate) fn require_not_infinite(field: &'static str, value: f32) -> Result<(), ValidationError> {
    if value.is_infinite() {
        Err(ValidationError::NonFinite { field })
    } else {
        Ok(())
    }
}

/// Require a string to be non-empty.
pub(crate) fn require_non_empty(field: &'static str, value: &str) -> Result<(), ValidationError> {
    if value.is_empty() {
        Err(ValidationError::EmptyId { field })
    } else {
        Ok(())
    }
}

/// Build an [`ValidationError::Invariant`] from a static message.
pub(crate) fn invariant(detail: &'static str) -> ValidationError {
    ValidationError::Invariant {
        detail: detail.to_string(),
    }
}
