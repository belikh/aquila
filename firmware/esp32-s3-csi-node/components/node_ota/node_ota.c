/*
 * node_ota.c — the node-side firmware OTA agent (docs/spec/firmware-ota.md §5,
 * §6, §8; capture-and-firmware.md §6.2).
 *
 * One transfer at a time. Chunks are written sequentially into the inactive A/B
 * partition; each chunk is checked against its CRC-32, and the whole image
 * against its SHA-256 and an Ed25519 signature over that digest (the trust
 * anchor is provisioned in NVS or compiled in). Only a fully verified image is
 * made the next boot target; the following boot is a pending-verify health
 * window that either confirms the image or lets the bootloader roll back. An
 * anti-rollback security-version floor rejects downgrades.
 *
 * All message handling runs on the control task (single-threaded); the health
 * task shares only atomics and NVS. Session state is guarded by a mutex so the
 * health task's post-activation bookkeeping cannot race a live transfer.
 *
 * Compile-verified with ESP-IDF v5.5.2 (`nix develop .#esp`); not flashed or
 * run on hardware (see the firmware README).
 */

#include "node_ota.h"

#include <stdatomic.h>
#include <stdio.h>
#include <string.h>

#include "esp_log.h"
#include "esp_ota_ops.h"
#include "esp_partition.h"
#include "esp_rom_crc.h"
#include "esp_system.h"
#include "esp_timer.h"
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"
#include "freertos/task.h"
#include "mbedtls/sha256.h"
#include "nvs.h"
#include "nvs_flash.h"

#include "ed25519.h"
#include "node_wire.h"
#include "node_wire_util.h"

static const char *TAG = "node_ota";

#define OTA_NVS_NAMESPACE "ota"
#define OTA_DEFAULT_WINDOW_MS ((uint32_t)CONFIG_AQUILA_OTA_HEALTH_WINDOW_S * 1000u)
#define OTA_HEALTH_STACK 4096u
#define OTA_HEALTH_PRIO 4u
#define OTA_VERSION_MAX 32u

/* The signed image trust anchor: a 32-byte Ed25519 public key. */
static uint8_t s_pubkey[32];
static bool s_have_pubkey;

/* The anti-rollback floor: the stored value and the compiled-in minimum. */
static uint32_t s_stored_secver;
static uint32_t s_compiled_secver = (uint32_t)CONFIG_AQUILA_FW_SECURITY_VERSION;

static node_ota_config_t s_config;
static SemaphoreHandle_t s_mutex;

static atomic_bool s_pending_verify;
static atomic_bool s_activated_pending;

/* The single in-flight transfer. */
typedef struct {
    bool active;
    uint32_t image_id;
    uint32_t size;
    uint32_t received;
    uint16_t chunk_size;
    uint8_t window;
    uint32_t security_version;
    char version[OTA_VERSION_MAX + 1u];
    uint8_t sha256[32];
    uint8_t signature[64];
    const esp_partition_t *partition;
    esp_ota_handle_t handle;
    bool handle_open;
    mbedtls_sha256_context digest;
    bool digest_started;
} ota_session_t;

static ota_session_t s_session;
static atomic_uint s_state; /* OtaState value, for OTA_STATUS */

/* ------------------------------------------------------------------ */
/* NVS helpers                                                         */
/* ------------------------------------------------------------------ */

static void load_trust_anchor(void)
{
    memset(s_pubkey, 0, sizeof(s_pubkey));
    s_have_pubkey = false;

    nvs_handle_t handle;
    if (nvs_open(OTA_NVS_NAMESPACE, NVS_READONLY, &handle) == ESP_OK) {
        size_t len = sizeof(s_pubkey);
        if (nvs_get_blob(handle, "fw_pubkey", s_pubkey, &len) == ESP_OK &&
            len == sizeof(s_pubkey)) {
            s_have_pubkey = true;
        }
        nvs_close(handle);
    }
    if (s_have_pubkey) {
        return;
    }

    /* Fall back to the compiled-in hex key (a 64-character hex string). */
    const char *hex = CONFIG_AQUILA_FW_PUBKEY_HEX;
    if (strlen(hex) == 64u) {
        bool ok = true;
        for (size_t i = 0; i < 32u; ++i) {
            int hi = 0;
            int lo = 0;
            char a = hex[2 * i];
            char b = hex[2 * i + 1];
            if (a >= '0' && a <= '9') hi = a - '0';
            else if (a >= 'a' && a <= 'f') hi = a - 'a' + 10;
            else if (a >= 'A' && a <= 'F') hi = a - 'A' + 10;
            else ok = false;
            if (b >= '0' && b <= '9') lo = b - '0';
            else if (b >= 'a' && b <= 'f') lo = b - 'a' + 10;
            else if (b >= 'A' && b <= 'F') lo = b - 'A' + 10;
            else ok = false;
            s_pubkey[i] = (uint8_t)((hi << 4) | lo);
        }
        s_have_pubkey = ok;
    }
    if (!s_have_pubkey) {
        ESP_LOGW(TAG, "no firmware trust anchor provisioned; OTA will fail closed");
    }
}

