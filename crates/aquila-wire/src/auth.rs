//! Per-node authentication of the node control plane (`capture-and-firmware.md`
//! §6.3).
//!
//! A paired node authenticates every inbound control command with the long-lived
//! token the core issued in `JOIN_ACK`. The token is the HMAC-SHA256 key; the
//! message is the control frame's header and body (everything before the
//! trailer). [`encode_control_authenticated`](crate::encode_control_authenticated)
//! appends the 32-byte tag and
//! [`decode_control_authenticated`](crate::decode_control_authenticated) verifies
//! it before decoding, so an unauthenticated or tampered command never reaches a
//! handler.
//!
//! This is a *keyed* primitive, unlike the content-addressing hash in
//! `aquila-types`; it is the only MAC in the workspace and exists so the node
//! can tell a genuine core command from a LAN forgery. Comparison is
//! constant-time. The scheme is additive: the plain `encode_control` /
//! `decode_control` pair is unchanged, and a tokenless core keeps working until
//! it is upgraded to attach the tag (the node fails closed in the meantime).

use sha2::{Digest, Sha256};

/// The per-node pairing token is 32 bytes; the HMAC-SHA256 block size is 64.
const TOKEN_LEN: usize = crate::constants::CONTROL_AUTH_LEN;
const BLOCK_LEN: usize = 64;

/// HMAC-SHA256 of `message` under `key` (RFC 2104).
fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut block = [0u8; BLOCK_LEN];
    if key.len() > BLOCK_LEN {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }

    let mut ipad = [0x36u8; BLOCK_LEN];
    let mut opad = [0x5cu8; BLOCK_LEN];
    for index in 0..BLOCK_LEN {
        ipad[index] ^= block[index];
        opad[index] ^= block[index];
    }

    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(message);
    let inner = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner);
    let mut tag = [0u8; 32];
    tag.copy_from_slice(&outer.finalize());
    tag
}

/// The AQCC authentication tag over a control frame, keyed by the pairing token.
///
/// `frame` is the control header and body with no trailer
/// (`AQ_CONTROL_HEADER_LEN + body_len` bytes). The result is written as the
/// message's 32-byte trailer.
#[must_use]
pub fn control_auth_tag(token: &[u8; TOKEN_LEN], frame: &[u8]) -> [u8; 32] {
    hmac_sha256(token, frame)
}

/// Constant-time verification of a control frame's authentication tag.
///
/// Returns `false` for a tag of the wrong length without touching memory, and
/// otherwise accumulates the difference over the whole tag so the comparison
/// leaks no position.
#[must_use]
pub fn verify_control_auth(token: &[u8; TOKEN_LEN], frame: &[u8], tag: &[u8]) -> bool {
    if tag.len() != TOKEN_LEN {
        return false;
    }
    let expected = control_auth_tag(token, frame);
    let mut difference = 0u8;
    for (a, b) in expected.iter().zip(tag.iter()) {
        difference |= a ^ b;
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 4231 test case 2 pins the HMAC construction byte for byte.
    #[test]
    fn hmac_matches_rfc4231_case2() {
        let tag = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        let mut hex = String::new();
        for byte in tag {
            use core::fmt::Write as _;
            let _ = write!(hex, "{byte:02x}");
        }
        assert_eq!(
            hex,
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn verification_rejects_a_bad_tag_length() {
        let token = [0x11u8; 32];
        assert!(!verify_control_auth(&token, b"frame", &[0u8; 31]));
        assert!(!verify_control_auth(&token, b"frame", &[]));
    }

    #[test]
    fn verification_is_key_and_message_sensitive() {
        let token = [0x11u8; 32];
        let other = [0x22u8; 32];
        let good = control_auth_tag(&token, b"frame");
        assert!(verify_control_auth(&token, b"frame", &good));
        assert!(!verify_control_auth(&other, b"frame", &good));
        assert!(!verify_control_auth(&token, b"framf", &good));
    }
}
