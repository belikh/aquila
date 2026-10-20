idf_component_register(
    SRCS "main.c"
    INCLUDE_DIRS "."
    REQUIRES
        csi_node
        node_control
        node_ota
        node_wire
        esp_wifi
        esp_event
        esp_netif
        esp_timer
        nvs_flash
)
