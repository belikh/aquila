/*
 * csi_node.c — ESP32-S3 CSI capture and the CSI data plane.
 *
 * Capture is RX-event-driven and passive (docs/spec/capture-and-firmware.md
 * §5.3): a CSI frame exists only when the radio receives an 802.11 frame. The
 * receive callback copies the raw CSI buffer and metadata into a fixed-capacity
 * single-producer/single-consumer ring; a sender task drains the ring to the
 * core, one AQCI datagram per frame, subject to the configured transmit-rate
 * cap. The callback never allocates, never blocks and never uses floating point
 * (§9.1.3). The payload is the raw signed int8 I/Q buffer, copied verbatim
 * (ADR-0005); the node performs no amplitude/phase conversion.
 *
 * Compile-verified with ESP-IDF v5.5.2 (`nix develop .#esp`); not flashed or
 * run on hardware (see the firmware README).
 */

#include "csi_node.h"

#include <stdatomic.h>
#include <string.h>

#include "esp_log.h"
#include "esp_timer.h"
#include "esp_wifi.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "lwip/inet.h"
#include "lwip/sockets.h"

#include "node_wire.h"
#include "node_wire_util.h"

static const char *TAG = "csi_node";

/* The ring holds this many captured frames before overflow drops occur. */
#define CSI_RING_SLOTS 32u
/* The raw int8 CSI is at most two bytes per subcarrier. */
#define CSI_RAW_MAX ((size_t)AQ_CSI_MAX_SUBCARRIERS * 2u)
/* The sender task stack and priority. */
#define CSI_SENDER_STACK 4096u
#define CSI_SENDER_PRIO 5u

typedef struct {
    uint32_t sequence;
    uint64_t device_ts_us;
    uint8_t band;
    uint8_t channel;
    int8_t rssi;
    int8_t noise_floor;
    uint16_t phy_flags;
    bool first_word_invalid;
    uint16_t csi_len;
    int8_t csi[CSI_RAW_MAX];
} csi_slot_t;

static csi_slot_t s_ring[CSI_RING_SLOTS];
static _Atomic uint32_t s_head;    /* consumer index */
static _Atomic uint32_t s_tail;    /* producer index */
static _Atomic uint32_t s_dropped;
static _Atomic uint64_t s_sequence;
static _Atomic uint64_t s_last_sequence;
static _Atomic bool s_streaming;

static portMUX_TYPE s_cfg_mux = portMUX_INITIALIZER_UNLOCKED;
static csi_node_config_t s_config;
static uint16_t s_raw_cap = (uint16_t)CSI_RAW_MAX;
static uint16_t s_boot_epoch;

static int s_sock = -1;
static struct sockaddr_in s_core;
static int64_t s_last_send_us;

/* The 2.4 GHz band byte for channels 1..14, otherwise the 5 GHz band byte. */
static uint8_t band_from_channel(uint8_t channel)
{
    return channel <= 14u ? 0u : 1u;
}

/* Map the receiver's sig_mode to the Aquila PHY flag bits. */
static uint16_t phy_flags_from_rx(const wifi_pkt_rx_ctrl_t *rx)
{
    uint16_t flags = 0u;
    if (rx->sig_mode >= 1u) { /* 1 HT, 2 VHT, 3 HE all imply 11n-or-later */
        flags |= 1u << 0;     /* HT */
    }
    if (rx->cwb != 0u) {
        flags |= 1u << 1; /* secondary present */
    }
    if (rx->stbc != 0u) {
        flags |= 1u << 2; /* STBC */
    }
    if (rx->fec_coding != 0u) {
        flags |= 1u << 3; /* LDPC */
    }
    if (rx->sgi != 0u) {
        flags |= 1u << 4; /* short GI */
    }
    return flags;
}

/*
 * The CSI receive callback: copy and enqueue. No allocation, no logging, no
 * floating point, no blocking; on a full ring the frame is dropped and counted
 * (§5.4).
 */
