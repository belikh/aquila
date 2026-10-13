//! The scene script: a typed, deterministic description of a simulated home.
//!
//! `capture-and-firmware.md` §7.2 describes a scene script (TOML in the spec)
//! that declares the topology, node positions, a Baseline, actors with motion
//! profiles, per-node clock skew, per-node noise, loss/jitter/reorder
//! impairments, channel, bandwidth, tick rate and duration. No TOML crate is
//! provisioned for this workspace, so the script is a typed Rust value that also
//! parses from canonical JSON.
//!
//! Everything here is plain data, so a scene is a value a test can construct,
//! compare and serialise.

use aquila_types::{Band, NodeId, RoomId, Slug};
use aquila_wire::MAX_SUBCARRIERS;
use serde::{Deserialize, Serialize};

use crate::error::SimError;

/// The speed of light in metres per second, the constant the path-phase model
/// uses (`capture-and-firmware.md` §7.2).
pub const SPEED_OF_LIGHT_MPS: f64 = 299_792_458.0;

/// A full-turn constant.
const TAU: f64 = core::f64::consts::TAU;

/// A scene script: one deterministic home, its actors and its impairments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneScript {
    /// A human-readable scene name.
    pub name: String,
    /// The home the scene belongs to.
    pub home: Slug,
    /// The configured nodes.
    pub nodes: Vec<NodeSpec>,
    /// The moving actors (people) in the scene.
    pub actors: Vec<Actor>,
    /// The radio band.
    pub band: Band,
    /// The 802.11 channel.
    pub channel: u8,
    /// The channel width in MHz.
    pub bandwidth_mhz: u8,
    /// The number of subcarriers per frame.
    pub subcarriers: usize,
    /// The subcarrier spacing in hertz.
    pub subcarrier_spacing_hz: f64,
    /// The simulation tick rate in hertz.
    pub tick_hz: f64,
    /// The scene duration in seconds.
    pub duration_s: f64,
    /// The per-subcarrier additive noise amplitude.
    pub noise_amplitude: f64,
    /// Network impairments applied at the datagram layer.
    pub impairments: Impairments,
}

/// One configured node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeSpec {
    /// The dense node identity.
    pub id: NodeId,
    /// The room the node is assigned to.
    pub room: RoomId,
    /// The node position in room-local metres.
    pub pos_m: [f64; 3],
    /// The node's clock skew in parts per million.
    pub skew_ppm: f64,
    /// A per-node noise multiplier on top of [`SceneScript::noise_amplitude`].
    pub noise_scale: f64,
}

/// A moving actor (a person) in one room.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Actor {
    /// The room the actor occupies.
    pub room: RoomId,
    /// The actor's base position in room-local metres.
    pub position_m: [f64; 3],
    /// The time the actor appears, in seconds from the scene start.
    pub active_from_s: f64,
    /// The time the actor leaves, in seconds from the scene start.
    pub active_to_s: f64,
    /// The actor's gross motion profile.
    pub motion: Motion,
    /// The breathing displacement, if the actor is breathing.
    pub breathing: Option<Rhythm>,
    /// The heart displacement, if modelled.
    pub heart: Option<Rhythm>,
}

/// A sinusoidal displacement: breathing or heart.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rhythm {
    /// The displacement amplitude in metres.
    pub amplitude_m: f64,
    /// The oscillation rate in hertz.
    pub rate_hz: f64,
    /// The oscillation phase in radians.
    pub phase_rad: f64,
}

impl Rhythm {
    /// The displacement, in metres, at time `t` seconds.
    #[must_use]
    pub fn displacement_m(&self, t: f64) -> f64 {
        self.amplitude_m * (TAU * self.rate_hz * t + self.phase_rad).sin()
    }
}

/// An actor's gross motion profile.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Motion {
    /// The actor does not move.
    Stationary,
    /// The actor walks along `axis`, `amplitude_m` either side of its base.
    Walk {
        /// The unit-ish axis of travel.
        axis: [f64; 3],
        /// The peak displacement along the axis, in metres.
        amplitude_m: f64,
        /// The oscillation period, in seconds.
        period_s: f64,
        /// The oscillation phase, in radians.
        phase_rad: f64,
    },
    /// The actor sways gently along `axis`.
    Sway {
        /// The unit-ish axis of sway.
        axis: [f64; 3],
        /// The peak displacement along the axis, in metres.
        amplitude_m: f64,
        /// The oscillation period, in seconds.
        period_s: f64,
        /// The oscillation phase, in radians.
        phase_rad: f64,
    },
}

