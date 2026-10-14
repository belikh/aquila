//! The workspace's single SHA-256 implementation.
//!
//! The content-addressed identities in this repository — a session trailer, a
//! `MeasurementId`, a calibration `body_sha256` and `calibration_id` — are all
//! SHA-256 of a canonical byte string. Before this module each crate that
//! needed one carried its own hand-rolled FIPS 180-4 implementation (audit
//! #13); there is now exactly one, owned by the level-0 vocabulary crate and
//! backed by the provisioned RustCrypto [`sha2`] crate.
//!
//! These functions are used only for identity and reproducibility: they are
//! **not** a keyed security boundary and must never be relied on for anything
//! but deduplication, integrity and content addressing.

use sha2::{Digest, Sha256};

/// The SHA-256 digest of `data`, as 32 big-endian bytes.
///
/// This is the one hashing primitive the workspace uses. Every content
/// address, integrity trailer and derived identifier calls it.
#[must_use]
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(data);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

/// Render a byte string as lower-case hexadecimal.
///
/// Intended for a 32-byte digest, but accepts any slice. Used to format
/// `sha256:<hex>` identities and error messages.
#[must_use]
pub fn to_hex(bytes: &[u8]) -> String {
    use core::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        // Writing to a `String` cannot fail.
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{sha256, to_hex};

    #[test]
    fn matches_the_published_vectors() {
        assert_eq!(
            to_hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            to_hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            to_hex(&sha256(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn padding_boundaries_are_distinct() {
        assert_eq!(sha256(&[0u8; 55]).len(), 32);
        assert_ne!(sha256(&[0u8; 55]), sha256(&[0u8; 56]));
        assert_ne!(sha256(&[0u8; 63]), sha256(&[0u8; 64]));
    }

    #[test]
    fn is_deterministic_across_lengths() {
        for n in [1usize, 55, 56, 63, 64, 65, 127, 128, 129, 1000] {
            let data: Vec<u8> = (0..n).map(|i| (i % 251) as u8).collect();
            assert_eq!(sha256(&data), sha256(&data));
        }
    }
}
