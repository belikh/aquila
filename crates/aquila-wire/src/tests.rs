//! Unit and property tests for `aquila-wire`.
//!
//! The suite is the pin on the on-air layout: a fixed golden vector fixes the
//! exact byte offsets, round-trips pin the codec, proptest pins the codec over
//! generated frames, and the malformed-buffer tests pin the typed-error surface
//! (the decoders must never panic).

use aquila_types::{Band, CsiFrame, MacAddr, NodeId, PhyFlags};
use proptest::prelude::*;

use crate::constants::{Opcode, MAX_SUBCARRIERS};
use crate::*;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// The representative frame the golden vector pins: two subcarriers, every
/// field chosen so the byte offsets are unambiguous.
fn golden_frame() -> CsiFrame {
    CsiFrame {
        node_id: NodeId(0x0102),
        boot_epoch: 7,
        sequence: 0x0102_0304_0506_0708,
        device_time_us: 0x1122_3344_5566_7788,
        band: Band::B2G4,
        channel: 6,
        bandwidth_mhz: 20,
        subcarrier_offset: -3,
        i_q: vec![1, 2, 3, 4],
        rssi_dbm: -42,
        noise_floor_dbm: -95,
        phy: PhyFlags(PhyFlags::HT),
        first_word_invalid: false,
    }
}

/// The exact `AQCI` datagram for [`golden_frame`], laid out byte for byte.
///
/// 16-byte header, 28-byte fixed body, then the interleaved signed int8 I/Q
/// sample (`1, 2, 3, 4`).
const GOLDEN: [u8; 48] = [
    // header
    0x41, 0x51, 0x43, 0x49, // magic "AQCI"
    0x01, 0x00, // wire major 1, minor 0
    0x01, 0x01, // message type 1, subcarrier format 1 (int8 I/Q)
    0x02, 0x01, // node_id 0x0102 (LE)
    0x20, 0x00, // payload_len 32 (LE)
    0x00, // band 2.4 GHz
    0x06, // channel 6
    0x14, // bandwidth 20 MHz
    0x00, // reserved
    // fixed body
    0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, // sequence
    0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, // device_time_us
    0x07, 0x00, // boot_epoch 7
    0xFD, 0xFF, // subcarrier_offset -3
    0x02, 0x00, // n_subcarriers 2
    0xD6, // rssi_dbm -42
    0xA1, // noise_floor_dbm -95
    0x01, 0x00, // phy_flags HT
    0x00, 0x00, // reserved
    // interleaved int8 I/Q: 1, 2, 3, 4
    0x01, 0x02, 0x03, 0x04,
];

/// A representative live-looking frame: 64 subcarriers at 20 MHz HT.
fn representative_frame() -> CsiFrame {
    let n = 64;
    CsiFrame {
        node_id: NodeId(17),
        boot_epoch: 4,
        sequence: 9_001,
        device_time_us: 5_000_123,
        band: Band::B2G4,
        channel: 1,
        bandwidth_mhz: 20,
        subcarrier_offset: -32,
        i_q: (0..2 * n)
            .map(|i| ((i * 7 + 11) % 256) as u8 as i8)
            .collect(),
        rssi_dbm: -38,
        noise_floor_dbm: -92,
        phy: PhyFlags(PhyFlags::HT | PhyFlags::SHORT_GI),
        first_word_invalid: false,
    }
}

// ---------------------------------------------------------------------------
// Golden vector
// ---------------------------------------------------------------------------

#[test]
fn golden_layout_is_pinned() {
    let frame = golden_frame();
    assert_eq!(
        encode_datagram(&frame).as_slice(),
        &GOLDEN[..],
        "encode output drifted from the pinned layout"
    );
    assert_eq!(GOLDEN.len(), 48, "golden datagram length");
    assert_eq!(
        decode_datagram(&GOLDEN).expect("decode golden"),
        frame,
        "golden datagram does not decode to its frame"
    );
    assert_eq!(datagram_len(2), Some(48));
    // Header offset 15 is the flags byte; a clean frame leaves it zero.
    assert_eq!(GOLDEN[15], 0, "the flags byte must be zero when clean");
}