static void csi_rx_cb(void *ctx, wifi_csi_info_t *info)
{
    (void)ctx;
    if (!atomic_load_explicit(&s_streaming, memory_order_relaxed)) {
        return;
    }
    if (info == NULL || info->buf == NULL) {
        return;
    }
    int len = info->len;
    if (len <= 0 || (uint16_t)len > s_raw_cap) {
        atomic_fetch_add_explicit(&s_dropped, 1u, memory_order_relaxed);
        return;
    }

    uint32_t tail = atomic_load_explicit(&s_tail, memory_order_relaxed);
    uint32_t next = (tail + 1u) % CSI_RING_SLOTS;
    uint32_t head = atomic_load_explicit(&s_head, memory_order_acquire);
    if (next == head) {
        atomic_fetch_add_explicit(&s_dropped, 1u, memory_order_relaxed);
        return;
    }

    csi_slot_t *slot = &s_ring[tail];
    uint64_t sequence = atomic_fetch_add_explicit(&s_sequence, 1u, memory_order_relaxed);

    slot->sequence = (uint32_t)sequence;
    slot->device_ts_us = (uint64_t)esp_timer_get_time();
    slot->band = band_from_channel(info->rx_ctrl.channel);
    slot->channel = info->rx_ctrl.channel;
    slot->rssi = info->rx_ctrl.rssi;
    slot->noise_floor = info->rx_ctrl.noise_floor;
    slot->phy_flags = phy_flags_from_rx(&info->rx_ctrl);
    slot->first_word_invalid = info->first_word_invalid;
    slot->csi_len = (uint16_t)len;
    memcpy(slot->csi, info->buf, (size_t)len);

    atomic_store_explicit(&s_last_sequence, sequence, memory_order_release);
    atomic_store_explicit(&s_tail, next, memory_order_release);
}

/* Snapshot the configuration under the short spinlock. */
static csi_node_config_t snapshot_config(void)
{
    portENTER_CRITICAL(&s_cfg_mux);
    csi_node_config_t cfg = s_config;
    portEXIT_CRITICAL(&s_cfg_mux);
    return cfg;
}

/* Build and apply the esp-csi configuration from `cfg`. */
static void apply_esp_csi(const csi_node_config_t *cfg)
{
    wifi_csi_config_t csi_config = {
        .lltf_en = cfg->lltf_enable,
        .htltf_en = cfg->ht_ltf_enable,
        .stbc_htltf2_en = cfg->stbc_ht_ltf_enable,
        .ltf_merge_en = true,
        .channel_filter_en = false,
        .manu_scale = false,
        .shift = false,
    };
    esp_wifi_set_csi_config(&csi_config);
}

/*
 * The sender task: drain the ring, frame the datagram with the raw signed int8
 * I/Q buffer copied verbatim (ADR-0005) and unicast it to the core.
 */
static void csi_sender_task(void *arg)
{
    (void)arg;
    static uint8_t datagram[AQ_CSI_MAX_DATAGRAM_LEN];
    csi_slot_t slot;

    for (;;) {
        uint32_t head = atomic_load_explicit(&s_head, memory_order_acquire);
        uint32_t tail = atomic_load_explicit(&s_tail, memory_order_acquire);
        if (head == tail) {
            vTaskDelay(pdMS_TO_TICKS(1));
            continue;
        }
        memcpy(&slot, &s_ring[head], sizeof(slot));
        atomic_store_explicit(&s_head, (head + 1u) % CSI_RING_SLOTS,
                              memory_order_release);

        if (!atomic_load_explicit(&s_streaming, memory_order_relaxed)) {
            continue;
        }

        csi_node_config_t cfg = snapshot_config();

        /* Rate cap: a frame over the cap is dropped, never queued (§5.3). */
        int64_t now_us = esp_timer_get_time();
        if (cfg.max_tx_rate_hz > 0u) {
            int64_t min_interval_us = 1000000 / (int64_t)cfg.max_tx_rate_hz;
            if (now_us - s_last_send_us < min_interval_us) {
                atomic_fetch_add_explicit(&s_dropped, 1u, memory_order_relaxed);
                continue;
            }
        }
        s_last_send_us = now_us;

        uint16_t n = (uint16_t)(slot.csi_len / 2u);
        if (n == 0u) {
            continue;
        }
        if (n > AQ_CSI_MAX_SUBCARRIERS) {
            n = AQ_CSI_MAX_SUBCARRIERS;
        }
        /* The leading complex sample (two bytes) is unreliable when the driver
         * says so; null it and mark the header flags byte, exactly as the
         * simulator and the core decoder do (capture-and-firmware.md §7.4). */
        if (slot.first_word_invalid) {
            slot.csi[0] = 0;
            slot.csi[1] = 0;
        }

        int16_t subcarrier_offset = (int16_t)(-(int16_t)(n / 2u));
        size_t length = aq_frame_csi(
            cfg.node_id, slot.band, slot.channel, cfg.bandwidth_mhz, slot.sequence,
            slot.device_ts_us, s_boot_epoch, subcarrier_offset, n, slot.rssi,
            slot.noise_floor, slot.phy_flags, slot.first_word_invalid, slot.csi,
            datagram, sizeof(datagram));
        if (length > 0u && s_sock >= 0) {
            sendto(s_sock, datagram, length, 0, (struct sockaddr *)&s_core,
                   sizeof(s_core));
        }
    }
}

