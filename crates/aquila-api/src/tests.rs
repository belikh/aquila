//! `aquila-api` tests: the control plane is exercised over real loopback
//! sockets.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use aquila_geometry::Topology;
use aquila_metrics::{Measurement, MetricSink, MetricsError};
use aquila_sensing::Sink;
use aquila_types::{
    sha256, DeviceType, Event, EventKind, EventPayload, EventScope, FirmwareRef, Floor, Home,
    MacAddr, Node, NodeId, Room, RoomKind, Slug, Witness, CONTRACT_VERSION,
};
use aquila_wire::{ControlBody, ControlMessage, OtaAck, OtaAckStatus, OtaOutcome, OtaResult};
use ed25519_dalek::{Signer, SigningKey};

use crate::{
    ApiError, Auth, ChannelSink, FirmwareCompat, FirmwareStore, ImportRequest, OtaError,
    OtaTransport, Server, StoreError,
};

fn slug(value: &str) -> Slug {
    Slug(value.to_string())
}

fn topology() -> Topology {
    let home = slug("home");
    let floor = slug("ground");
    let room = slug("living-room");
    let firmware = FirmwareRef {
        version: "test".to_string(),
        sha256: None,
    };
    Topology::load(aquila_types::Topology {
        home: Home {
            id: home.clone(),
            name: "Home".to_string(),
            time_zone: "Australia/Sydney".to_string(),
            contract: CONTRACT_VERSION,
        },
        floors: vec![Floor {
            id: floor.clone(),
            home: home.clone(),
            name: "Ground".to_string(),
            level: 0,
            elevation_m: 0.0,
        }],
        rooms: vec![Room {
            id: room.clone(),
            floor: floor.clone(),
            name: "Living".to_string(),
            kind: RoomKind::Living,
            origin_home_m: [0.0, 0.0, 0.0],
            yaw_deg: 0.0,
            area_m2: None,
        }],
        nodes: vec![Node {
            id: NodeId(1),
            room: room.clone(),
            name: "node-1".to_string(),
            mac: MacAddr([0, 0, 0, 0, 0, 1]),
            device: DeviceType::Esp32S3,
            firmware,
            pos_room_m: Some([0.0, 0.0, 2.4]),
            installed_unix_ms: 0,
        }],
        pairs: vec![],
    })
    .expect("valid topology")
}

fn presence_event() -> Event {
    Event {
        id: "evt-1".to_string(),
        home: slug("home"),
        t_unix_ms: 1_000,
        dedup_key: "home:presence:living-room".to_string(),
        scope: EventScope::Room(slug("living-room")),
        kind: EventKind::PresenceChanged,
        payload: EventPayload::Presence {
            occupied: true,
            probability: 0.95,
        },
        witness: Witness::Measured,
    }
}

/// A blocking one-shot HTTP GET, returning the whole response as a string.
fn http_get(addr: SocketAddr, path: &str) -> String {
    http_request(
        addr,
        &format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"),
    )
}

/// A blocking one-shot HTTP request with a raw request head.
fn http_request(addr: SocketAddr, raw: &str) -> String {
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream.write_all(raw.as_bytes()).expect("write request");
    let mut response = String::new();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set timeout");
    stream.read_to_string(&mut response).expect("read response");
    response
}

#[test]
fn binding_a_non_loopback_address_is_refused() {
    let non_loopback: SocketAddr = "0.0.0.0:0".parse().expect("addr");
    let error = match Server::bind(non_loopback, &topology(), "test") {
        Ok(_) => panic!("a non-loopback bind must be refused"),
        Err(error) => error,
    };
    assert!(matches!(error, ApiError::NonLoopback(_)));
}

