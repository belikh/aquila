//! `FakeCoreApi` — the recorded-fixture client (`docs/spec/dashboard.md` §3, §9).
//!
//! It implements the [`CoreApi`](crate::api::CoreApi) transport primitives over
//! an in-memory topology so screens and flows can be exercised with no network.
//! It is compiled for every target (not just WASM), so host unit tests drive
//! the same seam the browser uses.
//!
//! # Prose
//!
//! Australian English throughout (serialisation, behaviour, normalise).

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use crate::api::{ApiError, CoreApi};
use crate::contract::{
    CalibrationSession, CalibrationStep, Health, Schema, SchemaHome, Snapshot, Subsystem,
};

/// The mutable fixture state behind a [`FakeCoreApi`].
#[derive(Debug)]
struct FakeState {
    schema: Schema,
    snapshot: Snapshot,
    health: Health,
    calibration: Option<CalibrationSession>,
    nodes: Vec<Value>,
    pairs: Vec<Value>,
    next_id: u64,
}

/// A `CoreApi` served from an in-memory fixture (`dashboard.md` §9).
#[derive(Clone)]
pub struct FakeCoreApi {
    state: Arc<Mutex<FakeState>>,
    base: String,
}

impl FakeCoreApi {
    /// A fixture home: two floors, three rooms, two nodes and one node pair.
    #[must_use]
    pub fn fixture() -> Self {
        let schema = Schema {
            instance_id: "inst_fixture".to_string(),
            contract: "1.0".to_string(),
            home: SchemaHome {
                id: "hom_fixture".to_string(),
                name: "Fixture Home".to_string(),
                time_zone: "Australia/Sydney".to_string(),
            },
            floors: vec![
                crate::contract::Floor {
                    id: "flr_ground".to_string(),
                    name: "Ground".to_string(),
                    level: 0,
                    elevation_m: 0.0,
                },
                crate::contract::Floor {
                    id: "flr_up".to_string(),
                    name: "Upstairs".to_string(),
                    level: 1,
                    elevation_m: 3.0,
                },
            ],
            rooms: vec![
                crate::contract::Room {
                    id: "rom_kitchen".to_string(),
                    floor_id: "flr_ground".to_string(),
                    name: "Kitchen".to_string(),
                    kind: "kitchen".to_string(),
                },
                crate::contract::Room {
                    id: "rom_living".to_string(),
                    floor_id: "flr_ground".to_string(),
                    name: "Living".to_string(),
                    kind: "living".to_string(),
                },
                crate::contract::Room {
                    id: "rom_study".to_string(),
                    floor_id: "flr_up".to_string(),
                    name: "Study".to_string(),
                    kind: "study".to_string(),
                },
            ],
            metrics: Vec::new(),
        };

        let mut zones = std::collections::BTreeMap::new();
        zones.insert(
            "rom_kitchen".to_string(),
            crate::contract::Zone {
                kind: "room".to_string(),
                name: "Kitchen".to_string(),
                presence: Some(true),
                presence_confidence: Some(0.93),
                person_count: Some(1),
                breathing_rate: Some(14.0),
                heart_rate: Some(68.0),
            },
        );
        zones.insert(
            "rom_living".to_string(),
            crate::contract::Zone {
                kind: "room".to_string(),
                name: "Living".to_string(),
                presence: Some(false),
                presence_confidence: Some(0.88),
                person_count: Some(0),
                ..Default::default()
            },
        );
        let snapshot = Snapshot {
            instance_id: "inst_fixture".to_string(),
            home: "hom_fixture".to_string(),
            zones,
        };

        let health = Health {
            status: "degraded".to_string(),
            uptime_ms: 813_000,
            api: crate::contract::ApiVersion { major: 1, minor: 0 },
            subsystems: vec![
                Subsystem {
                    name: "sensing".to_string(),
                    status: "ok".to_string(),
                    detail: None,
                },
                Subsystem {
                    name: "sources".to_string(),
                    status: "degraded".to_string(),
                    detail: Some("1 of 2 nodes offline".to_string()),
                },
            ],
        };

        let nodes = vec![
            json!({
                "id": "nod_1", "name": "Kitchen node", "roomId": "rom_kitchen",
                "mac": "aa:bb:cc:dd:ee:01", "hardware": "esp32s3",
                "firmware": { "installed_version": "0.1.0" }, "state": "online"
            }),
            json!({
                "id": "nod_2", "name": "Living node", "roomId": "rom_living",
                "mac": "aa:bb:cc:dd:ee:02", "hardware": "esp32s3",
                "firmware": { "installed_version": "0.1.0" }, "state": "offline"
            }),
        ];
        let pairs = vec![json!({
            "id": "prx_1", "nodeA": "nod_1", "nodeB": "nod_2",
            "enabled": true, "weight": 1.0, "quality": 0.72,
            "source": "declared", "revision": 1
        })];

        Self {
            state: Arc::new(Mutex::new(FakeState {
                schema,
                snapshot,
                health,
                calibration: None,
                nodes,
                pairs,
                next_id: 100,
            })),
            base: String::new(),
        }
    }

