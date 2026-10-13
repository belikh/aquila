//! A tiny, dependency-free deterministic PRNG.
//!
//! The simulator is a pure function of its inputs and uses only a seeded PRNG
//! (`capture-and-firmware.md` §7.1), so this module provides SplitMix64 rather
//! than pulling a random-number crate. Every draw is a pure function of
//! `(seed, a, b, c)`, so the noise in a frame depends only on its coordinates
//! and never on iteration order: the same inputs produce byte-identical frames
//! on every run.

/// The SplitMix64 mixing function.
///
/// This is the standard public-domain construction: add the golden-ratio
/// increment, then two xor-shift-multiply rounds. It is used here as a pure
/// hash, not as an evolving stream, because a *stateless* hash keeps synthesis
/// order-independent and reproducible.
#[must_use]
pub(crate) fn splitmix64(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A uniform draw in `[0, 1)` keyed by `(seed, a, b, c, d)`.
///
/// Four coordinates let a draw depend on node, tick, subcarrier and stream at
/// once without folding them into fewer lanes.
#[must_use]
pub(crate) fn unit4(seed: u64, a: u64, b: u64, c: u64, d: u64) -> f64 {
    let hashed = splitmix64(seed ^ splitmix64(a ^ splitmix64(b ^ splitmix64(c ^ splitmix64(d)))));
    (hashed >> 11) as f64 / (1u64 << 53) as f64
}

/// A uniform draw in `[-1, 1)` keyed by `(seed, a, b, c, d)`.
#[must_use]
pub(crate) fn signed4(seed: u64, a: u64, b: u64, c: u64, d: u64) -> f64 {
    unit4(seed, a, b, c, d) * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::{signed4, splitmix64, unit4};

    #[test]
    fn deterministic_and_in_range() {
        for i in 0..64u64 {
            let a = unit4(7, i, i + 1, i + 2, i + 3);
            assert!((0.0..1.0).contains(&a));
            assert_eq!(a, unit4(7, i, i + 1, i + 2, i + 3));
            assert!(signed4(7, i, i + 1, i + 2, i + 3) >= -1.0);
        }
    }

    #[test]
    fn seed_changes_the_stream() {
        assert_ne!(splitmix64(1), splitmix64(2));
        assert_ne!(unit4(1, 0, 0, 0, 0), unit4(2, 0, 0, 0, 0));
    }
}
