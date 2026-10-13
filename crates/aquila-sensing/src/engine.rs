//! The sensing core: the single stateful pipeline.
//!
//! [`Sensing`] owns the state the pure crates cannot: a common time base and
//! per-node drift, sliding windows and arrival jitter, feature-level fusion
//! across nodes, and the staged room → floor → cross-floor escalation
//! (`architecture.md` §4). Its public interface is deliberately tiny — construct
//! with adapters, [`step`](Sensing::step) one window, [`calibrate`](Sensing::calibrate)
//! from a sample set, [`sink`](Sensing::sink) the output.
//!
//! Presence and count decisions are emitted as `aquila_types::Event`s to the
//! [`Sink`] seam; per-step [`Measurement`] rows are written to the
//! [`MetricSink`](aquila_metrics::MetricSink) seam. The internal modules
//! (`time`, `window`, `fusion`, `escalation`, `calibration`, `features`,
//! `bridge`) are private and are exercised by the crate's own tests through this
//! interface.

use std::collections::BTreeMap;
use std::sync::Arc;

use aquila_geometry::Topology;
use aquila_inference::{PoseModel, VitalsModel};
use aquila_metrics::{
    Authority, CommitSha, Grade, HostFingerprint, Measurement, MeasurementValue, MetricName,
    MetricSink, Timestamp, ToolVersions, Unit,
};
use aquila_signal::{Calibration as SignalCalibration, Config as SignalConfig};
use aquila_sources::{CsiSource, FrameOrEnd};
use aquila_types::{
    Clock, CsiEnvelope, Estimate, Event, EventKind, EventPayload, EventScope, RoomId, SensingScope,
    TimeSpan, Topology as ContractTopology, VitalsReading, VitalsStatus, Witness,
};

use crate::calibration::CalibrationReport;
use crate::config::SensingConfig;
use crate::error::SensingError;
use crate::features::extract_motion;
use crate::fusion::{self, NodeEvidence};
use crate::sink::Sink;
use crate::time::TimeSync;
use crate::window::{WindowAssembler, WindowBatch};

/// What one call to [`Sensing::step`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StepReport {
    /// Frames pulled from the source in this step (0 or 1).
    pub frames_ingested: usize,
    /// Windows closed and processed in this step (0 or 1).
    pub windows_emitted: usize,
    /// Events emitted to the sink in this step.
    pub events_emitted: usize,
    /// Whether the source reached end-of-stream.
    pub ended: bool,
}

/// The per-room state carried between windows for hysteresis.
#[derive(Debug, Clone, Default)]
struct RoomState {
    occupied: bool,
    count: u8,
    presence_probability: f64,
    pending_occupied: bool,
    hold_occupied: u32,
    pending_count: u8,
    hold_count: u32,
}

/// The single stateful core.
pub struct Sensing {
    source: Box<dyn CsiSource>,
    clock: Arc<dyn Clock>,
    output: Box<dyn Sink>,
    metrics: Box<dyn MetricSink>,
    pose: Option<Arc<dyn PoseModel>>,
    vitals: Option<Arc<dyn VitalsModel>>,
    topology: Topology,
    config: SensingConfig,
    assembler: WindowAssembler,
    sync: TimeSync,
    signal_config: SignalConfig,
    signal_cal: SignalCalibration,
    rooms: Vec<RoomId>,
    previous: BTreeMap<RoomId, RoomState>,
    /// Per-room rolling buffers of the envelopes vitals are accumulated over.
    vitals_buffers: BTreeMap<RoomId, Vec<CsiEnvelope>>,
    /// The core time at which each room's vitals buffer first saw a frame, so
    /// readiness is measured from the start of accumulation, not from the
    /// (trimmed) buffer head.
    vitals_first_seen: BTreeMap<RoomId, u64>,
    /// The number of vitals windows estimated so far.
    vitals_windows: usize,
    windows_processed: usize,
    finished: bool,
}

