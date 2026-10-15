# Core API — specification

Status: draft, v1. Owner: Core API author. Scope: the private control and
observation API exposed by the Aquila core (the Rust service inside the Home
Assistant add-on). This document is the contract for the bundled management
dashboard (Rust→WASM, served by the add-on through sidebar ingress) and the
bundled HACS custom component. Nothing else may depend on it.

Related: the sensing pipeline, calibration, fusion and measurement ledger are
specified separately; this document only defines how a client asks for their
state and streams their output.

## 1. Scope and non-goals

The API is a **private, versioned, non-public** HTTP + WebSocket surface.

Explicit non-goals (all out of scope for v1, by decision):

- **No public REST contract.** HTTP/JSON is used as a transport, but there is no
  compatibility promise to any third party. The core, the dashboard and the
  component ship together under one version; the only consumer is in-tree.
- **No MQTT.** The control plane is this HTTP+WS API plus the direct node
  control channel. MQTT is not part of Aquila.
- **No gRPC**, no protobuf, no GraphQL, no OpenAPI-driven codegen promise.
- **No MCP.** No tool/resource exposure to model agents.
- **No SDK**, no plugins, no user-extensible routes. In-tree consumers only.
- **Not reachable from the LAN.** The core never binds a LAN address and never
  creates a public URL. There is no remote or cloud path.

Home Assistant-facing entity state (HA sensors, select entities, services) is the
custom component's concern. This API does not model HA; it models Aquila.

## 2. Transport and binding

| Property | Requirement |
| --- | --- |
| Transports | HTTP/1.1 (and HTTP/2 where the proxy offers it) over TLS-less loopback or the add-on's private Docker network; WebSocket over the same connection, RFC 6455. |
| Core listener | Binds `127.0.0.1`/`::1` in the default library posture; the add-on binds its container interface so the HA core container can reach it over the `hassio` Docker network. It MUST NOT bind a host LAN interface, and the port MUST NOT be published to the host (ADR-0003 §7; `home-assistant.md` §2). |
| Port | Fixed, add-on-owned, non-privileged (default `8099`), overridable by add-on config. |
| Media types | `application/json; charset=utf-8`; errors `application/problem+json`; stream frames are JSON text frames. |
| Compression | `gzip` / `br` permitted on JSON responses when negotiated; never required. |
| Keep-alive | HTTP keep-alive on; idle HTTP connection timeout 65 s. |
| TLS | Not used. Network placement — a private or loopback peer on an unpublished port — is the trust boundary, not encryption; see §3 and §15. |

**Deployment seam.** The control-plane port (`8099`) is **not** published to the
host or the LAN. Exactly two paths reach the core:

- the **HACS component in the HA core container** calls the core directly at
  `http://aquila-core:8099` over the `hassio` Docker network, so the core
  observes a private (RFC 1918) peer (`home-assistant.md` §2, §5.2);
- the **dashboard** is reached through Home Assistant ingress: a minimal ingress
  proxy (owned by the add-on, specified elsewhere) terminates ingress,
  authenticates the HA user, stamps a signed identity header, and
  reverse-proxies to the core.

A peer outside the private network cannot reach the port at all; if the port were
ever published by mistake, such a peer would still fail the trust check in §3.
The exact HA network topology is recorded in ADR-0003; if it changes, only this
seam changes, and this section is the invariant that must hold.

## 3. Authentication and trust

Every request MUST be authenticated, and there is no anonymous mode except the
liveness probe in §4. Authentication is the **private-network/ingress trust
boundary**: the core port is unpublished (§2), so the only peers that can reach
it are the add-on's own process and the HA core container on the add-on's private
Docker network. A request is authenticated by exactly one of three mutually
exclusive modes.

### 3.1 Trusted peer

A connection whose peer address is **loopback or private** is trusted:

- loopback — `127.0.0.0/8`, `::1`;
- RFC 1918 private — `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`;
- link-local — `169.254.0.0/16`, `fe80::/10`;
- IPv6 unique-local — `fc00::/7`.

This covers the add-on's own process, its CLI and probes, and the HA core
container, which reaches the add-on at `http://aquila-core:8099` over the
`hassio` Docker network (`home-assistant.md` §5.2) on a Docker-bridge address
such as `172.17.0.0/16`, which is RFC 1918 private. The trusted peer **is** the
credential; no header is required.

