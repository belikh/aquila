/*
 * node_wire_util.c — the C framing helpers, pinned to the generated layout.
 */

#include "node_wire_util.h"

#include <string.h>

size_t aq_csi_datagram_len(uint16_t n_subcarriers)
{
    if (n_subcarriers == 0u || n_subcarriers > AQ_CSI_MAX_SUBCARRIERS) {
        return 0u;
    }
    return (size_t)AQ_CSI_HEADER_LEN + (size_t)AQ_CSI_BODY_FIXED_LEN +
           (size_t)2u * (size_t)n_subcarriers;
}

void aq_put_u16_le(uint8_t *out, uint16_t value)
{
    out[0] = (uint8_t)(value & 0xFFu);
    out[1] = (uint8_t)((value >> 8) & 0xFFu);
}

void aq_put_u32_le(uint8_t *out, uint32_t value)
{
    out[0] = (uint8_t)(value & 0xFFu);
    out[1] = (uint8_t)((value >> 8) & 0xFFu);
    out[2] = (uint8_t)((value >> 16) & 0xFFu);
    out[3] = (uint8_t)((value >> 24) & 0xFFu);
}

void aq_put_u64_le(uint8_t *out, uint64_t value)
{
    for (int i = 0; i < 8; ++i) {
        out[i] = (uint8_t)((value >> (8 * i)) & 0xFFu);
    }
}

void aq_put_i16_le(uint8_t *out, int16_t value)
{
    aq_put_u16_le(out, (uint16_t)value);
}

uint16_t aq_get_u16_le(const uint8_t *in)
{
    return (uint16_t)in[0] | ((uint16_t)in[1] << 8);
}

uint32_t aq_get_u32_le(const uint8_t *in)
{
    return (uint32_t)in[0] | ((uint32_t)in[1] << 8) | ((uint32_t)in[2] << 16) |
           ((uint32_t)in[3] << 24);
}

uint64_t aq_get_u64_le(const uint8_t *in)
{
    uint64_t value = 0u;
    for (int i = 0; i < 8; ++i) {
        value |= ((uint64_t)in[i]) << (8 * i);
    }
    return value;
}

int16_t aq_get_i16_le(const uint8_t *in)
{
    return (int16_t)aq_get_u16_le(in);
}

