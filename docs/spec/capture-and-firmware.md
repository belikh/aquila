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
by the `aquila-source` crate (roadmap). The seam MUST be satisfied by at least
three adapters — simulator, recorded-replay, live-UDP — so that replay and live
are genuinely interchangeable and the seam is real, not speculative.

### 3.1 Types

```rust
// aquila-source

/// Stable node identity: the ESP32-S3 factory base MAC (six bytes).
/// The home model maps it to a floor and room; it is the only node id on the wire.
pub type NodeId = MacAddr; // [u8; 6]

/// A node's power-cycle epoch. Random per boot; sequence resets to zero per boot.
pub type BootId = u32;

/// Per-boot monotonic frame counter. Assigned on the node at capture time.
pub type Sequence = u64;

/// One decoded CSI frame plus host-side ingest metadata.
pub struct CsiFrame {
    pub node: NodeId,
    pub boot: BootId,
    pub sequence: Sequence,
    /// Node monotonic clock at capture, microseconds. Never disciplined.
    pub device_ts_us: u64,
    /// Host clock when the source ingested the frame, microseconds.
    pub host_arrival_us: u64,
    /// 802.11 TSF if the driver reported it; None otherwise.
    pub tsf_us: Option<u32>,
    /// Channel, bandwidth, MCS, RSSI, noise floor, antenna (see §5).
    pub phy: PhyInfo,
    /// The driver marked the first CSI word invalid; consumers must treat the
    /// first four CSI bytes as unusable.
    pub first_word_invalid: bool,
    /// Transmitter of the frame that elicited this CSI, when known.
    pub source_mac: Option<MacAddr>,
    /// Raw complex coefficients, interleaved signed 8-bit I, Q, I, Q, …
    /// Subcarrier count is `csi.len() / 2`.
    pub csi: CsiBuffer,
}

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

All three live in `aquila-source` unless noted. Each takes an injected
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

`aquila-clock` owns the `Clock` seam and per-node drift estimation (roadmap,
Slice 3). Nodes do not own a shared clock.

```rust
// aquila-clock
pub trait Clock: Send + Sync {
    /// Host monotonic time in microseconds.
    fn now_us(&self) -> u64;
}
// SystemClock: CLOCK_MONOTONIC. FakeClock: manual advance, for tests.
```

The node stamps each CSI frame with its own **free-running** monotonic clock
(`esp_timer_get_time`, microseconds) and never disciplines it. Two independent
node clocks therefore drift by tens of ppm, and arrivals interleave
out of order across nodes. The core reconciles this into one grid:

1. **RTT anchor (control plane).** The core sends `SYNC_PING` with `t1` (host
   send); the node replies `SYNC_ECHO` with `t2`/`t3` (node receive/send). The
   core computes an NTP-style offset and round-trip delay; the minimum-RTT
   sample is the anchor.
2. **Stream regression (data plane).** Over many frames the core fits an affine
   map `device_ts_us → host_ts` per node by least squares on `(device_ts_us,
   host_arrival_us)` pairs, giving offset and skew (drift).
3. The estimator exposes the current map and its residual for the dashboard's
   `node.health` and `system.health` (`drift estimate` in `dashboard.md`).

Inter-node drift is **estimated and applied, never assumed away** (roadmap,
Slice 4). Phase-coherent RF reference is not used and remains future-only.

## 5. CSI data-plane wire protocol

One CSI frame per UDP datagram, node → core, unicast, little-endian. No ACK: a
lossy stream of observations tolerates loss, and loss is made visible rather
than recovered (§5.4). Default port is `9100/udp`, configurable, private-only.
The codec is owned by `aquila-wire` and is the single source of truth for both
Rust and the firmware (§9.6).

### 5.1 Datagram layout

`aqci` header is 56 bytes, fixed. `header_len` exists so fields can be appended
without a version bump.

| Offset | Size | Field | Notes |
| --- | --- | --- | --- |
| 0 | 4 | `magic` | ASCII `AQCI` (`41 51 43 49`) |
| 4 | 1 | `version` | `1` |
| 5 | 1 | `flags` | bit0 `FIRST_WORD_INVALID`; bit1 `TSF_VALID`; bit2 `SOURCE_KNOWN`; bits3–7 reserved, zero |
| 6 | 2 | `header_len` | `56` |
| 8 | 6 | `node` | factory base MAC |
| 14 | 4 | `boot` | `BootId`, random per boot |
| 18 | 8 | `sequence` | per-boot, from 0 |
| 26 | 8 | `device_ts_us` | node monotonic, µs |
| 34 | 4 | `tsf_us` | 802.11 TSF; zero unless `TSF_VALID` |
| 38 | 1 | `chan_primary` | 802.11 channel |
| 39 | 1 | `chan_bw` | 0=20, 1=40, 2=80, 3=160 MHz |
| 40 | 1 | `chan_secondary` | 0=none, 1=above, 2=below |
| 41 | 1 | `sig_mode` | 0=non-HT, 1=HT, 2=VHT, 3=HE |
| 42 | 1 | `mcs` | |
| 43 | 1 | `rate` | raw driver rate index |
| 44 | 1 | `rssi` | `i8`, dBm |
| 45 | 1 | `noise_floor` | `i8`, dBm |
| 46 | 1 | `antenna` | |
| 47 | 1 | `reserved` | zero |
| 48 | 6 | `source_mac` | eliciting frame transmitter; zero unless `SOURCE_KNOWN` |
| 54 | 2 | `csi_len` | bytes, even; subcarriers = `csi_len / 2` |
| 56 | `csi_len` | `csi` | `int8`, interleaved I, Q |

`csi_len` ≤ 612 (esp-csi maximum), so a datagram is ≤ 668 bytes — comfortably
under a 1500-byte MTU, with no fragmentation. At 20 MHz HT there are ~64
subcarriers (~128 bytes); at 40 MHz ~128.

The core decodes every datagram, but a mismatch between `header_len` and
`version` is not fatal: unknown trailing bytes are ignored, so a newer node
firmware is forward-compatible with an older core.

### 5.2 Sequence

`sequence` is monotonic per `(node, boot)`, assigned in the capture callback,
starting at 0. A `(node, boot)` change resets the core's per-node state.
Gaps are meaningful: a gap is a dropped frame, not a reordering (UDP does not
reorder within a socket pair in practice, and any reorder is surfaced by the
same gap detection). The core keeps a high-water sequence per node and counts
gaps as `csi.gap.total` for `system.health`.

### 5.3 Device timestamp and pacing

`device_ts_us` is captured as close to the CSI callback as possible and is the
canonical ordering key for one node; `host_arrival_us` is core-side only and
never on the wire. `tsf_us` is optional and secondary.

**Pacing is RX-event-driven, not self-generated.** v1 nodes are passive
observers: a CSI frame exists only when the radio receives an 802.11 frame,
which in v1 comes from the home's own ambient traffic. The node does not invent
a sample cadence, does not batch, and does not delay. It sends one CSI frame per
datagram as fast as the ambient traffic and the sender task allow, subject to a
configurable `max_tx_rate_hz` cap that protects the network (frames over the cap
are dropped and counted, not queued). Inter-frame spacing is preserved by the
node's own `device_ts_us`.

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
Owned by the core (roadmap: `aquila-host`; `dashboard.md` calls the same core
`aquila-core`).

### 6.1 Framing

| Offset | Size | Field | Notes |
| --- | --- | --- | --- |
| 0 | 4 | `magic` | ASCII `AQCC` (`41 51 43 43`) |
| 4 | 1 | `version` | `1` |
| 5 | 1 | `opcode` | §6.2 |
| 6 | 2 | `body_len` | |
| 8 | 4 | `request_id` | caller nonce; replies echo it |
| 12 | `body_len` | `body` | opcode-specific, little-endian |

Bodies are small fixed C-compatible structs, not JSON or CBOR, so the firmware
needs no parser. Every request is idempotent and keyed by `(node, request_id)`;
a retransmit is deduplicated by the node within a bounded window.

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

`SET_CONFIG` carries a `config_version` and a `config_hash`; the node
validates, applies atomically, and replies `ACTIVE` or `REJECTED` with a
reason. Indicative fields: channel, bandwidth, secondary, csi enable,
LLTF/HT-LTF/STBC-HT-LTF enables, CSI length cap, `max_tx_rate_hz`,
`heartbeat_ms`, and an optional allow-list of source MACs.

### 6.3 Pairing

Aligned with `dashboard.md`'s node-pairing flow. `POST /nodes/pairing` returns a
short-lived code; the operator flashes a node and enters it. The node, once
associated, sends `JOIN_REQUEST {node, boot, pairing_code, fw_version}` to the
core endpoint. The core marks the node **pending** (visible in the dashboard);
on operator approval the core replies `JOIN_ACK {node, token, config}`. The node
stores the token in NVS and is paired; subsequent control frames authenticate
with it. Approval is where the node is assigned to a room, in the home model.

### 6.4 Heartbeat and health

The node sends `STATUS` every `heartbeat_ms` (default 5000 ms, ~0.2 Hz, matching
the dashboard's `node.health` rate): uptime, `boot`, last `sequence`, `dropped`,
ring depth, temperature, `config_version`, and last measured core RTT. This is
the node's only self-report; it carries no CSI and no inference.

## 7. Deterministic CSI simulator

`aquila-simulator` provides a hardware-free, byte-deterministic CSI source for
tests, CI and the sim-first slices. It exists so that any ledger row can be
reproduced from committed inputs without a node.

### 7.1 Determinism contract

The simulator is a pure function of `(scene script, seed, virtual start time,
tick schedule)`. It reads no wall clock, spawns no threads, and uses only a
seeded PRNG. It drives a **virtual clock**: `device_ts_us` comes from a virtual
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
- Coefficients are quantised to `int8` with a configured scale; RSSI and noise
  floor are derived from the same model.

Modulations:

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
reordering applied at the datagram layer; random `FIRST_WORD_INVALID`; and the
subcarrier count implied by bandwidth. Fakes for the whole pipeline — including
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
crate; it lives under `firmware/esp32s3/`.

### 9.1 Responsibilities

1. Join the home wireless network so the core endpoint is reachable (an ESP-IDF
   association detail; it confers no Aquila role).
2. Configure CSI via esp-csi — `wifi_csi_config_t` from the control-channel
   config, `esp_wifi_set_csi_rx_cb`, `esp_wifi_set_csi(true)` — and enable
   promiscuous reception so ambient frames not addressed to the node still
   produce CSI.
3. In the CSI callback: copy the CSI buffer, snapshot `rx_ctrl`, read
   `esp_timer_get_time()` (monotonic µs), read the eliciting `mac`, assign the
   per-boot `sequence`, set `FIRST_WORD_INVALID`, and enqueue into the ring. No
   allocation, no logging, no floating point.
4. A sender task drains the ring to the core's UDP endpoint, one CSI frame per
   datagram, subject to `max_tx_rate_hz`; drop-and-count on overflow (§5.4).
5. `boot` = random `u32` at boot; `sequence` resets per boot.
6. Serve the control channel: pairing, config, status, sync echo, reboot,
   identify, factory reset (§6).
7. Send a periodic `STATUS` heartbeat (§6.4).
8. Persist config, token and credentials in NVS; load and apply at boot; enter
   pairing if unpaired (§6.3).
9. `IDENTIFY` blinks an LED; `REBOOT` reboots; `FACTORY_RESET` clears NVS.
10. Serial logging is development-only and off or trivial in production.

### 9.2 What it MUST NOT do

- No DSP, inference, fusion, calibration, feature extraction, filtering,
  resampling, FFT or phase sanitisation. It ships the raw complex coefficients
  exactly as esp-csi yields them.
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
`aquila-wire` owns the definition and generates `node_wire.h`; CI checks the
committed header matches and runs shared golden vectors through both
implementations, so the two ends cannot silently drift.

### 9.5 Firmware update (OTA)

Node firmware is updated over the LAN by the core, with no cloud. OTA rides the
**node control plane** (`9101/udp`) via opcodes `0x0E`–`0x14` (new, extending
§6.2), not a node-facing HTTP server. The node uses ESP-IDF dual-partition A/B OTA
with Ed25519 verification, an anti-rollback version floor, and automatic rollback
if the new image does not self-confirm. The design is specified in
`docs/spec/firmware-ota.md`; Home Assistant surfaces it as an `update` entity per
node (`home-assistant.md` §6).

## 10. Crate map (proposed additions)

The roadmap's crate set is provisional and ADR-revisable. This document needs
four crates beyond it, all `aquila-<english-descriptor>`; each owns one deep
module:

| Crate | Role | Depends on |
| --- | --- | --- |
| `aquila-wire` | CSI + control frame codec; generates the C header | — |
| `aquila-session` | `.aqs` Session container read/write; manifest | `aquila-wire` |
| `aquila-simulator` | deterministic scene synthesis; implements `CsiSource` | `aquila-source`, `aquila-wire`, `aquila-clock` |
| `aquila-capture` | capture/synth/verify/provision binary (dev tool) | all of the above |

`CsiSource` and its replay/live adapters live in the roadmap's `aquila-source`;
`Clock` and the drift estimator in `aquila-clock`; the `GroundTruth` adapter in
`aquila-groundtruth`. The DAG is acyclic:
`wire ← session ← source ← simulator ← capture`, with `clock` a leaf shared by
`source` and `simulator`. The existing dependency-DAG check enforces it.
Firmware (`firmware/esp32s3/`) is not a crate.

## 11. Open decisions

Recorded here for the ADR(s) this spec will spawn; each is a protocol decision
made above and open to review:

1. **Node identity.** The wire and home model use the factory base MAC as the
   stable node id; a short display id is a presentation concern only.
2. **Elicitation.** v1 is passive/ambient; the firmware does not transmit.
   Dedicated elicitation is deferred to Track 2. Consequence stated in §5.3.
3. **Control channel.** UDP request/response, not TCP/HTTP, so the firmware
   needs no HTTP stack; pairing by short-lived code, then a long-lived token.
4. **Data plane.** One CSI frame per datagram, unicast, no ACK; loss surfaced by
   sequence gaps and the node `dropped` counter. Default ports `9100`/`9101`.
5. **Timestamp.** `esp_timer_get_time` monotonic µs is canonical; TSF is
   optional; the host estimates offset and drift (NTP-style anchor plus stream
   regression).
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