This is a **network-placement** trust decision, not a confidentiality claim: the
private network is **not** encrypted, so anything able to place traffic on it is
trusted by address. A bearer token (§3.3) is the only mode that authenticates
independently of network placement. A peer outside the loopback/private ranges
is **not** trusted and MUST present the ingress identity header (§3.2) or a
valid bearer token (§3.3), or it is rejected with `401`.

### 3.2 Ingress identity

Requests that arrive through the add-on ingress proxy carry identity headers set
by the proxy (and, when a per-boot HMAC key is configured, signed with it):

- `X-Aquila-Ingress: <hmac>` — signature over the headers below plus method, path
  and body digest.
- `X-Remote-User-Id`, `X-Remote-User-Name`, `X-Remote-User-Display-Name`.
- `X-Aquila-Request-Id` — opaque correlation id, echoed in every response.

The core verifies the HMAC in constant time when a key is configured and rejects
the request if the signature is missing or invalid. Ingress therefore provides
**user authentication**; the core does not re-authenticate the HA user.

### 3.3 Local token

A process that prefers not to rely on the loopback peer may authenticate with a
bearer token:

- Generated at first start, 256 bits, written to `<data>/core.token`, mode `0600`.
- Presented as `Authorization: Bearer <token>`.
- Compared in constant time; never logged; rotated by deleting the file and
  restarting the core.

A token **MUST NOT** be passed in a query string (log leakage); the core rejects
`?token=`, `?access_token=` and similar with `401` **before** any other check,
including on a loopback connection.

### 3.4 Authorisation

v1 has a single role: an authenticated principal may read and write everything in
the single Aquila home. There is no tenant model and no per-room ACL. The API
records the acting identity on every mutation for the audit log.

### 3.5 Failure responses

| Condition | Status | `code` |
| --- | --- | --- |
| No accepted trust mode | 401 | `unauthenticated` |
| Bad token / bad ingress HMAC | 401 | `unauthenticated` |
| Token present in a query string | 401 | `unauthenticated` |
| Authenticated but disallowed | 403 | `forbidden` |

## 4. Health and capabilities

| Method | Path | Auth | Purpose |
| --- | --- | --- | --- |
| `GET` | `/api/v1/live` | none | Cheap liveness for container healthchecks; `200 {"status":"ok"}`. No data. |
| `GET` | `/api/v1/health` | required | Subsystem health (§4.1). |
| `GET` | `/api/v1/capabilities` | required | Version + feature negotiation (§4.2). |

### 4.1 `GET /api/v1/health`

```json
{
  "status": "ok",
  "uptimeMs": 813000,
  "api": { "major": 1, "minor": 0 },
  "subsystems": [
    { "name": "sensing", "status": "ok" },
    { "name": "sources", "status": "degraded", "detail": "2 of 9 nodes offline" },
    { "name": "models",   "status": "ok", "detail": "pose@3f1a, vitals@9c07" },
    { "name": "clock",    "status": "ok", "detail": "9/9 synced, max drift 3.1 ppm" },
    { "name": "ledger",   "status": "ok", "detail": "commit 9f2c41e" },
    { "name": "calibration", "status": "ok", "detail": "global, committed 2026-10-02" }
  ]
}
```

`status` is the worst of `subsystems`: `ok` < `degraded` < `down`.

### 4.2 `GET /api/v1/capabilities`

```json
{
  "api": { "major": 1, "minor": 0 },
  "transports": { "http": true, "websocket": false },
  "features": {
    "polling": true, "calibrationWizard": false,
    "nodeControl": false, "firmwareOta": false
  },
  "limits": { "httpBodyBytes": 1048576 },
  "minComponentVersion": "0.1.0"
}
```

Clients MUST call this once per connection and MUST refuse to operate if the
returned `api.major` differs from theirs. In v1 `transports.websocket` is
`false`: the stream is HTTP polling only (§10), and `GET /api/v1/stream` returns
a typed `501`.

## 5. Conventions

- **Identifiers** are prefixed ULIDs (Crockford base32, lowercase): `hom_`, `flr_`,
  `rom_`, `nod_`, `prx_`, `cal_`, `cmd_`. Opaque to clients; never parsed.
- **Timestamps** are RFC 3339, UTC, millisecond precision, e.g.
  `2026-10-13T20:45:00.000Z`.
