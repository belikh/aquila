# Privacy & Security Specification

Status: draft for v1. Scope: the sensing core, the Home Assistant (HA) add-on, the
management dashboard, and the ESP32-S3 nodes. Out of scope: the future SDK, public
REST/WS/MCP, plugins, and phase-coherent RF reference (none exist in v1).

This document states a **privacy posture** as enforceable guarantees, gives a
**threat model**, and notes **sensing-specific risks** factually. Every guarantee
carries a testable statement. Guarantees are exercised by a privacy/security test
suite and recorded in the commit-keyed measurement ledger as deterministic (gated)
metrics — see the measurement spec. Nothing here is enforced by CI yet; the
ledger records pass/fail per commit.

Terminology follows `AGENTS.md` and the brief: Aquila, sensing core, add-on,
ingress, node, node-pair, seam (`CsiSource`, `Clock`, `PoseModel`/`VitalsModel`,
`GroundTruth`, `Sink`, `MetricSink`). Australian English throughout.

---

## 1. Privacy posture

Aquila adopts the Home Assistant philosophy: **local, self-hosted, no cloud, no
telemetry; the user owns the data; no retention policy is imposed.** In practice:

- The system is designed so that a **cloud dependency cannot be created by
  accident**: there is no telemetry, no analytics, no crash reporting, no update
  phone-home, and no remote-control path. This is a structural property, not a
  setting.
- Data is created, processed, and stored on the user's own host, inside the
  add-on's data directory. Raw CSI never leaves the LAN; derived sensing output
  never leaves the host except through the user's own HA.
- The user decides what to keep. Aquila deletes nothing on its own and imposes no
  expiry, quota, or "data improvement" collection.
- Sensing is **non-identifying by design** in v1: presence/occupancy, person
  count, coarse pose, breathing and heart rate. No faces, no audio, no identity,
  no gait fingerprint, no re-identification.

The threat model below assumes the host is not already compromised; a compromised
host is out of scope for v1 (see residual risks).

---

## 2. Enforceable guarantees

Each guarantee has an ID, a statement, how it is enforced, and how it is tested.
Tests run against the real core with **fake adapters** (fixture-replaying
`CsiSource`, deterministic `Clock`) where a full capture is not needed, and
against the real binaries where a process-level property (sockets, capabilities)
is asserted.

| ID | Guarantee (one line) |
| --- | --- |
| G1 | No telemetry and no phone-home. |
| G2 | No outbound connection outside the LAN; no name resolution. |
| G3 | Raw CSI is confined to the air and the core host; never re-emitted. |
| G4 | Management HTTP+WS is reachable only on the add-on's private network and/or through ingress; never host- or LAN-published. |
| G5 | Management requires a trusted private/loopback peer, a valid HA ingress session, or a bearer; no separate login. |
| G6 | Only enrolled nodes enter the pipeline; everything else is dropped. |
| G7 | Control commands are authenticated per node and replay-resistant; CSI ingest authentication is deferred in v1 (see §5). |
| G8 | No public API, SDK, or MCP surface exists in v1. |
| G9 | No imposed retention: Aquila never deletes or expires user data on its own. |
| G10 | Export and erasure are explicit, local, user-initiated operations. |
| G11 | The add-on runs least-privilege and non-root. |
| G12 | The dashboard runs sandboxed under a strict CSP; no third-party scripts. |
| G13 | Secrets are Supervisor-managed; logs redact CSI, biometrics, and secrets. |
| G14 | Build and supply chain are pinned, audited, licence-gated, and checked. |
| G15 | Sensing limits are disclosed honestly, in product and docs. |
| G16 | Every guarantee above is measured and ledgered. |

### G1 — No telemetry, no phone-home

**Statement.** The core, add-on, and dashboard contain no telemetry, analytics,
crash-reporting, update-check, or remote-management client. Running Aquila without
internet access loses no functionality.

**Enforcement.** No such dependency exists in the enforced dependency DAG;
forbidden crates/features (HTTP clients used for egress, analytics SDKs, crash
uploaders) are listed in the DAG check.

**Test.** Operate the core for a fixed scenario in an isolated network namespace
with only loopback and an emulated LAN. Assert the complete set of outbound
destination addresses is empty (or LAN-only, per G2). Deterministic: the
observation must hold on every run.

### G2 — No outbound connection outside the LAN; no name resolution

