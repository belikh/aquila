/*
 * ed25519.h — detached Ed25519 verification for the OTA agent.
 *
 * ESP-IDF v5.5.2 bundles mbedTLS 3.6.5, which implements Curve25519 for ECDH
 * but has no Ed25519 *signature* primitive (`PSA_ALG_PURE_EDDSA` is declared
 * but not implemented). The node therefore carries a small, self-contained
 * Ed25519 verifier; SHA-256 for the image digest is still mbedTLS.
 */
#ifndef AQUILA_NODE_OTA_ED25519_H
#define AQUILA_NODE_OTA_ED25519_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Verify an Ed25519 signature over a 32-byte message.
 *
 * The core signs the 32-byte SHA-256 digest of the image as an Ed25519 message
 * (`ed25519_dalek` `verify_strict(&digest, &signature)`); this matches.
 *
 * `public_key` is 32 bytes, `signature` 64 bytes and `message` 32 bytes.
 * Returns true only on a valid signature.
 */
bool aq_ed25519_verify(const uint8_t public_key[32],
                       const uint8_t signature[64],
                       const uint8_t message[32]);

#ifdef __cplusplus
}
#endif

#endif /* AQUILA_NODE_OTA_ED25519_H */
