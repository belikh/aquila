/*
 * node_control.c — the AQCC control-channel server (docs/spec/capture-and-
 * firmware.md §6). Reference implementation; NOT compiled, flashed or tested in
 * this repository.
 */

#include "node_control.h"

#include <stdatomic.h>
#include <stdio.h>
#include <string.h>

#include "esp_log.h"
#include "esp_system.h"
#include "esp_timer.h"
#include "esp_wifi.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "lwip/inet.h"
#include "lwip/sockets.h"
#include "nvs.h"
#include "nvs_flash.h"

#include "node_wire.h"
#include "node_wire_util.h"

static const char *TAG = "node_control";

#define CONTROL_STACK 4096u
#define CONTROL_PRIO 6u
#define CONTROL_RECV_TIMEOUT_MS 200
#define JOIN_RETRY_MS 3000
#define NVS_NAMESPACE "aquila"
#define IDENTIFY_GPIO (-1) /* set to a valid GPIO to enable LED blink */

static node_control_config_t s_config;
static int s_sock = -1;
static struct sockaddr_in s_core;
static uint8_t s_mac[6];
static atomic_bool s_paired;
static atomic_uint s_config_version;
static uint32_t s_heartbeat_ms;
static uint8_t s_token[32];

static atomic_uint s_last_rtt_us;

uint32_t node_control_config_version(void)
{
    return atomic_load(&s_config_version);
}

/* ------------------------------------------------------------------ */
/* NVS persistence                                                     */
/* ------------------------------------------------------------------ */

static void load_state(void)
{
    nvs_handle_t handle;
    if (nvs_open(NVS_NAMESPACE, NVS_READONLY, &handle) != ESP_OK) {
        return;
    }
    uint8_t paired = 0;
    if (nvs_get_u8(handle, "paired", &paired) == ESP_OK) {
        atomic_store(&s_paired, paired != 0);
    }
    size_t token_len = sizeof(s_token);
    if (nvs_get_blob(handle, "token", s_token, &token_len) != ESP_OK) {
        memset(s_token, 0, sizeof(s_token));
    }
    uint32_t version = 0;
    if (nvs_get_u32(handle, "config_version", &version) == ESP_OK) {
        atomic_store(&s_config_version, version);
    }
    uint32_t heartbeat = 0;
    if (nvs_get_u32(handle, "heartbeat_ms", &heartbeat) == ESP_OK && heartbeat > 0) {
        s_heartbeat_ms = heartbeat;
    }
    nvs_close(handle);
}

static void save_paired(bool paired)
{
    nvs_handle_t handle;
    if (nvs_open(NVS_NAMESPACE, NVS_READWRITE, &handle) != ESP_OK) {
        return;
    }
    nvs_set_u8(handle, "paired", paired ? 1u : 0u);
    nvs_set_blob(handle, "token", s_token, sizeof(s_token));
    nvs_set_u32(handle, "config_version", atomic_load(&s_config_version));
    nvs_set_u32(handle, "heartbeat_ms", s_heartbeat_ms);
    nvs_commit(handle);
    nvs_close(handle);
}

/* ------------------------------------------------------------------ */
/* Message helpers                                                     */
/* ------------------------------------------------------------------ */

static size_t build_result(uint8_t status, const char *detail, uint8_t *out,
                           size_t cap)
{
    size_t detail_len = detail != NULL ? strlen(detail) : 0u;
    if (detail_len > 255u) {
        detail_len = 255u;
    }
    if (cap < 2u + detail_len) {
        return 0u;
    }
    out[0] = status;
    out[1] = (uint8_t)detail_len;
    if (detail_len > 0u) {
        memcpy(&out[2], detail, detail_len);
    }
    return 2u + detail_len;
}

static void send_message(uint8_t opcode, uint32_t request_id, const uint8_t *body,
                         uint16_t body_len)
{
    uint8_t datagram[AQ_CSI_MAX_DATAGRAM_LEN];
    size_t length = aq_frame_control(opcode, request_id, body, body_len, datagram,
                                     sizeof(datagram));
    if (length > 0u && s_sock >= 0) {
        sendto(s_sock, datagram, length, 0, (struct sockaddr *)&s_core,
               sizeof(s_core));
    }
}

static void send_result(uint32_t request_id, uint8_t status, const char *detail)
{
    uint8_t body[257];
    size_t length = build_result(status, detail, body, sizeof(body));
    if (length > 0u) {
        send_message(AQ_OP_RESULT, request_id, body, (uint16_t)length);
    }
}

