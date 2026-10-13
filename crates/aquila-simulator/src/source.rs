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
    GroundTruthRef, GtKind, PhyFlags, RoomId, Session, SessionId, CONTRACT_VERSION,
};
use aquila_wire::{decode_datagram, encode_datagram};

use crate::error::SimError;
use crate::rng::{signed4, unit4};
use crate::scene::{SceneScript, SPEED_OF_LIGHT_MPS};

/// The scattering strength of each dynamic term (torso/limbs).
const ACTOR_SCATTER: f64 = 0.7;

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
/// sequence gap) or null the first subcarrier.
pub struct SimulatorSource {
    script: SceneScript,
    config: SimConfig,
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
        Ok(Self {
            script,
            config,
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
    fn synth_frame(&self, node_index: usize, tick: usize) -> CsiFrame {
        let node = &self.script.nodes[node_index];
        let t = tick as f64 / self.script.tick_hz;
        let n = self.script.subcarriers;
        let centre_hz = centre_freq_hz(self.script.band, self.script.channel);
        let mut amplitude = Vec::with_capacity(n);
        let mut phase = Vec::with_capacity(n);

        for k in 0..n {
            let f_rf = centre_hz
                + (k as f64 - n as f64 / 2.0) * self.script.subcarrier_spacing_hz;
            let (mut re, mut im) = self.baseline(node_index, k);
            for actor in &self.script.actors {
                if actor.room != node.room || !actor.present(t) {
                    continue;
                }
                let d = actor.path_length_m(node.pos_m, t);
                let theta = -TAU * f_rf * d / SPEED_OF_LIGHT_MPS;
                re += ACTOR_SCATTER * theta.cos();
                im += ACTOR_SCATTER * theta.sin();
            }
            re += self.noise(node_index, tick, k, TAG_NOISE_RE);
            im += self.noise(node_index, tick, k, TAG_NOISE_IM);

            amplitude.push((re * re + im * im).sqrt() as f32);
            phase.push(im.atan2(re) as f32);
        }

        if self.is_impaired(node_index, tick) {
            // A nulled first subcarrier stands in for the capture layer's
            // `FIRST_WORD_INVALID`; the frozen `CsiFrame` has no such flag
            // (ADR-0004 chose the f32 layout), so the impairment is expressed
            // as an unusable subcarrier rather than an invented field.
            amplitude[0] = 0.0;
        }

        let mean = amplitude.iter().map(|a| f64::from(*a)).sum::<f64>() / n as f64;
        let rssi = (-90.0 + 15.0 * mean).clamp(-120.0, -10.0).round() as i8;
        let noise_floor = (f64::from(rssi) - 30.0).clamp(-120.0, -10.0).round() as i8;

        let mut phy = PhyFlags::HT | PhyFlags::SHORT_GI;
        if self.script.bandwidth_mhz >= 40 {
            phy |= PhyFlags::SECONDARY_PRESENT;
        }

        CsiFrame {
            node_id: node.id,
            boot_epoch: 1,
            sequence: tick as u64,
            device_time_us: self.device_time_us(node_index, tick),
            band: self.script.band,
            channel: self.script.channel,
            bandwidth_mhz: self.script.bandwidth_mhz,
            subcarrier_offset: -((n / 2) as i16),
            amplitude,
            phase,
            rssi_dbm: rssi,
            noise_floor_dbm: noise_floor,
            phy: PhyFlags(phy),
        }
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
    let mhz = match band {
        Band::B2G4 => 2_407.0 + f64::from(channel) * 5.0,
        Band::B5G => 5_000.0 + f64::from(channel) * 5.0,
        Band::Unknown => 0.0,
    };
    mhz * 1e6
}

/// The number of ticks a scene covers.
fn total_ticks(script: &SceneScript) -> usize {
    ((script.duration_s * script.tick_hz).ceil() as usize).max(1)
}

/// The rooms named by the scene's nodes, first-seen order.
fn rooms_of(script: &SceneScript) -> Vec<RoomId> {
    let mut rooms: Vec<RoomId> = Vec::new();
    for node in &script.nodes {
        if !rooms.contains(&node.room) {
            rooms.push(node.room.clone());
        }
    }
    rooms
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
    let rooms = rooms_of(script);
    let mut out = Vec::new();
    for tick in 0..ticks {
        let t = tick as f64 / script.tick_hz;
        let host_us = config.start_host_us
            + ((tick as f64) * 1_000_000.0 / script.tick_hz).round() as u64;
        let t_unix_ms = config.start_unix_ms + (t * 1_000.0).round() as i64;
        for room in &rooms {
            let present: Vec<&crate::scene::Actor> = script
                .actors
                .iter()
                .filter(|actor| actor.room == *room && actor.present(t))
                .collect();
            let count = present.len() as u8;
            let mut push = |payload: EventPayload| {
                out.push((
                    host_us,
                    GroundTruthRecord {
                        session: config.session.clone(),
                        t_unix_ms,
                        kind: GtKind::Synthetic,
                        scope: EventScope::Room(room.clone()),
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
        config_hash: [0u8; 32],
        seed: config.seed,
        artifacts: vec![],
        ground_truth: Some(GroundTruthRef {
            path: "ground_truth.jsonl".into(),
            sha256: [0u8; 32],
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
