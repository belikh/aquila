# Capture and firmware — specification

Status: draft for review (v1).

Owner: capture & firmware author.

Scope: the two ends of the sensing pipeline that touch hardware — the
`CsiSource` seam and its adapters, the ESP32-S3 node firmware, the CSI data
plane and the node control channel, the deterministic simulator, and the
Session (recorded-fixture) format. The DSP, inference, fusion, calibration and
measurement are specified elsewhere; this document defines only how CSI frames
enter the core and how they are produced, transported, simulated and recorded.

This document is subordinate to `CONTEXT.md` (canonical vocabulary) and to
`docs/spec/roadmap.md` (sequence and seams). Where they conflict, they win and
this document is wrong. Interfaces named here become binding only when ratified
by an ADR.

## 1. Vocabulary and non-goals

Terms are used exactly as `CONTEXT.md` defines them:

- **CSI frame** — one received 802.11 packet's complex channel state
  information from a single node: a complex coefficient per subcarrier plus
  capture metadata. The raw wireless observation.
- **Node** — a single fixed ESP32-S3 endpoint that streams raw CSI. A dumb
  sensor: it performs no DSP, inference or fusion. Identified by a stable node
  id and assigned to exactly one room.
- **Session** — a recorded, replayable, time-bounded stream of CSI frames from
  one or more nodes, with metadata and provenance. Sessions drive development
  and scoring ahead of live deployment. A "recorded fixture" is a Session used
  in tests.
- **Node-pair** — an ordered pair of nodes whose CSI frames are jointly
  processed. The edge of the sensing graph.
- **Ground truth** — an independent reference measurement (for example a
  reference breath belt). Research-only; never a runtime input.
- **Baseline** — the node-pair or zone reference state recorded with no person
  present.

Explicitly not used here, per `CONTEXT.md`: ESP-NOW, MQTT, Matter, Zigbee, BLE
mesh, PoE; node roles (access point, station/client, gateway, mesh router);
public REST/WS/MCP, SDK, plugins; raw I/Q, SDR, Doppler, phase-coherent RF
reference; per-room or per-floor calibration.

Non-goals for this document:

- Active sensing. v1 nodes do not emit elicitation waveforms, select
  subcarriers or steer. Any transmit/steering approach is research-only
  (`docs/research/research-tracks.md`, Track 2) and would arrive as a new
  control-channel action, not a v1 capture feature.
- Raw I/Q. Capture is esp-csi CSI only. ESP32-as-SDR raw I/Q is research-only
  (`docs/research/esp32-as-sdr.md`) and out of scope.
- On-node signal processing of any kind.

## 2. Topology and the two planes

A home has ~9+ nodes over three floors. Every node and the core sit on one
private LAN. There are exactly **two** network planes between nodes and the
core, and they are distinct protocols on distinct ports:

| Plane | Direction | Transport | Content |
| --- | --- | --- | --- |
| CSI data plane | node → core | UDP unicast | one CSI frame per datagram |
| Node control plane | core ↔ node | UDP request/response | pairing, config, commands, time-sync, heartbeat |

Neither plane is a public surface. The core's other plane — its **private
HTTP + WS** control plane to Home Assistant and the dashboard — is specified in
`docs/spec/dashboard.md` and is not a node protocol.

**ESP-NOW is dropped.** It is a separate, vendor-specific link layer that does
not route across a multi-floor home LAN, ties the design to one silicon vendor,
and would make the node a peer in a mesh rather than a dumb sensor. Dropped for
v1 and reserved in `CONTEXT.md` as never-used.

**MQTT is not used.** A broker adds a third process, a message envelope and
retained-state semantics to a high-rate, lossy, one-way sensor stream that does
not need any of them. The control plane is the core's own private HTTP + WS and
the direct node control channel; there is no broker in the path.

## 3. The `CsiSource` seam

`CsiSource` is the one seam where "where CSI comes from" is decided. It is owned
by the `aquila-sources` crate (roadmap). The seam MUST be satisfied by at least
three adapters — simulator, recorded-replay, live-UDP — so that replay and live
are genuinely interchangeable and the seam is real, not speculative.

### 3.1 Types

```rust
// aquila-sources

use aquila_types::{CsiEnvelope, CsiFrame};

// The seam yields the one canonical contract frame, `aquila_types::CsiFrame`
// (raw signed int8 I/Q, interleaved `Q, I` — **imaginary first**, `[Q0, I0, …]`
// — `data-contracts.md` §2, ADR-0005/ADR-0006), wrapped with host arrival data
// on `aquila_types::CsiEnvelope`. There is **no** second `CsiFrame` at the seam;
// the firmware emits the wire form and does not own a competing struct.
//
// Node identity on the wire is the dense `NodeId(u16)`; the ESP32-S3 factory
// `MacAddr` is mapped to and from it at the seam by `aquila-wire`'s
// node-identity map (ADR-0003 §3). The wire carries neither `tsf_us` nor
// `host_arrival_us` (ADR-0004/ADR-0005); host arrival is added core-side from
// the injected `Clock` (§4), on the envelope, never on the frame.

/// A CSI source yields frames, or reports clean end-of-stream for finite sources.
pub enum FrameOrEnd {
    Frame(CsiFrame),
    End,
}

pub enum SourceError {
    Io(std::io::Error),
    Decode(aquila_wire::DecodeError),
    /// Returned by a pending `recv` after `shutdown`.
    Stopped,
    Other(String),
}

/// Pull-based, blocking, object-safe. One live implementation may yield frames
/// from many nodes; a replay implementation reads one Session.
pub trait CsiSource: Send {
    /// Blocking pull. Returns `Frame` until a finite source is exhausted
    /// (`End`), or `Err(SourceError::Stopped)` after `shutdown`.
    fn recv(&mut self) -> Result<FrameOrEnd, SourceError>;

    /// Request stop; unblocks a pending `recv`. Idempotent, safe from any thread
    /// that can reach the source.
    fn shutdown(&mut self);
}
```