impl Motion {
    /// The displacement from the actor's base position at time `t` seconds.
    #[must_use]
    pub fn offset_m(&self, t: f64) -> [f64; 3] {
        let (axis, amplitude_m, period_s, phase_rad) = match self {
            Self::Stationary => return [0.0; 3],
            Self::Walk {
                axis,
                amplitude_m,
                period_s,
                phase_rad,
            }
            | Self::Sway {
                axis,
                amplitude_m,
                period_s,
                phase_rad,
            } => (*axis, *amplitude_m, *period_s, *phase_rad),
        };
        let scale = amplitude_m * (TAU * t / period_s + phase_rad).sin();
        [axis[0] * scale, axis[1] * scale, axis[2] * scale]
    }
}

/// Network impairments applied at the datagram layer (`capture-and-firmware.md`
/// §7.4).
///
/// Reordering is not modelled by the single-frame pull seam; loss is expressed
/// as a sequence gap and jitter as host-arrival spread, both of which the core
/// already detects (`capture-and-firmware.md` §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Impairments {
    /// The probability that a given frame is dropped.
    pub loss_prob: f64,
    /// The host-arrival jitter magnitude, in microseconds.
    pub jitter_us: f64,
    /// The probability that a frame's first subcarrier is nulled.
    pub impaired_prob: f64,
}

impl Default for Impairments {
    fn default() -> Self {
        Self {
            loss_prob: 0.0,
            jitter_us: 0.0,
            impaired_prob: 0.0,
        }
    }
}

impl SceneScript {
    /// A two-node, one-actor demo scene used by tests and documentation.
    #[must_use]
    pub fn demo() -> Self {
        let kitchen: RoomId = Slug("kitchen".into());
        let lounge: RoomId = Slug("lounge".into());
        Self {
            name: "demo".into(),
            home: Slug("demo-home".into()),
            nodes: vec![
                NodeSpec {
                    id: NodeId(1),
                    room: kitchen.clone(),
                    pos_m: [0.0, 0.0, 2.4],
                    skew_ppm: 20.0,
                    noise_scale: 1.0,
                },
                NodeSpec {
                    id: NodeId(2),
                    room: lounge.clone(),
                    pos_m: [3.0, 0.0, 2.4],
                    skew_ppm: -15.0,
                    noise_scale: 1.0,
                },
            ],
            actors: vec![Actor {
                room: kitchen,
                position_m: [1.5, 0.0, 0.0],
                active_from_s: 0.0,
                active_to_s: 2.0,
                motion: Motion::Sway {
                    axis: [0.0, 1.0, 0.0],
                    amplitude_m: 0.1,
                    period_s: 4.0,
                    phase_rad: 0.0,
                },
                breathing: Some(Rhythm {
                    amplitude_m: 0.005,
                    rate_hz: 0.25,
                    phase_rad: 0.0,
                }),
                heart: Some(Rhythm {
                    amplitude_m: 0.0002,
                    rate_hz: 1.2,
                    phase_rad: 0.0,
                }),
            }],
            band: Band::B2G4,
            channel: 6,
            bandwidth_mhz: 20,
            subcarriers: 64,
            subcarrier_spacing_hz: 312_500.0,
            tick_hz: 20.0,
            duration_s: 2.0,
            noise_amplitude: 0.05,
            impairments: Impairments::default(),
        }
    }

    /// Parse a scene script from canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns [`SimError::Config`] if the JSON is invalid.
    pub fn from_json(json: &[u8]) -> Result<Self, SimError> {
        serde_json::from_slice(json).map_err(|e| SimError::Config(e.to_string()))
    }

