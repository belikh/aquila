/*
 * node_control.c — the AQCC control-channel server (docs/spec/capture-and-
 * firmware.md §6). Compile-verified with ESP-IDF v5.5.2 (`nix develop .#esp`);
 * not flashed or run on hardware.
 *
 * Security (F21): after pairing, every inbound control command must carry the
 * 32-byte HMAC-SHA256 trailer defined in `capture-and-firmware.md` §6.1, keyed
 * by the pairing token issued in `JOIN_ACK`. The datagram source must also be
 * the pinned core endpoint, and a recent `request_id` is rejected as a replay
 * (§6.1). `JOIN_ACK` is the one message accepted before pairing, because it is
 * what delivers the token. Until the core sends tagged commands, the node fails
 * closed: it refuses everything but pairing.
 */

#include "node_control.h"

#include <stdatomic.h>
#include <stdio.h>
#include <string.h>

#include "driver/gpio.h"
#include "driver/temperature_sensor.h"
#include "esp_log.h"
#include "esp_system.h"
#include "esp_timer.h"
#include "esp_wifi.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "lwip/inet.h"
#include "lwip/sockets.h"
#include "mbedtls/md.h"
#include "nvs.h"
#include "nvs_flash.h"

#include "node_ota.h"
#include "node_wire.h"
#include "node_wire_util.h"

static const char *TAG = "node_control";

#define CONTROL_STACK 4096u
#define CONTROL_PRIO 6u
#define CONTROL_RECV_TIMEOUT_MS 200
#define JOIN_RETRY_MS 3000
#define NVS_NAMESPACE "aquila"
#define IDENTIFY_GPIO (-1) /* operator must set a valid GPIO here; IDENTIFY blink is compiled out until then */
#define REPLAY_WINDOW 16u  /* recent request_ids kept to reject retransmits */

static node_control_config_t s_config;
static int s_sock = -1;
static struct sockaddr_in s_core;
static uint8_t s_mac[6];
static atomic_bool s_paired;
static atomic_bool s_responsive;
static atomic_uint s_config_version;
static uint32_t s_heartbeat_ms;
static uint8_t s_token[32];

/* The node's own SYNC_PING echo turnaround (t3 - t2), microseconds. The core
 * computes the true RTT; the node can only report its own turnaround (F32). */
static atomic_uint s_echo_turnaround_us;

/* The internal temperature sensor; NULL when it could not be installed. */
static temperature_sensor_handle_t s_temperature_sensor;

/* A bounded ring of recently seen request_ids, for retransmit deduplication
 * (`capture-and-firmware.md` §6.1, F8). */
static uint32_t s_seen_ids[REPLAY_WINDOW];
static size_t s_seen_count;
static size_t s_seen_next;

static void send_message(uint8_t opcode, uint32_t request_id,
                         const uint8_t *body, uint16_t body_len);

uint32_t node_control_config_version(void)
{
    return atomic_load(&s_config_version);
}

void node_control_send_reply(uint8_t opcode, uint32_t request_id,
                             const uint8_t *body, uint16_t body_len)
{
    send_message(opcode, request_id, body, body_len);
}

bool node_control_is_responsive(void)
{
    return atomic_load(&s_responsive);
}

/* ------------------------------------------------------------------ */
/* Authentication (F21)                                                */
/* ------------------------------------------------------------------ */

/* Constant-time byte comparison, so the tag check leaks no position. */
static bool constant_time_eq(const uint8_t *a, const uint8_t *b, size_t len)
{
    uint8_t difference = 0u;
    for (size_t i = 0; i < len; ++i) {
        difference |= (uint8_t)(a[i] ^ b[i]);
    }
    return difference == 0u;
}

/*
 * Verify the 32-byte HMAC-SHA256 trailer over `frame_len` bytes of `frame`,
 * keyed by the stored pairing token. mbedtls is the ESP-IDF crypto backend.
 * Returns true only on an exact match.
 */
static bool verify_control_tag(const uint8_t *frame, size_t frame_len,
                               const uint8_t *tag)
{
    const mbedtls_md_info_t *md = mbedtls_md_info_from_type(MBEDTLS_MD_SHA256);
    if (md == NULL) {
        return false;
    }
    uint8_t computed[32];
    if (mbedtls_md_hmac(md, s_token, sizeof(s_token), frame, frame_len,
                        computed) != 0) {
        return false;
    }
    return constant_time_eq(computed, tag, sizeof(computed));
}

