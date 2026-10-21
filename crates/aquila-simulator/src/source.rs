//! The deterministic CSI simulator: a hardware-free [`CsiSource`].
//!
//! `capture-and-firmware.md` §7 defines the simulator as a pure function of a
//! scene script, a seed, a virtual start time and a tick schedule. It reads no
//! wall clock, spawns no threads and uses only a seeded PRNG, so the same
//! inputs produce byte-identical frames on every run (§7.1).
//!
//! At tick `t`, for each node and subcarrier `k` the model is
//! `H_k(t) = H0_k + Σ_i a_i(t)·exp(-j·2π·f_k·d_i(t)/c) + n_k(t)` (§7.2). The
//! static multipath baseline `H0_k` and the noise are pure, order-independent
//! functions of `(seed, node, tick, subcarrier)`, so synthesis is reproducible
//! regardless of pull order.
//!
//! Each synthesised frame is encoded through [`aquila_wire`] and then decoded
//! again, so the simulator exercises the real wire codec and the pipeline cannot
//! tell simulation from live except by metadata (§3.2).
//!
//! The simulator also emits a synthesised [`GroundTruthRecord`] stream (§7.3):
//! presence, count and the breathing/heart references, per tick and zone.

use std::f64::consts::TAU;

use aquila_session::{Manifest, SessionWriter};
use aquila_sources::{CsiSource, FrameOrEnd, SourceError};
use aquila_types::{
    Band, BuildId, CsiEnvelope, CsiFrame, EventPayload, EventScope, GroundTruthRecord,
    GroundTruthRef, GtKind, Keypoint, KeypointId, MacAddr, PhyFlags, Pose, Session, SessionId,
    SubjectId, Witness, ZoneId, CONTRACT_VERSION, KEYPOINT_COUNT,
};
use aquila_wire::{decode_datagram, encode_datagram};

use crate::error::SimError;
use crate::rng::{signed4, unit4};
use crate::scene::{Body, SceneScript, SPEED_OF_LIGHT_MPS};

/// The scattering strength of each dynamic term (torso/limbs).
const ACTOR_SCATTER: f64 = 0.7;

/// The scattering strength of one torso keypoint scatterer of a body-carrying
/// actor. A body distributes its scattering over the torso keypoints, so each
/// single point is weaker than the point-actor [`ACTOR_SCATTER`].
const ACTOR_TORSO_SCATTER: f64 = 0.5;

/// The modelled esp-csi fixed-point gain: the int8 counts one unit of the
/// synthesised linear field maps to.
///
/// The real receiver selects a gain so a typical frame fills a useful fraction
/// of the int8 range without clipping (ADR-0005). The simulator models that as a
/// **fixed gain sized to the scene's bounded field**: baseline and scatterers
/// sum coherently to at most ~3.5 units, so a gain of 28 maps the busiest frame
/// to ~100 counts and leaves headroom. This replaces the old gain that drove the
/// composite field past 127 and silently saturated (`sister-issues-core.md`
/// S10). The core divides by `aquila_types::IQ_COUNTS_PER_UNIT` regardless, so
/// the modelled gain sets the absolute amplitude scale, never the derived phase.
const SIM_IQ_SCALE: f64 = 28.0;

/// The modelled receiver noise floor, in dBm. The reported RSSI is this floor
/// plus the frame's signal-to-noise ratio, so it is derived from a link budget
/// rather than a tuned clamp of the quantised mean (`sister-issues-core.md`
/// S10).
const SIM_NOISE_FLOOR_DBM: f64 = -95.0;

/// The int8 magnitude at which the quantiser would clip; reaching it is flagged
/// on the frame rather than left silent.
const IQ_CLIP_LIMIT: f64 = 127.0;

/// Quantise one synthesised complex component to the raw int8 the node emits,
/// reporting whether the modelled gain drove it past the int8 range.
fn quantise_i8(value: f64) -> (i8, bool) {
    let scaled = (value * SIM_IQ_SCALE).round();
    let clipped = scaled.abs() > IQ_CLIP_LIMIT;
    (scaled.clamp(-128.0, 127.0) as i8, clipped)
}

