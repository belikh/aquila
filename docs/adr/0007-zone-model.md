# ADR-0007: Zones are the only spatial concept — no rooms, no floors

- Date: 2026-10-16
- Status: Accepted
- Supersedes: ADR-0007 "Zones are virtual detection regions; rooms are
  installation metadata only" (the intermediate compromise that kept `RoomId`
  and `FloorId` as backend metadata).
- Related: `docs/spec/data-contracts.md` §3; `docs/spec/capture-and-firmware.md`
  §2; `docs/spec/geometry-calibration.md` §1; `CONTEXT.md` physical-topology
  section.
- Implemented by: `crates/aquila-types`, `crates/aquila-geometry`,
  `crates/aquila-ha`, `crates/aquila-api`, `crates/aquila-addon`,
  `crates/aquila-evaluation`, `crates/aquila-simulator`, `crates/aquila-testkit`,
  `crates/aquila-sensing`, `crates/aquila-inference`
- Reference: none — an internal domain-model decision.

## Context

The previous model kept **Room** and **Floor** as backend metadata even though
zones are the detection scope:

- `RoomId`/`FloorId` survive in `Node.room`, `Zone.room`, topology validation,
  the dashboard structure screen, and `pairing` flows.
- `ZoneId` is an independent alias but every zone still points at a room, and
  every node is still "installed in exactly one room".

That still forces the user to think about rooms and floors — physical
partitions — before they can define where a person can be detected. It also
keeps detection scoped to rooms by construction, which is exactly the
`node → room → 1 person detected by node → 1 person detected in room` chain
the user rejected.

A zone like **"bed"** inside a bedroom, **"shower"** vs **"toilet"** inside a
bathroom, or a zone spanning two rooms cannot be expressed cleanly when rooms
and floors are still first-class concepts the topology is built around.

## Decision

**Zones are the only spatial concept.** Rooms and floors are removed from the
domain model entirely.

1. **Zone** — a named virtual detection region in 3D space (`ZoneId = Slug`).
   Each zone carries a home-frame position and extent:
   - `origin_home_m: [f32; 3]` — the zone's origin in the home frame (metres)
   - `extent_home_m: [f32; 3]` — the zone's half-extents in x/y/z (metres)
   A zone is any region the operator draws: "bed", "shower", "toilet",
   "lounge", "whole home". Zones may overlap; a person is attributed to a zone
   by the fused pipeline, not by which room a node is in.

2. **Node** — a dumb ESP32-S3 sensor at a home-frame position:
   - `pos_home_m: [f32; 3]` — where the node's antenna is mounted
   - `ZoneId` is *not* on the node; nodes are not "in" a zone. Nodes contribute
     CSI to zone detection collaboratively via node-pairs.

3. **Home** — holds zones and nodes directly: **Home → Zones → Nodes →
   NodePairs**. No floors, no rooms.

4. **Detection/Event scope** — `EventScope::Zone(ZoneId)` and
   `DetectionScope::Zone(ZoneId)` are the only spatial scopes besides `Home`.
   HA entities are `aquila_<zone>_presence`, `aquila_<zone>_breathing_rate`,
   etc. — zone-scoped, node-agnostic.

5. **Dashboard** — creates zones directly (draw a region in the home map, give
   it a name). No floor/room wizard. Zones are the detection views.

## Alternatives considered

1. **Keep rooms as "where nodes are mounted".** Rejected: it reintroduces the
   room concept the user rejected, and a node's mount point is just an
   (x,y,z) position — the room label adds no sensing value.

2. **Keep floors for multi-floor homes.** Rejected: floor is just `z` in the
   home frame. A zone on the first floor is a zone with a higher `z` origin;
   cross-floor attribution is a fusion problem solved by all node-pairs, not a
   topology concept.

3. **Keep `Zone.room` as an optional sub-region link.** Rejected: it keeps the
   room concept alive and constrains zones to rooms, preventing zones that span
   rooms or exist between them.

4. **Defer to hardware.** Rejected: the model hardcodes the user out of
   sub-room detection today. Fixing it before hardware means the first capture
   tests the right model.

## Consequences

- Positive: operators define zones such as "bed", "shower", "toilet" —
  arbitrary 3D detection regions independent of any room/floor partition.
- Positive: detection is attributed by the fused pipeline from all node-pairs,
  matching "all nodes collaborate on a global map of the house".
- Positive: the topology JSON shrinks to Home → Zones → Nodes → NodePairs;
  positions are home-frame (x,y,z), not room-local.
- Accepted cost: every test topology, the add-on demo topology, the HA entity
  model, the schema projection, geometry attribution (`zone_of` becomes
  position-based, not room-based), the dashboard structure screen, and
  `CONTEXT.md`/`data-contracts.md` vocabulary all change. Existing
  `topology.json` files are not forward-compatible (rooms/floors removed).