- **Concurrency.** Every mutable resource carries an integer `revision`. `PATCH`
  and `DELETE` on an existing resource REQUIRE `If-Match: "<revision>"`; a missing
  header is `428 precondition_required`, a stale one is `412 precondition_failed`.
  This prevents lost updates from the dashboard and the component racing.
- **Idempotency.** `POST` accepts `Idempotency-Key`. The core stores the response
  for 24 h and replays it for a repeated key with an identical body; a repeated key
  with a different body is `409 idempotency_conflict`. Required in practice for node
  commands and calibration starts.
- **Unknown fields** in requests are ignored; unknown fields in responses MUST be
  ignored by clients. Unknown query parameters are ignored.
- **Enumerations** are lowercase snake_case; clients MUST tolerate unknown values
  by treating them as the closest known state or, where impossible, ignoring the
  event.

## 6. Endpoint table

All paths are relative to `/api/v1`. `R` = requires auth; all mutating endpoints
require `If-Match` where a `revision` exists.

| Method | Path | Purpose | Idem. |
| --- | --- | --- | --- |
| `GET` | `/state` | Full current snapshot: every zone's readings plus health (client poll / initial load). | — |
| `GET` | `/schema` | Home → floor → room tree, metric vocabulary and units. | — |
| `GET` | `/home` | Read the home (name, timezone, floor count). | — |
| `PATCH` | `/home` | Update home metadata. | — |
| `GET` | `/floors` | List floors (ordered by `index`). | — |
| `POST` | `/floors` | Create a floor. | key |
| `GET` | `/floors/{floorId}` | Read a floor. | — |
| `PATCH` | `/floors/{floorId}` | Update a floor. | — |
| `DELETE` | `/floors/{floorId}` | Delete a floor (fails if it has rooms). | — |
| `GET` | `/floors/{floorId}/rooms` | List rooms on a floor. | — |
| `POST` | `/floors/{floorId}/rooms` | Create a room on a floor. | key |
| `GET` | `/rooms/{roomId}` | Read a room. | — |
| `PATCH` | `/rooms/{roomId}` | Update a room (incl. `floorId` to move it). | — |
| `DELETE` | `/rooms/{roomId}` | Delete a room (fails if it has nodes). | — |
| `GET` | `/nodes` | List nodes; filter `?roomId=&state=`. | — |
| `POST` | `/nodes` | Register/commission a node. | key |
| `GET` | `/nodes/{nodeId}` | Read a node. | — |
| `PATCH` | `/nodes/{nodeId}` | Update a node (incl. `roomId` assignment). | — |
| `DELETE` | `/nodes/{nodeId}` | Deregister a node. | — |
| `GET` | `/nodes/{nodeId}/commands` | List recent commands for a node. | — |
| `POST` | `/nodes/{nodeId}/commands` | Send a control command (§9). | key |
| `GET` | `/nodes/{nodeId}/commands/{commandId}` | Read a command's outcome. | — |
| `GET` | `/node-pairs` | List node pairs; filter `?roomId=&floorId=`. | — |
| `POST` | `/node-pairs` | Declare a pair. | key |
| `GET` | `/node-pairs/{pairId}` | Read a pair. | — |
| `PATCH` | `/node-pairs/{pairId}` | Update a pair (enable/weight). | — |
| `DELETE` | `/node-pairs/{pairId}` | Delete a pair. | — |
| `POST` | `/node-pairs/derive` | Suggest pairs from calibration data. | key |
| `GET` | `/calibration/current` | Active calibration session, or `null`. | — |
| `POST` | `/calibration` | Start a wizard session (v1: `scope: global`). | key |
| `GET` | `/calibration/{sessionId}` | Session state + step list. | — |
| `GET` | `/calibration/{sessionId}/steps/{stepId}` | Read one step's instruction/state. | — |
| `POST` | `/calibration/{sessionId}/steps/{stepId}/complete` | Submit step result / confirm. | key |
| `POST` | `/calibration/{sessionId}/steps/{stepId}/skip` | Skip a skippable step. | key |
| `POST` | `/calibration/{sessionId}/commit` | Finalise and persist the calibration. | key |
| `POST` | `/calibration/{sessionId}/abort` | Abort and discard the session. | key |
| `GET` | `/metrics/snapshot` | Read the commit-keyed metric snapshot (§13). | — |
| `GET` | `/firmware` | Firmware image store manifest (`firmware-ota.md` §7). | — |
| `GET` | `/nodes/{nodeId}/firmware` | Installed version, boot partition, pending state, last OTA result. | — |
| `POST` | `/nodes/{nodeId}/firmware/install` | Begin a firmware OTA (`{image_id, force?}`) → job id. | key |
| `GET` | `/nodes/{nodeId}/firmware/job` | OTA job progress/state. | — |
| `POST` | `/nodes/{nodeId}/firmware/abort` | Abort an in-progress OTA. | key |
| `GET` | `/health` | Subsystem health (§4.1). | — |
| `GET` | `/capabilities` | Version/feature negotiation (§4.2). | — |
| `GET` | `/stream` | WebSocket upgrade — `501` in v1; clients poll (§10). | — |