size_t aq_frame_csi(uint16_t node_id, uint8_t band, uint8_t channel,
                    uint8_t bandwidth_mhz, uint64_t sequence,
                    uint64_t device_time_us, uint16_t boot_epoch,
                    int16_t subcarrier_offset, uint16_t n_subcarriers,
                    int8_t rssi_dbm, int8_t noise_floor_dbm, uint16_t phy_flags,
                    bool first_word_invalid, uint16_t rx_seq,
                    uint16_t rx_flags, uint32_t rx_timestamp,
                    const uint8_t *source_mac, const int8_t *i_q, uint8_t *out,
                    size_t out_cap)
{
    const size_t payload_len = (size_t)AQ_CSI_BODY_FIXED_LEN + (size_t)2u * (size_t)n_subcarriers;
    const size_t total = (size_t)AQ_CSI_HEADER_LEN + payload_len;
    if (i_q == NULL || out == NULL) {
        return 0u;
    }
    if (n_subcarriers == 0u || n_subcarriers > AQ_CSI_MAX_SUBCARRIERS ||
        total > out_cap || total > AQ_CSI_MAX_DATAGRAM_LEN) {
        return 0u;
    }

    /* Fixed 16-byte header. */
    out[AQ_CSI_OFF_MAGIC + 0] = AQ_CSI_MAGIC_0;
    out[AQ_CSI_OFF_MAGIC + 1] = AQ_CSI_MAGIC_1;
    out[AQ_CSI_OFF_MAGIC + 2] = AQ_CSI_MAGIC_2;
    out[AQ_CSI_OFF_MAGIC + 3] = AQ_CSI_MAGIC_3;
    out[AQ_CSI_OFF_WIRE_MAJOR] = AQ_WIRE_MAJOR;
    out[AQ_CSI_OFF_WIRE_MINOR] = AQ_WIRE_MINOR;
    out[AQ_CSI_OFF_MESSAGE_TYPE] = AQ_CSI_MESSAGE_TYPE;
    out[AQ_CSI_OFF_SUBCARRIER_FORMAT] = AQ_SUBCARRIER_FORMAT_INT8_IQ;
    aq_put_u16_le(&out[AQ_CSI_OFF_NODE_ID], node_id);
    aq_put_u16_le(&out[AQ_CSI_OFF_PAYLOAD_LEN], (uint16_t)payload_len);
    out[AQ_CSI_OFF_BAND] = band;
    out[AQ_CSI_OFF_CHANNEL] = channel;
    out[AQ_CSI_OFF_BANDWIDTH_MHZ] = bandwidth_mhz;
    out[AQ_CSI_OFF_FLAGS] =
        first_word_invalid ? AQ_CSI_FLAG_FIRST_WORD_INVALID : 0u;

    /* Fixed body, relative to the start of the body. */
    uint8_t *body = out + AQ_CSI_HEADER_LEN;
    aq_put_u64_le(&body[AQ_CSI_BODY_OFF_SEQUENCE], sequence);
    aq_put_u64_le(&body[AQ_CSI_BODY_OFF_DEVICE_TIME_US], device_time_us);
    aq_put_u16_le(&body[AQ_CSI_BODY_OFF_BOOT_EPOCH], boot_epoch);
    aq_put_i16_le(&body[AQ_CSI_BODY_OFF_SUBCARRIER_OFFSET], subcarrier_offset);
    aq_put_u16_le(&body[AQ_CSI_BODY_OFF_N_SUBCARRIERS], n_subcarriers);
    body[AQ_CSI_BODY_OFF_RSSI_DBM] = (uint8_t)rssi_dbm;
    body[AQ_CSI_BODY_OFF_NOISE_FLOOR_DBM] = (uint8_t)noise_floor_dbm;
    aq_put_u16_le(&body[AQ_CSI_BODY_OFF_PHY_FLAGS], phy_flags);
    /*
     * The 802.11 sequence number of the received frame. The driver reports
     * 0xFFFF when it does not know it; that sentinel is not a legal sequence
     * number, so it carries through unchanged.
     */
    aq_put_u16_le(&body[AQ_CSI_BODY_OFF_RX_SEQ], rx_seq);

    /*
     * The receiver's per-frame link parameters and its receive timestamp
     * (wire minor 2 and later).
     */
    aq_put_u16_le(&body[AQ_CSI_BODY_OFF_RX_FLAGS], rx_flags);
    aq_put_u32_le(&body[AQ_CSI_BODY_OFF_RX_TIMESTAMP], rx_timestamp);

    /*
     * The six-byte MAC address of the station that transmitted the frame
     * (wire minor 1). The core needs it to tell two nodes' captures of the
     * same ambient packet apart from two different packets.
     */
    if (source_mac != NULL) {
        memcpy(&body[AQ_CSI_BODY_OFF_SOURCE_MAC], source_mac, 6u);
    } else {
        memset(&body[AQ_CSI_BODY_OFF_SOURCE_MAC], 0, 6u);
    }

    /*
     * Raw signed int8 I/Q, interleaved Q then I (imaginary first) per
     * subcarrier: `2 * n` bytes. The receive buffer is copied verbatim; the
     * node does no floating-point conversion (ADR-0005).
     */
    memcpy(body + AQ_CSI_BODY_OFF_IQ, i_q,
           sizeof(int8_t) * 2u * (size_t)n_subcarriers);

    return total;
}

size_t aq_control_msg_len(uint16_t body_len)
{
    return (size_t)AQ_CONTROL_HEADER_LEN + (size_t)body_len;
}

size_t aq_frame_control(uint8_t opcode, uint32_t request_id, const void *body,
                        uint16_t body_len, uint8_t *out, size_t out_cap)
{
    const size_t total = aq_control_msg_len(body_len);
    if (out == NULL || total > out_cap) {
        return 0u;
    }
    if (body_len > 0u && body == NULL) {
        return 0u;
    }

    out[AQ_CONTROL_OFF_MAGIC + 0] = AQ_CONTROL_MAGIC_0;
    out[AQ_CONTROL_OFF_MAGIC + 1] = AQ_CONTROL_MAGIC_1;
    out[AQ_CONTROL_OFF_MAGIC + 2] = AQ_CONTROL_MAGIC_2;
    out[AQ_CONTROL_OFF_MAGIC + 3] = AQ_CONTROL_MAGIC_3;
    out[AQ_CONTROL_OFF_VERSION] = AQ_WIRE_MAJOR;
    out[AQ_CONTROL_OFF_OPCODE] = opcode;
    aq_put_u16_le(&out[AQ_CONTROL_OFF_BODY_LEN], body_len);
    aq_put_u32_le(&out[AQ_CONTROL_OFF_REQUEST_ID], request_id);
    if (body_len > 0u) {
        memcpy(out + AQ_CONTROL_OFF_BODY, body, body_len);
    }
    return total;
}