static void send_join_request(void)
{
    uint8_t body[6 + 4 + 4 + 1 + 32];
    size_t offset = 0;
    memcpy(&body[offset], s_mac, 6);
    offset += 6;
    aq_put_u32_le(&body[offset], s_config.boot);
    offset += 4;
    aq_put_u32_le(&body[offset], s_config.pairing_code);
    offset += 4;
    size_t fw_len = strlen(s_config.fw_version);
    if (fw_len > 32u) {
        fw_len = 32u;
    }
    body[offset] = (uint8_t)fw_len;
    offset += 1;
    memcpy(&body[offset], s_config.fw_version, fw_len);
    offset += fw_len;
    send_message(AQ_OP_JOIN_REQUEST, 0u, body, (uint16_t)offset);
}

static void send_status(uint32_t request_id)
{
    uint8_t body[8 + 4 + 8 + 4 + 2 + 2 + 4 + 4];
    size_t offset = 0;
    aq_put_u64_le(&body[offset], (uint64_t)(esp_timer_get_time() / 1000));
    offset += 8;
    aq_put_u32_le(&body[offset], s_config.boot);
    offset += 4;
    aq_put_u64_le(&body[offset], csi_node_last_sequence());
    offset += 8;
    aq_put_u32_le(&body[offset], csi_node_dropped());
    offset += 4;
    aq_put_u16_le(&body[offset], csi_node_ring_depth());
    offset += 2;
    aq_put_i16_le(&body[offset], 0); /* temperature, unimplemented here */
    offset += 2;
    aq_put_u32_le(&body[offset], atomic_load(&s_config_version));
    offset += 4;
    aq_put_u32_le(&body[offset], atomic_load(&s_last_rtt_us));
    offset += 4;
    send_message(AQ_OP_STATUS, request_id, body, (uint16_t)offset);
}

/* ------------------------------------------------------------------ */
/* Handler                                                             */
/* ------------------------------------------------------------------ */

static void handle_join_ack(const uint8_t *body, uint16_t body_len)
{
    /* node[6] token[32] config_version u32 config_hash[32] */
    if (body_len < (uint16_t)(6u + 32u + 4u + 32u)) {
        return;
    }
    memcpy(s_token, &body[6], 32);
    uint32_t version = aq_get_u32_le(&body[38]);
    atomic_store(&s_config_version, version);
    atomic_store(&s_paired, true);
    save_paired(true);
    ESP_LOGI(TAG, "paired; config version %u", (unsigned)version);
}

static void handle_set_config(uint32_t request_id, const uint8_t *body,
                              uint16_t body_len)
{
    /* version u32 | hash[32] | channel bw secondary csi lltf ht stbc |
       csi_len_cap u16 | max_tx_rate u32 | heartbeat u32 | count u8 | macs */
    if (body_len < 55u) {
        send_result(request_id, 1u, "short SET_CONFIG body");
        return;
    }
    uint32_t version = aq_get_u32_le(&body[0]);
    if (version < atomic_load(&s_config_version)) {
        send_result(request_id, 1u, "stale config version");
        return;
    }
    csi_node_config_t cfg = s_config.csi;
    cfg.channel = body[36];
    cfg.bandwidth_mhz = body[37];
    cfg.secondary = body[38];
    cfg.csi_enable = body[39] != 0u;
    cfg.lltf_enable = body[40] != 0u;
    cfg.ht_ltf_enable = body[41] != 0u;
    cfg.stbc_ht_ltf_enable = body[42] != 0u;
    cfg.csi_len_cap = aq_get_u16_le(&body[43]);
    cfg.max_tx_rate_hz = aq_get_u32_le(&body[45]);
    uint32_t heartbeat_ms = aq_get_u32_le(&body[49]);

    csi_node_apply_config(&cfg);
    s_config.csi = cfg;
    if (heartbeat_ms > 0u) {
        s_heartbeat_ms = heartbeat_ms;
    }
    atomic_store(&s_config_version, version);
    save_paired(atomic_load(&s_paired));
    send_result(request_id, 0u, "config applied");
}