    /// A fixture bound to an explicit ingress base path, for base-path tests.
    #[must_use]
    pub fn with_base(base: impl Into<String>) -> Self {
        let mut fake = Self::fixture();
        fake.base = base.into();
        fake
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, FakeState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn mint(&self, prefix: &str) -> String {
        let mut state = self.lock();
        state.next_id += 1;
        format!("{prefix}_{}", state.next_id)
    }
}

impl CoreApi for FakeCoreApi {
    fn base_path(&self) -> String {
        self.base.clone()
    }

    async fn get_json(&self, path: &str) -> Result<Value, ApiError> {
        let state = self.lock();
        match path {
            "/capabilities" => serde_json::to_value(json!({
                "api": { "major": 1, "minor": 0 },
                "transports": { "http": true, "websocket": false },
                "features": { "polling": true, "calibrationWizard": true,
                              "nodeControl": false, "firmwareOta": true },
                "minComponentVersion": "0.1.0"
            }))
            .map_err(|error| ApiError::Decode(error.to_string())),
            "/schema" => serde_json::to_value(&state.schema)
                .map_err(|error| ApiError::Decode(error.to_string())),
            "/state" => serde_json::to_value(&state.snapshot)
                .map_err(|error| ApiError::Decode(error.to_string())),
            "/health" => serde_json::to_value(&state.health)
                .map_err(|error| ApiError::Decode(error.to_string())),
            "/home" => serde_json::to_value(json!({
                "id": state.schema.home.id,
                "name": state.schema.home.name,
                "timezone": state.schema.home.time_zone,
                "floors": state.schema.floors.len(),
                "revision": 1
            }))
            .map_err(|error| ApiError::Decode(error.to_string())),
            "/nodes" => Ok(json!({ "items": state.nodes })),
            "/node-pairs" => Ok(json!({ "items": state.pairs })),
            "/metrics/snapshot" => Ok(json!({
                "commit": "9f2c41e", "generatedAt": "2026-10-13T20:45:00.000Z",
                "revision": 17,
                "metrics": [
                    { "name": "accuracy.presence_f1", "kind": "gated", "unit": "ratio",
                      "value": 0.94, "n": 12000, "reference": "camera-gt",
                      "gate": { "bar": 0.9, "pass": true } }
                ],
                "bars": {
                    "presenceF1": { "target": 0.90, "observed": 0.94, "pass": true }
                }
            })),
            "/calibration/current" => match &state.calibration {
                Some(session) => serde_json::to_value(session)
                    .map_err(|error| ApiError::Decode(error.to_string())),
                None => Ok(Value::Null),
            },
            _ => Err(ApiError::not_found()),
        }
    }