#[test]
fn health_and_state_are_served_over_loopback() {
    let server =
        Server::bind("127.0.0.1:0".parse().expect("addr"), &topology(), "test").expect("bind");
    let addr = server.local_addr().expect("local addr");
    assert!(addr.ip().is_loopback());
    let handle = server.handle();
    let mut guard = server.serve_in_background().expect("serve");

    let health = http_get(addr, "/api/v1/health");
    assert!(health.starts_with("HTTP/1.1 200 OK"), "{health}");
    assert!(health.contains("\"status\":\"ok\""), "{health}");
    assert!(health.contains("\"major\":1"), "{health}");

    // Publish before reading state; the snapshot must reflect it.
    handle.publish(&presence_event());
    let state = http_get(addr, "/api/v1/state");
    assert!(state.starts_with("HTTP/1.1 200 OK"), "{state}");
    assert!(state.contains("\"presence\":true"), "{state}");
    assert!(state.contains("living-room"), "{state}");

    let missing = http_get(addr, "/api/v1/nope");
    assert!(missing.starts_with("HTTP/1.1 404"), "{missing}");

    guard.shutdown();
}

#[test]
fn the_canonical_read_endpoints_are_served() {
    let server =
        Server::bind("127.0.0.1:0".parse().expect("addr"), &topology(), "test").expect("bind");
    let addr = server.local_addr().expect("local addr");
    let mut guard = server.serve_in_background().expect("serve");

    let schema = http_get(addr, "/api/v1/schema");
    assert!(schema.starts_with("HTTP/1.1 200 OK"), "{schema}");
    assert!(schema.contains("\"living-room\""), "{schema}");
    assert!(schema.contains("breathing_rate"), "{schema}");

    let nodes = http_get(addr, "/api/v1/nodes");
    assert!(nodes.starts_with("HTTP/1.1 200 OK"), "{nodes}");
    assert!(nodes.contains("00:00:00:00:00:01"), "{nodes}");
    assert!(nodes.contains("\"installed_version\":\"test\""), "{nodes}");

    let firmware = http_get(addr, "/api/v1/firmware");
    assert!(firmware.starts_with("HTTP/1.1 200 OK"), "{firmware}");
    assert!(firmware.contains("\"items\":[]"), "{firmware}");

    let node_firmware = http_get(addr, "/api/v1/nodes/00:00:00:00:00:01/firmware");
    assert!(
        node_firmware.starts_with("HTTP/1.1 200 OK"),
        "{node_firmware}"
    );
    assert!(
        node_firmware.contains("\"installed_version\":\"test\""),
        "{node_firmware}"
    );

    let metrics = http_get(addr, "/api/v1/metrics/snapshot");
    assert!(metrics.starts_with("HTTP/1.1 200 OK"), "{metrics}");
    assert!(metrics.contains("\"metrics\":[]"), "{metrics}");

    guard.shutdown();
}

#[test]
fn a_token_in_the_query_string_is_rejected() {
    let server =
        Server::bind("127.0.0.1:0".parse().expect("addr"), &topology(), "test").expect("bind");
    let addr = server.local_addr().expect("local addr");
    let mut guard = server.serve_in_background().expect("serve");

    let response = http_get(addr, "/api/v1/state?token=leaked");
    assert!(response.starts_with("HTTP/1.1 401"), "{response}");
    assert!(response.contains("unauthenticated"), "{response}");

    guard.shutdown();
}

#[test]
fn an_invalid_bearer_token_is_rejected_even_on_loopback() {
    let server = Server::bind_with_auth(
        "127.0.0.1:0".parse().expect("addr"),
        &topology(),
        "test",
        Auth::new(Some("secret".to_string())),
    )
    .expect("bind");
    let addr = server.local_addr().expect("local addr");
    let mut guard = server.serve_in_background().expect("serve");

    let wrong = http_request(
        addr,
        "GET /api/v1/state HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer nope\r\n\r\n",
    );
    assert!(wrong.starts_with("HTTP/1.1 401"), "{wrong}");

    let right = http_request(
        addr,
        "GET /api/v1/state HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer secret\r\n\r\n",
    );
    assert!(right.starts_with("HTTP/1.1 200 OK"), "{right}");

    guard.shutdown();
}