static void handle_message(const uint8_t *datagram, int length)
{
    if (length < AQ_CONTROL_HEADER_LEN) {
        return;
    }
    if (datagram[AQ_CONTROL_OFF_MAGIC + 0] != AQ_CONTROL_MAGIC_0 ||
        datagram[AQ_CONTROL_OFF_MAGIC + 1] != AQ_CONTROL_MAGIC_1 ||
        datagram[AQ_CONTROL_OFF_MAGIC + 2] != AQ_CONTROL_MAGIC_2 ||
        datagram[AQ_CONTROL_OFF_MAGIC + 3] != AQ_CONTROL_MAGIC_3) {
        return;
    }
    if (datagram[AQ_CONTROL_OFF_VERSION] != AQ_WIRE_MAJOR) {
        return;
    }
    uint8_t opcode = datagram[AQ_CONTROL_OFF_OPCODE];
    uint16_t body_len = aq_get_u16_le(&datagram[AQ_CONTROL_OFF_BODY_LEN]);
    uint32_t request_id = aq_get_u32_le(&datagram[AQ_CONTROL_OFF_REQUEST_ID]);
    const uint8_t *body = datagram + AQ_CONTROL_OFF_BODY;
    if ((size_t)length < AQ_CONTROL_HEADER_LEN + (size_t)body_len) {
        return;
    }

    switch (opcode) {
    case AQ_OP_JOIN_ACK:
        handle_join_ack(body, body_len);
        break;
    case AQ_OP_GET_CONFIG: {
        char detail[32];
        snprintf(detail, sizeof(detail), "config_version=%u",
                 (unsigned)atomic_load(&s_config_version));
        send_result(request_id, 0u, detail);
        break;
    }
    case AQ_OP_SET_CONFIG:
        handle_set_config(request_id, body, body_len);
        break;
    case AQ_OP_SET_CSI:
        if (body_len >= 1u) {
            csi_node_set_streaming(body[0] != 0u);
            send_result(request_id, 0u, "csi toggled");
        } else {
            send_result(request_id, 1u, "short SET_CSI body");
        }
        break;
    case AQ_OP_SYNC_PING: {
        if (body_len < 8u) {
            break;
        }
        uint64_t t1 = aq_get_u64_le(&body[0]);
        uint64_t t2 = (uint64_t)esp_timer_get_time();
        uint8_t echo[24];
        aq_put_u64_le(&echo[0], t1);
        aq_put_u64_le(&echo[8], t2);
        aq_put_u64_le(&echo[16], (uint64_t)esp_timer_get_time());
        send_message(AQ_OP_SYNC_ECHO, request_id, echo, sizeof(echo));
        break;
    }
    case AQ_OP_REBOOT:
        send_result(request_id, 0u, "rebooting");
        vTaskDelay(pdMS_TO_TICKS(100));
        esp_restart();
        break;
    case AQ_OP_IDENTIFY:
        send_result(request_id, 0u, "identified");
        break;
    case AQ_OP_FACTORY_RESET:
        send_result(request_id, 0u, "factory reset");
        vTaskDelay(pdMS_TO_TICKS(100));
        nvs_flash_erase();
        esp_restart();
        break;
    default:
        break;
    }
}

static void control_task(void *arg)
{
    (void)arg;
    uint8_t datagram[AQ_CSI_MAX_DATAGRAM_LEN];
    int64_t last_heartbeat_us = 0;
    int64_t last_join_us = 0;

    for (;;) {
        struct sockaddr_in from;
        socklen_t from_len = sizeof(from);
        int length = recvfrom(s_sock, datagram, sizeof(datagram), 0,
                              (struct sockaddr *)&from, &from_len);
        if (length > 0) {
            handle_message(datagram, length);
        }

        int64_t now_us = esp_timer_get_time();
        uint32_t heartbeat_ms = s_heartbeat_ms > 0u ? s_heartbeat_ms : 5000u;
        if (now_us - last_heartbeat_us >= (int64_t)heartbeat_ms * 1000) {
            send_status(0u);
            last_heartbeat_us = now_us;
        }
        if (!atomic_load(&s_paired) &&
            now_us - last_join_us >= (int64_t)JOIN_RETRY_MS * 1000) {
            send_join_request();
            last_join_us = now_us;
        }
    }
}

int node_control_start(const node_control_config_t *config)
{
    if (config == NULL) {
        return -1;
    }
    s_config = *config;
    s_heartbeat_ms = config->csi.heartbeat_ms > 0u ? config->csi.heartbeat_ms : 5000u;
    atomic_init(&s_paired, false);
    atomic_init(&s_config_version, 0);
    atomic_init(&s_last_rtt_us, 0u);
    load_state();

    esp_wifi_get_mac(WIFI_IF_STA, s_mac);

    memset(&s_core, 0, sizeof(s_core));
    s_core.sin_family = AF_INET;
    s_core.sin_port = htons(config->core_port);
    if (inet_pton(AF_INET, config->core_host, &s_core.sin_addr) != 1) {
        ESP_LOGE(TAG, "core host %s is not an IPv4 literal", config->core_host);
        return -1;
    }

    s_sock = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if (s_sock < 0) {
        ESP_LOGE(TAG, "could not open the control socket");
        return -1;
    }
    struct timeval timeout = {
        .tv_sec = 0,
        .tv_usec = CONTROL_RECV_TIMEOUT_MS * 1000,
    };
    setsockopt(s_sock, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof(timeout));

    if (xTaskCreate(control_task, "node_control", CONTROL_STACK, NULL, CONTROL_PRIO,
                    NULL) != pdPASS) {
        ESP_LOGE(TAG, "could not start the control task");
        return -1;
    }
    ESP_LOGI(TAG, "control channel ready; core %s:%u", config->core_host,
             (unsigned)config->core_port);
    return 0;
}