// Stream tags keep independent draws independent.
const TAG_LOSS: u64 = 11;
const TAG_IMPAIRED: u64 = 22;
const TAG_JITTER: u64 = 33;
const TAG_BASE_RE: u64 = 44;
const TAG_BASE_IM: u64 = 55;
const TAG_NOISE_RE: u64 = 66;
const TAG_NOISE_IM: u64 = 77;

/// The immutable inputs that complete a scene: the RNG seed, the session
/// identity and the virtual clock origins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimConfig {
    /// The RNG seed. Same seed implies identical bytes (§7.1).
    pub seed: u64,
    /// The session identity recorded in the synthesised manifest.
    pub session: SessionId,
    /// The virtual wall-clock origin, in Unix milliseconds.
    pub start_unix_ms: i64,
    /// The virtual host-clock origin, in microseconds.
    pub start_host_us: u64,
}

impl SimConfig {
    /// Construct a configuration.
    #[must_use]
    pub fn new(seed: u64, session: SessionId, start_unix_ms: i64, start_host_us: u64) -> Self {
        Self {
            seed,
            session,
            start_unix_ms,
            start_host_us,
        }
    }
}

/// A hardware-free, byte-deterministic [`CsiSource`].
///
/// Frames are yielded node-by-node within each tick, in configured node order,
/// with the tick advancing after the last node. Impairments drop frames (a
/// sequence gap) or null the first complex sample and set `first_word_invalid`.
pub struct SimulatorSource {
    script: SceneScript,
    config: SimConfig,
    /// The per-actor pose bodies with the seed-drawn dither frequencies applied
    /// (`sister-issues-core.md` S4). `None` for a non-body actor.
    bodies: Vec<Option<Body>>,
    total_ticks: usize,
    total_frames: usize,
    cursor: usize,
    stopped: bool,
}

impl SimulatorSource {
    /// Build a simulator for `script` and `config`.
    ///
    /// # Errors
    ///
    /// Returns [`SimError::Config`] if the scene fails validation.
    pub fn new(script: SceneScript, config: SimConfig) -> Result<Self, SimError> {
        script.validate()?;
        let total_ticks = total_ticks(&script);
        let total_frames = total_ticks * script.nodes.len();
        // Draw each body's scatterer dither frequencies per seed, so the
        // synthetic pose signature is unknown to the estimator (S4).
        let bodies = script
            .actors
            .iter()
            .enumerate()
            .map(|(index, actor)| {
                actor
                    .body
                    .as_ref()
                    .map(|body| body.randomised_dither(config.seed, index))
            })
            .collect();
        Ok(Self {
            script,
            config,
            bodies,
            total_ticks,
            total_frames,
            cursor: 0,
            stopped: false,
        })
    }

    /// The scene being simulated.
    #[must_use]
    pub const fn script(&self) -> &SceneScript {
        &self.script
    }

    /// The simulation configuration.
    #[must_use]
    pub const fn config(&self) -> &SimConfig {
        &self.config
    }

    /// The number of ticks in the scene.
    #[must_use]
    pub const fn total_ticks(&self) -> usize {
        self.total_ticks
    }

    /// The total number of frames the scene will yield, before loss.
    #[must_use]
    pub const fn frame_count(&self) -> usize {
        self.total_frames
    }

    /// The synthesised ground truth for the whole scene.
    ///
    /// The truth is a pure function of the scene and config, so it is available
    /// without consuming frames.
    #[must_use]
    pub fn ground_truth(&self) -> Vec<(u64, GroundTruthRecord)> {
        synthesise_ground_truth(&self.script, &self.config)
    }