`CsiSource` is deliberately small: one pull, one stop. It carries no async
runtime, no channel and no backpressure policy, so a fake is a `Vec<CsiFrame>`
and tests need no network.

### 3.2 Adapters

All three live in `aquila-sources` unless noted. Each takes an injected
`Arc<dyn Clock>` (§4) so that `host_arrival_us` is deterministic under test.

```rust
pub struct ReplaySource { /* ... */ }
pub enum ReplayMode {
    /// Ignore recorded arrival times; re-stamp arrivals from the injected Clock
    /// and preserve record order. Byte-identical output for identical input.
    Deterministic,
    /// Honour recorded inter-record arrival gaps in real time.
    Realtime,
}
impl ReplaySource {
    pub fn open(path: &Path, mode: ReplayMode, clock: Arc<dyn Clock>)
        -> Result<Self, SourceError>;
    pub fn from_reader(r: SessionReader, mode: ReplayMode, clock: Arc<dyn Clock>) -> Self;
}

pub struct UdpLiveSource { /* ... */ }
pub struct LiveStats { /* atomic counters: datagrams, decoded, dropped, bad_version, bad_frame */ }
impl UdpLiveSource {
    /// Binds the CSI data port and serves all nodes from one socket.
    pub fn bind(addr: SocketAddr, clock: Arc<dyn Clock>, cfg: LiveConfig)
        -> Result<Self, SourceError>;
    pub fn stats(&self) -> Arc<LiveStats>;
}
```

- **Simulator adapter.** `SimulatorSource` lives in `aquila-simulator` and also
  implements `CsiSource`; see §7. It encodes each synthesised CSI frame through
  `aquila-wire` and then decodes it again, so the simulator exercises the real
  wire codec and the pipeline cannot tell sim from live except by metadata.
- **Recorded-replay adapter.** Reads a Session (§8) and replays it. In
  `Deterministic` mode it re-stamps `host_arrival_us` from the injected `Clock`,
  which is what makes replay byte-identical for CI.
- **Live adapter.** One UDP socket on the CSI data port yields frames from every
  node, demultiplexed by `node`. It tolerates and counts malformed or
  unknown-version datagrams rather than ending the stream, and never blocks a
  sender.

A `FanInSource` adapter multiplexes several `CsiSource`s into one stream ordered
by `host_arrival_us`. It is not needed live (one socket already covers all
nodes) but lets tests combine two recorded Sessions into one multi-node stream.

### 3.3 Composition with `Sink`

`CsiSource` produces; the `Sink` seam (owned elsewhere) consumes. The core's
pipeline is `pump(src, sink)`, so adapters swap without touching downstream
code. The determinism contract below is stated over this composition.

### 3.4 Determinism contract

For every `CsiSource`:

1. A `Deterministic` replay of a Session with a fixed `Clock` produces
   byte-identical `CsiFrame`s on every run, and an identical replay digest.
2. The simulator with a fixed scene, seed and virtual start time produces
   byte-identical frames on every run, on any machine.
3. No adapter reads the wall clock directly. Time comes only from the injected
   `Clock` or from frame metadata.

These three rules are what let a metric be **deterministic** in the ledger
(`replay.digest`) rather than merely statistical.

## 4. Time: the `Clock` seam and software time sync

The `Clock` trait is declared in `aquila-types` (ADR-0003 §5); the seam is owned
by `aquila-sensing`, which also owns the per-node drift estimator (roadmap,
Slice 3). Nodes do not own a shared clock.

```rust
// trait in aquila-types; implementations and the drift estimator in aquila-sensing
pub trait Clock: Send + Sync {
    /// Monotonic core time in nanoseconds since core boot.
    fn now(&self) -> CoreTime;
}
// SystemClock: CLOCK_MONOTONIC. ReplayClock/FakeClock: manual advance, for tests.
```

The trait is declared once, as `fn now(&self) -> CoreTime`, matching
`aquila-types` and `fusion-sync.md` §1.1; the `now_us`/`SystemClock`/`FakeClock`
sketch that used to appear here is superseded (microsecond device stamps are
converted at the ingest seam, `data-contracts.md` §10).

The node stamps each CSI frame with its own **free-running** monotonic clock
(`esp_timer_get_time`, microseconds) and never disciplines it. Two independent
node clocks therefore drift by tens of ppm, and arrivals interleave
out of order across nodes. The core reconciles this into one grid:

1. **RTT anchor (control plane).** The core sends `SYNC_PING` with `t1` (host
   send); the node replies `SYNC_ECHO` with `t2`/`t3` (node receive/send). The
   core computes an NTP-style offset and round-trip delay; the minimum-RTT
   sample is the anchor.