**Statement.** Every outbound connection or datagram is addressed to loopback or
to an enrolled node on the home LAN. No component performs DNS resolution or
contacts any address outside the LAN.

**Enforcement.** The only network-egress code is the node control channel; there
is no general-purpose HTTP client in the core. The node address set is exactly the
enrolled-node set.

**Test.** (a) Syscall-level: intercept `connect`/`sendto`/`sendmsg` for the whole
process and assert each destination is in `{127.0.0.0/8} ∪ {enrolled node
addresses}`. (b) No-DNS test: run with no resolver configured and assert no
resolution is attempted and functionality is unaffected. Both deterministic.

### G3 — Raw CSI is confined to the air and the core host

**Statement.** Raw CSI exists in exactly two places: on the Wi-Fi air between a
node and the core, and in the core host's memory and data directory. The core
never re-transmits raw CSI to any other host, never to a destination outside the
LAN, and never through a `Sink` other than the local stores.

**Enforcement.** v1 ships only local `Sink` implementations; a network `Sink` is
not a permitted module in the DAG. The node control channel carries control
messages only, never CSI payloads.

**Test.** (a) DAG test asserts no network-egress `Sink` exists. (b) Run a full
capture replay and assert the set of transmitted payloads observed on the wire
contains no CSI frame bytes (control + ingest ACKs only).

**v1 limitation (honest).** The CSI payload is **not** AEAD-sealed in v1; it
travels as plaintext in the `AQCI` UDP datagram. On-the-air confidentiality
therefore depends entirely on the WLAN's link-layer encryption. On a WPA2/WPA3
personal network this still keeps subcarriers out of reach of an off-network
observer, but a device sharing the same PSK (or an open network) could recover
them. Per-node sealing is deferred; see G7 and §5.

### G4 — Management HTTP+WS is private-network/ingress only

**Statement.** The private HTTP+WS server used by the dashboard and the HACS
component binds only to the loopback interface, the add-on's private container
interface, and/or the Supervisor-provided ingress socket. It never binds a host
LAN interface, and its port is not published to the host.

**Enforcement.** Add-on configuration enables ingress and publishes no `ports`
mapping; where the core binds a non-loopback address it is the add-on's private
container interface, reachable only on the `hassio` Docker network
(`home-assistant.md` §2).

**Test.** Start the add-on; enumerate listening sockets and assert every
HTTP+WS listener is loopback, a private (RFC 1918 / link-local / IPv6 ULA)
address, or the ingress socket, and that no listener is bound to a public
address and no host `ports` mapping is published. Deterministic.

### G5 — Management requires a valid identity

**Statement.** The dashboard is reachable only through HA Ingress, which
authenticates the HA user. The management API deliberately also accepts a
**trusted private/loopback peer** — the HACS component in the HA core container
reaches it directly over the private Docker network (`core-api.md` §3.1,
`home-assistant.md` §5.2) — and a configured **bearer token** for other
placements. Aquila implements no second password store and no login of its own.
Requests that lack all three (no trusted peer, no Supervisor ingress identity
headers, no bearer) are refused.

**Enforcement.** Validate `X-Remote-User-Id` (and related ingress headers) on
every request; otherwise require a loopback/private peer or a constant-time
bearer check; reject when none is present. WebSocket upgrades are same-origin and
checked. No wildcard CORS; the `Host` header is never trusted for routing.

**Test.** (a) Request the management endpoint with no ingress headers from a
**public** peer → refused. (b) Request from a private peer on the container
network but off the ingress path → accepted as a trusted peer (the component
path), while the same request from a public peer → refused. (c) Assert the
add-on ships no password/user database. Deterministic.

### G6 — Only enrolled nodes enter the pipeline

**Statement.** A node must be enrolled before any of its packets are processed.
Unenrolled or unknown sources are dropped before parsing and counted; they cannot
influence sensing, calibration, or measurement. **v1 limitation:** because ingest
is not authenticated (G7), a source that spoofs an *enrolled* node's address is
not distinguishable from that node in v1; spoofed-source detection on the data
plane is deferred with ingest authentication.

**Enforcement.** Enrolment is an out-of-band pairing step (user-triggered in the
dashboard; node presents a one-time secret). The core maintains the per-node
pairing token (used to authenticate **control** commands) and an enrolled-node
**address** allowlist; the ingest path is allowlist-first, default-deny.

