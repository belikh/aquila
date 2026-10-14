/*
 * node_wire_util.h — little-endian framing helpers for the Aquila wire.
 *
 * This component owns the C half of "one wire layout, two languages"
 * (docs/spec/capture-and-firmware.md §9.4). The byte layout itself is defined in
 * the generated `node_wire.h` (from aquila-wire::generate_c_header()); this
 * header only adds the framing helpers the firmware needs, so the layout has a
 * single definition.
 *
 * Nothing here allocates or logs. Every helper is bounded by a caller-supplied
 * capacity and returns 0 on overflow rather than writing past the buffer.
 */
#ifndef AQUILA_NODE_WIRE_UTIL_H
#define AQUILA_NODE_WIRE_UTIL_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "node_wire.h"

#ifdef __cplusplus
extern "C" {
#endif

/* The number of bytes a CSI datagram carrying `n_subcarriers` occupies. */
size_t aq_csi_datagram_len(uint16_t n_subcarriers);

/*
 * Frame one AQCI datagram into `out` (capacity `out_cap`).
 *
 * `i_q` holds `2 * n_subcarriers` raw signed int8 I/Q bytes, interleaved `Q, I`
 * (imaginary first) per subcarrier in ascending order, as AQCI requires
 * (ADR-0005). The buffer is copied verbatim; no floating-point conversion
 * happens on the node. `first_word_invalid` sets bit 0 of the header flags
 * byte. Returns the datagram length in bytes, or 0 when the buffer is too small
 * or the subcarrier count is out of range.
 */
size_t aq_frame_csi(uint16_t node_id, uint8_t band, uint8_t channel,
                    uint8_t bandwidth_mhz, uint64_t sequence,
                    uint64_t device_time_us, uint16_t boot_epoch,
                    int16_t subcarrier_offset, uint16_t n_subcarriers,
                    int8_t rssi_dbm, int8_t noise_floor_dbm, uint16_t phy_flags,
                    bool first_word_invalid, const int8_t *i_q, uint8_t *out,
                    size_t out_cap);

/* The total AQCC message length for a body of `body_len` bytes. */
size_t aq_control_msg_len(uint16_t body_len);

/*
 * Frame one AQCC control message into `out` (capacity `out_cap`).
 *
 * Returns the message length in bytes, or 0 on overflow.
 */
size_t aq_frame_control(uint8_t opcode, uint32_t request_id, const void *body,
                        uint16_t body_len, uint8_t *out, size_t out_cap);

/* Little-endian scalar writers. `out` must have the stated width. */
void aq_put_u16_le(uint8_t *out, uint16_t value);
void aq_put_u32_le(uint8_t *out, uint32_t value);
void aq_put_u64_le(uint8_t *out, uint64_t value);
void aq_put_i16_le(uint8_t *out, int16_t value);

/* Little-endian scalar readers. `in` must have the stated width. */
uint16_t aq_get_u16_le(const uint8_t *in);
uint32_t aq_get_u32_le(const uint8_t *in);
uint64_t aq_get_u64_le(const uint8_t *in);
int16_t aq_get_i16_le(const uint8_t *in);

#ifdef __cplusplus
}
#endif

#endif /* AQUILA_NODE_WIRE_UTIL_H */
