/*
 * main.c — the Aquila ESP32-S3 CSI node entry point.
 *
 * Joins the home wireless network, enables promiscuous reception so ambient
 * frames not addressed to the node still produce CSI, then starts the CSI data
 * plane and the node control channel. It confers no Aquila role on the node and
 * performs no DSP, inference or clock discipline
 * (docs/spec/capture-and-firmware.md §9).
 *
 * Reference implementation; NOT compiled, flashed or tested in this repository
 * (see the firmware README).
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
#include "nvs_flash.h"

#include "csi_node.h"
#include "node_control.h"

static const char *TAG = "aquila_node";

#define WIFI_CONNECTED_BIT BIT0

static EventGroupHandle_t s_wifi_events;

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

static void wifi_start(void)
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
    strncpy((char *)wifi_config.sta.ssid, CONFIG_AQUILA_WIFI_SSID,
            sizeof(wifi_config.sta.ssid) - 1);
    strncpy((char *)wifi_config.sta.password, CONFIG_AQUILA_WIFI_PASSWORD,
            sizeof(wifi_config.sta.password) - 1);

    ESP_ERROR_CHECK(esp_wifi_set_mode(WIFI_MODE_STA));
    ESP_ERROR_CHECK(esp_wifi_set_config(WIFI_IF_STA, &wifi_config));
    ESP_ERROR_CHECK(esp_wifi_start());

    /* Promiscuous reception so ambient frames not addressed to the node still
     * elicit CSI (§9.1.2). */
    ESP_ERROR_CHECK(esp_wifi_set_promiscuous(true));

    xEventGroupWaitBits(s_wifi_events, WIFI_CONNECTED_BIT, pdFALSE, pdTRUE,
                        portMAX_DELAY);
    ESP_LOGI(TAG, "associated with %s", CONFIG_AQUILA_WIFI_SSID);
}

void app_main(void)
{
    esp_err_t err = nvs_flash_init();
    if (err == ESP_ERR_NVS_NO_FREE_PAGES || err == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        ESP_ERROR_CHECK(nvs_flash_erase());
        ESP_ERROR_CHECK(nvs_flash_init());
    }

    wifi_start();

    csi_node_config_t csi = csi_node_default_config();
    csi.node_id = (uint16_t)CONFIG_AQUILA_NODE_ID;
    csi.boot = (uint16_t)esp_random();
    csi.channel = 6u;
    csi.bandwidth_mhz = 20u;
    csi.csi_enable = true;
    csi.max_tx_rate_hz = 200u;

    if (csi_node_start(&csi, CONFIG_AQUILA_CORE_HOST,
                       (uint16_t)CONFIG_AQUILA_CORE_DATA_PORT) != 0) {
        ESP_LOGE(TAG, "CSI data plane failed to start");
        return;
    }

    node_control_config_t control = {
        .node_id = csi.node_id,
        .boot = csi.boot,
        .core_host = CONFIG_AQUILA_CORE_HOST,
        .core_port = (uint16_t)CONFIG_AQUILA_CORE_CONTROL_PORT,
        .pairing_code = (uint32_t)CONFIG_AQUILA_PAIRING_CODE,
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