#[test]
fn first_word_invalid_rides_the_flags_byte() {
    let mut frame = golden_frame();
    frame.first_word_invalid = true;
    let bytes = encode_datagram(&frame);
    assert_eq!(
        bytes[15], CSI_FLAG_FIRST_WORD_INVALID,
        "bit 0 of the header flags byte carries first_word_invalid"
    );
    let decoded = decode_datagram(&bytes).expect("decode flagged");
    assert!(decoded.first_word_invalid);
    // Clearing the flag clears the bit and round-trips.
    frame.first_word_invalid = false;
    let bytes = encode_datagram(&frame);
    assert_eq!(bytes[15], 0);
    assert!(!decode_datagram(&bytes).expect("decode clean").first_word_invalid);
}

// ---------------------------------------------------------------------------
// Round-trip
// ---------------------------------------------------------------------------

fn assert_round_trips(frame: &CsiFrame) {
    let bytes = encode_datagram(frame);
    let decoded = decode_datagram(&bytes).expect("decode");
    assert_eq!(&decoded, frame, "round trip mismatch");
    assert_eq!(decoded.n_subcarriers(), frame.n_subcarriers());
}

#[test]
fn representative_frames_round_trip() {
    assert_round_trips(&golden_frame());
    assert_round_trips(&representative_frame());

    // A single-subcarrier frame is the smallest legal frame.
    let mut tiny = golden_frame();
    tiny.i_q = vec![7, -7];
    assert_round_trips(&tiny);

    // Unknown band and an unrecognised band byte both survive.
    let mut unknown = representative_frame();
    unknown.band = Band::Unknown;
    assert_round_trips(&unknown);

    // The maximum subcarrier count fits and round-trips.
    let n = MAX_SUBCARRIERS as usize;
    let mut max = representative_frame();
    max.i_q = vec![1; 2 * n];
    assert_round_trips(&max);
    assert_eq!(datagram_len(n), Some(16 + 28 + 2 * n));
}

#[test]
fn over_budget_frame_is_clamped_not_panicked() {
    let n = MAX_SUBCARRIERS as usize + 32;
    let mut frame = representative_frame();
    frame.i_q = vec![1; 2 * n];

    let decoded = decode_datagram(&encode_datagram(&frame)).expect("decode clamped");
    assert_eq!(decoded.n_subcarriers(), MAX_SUBCARRIERS as usize);
}

#[test]
fn odd_sample_is_truncated_not_panicked() {
    // An odd-length interleaved sample is not whole I/Q pairs; the encoder
    // clamps to the whole pairs it can carry and the decoder round-trips that.
    let mut frame = representative_frame();
    let pairs = frame.n_subcarriers();
    frame.i_q.push(7);

    let decoded = decode_datagram(&encode_datagram(&frame)).expect("decode truncated");
    assert_eq!(decoded.n_subcarriers(), pairs);
    assert_eq!(decoded.i_q, frame.i_q[..2 * pairs]);
}

// ---------------------------------------------------------------------------
// Malformed input: typed errors, never panics
// ---------------------------------------------------------------------------

#[test]
fn empty_and_short_buffers_are_truncated() {
    assert_eq!(
        decode_datagram(&[]),
        Err(WireError::Truncated {
            needed: 4,
            actual: 0
        })
    );
    match decode_datagram(&GOLDEN[..10]) {
        Err(WireError::Truncated { .. }) => {}
        other => panic!("expected Truncated, got {other:?}"),
    }
}

#[test]
fn bad_magic_is_rejected() {
    let mut bytes = GOLDEN;
    bytes[0] = b'Z';
    assert_eq!(
        decode_datagram(&bytes),
        Err(WireError::BadMagic {
            expected: *b"AQCI",
            found: *b"ZQCI",
        })
    );
}

#[test]
fn bad_major_version_is_rejected_but_minor_is_accepted() {
    let mut bad_major = GOLDEN;
    bad_major[4] = 2;
    assert_eq!(
        decode_datagram(&bad_major),
        Err(WireError::UnsupportedVersion {
            major: 2,
            expected: 1
        })
    );

    // A different minor is forward-compatible and still decodes.
    let mut new_minor = GOLDEN;
    new_minor[5] = 9;
    assert_eq!(
        decode_datagram(&new_minor).expect("minor accepted"),
        golden_frame()
    );
}