2. **Stream regression (data plane).** Over many frames the core fits an affine
   map `device_time_us → host_ts` per node by least squares on
   `(device_time_us, host_arrival_us)` pairs, giving offset and skew (drift).
3. The estimator exposes the current map and its residual for the dashboard's
   `node.health` and `system.health` (`drift estimate` in `dashboard.md`).

Inter-node drift is **estimated and applied, never assumed away** (roadmap,
Slice 4). Phase-coherent RF reference is not used and remains future-only.

## 5. CSI data-plane wire protocol

One CSI frame per UDP datagram, node → core, unicast, little-endian. No ACK: a
lossy stream of observations tolerates loss, and loss is made visible rather
than recovered (§5.4). Default port is `9100/udp`, configurable, private-only.
The codec is owned by `aquila-wire` and is the single source of truth for both
Rust and the firmware (§9.4).

### 5.1 Datagram layout

Little-endian, packed, no trailing padding. This is exactly the
`data-contracts.md` §9.1 layout carried under the `AQCI` magic (ADR-0004): the
payload is the frozen `CsiFrame` contract (`data-contracts.md` §2) as the raw
signed `int8` I/Q sample the esp-csi capture produced (ADR-0005), interleaved
`Q, I` — **imaginary first** (`[Q0, I0, Q1, I1, …]`, ADR-0006) — **not** a
derived `f32` amplitude/phase pair. Max datagram is 1472 bytes (under a
1500-byte MTU); oversized frames are dropped and counted, never fragmented at
the application layer.

**Fixed header — 16 bytes**

| Offset | Size | Field | Encoding |
| --- | --- | --- | --- |
| 0 | 4 | `magic` | ASCII `AQCI` (`41 51 43 49`) |
| 4 | 1 | `wire_major` | `u8` |
| 5 | 1 | `wire_minor` | `u8` |
| 6 | 1 | `msg_type` | `u8` (1 = `CsiFrame`) |
| 7 | 1 | `sc_format` | `u8` (1 = raw signed int8 I/Q, imaginary first) |
| 8 | 2 | `node_id` | `u16` (dense contract identity; ADR-0003 §3) |
| 10 | 2 | `payload_len` | `u16` (bytes after the header) |
| 12 | 1 | `band` | `u8` (0 = 2.4 GHz, 1 = 5 GHz, 255 = unknown) |
| 13 | 1 | `channel` | `u8` |
| 14 | 1 | `bandwidth_mhz` | `u8` |
| 15 | 1 | `flags` | `u8`; bit 0 = `first_word_invalid` (leading complex sample unreliable); other bits reserved = 0 |

**Body (`msg_type` 1) — 34-byte fixed part (wire minor 1), then `2 × n` bytes**

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 8 | `sequence` |
| 8 | 8 | `device_time_us` |
| 16 | 2 | `boot_epoch` |
| 18 | 2 | `subcarrier_offset` (`i16`) |
| 20 | 2 | `n_subcarriers` (`u16`) |
| 22 | 1 | `rssi_dbm` (`i8`) |
| 23 | 1 | `noise_floor_dbm` (`i8`) |
| 24 | 2 | `phy_flags` (`u16`) |
| 26 | 2 | reserved = 0 |
| 28 | 6 | `source_mac` — the MAC address of the station that transmitted the frame (wire minor 1 and later; all zeroes means "transmitter unknown") |
| 34 | 2n | `i_q[0..2n]` (interleaved signed int8 `Q, I` per subcarrier, imaginary first) |

A receiver validates `magic`, `wire_major`, `payload_len` and
`payload_len == 34 + 2 * n_subcarriers` before trusting any field. A `wire_minor`
mismatch is accepted (additive); a `wire_major` mismatch is rejected and counted
once per `(node_id, boot_epoch)`, never guessed. The codec knows each minor's
body layout: **wire minor 0** carries no `source_mac` and its fixed body is 28
bytes, so `payload_len == 28 + 2 * n_subcarriers` and the payload starts at body
offset 28; **wire minor 1** and later carry the six-byte `source_mac` and the
fixed body is 34 bytes. The wire carries the dense
`node_id`; the factory `MacAddr` is translated at the seam by `aquila-wire`'s
node-identity map (ADR-0003 §3, ADR-0004 §2). The payload is copied from the
receive buffer verbatim — interleaved `Q, I`, imaginary byte first (ADR-0006;
the raw int8 sample verbatim is ADR-0005) — so the node performs no
floating-point conversion. Header `flags`
bit 0 (`first_word_invalid`) marks a frame whose leading complex sample (two
bytes) the driver reported unreliable. The codec is `aquila-wire`'s
`encode_datagram`/`decode_datagram`; `postcard` is not used on the wire
(`data-contracts.md` §9.1).

`source_mac` is the address of the station that *transmitted* the captured
frame, copied verbatim from the driver's `wifi_csi_info_t.mac`. It is the node's
only view of *who* sent the packet, and it is what makes two nodes' captures of
the same ambient packet comparable: both nodes hear the same access point's
beacons, and the transmitter's address is the only exact key that says which
captured frames are the same transmission. A core that pairs frames on arrival
time and received power alone cannot say so — the beacons of two access points
interleave at about half the beacon interval and every beacon of one access
point arrives at nearly the same power — and a pairing that mixes transmitters
produces a phase residual that is an artefact of the guess rather than of the
channel.