**Test.** Send CSI-shaped UDP from an unenrolled source → zero frames enter the
pipeline, the drop counter increments, and the sensing output is unchanged. Then
enrol and assert frames are accepted. Deterministic.

### G7 — Authenticated, replay-resistant control; ingest authentication deferred

**Statement (v1, honest).** Control commands from the core to a node are
authenticated per node and replay-resistant. CSI **ingest** is **not**
authenticated in v1: the ingest path trusts the source address of an enrolled
node, which an on-LAN attacker can spoof, and no packet-level MAC or seal is
present. This is a stated v1 limitation, not an enforced guarantee.

**Enforcement (control).** A paired node authenticates every inbound command
with a 32-byte HMAC-SHA256 trailer keyed by the pairing token issued in
`JOIN_ACK` (`capture-and-firmware.md` §6.1, §6.3), pins the core's address, and
rejects a `request_id` seen in a bounded replay window. The trailer is verified
in constant time; a message with no tag, the wrong tag length, or a wrong tag is
rejected before any handler runs. The node **fails closed** until the core's
sender attaches the trailer, so v1 authenticates the core → node direction only;
node → core replies and `STATUS` are not yet authenticated, and "mutually
authenticated" is a target, not a v1 property.

**Enforcement (ingest, deferred).** v1 relies on the **G6** enrolment allowlist
and network placement: raw CSI is confined to the LAN (G3 niche below), a
recommended dedicated node SSID/VLAN limits who can inject, and frames are
parsed by a bounded, total parser. There is no per-node seal, key-derived
identity, or authenticated sequence number on the data plane in v1.

**Test.** (a) A control command with a tampered body or tag → rejected, no state
change. (b) A control command with a stale `request_id` → rejected as a replay.
(c) Fuzz the control and ingest parsers with malformed/oversized input under a
memory-safety harness (Rust) → no panic, no unsafe behaviour. Ingest
authentication has no v1 test because there is no v1 mechanism; the deferred
mechanism must land with its own negative tests (§5). Deterministic.

### G8 — No public API, SDK, or MCP surface

**Statement.** v1 exposes no public REST/WS/MCP endpoint and no plugin/SDK
surface. The only data consumers are in-tree modules, the bundled HACS
component, and the ingress dashboard.

**Enforcement.** The API-facing crate does not depend on the server crate (DAG);
the only listener is the private management server of G4/G5.

**Test.** DAG test asserts no public API crate depends on internals and no
externally advertised API module exists. Deterministic.

### G9 — No imposed retention

**Statement.** Aquila never deletes or expires user data on its own. There is no
default TTL, no auto-purge, and no quota that silently discards data. Retention
and deletion are the user's policy; the default is to retain.

**Enforcement.** No retention/deletion scheduler runs unless the user configures
one; data-handling code paths that remove data are user-invoked only.

**Test.** Run for a fixed scenario with no retention policy set and assert no
file under the data directory is removed and no deletion job is scheduled.
Deterministic.

### G10 — Export and erasure are explicit and local

**Statement.** Export and deletion of user data are explicit, user-initiated,
local operations confined to the add-on data directory. Nothing is uploaded, and
no "share"/"sync" path exists.

**Enforcement.** Export writes to a user-chosen local path; deletion is an
explicit user action. Both are outside the automatic pipeline.

**Test.** (a) Trigger export and assert the write destination is local and within
the permitted set; assert no network destination is contacted (ties to G2). (b)
Assert no automatic code path calls the deletion routine. Deterministic.

### G11 — Least-privilege, non-root add-on

**Statement.** The add-on runs as a non-root user with all capabilities dropped,
without `privileged`, `host_pid`, or the Docker socket, with a read-only root
filesystem and write access limited to its data directory. Network access is the
minimum required for node ingest/control.