#[test]
fn bad_message_type_and_format_are_rejected() {
    let mut msg = GOLDEN;
    msg[6] = 2;
    assert_eq!(decode_datagram(&msg), Err(WireError::UnknownMessageType(2)));

    let mut fmt = GOLDEN;
    fmt[7] = 0;
    assert_eq!(
        decode_datagram(&fmt),
        Err(WireError::UnknownSubcarrierFormat(0))
    );
}

#[test]
fn inconsistent_payload_length_is_rejected() {
    let mut bytes = GOLDEN;
    bytes[10] = 0x1E; // declare 30, but n = 2 implies 32
    assert_eq!(
        decode_datagram(&bytes),
        Err(WireError::LengthMismatch {
            declared: 30,
            expected: 32
        })
    );
}

#[test]
fn zero_subcarriers_are_rejected() {
    let mut bytes = GOLDEN;
    bytes[10] = 0x1C; // payload_len 28
    bytes[36] = 0x00;
    bytes[37] = 0x00;
    assert!(matches!(
        decode_datagram(&bytes),
        Err(WireError::SubcarrierCount { count: 0, .. })
    ));
}

#[test]
fn trailing_bytes_are_forward_compatible() {
    let mut bytes = GOLDEN.to_vec();
    bytes.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
    assert_eq!(
        decode_datagram(&bytes).expect("trailing ok"),
        golden_frame()
    );
}

#[test]
fn decode_never_panics_on_arbitrary_bytes() {
    for len in 0..GOLDEN.len() + 8 {
        let _ = decode_datagram(&GOLDEN[..len.min(GOLDEN.len())]);
        let _ = decode_datagram(&vec![0xFFu8; len]);
        let _ = decode_control(&vec![0xABu8; len]);
    }
    // Mutating any single byte must not panic either.
    for i in 0..GOLDEN.len() {
        let mut bytes = GOLDEN;
        bytes[i] ^= 0xFF;
        let _ = decode_datagram(&bytes);
    }
}

// ---------------------------------------------------------------------------
// Control plane
// ---------------------------------------------------------------------------

fn sample_mac(n: u8) -> MacAddr {
    MacAddr([0x02, 0x00, 0x00, 0x00, 0x00, n])
}

fn control_fixtures() -> Vec<ControlMessage> {
    vec![
        ControlMessage {
            request_id: 1,
            body: ControlBody::JoinRequest(JoinRequest {
                node: sample_mac(1),
                boot: 0x1122,
                pairing_code: 0x0000_ABCD,
                fw_version: "aquila-node 1.0.0".to_string(),
            }),
        },
        ControlMessage {
            request_id: 2,
            body: ControlBody::JoinAck(JoinAck {
                node: sample_mac(2),
                token: [0x42; 32],
                config_version: 3,
                config_hash: [0x7F; 32],
            }),
        },
        result_message(3, AckStatus::Rejected, "config hash mismatch"),
        ControlMessage {
            request_id: 4,
            body: ControlBody::Status(Status {
                uptime_ms: 1_234_567,
                boot: 9,
                last_sequence: 88_000,
                dropped: 12,
                ring_depth: 7,
                temperature_c_x100: 4_150,
                config_version: 3,
                last_rtt_us: 812,
            }),
        },
        ControlMessage {
            request_id: 5,
            body: ControlBody::GetConfig,
        },
        ControlMessage {
            request_id: 6,
            body: ControlBody::SetConfig(SetConfig {
                config_version: 4,
                config_hash: [0x11; 32],
                channel: 36,
                bandwidth_mhz: 40,
                secondary: 1,
                csi_enable: true,
                lltf_enable: true,
                ht_ltf_enable: false,
                stbc_ht_ltf_enable: false,
                csi_len_cap: 612,
                max_tx_rate_hz: 200,
                heartbeat_ms: 5_000,
                source_mac_allowlist: vec![sample_mac(3), sample_mac(4)],
            }),
        },
        ControlMessage {
            request_id: 7,
            body: ControlBody::SetCsi(SetCsi { enable: false }),
        },
        ControlMessage {
            request_id: 8,
            body: ControlBody::SyncPing(SyncPing {
                t1_us: 1_700_000_000_000_000,
            }),
        },
        ControlMessage {
            request_id: 9,
            body: ControlBody::SyncEcho(SyncEcho {
                t1_us: 1_700_000_000_000_000,
                t2_us: 1_700_000_000_000_500,
                t3_us: 1_700_000_000_000_520,
            }),
        },
        ControlMessage {
            request_id: 10,
            body: ControlBody::Reboot,
        },
        ControlMessage {
            request_id: 11,
            body: ControlBody::Identify,
        },
        ControlMessage {
            request_id: 12,
            body: ControlBody::FactoryReset,
        },
        ControlMessage {
            request_id: 13,
            body: ControlBody::Event(EventNotice {
                code: 2,
                detail: "ring overflow".to_string(),
            }),
        },
    ]
}