static void load_security_version(void)
{
    s_stored_secver = s_compiled_secver;
    nvs_handle_t handle;
    if (nvs_open(OTA_NVS_NAMESPACE, NVS_READONLY, &handle) != ESP_OK) {
        return;
    }
    uint32_t stored = 0;
    /* The NVS key is fifteen characters at most, so the anti-rollback floor
     * is stored under a name that fits; "security_version" does not. */
    if (nvs_get_u32(handle, "sec_version", &stored) == ESP_OK &&
        stored > s_stored_secver) {
        s_stored_secver = stored;
    }
    nvs_close(handle);
}

static void save_security_version(uint32_t value)
{
    nvs_handle_t handle;
    if (nvs_open(OTA_NVS_NAMESPACE, NVS_READWRITE, &handle) != ESP_OK) {
        return;
    }
    nvs_set_u32(handle, "sec_version", value);
    nvs_commit(handle);
    nvs_close(handle);
}

static void save_pending_flag(bool pending)
{
    nvs_handle_t handle;
    if (nvs_open(OTA_NVS_NAMESPACE, NVS_READWRITE, &handle) != ESP_OK) {
        return;
    }
    nvs_set_u8(handle, "pending_verify", pending ? 1u : 0u);
    nvs_commit(handle);
    nvs_close(handle);
}

static void save_pending_secver(uint32_t value)
{
    nvs_handle_t handle;
    if (nvs_open(OTA_NVS_NAMESPACE, NVS_READWRITE, &handle) != ESP_OK) {
        return;
    }
    nvs_set_u32(handle, "pending_secver", value);
    nvs_commit(handle);
    nvs_close(handle);
}

static uint32_t load_pending_secver(void)
{
    uint32_t value = 0;
    nvs_handle_t handle;
    if (nvs_open(OTA_NVS_NAMESPACE, NVS_READONLY, &handle) == ESP_OK) {
        (void)nvs_get_u32(handle, "pending_secver", &value);
        nvs_close(handle);
    }
    return value;
}

/* ------------------------------------------------------------------ */
/* Replies                                                             */
/* ------------------------------------------------------------------ */

static void send_ota_ack(uint32_t request_id, uint32_t received_upto,
                         uint8_t status)
{
    uint8_t body[5];
    aq_put_u32_le(&body[0], received_upto);
    body[4] = status;
    if (s_config.send != NULL) {
        s_config.send(AQ_OP_OTA_ACK, request_id, body, sizeof(body));
    }
}

static void send_ota_status(uint32_t request_id, uint8_t state,
                            uint32_t received_upto, uint32_t size)
{
    uint8_t body[9];
    body[0] = state;
    aq_put_u32_le(&body[1], received_upto);
    aq_put_u32_le(&body[5], size);
    if (s_config.send != NULL) {
        s_config.send(AQ_OP_OTA_STATUS, request_id, body, sizeof(body));
    }
}

static void send_ota_result(uint32_t request_id, uint8_t outcome,
                            const char *reason)
{
    size_t reason_len = reason != NULL ? strlen(reason) : 0u;
    if (reason_len > 255u) {
        reason_len = 255u;
    }
    uint8_t body[257];
    body[0] = outcome;
    body[1] = (uint8_t)reason_len;
    if (reason_len > 0u) {
        memcpy(&body[2], reason, reason_len);
    }
    if (s_config.send != NULL) {
        s_config.send(AQ_OP_OTA_RESULT, request_id, body,
                      (uint16_t)(2u + reason_len));
    }
}