    /// The synthesised coarse-pose ground truth for the whole scene.
    ///
    /// One [`Pose`] per simulation tick per present body-carrying actor, with
    /// the four physical torso keypoints filled and the remaining COCO-17
    /// keypoints honestly absent (`confidence == 0.0`). Research-only, and a
    /// pure function of the scene and config.
    #[must_use]
    pub fn pose_ground_truth(&self) -> Vec<Pose> {
        synthesise_pose_ground_truth(&self.script, &self.config)
    }

    /// Whether a frame at `(node_index, tick)` is dropped by loss.
    fn is_lost(&self, node_index: usize, tick: usize) -> bool {
        if self.script.impairments.loss_prob <= 0.0 {
            return false;
        }
        unit4(
            self.config.seed,
            node_index as u64,
            tick as u64,
            0,
            TAG_LOSS,
        ) < self.script.impairments.loss_prob
    }

    /// Whether a frame at `(node_index, tick)` is impaired.
    fn is_impaired(&self, node_index: usize, tick: usize) -> bool {
        if self.script.impairments.impaired_prob <= 0.0 {
            return false;
        }
        unit4(
            self.config.seed,
            node_index as u64,
            tick as u64,
            0,
            TAG_IMPAIRED,
        ) < self.script.impairments.impaired_prob
    }

    /// The static multipath baseline `H0_k` for one node and subcarrier.
    fn baseline(&self, node_index: usize, k: usize) -> (f64, f64) {
        let re = 0.5
            + 0.5
                * unit4(
                    self.config.seed,
                    node_index as u64,
                    k as u64,
                    0,
                    TAG_BASE_RE,
                );
        let im = signed4(
            self.config.seed,
            node_index as u64,
            k as u64,
            0,
            TAG_BASE_IM,
        );
        (re, im)
    }

    /// Additive noise for one `(node, tick, subcarrier, stream)`.
    fn noise(&self, node_index: usize, tick: usize, k: usize, tag: u64) -> f64 {
        let scale = self.script.noise_amplitude * self.script.nodes[node_index].noise_scale;
        scale
            * signed4(
                self.config.seed,
                node_index as u64,
                tick as u64,
                k as u64,
                tag,
            )
    }

    /// The virtual host arrival for one node's frame at `tick`, including jitter.
    fn host_arrival_us(&self, node_index: usize, tick: usize) -> u64 {
        let base = self.config.start_host_us
            + ((tick as f64) * 1_000_000.0 / self.script.tick_hz).round() as u64;
        let jitter = (self.script.impairments.jitter_us
            * signed4(
                self.config.seed,
                node_index as u64,
                tick as u64,
                0,
                TAG_JITTER,
            ))
        .round();
        if jitter >= 0.0 {
            base.saturating_add(jitter as u64)
        } else {
            base.saturating_sub((-jitter) as u64)
        }
    }

    /// The virtual device monotonic timestamp, in microseconds, with skew.
    fn device_time_us(&self, node_index: usize, tick: usize) -> u64 {
        let t = tick as f64 / self.script.tick_hz;
        let skew = self.script.nodes[node_index].skew_ppm * 1e-6;
        (t * 1e6 * (1.0 + skew)).max(0.0).round() as u64
    }