#[test]
fn control_messages_round_trip() {
    for message in control_fixtures() {
        let bytes = encode_control(&message).expect("encode");
        let decoded = decode_control(&bytes).expect("decode");
        assert_eq!(decoded, message, "control round trip mismatch");
        assert_eq!(
            message_mac(&decoded).is_some(),
            matches!(
                message.body,
                ControlBody::JoinRequest(_) | ControlBody::JoinAck(_)
            )
        );
    }
}

#[test]
fn control_opcodes_are_stable() {
    assert_eq!(Opcode::JoinRequest.to_u8(), 0x01);
    assert_eq!(Opcode::Event.to_u8(), 0x0D);
    assert_eq!(Opcode::from_u8(0x0D), Some(Opcode::Event));
    assert_eq!(Opcode::from_u8(0x7F), None);
    for m in control_fixtures() {
        assert_eq!(
            Opcode::from_u8(m.body.opcode().to_u8()),
            Some(m.body.opcode())
        );
    }
}

#[test]
fn control_malformed_buffers_are_typed_errors() {
    let good = encode_control(&control_fixtures()[0]).expect("encode");

    let mut bad_magic = good.clone();
    bad_magic[0] = b'Z';
    assert!(matches!(
        decode_control(&bad_magic),
        Err(WireError::BadMagic { .. })
    ));

    let mut bad_version = good.clone();
    bad_version[4] = 2;
    assert!(matches!(
        decode_control(&bad_version),
        Err(WireError::UnsupportedVersion { major: 2, .. })
    ));

    let mut bad_opcode = good.clone();
    bad_opcode[5] = 0x7F;
    assert_eq!(
        decode_control(&bad_opcode),
        Err(WireError::UnknownOpcode(0x7F))
    );

    assert!(matches!(
        decode_control(&good[..8]),
        Err(WireError::Truncated { .. })
    ));

    let mut trailing = good;
    trailing.push(0x00);
    assert_eq!(
        decode_control(&trailing),
        Err(WireError::UnexpectedTrailingBytes { extra: 1 })
    );
}

#[test]
fn control_body_length_mismatch_is_rejected() {
    // A JOIN_ACK body declares 74 bytes; corrupt the default body by declaring
    // 75 in the header while providing 74.
    let mut message = ControlMessage {
        request_id: 1,
        body: ControlBody::JoinAck(JoinAck {
            node: sample_mac(1),
            token: [0; 32],
            config_version: 1,
            config_hash: [0; 32],
        }),
    };
    let mut bytes = encode_control(&message).expect("encode");
    let declared = u16::from_le_bytes([bytes[6], bytes[7]]);
    bytes[6..8].copy_from_slice(&(declared + 1).to_le_bytes());
    assert!(matches!(
        decode_control(&bytes),
        Err(WireError::UnexpectedTrailingBytes { .. } | WireError::Truncated { .. })
    ));

    // A body with surplus bytes inside the declared length is a length mismatch.
    message.body = ControlBody::SetCsi(SetCsi { enable: true });
    let mut bytes = encode_control(&message).expect("encode");
    let declared = u16::from_le_bytes([bytes[6], bytes[7]]);
    bytes[6..8].copy_from_slice(&(declared + 4).to_le_bytes());
    bytes.extend_from_slice(&[0, 0, 0, 0]);
    assert_eq!(
        decode_control(&bytes),
        Err(WireError::BodyLengthMismatch {
            opcode: Opcode::SetCsi.to_u8(),
            declared: 5,
            consumed: 1
        })
    );
}

