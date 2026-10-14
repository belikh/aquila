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

static const char *TAG = "aquila_node";

#define WIFI_CONNECTED_BIT BIT0
#define NVS_NAMESPACE "aquila"

static EventGroupHandle_t s_wifi_events;

/* The node's provisioning, loaded from NVS over the Kconfig factory defaults. */
typedef struct {
    char ssid[33];
    char password[65];
    char core_host[16];
    uint16_t data_port;
    uint16_t control_port;
    uint16_t node_id;
    uint32_t pairing_code;
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
    if (nvs_get_u16(handle, "core_control_port", &port) == ESP_OK && port != 0u) {
        out->control_port = port;
    }
    if (nvs_get_u16(handle, "node_id", &port) == ESP_OK && port != 0u) {
        out->node_id = port;
    }
    uint32_t code = 0;
    if (nvs_get_u32(handle, "pairing_code", &code) == ESP_OK) {
        out->pairing_code = code;
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

    csi_node_config_t csi = csi_node_default_config();
    csi.node_id = provisioning.node_id;
    csi.boot = (uint16_t)esp_random();
    csi.channel = 6u;
    csi.bandwidth_mhz = 20u;
    csi.csi_enable = true;
    csi.max_tx_rate_hz = 200u;
    csi.heartbeat_ms = 5000u;

    if (csi_node_start(&csi, provisioning.core_host, provisioning.data_port) != 0) {
        ESP_LOGE(TAG, "CSI data plane failed to start");
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