    /// Synthesise one CSI frame.
    ///
    /// The scatterer phase uses each subcarrier's **absolute RF frequency**
    /// (`f_rf = centre + offset`), not its offset from the band centre: the
    /// spec's `f_k` is "the k-th subcarrier frequency" and the modulation
    /// `Δφ_k = 2π·f_k·Δd/c` is only physical when `f_k` is the RF frequency
    /// (`capture-and-firmware.md` §7.2). Using the offset alone understates the
    /// phase modulation by ~`f_c/Δf` (~244× at 2.4 GHz with 312.5 kHz spacing),
    /// which buries the breathing and heart rhythms below the noise floor and
    /// makes them unrecoverable. This is a correction, not a tuning: the
    /// dynamics are unchanged and determinism (`§7.1`) still holds, because the
    /// centre is a pure function of `(band, channel)`.
    ///
    /// The synthesised complex `H_k` is then quantised to the raw signed int8
    /// `I/Q` the hardware emits, so the frame the core receives is byte-identical
    /// in kind to a live one (ADR-0005).
    fn synth_frame(&self, node_index: usize, tick: usize) -> CsiFrame {
        let node = &self.script.nodes[node_index];
        let t = tick as f64 / self.script.tick_hz;
        let n = self.script.subcarriers;
        let centre_hz = centre_freq_hz(self.script.band, self.script.channel);
        let noise_scale = self.script.noise_amplitude * self.script.nodes[node_index].noise_scale;
        let mut i_q = Vec::with_capacity(2 * n);
        let mut field_power = 0.0_f64;
        let mut saturated = false;

        for k in 0..n {
            let f_rf = centre_hz + (k as f64 - n as f64 / 2.0) * self.script.subcarrier_spacing_hz;
            let (mut re, mut im) = self.baseline(node_index, k);
            for (index, actor) in self.script.actors.iter().enumerate() {
                if actor.zone != node.zone || !actor.present(t) {
                    continue;
                }
                if let Some(body) = self.bodies[index].as_ref() {
                    // A body-carrying actor scatters from its four torso
                    // keypoints, each with its own seed-drawn radial dither, so
                    // the CSI carries a pose-correlated signature.
                    let common = actor
                        .breathing
                        .as_ref()
                        .map_or(0.0, |rhythm| rhythm.displacement_m(t))
                        + actor
                            .heart
                            .as_ref()
                            .map_or(0.0, |rhythm| rhythm.displacement_m(t));
                    let paths =
                        body.scatter_path_lengths_m(actor.position_at(t), node.pos_m, t, common);
                    for d in paths {
                        let theta = -TAU * f_rf * d / SPEED_OF_LIGHT_MPS;
                        re += ACTOR_TORSO_SCATTER * theta.cos();
                        im += ACTOR_TORSO_SCATTER * theta.sin();
                    }
                    continue;
                }
                let d = actor.path_length_m(node.pos_m, t);
                let theta = -TAU * f_rf * d / SPEED_OF_LIGHT_MPS;
                re += ACTOR_SCATTER * theta.cos();
                im += ACTOR_SCATTER * theta.sin();
            }
            re += self.noise(node_index, tick, k, TAG_NOISE_RE);
            im += self.noise(node_index, tick, k, TAG_NOISE_IM);

            // The unquantised field power, for the link-budget RSSI.
            field_power += re * re + im * im;

            // Model the hardware: quantise the complex sample to signed int8,
            // imaginary-first (Q then I), matching the esp-csi wire order.
            let (q, q_clipped) = quantise_i8(im);
            let (i, i_clipped) = quantise_i8(re);
            saturated |= q_clipped || i_clipped;
            i_q.push(q);
            i_q.push(i);
        }

        let first_word_invalid = self.is_impaired(node_index, tick);
        if first_word_invalid {
            // The capture layer's `FIRST_WORD_INVALID`: the leading complex
            // sample (two bytes) is nulled and the frame marks the flag, exactly
            // as the node and the core decoder do (`capture-and-firmware.md`
            // §7.4).
            i_q[0] = 0;
            i_q[1] = 0;
        }

        // The reported RSSI comes from a link budget: the modelled noise floor
        // plus the frame's signal-to-noise ratio, derived from the *unquantised*
        // field power, never a tuned clamp of the quantised mean
        // (`sister-issues-core.md` S10).
        let mean_power = field_power / n as f64;
        let noise_power = 2.0 * noise_scale * noise_scale;
        let snr_linear = if noise_power > 0.0 {
            (mean_power / noise_power).max(1.0)
        } else {
            1.0
        };
        let rssi = (SIM_NOISE_FLOOR_DBM + 10.0 * snr_linear.log10())
            .clamp(-120.0, -10.0)
            .round() as i8;
        let noise_floor = SIM_NOISE_FLOOR_DBM.clamp(-120.0, -10.0).round() as i8;

        let mut phy = PhyFlags::HT | PhyFlags::SHORT_GI;
        if self.script.bandwidth_mhz >= 40 {
            phy |= PhyFlags::SECONDARY_PRESENT;
        }
        if saturated {
            // Never silently clip: mark frames the modelled gain could not fit.
            phy |= PhyFlags::IQ_SATURATED;
        }

        // PROVISIONAL: the contract has not yet pinned the HT20/HT40
        // subcarrier-offset convention, so `-(n/2)` is a placeholder shared with
        // the firmware, not a verified bin map (signal.md §7; S11, F19). The
        // simulator must agree with the node byte-for-byte; when a real capture
        // fixes the map, both change together.
        let subcarrier_offset = -((n / 2) as i16);

        CsiFrame {
            node_id: node.id,
            boot_epoch: 1,
            sequence: tick as u64,
            device_time_us: self.device_time_us(node_index, tick),
            band: self.script.band,
            channel: self.script.channel,
            bandwidth_mhz: self.script.bandwidth_mhz,
            subcarrier_offset,
            i_q,
            rssi_dbm: rssi,
            noise_floor_dbm: noise_floor,
            phy: PhyFlags(phy),
            first_word_invalid,
            source_mac: Some(self.transmitter_mac()),
            // The 802.11 sequence number is twelve bits and counts up from
            // the transmitter, so it wraps.
            rx_seq: Some((tick % 4096) as u16),
            rx_flags: 0,
            rx_timestamp: 0,
        }
    }