/* OTA_ACK status bytes (firmware-ota.md §5). */
#define OTA_ACK_OK 0u
#define OTA_ACK_NACK 1u

/* OTA_RESULT outcome bytes (firmware-ota.md §5). */
#define OTA_OUTCOME_VERIFIED 0u
#define OTA_OUTCOME_ACTIVATED 1u
#define OTA_OUTCOME_ROLLBACK 2u
#define OTA_OUTCOME_FAILED 3u

/* OTA_STATUS state bytes (firmware-ota.md §5). */
#define OTA_STATE_IDLE 0u
#define OTA_STATE_RECEIVING 1u
#define OTA_STATE_VERIFYING 2u
#define OTA_STATE_ACTIVATING 3u
#define OTA_STATE_DONE 4u
#define OTA_STATE_FAILED 5u
#define OTA_STATE_ROLLBACK 6u

/* ------------------------------------------------------------------ */
/* Session control                                                     */
/* ------------------------------------------------------------------ */

static void reset_session(void)
{
    if (s_session.digest_started) {
        mbedtls_sha256_free(&s_session.digest);
        s_session.digest_started = false;
    }
    memset(&s_session, 0, sizeof(s_session));
    atomic_store(&s_state, OTA_STATE_IDLE);
}

/* Discard an in-flight transfer and report failure. Caller holds the mutex. */
static void fail_session(uint32_t request_id, const char *reason)
{
    if (s_session.active && s_session.handle_open) {
        esp_ota_abort(s_session.handle);
    }
    reset_session();
    atomic_store(&s_state, OTA_STATE_FAILED);
    ESP_LOGW(TAG, "OTA failed: %s", reason);
    send_ota_result(request_id, OTA_OUTCOME_FAILED, reason);
}

/*
 * The anti-rollback security version: the leading `major.minor.patch` of the
 * image version string, encoded as `major * 1_000_000 + minor * 1000 + patch`.
 * Returns false when the string does not begin with such a triplet, which the
 * caller treats as a rejection (fail closed).
 */
static bool parse_security_version(const char *version, uint32_t *out)
{
    unsigned major = 0;
    unsigned minor = 0;
    unsigned patch = 0;
    int consumed = 0;
    if (sscanf(version, "%u.%u.%u%n", &major, &minor, &patch, &consumed) != 3) {
        return false;
    }
    char tail = version[consumed];
    if (tail != '\0' && tail != '-' && tail != '+' && tail != ' ') {
        return false;
    }
    if (major > 999u || minor > 999u || patch > 999u) {
        return false;
    }
    *out = major * 1000000u + minor * 1000u + patch;
    return true;
}

/* ------------------------------------------------------------------ */
/* Handlers                                                            */
/* ------------------------------------------------------------------ */