#[test]
fn control_encode_limits_are_typed_errors() {
    let too_long = ControlMessage {
        request_id: 1,
        body: ControlBody::Event(EventNotice {
            code: 0,
            detail: "x".repeat(300),
        }),
    };
    assert!(matches!(
        encode_control(&too_long),
        Err(WireError::StringTooLong { .. })
    ));

    let too_many = ControlMessage {
        request_id: 1,
        body: ControlBody::SetConfig(SetConfig {
            config_version: 1,
            config_hash: [0; 32],
            channel: 1,
            bandwidth_mhz: 20,
            secondary: 0,
            csi_enable: true,
            lltf_enable: false,
            ht_ltf_enable: false,
            stbc_ht_ltf_enable: false,
            csi_len_cap: 0,
            max_tx_rate_hz: 0,
            heartbeat_ms: 0,
            source_mac_allowlist: vec![sample_mac(1); 256],
        }),
    };
    assert!(matches!(
        encode_control(&too_many),
        Err(WireError::TooManyMacs { .. })
    ));
}

// ---------------------------------------------------------------------------
// Control-plane authentication (capture-and-firmware.md §6.3)
// ---------------------------------------------------------------------------

#[test]
fn authenticated_control_round_trips_with_an_extra_trailer() {
    let token = [0x5Au8; 32];
    for message in control_fixtures() {
        let plain = encode_control(&message).expect("plain encode");
        let authed = encode_control_authenticated(&message, &token).expect("auth encode");
        assert_eq!(authed.len(), plain.len() + CONTROL_AUTH_LEN);
        assert_eq!(&authed[..plain.len()], plain.as_slice());
        assert_eq!(
            decode_control_authenticated(&authed, &token).expect("auth decode"),
            message
        );
    }
}

#[test]
fn authenticated_control_rejects_tampering_wrong_key_and_plain_frames() {
    let token = [0x5Au8; 32];
    let message = control_fixtures()[5].clone(); // SET_CONFIG
    let authed = encode_control_authenticated(&message, &token).expect("encode");

    // A plain (untagged) frame is not accepted as authenticated.
    let plain = encode_control(&message).expect("plain");
    assert_eq!(
        decode_control_authenticated(&plain, &token),
        Err(WireError::AuthenticationFailed)
    );

    // A tampered body changes the tag input.
    let mut tampered = authed.clone();
    tampered[CONTROL_HEADER_LEN + 4] ^= 0xFF;
    assert_eq!(
        decode_control_authenticated(&tampered, &token),
        Err(WireError::AuthenticationFailed)
    );

    // A tampered trailer fails.
    let mut bad_tag = authed.clone();
    let last = bad_tag.len() - 1;
    bad_tag[last] ^= 0x01;
    assert_eq!(
        decode_control_authenticated(&bad_tag, &token),
        Err(WireError::AuthenticationFailed)
    );

    // A different token fails.
    assert_eq!(
        decode_control_authenticated(&authed, &[0x00u8; 32]),
        Err(WireError::AuthenticationFailed)
    );

    // A buffer shorter than the trailer cannot be split.
    assert!(matches!(
        decode_control_authenticated(&authed[..CONTROL_AUTH_LEN - 1], &token),
        Err(WireError::Truncated { .. })
    ));
}

// ---------------------------------------------------------------------------
// Identity mapping
// ---------------------------------------------------------------------------

#[test]
fn identity_map_translates_both_ways() {
    let mut map = NodeIdentityMap::new();
    assert!(map.is_empty());

    let mac = sample_mac(9);
    let node = NodeId(9);
    map.insert(mac, node).expect("insert");
    assert_eq!(map.node_id(mac), Some(node));
    assert_eq!(map.mac(node), Some(mac));
    assert!(map.contains_mac(mac));
    assert!(map.contains_node(node));
    assert_eq!(map.len(), 1);

    // Idempotent re-insert succeeds; conflicting insert is rejected.
    map.insert(mac, node).expect("re-insert same");
    assert_eq!(
        map.insert(mac, NodeId(10)),
        Err(WireError::IdentityConflict { kind: "mac" })
    );
    assert_eq!(
        map.insert(sample_mac(10), node),
        Err(WireError::IdentityConflict { kind: "node id" })
    );

    assert_eq!(map.remove_mac(mac), Some(node));
    assert!(map.is_empty());
    assert_eq!(map.node_id(mac), None);
    assert_eq!(map.mac(node), None);
}