### 5.2 Sequence

`sequence` is monotonic per `(node_id, boot_epoch, band, channel)`, assigned in
the capture callback, starting at 0 (`data-contracts.md` §2). A
`(node_id, boot_epoch)` change resets the core's per-node state.
Gaps are meaningful: a gap is a dropped frame, not a reordering (UDP does not
reorder within a socket pair in practice, and any reorder is surfaced by the
same gap detection). The core keeps a high-water sequence per node and counts
gaps as `csi.gap.total` for `system.health`.

### 5.3 Device timestamp and pacing

`device_time_us` is captured as close to the CSI callback as possible and is the
canonical ordering key for one node; host arrival is core-side only, added by the
`CsiEnvelope` wrapper (`data-contracts.md` §2), and never on the wire. The wire
carries no TSF (ADR-0004 replaced the earlier 56-byte header and its optional
`tsf_us`).

**Pacing is RX-event-driven, not self-generated.** v1 nodes are passive
observers: a CSI frame exists only when the radio receives an 802.11 frame,
which in v1 comes from the home's own ambient traffic. The node does not invent
a sample cadence, does not batch, and does not delay. It sends one CSI frame per
datagram as fast as the ambient traffic and the sender task allow, subject to a
configurable `max_tx_rate_hz` cap that protects the network (frames over the cap
are dropped and counted, not queued). Inter-frame spacing is preserved by the
node's own `device_time_us`.

Consequence, stated honestly: live CSI rate and cross-node frame alignment
depend on ambient traffic and cannot be guaranteed. The simulator and Sessions
provide deterministic, controllable, high-rate data for development and
scoring; live validates the real, un-controlled case. Dedicated elicitation
waveforms are the Track 2 question and are not a v1 feature.

### 5.4 Loss and backpressure

A dropped frame is dropped, never retransmitted. The node assigns `sequence`
before enqueueing into a fixed-capacity lock-free ring; if the ring is full the
new frame is dropped and `dropped` is incremented. The capture callback never
blocks and never allocates. Loss thus appears in two places the core can see:
sequence gaps (§5.2) and the node's `dropped` counter in its heartbeat (§6.4).
There is no ACK, no retransmit and no flow control on the data plane.

## 6. Node control channel

A separate UDP request/response protocol on the control port (default
`9101/udp`), distinct from the CSI data plane. It is the "direct node control
channel" of the brief; it is private, and it is not the core's HTTP+WS plane.
The `NodeControl` half of the `CsiSource` seam lives in `aquila-sources`; the
core relays it through `aquila-api` inside the `aquila-addon` composition root
(ADR-0003 §3, `interface-register.md`).

### 6.1 Framing

| Offset | Size | Field | Notes |
| --- | --- | --- | --- |
| 0 | 4 | `magic` | ASCII `AQCC` (`41 51 43 43`) |
| 4 | 1 | `version` | `1` |
| 5 | 1 | `opcode` | §6.2 |
| 6 | 2 | `body_len` | |
| 8 | 4 | `request_id` | caller nonce; replies echo it |
| 12 | `body_len` | `body` | opcode-specific, little-endian |
| `12 + body_len` | 32 | `auth` | optional; present only on an authenticated message (§6.3) |

Bodies are small fixed C-compatible structs, not JSON or CBOR, so the firmware
needs no parser. Every request is idempotent and keyed by `(node, request_id)`;
a retransmit is deduplicated by the node within a bounded window.

**Authentication trailer (v1).** Once a node is paired, every control command
carries an `auth` trailer: 32 bytes of HMAC-SHA256 keyed by the pairing token
issued in `JOIN_ACK`, computed over the header and body bytes (everything before
the trailer). A receiver verifies the tag in constant time and rejects a message
whose tag is absent, the wrong length, or does not match
(`decode_control_authenticated` in `aquila-wire` emits
`WireError::AuthenticationFailed`). The trailer is **additive**: the plain
framing is unchanged, so a codec that does not speak it still frames the same
header and body. `JOIN_ACK` is the one message that carries no trailer, because
it is what delivers the token.

**Honest v1 scope.** The node implements this verification and **fails closed**.
The wire codec (`encode_control_authenticated` / `decode_control_authenticated`)
and the `NodeControl` sender (an optional pairing token) both support the
trailer, but the running `aquila-addon` composition root does not yet wire the
node control plane, so no live core sends tagged commands and a paired node
refuses every command but pairing. v1 authenticates the core → node direction
only; node → core replies and `STATUS` are not yet authenticated, so "mutually
authenticated" is a target, not a v1 property. The CSI **data plane** is not
sealed or authenticated in v1 at all (see `privacy-security.md` §5).

**`NodeControl` interface.** `NodeControl` is the outbound half of the
`CsiSource` protocol, owned by `aquila-sources` (ADR-0001 §3). It is deliberately
small and object-safe:

```rust
// aquila-sources
pub enum ControlError {
    Io(std::io::Error),
    Decode(aquila_wire::DecodeError),
    /// The node did not answer within the deadline.
    Timeout { request_id: u32 },
    /// A reply did not echo the expected `request_id`.
    StaleReply { request_id: u32 },
    Other(String),
}

pub trait NodeControl: Send {
    /// Send one control request and block for its `RESULT`/ack. `body` is the
    /// opcode-specific body of §6.2; the framing, request id and (once paired)
    /// the §6.1 auth trailer are added here.
    fn send(&mut self, opcode: Opcode, body: &[u8]) -> Result<Vec<u8>, ControlError>;
    /// Request stop; unblocks a pending `send`. Idempotent.
    fn shutdown(&mut self);
}
```

The real adapter is `UdpNodeControl` (the same crate, the same socket pair as
`UdpLiveSource`); the test adapter is `FakeNodeControl`, which records commands
and replays scripted replies. Only `send`/`shutdown` are exposed; retransmission,
`request_id` dedup and the auth trailer stay inside.

### 6.2 Opcodes

| Code | Name | Direction | Purpose |
| --- | --- | --- | --- |
| `0x01` | `JOIN_REQUEST` | node → core | bootstrap pairing (§6.3) |
| `0x02` | `JOIN_ACK` | core → node | pairing accepted; token + config |
| `0x03` | `RESULT` | both | reply to a `request_id` (status + body) |
| `0x04` | `STATUS` | node → core | heartbeat (unsolicited) |
| `0x05` | `GET_CONFIG` | core → node | read current config |
| `0x06` | `SET_CONFIG` | core → node | apply config (transactional) |
| `0x07` | `SET_CSI` | core → node | enable/disable CSI streaming |
| `0x08` | `SYNC_PING` | core → node | `t1` host send time |
| `0x09` | `SYNC_ECHO` | node → core | `t1` echo, `t2`/`t3` node times |
| `0x0A` | `REBOOT` | core → node | reboot |
| `0x0B` | `IDENTIFY` | core → node | blink LED |
| `0x0C` | `FACTORY_RESET` | core → node | clear NVS and unpair |
| `0x0D` | `EVENT` | node → core | noteworthy unsolicited event |
| `0x0E` | `OTA_BEGIN` | core → node | start a firmware transfer (see §9.5, `firmware-ota.md` §5) |
| `0x0F` | `OTA_CHUNK` | core → node | one image chunk |
| `0x10` | `OTA_ACK` | node → core | cumulative chunk acknowledgement |
| `0x11` | `OTA_STATUS` | node → core | OTA progress/state |
| `0x12` | `OTA_END` | core → node | finalise, verify and activate |
| `0x13` | `OTA_ABORT` | both | discard the transfer |
| `0x14` | `OTA_RESULT` | node → core | OTA outcome |

`0x0E`–`0x14` are the firmware-OTA extension of this table; their bodies and
rules are specified in `docs/spec/firmware-ota.md` §5 and summarised in §9.5.
`aquila-wire` encodes/decodes them and the generated `node_wire.h` carries them
(`firmware/esp32-s3-csi-node/components/node_wire/include/node_wire.h`); the
node firmware agent (`node_ota`, ticket #14) handles `OTA_BEGIN`/`OTA_CHUNK`/
`OTA_END`/`OTA_ABORT` on the device. Everything is compile-verified with
ESP-IDF v5.5.2 but **not flashed or run on hardware**: the node's Ed25519 trust
anchor must be provisioned or OTA fails closed, and end-to-end OTA also needs
the core's signed image store and a paired-node transport wired (ticket #13,
P3). These rows are specification plus compile-verified code, not hardware
evidence (F2/F29/F36/F38).

`SET_CONFIG` carries a `config_version` and a `config_hash`; the node rejects a
stale `config_version`, applies atomically, and replies `ACTIVE` or `REJECTED`
with a reason. **v1 limitation:** the `config_hash` is carried but **not**
verified — no canonical config-hash algorithm is defined on the core side, so
the node has nothing to recompute against (F7). It is reserved until that
algorithm is pinned. Indicative fields: channel, bandwidth, secondary, csi enable,
LLTF/HT-LTF/STBC-HT-LTF enables, CSI length cap, `max_tx_rate_hz`,
`heartbeat_ms`, and an optional allow-list of source MACs.

### 6.3 Pairing

Aligned with `dashboard.md`'s node-pairing flow. `POST /nodes/pairing` returns a
short-lived code; the operator flashes a node and enters it. The node, once
associated, sends `JOIN_REQUEST {node, boot, pairing_code, fw_version}` to the
core endpoint. The core marks the node **pending** (visible in the dashboard);
on operator approval the core replies `JOIN_ACK {node, token, config}`. The node
stores the token in NVS and is paired; every subsequent control frame
authenticates with it by carrying the HMAC-SHA256 trailer of §6.1, and the node
also pins the core endpoint and rejects a replayed `request_id`. Approval is
where the node is assigned to a room, in the home model.

### 6.4 Heartbeat and health