static void handle_begin(uint32_t request_id, const uint8_t *body,
                         uint16_t body_len)
{
    /* image_id u32 | size u32 | sha256[32] | signature[64] | version
       (u8 len + bytes) | chunk_size u16 | window u8 */
    if (body_len < 108u) {
        fail_session(request_id, "short OTA_BEGIN body");
        return;
    }
    uint32_t image_id = aq_get_u32_le(&body[0]);
    uint32_t size = aq_get_u32_le(&body[4]);
    uint8_t version_len = body[104];
    if (version_len == 0u || version_len > OTA_VERSION_MAX ||
        (size_t)body_len < (size_t)108u + (size_t)version_len) {
        fail_session(request_id, "bad OTA_BEGIN version");
        return;
    }
    const uint8_t *version = &body[105];
    uint16_t chunk_size = aq_get_u16_le(&body[105 + version_len]);
    uint8_t window = body[107 + version_len];

    if (s_session.active) {
        send_ota_result(request_id, OTA_OUTCOME_FAILED, "transfer already in progress");
        return;
    }
    if (size == 0u) {
        send_ota_result(request_id, OTA_OUTCOME_FAILED, "empty image");
        return;
    }
    if (chunk_size == 0u || chunk_size > AQ_OTA_MAX_CHUNK_SIZE) {
        send_ota_result(request_id, OTA_OUTCOME_FAILED, "bad chunk size");
        return;
    }

    char version_text[OTA_VERSION_MAX + 1u];
    memcpy(version_text, version, version_len);
    version_text[version_len] = '\0';

    uint32_t image_secver = 0;
    if (!parse_security_version(version_text, &image_secver)) {
        send_ota_result(request_id, OTA_OUTCOME_FAILED, "unparseable version");
        return;
    }
#if !CONFIG_AQUILA_FW_ALLOW_DOWNGRADE
    if (image_secver < node_ota_security_version()) {
        send_ota_result(request_id, OTA_OUTCOME_FAILED, "below security floor");
        return;
    }
#endif

    const esp_partition_t *partition = esp_ota_get_next_update_partition(NULL);
    if (partition == NULL) {
        send_ota_result(request_id, OTA_OUTCOME_FAILED, "no inactive partition");
        return;
    }
    if (size > partition->size) {
        send_ota_result(request_id, OTA_OUTCOME_FAILED, "image larger than slot");
        return;
    }

    esp_ota_handle_t handle = 0;
    esp_err_t err = esp_ota_begin(partition, size, &handle);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "esp_ota_begin failed: %s", esp_err_to_name(err));
        send_ota_result(request_id, OTA_OUTCOME_FAILED, "esp_ota_begin failed");
        return;
    }

    reset_session();
    s_session.active = true;
    s_session.image_id = image_id;
    s_session.size = size;
    s_session.received = 0u;
    s_session.chunk_size = chunk_size;
    s_session.window = window;
    s_session.security_version = image_secver;
    memcpy(s_session.version, version_text, (size_t)version_len + 1u);
    memcpy(s_session.sha256, &body[8], 32);
    memcpy(s_session.signature, &body[40], 64);
    s_session.partition = partition;
    s_session.handle = handle;
    s_session.handle_open = true;
    mbedtls_sha256_init(&s_session.digest);
    mbedtls_sha256_starts(&s_session.digest, 0);
    s_session.digest_started = true;
    atomic_store(&s_state, OTA_STATE_RECEIVING);

    ESP_LOGI(TAG, "OTA begin: image %u, %u bytes, version %s, slot %s",
             (unsigned)image_id, (unsigned)size, version_text, partition->label);
    send_ota_ack(request_id, 0u, OTA_ACK_OK);
    send_ota_status(request_id, OTA_STATE_RECEIVING, 0u, size);
}

static void handle_chunk(uint32_t request_id, const uint8_t *body,
                         uint16_t body_len)
{
    /* offset u32 | len u16 | crc32 u32 | bytes[len] */
    if (body_len < 10u) {
        fail_session(request_id, "short OTA_CHUNK body");
        return;
    }
    if (!s_session.active) {
        send_ota_ack(request_id, 0u, OTA_ACK_NACK);
        return;
    }
    uint32_t offset = aq_get_u32_le(&body[0]);
    uint16_t len = aq_get_u16_le(&body[4]);
    uint32_t crc = aq_get_u32_le(&body[6]);
    if ((size_t)body_len < (size_t)10u + (size_t)len) {
        fail_session(request_id, "OTA_CHUNK length mismatch");
        return;
    }
    const uint8_t *bytes = &body[10];

    /* A gap means chunks were lost: ask for a retransmit from what we have. */
    if (offset > s_session.received) {
        send_ota_ack(request_id, s_session.received, OTA_ACK_NACK);
        return;
    }
    /* A chunk below the watermark was already written: re-acknowledge. */
    if (offset < s_session.received) {
        send_ota_ack(request_id, s_session.received, OTA_ACK_OK);
        return;
    }
    if ((uint64_t)offset + (uint64_t)len > (uint64_t)s_session.size) {
        fail_session(request_id, "OTA_CHUNK past image end");
        return;
    }

    /* `esp_rom_crc32_le(0, ...)` returns the standard (zlib/IEEE) CRC-32, the
     * same value `crc32fast::hash` produces on the core (firmware-ota.md §5). */
    uint32_t computed = esp_rom_crc32_le(0u, bytes, len);
    if (computed != crc) {
        ESP_LOGW(TAG, "OTA chunk CRC mismatch at %u", (unsigned)offset);
        send_ota_ack(request_id, s_session.received, OTA_ACK_NACK);
        return;
    }

    esp_err_t err = esp_ota_write(s_session.handle, bytes, len);
    if (err != ESP_OK) {
        fail_session(request_id, "esp_ota_write failed");
        return;
    }
    mbedtls_sha256_update(&s_session.digest, bytes, len);
    s_session.received += len;
    send_ota_ack(request_id, s_session.received, OTA_ACK_OK);
}

