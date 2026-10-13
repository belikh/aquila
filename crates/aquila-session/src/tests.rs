//! Unit tests for the `.aqs` container.
//!
//! These pin the round trip, the canonical manifest, the frames iterator, and
//! the typed failure surface: a corrupted session must fail loudly, never read
//! back as a plausible but wrong stream.

use aquila_types::{BuildId, ContractVersion, NodeId, Session, Slug};

use crate::*;

/// A minimal valid session for container tests.
fn demo_session() -> Session {
    Session {
        id: Slug("demo-session".into()),
        name: "Demo session".into(),
        home: Slug("demo-home".into()),
        nodes: vec![NodeId(1), NodeId(2)],
        started_unix_ms: 1_700_000_000_000,
        ended_unix_ms: Some(1_700_000_060_000),
        contract: ContractVersion::new(1, 0),
        build: BuildId {
            git_sha: "deadbeefdeadbeef".into(),
            dirty: false,
        },
        config_hash: [0u8; 32],
        seed: 42,
        artifacts: vec![],
        ground_truth: None,
        notes: "fixture".into(),
    }
}

/// A manifest with a fixed creation time, so its bytes are reproducible.
fn demo_manifest() -> Manifest {
    Manifest::new(demo_session())
}

/// A small but structurally valid `AQCI` datagram, produced by the real codec.
fn demo_datagram(node: u16, sequence: u64) -> Vec<u8> {
    let frame = aquila_types::CsiFrame {
        node_id: NodeId(node),
        boot_epoch: 1,
        sequence,
        device_time_us: sequence * 1_000,
        band: aquila_types::Band::B2G4,
        channel: 6,
        bandwidth_mhz: 20,
        subcarrier_offset: -1,
        amplitude: vec![1.0, 2.0],
        phase: vec![0.0, -0.5],
        rssi_dbm: -40,
        noise_floor_dbm: -90,
        phy: aquila_types::PhyFlags(aquila_types::PhyFlags::HT),
    };
    aquila_wire::encode_datagram(&frame)
}

/// Build a container with a CSI record, a ground-truth record and an annotation.
fn demo_container() -> Vec<u8> {
    let mut writer = SessionWriter::new(&demo_manifest()).unwrap();
    writer.write_frame(1_000, &demo_datagram(1, 0)).unwrap();
    writer.write_frame(2_000, &demo_datagram(2, 0)).unwrap();
    writer
        .write_ground_truth(2_000, r#"{"presence":true,"count":1}"#)
        .unwrap();
    writer
        .write_record(&Record::Annotation {
            json: r#"{"note":"kitchen"}"#.into(),
        })
        .unwrap();
    writer.finish().unwrap()
}

#[test]
fn round_trip_preserves_records_and_manifest() {
    let bytes = demo_container();
    let reader = SessionReader::from_bytes(&bytes).unwrap();
    assert_eq!(reader.version(), CONTAINER_VERSION);
    assert_eq!(reader.manifest(), &demo_manifest());
    assert_eq!(reader.len(), 4);
    assert_eq!(reader.records()[0].kind(), RecordKind::Csi);
    assert_eq!(reader.records()[2].kind(), RecordKind::GroundTruth);
    assert_eq!(reader.records()[3].kind(), RecordKind::Annotation);

    let frames: Vec<(u64, Vec<u8>)> = reader
        .frames()
        .map(|(us, datagram)| (us, datagram.to_vec()))
        .collect();
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0], (1_000, demo_datagram(1, 0)));
    assert_eq!(frames[1], (2_000, demo_datagram(2, 0)));

    let frame = aquila_wire::decode_datagram(&frames[1].1).unwrap();
    assert_eq!(frame.node_id, NodeId(2));
}

#[test]
fn verify_accepts_a_fresh_container() {
    let bytes = demo_container();
    let report = verify_bytes(&bytes).unwrap();
    assert_eq!(report.record_count, 4);
    assert_eq!(report.frames, 2);
    assert_eq!(report.session_id, "demo-session");
    assert_eq!(report.version, CONTAINER_VERSION);

    let reader = SessionReader::from_bytes(&bytes).unwrap();
    assert_eq!(reader.verify().unwrap(), report);
    assert_eq!(reader.trailer_digest().record_count, 4);
}