/* True when the datagram source is the pinned core endpoint. */
static bool from_core(const struct sockaddr_in *from)
{
    return from->sin_addr.s_addr == s_core.sin_addr.s_addr &&
           from->sin_port == s_core.sin_port;
}

/* True when `request_id` was seen in the replay window; records it otherwise. */
static bool request_is_duplicate(uint32_t request_id)
{
    for (size_t i = 0; i < s_seen_count; ++i) {
        if (s_seen_ids[i] == request_id) {
            return true;
        }
    }
    s_seen_ids[s_seen_next] = request_id;
    s_seen_next = (s_seen_next + 1u) % REPLAY_WINDOW;
    if (s_seen_count < REPLAY_WINDOW) {
        ++s_seen_count;
    }
    return false;
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
        s_config.csi.heartbeat_ms = heartbeat;
    }
    /* The persisted CSI config (capture-and-firmware.md §9.3, F9). */
    uint8_t channel = 0;
    if (nvs_get_u8(handle, "csi_channel", &channel) == ESP_OK && channel != 0u) {
        s_config.csi.channel = channel;
    }
    uint8_t bandwidth = 0;
    if (nvs_get_u8(handle, "csi_bandwidth", &bandwidth) == ESP_OK && bandwidth != 0u) {
        s_config.csi.bandwidth_mhz = bandwidth;
    }
    uint8_t secondary = 0;
    if (nvs_get_u8(handle, "csi_secondary", &secondary) == ESP_OK) {
        s_config.csi.secondary = secondary;
    }
    uint16_t cap = 0;
    if (nvs_get_u16(handle, "csi_len_cap", &cap) == ESP_OK && cap != 0u) {
        s_config.csi.csi_len_cap = cap;
    }
    uint32_t rate = 0;
    if (nvs_get_u32(handle, "max_tx_rate_hz", &rate) == ESP_OK && rate > 0u) {
        s_config.csi.max_tx_rate_hz = rate;
    }
    nvs_close(handle);
}