The node sends `STATUS` every `heartbeat_ms` (default 5000 ms, ~0.2 Hz, matching
the dashboard's `node.health` rate): uptime, `boot`, last `sequence`, `dropped`,
ring depth, temperature (the internal on-die sensor, or `0` when it is
unavailable), `config_version`, and a latency field. That field is the node's own
`SYNC_PING` echo turnaround (`t3 − t2`); only the core can measure the true
round-trip time, so the node reports what it can observe rather than a fabricated
RTT (F32). This is the node's only self-report; it carries no CSI and no
inference.

## 7. Deterministic CSI simulator

`aquila-simulator` provides a hardware-free, byte-deterministic CSI source for
tests, CI and the sim-first slices. It exists so that any ledger row can be
reproduced from committed inputs without a node.

### 7.1 Determinism contract

The simulator is a pure function of `(scene script, seed, virtual start time,
tick schedule)`. It reads no wall clock, spawns no threads, and uses only a
seeded PRNG. It drives a **virtual clock**: `device_time_us` comes from a virtual
node clock (with modelled skew), and `host_arrival_us` from a virtual host
clock. Same inputs ⇒ byte-identical frames, on any machine. The digest of a
synthesised Session is asserted in CI (`replay.digest`, deterministic class).

### 7.2 Scene model

A scene script (TOML) declares the home → floor → room → node → node-pair
topology, node positions, a Baseline (empty-home) state, actors with motion
profiles, per-node clock skew (ppm), per-node noise, loss/jitter/reorder
impairments, channel and bandwidth, tick rate and duration.

At tick `t`, for each node or node-pair and each subcarrier `k`:

```
H_k(t) = H0_k + Σ_i a_i(t) · exp( -j · 2π · f_k · d_i(t) / c ) + n_k(t)
```

- `H0_k` — seeded static multipath baseline (the empty-home channel).
- `i` — dynamic scatterers: a person's torso and limbs.
- `d_i(t) = d0_i + d_breath_i(t) + d_heart_i(t) + d_motion_i(t)` — path length.
- `f_k` — the k-th subcarrier frequency; `c` — speed of light.
- `n_k(t)` — additive noise at the configured floor.
- `H_k(t)` is **quantised to raw signed int8 I/Q** — the contract payload
  (`data-contracts.md` §2, ADR-0005) — so the synthesised frame is byte-identical
  in kind to a live one and the core's `int8 → f32` derivation is exercised.
  Amplitude and phase are not emitted; the core derives them. RSSI and noise
  floor are derived from the same model.

Modulations (these amplitudes and rates are **modelling assumptions**, not
measured facts; the determinism test pins reproducibility, not physical
correctness, so they are labelled `Claimed` and carry no `sources.json` id):

- **Breathing** — `d_breath(t) = A_b · sin(2π f_b t + φ_b)`, `A_b ≈ 3–8 mm`,
  `f_b = 0.1–0.5 Hz`. A small chest displacement rotates each subcarrier's phase
  (`Δφ_k = 2π f_k Δd / c`) and, through multipath, changes amplitude.
- **Heart** — `d_heart(t) = A_h · sin(2π f_h t + φ_h)`, `A_h ≈ 0.05–0.2 mm`,
  `f_h = 0.8–2.0 Hz`. Superimposed on breathing at lower amplitude.
- **Motion** — `p_i(t)` from the actor's motion profile, with
  `d_i(t) = |p_i − p_tx| + |p_rx − p_i|`; gross movement sweeps the phase
  rapidly and models blockage by modulating `a_i(t)`.
- **Presence** — an absent actor removes its dynamic terms; the empty-home
  Baseline is `H0_k + n_k`.
- **Count** — the number of actors.

### 7.3 Ground truth

The simulator also implements the `GroundTruth` seam, emitting per-tick truth
(presence, count, coarse pose keypoints, breathing reference, heart reference)
synchronously with the frames, so a synthesised Session is self-labelling and
the measurement pipeline can score against it. This is the reference used for
sim-based metrics; it is not a runtime input (§8.5).

### 7.4 Impairments

Per-node clock skew (ppm) applied to timestamps; network jitter, loss and
reordering applied at the datagram layer; random first-word corruption (the
first complex sample — two bytes, one subcarrier — zeroed and the
`first_word_invalid` flag set, the internal esp-csi quirk); and the subcarrier
count implied by bandwidth. Fakes for the whole pipeline — including
a fake `Clock` and a fake `Sink` — are ordinary structs, not fixtures.

## 8. Session format (recorded fixtures)

A **Session** is the recorded artefact. The on-disk container is owned by
`aquila-session` and has the extension `.aqs`. It is streaming-friendly: header,
record stream, digest trailer.

### 8.1 Layout

```
"AQS" magic | u8 version | u32 metadata_len | metadata (UTF-8 JSON)
record* (typed, length-prefixed)
"AQE" trailer | u64 record_count | sha256(record stream)
```

Metadata (canonical JSON, sorted keys) records: schema version, session id,
created UTC, capture-tool and core versions, node inventory (node id → floor,
room, position), topology, channel/bandwidth, elicitation context (v1: ambient),
whether ground truth is present and its provenance, licence, and provenance.

### 8.2 Records

Each record is `u8 type | u32 body_len | body`:

| Type | Name | Body |
| --- | --- | --- |
| `0x01` | CSI frame | `u64 host_arrival_us` + the exact on-wire `AQCI` datagram bytes |
| `0x02` | Ground truth | `u64 host_arrival_us` + JSON (presence, count, keypoints, references) |
| `0x03` | Event | `u64 host_arrival_us` + JSON (join/leave, config change, sync, drops) |
| `0x04` | Annotation | free-form JSON (operator scene notes) |

Storing the **exact wire bytes** for a CSI frame is deliberate: replay runs the
real decoder, so the wire codec is exercised by every replay test and cannot
drift from live.

### 8.3 Capture

`aquila-capture` (a small binary, not part of the runtime) provides:

- `record` — bind the CSI data port, write a Session from live nodes; optionally
  attach a `GroundTruth` adapter for labelled captures.
- `synth` — write a Session from a scene script via `aquila-simulator`.
- `verify` — validate Sessions against the manifest and container invariants.
- `inspect` — print a Session summary.
- `provision` — write NVS (network credentials, core endpoint) for a node;
  pairing itself happens on the control channel (§6.3).

No hardware is needed for CI: `synth` plus one committed golden Session covers
the codec and the pipeline.

### 8.4 Check-in and size

Raw CSI is large (hundreds of kB/s per node). Committed Sessions are therefore
**short, curated clips** (roughly ≤ 60 s, a subset of nodes) stored via Git LFS;
a small, deterministic **golden** Session produced by `synth` is committed
directly for codec tests. Full captures are archived outside the repository.
`fixtures/manifest.json` (committed, not LFS) maps every Session to its sha256,
byte size, node count, duration, channels, ground-truth presence, provenance and
licence. `aquila-capture verify` recomputes the digests; a CI job runs it. Using
Git LFS for the clips is a decision to confirm with the maintainer.

### 8.5 Provenance and privacy

Sessions are own captures, GPLv3, non-commercial. A Session contains CSI (RF),
never imagery. Ground truth from a camera track is research-only and is stored
as **derived labels only** (presence, count, keypoints, references), never video
or frames. Ground-truth records are present only in evaluation Sessions and are
never consumed at runtime (the `GroundTruth` seam is research-only).

## 9. ESP32-S3 node firmware

Reference target: ESP32-S3 (ESP-IDF). ESP32-C6 is future and reuses the same
components with a different chip target. The firmware is C/ESP-IDF, not a Rust
crate; it lives under `firmware/esp32-s3-csi-node/`.

### 9.1 Responsibilities

1. Join the home wireless network so the core endpoint is reachable (an ESP-IDF
   association detail; it confers no Aquila role).
2. Configure CSI via esp-csi — `wifi_csi_config_t` from the control-channel
   config, `esp_wifi_set_csi_rx_cb`, `esp_wifi_set_csi(true)` — and enable
   promiscuous reception so ambient frames not addressed to the node still
   produce CSI.
3. In the CSI callback: copy the CSI buffer, snapshot `rx_ctrl`, read
   `esp_timer_get_time()` (monotonic µs), assign the per-boot `sequence`, record
   the internal first-word-invalid quirk (the first complex sample — two bytes —
   is unreliable; the node nulls it and sets the header `flags` bit 0), and
   enqueue into the ring. No allocation, no logging, no floating point.
4. A sender task drains the ring to the core's UDP endpoint, one CSI frame per
   datagram, subject to `max_tx_rate_hz`; drop-and-count on overflow (§5.4). The
   sender copies the raw signed int8 I/Q buffer into the payload verbatim —
   interleaved `Q, I`, imaginary byte first — so the node performs no conversion
   (ADR-0005, ADR-0006).
5. `boot_epoch` (a `u16`, matching the contract frame and the control-plane
   `boot` field) = random at boot; `sequence` resets per boot.
6. Serve the control channel: pairing, config, status, sync echo, reboot,
   identify, factory reset (§6).
7. Send a periodic `STATUS` heartbeat (§6.4).
8. Persist config, token and credentials in NVS; load and apply at boot; enter
   pairing if unpaired (§6.3).
9. `IDENTIFY` blinks an LED; `REBOOT` reboots; `FACTORY_RESET` clears NVS. The
   `IDENTIFY` blink is **compiled out** until a board GPIO is assigned
   (`IDENTIFY_GPIO (-1)`), so today the node acknowledges the command but shows
   no light (F33).
10. Serial logging is development-only and off or trivial in production.

### 9.2 What it MUST NOT do

- No DSP, inference, fusion, calibration, feature extraction, filtering,
  resampling, FFT or phase sanitisation. It ships the raw signed int8 I/Q sample
  the esp-csi capture produced, verbatim, without interpreting it or converting
  it to amplitude/phase (ADR-0005).
- Never sets, disciplines or corrects its clock; it emits free-running monotonic
  timestamps only. Time sync is host-side (§4).
- Never transmits elicitation waveforms, selects subcarriers, steers, or shapes
  its transmit pattern. Active sensing is research-only (Track 2).
- No ESP-NOW, MQTT, Matter, Zigbee, BLE, mesh, or node-to-node coordination. It
  talks only to the core.
- No node-hosted UI, HTTP server, SDK or plugin surface; no cloud; no telemetry
  to anywhere but the configured core endpoint.
- No buffering, storage or aggregation of CSI across frames (one frame per
  datagram), and no retransmission of the CSI stream.
- No ground truth and no sensing claims of any kind.
- Never forwards another node's data.

### 9.3 Components and config

ESP-IDF components: `csi_node` (capture + send), `node_control` (control
channel), `node_wire` (framing/encode/decode). NVS keys: network credentials,
core host/ports, pairing token, CSI config, `max_tx_rate_hz`, `heartbeat_ms`.
Compile-time: firmware version and chip target.

### 9.4 One wire layout, two languages

`aquila-wire` (Rust) and `node_wire` (C) MUST implement one identical layout.
`aquila-wire` owns the definition and generates `node_wire.h`. CI currently
checks only that the committed **header text** matches the generator; it does
**not** compile or run the C framing helpers, so `node_wire_util.c` could still
drift from the declared layout. Shared golden vectors run through both
implementations — the check that would close this — are planned, not yet
present (F37).

### 9.5 Firmware update (OTA)

> **Specified and compile-verified; hardware-gated end to end.** The node
> firmware agent (`components/node_ota/`, ticket #14) handles `OTA_BEGIN`/
> `OTA_CHUNK`/`OTA_END`/`OTA_ABORT`, writes chunks to the inactive A/B partition
> with `esp_ota_*`, checks CRC-32 per chunk and SHA-256 + Ed25519 over the whole
> image, activates and reboots, then runs the pending-verify health window with
> automatic rollback (`partitions.csv` provides the dual-OTA slots; `node_wire.h`
> carries opcodes `0x0E`–`0x14`). Everything is compile-verified with ESP-IDF
> v5.5.2 but **not flashed or run on hardware**: the node's Ed25519 trust anchor
> (`AQUILA_FW_PUBKEY_HEX` or NVS `ota`/`fw_pubkey`) must be provisioned or OTA
> fails closed, and end-to-end OTA also needs the core's signed image store and
> a paired-node transport wired (ticket #13, P3) plus the HA `update` entity
> (ticket #15). The core's OTA endpoints fail closed with a typed
> `503 ota_unavailable` until those are in place.

Node firmware is updated over the LAN by the core, with no cloud. OTA rides the
**node control plane** (`9101/udp`) via opcodes `0x0E`–`0x14` (new, extending
§6.2), not a node-facing HTTP server. The node uses ESP-IDF dual-partition A/B OTA
with Ed25519 verification, an anti-rollback version floor, and automatic rollback
if the new image does not self-confirm. The design is specified in
`docs/spec/firmware-ota.md`; Home Assistant surfaces it as an `update` entity per
node (`home-assistant.md` §6).

## 10. Crate map

This document does not define a crate graph; `docs/spec/interface-register.md`
is the single canonical crate list, settled by ADR-0003. Four of the register's
crates are specified here, all `aquila-<english-descriptor>`; each owns one deep
module:

| Crate | Role | Depends on (register) |
| --- | --- | --- |
| `aquila-wire` | CSI + control frame codec; generates the C header | `aquila-types` |
| `aquila-session` | `.aqs` Session container read/write; manifest | `aquila-types`, `aquila-wire` |
| `aquila-simulator` | deterministic scene synthesis; implements `CsiSource` | `aquila-types`, `aquila-wire`, `aquila-session`, `aquila-sources` |
| `aquila-capture` | capture/synth/verify/provision binary (dev tool) | `aquila-types`, `aquila-session`, `aquila-sources`, `aquila-simulator`, `aquila-wire` |

`CsiSource` and its replay/live adapters live in `aquila-sources`; the `Clock`
trait is declared in `aquila-types` and the drift estimator lives in
`aquila-sensing` (ADR-0003 §5); the `GroundTruth` adapter lives in
`aquila-evaluation`. The DAG is acyclic and enforced by the dependency-DAG check
(`tools/dag.toml`). Firmware (`firmware/esp32-s3-csi-node/`) is not a crate.

## 11. Open decisions

Recorded here for the ADR(s) this spec will spawn; each is a protocol decision
made above and open to review. Items marked **settled** are decided, not open —
they are listed for traceability only:

1. **Node identity — settled by ADR-0003 §3.** The wire carries the dense
   `NodeId(u16)` (`data-contracts.md` §1); the factory base MAC (`MacAddr`) is
   mapped to and from it at the seam by `aquila-wire`'s node-identity map. There
   is no MAC-typed node id on the wire or in the home model.
2. **Elicitation.** v1 is passive/ambient; the firmware does not transmit.
   Dedicated elicitation is deferred to Track 2. Consequence stated in §5.3.
3. **Control channel.** UDP request/response, not TCP/HTTP, so the firmware
   needs no HTTP stack; pairing by short-lived code, then a long-lived token.
4. **Data plane — settled by ADR-0003 §7.** One CSI frame per datagram, unicast,
   no ACK; loss surfaced by sequence gaps and the node `dropped` counter. Ports
   `9100/udp` (data) and `9101/udp` (control).
5. **Timestamp.** `esp_timer_get_time` monotonic µs is canonical; the wire
   carries **no** TSF (ADR-0004 replaced the earlier 56-byte header and its
   optional `tsf_us`); the host estimates offset and drift (NTP-style anchor plus
   stream regression).
6. **Session container.** `.aqs`, typed records carrying exact wire bytes, JSON
   metadata, digest trailer; Git LFS for large clips, a golden Session committed.
7. **Shared wire layout.** `aquila-wire` generates `node_wire.h`, checked in CI.

## 12. References

- `CONTEXT.md` — canonical vocabulary.
- `docs/spec/roadmap.md` — slice sequence, seams and crate map.
- `docs/spec/dashboard.md` — the core's private HTTP+WS plane and node pairing.
- `docs/research/esp32-as-sdr.md` — why raw I/Q is out of scope for v1.
- `docs/research/research-tracks.md` — Track 2 (active sensing) and Track 1
  (camera ground truth) as research-only.
