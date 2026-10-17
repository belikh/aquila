//! Read-only JSON projections built from the topology at server start.
//!
//! The control plane serves three documents that are pure functions of the
//! configured [`Topology`] and do not change as sensing events arrive:
//!
//! - the node list (`GET /api/v1/nodes`, `core-api.md` §7);
//! - the zone/metric schema (`GET /api/v1/schema`);
//! - the firmware store manifest and the per-node firmware state
//!   (`GET /api/v1/firmware`, `GET /api/v1/nodes/{node}/firmware`,
//!   `firmware-ota.md` §7).
//!
//! Building them once, from the same source the entity model uses, keeps a
//! single writer of topology truth and makes the routes cheap.

use std::collections::BTreeMap;

use aquila_geometry::Topology;
use aquila_types::{to_hex, DeviceType, Node};
use serde_json::{json, Value};

/// The topology-derived documents a running server serves.
pub(crate) struct Projections {
    /// The node list, shaped as `{ "items": [ ... ] }`.
    pub nodes: Value,
    /// The zone tree, metric vocabulary and units.
    pub schema: Value,
    /// Per-node firmware state, keyed by lower-case MAC and by dense id string.
    ///
    /// The firmware store manifest (`GET /api/v1/firmware`) is served from the
    /// OTA runtime, not a static projection, because it changes as images are
    /// imported (`firmware-ota.md` §7).
    pub node_firmware: BTreeMap<String, Value>,
    /// The target-chip string per node, keyed by lower-case MAC and dense id.
    pub node_target: BTreeMap<String, String>,
}

/// Build every topology-derived document.
pub(crate) fn build(topology: &Topology, instance_id: &str) -> Projections {
    let model = topology.model();
    let home = &model.home;

    let mut nodes = Vec::new();
    let mut node_firmware = BTreeMap::new();
    let mut node_target = BTreeMap::new();
    for node in &model.nodes {
        nodes.push(node_json(node));
        let firmware = node_firmware_json(node);
        let target = hardware_name(node.device).to_string();
        node_firmware.insert(mac_string(node), firmware.clone());
        node_firmware.insert(node.id.0.to_string(), firmware);
        node_target.insert(mac_string(node), target.clone());
        node_target.insert(node.id.0.to_string(), target);
    }

    let zones: Vec<Value> = model
        .zones
        .iter()
        .map(|zone| {
            json!({
                "id": zone.id.as_str(),
                "name": zone.name,
                "origin_home_m": zone.origin_home_m,
                "extent_home_m": zone.extent_home_m,
            })
        })
        .collect();

    let schema = json!({
        "instance_id": instance_id,
        "contract": home.contract.to_string(),
        "home": { "id": home.id.as_str(), "name": home.name, "time_zone": home.time_zone },
        "zones": zones,
        "metrics": metric_vocabulary(),
    });

    Projections {
        nodes: json!({ "items": nodes }),
        schema,
        node_firmware,
        node_target,
    }
}

/// One node as the canonical `Node` representation (`core-api.md` §7).
fn node_json(node: &Node) -> Value {
    json!({
        "id": node.id.0.to_string(),
        "name": node.name,
        "pos_home_m": node.pos_home_m,
        "mac": mac_string(node),
        "hardware": hardware_name(node.device),
        "firmware": node_firmware_json(node),
        "state": "unknown",
    })
}

/// The static per-node firmware state (`firmware-ota.md` §7).
///
/// `installed_version` is real (from the topology). The live OTA fields
/// (`latest_version`, `pending`, `last_result`) are filled in by
/// [`Handle::node_firmware_json`](crate::Handle) from the OTA runtime; here they
/// are honestly absent rather than fabricated.
fn node_firmware_json(node: &Node) -> Value {
    json!({
        "node": mac_string(node),
        "installed_version": node.firmware.version,
        "installed_sha256": node.firmware.sha256.map(|bytes| to_hex(&bytes)),
        "latest_version": Value::Null,
        "boot_partition": Value::Null,
        "pending": Value::Null,
        "last_result": Value::Null,
    })
}

/// The metric vocabulary and units the component projects (`home-assistant.md`
/// §6.1). Ids are the canonical lowercase snake_case names.
fn metric_vocabulary() -> Value {
    json!([
        { "id": "occupancy", "unit": Value::Null },
        { "id": "presence", "unit": Value::Null },
        { "id": "person_count", "unit": "people" },
        { "id": "breathing_rate", "unit": "brpm" },
        { "id": "heart_rate", "unit": "bpm" },
    ])
}

/// The HA hardware string for a device type.
fn hardware_name(device: DeviceType) -> &'static str {
    match device {
        DeviceType::Esp32S3 => "esp32s3",
        DeviceType::Esp32C6 => "esp32c6",
        DeviceType::Unknown => "unknown",
    }
}

/// The canonical lower-case colon-separated MAC string.
fn mac_string(node: &Node) -> String {
    let octets = node.mac.octets();
    octets
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}