#[test]
fn unknown_identity_lookups_are_none() {
    let map = NodeIdentityMap::new();
    assert_eq!(map.node_id(sample_mac(1)), None);
    assert_eq!(map.mac(NodeId(1)), None);
}

// ---------------------------------------------------------------------------
// Constants and the generated C header
// ---------------------------------------------------------------------------

#[test]
fn ports_and_limits_are_the_registered_ones() {
    assert_eq!(CSI_DATA_PORT, 9100);
    assert_eq!(CONTROL_PORT, 9101);
    assert_eq!(CSI_MAGIC, *b"AQCI");
    assert_eq!(CONTROL_MAGIC, *b"AQCC");
    assert_eq!(MAX_SUBCARRIERS, 714);
}

#[test]
fn generated_c_header_pins_the_layout() {
    let header = generate_c_header();
    for needle in [
        "#define AQ_WIRE_MAJOR 1u",
        "#define AQ_CSI_MAGIC_0 0x41u",
        "#define AQ_CSI_HEADER_LEN 16u",
        "#define AQ_CSI_BODY_FIXED_LEN 28u",
        "#define AQ_CSI_MAX_SUBCARRIERS 714u",
        "#define AQ_CSI_OFF_MAGIC 0u",
        "#define AQ_CSI_OFF_FLAGS 15u",
        "#define AQ_CSI_FLAG_FIRST_WORD_INVALID 1u",
        "#define AQ_CSI_BODY_OFF_IQ 28u",
        "#define AQ_SUBCARRIER_FORMAT_INT8_IQ 1u",
        "#define AQ_OP_SYNC_ECHO 0x09u",
        "#define AQ_CONTROL_AUTH_LEN 32u",
        "#define AQ_CSI_DATA_PORT 9100u",
        "_Static_assert(sizeof(aq_csi_header_t) == AQ_CSI_HEADER_LEN",
        "#define AQ_CSI_FRAME_SIZE(n_subcarriers)",
        "int8_t payload[];",
        "#define AQ_CSI_IQ(body)",
        "_Static_assert(offsetof(aq_csi_body_t, payload) == AQ_CSI_BODY_FIXED_LEN",
    ] {
        assert!(
            header.contains(needle),
            "generated header is missing {needle:?}"
        );
    }
    // Deterministic output.
    assert_eq!(header, generate_c_header());
}

// ---------------------------------------------------------------------------
// Property: round-trip over generated frames
// ---------------------------------------------------------------------------

prop_compose! {
    fn arb_frame_n(n: usize)
        (node_id: u16,
         boot_epoch: u16,
         sequence: u64,
         device_time_us: u64,
         band in prop_oneof![Just(Band::B2G4), Just(Band::B5G), Just(Band::Unknown)],
         channel: u8,
         bandwidth_mhz: u8,
         subcarrier_offset: i16,
         i_q in prop::collection::vec(any::<i8>(), 2 * n..=2 * n),
         rssi_dbm: i8,
         noise_floor_dbm: i8,
         phy: u16,
         first_word_invalid: bool)
        -> CsiFrame {
        CsiFrame {
            node_id: NodeId(node_id),
            boot_epoch,
            sequence,
            device_time_us,
            band,
            channel,
            bandwidth_mhz,
            subcarrier_offset,
            i_q,
            rssi_dbm,
            noise_floor_dbm,
            phy: PhyFlags(phy),
            first_word_invalid,
        }
    }
}

fn arb_frame() -> impl Strategy<Value = CsiFrame> {
    (1usize..=MAX_SUBCARRIERS as usize).prop_flat_map(arb_frame_n)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn datagram_round_trips_any_valid_frame(frame in arb_frame()) {
        let bytes = encode_datagram(&frame);
        prop_assert_eq!(bytes.len(), datagram_len(frame.n_subcarriers()).unwrap());
        prop_assert!(bytes.len() <= MAX_DATAGRAM_LEN);
        let decoded = decode_datagram(&bytes).expect("decode");
        prop_assert_eq!(decoded, frame);
    }

    #[test]
    fn control_round_trips_any_message(index in 0usize..13usize) {
        let message = control_fixtures()[index].clone();
        let bytes = encode_control(&message).expect("encode");
        prop_assert_eq!(decode_control(&bytes).expect("decode"), message);
    }
}