impl Sensing {
    /// Construct the core from its adapters and configuration.
    ///
    /// `topology` is the contract home model (`aquila_types::Topology`); the core
    /// validates and indexes it through
    /// [`aquila_geometry::Topology`] itself, so a caller needs no geometry
    /// dependency. `pose` and `vitals` are optional because slice 1 only decides
    /// presence and count.
    ///
    /// When [`SensingConfig::vitals_enabled`] is set the slice-2 v1 path uses
    /// the deterministic DSP estimator in `aquila-signal` directly, so the
    /// supplied `vitals` model is not consulted yet: the [`VitalsModel`] seam
    /// is retained for learned models that will replace the estimator without
    /// changing this interface.
    ///
    /// # Errors
    ///
    /// Returns [`SensingError::Config`] when the configuration is inconsistent,
    /// or [`SensingError::Geometry`] when the topology fails referential
    /// integrity.
    // The constructor is the composition seam; each argument is a distinct
    // adapter or value, so bundling them would only move the list elsewhere.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source: Box<dyn CsiSource>,
        clock: Arc<dyn Clock>,
        output: Box<dyn Sink>,
        metrics: Box<dyn MetricSink>,
        pose: Option<Arc<dyn PoseModel>>,
        vitals: Option<Arc<dyn VitalsModel>>,
        topology: ContractTopology,
        config: SensingConfig,
    ) -> Result<Self, SensingError> {
        config.validate()?;
        let topology = Topology::load(topology)?;
        let signal_config =
            SignalConfig::default_reference(aquila_signal::SubcarrierLayout::ht20());
        let rooms = topology
            .model()
            .rooms
            .iter()
            .map(|room| room.id.clone())
            .collect();
        let assembler = WindowAssembler::new(&config);
        Ok(Self {
            source,
            clock,
            output,
            metrics,
            pose,
            vitals,
            topology,
            config,
            assembler,
            sync: TimeSync::new(64, 4),
            signal_config,
            signal_cal: SignalCalibration::default(),
            rooms,
            previous: BTreeMap::new(),
            vitals_buffers: BTreeMap::new(),
            vitals_first_seen: BTreeMap::new(),
            vitals_windows: 0,
            windows_processed: 0,
            finished: false,
        })
    }

    /// Pull one frame and, when it closes a window, run fusion and emit.
    ///
    /// # Errors
    ///
    /// Returns a typed [`SensingError`] if the source, the metric sink or the
    /// output sink fails.
    pub fn step(&mut self) -> Result<StepReport, SensingError> {
        if self.finished {
            return Ok(StepReport {
                ended: true,
                ..StepReport::default()
            });
        }

        match self.source.recv()? {
            FrameOrEnd::End => {
                let mut report = StepReport {
                    ended: true,
                    ..StepReport::default()
                };
                if let Some(batch) = self.assembler.finish() {
                    report.windows_emitted = 1;
                    report.events_emitted = self.process_batch(&batch)?;
                }
                self.finished = true;
                Ok(report)
            }
            FrameOrEnd::Frame(envelope) => {
                let node = envelope.frame.node_id;
                let device_ns = envelope.frame.device_time_us.saturating_mul(1_000);
                self.sync.observe(node, device_ns, envelope.host_rx_mono_ns);

                let mut report = StepReport {
                    frames_ingested: 1,
                    ..StepReport::default()
                };
                if let Some(batch) = self.assembler.push(envelope) {
                    report.windows_emitted = 1;
                    report.events_emitted = self.process_batch(&batch)?;
                }
                Ok(report)
            }
        }
    }

    /// Run the global calibration process.
    ///
    /// This is a documented slice-1 stub: it reports honestly that no
    /// calibration was computed and records that fact, and never fabricates a
    /// correction ([`calibration`](crate::calibration) module).
    ///
    /// # Errors
    ///
    /// Returns a typed [`SensingError`] if recording the stub measurement fails.
    pub fn calibrate(&mut self) -> Result<CalibrationReport, SensingError> {
        let report = CalibrationReport::stub(self.windows_processed);
        let stamp = Timestamp::now(self.clock.as_ref());
        self.record(
            "calibration.performed",
            MeasurementValue::Flag(false),
            "flag",
            Grade::Deterministic,
            stamp,
        )?;
        Ok(report)
    }

    /// Flush both output seams.
    ///
    /// # Errors
    ///
    /// Returns a typed [`SensingError`] if either flush fails.
    pub fn sink(&mut self) -> Result<(), SensingError> {
        self.output.flush()?;
        self.metrics.flush()?;
        Ok(())
    }

    /// The configuration in use.
    #[must_use]
    pub const fn config(&self) -> &SensingConfig {
        &self.config
    }

    /// The indexed topology in use.
    #[must_use]
    pub const fn topology(&self) -> &Topology {
        &self.topology
    }

    /// The number of windows processed so far.
    #[must_use]
    pub const fn windows_processed(&self) -> usize {
        self.windows_processed
    }

    /// The number of vitals windows estimated so far.
    #[must_use]
    pub const fn vitals_windows(&self) -> usize {
        self.vitals_windows
    }

    /// The optional pose model.
    #[must_use]
    pub const fn pose_model(&self) -> Option<&Arc<dyn PoseModel>> {
        self.pose.as_ref()
    }

    /// The optional vitals model.
    #[must_use]
    pub const fn vitals_model(&self) -> Option<&Arc<dyn VitalsModel>> {
        self.vitals.as_ref()
    }

    /// Fuse one closed window and emit its decisions and measurements.
    fn process_batch(&mut self, batch: &WindowBatch) -> Result<usize, SensingError> {
        self.windows_processed += 1;
        let max_count = usize::from(self.config.max_count);

        // 1. Per-node evidence, grouped by room.
        let mut by_room: BTreeMap<RoomId, Vec<NodeEvidence>> = BTreeMap::new();
        let mut signal_energy_total = 0.0_f64;
        let mut presence_energy_total = 0.0_f64;
        let mut motion_cv_total = 0.0_f64;
        let mut motion_std_total = 0.0_f64;
        let mut signal_nodes = 0u64;
        let mut nodes_seen = 0usize;
        for sensing_frame in &batch.frames {
            let SensingScope::Node(node) = sensing_frame.scope else {
                continue;
            };
            let Some(room) = self.topology.room_of(node).value.cloned() else {
                continue;
            };
            let feature = extract_motion(
                node,
                &sensing_frame.frames,
                &self.signal_config,
                &self.signal_cal,
            );
            signal_energy_total += feature.signal_motion_energy;
            presence_energy_total += feature.presence_energy;
            motion_cv_total += feature.amplitude_cv;
            motion_std_total += feature.amplitude_std;
            if feature.used_signal {
                signal_nodes += 1;
            }
            nodes_seen += 1;
            if sensing_frame.frames.len() < self.config.min_frames_per_node {
                continue;
            }
            let logit = (feature.amplitude_std - self.config.presence_motion_threshold)
                / self.config.presence_motion_scale;
            let presence_probability = fusion::sigmoid(logit);
            let weight = self.config.node_weight.clamp(0.0, 1.0) * self.sync.weight(node);
            let count_pmf = fusion::node_count_pmf(presence_probability, max_count);
            by_room.entry(room.clone()).or_default().push(NodeEvidence {
                node,
                room,
                presence_logit: logit,
                count_pmf,
                weight,
                frames: sensing_frame.frames.len(),
            });
        }
        let nodes_denominator = nodes_seen.max(1) as f64;

        // 2. Room decisions, in canonical room order, with edge-triggered events.
        let t_ms = (batch.start.0 / 1_000_000) as i64;
        let stamp = Timestamp::now(self.clock.as_ref());
        let mut emitted = 0usize;
        let rooms = self.rooms.clone();

        for room in &rooms {
            let evidence = by_room.remove(room).unwrap_or_default();
            let probability = fusion::pooled_presence_probability(
                self.config.prior_logit,
                &evidence,
                self.config.logit_clamp,
            );
            let pmf = fusion::mix_count_pmf(&evidence, max_count);
            let (map_count, _) = pmf.map();

            let state = self.previous.entry(room.clone()).or_default();
            state.presence_probability = probability;

            let desired_occupied = if state.occupied {
                probability >= self.config.p_lo
            } else {
                probability >= self.config.p_hi
            };
            let occupied_changed = debounce_occupied(state, desired_occupied, self.config.t_hold);
            let desired_count = if state.occupied { map_count } else { 0 };
            let count_changed = debounce_count(state, desired_count, self.config.t_hold);
            // End the borrow of `self.previous` before emitting.
            let occupied = state.occupied;
            let count = state.count;

            if occupied_changed {
                self.emit(
                    room,
                    EventKind::PresenceChanged,
                    EventPayload::Presence {
                        occupied,
                        probability: probability as f32,
                    },
                    t_ms,
                )?;
                emitted += 1;
            }
            if count_changed {
                self.emit(
                    room,
                    EventKind::CountChanged,
                    EventPayload::Count { people: count },
                    t_ms,
                )?;
                emitted += 1;
            }

            self.record(
                &format!("room.presence.{}", room.as_str()),
                MeasurementValue::Scalar(probability),
                "ratio",
                Grade::Statistical,
                stamp,
            )?;
        }

        self.record(
            "window.frames",
            MeasurementValue::Count(batch.frame_count() as u64),
            "count",
            Grade::Deterministic,
            stamp,
        )?;
        self.record(
            "window.index",
            MeasurementValue::Count(self.windows_processed as u64),
            "count",
            Grade::Deterministic,
            stamp,
        )?;
        self.record(
            "window.presence_energy",
            MeasurementValue::Scalar(presence_energy_total / nodes_denominator),
            "ratio",
            Grade::Statistical,
            stamp,
        )?;
        self.record(
            "window.motion_cv",
            MeasurementValue::Scalar(motion_cv_total / nodes_denominator),
            "ratio",
            Grade::Statistical,
            stamp,
        )?;
        self.record(
            "window.motion_std",
            MeasurementValue::Scalar(motion_std_total / nodes_denominator),
            "ratio",
            Grade::Statistical,
            stamp,
        )?;
        self.record(
            "window.signal_motion_energy",
            MeasurementValue::Scalar(signal_energy_total),
            "ratio",
            Grade::Statistical,
            stamp,
        )?;
        self.record(
            "window.signal_nodes",
            MeasurementValue::Count(signal_nodes),
            "count",
            Grade::Deterministic,
            stamp,
        )?;

        if self.config.vitals_enabled {
            emitted += self.process_vitals(batch, stamp)?;
        }

        Ok(emitted)
    }

    /// Accumulate this window's frames and, once a room has a full vitals span,
    /// estimate breathing and heart rate from exactly those frames.
    ///
    /// Vitals ride the same [`WindowBatch`] as presence: the envelopes appended
    /// here are the same [`CsiEnvelope`]s the presence loop consumes, on the
    /// same core clock, so vitals are not a parallel path
    /// (`roadmap.md`, Slice 2). Each room's estimate is computed by
    /// [`aquila_signal::process`] over the accumulated buffer, then the
    /// highest-confidence node estimate per band is reported. A reading with a
    /// non-`Ok` status carries no value; only `Ok` readings are emitted
    /// (`data-contracts.md` §5).
    fn process_vitals(
        &mut self,
        batch: &WindowBatch,
        stamp: Timestamp,
    ) -> Result<usize, SensingError> {
        let window_ns = self.config.vitals_window_ns;
        let cutoff = batch.end.0.saturating_sub(window_ns);

        // Append this window's per-node envelopes, grouped by room, and note
        // which rooms saw frames (so a silent room is not re-estimated).
        let mut touched: std::collections::BTreeSet<RoomId> = std::collections::BTreeSet::new();
        for sensing_frame in &batch.frames {
            let SensingScope::Node(node) = sensing_frame.scope else {
                continue;
            };
            let Some(room) = self.topology.room_of(node).value.cloned() else {
                continue;
            };
            touched.insert(room.clone());
            self.vitals_first_seen.entry(room.clone()).or_insert(
                sensing_frame
                    .frames
                    .first()
                    .map_or(batch.end.0, |env| env.host_rx_mono_ns),
            );
            let buffer = self.vitals_buffers.entry(room).or_default();
            buffer.extend(sensing_frame.frames.iter().cloned());
        }

        let mut emitted = 0usize;
        for room in touched {
            // Trim the buffer to the trailing vitals span.
            if let Some(buffer) = self.vitals_buffers.get_mut(&room) {
                let keep_from = buffer
                    .iter()
                    .position(|env| env.host_rx_mono_ns >= cutoff)
                    .unwrap_or(buffer.len());
                buffer.drain(0..keep_from);
            }
            let Some(first_seen) = self.vitals_first_seen.get(&room).copied() else {
                continue;
            };
            // Ready once a full span has been observed since accumulation
            // began; a trimmed buffer spans slightly less than the window.
            if batch.end.0.saturating_sub(first_seen) < window_ns {
                continue;
            }
            let buffer = match self.vitals_buffers.get(&room) {
                Some(buffer) if buffer.len() >= 2 => buffer.clone(),
                _ => continue,
            };
            let start_ns = buffer.first().map_or(0, |env| env.host_rx_mono_ns);
            let end_ns = buffer.last().map_or(0, |env| env.host_rx_mono_ns);
            if end_ns <= start_ns {
                continue;
            }

            // The same signal pipeline as presence, over the accumulated span.
            let chunk = crate::bridge::to_signal_chunk(&buffer);
            let Ok(features) =
                aquila_signal::process(&chunk, &self.signal_cal, &self.signal_config)
            else {
                continue;
            };

            // Highest-confidence node estimate per band.
            let mut best_breathing: Option<aquila_signal::VitalEstimate> = None;
            let mut best_heart: Option<aquila_signal::VitalEstimate> = None;
            for node_features in &features.nodes {
                let breathing = node_features.vitals.breathing;
                if breathing.is_ok()
                    && best_breathing.is_none_or(|best| breathing.confidence > best.confidence)
                {
                    best_breathing = Some(breathing);
                }
                let heart = node_features.vitals.heart;
                if heart.is_ok() && best_heart.is_none_or(|best| heart.confidence > best.confidence)
                {
                    best_heart = Some(heart);
                }
            }

            let breathing = best_breathing
                .filter(|estimate| (4.0..=60.0).contains(&estimate.rate_per_min))
                .map(|estimate| Estimate {
                    value: estimate.rate_per_min,
                    confidence: estimate.confidence,
                });
            let heart_rate = best_heart
                .filter(|estimate| (30.0..=220.0).contains(&estimate.rate_per_min))
                .map(|estimate| Estimate {
                    value: estimate.rate_per_min,
                    confidence: estimate.confidence,
                });
            let status = if breathing.is_some() || heart_rate.is_some() {
                VitalsStatus::Ok
            } else {
                VitalsStatus::InsufficientSignal
            };

            let reading = VitalsReading {
                subject: None,
                room: room.clone(),
                span: TimeSpan {
                    start_mono_ns: start_ns,
                    end_mono_ns: end_ns,
                    start_unix_ms: (start_ns / 1_000_000) as i64,
                    end_unix_ms: (end_ns / 1_000_000) as i64,
                },
                breathing,
                heart_rate,
                status,
                witness: Witness::Measured,
            };
            reading.validate()?;

            self.vitals_windows += 1;
            self.record(
                "vitals.window.count",
                MeasurementValue::Count(self.vitals_windows as u64),
                "count",
                Grade::Deterministic,
                stamp,
            )?;
            self.record(
                "vitals.window.frames",
                MeasurementValue::Count(buffer.len() as u64),
                "count",
                Grade::Deterministic,
                stamp,
            )?;

            let t_ms = (batch.end.0 / 1_000_000) as i64;
            if let Some(estimate) = &reading.breathing {
                self.emit_vital(&room, "breathing_rate", estimate.value, "brpm", t_ms)?;
                emitted += 1;
                self.record(
                    "vitals.breathing.brpm",
                    MeasurementValue::Scalar(f64::from(estimate.value)),
                    "brpm",
                    Grade::Statistical,
                    stamp,
                )?;
                self.record(
                    "vitals.breathing.confidence",
                    MeasurementValue::Scalar(f64::from(estimate.confidence)),
                    "ratio",
                    Grade::Statistical,
                    stamp,
                )?;
            }
            if let Some(estimate) = &reading.heart_rate {
                self.emit_vital(&room, "heart_rate", estimate.value, "bpm", t_ms)?;
                emitted += 1;
                self.record(
                    "vitals.heart.bpm",
                    MeasurementValue::Scalar(f64::from(estimate.value)),
                    "bpm",
                    Grade::Statistical,
                    stamp,
                )?;
                self.record(
                    "vitals.heart.confidence",
                    MeasurementValue::Scalar(f64::from(estimate.confidence)),
                    "ratio",
                    Grade::Statistical,
                    stamp,
                )?;
            }
        }

        Ok(emitted)
    }

    /// Emit one vitals event carrying a measured rate.
    fn emit_vital(
        &mut self,
        room: &RoomId,
        metric: &str,
        value: f32,
        unit: &str,
        t_ms: i64,
    ) -> Result<(), SensingError> {
        let home = self.topology.home().id.clone();
        let event = Event {
            id: format!(
                "{}:vital:{}:{}:{}",
                home.as_str(),
                room.as_str(),
                metric,
                t_ms
            ),
            home,
            t_unix_ms: t_ms,
            dedup_key: format!(
                "{}:vital:{}:{}",
                self.topology.home().id.as_str(),
                room.as_str(),
                metric
            ),
            scope: EventScope::Room(room.clone()),
            kind: EventKind::VitalAlert,
            payload: EventPayload::Vitals {
                metric: metric.to_string(),
                value,
                unit: unit.to_string(),
            },
            witness: Witness::Measured,
        };
        event.validate()?;
        self.output.emit(&event)?;
        Ok(())
    }

    /// Build and emit one edge-triggered event.
    fn emit(
        &mut self,
        room: &RoomId,
        kind: EventKind,
        payload: EventPayload,
        t_ms: i64,
    ) -> Result<(), SensingError> {
        let home = self.topology.home().id.clone();
        let scope = EventScope::Room(room.clone());
        let kind_label = format!("{kind:?}");
        let event = Event {
            id: format!("{}:{kind_label}:{}:{t_ms}", home.as_str(), room.as_str()),
            home,
            t_unix_ms: t_ms,
            dedup_key: format!(
                "{}:{kind_label}:{}",
                self.topology.home().id.as_str(),
                room.as_str()
            ),
            scope,
            kind,
            payload,
            witness: Witness::Measured,
        };
        event.validate()?;
        self.output.emit(&event)?;
        Ok(())
    }

    /// Build and record one measurement.
    fn record(
        &mut self,
        suffix: &str,
        value: MeasurementValue,
        unit: &str,
        grade: Grade,
        stamp: Timestamp,
    ) -> Result<(), SensingError> {
        let name = MetricName::new(format!("{}.{}", self.config.measurement.prefix, suffix))?;
        let measurement = Measurement {
            name,
            version: 1,
            value,
            unit: Unit::new(unit)?,
            samples: 1,
            seed: None,
            commit: CommitSha::new(self.config.measurement.commit.clone())?,
            dirty: false,
            host: HostFingerprint::new(self.config.measurement.host.clone())?,
            tools: ToolVersions::new(),
            authority: Authority::Replayed,
            grade,
            timestamp: stamp,
        };
        self.metrics.record(&measurement)?;
        Ok(())
    }
}

/// Debounce a boolean transition over `t_hold` consecutive windows.
///
/// Returns `true` when the transition is committed this window.
fn debounce_occupied(state: &mut RoomState, desired: bool, t_hold: u32) -> bool {
    if desired == state.occupied {
        state.pending_occupied = desired;
        state.hold_occupied = 0;
        return false;
    }
    if state.pending_occupied == desired {
        state.hold_occupied = state.hold_occupied.saturating_add(1);
    } else {
        state.pending_occupied = desired;
        state.hold_occupied = 1;
    }
    if state.hold_occupied >= t_hold.max(1) {
        state.occupied = desired;
        state.hold_occupied = 0;
        true
    } else {
        false
    }
}

/// Debounce a count transition over `t_hold` consecutive windows.
fn debounce_count(state: &mut RoomState, desired: u8, t_hold: u32) -> bool {
    if desired == state.count {
        state.pending_count = desired;
        state.hold_count = 0;
        return false;
    }
    if state.pending_count == desired {
        state.hold_count = state.hold_count.saturating_add(1);
    } else {
        state.pending_count = desired;
        state.hold_count = 1;
    }
    if state.hold_count >= t_hold.max(1) {
        state.count = desired;
        state.hold_count = 0;
        true
    } else {
        false
    }
}