#[test]
fn verify_rejects_a_corrupted_record_stream() {
    let mut bytes = demo_container();
    // Flip a byte inside the first CSI record's datagram, leaving every length
    // field and JSON body intact.
    let metadata_len = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let datagram_start = HEADER_FIXED_LEN + metadata_len + RECORD_PREFIX_LEN + 8;
    bytes[datagram_start] ^= 0xff;

    // The corruption is structurally invisible, so parsing still succeeds…
    let reader = SessionReader::from_bytes(&bytes).unwrap();
    // …but verification catches it, typed.
    let err = reader.verify().unwrap_err();
    assert_eq!(err.kind(), "digest_mismatch");
    assert!(matches!(err, SessionError::DigestMismatch { .. }));
    assert_eq!(verify_bytes(&bytes).unwrap_err().kind(), "digest_mismatch");
}

#[test]
fn verify_rejects_a_corrupted_trailer_count() {
    let mut bytes = demo_container();
    let count_offset = bytes.len() - TRAILER_LEN + 3;
    bytes[count_offset] = bytes[count_offset].wrapping_add(1);
    let reader = SessionReader::from_bytes(&bytes).unwrap();
    let err = reader.verify().unwrap_err();
    assert_eq!(err.kind(), "record_count_mismatch");
}

#[test]
fn rejects_bad_magic_and_version() {
    let mut bytes = demo_container();
    bytes[0] = b'X';
    assert_eq!(
        SessionReader::from_bytes(&bytes).unwrap_err().kind(),
        "bad_magic"
    );

    let mut bytes = demo_container();
    bytes[3] = 9;
    assert_eq!(
        SessionReader::from_bytes(&bytes).unwrap_err().kind(),
        "unsupported_version"
    );
}

#[test]
fn rejects_truncation() {
    let bytes = demo_container();
    let short = &bytes[..HEADER_FIXED_LEN + TRAILER_LEN - 1];
    assert_eq!(
        SessionReader::from_bytes(short).unwrap_err().kind(),
        "truncated"
    );
}

#[test]
fn rejects_unknown_record_type() {
    let mut writer = SessionWriter::new(&demo_manifest()).unwrap();
    writer.write_frame(0, &demo_datagram(1, 0)).unwrap();
    let mut bytes = writer.finish().unwrap();
    // The first record's type byte sits just after header + metadata.
    let metadata_len = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let first_record_type = HEADER_FIXED_LEN + metadata_len;
    bytes[first_record_type] = 0x7f;
    assert_eq!(
        SessionReader::from_bytes(&bytes).unwrap_err().kind(),
        "unknown_record_type"
    );
}

#[test]
fn digest_is_stable_for_identical_input() {
    assert_eq!(demo_container(), demo_container());
    let report = verify_bytes(&demo_container()).unwrap();
    assert_eq!(report.digest, verify_bytes(&demo_container()).unwrap().digest);
}

#[test]
fn manifest_json_is_canonical_sorted() {
    let json = demo_manifest().to_canonical_json().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&json).unwrap();
    let object = value.as_object().unwrap();
    let keys: Vec<&String> = object.keys().collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "manifest keys must be sorted");
}

#[test]
fn empty_container_round_trips() {
    let writer = SessionWriter::new(&demo_manifest()).unwrap();
    assert_eq!(writer.record_count(), 0);
    let bytes = writer.finish().unwrap();
    assert_eq!(bytes.len(), HEADER_FIXED_LEN + TRAILER_LEN + {
        demo_manifest().to_canonical_json().unwrap().len()
    });
    let reader = SessionReader::from_bytes(&bytes).unwrap();
    assert!(reader.is_empty());
    assert_eq!(reader.verify().unwrap().frames, 0);
}

#[test]
fn parse_manifest_matches_the_header() {
    let bytes = demo_container();
    let metadata_len = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let parsed = parse_manifest(&bytes[HEADER_FIXED_LEN..HEADER_FIXED_LEN + metadata_len]).unwrap();
    assert_eq!(parsed, demo_manifest());
}
