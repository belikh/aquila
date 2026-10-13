idf_component_register(
    SRCS "node_control.c"
    INCLUDE_DIRS "include"
    REQUIRES
        csi_node
        node_wire
        esp_wifi
        esp_timer
        nvs_flash
        freertos
        lwip
)