    /// Validate the scene's invariants before simulation.
    ///
    /// # Errors
    ///
    /// Returns [`SimError::Config`] naming the first broken invariant.
    pub fn validate(&self) -> Result<(), SimError> {
        if self.name.is_empty() {
            return Err(SimError::Config("scene name must be non-empty".into()));
        }
        if self.nodes.is_empty() {
            return Err(SimError::Config("scene requires at least one node".into()));
        }
        if self.subcarriers == 0 || self.subcarriers > MAX_SUBCARRIERS as usize {
            return Err(SimError::Config(format!(
                "subcarrier count {} is out of range 1..={}",
                self.subcarriers, MAX_SUBCARRIERS
            )));
        }
        if !(self.subcarrier_spacing_hz.is_finite() && self.subcarrier_spacing_hz > 0.0) {
            return Err(SimError::Config(
                "subcarrier_spacing_hz must be finite and positive".into(),
            ));
        }
        if !(self.tick_hz.is_finite() && self.tick_hz > 0.0) {
            return Err(SimError::Config(
                "tick_hz must be finite and positive".into(),
            ));
        }
        if !(self.duration_s.is_finite() && self.duration_s > 0.0) {
            return Err(SimError::Config(
                "duration_s must be finite and positive".into(),
            ));
        }
        if !(self.noise_amplitude.is_finite() && self.noise_amplitude >= 0.0) {
            return Err(SimError::Config(
                "noise_amplitude must be finite and non-negative".into(),
            ));
        }
        for (i, node) in self.nodes.iter().enumerate() {
            if node.room.0.is_empty() {
                return Err(SimError::Config(format!("node {i} has an empty room")));
            }
            if !(node.skew_ppm.is_finite()) {
                return Err(SimError::Config(format!("node {i} has a non-finite skew")));
            }
            if !(node.noise_scale.is_finite() && node.noise_scale >= 0.0) {
                return Err(SimError::Config(format!(
                    "node {i} has an invalid noise scale"
                )));
            }
            if self.nodes[i + 1..].iter().any(|other| other.id == node.id) {
                return Err(SimError::Config(format!(
                    "node id {} appears more than once",
                    node.id.0
                )));
            }
        }
        for actor in &self.actors {
            if !(actor.active_from_s.is_finite() && actor.active_to_s.is_finite()) {
                return Err(SimError::Config(
                    "actor active window must be finite".into(),
                ));
            }
        }
        let p = &self.impairments;
        for (name, value) in [
            ("loss_prob", p.loss_prob),
            ("impaired_prob", p.impaired_prob),
        ] {
            if !(0.0..=1.0).contains(&value) {
                return Err(SimError::Config(format!("{name} must be in [0, 1]")));
            }
        }
        if !(p.jitter_us.is_finite() && p.jitter_us >= 0.0) {
            return Err(SimError::Config(
                "jitter_us must be finite and non-negative".into(),
            ));
        }
        Ok(())
    }

    /// The Euclidian distance between two room-local points, in metres.
    #[must_use]
    pub fn distance_m(a: [f64; 3], b: [f64; 3]) -> f64 {
        let dx = a[0] - b[0];
        let dy = a[1] - b[1];
        let dz = a[2] - b[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

impl Actor {
    /// Whether the actor is present at time `t` seconds.
    #[must_use]
    pub fn present(&self, t: f64) -> bool {
        self.active_from_s <= t && t < self.active_to_s
    }

    /// The actor's position, in room-local metres, at time `t` seconds.
    #[must_use]
    pub fn position_at(&self, t: f64) -> [f64; 3] {
        let offset = self.motion.offset_m(t);
        [
            self.position_m[0] + offset[0],
            self.position_m[1] + offset[1],
            self.position_m[2] + offset[2],
        ]
    }

    /// The instantaneous scatterer path length, in metres, from `node_pos`.
    ///
    /// `d_i(t) = d0_i + d_breath_i(t) + d_heart_i(t)`, with `d0_i` the straight
    /// distance to the actor's moving position (`capture-and-firmware.md` §7.2).
    #[must_use]
    pub fn path_length_m(&self, node_pos: [f64; 3], t: f64) -> f64 {
        let mut d = SceneScript::distance_m(node_pos, self.position_at(t));
        if let Some(breathing) = &self.breathing {
            d += breathing.displacement_m(t);
        }
        if let Some(heart) = &self.heart {
            d += heart.displacement_m(t);
        }
        d.max(0.0)
    }
}