**Enforcement.** Add-on manifest declares no extra capabilities, no privileged
mode, no Docker socket; the data directory is the only writable path.
**v1 limitation:** the read-only root filesystem and dropped-capability runtime
flags are set by the image/packaging (ticket #11, `aquila_core/Dockerfile`),
which is not built in this repository; the shipped `config.yaml` requests no
extras, but the runtime hardening is unverified until that build exists (F25).

**Test.** Inspect the add-on manifest and the running container: assert the
capability set is empty (or exactly the documented minimum), the user is non-root,
the rootfs is read-only, and no sensitive host mounts exist. Deterministic.

### G12 — Sandboxed dashboard

**Statement.** The dashboard (Rust→WASM) runs under a strict Content Security
Policy with no inline scripts, no `eval`, and no remote/third-party script or
style origins. WASM receives no ambient capabilities (no filesystem, no network);
it can only use what the page passes in. WebSocket use is same-origin with Origin
validation.

**Enforcement.** Served from the same ingress origin with a CSP such as
`default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self';
connect-src 'self'; img-src 'self' data:; frame-ancestors <HA>`; no external CDNs.
**v1 limitation:** the dashboard crate is a host-buildable stub and the add-on
does not yet serve it (ticket #10), so no CSP header is emitted in v1 and this
guarantee is not yet exercised; the CSP header lands with the dashboard server
(F25). The JSON API serves no HTML, so there is nothing to sandbox yet.

**Test.** (a) Assert the served CSP header matches the policy and contains no
`unsafe-inline` or remote origins. (b) Assert the built WASM imports no
networking/filesystem host functions. (c) Assert no third-party asset origin is
referenced. Deterministic.

### G13 — Secret hygiene

**Statement.** Per-node keys and any HA token live in Supervisor-managed add-on
options, never baked into images, never written to logs, and never written to the
ledger. Logs and the measurement ledger contain no raw CSI bytes, no biometric
values, and no secrets by default; any debug capture of such data is explicit,
local, and opt-in.

**Test.** (a) Log-scrub test: run with sample frames containing known CSI bytes,
vital values, and a known secret; assert none appear in log output or the ledger.
(b) Assert no secret literal appears in the built image. Deterministic.

### G14 — Supply-chain integrity

**Statement.** Dependencies are pinned with committed lockfiles and audited
(`cargo deny`/`cargo vet`/`cargo audit`); the licence gate admits only
GPLv3-compatible licences; vendored model seed weights are checksummed and their
licence documented; an SBOM is produced; builds perform no undeclared network
access and run no install scripts.

**Test.** Assert: lockfiles present and pinned; `cargo deny` passes with the
declared licence policy; every vendored weight file matches its recorded digest;
an SBOM is emitted; a build in a network-isolated environment succeeds;
dependency DAG contains no network-egress crate (ties to G1/G2).

### G15 — Honest sensing disclosure

**Statement.** The product and docs state plainly what sensing can and cannot do,
including that sensing may extend beyond the room a node is in and may concern
people who did not consent. Output is coarse and non-identifying; no identity
recognition is offered.

**Test.** (a) Docs/lint check asserts the sensing-limits section is present and
the claims match the measured bars (honest-claims check in the measurement spec).
(b) Assert no identity/face/audio capability is exposed by any module. (Partly
procedural; the measurable parts are deterministic.)

### G16 — Every guarantee is measured

**Statement.** Each guarantee above is exercised by the privacy/security test
suite, producing deterministic per-commit results in the measurement ledger.

**Test.** The suite runs the G1–G15 checks as named, deterministic cases (e.g.
`privacy.no_outbound_sockets`, `privacy.management_not_host_published`,
`security.enrolment_required`, `security.no_dns`, `supplychain.licence_gate`) and
the ledger records each as pass/fail keyed by commit.

---

## 3. Threat model

Method: list assets, actors, the trust boundaries between them, then per-surface
threats and mitigations. v1 has exactly **three network surfaces**:

1. **Node ingest** — nodes → core, raw CSI over Wi-Fi UDP on the LAN.
2. **Node control** — core → nodes, direct control channel on the LAN.
3. **Management** — the HACS component in HA Core → HTTP+WS, directly over the
   add-on's private Docker network; and HA user → dashboard/HTTP+WS, via HA
   Ingress only.

Everything else is in-process or on-disk on the host.

### 3.1 Assets

- **Raw CSI streams** — the most sensitive asset; can reveal presence, coarse
  pose, and vitals, and can carry information about adjacent spaces.
- **Derived sensing output** — occupancy, person count, coarse pose,
  breathing/heart rate. Health-adjacent.
- **Calibration data** — node placement, floor/room geometry; reveals home layout.
- **Node keys and enrolment material** — authorise ingest and control.
- **HA session/ingress identity and any HA token** — grants management access.
- **Model weights and training captures** — MM-Fi plus own captures (may contain
  biometric ground truth).
- **Measurement ledger** — provenance and metrics (no raw CSI, by G13).
- **The add-on container and host** — the substrate for everything above.

### 3.2 Actors

- **Remote internet attacker** — no reachable surface by construction (G1–G5);
  in scope as a design constraint, not a live surface.
- **LAN attacker** — a compromised IoT device or guest on the WLAN; the primary
  realistic adversary for ingest and control.
- **Rogue/compromised node** — an enrolled node that turns hostile; spoofing,
  replay, flood.
- **Unenrolled sender** — any device emitting CSI-shaped UDP to the ingest port.
- **Curious or malicious household member or guest** — legitimate LAN/host access
  plus physical presence; the multi-occupant consent problem (see §4).
- **Another add-on on the Supervisor network** — shares the private Docker
  network, so it is trusted by peer address (`core-api.md` §3.1) and can reach
  management unless the port were published and unauthenticated; this is the
  stated v1 limitation of address-based peer trust, contained by the unpublished
  port rather than by a per-add-on credential.
- **Supply-chain attacker** — upstream crate, vendored weights, HACS release.
- **Vendor/distributor** — designed out: no phone-home, no remote control (G1).

### 3.3 Node ingest (UDP/LAN)

Threats: spoofed CSI injection to poison fusion or calibration; replay; flood /
resource exhaustion; malformed-packet parser bugs.

Mitigations: **G6** enrolment allowlist (default-deny) — frames from a source
that is not an enrolled node's address are dropped and counted, not parsed —
plus a bounded total parser, Rust memory safety, rate limits, and bounded
buffers. **v1 limitation:** ingest is not authenticated, so an on-LAN attacker
that spoofs an enrolled node's address can inject CSI-shaped datagrams; a
sequence number is present but unauthenticated. A rogue node can at worst
pollute its own contribution — mitigated by per-node attribution in fusion and
by the measurement ledger flagging anomalies.

### 3.4 Node control (core → node, LAN)

Threats: command injection; rogue core; downgrade or MITM on the LAN.

Mitigations: a pinned per-node channel whose commands are authenticated with the
pairing token (**G7**); no broadcast; the command set is allowlisted and
minimal; nodes reject unauthenticated control and fail closed. v1 authenticates
core → node only (node → core replies are not yet authenticated). A rogue core is
out of scope beyond the LAN confidentiality boundary, because control confers no
access to management or to other nodes' keys.

### 3.5 Management (HTTP+WS via ingress)

Threats: unauthenticated access; direct LAN access; another add-on reaching the
port; CSRF; WebSocket cross-origin hijack; DNS rebinding; Host-header abuse.

Mitigations: **G4** private-network/ingress-only binding with no host-published
port; **G5** ingress identity-header validation plus a trusted private/loopback
peer or bearer, and refusal otherwise;
same-origin WS with Origin checks; no wildcard CORS; `Host` never trusted;
CSRF token on state-changing calls; strict CSP (**G12**).

### 3.6 Dashboard (browser/WASM)

Threats: XSS via any rendered content; malicious page in the same browser;
sandbox escape.

Mitigations: **G12** strict CSP with no inline/eval and no remote origins; WASM
has no ambient capabilities; any dynamic text is rendered as text, not HTML;
session material is not persisted beyond the HA session; no third-party assets.

### 3.7 Add-on container and host

Threats: privilege escalation; container escape; lateral movement to HA or
Supervisor; secret theft; noisy-neighbour abuse of the Supervisor network.

Mitigations: **G11** least-privilege, non-root, read-only rootfs, no Docker
socket, no `host_pid`/`privileged`; **G13** secrets in Supervisor options;
egress restricted by **G2**; management gated by **G4/G5** with no host-published
port. Note the honest v1 limitation: because a private-network peer is trusted by
address (`core-api.md` §3.1), another container on the same `hassio` Docker
network is trusted too — the containment is the *unpublished port and private
network*, not a per-add-on credential. Node UDP requires host-network
reachability; this is the single justified network capability and is documented
and minimised.

### 3.8 Supply chain

Threats: malicious or typosquatted dependency; compromised vendored weights;
tampered HACS release; licence pollution.

Mitigations: **G14** pinning, audit, licence gate, checksummed weights, SBOM,
network-isolated builds, no install scripts. HACS releases should be version-pinned
and integrity-checked. See §5 for the NC-weights licensing caveat.

---

## 4. Sensing-specific risks (factual)

Wi-Fi CSI sensing measures how radio waves propagate and reflect through a space.
It is **not confined to a single room**: multipath and through-wall propagation
mean reflections can carry information about adjacent rooms and, at range, about
neighbouring dwellings. It is **passive** — subjects carry nothing and are not
aware they are sensed — and it **does not stop at the boundary of consent**:
guests, visitors, and neighbours have not agreed to be sensed. Stating this
plainly is required (G15); pretending otherwise would be dishonest.

What Aquila does about it, technically and honestly:

- **Coarse, non-identifying output only.** v1 yields presence/occupancy, person
  count, coarse pose, breathing and heart rate — no identity, no face, no audio,
  no gait re-identification, no recording of individuals as such.
- **Local and confined.** Data stays on the host and LAN (G1–G3); nothing is
  uploaded; there is no cloud to leak to.
- **User-controlled retention.** Nothing is deleted or shared behind the user's
  back (G9–G10); the user decides what is kept.
- **No covert channel.** There is no remote-access path, so a sensor cannot be
  repurposed by a third party (G1).
- **Configurable disclosure.** Because output is local, the user can surface a
  "sensing active" indicator in their own HA dashboard. Aquila provides the data
  and documents the risk; it cannot make a household's consent decisions.

Residual limit to state: **consent is the user's responsibility.** Aquila reduces
harm (coarse output, local-only, non-identifying, no cloud) but does not and
cannot guarantee that people in or near the home have consented. Documentation
must say so.

---

## 5. Residual risks and honest limitations

- **Compromised host** is out of scope for v1: if the host is owned, all bets are
  off. G11 limits blast radius; it does not defend an already-root host.
- **Unsealed CSI ingest (v1).** Per-node AEAD sealing of CSI payloads is
  **deferred past v1**: the data plane is plaintext `AQCI` over UDP and its
  sequence number is unauthenticated. Confidentiality therefore rests on the
  WLAN's link-layer encryption; on a shared PSK, an open network, or to an
  on-LAN attacker that spoofs an enrolled node's address, subcarriers are
  exposed and injectable. This is the single largest v1 residual risk and is
  stated in G7 and §3.3 rather than hidden behind an AEAD claim.
- **Unidirectional control authentication (v1).** Control commands are
  authenticated core → node; node → core replies and `STATUS` are not yet
  authenticated, so mutual authentication remains a follow-up.
- **Node hardware.** The ESP32-S3 reference device may not enable secure boot and
  flash encryption in v1; a physical attacker could then extract a node key and
  impersonate one node. Blast radius is one node's contribution; keys are
  per-node and rotatable. Documented as a residual risk.
- **Through-wall reach** is a property of physics, not a bug; it cannot be
  eliminated, only disclosed (G15) and constrained by output coarseness.
- **NC seed weights are not open source.** "Non-commercial" weights are not
  GPLv3-compatible, and a model derived from them may inherit NC terms. This
  tension between the GPLv3 code licence and NC-derived model artefacts is a
  supply-chain and licensing risk that must be resolved and documented (G14); it
  is flagged here rather than silently ignored.
- **No CI gating yet.** Guarantees are measured and ledgered per commit (G16) but
  are not yet enforced by CI, consistent with the measurement-first stance.

---

## 6. How guarantees are tested

- **Fake adapters and fixtures.** A fixture-replaying `CsiSource` and a
  deterministic `Clock` drive most tests without hardware or captures; a fake
  `GroundTruth` bounds biometric material in tests.
- **Process-level assertions.** Sockets (G2, G4), capabilities (G11), headers and
  ingress identity (G5), and CSP (G12) are asserted against the real binaries.
- **Static DAG assertions.** Forbidden egress crates, network `Sink` modules, and
  public-API dependencies are rejected structurally (G1, G3, G8, G14).
- **Parser fuzzing.** The ingest frame parser is fuzzed for malformed and
  oversized input (G7).
- **Ledger.** Results are recorded as deterministic metrics in the commit-keyed
  measurement ledger (G16), e.g. `privacy.no_outbound_sockets`,
  `privacy.management_not_host_published`, `privacy.no_imposed_retention`,
  `security.enrolment_required`, `security.replay_rejected`,
  `security.no_dns`, `dashboard.csp_strict`, `supplychain.licence_gate`.