static void save_state(void)
{
    nvs_handle_t handle;
    if (nvs_open(NVS_NAMESPACE, NVS_READWRITE, &handle) != ESP_OK) {
        return;
    }
    nvs_set_u8(handle, "paired", atomic_load(&s_paired) ? 1u : 0u);
    nvs_set_blob(handle, "token", s_token, sizeof(s_token));
    nvs_set_u32(handle, "config_version", atomic_load(&s_config_version));
    nvs_set_u32(handle, "heartbeat_ms", s_heartbeat_ms);
    nvs_set_u8(handle, "csi_channel", s_config.csi.channel);
    nvs_set_u8(handle, "csi_bandwidth", s_config.csi.bandwidth_mhz);
    nvs_set_u8(handle, "csi_secondary", s_config.csi.secondary);
    nvs_set_u16(handle, "csi_len_cap", s_config.csi.csi_len_cap);
    nvs_set_u32(handle, "max_tx_rate_hz", s_config.csi.max_tx_rate_hz);
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
    uint8_t body[6 + 2 + 4 + 1 + 32];
    size_t offset = 0;
    memcpy(&body[offset], s_mac, 6);
    offset += 6;
    aq_put_u16_le(&body[offset], s_config.boot);
    offset += 2;
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

/* Read the internal temperature sensor, scaled by 100. Returns 0 when the
 * sensor is unavailable; `*valid` reports which (F31). */
static int16_t read_temperature_c_x100(bool *valid)
{
    if (s_temperature_sensor == NULL) {
        *valid = false;
        return 0;
    }
    float celsius = 0.0f;
    if (temperature_sensor_get_celsius(s_temperature_sensor, &celsius) != ESP_OK) {
        *valid = false;
        return 0;
    }
    *valid = true;
    return (int16_t)(celsius * 100.0f);
}

static void send_status(uint32_t request_id)
{
    uint8_t body[8 + 2 + 8 + 4 + 2 + 2 + 4 + 4];
    size_t offset = 0;
    aq_put_u64_le(&body[offset], (uint64_t)(esp_timer_get_time() / 1000));
    offset += 8;
    aq_put_u16_le(&body[offset], s_config.boot);
    offset += 2;
    aq_put_u64_le(&body[offset], csi_node_last_sequence());
    offset += 8;
    aq_put_u32_le(&body[offset], csi_node_dropped());
    offset += 4;
    aq_put_u16_le(&body[offset], csi_node_ring_depth());
    offset += 2;
    bool temperature_valid = false;
    int16_t temperature = read_temperature_c_x100(&temperature_valid);
    (void)temperature_valid; /* 0 already means "unavailable" on the wire */
    aq_put_i16_le(&body[offset], temperature);
    offset += 2;
    aq_put_u32_le(&body[offset], atomic_load(&s_config_version));
    offset += 4;
    /* The contract's STATUS latency field is the node's own SYNC_PING echo
     * turnaround (t3 - t2), not an RTT: only the core can measure the true
     * round-trip time (`capture-and-firmware.md` §6.4, F32). */
    aq_put_u32_le(&body[offset], atomic_load(&s_echo_turnaround_us));
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
    save_state();
    ESP_LOGI(TAG, "paired; config version %u", (unsigned)version);
}

static void handle_set_config(uint32_t request_id, const uint8_t *body,
                              uint16_t body_len)
{
    /* version u32 | hash[32] | channel bw secondary csi lltf ht stbc |
       csi_len_cap u16 | max_tx_rate u32 | heartbeat u32 | count u8 | macs */
    if (body_len < 54u) {
        send_result(request_id, 1u, "short SET_CONFIG body");
        return;
    }
    uint32_t version = aq_get_u32_le(&body[0]);
    if (version < atomic_load(&s_config_version)) {
        send_result(request_id, 1u, "stale config version");
        return;
    }
    /*
     * `config_hash` (body[4..36]) is carried but not yet verified: v1 defines no
     * canonical config-hash algorithm on the core side, so the node cannot
     * recompute it (F7). It is reserved until that algorithm is pinned.
     */
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
    if (heartbeat_ms > 0u) {
        cfg.heartbeat_ms = heartbeat_ms;
    }

    csi_node_apply_config(&cfg);
    s_config.csi = cfg;
    s_heartbeat_ms = cfg.heartbeat_ms > 0u ? cfg.heartbeat_ms : 5000u;
    atomic_store(&s_config_version, version);
    save_state();
    send_result(request_id, 0u, "config applied");
}

/*
 * `handle_message` runs only for a datagram that has already passed the
 * authenticated-path checks in `control_task`: magic, version, framing, the
 * pinned source, the HMAC trailer (once paired) and the replay window.
 */
static void handle_message(const uint8_t *datagram, int length)
{
    uint8_t opcode = datagram[AQ_CONTROL_OFF_OPCODE];
    uint16_t body_len = aq_get_u16_le(&datagram[AQ_CONTROL_OFF_BODY_LEN]);
    uint32_t request_id = aq_get_u32_le(&datagram[AQ_CONTROL_OFF_REQUEST_ID]);
    const uint8_t *body = datagram + AQ_CONTROL_OFF_BODY;
    (void)length;

    switch (opcode) {
    case AQ_OP_JOIN_ACK:
        handle_join_ack(body, body_len);
        break;
    case AQ_OP_GET_CONFIG: {
        csi_node_config_t cfg = s_config.csi;
        char detail[192];
        snprintf(detail, sizeof(detail),
                 "config_version=%u channel=%u bandwidth_mhz=%u secondary=%u "
                 "csi=%u lltf=%u ht_ltf=%u stbc=%u csi_len_cap=%u "
                 "max_tx_rate_hz=%u heartbeat_ms=%u",
                 (unsigned)atomic_load(&s_config_version), (unsigned)cfg.channel,
                 (unsigned)cfg.bandwidth_mhz, (unsigned)cfg.secondary,
                 cfg.csi_enable ? 1u : 0u, cfg.lltf_enable ? 1u : 0u,
                 cfg.ht_ltf_enable ? 1u : 0u, cfg.stbc_ht_ltf_enable ? 1u : 0u,
                 (unsigned)cfg.csi_len_cap, (unsigned)cfg.max_tx_rate_hz,
                 (unsigned)cfg.heartbeat_ms);
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
        uint64_t t3 = (uint64_t)esp_timer_get_time();
        aq_put_u64_le(&echo[16], t3);
        atomic_store(&s_echo_turnaround_us, (uint32_t)(t3 - t2));
        send_message(AQ_OP_SYNC_ECHO, request_id, echo, sizeof(echo));
        break;
    }
    case AQ_OP_REBOOT:
        send_result(request_id, 0u, "rebooting");
        vTaskDelay(pdMS_TO_TICKS(100));
        esp_restart();
        break;
    case AQ_OP_IDENTIFY:
#if IDENTIFY_GPIO >= 0
        gpio_set_direction(IDENTIFY_GPIO, GPIO_MODE_OUTPUT);
        for (int i = 0; i < 3; ++i) {
            gpio_set_level(IDENTIFY_GPIO, 1);
            vTaskDelay(pdMS_TO_TICKS(100));
            gpio_set_level(IDENTIFY_GPIO, 0);
            vTaskDelay(pdMS_TO_TICKS(100));
        }
#endif
        send_result(request_id, 0u, "identified");
        break;
    case AQ_OP_FACTORY_RESET:
        send_result(request_id, 0u, "factory reset");
        vTaskDelay(pdMS_TO_TICKS(100));
        nvs_flash_erase();
        esp_restart();
        break;
    case AQ_OP_OTA_BEGIN:
    case AQ_OP_OTA_CHUNK:
    case AQ_OP_OTA_END:
    case AQ_OP_OTA_ABORT:
        /* The firmware OTA agent writes the inactive partition and replies via
         * OTA_ACK/OTA_STATUS/OTA_RESULT (firmware-ota.md §5). */
        node_ota_handle(opcode, request_id, body, body_len);
        break;
    default:
        break;
    }
}

/*
 * Validate one received datagram and, if it is admissible, dispatch it.
 *
 * Admissible means: the fixed framing is well formed; either it is a `JOIN_ACK`
 * delivered before pairing, or the node is paired, the source is the pinned
 * core endpoint, the frame carries a valid HMAC trailer, and the `request_id`
 * is not a replay. Anything else is dropped without a reply.
 */
static void receive_message(const uint8_t *datagram, int length,
                            const struct sockaddr_in *from)
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
    size_t frame_len = (size_t)AQ_CONTROL_HEADER_LEN + (size_t)body_len;
    if ((size_t)length < frame_len) {
        return;
    }

    if (opcode == AQ_OP_JOIN_ACK) {
        /* Pairing is the bootstrap: the token is delivered here, so this one
         * message is accepted before the node is paired. */
        if (!atomic_load(&s_paired) && from_core(from)) {
            atomic_store(&s_responsive, true);
            handle_message(datagram, length);
        }
        return;
    }

    if (!atomic_load(&s_paired) || !from_core(from)) {
        return;
    }
    if ((size_t)length != frame_len + (size_t)AQ_CONTROL_AUTH_LEN) {
        return;
    }
    if (!verify_control_tag(datagram, frame_len,
                            datagram + frame_len)) {
        ESP_LOGW(TAG, "dropped control opcode %u with a bad tag",
                 (unsigned)opcode);
        return;
    }
    uint32_t request_id = aq_get_u32_le(&datagram[AQ_CONTROL_OFF_REQUEST_ID]);
    if (request_is_duplicate(request_id)) {
        ESP_LOGD(TAG, "dropped duplicate control request %u", (unsigned)request_id);
        return;
    }
    atomic_store(&s_responsive, true);
    handle_message(datagram, length);
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
            receive_message(datagram, length, &from);
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
        /* Flush any deferred OTA message (for example the post-reboot
         * `OTA_RESULT activated`). */
        node_ota_poll();
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
    atomic_init(&s_responsive, false);
    atomic_init(&s_config_version, 0);
    atomic_init(&s_echo_turnaround_us, 0u);
    s_seen_count = 0;
    s_seen_next = 0;
    load_state();

    esp_wifi_get_mac(WIFI_IF_STA, s_mac);

    memset(&s_core, 0, sizeof(s_core));
    s_core.sin_family = AF_INET;
    s_core.sin_port = htons(config->core_port);
    if (inet_pton(AF_INET, config->core_host, &s_core.sin_addr) != 1) {
        ESP_LOGE(TAG, "core host %s is not an IPv4 literal", config->core_host);
        return -1;
    }

    /* The internal temperature sensor is best-effort: if it cannot be
     * installed the STATUS field reads 0, logged once (F31). */
    temperature_sensor_config_t temperature_config =
        TEMPERATURE_SENSOR_CONFIG_DEFAULT(-10, 80);
    if (temperature_sensor_install(&temperature_config, &s_temperature_sensor) ==
        ESP_OK) {
        if (temperature_sensor_enable(s_temperature_sensor) != ESP_OK) {
            ESP_LOGW(TAG, "temperature sensor could not be enabled");
            s_temperature_sensor = NULL;
        }
    } else {
        ESP_LOGW(TAG, "temperature sensor unavailable; STATUS reports 0");
        s_temperature_sensor = NULL;
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