    async fn send_json(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        _revision: Option<i64>,
    ) -> Result<Value, ApiError> {
        match (method, path) {
            ("POST", "/floors") => {
                let name = body_field(&body, "name").unwrap_or_else(|| "Floor".to_string());
                let level = body
                    .as_ref()
                    .and_then(|value| value.get("index"))
                    .and_then(Value::as_i64)
                    .unwrap_or(0) as i8;
                let id = self.mint("flr");
                let mut state = self.lock();
                state.schema.floors.push(crate::contract::Floor {
                    id: id.clone(),
                    name: name.clone(),
                    level,
                    elevation_m: 0.0,
                });
                Ok(json!({ "id": id, "name": name, "index": level, "revision": 1 }))
            }
            ("POST", "/nodes") => {
                let name = body_field(&body, "name").unwrap_or_else(|| "Node".to_string());
                let mac = body_field(&body, "mac").unwrap_or_default();
                let room = body_field(&body, "roomId").unwrap_or_default();
                let id = self.mint("nod");
                let node = json!({
                    "id": id, "name": name, "roomId": room, "mac": mac,
                    "hardware": "esp32s3", "firmware": { "installed_version": "0.1.0" },
                    "state": "online"
                });
                self.lock().nodes.push(node.clone());
                Ok(node)
            }
            ("POST", "/node-pairs") => {
                let a = body_field(&body, "nodeA").unwrap_or_default();
                let b = body_field(&body, "nodeB").unwrap_or_default();
                let id = self.mint("prx");
                let pair = json!({
                    "id": id, "nodeA": a, "nodeB": b, "enabled": true,
                    "weight": 1.0, "quality": Value::Null, "source": "declared",
                    "revision": 1
                });
                self.lock().pairs.push(pair.clone());
                Ok(pair)
            }
            ("POST", "/node-pairs/derive") => {
                let state = self.lock();
                Ok(json!({ "items": state.pairs }))
            }
            ("POST", "/calibration") => {
                let id = self.mint("cal");
                let session = CalibrationSession {
                    id: id.clone(),
                    scope: "global".to_string(),
                    status: "awaiting_user".to_string(),
                    steps: vec![
                        CalibrationStep {
                            id: "stp_1".to_string(),
                            kind: "node_health".to_string(),
                            order: 0,
                            status: "complete".to_string(),
                            awaiting: "none".to_string(),
                            instruction: "Checking node health.".to_string(),
                            progress: Some(1.0),
                        },
                        CalibrationStep {
                            id: "stp_2".to_string(),
                            kind: "baseline_empty".to_string(),
                            order: 1,
                            status: "awaiting".to_string(),
                            awaiting: "user".to_string(),
                            instruction: "Leave the home empty for the baseline.".to_string(),
                            progress: Some(0.0),
                        },
                    ],
                    current_step_id: Some("stp_2".to_string()),
                    revision: 1,
                };
                self.lock().calibration = Some(session.clone());
                Ok(serde_json::to_value(session)
                    .map_err(|error| ApiError::Decode(error.to_string()))?)
            }
            ("POST", p) if p.ends_with("/commit") => {
                let mut state = self.lock();
                let mut session = state.calibration.clone().ok_or_else(ApiError::not_found)?;
                session.status = "committed".to_string();
                session.revision += 1;
                state.calibration = Some(session.clone());
                Ok(serde_json::to_value(session)
                    .map_err(|error| ApiError::Decode(error.to_string()))?)
            }
            ("POST", p) if p.ends_with("/abort") => {
                let mut state = self.lock();
                let mut session = state.calibration.clone().ok_or_else(ApiError::not_found)?;
                session.status = "aborted".to_string();
                session.revision += 1;
                state.calibration = Some(session.clone());
                Ok(serde_json::to_value(session)
                    .map_err(|error| ApiError::Decode(error.to_string()))?)
            }
            ("POST", p) if p.ends_with("/complete") || p.ends_with("/skip") => {
                let state = self.lock();
                let session = state.calibration.clone().ok_or_else(ApiError::not_found)?;
                Ok(serde_json::to_value(session)
                    .map_err(|error| ApiError::Decode(error.to_string()))?)
            }
            ("POST", p) if p.contains("/rooms") => {
                let floor_id = p.split('/').nth(2).unwrap_or_default().to_string();
                let name = body_field(&body, "name").unwrap_or_else(|| "Room".to_string());
                let label = body_field(&body, "label").unwrap_or_else(|| "other".to_string());
                let id = self.mint("rom");
                let mut state = self.lock();
                state.schema.rooms.push(crate::contract::Room {
                    id: id.clone(),
                    floor_id: floor_id.clone(),
                    name: name.clone(),
                    kind: label.clone(),
                });
                Ok(json!({
                    "id": id, "floorId": floor_id, "name": name,
                    "label": label, "revision": 1
                }))
            }
            ("PATCH", "/home") => {
                let mut state = self.lock();
                if let Some(name) = body_field(&body, "name") {
                    state.schema.home.name = name;
                }
                if let Some(zone) = body_field(&body, "timezone") {
                    state.schema.home.time_zone = zone;
                }
                Ok(json!({
                    "id": state.schema.home.id,
                    "name": state.schema.home.name,
                    "timezone": state.schema.home.time_zone,
                    "floors": state.schema.floors.len(),
                    "revision": 2
                }))
            }
            ("PATCH", p) if p.starts_with("/rooms/") => {
                let id = p.trim_start_matches("/rooms/").to_string();
                let name = body_field(&body, "name").unwrap_or_else(|| "Room".to_string());
                let mut state = self.lock();
                if let Some(room) = state.schema.rooms.iter_mut().find(|room| room.id == id) {
                    room.name = name.clone();
                }
                Ok(json!({ "id": id, "name": name, "revision": 2 }))
            }
            ("DELETE", p) if p.starts_with("/rooms/") => {
                let id = p.trim_start_matches("/rooms/").to_string();
                self.lock().schema.rooms.retain(|room| room.id != id);
                Ok(Value::Null)
            }
            ("PATCH", p) if p.starts_with("/nodes/") => {
                let id = p.trim_start_matches("/nodes/").to_string();
                let room = body_field(&body, "roomId").unwrap_or_default();
                let mut state = self.lock();
                for node in &mut state.nodes {
                    if node.get("id").and_then(Value::as_str) == Some(id.as_str()) {
                        node["roomId"] = json!(room);
                    }
                }
                Ok(json!({ "id": id, "roomId": room, "revision": 2 }))
            }
            ("DELETE", p) if p.starts_with("/nodes/") => {
                let id = p.trim_start_matches("/nodes/").to_string();
                self.lock()
                    .nodes
                    .retain(|node| node.get("id").and_then(Value::as_str) != Some(id.as_str()));
                Ok(Value::Null)
            }
            ("DELETE", p) if p.starts_with("/node-pairs/") => {
                let id = p.trim_start_matches("/node-pairs/").to_string();
                self.lock()
                    .pairs
                    .retain(|pair| pair.get("id").and_then(Value::as_str) != Some(id.as_str()));
                Ok(Value::Null)
            }
            _ => Err(ApiError::not_found()),
        }
    }
}

/// Read a string field from a JSON request body.
fn body_field(body: &Option<Value>, key: &str) -> Option<String> {
    body.as_ref()
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewmodel::{structure_view, validate_new_room};

    /// Poll a future that is ready on the first poll (the fake never pends).
    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        use std::task::{Context, Poll};
        let mut context = Context::from_waker(std::task::Waker::noop());
        let mut future = Box::pin(future);
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("the fake client never yields"),
        }
    }

    #[test]
    fn the_structure_screen_renders_floors_and_rooms() {
        let client = FakeCoreApi::fixture();
        let schema = block_on(client.schema()).expect("schema");
        let view = structure_view(&schema);
        assert_eq!(view.floors.len(), 2);
        assert_eq!(view.room_count, 3);
        assert_eq!(view.home_name, "Fixture Home");
    }

    #[test]
    fn a_create_room_action_is_validated_then_persisted() {
        let client = FakeCoreApi::fixture();
        // Invalid input is rejected before any call.
        assert!(validate_new_room("", "", "").is_err());
        // A valid action persists through the seam and is visible on the next read.
        let room = validate_new_room("flr_ground", "Pantry", "other").expect("valid");
        block_on(client.create_room("flr_ground", &room)).expect("create");
        let schema = block_on(client.schema()).expect("schema");
        assert!(schema.rooms.iter().any(|room| room.name == "Pantry"));
    }

    #[test]
    fn a_known_route_reads_and_an_unknown_one_is_not_found() {
        let client = FakeCoreApi::fixture();
        assert!(block_on(client.health()).is_ok());
        let missing = block_on(client.get_json("/nonsense")).expect_err("missing");
        assert_eq!(
            missing,
            ApiError::Http {
                status: 404,
                code: "not_found".to_string(),
                detail: "no such route in the v1 control plane".to_string(),
            }
        );
    }

    #[test]
    fn the_fixture_carries_a_stable_base_path() {
        let client = FakeCoreApi::with_base("/api/hassio_ingress/tok");
        assert_eq!(client.base_path(), "/api/hassio_ingress/tok");
    }
}
