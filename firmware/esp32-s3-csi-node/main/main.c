/*
 * main.c — the Aquila ESP32-S3 CSI node entry point.
 *
 * Loads the node's provisioning (network credentials, core endpoint, node
 * identity) from NVS, joins the home wireless network, enables promiscuous
 * reception so ambient frames not addressed to the node still produce CSI, then
 * starts the CSI data plane and the node control channel. It confers no Aquila
 * role on the node and performs no DSP, inference or clock discipline
 * (docs/spec/capture-and-firmware.md §9).
 *
 * Credentials live in NVS, written by `aquila-capture provision`
 * (capture-and-firmware.md §8.3, §9.1.8, G13). The Kconfig values in
 * `Kconfig.projbuild` are factory defaults used only when a key is absent from
 * NVS; the WiFi password is never required to be baked into the image (F1/F23).
 *
 * Compile-verified with ESP-IDF v5.5.2 (`nix develop .#esp`); not flashed or
 * run on hardware (see the firmware README).
 */

#include <string.h>

#include "esp_event.h"
#include "esp_log.h"
#include "esp_netif.h"
#include "esp_random.h"
#include "esp_wifi.h"
#include "freertos/FreeRTOS.h"
#include "freertos/event_groups.h"
#include "freertos/task.h"
#include "nvs.h"
#include "nvs_flash.h"

#include "csi_node.h"
#include "node_control.h"
#include "node_ota.h"

static const char *TAG = "aquila_node";

#define WIFI_CONNECTED_BIT BIT0
#define NVS_NAMESPACE "aquila"

static EventGroupHandle_t s_wifi_events;

/* Parse an `aa:bb:cc:dd:ee:ff` MAC address into six octets. */
static bool parse_bssid(const char *text, uint8_t out[6])
{
    if (strlen(text) != 17u) {
        return false;
    }
    for (int index = 0; index < 6; ++index) {
        int high = 0;
        int low = 0;
        char a = text[index * 3];
        char b = text[index * 3 + 1];
        if (a >= '0' && a <= '9') { high = a - '0'; }
        else if (a >= 'a' && a <= 'f') { high = a - 'a' + 10; }
        else if (a >= 'A' && a <= 'F') { high = a - 'A' + 10; }
        else { return false; }
        if (b >= '0' && b <= '9') { low = b - '0'; }
        else if (b >= 'a' && b <= 'f') { low = b - 'a' + 10; }
        else if (b >= 'A' && b <= 'F') { low = b - 'A' + 10; }
        else { return false; }
        if (index > 0 && text[index * 3 - 1] != ':') {
            return false;
        }
        out[index] = (uint8_t)((high << 4) | low);
    }
    return true;
}

/* The node's provisioning, loaded from NVS over the Kconfig factory defaults. */
typedef struct {
    char ssid[33];
    char password[65];
    char core_host[16];
    uint16_t data_port;
    uint16_t control_port;
    uint16_t node_id;
    uint32_t pairing_code;
    /* Optional AP pinning: the BSSID and channel of the one access point the
     * node must join. Pinning is what keeps a mesh of nodes on ONE channel:
     * without it each node joins whichever AP it hears strongest, so two
     * nodes can end up on different channels and never see the same ambient
     * packets — which is what the cross-node phase measurement needs. */
    uint8_t ap_bssid[6];
    bool ap_bssid_set;
    uint8_t ap_channel;
    /* Manual CSI scaling: the automatic scale leaves the samples in a
     * handful of int8 levels, which costs phase precision. `csi_shift` is
     * the number of bits the scale is shifted by; 255 means automatic. */
    uint8_t csi_shift;
} provisioning_t;

static void wifi_event_handler(void *arg, esp_event_base_t base, int32_t id,
                               void *data)
{
    (void)arg;
    (void)data;
    if (base == WIFI_EVENT && id == WIFI_EVENT_STA_START) {
        esp_wifi_connect();
    } else if (base == WIFI_EVENT && id == WIFI_EVENT_STA_DISCONNECTED) {
        xEventGroupClearBits(s_wifi_events, WIFI_CONNECTED_BIT);
        ESP_LOGW(TAG, "disconnected; reconnecting");
        esp_wifi_connect();
    } else if (base == IP_EVENT && id == IP_EVENT_STA_GOT_IP) {
        xEventGroupSetBits(s_wifi_events, WIFI_CONNECTED_BIT);
    }
}