csi_node_config_t csi_node_default_config(void)
{
    csi_node_config_t cfg = {
        .node_id = 1u,
        .boot = 0u,
        .channel = 6u,
        .bandwidth_mhz = 20u,
        .secondary = 0u,
        .csi_enable = true,
        .lltf_enable = true,
        .ht_ltf_enable = true,
        .stbc_ht_ltf_enable = false,
        .csi_len_cap = (uint16_t)CSI_RAW_MAX,
        .max_tx_rate_hz = 200u,
        .heartbeat_ms = 5000u,
    };
    return cfg;
}

int csi_node_start(const csi_node_config_t *config, const char *core_host,
                   uint16_t core_port)
{
    if (config == NULL || core_host == NULL) {
        return -1;
    }
    s_boot_epoch = config->boot;
    s_config = *config;
    s_raw_cap = config->csi_len_cap < CSI_RAW_MAX ? config->csi_len_cap
                                                  : (uint16_t)CSI_RAW_MAX;
    atomic_store(&s_streaming, config->csi_enable);

    memset(&s_core, 0, sizeof(s_core));
    s_core.sin_family = AF_INET;
    s_core.sin_port = htons(core_port);
    if (inet_pton(AF_INET, core_host, &s_core.sin_addr) != 1) {
        ESP_LOGE(TAG, "core host %s is not an IPv4 literal", core_host);
        return -1;
    }
    s_sock = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if (s_sock < 0) {
        ESP_LOGE(TAG, "could not open the CSI data socket");
        return -1;
    }

    apply_esp_csi(config);
    esp_wifi_set_csi_rx_cb(csi_rx_cb, NULL);
    esp_wifi_set_csi(true);

    if (xTaskCreate(csi_sender_task, "csi_sender", CSI_SENDER_STACK, NULL,
                    CSI_SENDER_PRIO, NULL) != pdPASS) {
        ESP_LOGE(TAG, "could not start the sender task");
        return -1;
    }
    ESP_LOGI(TAG, "CSI node %u streaming to %s:%u", config->node_id, core_host,
             (unsigned)core_port);
    return 0;
}

void csi_node_apply_config(const csi_node_config_t *config)
{
    if (config == NULL) {
        return;
    }
    portENTER_CRITICAL(&s_cfg_mux);
    s_config.channel = config->channel;
    s_config.bandwidth_mhz = config->bandwidth_mhz;
    s_config.secondary = config->secondary;
    s_config.csi_enable = config->csi_enable;
    s_config.lltf_enable = config->lltf_enable;
    s_config.ht_ltf_enable = config->ht_ltf_enable;
    s_config.stbc_ht_ltf_enable = config->stbc_ht_ltf_enable;
    s_config.csi_len_cap = config->csi_len_cap;
    s_config.max_tx_rate_hz = config->max_tx_rate_hz;
    s_config.heartbeat_ms = config->heartbeat_ms;
    portEXIT_CRITICAL(&s_cfg_mux);

    s_raw_cap = config->csi_len_cap < CSI_RAW_MAX ? config->csi_len_cap
                                                  : (uint16_t)CSI_RAW_MAX;
    atomic_store(&s_streaming, config->csi_enable);
    apply_esp_csi(config);
    if (config->channel != 0u) {
        /* `secondary` (0 none, 1 above, 2 below) selects the 40 MHz offset;
         * anything unrecognised falls back to no secondary channel (F5). */
        wifi_second_chan_t second = WIFI_SECOND_CHAN_NONE;
        if (config->bandwidth_mhz > 20u) {
            if (config->secondary == 1u) {
                second = WIFI_SECOND_CHAN_ABOVE;
            } else if (config->secondary == 2u) {
                second = WIFI_SECOND_CHAN_BELOW;
            }
        }
        esp_wifi_set_channel(config->channel, second);
    }
}

void csi_node_set_streaming(bool enable)
{
    atomic_store(&s_streaming, enable);
}

uint16_t csi_node_phy_cap(void)
{
    return s_raw_cap;
}

uint64_t csi_node_last_sequence(void)
{
    return atomic_load(&s_last_sequence);
}

uint32_t csi_node_dropped(void)
{
    return atomic_load(&s_dropped);
}

uint16_t csi_node_ring_depth(void)
{
    uint32_t head = atomic_load(&s_head);
    uint32_t tail = atomic_load(&s_tail);
    return (uint16_t)((tail + CSI_RING_SLOTS - head) % CSI_RING_SLOTS);
}
