idf_component_register(
    SRCS
        "node_ota.c"
        "ed25519/ed25519.c"
    INCLUDE_DIRS "include"
    PRIV_INCLUDE_DIRS "ed25519"
    REQUIRES
        node_wire
        nvs_flash
        esp_timer
        freertos
        app_update
        esp_partition
        esp_rom
        mbedtls   # SHA-256 over the image digest
)
