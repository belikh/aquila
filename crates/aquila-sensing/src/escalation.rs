//! Room → floor → home escalation.
//!
//! Slice 1 emits room-scope (zone) presence and count: a room *is* a zone in v1
//! (`CONTEXT.md`). The floor and home steps are present and correct but minimal,
//! ready for the multi-room slices: floor occupancy is a soft-OR across the
//! floor's rooms, and home occupancy a soft-OR across floors
//! (`fusion-sync.md` §2.5).
//!
//! Soft-OR is used for occupancy because rooms are independent presence
//! witnesses; count is deliberately *not* summed here in slice 1, where a
//! single-room summary is all that is emitted.

/// The per-room summary the escalation step consumes.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomSummary {
    /// The room.
    pub room: aquila_types::RoomId,
    /// The pooled presence probability.
    pub presence_probability: f64,
    /// The MAP people count.
    pub count: u8,
}

/// Floor occupancy: `1 − Π(1 − p_room)`.
#[must_use]
pub fn floor_presence_probability(rooms: &[RoomSummary]) -> f64 {
    let mut complement = 1.0_f64;
    for room in rooms {
        complement *= 1.0 - room.presence_probability.clamp(0.0, 1.0);
    }
    (1.0 - complement).clamp(0.0, 1.0)
}

/// Home occupancy: a soft-OR over floors, each already a soft-OR over rooms.
#[must_use]
pub fn home_presence_probability(floors: &[f64]) -> f64 {
    let mut complement = 1.0_f64;
    for p in floors {
        complement *= 1.0 - p.clamp(0.0, 1.0);
    }
    (1.0 - complement).clamp(0.0, 1.0)
}
