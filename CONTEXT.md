# Aquila

Aquila is WiFi CSI sensing for the home: fixed nodes stream raw CSI to a sensing
core that infers presence, occupancy, person count, coarse pose and vital rates.
Home Assistant hosts the system and its UI. This file is the canonical
vocabulary for the whole repository; every skill, doc, issue and test name uses
these terms and no others.

Single bounded context: the entire `aquila` repository. Concepts outside it are
listed under **Explicitly not used**.

## Language

### Signal

**CSI frame**:
One received 802.11 packet's complex channel state information from a single
node: a complex coefficient per subcarrier plus capture metadata. The raw
wireless observation; a modem output, not raw I/Q.
_Avoid_: measurement, sample, packet, trace.

**Subcarrier**:
One OFDM frequency bin within a CSI frame, carrying a single complex channel
coefficient (amplitude and phase). ~64 at 20 MHz HT, ~128 at 40 MHz.
_Avoid_: bin, tone, channel.

**Sensing frame**:
The core's atomic unit of analysis: the CSI frames of one node or node-pair,
time-aligned onto a common clock grid over a fixed window. The input to feature
extraction and inference.
_Avoid_: window, batch, chunk, buffer.

**Session**:
A recorded, replayable, time-bounded stream of CSI frames from one or more
nodes, with metadata and provenance. Sessions drive development and scoring
ahead of live deployment.
_Avoid_: recording, capture, trace, log, dataset, run.

### Physical topology

**Home**:
One calibrated dwelling: all its floors, rooms, nodes, node-pairs and its global
calibration. The unit of installation and of global calibration.
_Avoid_: site, installation, deployment, house, property.

**Floor**:
A storey of a home, containing rooms. Cross-floor sensing is a staged
capability, not a v1 guarantee.
_Avoid_: level, storey.

**Room**:
A named physical partition of a floor. Rooms are the v1 spatial scope of a
sensing result.
_Avoid_: area, space, location.

**Zone**:
A named spatial region that a sensing result is attributed to. Rooms are zones;
a zone may also be a declared sub-region of a room. v1 emits one zone per room.
_Avoid_: region, area, location, sector.

### Hardware and topology

**Node**:
A single fixed ESP32-S3 endpoint that streams raw CSI. A dumb sensor: it performs
no DSP, inference or fusion. Identified by a stable node id and assigned to
exactly one room.
_Avoid_: sensor, device, client, STA, AP, access point, gateway, endpoint, radio.

**Node-pair**:
An ordered pair of nodes whose CSI frames are jointly processed to sense the path
between them. The unit of spatial sensing and the edge of the sensing graph.
_Avoid_: link, edge, mesh link, peer, MIMO pair.

### Inferred outputs

**Presence**:
The per-zone binary claim that at least one person is present. v1 gated metric:
F1 ≥ 0.9.
_Avoid_: detection, occupancy, motion, activity.

**Occupancy**:
The whole-home fused state of which zones are occupied and by how many people;
the home-level combination of per-zone presence.
_Avoid_: presence, count, activity.

**Count** (person count):
The number of people attributed to a zone or to the home.
_Avoid_: headcount, people-count, census.

**Pose**:
A coarse per-person configuration of body keypoints in a zone, estimated from
CSI. Low-fidelity by design. v1 gated metric: in-domain torso-PCK@20 ≥ 65%.
_Avoid_: skeleton, tracking, body model, mesh.

**Keypoint**:
One named body joint (e.g. shoulder, hip) within a pose.
_Avoid_: joint, landmark, marker, node.

**Vitals**:
The physiological rates estimated from CSI: breathing rate and heart rate.
_Avoid_: vital signs, biometrics, physiology, health metrics.

**Breathing rate**:
Breaths per minute (brpm) estimated from CSI. v1 gated metric: MAE ≤ 2 brpm
against a reference.
_Avoid_: respiration rate, resp rate, breathing frequency.

**Heart rate**:
Beats per minute (bpm) estimated from CSI. No HRV, SpO2 or rhythm analysis.
_Avoid_: pulse, HR.

### Calibration and measurement

**Calibration**:
The per-home process that corrects node and node-pair CSI to a common reference,
and the stored corrections it produces. v1 is global — one calibration for the
whole home — not per-room.
_Avoid_: training, tuning, commissioning, setup, normalisation.

**Baseline**:
The node-pair or zone reference state recorded with no person present, against
which deviations (presence, motion, vitals) are measured.
_Avoid_: ground truth, background, empty state, resting state.

**Ground truth**:
An independent reference measurement used to score a sensing estimate (e.g. a
camera or a reference breath belt). Research-only; never a runtime input.
_Avoid_: baseline, truth, label.

**Witness**:
The evidence grade recording how a result was established. Every sensing claim
carries exactly one of **five** grades, strongest first (the single authoritative
set, declared by `aquila_types::Witness` and `data-contracts.md` §1; `evaluation.md`
§5 refines the same five):
- **Gated**: measured and passing a defined v1 bar; the only grade a headline
  claim may carry.
- **Measured**: computed from real, replayed or simulated input and recorded in
  the commit-keyed ledger.
- **Claimed**: asserted without a recorded measurement.
- **Data-gated** (`DataGated`): blocked on missing input (fixture, labels,
  reference recording or paired samples); a blocked sub-grade of Claimed.
- **Hardware-gated** (`HardwareGated`): blocked on missing hardware (reference
  device, belt, camera rig or the reference-scale node deployment); a blocked
  sub-grade of Claimed.
_Avoid_: proven, verified, validated, certified, trusted, ground truth.

## Explicitly not used

Aquila does not use these concepts or their synonyms; do not introduce them.

- **Transports**: ESP-NOW, MQTT, Matter, Zigbee, BLE mesh, PoE.
- **Node roles**: access point, station/client, gateway, mesh router.
- **Interfaces**: public REST/WS/MCP API, SDK, plugins (v1 is in-tree only).
- **Signal**: raw I/Q, SDR, Doppler, phase-coherent RF reference. ESP32-as-SDR
  and camera ground truth are research-only, never v1 concepts.
- **Inference beyond scope**: person identity, recognition, gait,
  re-identification, fall detection, through-wall imaging, HRV, SpO2.
- **Calibration staging**: per-room or per-floor calibration is staged later,
  not a v1 concept.
