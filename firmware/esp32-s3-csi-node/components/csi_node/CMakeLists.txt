idf_component_register(
    SRCS "csi_node.c"
    INCLUDE_DIRS "include"
    REQUIRES
        node_wire
        esp_wifi
        esp_timer
        freertos
        lwip
)