#[test]
fn the_ota_mutations_fail_closed() {
    let server =
        Server::bind("127.0.0.1:0".parse().expect("addr"), &topology(), "test").expect("bind");
    let addr = server.local_addr().expect("local addr");
    let mut guard = server.serve_in_background().expect("serve");

    let install = http_request(
        addr,
        "POST /api/v1/nodes/00:00:00:00:00:01/firmware/install HTTP/1.1\r\n\
         Host: localhost\r\nContent-Length: 0\r\n\r\n",
    );
    assert!(install.starts_with("HTTP/1.1 503"), "{install}");
    assert!(install.contains("ota_unavailable"), "{install}");

    let unknown = http_request(
        addr,
        "POST /api/v1/nodes/00:00:00:00:00:99/firmware/abort HTTP/1.1\r\n\
         Host: localhost\r\nContent-Length: 0\r\n\r\n",
    );
    assert!(unknown.starts_with("HTTP/1.1 404"), "{unknown}");

    guard.shutdown();
}

#[test]
fn the_channel_sink_delivers_events_to_subscribers() {
    let server =
        Server::bind("127.0.0.1:0".parse().expect("addr"), &topology(), "test").expect("bind");
    let handle = server.handle();
    let receiver = handle.subscribe();
    let mut sink = ChannelSink::new(handle.clone());

    sink.emit(&presence_event()).expect("emit");
    let delivered = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("delivered");
    assert_eq!(delivered.id, "evt-1");
    assert_eq!(handle.revision(), 1);
}

#[test]
fn the_websocket_upgrade_is_a_typed_fail_closed_stub() {
    let server =
        Server::bind("127.0.0.1:0".parse().expect("addr"), &topology(), "test").expect("bind");
    let addr = server.local_addr().expect("local addr");
    let mut guard = server.serve_in_background().expect("serve");

    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .write_all(
            b"GET /api/v1/stream HTTP/1.1\r\nHost: localhost\r\n\
              Upgrade: websocket\r\nConnection: Upgrade\r\n\
              Sec-WebSocket-Protocol: aquila.v1\r\n\r\n",
        )
        .expect("write");
    let mut response = String::new();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    stream.read_to_string(&mut response).expect("read");
    assert!(response.starts_with("HTTP/1.1 501"), "{response}");
    assert!(response.contains("websocket_unimplemented"), "{response}");

    guard.shutdown();
}

// ---------------------------------------------------------------------------
// Firmware OTA (ticket #13)
// ---------------------------------------------------------------------------

/// A minimal in-test node OTA agent: it writes chunks sequentially, verifies
/// their CRC-32 and answers cumulative ACKs and a final result. It is the
/// `aquila-api` tests' own fake, so the crate keeps no dependency on testkit.
struct FakeNode {
    expected: [u8; 32],
    received: Vec<u8>,
}

impl FakeNode {
    fn handle(&mut self, message: &ControlMessage) -> Option<ControlMessage> {
        let request_id = message.request_id;
        match &message.body {
            ControlBody::OtaBegin(_) => Some(ControlMessage {
                request_id,
                body: ControlBody::OtaAck(OtaAck {
                    received_upto: self.received.len() as u32,
                    status: OtaAckStatus::Ok,
                }),
            }),
            ControlBody::OtaChunk(chunk) => {
                if chunk.offset as usize == self.received.len()
                    && crc32fast::hash(&chunk.bytes) == chunk.crc32
                {
                    self.received.extend_from_slice(&chunk.bytes);
                }
                Some(ControlMessage {
                    request_id,
                    body: ControlBody::OtaAck(OtaAck {
                        received_upto: self.received.len() as u32,
                        status: OtaAckStatus::Ok,
                    }),
                })
            }
            ControlBody::OtaEnd(_) => {
                let complete = sha256(&self.received) == self.expected;
                Some(ControlMessage {
                    request_id,
                    body: ControlBody::OtaResult(OtaResult {
                        outcome: if complete {
                            OtaOutcome::Activated
                        } else {
                            OtaOutcome::Failed
                        },
                        reason: String::new(),
                    }),
                })
            }
            _ => None,
        }
    }
}