static void handle_end(uint32_t request_id, const uint8_t *body,
                       uint16_t body_len)
{
    /* image_id u32 | sha256[32] */
    if (body_len < 36u) {
        fail_session(request_id, "short OTA_END body");
        return;
    }
    if (!s_session.active) {
        send_ota_result(request_id, OTA_OUTCOME_FAILED, "no transfer");
        return;
    }
    uint32_t image_id = aq_get_u32_le(&body[0]);
    if (image_id != s_session.image_id) {
        fail_session(request_id, "OTA_END image id mismatch");
        return;
    }
    if (s_session.received != s_session.size) {
        fail_session(request_id, "incomplete image");
        return;
    }
    if (memcmp(&body[4], s_session.sha256, 32) != 0) {
        fail_session(request_id, "OTA_END digest mismatch");
        return;
    }

    atomic_store(&s_state, OTA_STATE_VERIFYING);
    uint8_t digest[32];
    mbedtls_sha256_finish(&s_session.digest, digest);
    if (memcmp(digest, s_session.sha256, 32) != 0) {
        fail_session(request_id, "image SHA-256 mismatch");
        return;
    }
    if (!s_have_pubkey || !aq_ed25519_verify(s_pubkey, s_session.signature, digest)) {
        fail_session(request_id, "Ed25519 signature invalid");
        return;
    }

    esp_err_t err = esp_ota_end(s_session.handle);
    s_session.handle_open = false;
    if (err != ESP_OK) {
        fail_session(request_id, "esp_ota_end failed");
        return;
    }
    err = esp_ota_set_boot_partition(s_session.partition);
    if (err != ESP_OK) {
        fail_session(request_id, "esp_ota_set_boot_partition failed");
        return;
    }

    /* Remember the candidate security version; it only takes effect once the
     * new image confirms itself healthy. */
    save_pending_secver(s_session.security_version);

    atomic_store(&s_state, OTA_STATE_ACTIVATING);
    ESP_LOGI(TAG, "OTA verified; activating %s and rebooting",
             s_session.partition->label);
    send_ota_result(request_id, OTA_OUTCOME_VERIFIED,
                    "verified; rebooting for health check");
    reset_session();

    /* Let the datagram leave before the reboot. */
    vTaskDelay(pdMS_TO_TICKS(100));
    esp_restart();
}

static void handle_abort(uint32_t request_id, const uint8_t *body,
                         uint16_t body_len)
{
    (void)body;
    (void)body_len;
    if (!s_session.active) {
        send_ota_result(request_id, OTA_OUTCOME_FAILED, "no transfer");
        return;
    }
    if (s_session.handle_open) {
        esp_ota_abort(s_session.handle);
    }
    reset_session();
    atomic_store(&s_state, OTA_STATE_FAILED);
    send_ota_result(request_id, OTA_OUTCOME_FAILED, "aborted");
}

/* ------------------------------------------------------------------ */
/* Health check and rollback                                           */
/* ------------------------------------------------------------------ */

static void health_task(void *arg)
{
    (void)arg;
    uint32_t window_ms = s_config.health_window_ms > 0u
                             ? s_config.health_window_ms
                             : OTA_DEFAULT_WINDOW_MS;
    int64_t start_us = esp_timer_get_time();
    bool healthy = false;

    for (;;) {
        bool now_ok = s_config.health != NULL && s_config.health(s_config.health_ctx);
        int64_t elapsed_ms = (esp_timer_get_time() - start_us) / 1000;
        if ((uint64_t)elapsed_ms >= (uint64_t)window_ms && now_ok) {
            healthy = true;
            break;
        }
        /* One grace window for transient blips before giving up. */
        if ((uint64_t)elapsed_ms >= (uint64_t)window_ms * 2u) {
            break;
        }
        vTaskDelay(pdMS_TO_TICKS(1000));
    }

    if (healthy) {
#if CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE
        (void)esp_ota_mark_app_valid_cancel_rollback();
#endif
        save_pending_flag(false);
        uint32_t candidate = load_pending_secver();
        if (candidate > s_stored_secver) {
            s_stored_secver = candidate;
            save_security_version(candidate);
        }
        atomic_store(&s_pending_verify, false);
        atomic_store(&s_activated_pending, true);
        atomic_store(&s_state, OTA_STATE_DONE);
        ESP_LOGI(TAG, "health window passed; image confirmed");
    } else {
        ESP_LOGE(TAG, "health window failed; rolling back");
        send_ota_result(0u, OTA_OUTCOME_ROLLBACK, "health check failed");
        atomic_store(&s_state, OTA_STATE_ROLLBACK);
#if CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE
        esp_ota_mark_app_invalid_rollback_and_reboot();
#else
        esp_restart();
#endif
    }
    vTaskDelete(NULL);
}