    /// The MAC address of the access point the modelled traffic comes from.
    ///
    /// The simulator has one transmitter, so every frame it synthesises names
    /// the same station. Two nodes' captures of the same modelled beacon are
    /// then identifiable by the address alone, exactly as they are on real
    /// hardware.
    fn transmitter_mac(&self) -> MacAddr {
        // A locally administered address (the second-least-significant bit of
        // the first octet is set), derived from the home so the value is stable
        // across runs.
        let mut hash: u32 = 0x811c_9dc5;
        for byte in self.script.home.as_str().as_bytes() {
            hash ^= u32::from(*byte);
            hash = hash.wrapping_mul(0x0100_0193);
        }
        MacAddr([
            0x02,
            ((hash >> 24) & 0xFF) as u8,
            ((hash >> 16) & 0xFF) as u8,
            ((hash >> 8) & 0xFF) as u8,
            (hash & 0xFF) as u8,
            0x01,
        ])
    }
}

impl CsiSource for SimulatorSource {
    fn recv(&mut self) -> Result<FrameOrEnd, SourceError> {
        if self.stopped {
            return Err(SourceError::Stopped);
        }
        loop {
            if self.cursor >= self.total_frames {
                return Ok(FrameOrEnd::End);
            }
            let cursor = self.cursor;
            self.cursor += 1;
            let node_count = self.script.nodes.len();
            let tick = cursor / node_count;
            let node_index = cursor % node_count;

            if self.is_lost(node_index, tick) {
                continue; // sequence gap: the tick's sequence is simply absent
            }

            let frame = self.synth_frame(node_index, tick);
            // Exercise the real wire codec: encode, then decode.
            let bytes = encode_datagram(&frame);
            let frame = decode_datagram(&bytes)?;

            let host_us = self.host_arrival_us(node_index, tick);
            return Ok(FrameOrEnd::Frame(CsiEnvelope {
                frame,
                host_rx_mono_ns: host_us.saturating_mul(1_000),
                host_rx_unix_ms: self.config.start_unix_ms
                    + ((tick as f64) * 1_000.0 / self.script.tick_hz).round() as i64,
                source_addr: None,
            }));
        }
    }

    fn shutdown(&mut self) {
        self.stopped = true;
    }
}

/// The nominal centre frequency of a channel, in hertz.
///
/// This is the RF reference the scatterer phase is measured against, so the
/// synthesised CSI carries the physical `2π·f·Δd/c` sensitivity. It mirrors the
/// transport seam's band/channel mapping.
fn centre_freq_hz(band: Band, channel: u8) -> f64 {
    f64::from(aquila_types::channel_centre_freq_mhz(band, channel)) * 1e6
}