/*
 * Load provisioning from NVS, falling back to the Kconfig factory defaults for
 * any key the operator has not provisioned. A value read from NVS always wins.
 */
static void load_provisioning(provisioning_t *out)
{
    strncpy(out->ssid, CONFIG_AQUILA_WIFI_SSID, sizeof(out->ssid) - 1);
    out->ssid[sizeof(out->ssid) - 1] = '\0';
    strncpy(out->password, CONFIG_AQUILA_WIFI_PASSWORD, sizeof(out->password) - 1);
    out->password[sizeof(out->password) - 1] = '\0';
    strncpy(out->core_host, CONFIG_AQUILA_CORE_HOST, sizeof(out->core_host) - 1);
    out->core_host[sizeof(out->core_host) - 1] = '\0';
    out->data_port = (uint16_t)CONFIG_AQUILA_CORE_DATA_PORT;
    out->control_port = (uint16_t)CONFIG_AQUILA_CORE_CONTROL_PORT;
    out->node_id = (uint16_t)CONFIG_AQUILA_NODE_ID;
    out->pairing_code = (uint32_t)CONFIG_AQUILA_PAIRING_CODE;
    /* Every field must have a defined value before the NVS reads: the
     * struct is not zero-initialised by the caller, so a field left
     * unset would carry stack garbage. */
    memset(out->ap_bssid, 0, sizeof(out->ap_bssid));
    out->ap_bssid_set = false;
    out->ap_channel = 0;
    out->csi_shift = 255u; /* 255 means automatic scaling */

    nvs_handle_t handle;
    if (nvs_open(NVS_NAMESPACE, NVS_READONLY, &handle) != ESP_OK) {
        return;
    }
    /* A successful read overwrites the Kconfig default; a failed one leaves it
     * in place, so the factory default is used only when NVS is empty. */
    size_t len = sizeof(out->ssid);
    (void)nvs_get_str(handle, "wifi_ssid", out->ssid, &len);
    len = sizeof(out->password);
    (void)nvs_get_str(handle, "wifi_pass", out->password, &len);
    len = sizeof(out->core_host);
    (void)nvs_get_str(handle, "core_host", out->core_host, &len);
    uint16_t port = 0;
    if (nvs_get_u16(handle, "core_data_port", &port) == ESP_OK && port != 0u) {
        out->data_port = port;
    }
    /* The NVS key is fifteen characters at most, so the control port is
     * stored under a name that fits; "core_control_port" does not. */
    if (nvs_get_u16(handle, "ctrl_port", &port) == ESP_OK && port != 0u) {
        out->control_port = port;
    }
    if (nvs_get_u16(handle, "node_id", &port) == ESP_OK && port != 0u) {
        out->node_id = port;
    }
    uint32_t code = 0;
    if (nvs_get_u32(handle, "pairing_code", &code) == ESP_OK) {
        out->pairing_code = code;
    }
    /* The pinned access point, when the operator provisioned one. */
    char bssid_text[18];
    len = sizeof(bssid_text);
    if (nvs_get_str(handle, "ap_bssid", bssid_text, &len) == ESP_OK) {
        if (parse_bssid(bssid_text, out->ap_bssid)) {
            out->ap_bssid_set = true;
        } else {
            ESP_LOGW(TAG, "ap_bssid `%s` is not a MAC address; ignoring",
                     bssid_text);
        }
    }
    uint8_t channel = 0;
    if (nvs_get_u8(handle, "ap_channel", &channel) == ESP_OK) {
        out->ap_channel = channel;
    }
    uint8_t shift = 255u; /* 255 means automatic scaling */
    if (nvs_get_u8(handle, "csi_shift", &shift) == ESP_OK) {
        out->csi_shift = shift;
    }
    nvs_close(handle);
}

