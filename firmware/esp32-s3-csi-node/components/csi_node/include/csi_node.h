/*
 * csi_node.h — ESP32-S3 CSI capture and CSI data-plane sender.
 *
 * Responsibilities (docs/spec/capture-and-firmware.md §9.1): configure CSI via
 * the esp-csi API, capture in the receive callback without allocating or
 * blocking, and stream one AQCI datagram per captured frame to the core over
 * unicast UDP. The node performs no DSP, inference or clock discipline (§9.2).
 */
#ifndef AQUILA_CSI_NODE_H
#define AQUILA_CSI_NODE_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* The CSI configuration the node applies (`capture-and-firmware.md` §9.1). */
typedef struct {
    uint16_t node_id;      /* dense on-air node id (AQCI node_id) */
    uint16_t boot;         /* random boot epoch, u16 on the wire and control plane */
    uint8_t channel;       /* 802.11 primary channel */
    uint8_t bandwidth_mhz; /* 20, 40, 80 or 160 */
    uint8_t secondary;     /* 0 none, 1 above, 2 below */
    bool csi_enable;       /* stream while true */
    bool lltf_enable;
    bool ht_ltf_enable;
    bool stbc_ht_ltf_enable;
    uint16_t csi_len_cap;  /* maximum raw int8 CSI bytes (2 per subcarrier) */
    uint32_t max_tx_rate_hz; /* transmit-rate cap; frames over it are dropped */
    uint32_t heartbeat_ms; /* STATUS cadence in ms; SET_CONFIG may override */
    /* Manual CSI scale: the number of bits the scale is shifted by. The
     * automatic scale leaves the samples in a handful of int8 levels, which
     * costs phase precision; 255 means automatic. */
    uint8_t csi_shift;
} csi_node_config_t;

/* The v1 defaults for a freshly booted node. */
csi_node_config_t csi_node_default_config(void);

/*
 * Configure the radio for CSI capture and start the sender task.
 *
 * `core_host` is the core's address (an IP literal) and `core_port` its CSI
 * data port (default AQ_CSI_DATA_PORT). Returns 0 on success.
 */
int csi_node_start(const csi_node_config_t *config, const char *core_host,
                   uint16_t core_port);

/* Apply a new configuration transactionally; does not restart the task. */
void csi_node_apply_config(const csi_node_config_t *config);

/* Enable or disable CSI streaming (`SET_CSI`). */
void csi_node_set_streaming(bool enable);

/* Whether CSI streaming is currently enabled, for the OTA health window. */
bool csi_node_is_streaming(void);

/* The assignable (raw int8) CSI length cap for the current bandwidth. */
uint16_t csi_node_phy_cap(void);

/* Heartbeat accessors for the `STATUS` opcode (§6.4), each atomic. */
uint64_t csi_node_last_sequence(void);
uint32_t csi_node_dropped(void);
uint16_t csi_node_ring_depth(void);

#ifdef __cplusplus
}
#endif

#endif /* AQUILA_CSI_NODE_H */