/// The number of ticks a scene covers.
fn total_ticks(script: &SceneScript) -> usize {
    ((script.duration_s * script.tick_hz).ceil() as usize).max(1)
}

/// The zones named by the scene's nodes, first-seen order.
fn zones_of(script: &SceneScript) -> Vec<ZoneId> {
    let mut zones: Vec<ZoneId> = Vec::new();
    for node in &script.nodes {
        if !zones.contains(&node.zone) {
            zones.push(node.zone.clone());
        }
    }
    zones
}

/// Synthesise the per-tick ground truth for a scene (§7.3).
///
/// Emits, for every tick and every zone: a presence record, a count record and
/// one vitals reference per present actor's breathing and heart.
#[must_use]
pub fn synthesise_ground_truth(
    script: &SceneScript,
    config: &SimConfig,
) -> Vec<(u64, GroundTruthRecord)> {
    let ticks = total_ticks(script);
    let zones = zones_of(script);
    let mut out = Vec::new();
    for tick in 0..ticks {
        let t = tick as f64 / script.tick_hz;
        let host_us =
            config.start_host_us + ((tick as f64) * 1_000_000.0 / script.tick_hz).round() as u64;
        let t_unix_ms = config.start_unix_ms + (t * 1_000.0).round() as i64;
        for zone in &zones {
            let present: Vec<&crate::scene::Actor> = script
                .actors
                .iter()
                .filter(|actor| actor.zone == *zone && actor.present(t))
                .collect();
            let count = present.len() as u8;
            let mut push = |payload: EventPayload| {
                out.push((
                    host_us,
                    GroundTruthRecord {
                        session: config.session.clone(),
                        t_unix_ms,
                        kind: GtKind::Synthetic,
                        scope: EventScope::Zone(zone.clone()),
                        payload,
                    },
                ));
            };
            push(EventPayload::Presence {
                occupied: count > 0,
                probability: if count > 0 { 0.95 } else { 0.05 },
            });
            push(EventPayload::Count { people: count });
            for actor in present {
                if let Some(breathing) = &actor.breathing {
                    push(EventPayload::Vitals {
                        metric: "breathing_rate".into(),
                        value: (breathing.rate_hz * 60.0) as f32,
                        unit: "brpm".into(),
                    });
                }
                if let Some(heart) = &actor.heart {
                    push(EventPayload::Vitals {
                        metric: "heart_rate".into(),
                        value: (heart.rate_hz * 60.0) as f32,
                        unit: "bpm".into(),
                    });
                }
            }
        }
    }
    out
}

/// The COCO-17 keypoint order, so a synthesised pose's keypoint array is
/// self-consistent (`data-contracts.md` §4).
const KEYPOINT_ORDER: [KeypointId; KEYPOINT_COUNT] = [
    KeypointId::Nose,
    KeypointId::LeftEye,
    KeypointId::RightEye,
    KeypointId::LeftEar,
    KeypointId::RightEar,
    KeypointId::LeftShoulder,
    KeypointId::RightShoulder,
    KeypointId::LeftElbow,
    KeypointId::RightElbow,
    KeypointId::LeftWrist,
    KeypointId::RightWrist,
    KeypointId::LeftHip,
    KeypointId::RightHip,
    KeypointId::LeftKnee,
    KeypointId::RightKnee,
    KeypointId::LeftAnkle,
    KeypointId::RightAnkle,
];

/// A COCO-17 keypoint array with every keypoint honestly absent.
fn blank_keypoints() -> [Keypoint; KEYPOINT_COUNT] {
    let mut keys = [Keypoint {
        id: KeypointId::Nose,
        x_m: f32::NAN,
        y_m: f32::NAN,
        z_m: None,
        confidence: 0.0,
    }; KEYPOINT_COUNT];
    for (key, id) in keys.iter_mut().zip(KEYPOINT_ORDER) {
        key.id = id;
    }
    keys
}