static void wifi_start(const provisioning_t *provisioning)
{
    s_wifi_events = xEventGroupCreate();

    ESP_ERROR_CHECK(esp_netif_init());
    ESP_ERROR_CHECK(esp_event_loop_create_default());
    esp_netif_create_default_wifi_sta();

    wifi_init_config_t init = WIFI_INIT_CONFIG_DEFAULT();
    ESP_ERROR_CHECK(esp_wifi_init(&init));

    ESP_ERROR_CHECK(esp_event_handler_register(WIFI_EVENT, ESP_EVENT_ANY_ID,
                                               &wifi_event_handler, NULL));
    ESP_ERROR_CHECK(esp_event_handler_register(IP_EVENT, IP_EVENT_STA_GOT_IP,
                                               &wifi_event_handler, NULL));

    wifi_config_t wifi_config = {0};
    strncpy((char *)wifi_config.sta.ssid, provisioning->ssid,
            sizeof(wifi_config.sta.ssid) - 1);
    strncpy((char *)wifi_config.sta.password, provisioning->password,
            sizeof(wifi_config.sta.password) - 1);
    if (provisioning->ap_bssid_set) {
        /* Pin the node to one access point, and hint its channel: this is
         * what keeps every node of the mesh on the same channel, so they all
         * see the same ambient packets. */
        wifi_config.sta.bssid_set = 1;
        memcpy(wifi_config.sta.bssid, provisioning->ap_bssid, 6);
        wifi_config.sta.channel = provisioning->ap_channel;
        ESP_LOGI(TAG, "pinned to AP %02x:%02x:%02x:%02x:%02x:%02x on channel %u",
                 provisioning->ap_bssid[0], provisioning->ap_bssid[1],
                 provisioning->ap_bssid[2], provisioning->ap_bssid[3],
                 provisioning->ap_bssid[4], provisioning->ap_bssid[5],
                 (unsigned)provisioning->ap_channel);
    }

    ESP_ERROR_CHECK(esp_wifi_set_mode(WIFI_MODE_STA));
    ESP_ERROR_CHECK(esp_wifi_set_config(WIFI_IF_STA, &wifi_config));
    ESP_ERROR_CHECK(esp_wifi_start());

    /* Promiscuous reception so ambient frames not addressed to the node still
     * elicit CSI (§9.1.2). */
    ESP_ERROR_CHECK(esp_wifi_set_promiscuous(true));

    xEventGroupWaitBits(s_wifi_events, WIFI_CONNECTED_BIT, pdFALSE, pdTRUE,
                        portMAX_DELAY);
    ESP_LOGI(TAG, "associated with %s", provisioning->ssid);
}

/*
 * The OTA pending-verify health probe (firmware-ota.md §6): the new image is
 * healthy only while the node is associated, streaming CSI and the control
 * channel is responsive. Composed here because it spans the three subsystems.
 */
static bool node_ota_health(void *ctx)
{
    (void)ctx;
    if ((xEventGroupGetBits(s_wifi_events) & WIFI_CONNECTED_BIT) == 0) {
        return false;
    }
    if (!csi_node_is_streaming()) {
        return false;
    }
    return node_control_is_responsive();
}

/*
 * Dump the ESP32-S3's ROM PHY function table once at boot.
 *
 * The ROM is the chip's mask-programmed firmware — the WiFi MAC, the PHY and
 * the baseband, including the channel estimator that produces the CSI. Its
 * PHY function table is the list of hardware controls the chip exposes, and
 * it is the surface the application reaches the ROM through. The table lives
 * in the ROM's RAM and is filled at boot, so it cannot be read out of the
 * published ELF; the running node is the only place it exists.
 *
 * The dump is the reverse-engineering record for the ROM's interface. It is
 * off by default: it prints a table of addresses that is of no use to the
 * node's job.
 */
#define AQUILA_DUMP_ROM_PHY_TABLE 0

/*
 * The ROM's IQ/DC estimate switch.
 *
 * The ESP32-S3's receiver estimates its DC offset and I/Q imbalance. The ROM
 * exposes the switch as `rom_iq_est_disable`, at table offset 0xf4, and the
 * application can reach it through the ROM's own PHY function table.
 *
 * Measured on the node: the register's bits 0 and 1 are already clear before
 * the call, so the switch is a no-op in this configuration and the flip is
 * present with it in that state. The switch does not gate the flip.
 */
#define AQUILA_DISABLE_IQ_EST 0
#define ROM_PHY_IQ_EST_DISABLE_OFFSET 0xf4

/* The driver's copy of the ROM's PHY function table, filled at boot. */
extern void *g_phyFuns;