No other routes exist. Unknown paths return `404 not_found`.

**Implementation status.** The core OTA subsystem landed with ticket #13: the
firmware routes are served from the signed image store and transfer manager.
`POST .../firmware/install` begins a job (`202`), `GET .../firmware/job` reports
progress, and `POST .../firmware/abort` aborts it; the mutating routes return a
typed `503 ota_unavailable` only when the core has no store or node transport
wired. The node firmware agent (ticket #14) and the HA `update` entity (ticket
#15) remain. The topology CRUD and calibration routes are the dashboard's
contract (ticket #10); the HACS component consumes the read-only subset listed
in `home-assistant.md` §5.2.

## 7. Resource representations

Representations are canonical and stable within a major version. Selected fields:

**Floor** — `{ "id", "name", "index", "revision" }`. `index` is the floor order
(0 = lowest); it is unique per home.

**Room** — `{ "id", "floorId", "name", "label", "bounds", "revision" }`. `label`
is one of `living`, `bedroom`, `kitchen`, `bathroom`, `hall`, `stair`, `study`,
`garage`, `other`. `bounds` is an optional 2-D polygon in floor-local metres.

**Node** — `{ "id", "name", "roomId", "hardware", "mac", "ip", "firmware",
"antennaCount", "state", "capabilities", "revision" }`. `hardware` is `esp32s3`
(reference) or `esp32c6` (future). `state` is one of `offline`, `online`,
`capturing`, `degraded`, `error`. `capabilities` lists e.g. `csi_ht20`, `csi_ht40`.
Read-only runtime fields (`ip`, `firmware`) are accepted only from the core, not
from clients.

**NodePair** — `{ "id", "nodeA", "nodeB", "enabled", "weight", "quality",
"source", "revision" }`. `source` is `declared` or `derived`. `quality` is a
core-computed `[0,1]` link score; read-only.

**Home** — `{ "id":"hom_...", "name", "timezone", "floors", "revision" }`.
Singleton at `/home`.

List endpoints return `{ "items": [ ... ], "next": null }`; pagination is not used
in v1 because the resource set is small (a home, its floors/rooms, and ~9+ nodes).

## 8. Calibration wizard

Calibration is a server-driven wizard. The core owns the step plan; the client is
a renderer and a data-entry surface. v1 supports global whole-home calibration
only; the staged room→floor→cross-floor plan is expressed as future step kinds
without changing the envelope.

A session is `{ "id", "scope":"global", "status", "steps":[...], "currentStepId",
"revision" }` where `status` is one of `pending`, `running`, `awaiting_user`,
`computing`, `ready`, `committed`, `aborted`.

Each step is `{ "id", "kind", "order", "status", "awaiting", "instruction",
"inputs", "progress", "result" }`. `awaiting` is `user` (waiting on a human
action), `core` (the core is measuring/computing), or `none` (informational).
`progress` is `[0,1]` where applicable.

Step kinds for v1:

| `kind` | `awaiting` | What the client does |
| --- | --- | --- |
| `node_health` | core | Displays per-node link health; blocks if too few nodes are healthy. |
| `baseline_empty` | user | Instructs the occupant to leave; core captures the empty-house baseline. |
| `room_tag` | user | Asks the user to stand in each room so the core can tag cell-to-room. |
| `path_walk` | user | Guides a walk along each room→room path; core measures Doppler signatures. |
| `pair_quality` | core | Core scores each node pair; client may enable/disable pairs. |
| `global_fit` | core | Core fits the global model/phase/clock correction. |
| `verify` | user | Replays a short capture and reports expected vs observed metrics. |
| `commit` | user | Presents the summary; `POST .../commit` persists it to the ledger. |

`POST /calibration` returns `201` with the session and `Location`. Completing a
step is `POST .../steps/{stepId}/complete` with `{ "inputs": {...} }`; the core
validates, advances, and may return the next step inline or stream progress over
WS (`calibration_progress`). Only one session may be active per home; starting a
second returns `409 calibration_in_progress`. Committing writes a ledger entry and
is atomic.

## 9. Node control

`POST /nodes/{nodeId}/commands` body: `{ "command": "...", "args": {...} }`.
Returns `202 Accepted` with a command resource:

```json
{ "id": "cmd_01J...", "nodeId": "nod_...", "command": "set_channel",
  "args": { "channel": 6, "bandwidthMhz": 40 },
  "state": "accepted", "issuedAt": "2026-10-13T20:45:00.000Z", "result": null }
```

`state` progresses `accepted` → `dispatched` → `done` | `failed` | `timeout`.
The outcome is delivered as a `node_command` stream event and remains readable via
`GET .../commands/{commandId}`. Commands are relayed to nodes over the direct node
control channel; the core does not expose that channel to clients.

| `command` | `args` | Effect |
| --- | --- | --- |
| `start_capture` | `{ "channel"?, "bandwidthMhz"?, "rateHz"? }` | Begin CSI streaming. |
| `stop_capture` | `{}` | Stop CSI streaming. |
| `set_channel` | `{ "channel", "bandwidthMhz" }` | Retune; core validates against the plan. |
| `set_rate` | `{ "rateHz" }` | Throttle CSI reporting. |
| `sync_now` | `{}` | Force a time-sync sample (clock offset/drift refresh). |
| `identify` | `{ "seconds" }` | Blink the node LED for physical identification. |
| `ping` | `{}` | Liveness round-trip with `rttUs`. |
| `reboot` | `{}` | Reboot the node. Destructive; requires a fresh `If-Match` on the node. |

Unknown commands are `422 validation_failed`. Commands to an `offline` node return
`409 node_offline` unless the command is queued (only `reboot` and `ping` are
never queued).

## 10. Transport: HTTP polling (v1)

**v1 does not implement the WebSocket stream.** `GET /api/v1/stream` returns a
typed, fail-closed `501 websocket_unimplemented`, so no client ever
reconnect-loops against a stream that cannot carry state. Both in-tree clients
therefore **poll** the same authenticated HTTP surface.

This is a deliberate v1 scope decision, not an omission: the stream protocol is
recorded below so it can be implemented behind the same envelope without a
major-version bump, but until it lands polling is the contract and the clients
MUST NOT open a socket.

`GET /api/v1/stream` responses:

| Condition | Status | `code` |
| --- | --- | --- |
| `Upgrade: websocket` present | 501 | `websocket_unimplemented` |
| plain `GET` (no upgrade) | 400 | `validation_failed` |

### 10.1 Polling contract

- The HACS component polls `GET /api/v1/state` (the coordinator's reconciliation
  and primary source), and fetches `GET /api/v1/nodes`, `GET /api/v1/firmware`
  and, per node, `GET /api/v1/nodes/{nodeId}/firmware` best-effort on the same
  cadence (`home-assistant.md` §6.2). Its default interval is 60 s.
- The dashboard polls the §6 routes it needs; it opens no socket
  (`dashboard.md` §5).
- Poll responses are ordinary §6 resources: there is no envelope, no `seq` and
  no `subscribe`. Staleness is judged from the client's own successful-poll time
  (or `uptimeMs` where a resource carries it).
- Polling uses `GET`, is idempotent, and is subject to the read limits in §12.
  Clients MUST back off (base 1 s, factor 2, cap 30 s, with jitter) after a
  failed poll, exactly as a stream client would after a drop.

### 10.2 The deferred stream protocol

When the RFC 6455 stream is implemented (a future ticket) it will reuse the
`aquila.v1` envelope already reserved here: a `welcome` first frame, a monotonic
per-connection `seq`, `subscribe`/`subscribed`/`event`/`heartbeat` frames, and
the event kinds below. Until then `capabilities.transports.websocket` is `false`
and clients MUST treat `501` as "use polling", not as a retryable transport
error.

```json
{ "v": 1, "type": "event", "seq": 12, "ts": "2026-10-13T20:45:00.000Z",
  "id": "sub_01J...", "data": { "kind": "presence", "roomId": "rom_...",
    "present": true, "probability": 0.94 } }
```

| `kind` | Payload |
| --- | --- |
| `presence` | `{ "roomId", "present": bool, "probability": 0..1 }` |
| `count` | `{ "roomId", "count": int, "confidence": 0..1 }` |
| `pose` | `{ "roomId", "trackId", "frame": [ { "joint", "x", "y", "z", "confidence" } ], "quality" }` |
| `vitals` | `{ "roomId", "subjectId", "breathingBrpm", "heartBpm", "confidence", "windowS" }` |
| `node_status` | `{ "nodeId", "state", "rssi", "rateHz", "clockOffsetUs", "driftPpm" }` |
| `calibration_progress` | `{ "sessionId", "stepId", "status", "progress", "awaiting" }` |
| `node_command` | `{ "commandId", "nodeId", "command", "state", "result" }` |
| `measurement` | `{ "commit", "name", "kind", "unit", "value", "n", "gate" }` |
| `health` | Same shape as §4.1. |
| `firmware` | `{ "node", "state", "progress" }` |

Close codes and the backpressure model (`drop_oldest` / `disconnect`, a bounded
outbound queue) are reserved for that stream: `1000` normal, `1001` going away,
`1008` policy violation, `1013` try again later, `4001` unsupported protocol
version, `4002` too many subscriptions, `4003` message too large.

## 11. Error model

Errors use `application/problem+json` (RFC 9457) with two Aquila extensions:
`code` (a stable machine string) and `retryable` (bool). HTTP status is the
primary signal; `code` is the stable contract.

```json
{ "type": "https://aquila.invalid/problems/node_offline",
  "title": "Node is offline", "status": 409, "code": "node_offline",
  "retryable": true, "detail": "nod_01J... has not reported for 240 s",
  "instance": "/api/v1/nodes/nod_01J.../commands", "requestId": "req_..." }
```

(The `type` URI is a namespace, not a resolvable page — there is no public
documentation by design.)

| `code` | HTTP | Retryable | Meaning |
| --- | --- | --- | --- |
| `unauthenticated` | 401 | no | Missing/invalid credentials. |
| `forbidden` | 403 | no | Authenticated but not permitted. |
| `not_found` | 404 | no | Resource or route absent. |
| `conflict` | 409 | no | Generic state conflict. |
| `calibration_in_progress` | 409 | no | A session is already active. |
| `node_offline` | 409 | yes | Command cannot reach an offline node. |
| `idempotency_conflict` | 409 | no | Same key, different body. |
| `precondition_failed` | 412 | no | `If-Match` revision mismatch. |
| `payload_too_large` | 413 | no | Body exceeds `httpBodyBytes`. |
| `unsupported_media_type` | 415 | no | Body is not JSON. |
| `validation_failed` | 422 | no | Schema or semantic validation failed. |
| `precondition_required` | 428 | no | Missing `If-Match` on a mutable update. |
| `rate_limited` | 429 | yes | Token bucket exhausted; see `Retry-After`. |
| `internal` | 500 | yes | Unhandled fault. |
| `websocket_unimplemented` | 501 | no | v1 has no WebSocket stream; poll instead (§10). |
| `ota_unavailable` | 503 | yes | Firmware OTA is not built yet; the route fails closed (§6). |
| `degraded` | 503 | yes | Subsystem unavailable (e.g. models loading). |
| `version_unsupported` | 400 | no | Client major version unsupported. |

Validation errors include a `errors` array of
`{ "pointer": "/floors/0/index", "code": "out_of_range", "detail": "..." }` using
JSON Pointer. Errors never include stack traces, internal paths or tokens.

## 12. Rate and streaming limits

Limits are enforced per principal (token or ingress user) and per connection.
`429` responses set `Retry-After` (seconds). WS limit breaches emit a
non-fatal `error` frame, then close if the client persists.

| Scope | Limit |
| --- | --- |
| HTTP request body | 1 MiB (`payload_too_large` beyond). |
| HTTP read endpoints | 20 req/s, burst 40. |
| HTTP write endpoints | 5 req/s, burst 10. |
| Node control commands | 2 req/s, burst 4 per node. |
| Calibration mutating calls | 2 req/s, burst 4. |
| Concurrent HTTP requests | 64 per principal. |
| WS connection frame size | 64 KiB. |
| WS subscribe message size | 256 KiB total; ≤ 32 kinds. |
| WS subscriptions per connection | 32. |
| WS outbound queue | 256 KiB / 512 frames (see §10.6). |
| WS heartbeat / idle | 15 s / 45 s. |
| WS sustained event rate | 200 frames/s, burst 400; excess conflated or dropped. |
| Stream default/max rates | See §10.4. |

The dashboard and the component are expected to be far below these ceilings; the
limits exist to contain a malfunctioning client, not to meter normal use.

## 13. Metrics snapshot

`GET /api/v1/metrics/snapshot` is read-only and never triggers measurement.

Query: `?commit=<sha>` (default: current ledger commit), `?kinds=` a comma list of
`deterministic,statistical,gated` (default: all).

```json
{
  "commit": "9f2c41e", "generatedAt": "2026-10-13T20:45:00.000Z", "revision": 17,
  "metrics": [
    { "name": "accuracy.presence_f1", "kind": "gated", "unit": "ratio",
      "value": 0.94, "n": 12000, "reference": "camera-gt",
      "gate": { "bar": 0.9, "pass": true } },
    { "name": "accuracy.torso_pck_at_20", "kind": "statistical", "unit": "ratio",
      "value": 0.67, "n": 8400, "ci95": [0.64, 0.70] },
    { "name": "accuracy.breathing.mae_brpm", "kind": "statistical", "unit": "brpm",
      "value": 1.8, "n": 310 }
  ],
  "bars": {
    "torsoPck20":     { "target": 0.65, "observed": 0.67, "pass": true },
    "presenceF1":     { "target": 0.90, "observed": 0.94, "pass": true },
    "breathingMaeBrpm": { "target": 2.0, "observed": 1.8, "pass": true }
  }
}
```

The snapshot reflects the v1 bars exactly and carries no claim beyond the data it
can cite. `commit` is the ledger commit the values were computed at; a client that
caches a snapshot MUST key its cache on `commit`, not on wall-clock time.

## 14. Sequence example — calibrate then poll

```
client → GET  /api/v1/capabilities
client → GET  /api/v1/schema                                    → zone tree + metrics
client → POST /api/v1/calibration            (Idempotency-Key)   → 201 session
client → GET  /api/v1/calibration/{id}                           → running
client → GET  /api/v1/calibration/{id}                           → step baseline_empty, awaiting_user
client → POST /api/v1/calibration/{id}/steps/{sid}/complete  { inputs:{} }
client → GET  /api/v1/calibration/{id}                           → step global_fit, awaiting_core
client → GET  /api/v1/calibration/{id}                           → step commit, awaiting_user
client → POST /api/v1/calibration/{id}/commit                    → 200 committed
client → GET  /api/v1/state            (every poll interval)     → snapshot + health
client → GET  /api/v1/nodes                                      → node list + firmware
```

There is no `WS /api/v1/stream` step in v1 (§10): the client polls the resources
above on its interval.

## 15. Security notes

- **The unpublished port and the private network are the boundary.** The add-on
  binds its container interface but publishes no host or LAN port, so only the
  add-on's own process and the HA core container on the private Docker network
  can reach it; a public peer is rejected. There is no TLS termination, no
  certificate management and no cloud relay — and no encryption either: the
  private network is trusted for *placement*, not confidentiality. A component
  that must reach the core does so either directly over that private network or
  through the add-on ingress seam (§2), never by widening the bind.
- **Three trust modes, one role.** A trusted (loopback or private) peer, the
  ingress identity header, and the local bearer token are independently
  verifiable and mutually exclusive per request; none grants a different
  authorisation.
- **Constant-time comparison** for both the HMAC and the token; failures are
  indistinguishable to the caller.
- **No secrets in logs, errors or query strings.** Request bodies are not logged;
  the token and HMAC key never appear in `/health`, `/capabilities` or any error.
- **Mutation audit.** Every write and node command records acting principal,
  request id, timestamp and resource revision in the add-on audit log.
- **Denial-of-service containment.** The limits in §12 protect the sensing
  pipeline from the API, not the reverse.

## 16. Open questions

- The exact HA add-on/ingress network topology (which process terminates ingress)
  is an ADR, not a spec decision; §2 states only the invariant the core relies on.
- Whether `node-pairs/derive` should be read-only (suggestions only) or able to
  persist. Current intent: suggestions only; persistence is an explicit client
  `POST /node-pairs`.
- Whether non-global calibration scopes need their own WS progress kinds; the
  envelope already accommodates them without a version bump.