/// A control transport over a [`FakeNode`].
struct TestTransport {
    node: Arc<Mutex<FakeNode>>,
    queue: VecDeque<ControlMessage>,
}

impl TestTransport {
    fn new(node: Arc<Mutex<FakeNode>>) -> Self {
        Self {
            node,
            queue: VecDeque::new(),
        }
    }
}

impl OtaTransport for TestTransport {
    fn send(&mut self, message: &ControlMessage) -> Result<(), OtaError> {
        let reply = {
            let mut node = self.node.lock().expect("node lock");
            node.handle(message)
        };
        if let Some(reply) = reply {
            self.queue.push_back(reply);
        }
        Ok(())
    }

    fn try_recv(&mut self) -> Result<Option<ControlMessage>, OtaError> {
        Ok(self.queue.pop_front())
    }
}

/// A metric sink whose recorded rows a test can read back.
#[derive(Clone, Default)]
struct TestSink(Arc<Mutex<Vec<Measurement>>>);

impl TestSink {
    fn records(&self) -> Vec<Measurement> {
        self.0.lock().expect("sink lock").clone()
    }
}

impl MetricSink for TestSink {
    fn record(&mut self, measurement: &Measurement) -> Result<(), MetricsError> {
        measurement.validate()?;
        self.0.lock().expect("sink lock").push(measurement.clone());
        Ok(())
    }
}