/* The ROM's PHY table is a block of function pointers; this is its size. */
#define ROM_PHY_TABLE_BYTES 0x300

static void dump_rom_phy_table(void)
{
    void *table = g_phyFuns;
    if (table == NULL) {
        ESP_LOGW(TAG, "the ROM PHY table is not available");
        return;
    }
    ESP_LOGI(TAG, "ROMPHY base=%p", table);
    const uint32_t *words = (const uint32_t *)table;
    for (int offset = 0; offset < ROM_PHY_TABLE_BYTES; offset += 4) {
        uint32_t value = words[offset / 4];
        if (value != 0u) {
            ESP_LOGI(TAG, "ROMPHY %03x %08x", (unsigned)offset, (unsigned)value);
        }
    }
}

void app_main(void)
{
    esp_err_t err = nvs_flash_init();
    if (err == ESP_ERR_NVS_NO_FREE_PAGES || err == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        ESP_ERROR_CHECK(nvs_flash_erase());
        ESP_ERROR_CHECK(nvs_flash_init());
    }

    provisioning_t provisioning;
    load_provisioning(&provisioning);

    wifi_start(&provisioning);

#if AQUILA_DUMP_ROM_PHY_TABLE
    dump_rom_phy_table();
#endif

#if AQUILA_DISABLE_IQ_EST
    {
        /* The register the ROM's `rom_iq_est_disable` clears bits in. It is
         * read before and after the call so that the test's validity can be
         * checked rather than assumed: if the bits were already clear, the
         * call was a no-op and the result means nothing. */
        volatile uint32_t *iq_est_reg = (volatile uint32_t *)0x60006144u;
        uint32_t before = *iq_est_reg;
        void (**table)(void) = (void (**)(void))g_phyFuns;
        void (*disable)(void) = table[ROM_PHY_IQ_EST_DISABLE_OFFSET / 4u];
        if (disable != NULL) {
            disable();
            ESP_LOGI(TAG, "IQ estimation disabled through the ROM PHY table");
        } else {
            ESP_LOGE(TAG, "the ROM PHY table has no IQ-estimate switch");
        }
        uint32_t after = *iq_est_reg;
        ESP_LOGI(TAG, "IQ est register 0x%08x -> 0x%08x (bits 0-1: %u -> %u)",
                 (unsigned)before, (unsigned)after,
                 (unsigned)(before & 0x3u), (unsigned)(after & 0x3u));
    }
#endif

    csi_node_config_t csi = csi_node_default_config();
    csi.node_id = provisioning.node_id;
    csi.boot = (uint16_t)esp_random();
    csi.channel = 6u;
    csi.bandwidth_mhz = 20u;
    csi.csi_enable = true;
    csi.max_tx_rate_hz = 200u;
    csi.heartbeat_ms = 5000u;
    csi.csi_shift = provisioning.csi_shift;

    if (csi_node_start(&csi, provisioning.core_host, provisioning.data_port) != 0) {
        ESP_LOGE(TAG, "CSI data plane failed to start");
        return;
    }

    /*
     * The firmware OTA agent. Started before the control channel so the
     * dispatch in node_control can never reach an uninitialised agent; it owns
     * no socket and replies through the control channel's send path.
     */
    node_ota_config_t ota = {
        .node_id = csi.node_id,
        .send = node_control_send_reply,
        .health = node_ota_health,
        .health_ctx = NULL,
        .health_window_ms = 0u, /* use CONFIG_AQUILA_OTA_HEALTH_WINDOW_S */
    };
    if (node_ota_start(&ota) != 0) {
        ESP_LOGE(TAG, "OTA agent failed to start");
        return;
    }

    node_control_config_t control = {
        .node_id = csi.node_id,
        .boot = csi.boot,
        .core_host = provisioning.core_host,
        .core_port = provisioning.control_port,
        .pairing_code = provisioning.pairing_code,
        .fw_version = "0.0.0",
        .csi = csi,
    };
    if (node_control_start(&control) != 0) {
        ESP_LOGE(TAG, "control channel failed to start");
        return;
    }

    /* The capture, sender and control tasks do the work; idle here. */
    for (;;) {
        vTaskDelay(pdMS_TO_TICKS(1000));
    }
}