/// Synthesise the per-tick coarse-pose ground truth for a scene (§7.3).
///
/// One [`Pose`] per present body-carrying actor per tick. The four physical
/// torso scatterers (left/right shoulder, left/right hip) are the drifted
/// nominal keypoints; the neck and pelvis used by torso-PCK are derived by the
/// scorer as the shoulder and hip midpoints, so they need not be stored.
#[must_use]
pub fn synthesise_pose_ground_truth(script: &SceneScript, config: &SimConfig) -> Vec<Pose> {
    let ticks = total_ticks(script);
    let mut out = Vec::new();
    for tick in 0..ticks {
        let t = tick as f64 / script.tick_hz;
        let t_unix_ms = config.start_unix_ms + (t * 1_000.0).round() as i64;
        for (index, actor) in script.actors.iter().enumerate() {
            if !actor.present(t) {
                continue;
            }
            let Some(keypoints) = actor.torso_keypoints_m(t) else {
                continue;
            };
            let mut keys = blank_keypoints();
            for (slot, id) in [
                (0usize, KeypointId::LeftShoulder),
                (1, KeypointId::RightShoulder),
                (2, KeypointId::LeftHip),
                (3, KeypointId::RightHip),
            ] {
                keys[id as usize] = Keypoint {
                    id,
                    x_m: keypoints[slot][0] as f32,
                    y_m: keypoints[slot][1] as f32,
                    z_m: None,
                    confidence: 1.0,
                };
            }
            out.push(Pose {
                subject: SubjectId::new(index as u32),
                zone: actor.zone.clone(),
                t_unix_ms,
                keys,
                score: 1.0,
                witness: Witness::Claimed,
            });
        }
    }
    out
}

/// Build the [`Session`] metadata for a synthesised scene.
fn build_session(script: &SceneScript, config: &SimConfig) -> Session {
    let ended = config.start_unix_ms + (script.duration_s * 1_000.0).round() as i64;
    Session {
        id: config.session.clone(),
        name: format!("{} (simulated)", script.name),
        home: script.home.clone(),
        nodes: script.nodes.iter().map(|node| node.id).collect(),
        started_unix_ms: config.start_unix_ms,
        ended_unix_ms: Some(ended),
        contract: CONTRACT_VERSION,
        build: BuildId {
            git_sha: "simulated".into(),
            dirty: false,
        },
        config_hash: None,
        seed: config.seed,
        artifacts: vec![],
        ground_truth: Some(GroundTruthRef {
            path: "ground_truth.jsonl".into(),
            sha256: None,
            kind: GtKind::Synthetic,
        }),
        notes: "Synthesised by aquila-simulator".into(),
    }
}

/// Synthesise a complete `.aqs` session from a scene script.
///
/// Drains a [`SimulatorSource`] into a [`SessionWriter`], then appends the
/// synthesised ground-truth records. Same scene and seed imply byte-identical
/// bytes, so a caller can assert a golden digest in CI.
///
/// # Errors
///
/// Returns [`SimError::Config`] for an invalid scene, or a session / source
/// error if writing fails.
pub fn synthesise_session(script: &SceneScript, config: &SimConfig) -> Result<Vec<u8>, SimError> {
    script.validate()?;
    let manifest = Manifest::new(build_session(script, config));
    let mut writer = SessionWriter::new(&manifest)?;
    let mut source = SimulatorSource::new(script.clone(), config.clone())?;

    while let FrameOrEnd::Frame(envelope) = source.recv()? {
        let datagram = encode_datagram(&envelope.frame);
        writer.write_frame(envelope.host_rx_mono_ns / 1_000, &datagram)?;
    }

    for (host_us, record) in synthesise_ground_truth(script, config) {
        writer.write_ground_truth(host_us, serde_json::to_string(&record)?)?;
    }

    Ok(writer.finish()?)
}