/// A temp firmware directory unique to `name`.
fn firmware_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("aquila-api-ota-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// A signed image and its store identity.
fn signed_store(dir: &std::path::Path, bytes: &[u8]) -> (FirmwareStore, String) {
    let key = SigningKey::from_bytes(&[21u8; 32]);
    let store = FirmwareStore::open(dir, key.verifying_key()).expect("open store");
    let digest = sha256(bytes);
    let manifest = store
        .import(
            FirmwareCompat::encode(1, 0),
            "esp32s3",
            ImportRequest {
                image: bytes.to_vec(),
                signature: key.sign(&digest).to_bytes(),
                version: "1.0.0".to_string(),
                target: "esp32s3".to_string(),
                compatibility: FirmwareCompat::new(
                    FirmwareCompat::encode(1, 0),
                    FirmwareCompat::encode(1, 9),
                ),
                notes: String::new(),
            },
        )
        .expect("import");
    let id = manifest.id().to_string();
    (store, id)
}

#[test]
fn the_firmware_endpoints_begin_and_report_a_job() {
    let dir = firmware_dir("job");
    let bytes: Vec<u8> = (0..300u32).map(|i| (i % 251) as u8).collect();
    let (store, id) = signed_store(&dir, &bytes);

    let server =
        Server::bind("127.0.0.1:0".parse().expect("addr"), &topology(), "test").expect("bind");
    let addr = server.local_addr().expect("local addr");
    let handle = server.handle();
    let sink = TestSink::default();
    handle.ota().configure_store(store);
    handle.ota().set_metric_sink(Box::new(sink.clone()));
    let node = Arc::new(Mutex::new(FakeNode {
        expected: sha256(&bytes),
        received: Vec::new(),
    }));
    handle
        .ota()
        .register_transport("00:00:00:00:00:01", Box::new(TestTransport::new(node)));

    let mut guard = server.serve_in_background().expect("serve");

    // The store is served.
    let firmware = http_get(addr, "/api/v1/firmware");
    assert!(firmware.starts_with("HTTP/1.1 200 OK"), "{firmware}");
    assert!(firmware.contains(&id), "{firmware}");
    assert!(firmware.contains("\"target\":\"esp32s3\""), "{firmware}");

    // Begin the install.
    let body = format!("{{\"image_id\":\"{id}\"}}");
    let install = http_request(
        addr,
        &format!(
            "POST /api/v1/nodes/00:00:00:00:00:01/firmware/install HTTP/1.1\r\n\
             Host: localhost\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    );
    assert!(install.starts_with("HTTP/1.1 202 Accepted"), "{install}");
    assert!(install.contains("\"state\":\"succeeded\""), "{install}");
    assert!(install.contains("\"jobId\":\"fw-1\""), "{install}");

    // The job reports completion.
    let job = http_get(addr, "/api/v1/nodes/00:00:00:00:00:01/firmware/job");
    assert!(job.starts_with("HTTP/1.1 200 OK"), "{job}");
    assert!(job.contains("\"state\":\"succeeded\""), "{job}");
    assert!(job.contains("\"receivedBytes\":300"), "{job}");

    // The node firmware view carries the last result.
    let node_firmware = http_get(addr, "/api/v1/nodes/00:00:00:00:00:01/firmware");
    assert!(node_firmware.contains("\"last_result\""), "{node_firmware}");
    assert!(node_firmware.contains("\"succeeded\""), "{node_firmware}");

    // Both ledger rows were recorded (`firmware-ota.md` §9).
    let rows = sink.records();
    assert!(rows
        .iter()
        .any(|row| row.name.as_str() == "firmware.ota.result"));
    assert!(rows
        .iter()
        .any(|row| row.name.as_str() == "firmware.version.installed"));

    guard.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_install_without_a_transport_is_an_offline_node() {
    let dir = firmware_dir("offline");
    let bytes = vec![1u8, 2, 3, 4];
    let (store, id) = signed_store(&dir, &bytes);

    let server =
        Server::bind("127.0.0.1:0".parse().expect("addr"), &topology(), "test").expect("bind");
    let addr = server.local_addr().expect("local addr");
    server.handle().ota().configure_store(store);
    let mut guard = server.serve_in_background().expect("serve");

    let body = format!("{{\"image_id\":\"{id}\"}}");
    let response = http_request(
        addr,
        &format!(
            "POST /api/v1/nodes/00:00:00:00:00:01/firmware/install HTTP/1.1\r\n\
             Host: localhost\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    );
    assert!(response.starts_with("HTTP/1.1 409 Conflict"), "{response}");
    assert!(response.contains("node_offline"), "{response}");

    guard.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unknown_image_is_not_found() {
    let dir = firmware_dir("unknown");
    let (store, _id) = signed_store(&dir, &[9u8; 8]);
    let server =
        Server::bind("127.0.0.1:0".parse().expect("addr"), &topology(), "test").expect("bind");
    let addr = server.local_addr().expect("local addr");
    server.handle().ota().configure_store(store);
    let mut guard = server.serve_in_background().expect("serve");

    let body = "{\"image_id\":\"deadbeefdeadbeef\"}";
    let response = http_request(
        addr,
        &format!(
            "POST /api/v1/nodes/00:00:00:00:00:01/firmware/install HTTP/1.1\r\n\
             Host: localhost\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    );
    assert!(response.starts_with("HTTP/1.1 404"), "{response}");

    guard.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_store_rejects_a_bad_signature() {
    let dir = firmware_dir("badsig");
    let key = SigningKey::from_bytes(&[5u8; 32]);
    let store = FirmwareStore::open(&dir, key.verifying_key()).expect("open");
    let bytes = vec![1u8, 2, 3, 4, 5];
    let mut signature = key.sign(&sha256(b"different")).to_bytes();
    signature[10] ^= 0xFF;
    let result = store.import(
        FirmwareCompat::encode(1, 0),
        "esp32s3",
        ImportRequest {
            image: bytes,
            signature,
            version: "1.0.0".to_string(),
            target: "esp32s3".to_string(),
            compatibility: FirmwareCompat::new(
                FirmwareCompat::encode(1, 0),
                FirmwareCompat::encode(1, 9),
            ),
            notes: String::new(),
        },
    );
    assert!(matches!(result, Err(StoreError::BadSignature)));
    assert!(store.list().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