static void start_health_check(void)
{
    esp_ota_img_states_t state = ESP_OTA_IMG_UNDEFINED;
    const esp_partition_t *running = esp_ota_get_running_partition();
    if (running == NULL ||
        esp_ota_get_state_partition(running, &state) != ESP_OK ||
        state != ESP_OTA_IMG_PENDING_VERIFY) {
        atomic_store(&s_pending_verify, false);
        save_pending_flag(false);
        return;
    }

    atomic_store(&s_pending_verify, true);
    save_pending_flag(true);
    if (xTaskCreate(health_task, "node_ota_health", OTA_HEALTH_STACK, NULL,
                    OTA_HEALTH_PRIO, NULL) != pdPASS) {
        ESP_LOGE(TAG, "could not start the health task");
    }
    ESP_LOGW(TAG, "unconfirmed OTA image; starting %us health window",
             (unsigned)(s_config.health_window_ms / 1000u));
}

/* ------------------------------------------------------------------ */
/* Public API                                                          */
/* ------------------------------------------------------------------ */

uint32_t node_ota_security_version(void)
{
    return s_stored_secver > s_compiled_secver ? s_stored_secver : s_compiled_secver;
}

bool node_ota_pending_verify(void)
{
    return atomic_load(&s_pending_verify);
}

bool node_ota_handles(uint8_t opcode)
{
    return opcode == AQ_OP_OTA_BEGIN || opcode == AQ_OP_OTA_CHUNK ||
           opcode == AQ_OP_OTA_END || opcode == AQ_OP_OTA_ABORT;
}

void node_ota_handle(uint8_t opcode, uint32_t request_id, const uint8_t *body,
                     uint16_t body_len)
{
    if (!node_ota_handles(opcode)) {
        return;
    }
    xSemaphoreTake(s_mutex, portMAX_DELAY);
    switch (opcode) {
    case AQ_OP_OTA_BEGIN:
        handle_begin(request_id, body, body_len);
        break;
    case AQ_OP_OTA_CHUNK:
        handle_chunk(request_id, body, body_len);
        break;
    case AQ_OP_OTA_END:
        handle_end(request_id, body, body_len);
        break;
    case AQ_OP_OTA_ABORT:
        handle_abort(request_id, body, body_len);
        break;
    default:
        break;
    }
    xSemaphoreGive(s_mutex);
}

void node_ota_poll(void)
{
    if (atomic_exchange(&s_activated_pending, false)) {
        send_ota_result(0u, OTA_OUTCOME_ACTIVATED, "image activated and healthy");
    }
}

int node_ota_start(const node_ota_config_t *config)
{
    if (config == NULL) {
        return -1;
    }
    s_config = *config;
    if (s_config.health_window_ms == 0u) {
        s_config.health_window_ms = OTA_DEFAULT_WINDOW_MS;
    }

    s_mutex = xSemaphoreCreateMutex();
    if (s_mutex == NULL) {
        return -1;
    }
    atomic_init(&s_pending_verify, false);
    atomic_init(&s_activated_pending, false);
    atomic_init(&s_state, OTA_STATE_IDLE);
    reset_session();

    load_trust_anchor();
    load_security_version();

    ESP_LOGI(TAG, "OTA agent ready; security floor %u, trust anchor %s",
             (unsigned)node_ota_security_version(),
             s_have_pubkey ? "present" : "absent");

    start_health_check();
    return 0;
}
