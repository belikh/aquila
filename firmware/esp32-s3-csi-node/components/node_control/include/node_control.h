/*
 * node_control.h — the node control channel (AQCC) server.
 *
 * Implements the node side of docs/spec/capture-and-firmware.md §6: pairing
 * (§6.3), config, status heartbeat (§6.4), time-sync echo, reboot, identify and
 * factory reset. It is a UDP request/response protocol on its own port
 * (default 9101/udp), distinct from the CSI data plane. The node performs no
 * DSP, inference or clock discipline here either (§9.2).
 */
#ifndef AQUILA_NODE_CONTROL_H
#define AQUILA_NODE_CONTROL_H

#include <stdint.h>

#include "csi_node.h"

#ifdef __cplusplus
extern "C" {
#endif

/* The wiring the control task needs. */
typedef struct {
    uint16_t node_id;       /* dense node id */
    uint16_t boot;          /* boot epoch (u16 on the wire) */
    const char *core_host;  /* the core's IPv4 literal */
    uint16_t core_port;     /* the control port (default AQ_CONTROL_PORT) */
    uint32_t pairing_code;  /* the short-lived pairing code, 0 if none */
    const char *fw_version; /* firmware version string */
    csi_node_config_t csi;  /* the initial CSI configuration */
} node_control_config_t;

/* Start the control task. Returns 0 on success. */
int node_control_start(const node_control_config_t *config);

/* The active config version, for the heartbeat and replies. */
uint32_t node_control_config_version(void);

#ifdef __cplusplus
}
#endif

#endif /* AQUILA_NODE_CONTROL_H */
