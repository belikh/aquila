//! `aquila-api` tests: the control plane is exercised over real loopback
//! sockets.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use aquila_geometry::Topology;
use aquila_sensing::Sink;
use aquila_types::{
    DeviceType, Event, EventKind, EventPayload, EventScope, FirmwareRef, Floor, Home, MacAddr,
    Node, NodeId, Room, RoomKind, Slug, Witness, CONTRACT_VERSION,
};

use crate::{ApiError, Auth, ChannelSink, Server};

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
