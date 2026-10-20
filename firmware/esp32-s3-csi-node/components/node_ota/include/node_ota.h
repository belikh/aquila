/*
 * node_ota.h — the node-side firmware OTA agent (docs/spec/firmware-ota.md §5,
 * §6, §8).
 *
 * The agent handles the core→node OTA opcodes (`OTA_BEGIN`, `OTA_CHUNK`,
 * `OTA_END`, `OTA_ABORT`) over the existing AQCC control channel, writes the
 * image sequentially into the inactive A/B partition with `esp_ota_*`, checks a
 * CRC-32 per chunk and SHA-256 + Ed25519 over the whole image, activates it and
 * reboots. On the first boot of a new image it runs a pending-verify health
 * window and either confirms the image or lets the bootloader roll back.
 *
 * It owns no socket: the control channel receives the datagrams, authenticates
 * them and calls `node_ota_handle`; the agent replies through the injected
 * `send` callback so the OTA path reuses the one authenticated control channel
 * (§2). The agent depends on `node_control` only through that callback, so the
 * component dependency stays acyclic.
 *
 * Compile-verified with ESP-IDF v5.5.2 (`nix develop .#esp`); not flashed or
 * run on hardware (see the firmware README).
 */
#ifndef AQUILA_NODE_OTA_H
#define AQUILA_NODE_OTA_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Frame and send one control message back to the core. */
typedef void (*node_ota_send_fn)(uint8_t opcode, uint32_t request_id,
                                 const uint8_t *body, uint16_t body_len);

/* Health probe used by the pending-verify window: associated + streaming +
 * control responsive, composed by the caller. */
typedef bool (*node_ota_health_fn)(void *ctx);

typedef struct {
    uint16_t node_id;       /* dense node id, for the INFO logs */
    node_ota_send_fn send;  /* the control channel's reply path (may be NULL) */
    node_ota_health_fn health; /* pending-verify health probe (may be NULL) */
    void *health_ctx;       /* opaque context for `health` */
    uint32_t health_window_ms; /* 0 -> CONFIG_AQUILA_OTA_HEALTH_WINDOW_S */
} node_ota_config_t;

/* Initialise the agent: load the trust anchor and security version, and start
 * the pending-verify health task if this boot is an unconfirmed OTA image.
 * Returns 0 on success (a missing trust anchor is not a start failure: it just
 * makes every verification fail closed). */
int node_ota_start(const node_ota_config_t *config);

/* Whether `opcode` is a core→node OTA opcode the agent handles. */
bool node_ota_handles(uint8_t opcode);

/* Handle one authenticated OTA control message (called by node_control). */
void node_ota_handle(uint8_t opcode, uint32_t request_id, const uint8_t *body,
                     uint16_t body_len);

/* Flush any deferred node→core OTA message (for example the post-reboot
 * `OTA_RESULT activated`). Called from the control task loop. */
void node_ota_poll(void);

/* The effective anti-rollback security-version floor. */
uint32_t node_ota_security_version(void);

/* Whether this boot is an unconfirmed OTA image pending health verification. */
bool node_ota_pending_verify(void);

#ifdef __cplusplus
}
#endif

#endif /* AQUILA_NODE_OTA_H */
