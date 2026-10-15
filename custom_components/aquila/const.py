"""Constants for the Aquila HACS custom component.

Vocabulary and naming follow ``docs/spec/home-assistant.md`` §4/§6 exactly.
This module holds no transport and no sensing logic; it is the shared alphabet
for the component.
"""

from __future__ import annotations

from typing import Final

from homeassistant.const import Platform

DOMAIN: Final = "aquila"
MANUFACTURER: Final = "Aquila"

#: Component version; must match ``manifest.json`` ``version`` and is the value
#: compared against the core's ``min_component_version`` (home-assistant.md §9).
COMPONENT_VERSION: Final = "0.2.1"

#: The API major the component speaks; the core's ``api_version`` major must match.
API_MAJOR: Final = 1

#: Default core URL. HA Core resolves the add-on by hostname slug
#: (``aquila_core`` → ``aquila-core``); see home-assistant.md §2/§4.
DEFAULT_URL: Final = "http://aquila-core:8099"

#: Coordinator poll cadence in seconds (home-assistant.md §6.2). v1 has no
#: WebSocket stream (core-api.md §10), so polling is the primary source.
DEFAULT_UPDATE_INTERVAL: Final = 60
MIN_UPDATE_INTERVAL: Final = 10
MAX_UPDATE_INTERVAL: Final = 600

DEFAULT_EXPOSE_FLOORS: Final = True
DEFAULT_EXPOSE_ROOM_VITALS: Final = True

# Config entry / options keys.
CONF_URL: Final = "url"
CONF_TOKEN: Final = "token"
CONF_UPDATE_INTERVAL: Final = "update_interval"
CONF_EXPOSE_FLOORS: Final = "expose_floors"
CONF_EXPOSE_ROOM_VITALS: Final = "expose_room_vitals"

# Service names.
SERVICE_INSTALL_FIRMWARE: Final = "install_firmware"

#: Reserved zone id for core/system-scoped entities. home-assistant.md §4 fixes
#: the unique_id shape as ``aquila:{instance_id}:{zone_id}:{metric}``; core
#: health is not a zone, so this sentinel fills the zone slot for system
#: entities. It is a component-internal convention, not a core concept.
SYSTEM_ZONE_ID: Final = "system"

# Snapshot zone kinds (home-assistant.md §6).
ZONE_KIND_HOME: Final = "home"
ZONE_KIND_FLOOR: Final = "floor"
ZONE_KIND_ROOM: Final = "room"

# Snapshot metric keys within a zone.
METRIC_OCCUPANCY: Final = "occupancy"
METRIC_PRESENCE: Final = "presence"
METRIC_PERSON_COUNT: Final = "person_count"
METRIC_BREATHING_RATE: Final = "breathing_rate"
METRIC_HEART_RATE: Final = "heart_rate"
METRIC_NODES_ONLINE: Final = "nodes_online"
METRIC_NODES_TOTAL: Final = "nodes_total"
METRIC_CALIBRATION_STATE: Final = "calibration_state"
METRIC_CLOCK_DRIFT: Final = "clock_drift"
METRIC_LAST_CSI_PACKET: Final = "last_csi_packet"
METRIC_SELF_TEST: Final = "self_test"
METRIC_CORE_CONNECTED: Final = "core_connected"
METRIC_FIRMWARE: Final = "firmware"

# Firmware snapshot keys (firmware-ota.md §7). The coordinator merges the
# core's per-node firmware state and OTA job into ``node["firmware"]``; the
# update entity reads them. ``update_percentage`` is 0–100 to match Home
# Assistant's ``update`` platform; the core serves a 0–1 job fraction.
FIRMWARE_INSTALLED_VERSION: Final = "installed_version"
FIRMWARE_LATEST_VERSION: Final = "latest_version"
FIRMWARE_IMAGE_ID: Final = "image_id"
FIRMWARE_IN_PROGRESS: Final = "in_progress"
FIRMWARE_UPDATE_PERCENTAGE: Final = "update_percentage"
FIRMWARE_UPDATE_AVAILABLE: Final = "update_available"
FIRMWARE_RELEASE_NOTES: Final = "release_notes"
FIRMWARE_PENDING: Final = "pending"
FIRMWARE_LAST_RESULT: Final = "last_result"

# ``GET /api/v1/firmware`` manifest keys (firmware-ota.md §7).
MANIFEST_ITEMS: Final = "items"
IMAGE_ID: Final = "id"
IMAGE_VERSION: Final = "version"
IMAGE_NOTES: Final = "notes"

# ``GET /api/v1/nodes/{node}/firmware/job`` keys (core-api.md §7).
JOB_IMAGE_ID: Final = "imageId"
JOB_IMAGE_ID_SNAKE: Final = "image_id"
JOB_STATE: Final = "state"
JOB_PROGRESS: Final = "progress"
JOB_PROGRESS_FRACTION: Final = "fraction"
JOB_PROGRESS_RECEIVED: Final = "receivedBytes"
JOB_PROGRESS_TOTAL: Final = "totalBytes"
JOB_STATE_RUNNING: Final = "running"

# Service field names (firmware-ota.md §7).
SERVICE_FIELD_NODE: Final = "node"
SERVICE_FIELD_IMAGE_ID: Final = "image_id"
SERVICE_FIELD_VERSION: Final = "version"
SERVICE_FIELD_FORCE: Final = "force"

# Snapshot ``health`` keys (home-assistant.md §6).
HEALTH_CORE_CONNECTED: Final = "core_connected"
HEALTH_NODES_ONLINE: Final = "nodes_online"
HEALTH_NODES_TOTAL: Final = "nodes_total"
HEALTH_CALIBRATION_STATE: Final = "calibration_state"
HEALTH_CLOCK_DRIFT_MS: Final = "clock_drift_ms"
HEALTH_LAST_PACKET_TS: Final = "last_packet_ts"
HEALTH_SELF_TEST_OK: Final = "self_test_ok"

# ``GET /api/v1/health`` contract fields (core-api.md §4.1, home-assistant.md
# §5.2). The version lives in the canonical ``api`` object (``{major, minor}``);
# ``api_version`` is a tolerated legacy alias for a core that still sends it.
HEALTH_INSTANCE_ID: Final = "instance_id"
HEALTH_API: Final = "api"
HEALTH_API_VERSION: Final = "api_version"
HEALTH_MIN_COMPONENT_VERSION: Final = "min_component_version"

# Calibration state vocabulary (home-assistant.md §6).
CALIBRATION_STATES: Final = (
    "uncalibrated",
    "calibrating",
    "calibrated",
    "degraded",
)

PLATFORMS: Final = [
    Platform.BINARY_SENSOR,
    Platform.SENSOR,
    Platform.UPDATE,
]
