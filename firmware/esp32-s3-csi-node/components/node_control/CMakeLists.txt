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
    PRIV_REQUIRES
        mbedtls   # HMAC-SHA256 control-message authentication (F21)
        driver    # internal temperature sensor (F31) and IDENTIFY GPIO (F33)
)
