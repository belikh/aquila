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
use aquila_inference::{PoseModel, PoseNodeSeries, PoseRequest, VitalsModel};
use aquila_metrics::{
    Authority, CommitSha, Grade, HostFingerprint, Measurement, MeasurementValue, MetricName,
    MetricSink, Timestamp, ToolVersions, Unit,
};
use aquila_signal::{Calibration as SignalCalibration, Config as SignalConfig};
use aquila_sources::{CsiSource, FrameOrEnd};
use aquila_types::{
    Clock, CsiEnvelope, DetectionScope, Estimate, Event, EventKind, EventPayload, EventScope,
    FeatureVector, NodeId, Pose, SensingScope, SubjectId, TimeSpan, ZoneId,
    Topology as ContractTopology, VitalsReading, VitalsStatus, Witness,
};

use crate::calibration::{
    calibrate as run_calibration, calibrated_logit, CalibrationArtifact, CalibrationExpectation,
    CalibrationInputs, CalibrationReport, CalibrationRequest,
};
use crate::config::SensingConfig;
use crate::error::SensingError;
use crate::escalation::{
    aggregate_home, aggregate_home_vitals, Conflict, HomeSummary, ZoneAggregate, ZoneSummary,
    ZoneVitals,
};
use crate::features::extract_motion;
use crate::fusion::{self, NodeEvidence};
use crate::sink::Sink;
use crate::time::{NodeClockFit, TimeSync};
use crate::window::{WindowAssembler, WindowBatch};

/// One zone's pose input for a window: the node, its planar position, its
/// fusion weight, and this window's envelopes from that node.
type ZonePoseEntry<'a> = (NodeId, [f64; 2], f64, &'a [CsiEnvelope]);

/// What one call to [`Sensing::step`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StepReport {
    /// Frames pulled from the source in this step (0 or 1).
    pub frames_ingested: usize,
    /// Windows closed and processed in this step (0 or 1).
    pub windows_emitted: usize,
    /// Events emitted to the sink in this step, across every scope.
    pub events_emitted: usize,
    /// Whether the whole-home summary was aggregated in this step's window:
    /// `1` when a window was processed, `0` otherwise (Slice 6).
    pub home_summarised: usize,
    /// Whether the source reached end-of-stream.
    pub ended: bool,
}

/// One window's zone aggregates, retained for scoring and inspection.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneWindow {
    /// The window's start on the core clock, in milliseconds.
    pub t_ms: i64,
    /// The zone aggregates for that window, in canonical zone order.
    pub zones: Vec<ZoneAggregate>,
}

/// One window's whole-home summary, retained for scoring and inspection
/// (`roadmap.md`, Slice 6).
#[derive(Debug, Clone, PartialEq)]
pub struct HomeWindow {
    /// The window's start on the core clock, in milliseconds.
    pub t_ms: i64,
    /// The whole-home summary for that window.
    pub home: HomeSummary,
}

/// The per-unit state carried between windows for hysteresis. Reused for both
/// rooms and floors: the debounce logic is identical and unit-agnostic.
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
    zones: Vec<ZoneId>,
    previous: BTreeMap<ZoneId, RoomState>,
    /// The whole-home hysteresis state (Slice 6).
    home_previous: RoomState,
    /// The per-window whole-home summaries, in processing order (Slice 6).
    home_history: Vec<HomeWindow>,
    /// The per-window zone summaries, in processing order.
    zone_history: Vec<ZoneWindow>,
    /// Per-zone rolling buffers of the envelopes vitals are accumulated over.
    vitals_buffers: BTreeMap<ZoneId, Vec<CsiEnvelope>>,
    /// The core time at which each zone's vitals buffer first saw a frame, so
    /// readiness is measured from the start of accumulation, not from the
    /// (trimmed) buffer head.
    vitals_first_seen: BTreeMap<ZoneId, u64>,
    /// The number of vitals windows estimated so far.
    vitals_windows: usize,
    /// The coarse poses emitted so far, in emission order (`roadmap.md`,
    /// Slice 4). Poses are not edge-triggered events: they are per-window
    /// estimates and are retained here for scoring and for the room state.
    recorded_poses: Vec<Pose>,
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
        let zones = topology
            .model()
            .zones
            .iter()
            .map(|zone| zone.id.clone())
            .collect();
        let assembler = WindowAssembler::new(&config);
        // When pose is enabled but no model is supplied, default to the
        // deterministic DSP adapter so the core always reaches pose through the
        // `PoseModel` seam (`sister-issues-core.md` S8), never a direct call.
        let pose = pose.or_else(|| {
            config.pose_enabled.then(|| {
                Arc::new(crate::pose::DspPoseModel::new(
                    config.pose_geometry,
                    config.pose_dither_search,
                    config.pose_min_frames,
                )) as Arc<dyn PoseModel>
            })
        });
        let mut sensing = Self {
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
            zones,
            previous: BTreeMap::new(),
            home_previous: RoomState::default(),
            home_history: Vec::new(),
            zone_history: Vec::new(),
            vitals_buffers: BTreeMap::new(),
            vitals_first_seen: BTreeMap::new(),
            vitals_windows: 0,
            recorded_poses: Vec::new(),
            windows_processed: 0,
            finished: false,
        };
        // A calibration supplied through the config is applied (and its
        // per-node clock correction seeded) before the first frame; a stale or
        // invalid artefact is refused rather than silently reused.
        if let Some(artifact) = sensing.config.calibration.clone() {
            sensing.apply_calibration(&artifact)?;
        }
        Ok(sensing)
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
                    report.home_summarised = 1;
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
                    report.home_summarised = 1;
                }
                Ok(report)
            }
        }
    }

    /// Run the global whole-home calibration wizard over captured evidence.
    ///
    /// Produces a content-addressed [`CalibrationArtifact`]
    /// (`aquila.calibration/v1`), applies it (seeding per-node clock correction
    /// and per-zone thresholds), records the calibration's commit-keyed quality
    /// metrics, and reports the outcome. Empty inputs are refused honestly: no
    /// artefact is fabricated (`geometry-calibration.md` §10).
    ///
    /// # Errors
    ///
    /// Returns [`SensingError::Calibration`] when the artefact cannot be built,
    /// sealed, verified or applied; or [`SensingError::Metrics`] if recording a
    /// quality row fails.
    pub fn calibrate(
        &mut self,
        request: &CalibrationRequest,
        inputs: &CalibrationInputs,
    ) -> Result<CalibrationReport, SensingError> {
        let windows_observed = self.windows_processed;
        let stamp = Timestamp::now(self.clock.as_ref());
        if !inputs.has_baseline() {
            self.record(
                "calibration.performed",
                MeasurementValue::Flag(false),
                "flag",
                Grade::Deterministic,
                stamp,
            )?;
            return Ok(CalibrationReport::uncalibrated(
                windows_observed,
                "no empty-home capture supplied; nothing was calibrated",
            ));
        }

        // The artefact must be keyed to this core's commit so a later
        // application validates against the same facts.
        let mut request = request.clone();
        request.core_commit = self.config.measurement.commit.clone();
        let artifact = run_calibration(&self.topology, &self.config, &request, inputs)?;
        self.apply_calibration(&artifact)?;
        self.config.calibration = Some(artifact.clone());

        let metrics = artifact.quality.metrics.clone();
        let status = artifact.quality.status;
        let valid = artifact.quality.valid;
        let topology_digest = artifact.home.topology_digest.clone();
        let calibration_id = artifact.calibration_id.clone();

        self.record(
            "calibration.digest",
            MeasurementValue::Digest(calibration_id.clone()),
            "sha256",
            Grade::Deterministic,
            stamp,
        )?;
        self.record(
            "calibration.link_coverage",
            MeasurementValue::Scalar(metrics.link_coverage),
            "ratio",
            Grade::Statistical,
            stamp,
        )?;
        self.record(
            "calibration.baseline_snr_db",
            MeasurementValue::Scalar(metrics.baseline_snr_db),
            "db",
            Grade::Statistical,
            stamp,
        )?;
        self.record(
            "calibration.zone_separation",
            MeasurementValue::Scalar(metrics.zone_separation),
            "ratio",
            Grade::Statistical,
            stamp,
        )?;
        self.record(
            "calibration.presence_f1",
            MeasurementValue::Scalar(metrics.presence_f1),
            "score",
            Grade::Statistical,
            stamp,
        )?;
        self.record(
            "calibration.drift.per_node",
            MeasurementValue::Scalar(metrics.drift_residual_ms),
            "ms",
            Grade::Deterministic,
            stamp,
        )?;
        self.record(
            "calibration.performed",
            MeasurementValue::Flag(valid),
            "flag",
            Grade::Deterministic,
            stamp,
        )?;

        Ok(CalibrationReport {
            calibrated: valid,
            quality: Some(status),
            topology_digest,
            windows_observed,
            note: format!("calibration {calibration_id} produced with status {status:?}"),
            artifact: Some(artifact),
        })
    }

    /// Apply a calibration artefact: verify it, bind it to the topology, and
    /// seed the per-node clock correction.
    ///
    /// A stale or invalid artefact is refused with a typed error rather than
    /// silently reused (`geometry-calibration.md` §10).
    ///
    /// # Errors
    ///
    /// Returns [`SensingError::Calibration`] on a digest mismatch, a schema or
    /// semantics mismatch, a topology/commit/firmware change, a failed quality
    /// gate, or a JSON error computing the current topology digest.
    pub fn apply_calibration(
        &mut self,
        artifact: &CalibrationArtifact,
    ) -> Result<(), SensingError> {
        artifact.verify_digest()?;
        let topology_digest = crate::calibration::topology_digest(self.topology.model())?;
        let expect =
            CalibrationExpectation::new(self.config.measurement.commit.clone(), topology_digest);
        artifact.check(&expect)?;
        for node in &artifact.nodes {
            self.sync.seed(NodeClockFit {
                node: NodeId(node.node_id),
                slope: node.clock.slope,
                intercept_ns: node.clock.intercept_ns,
                samples: 0,
                residual_rms_ns: node.clock.fit_residual_ms * 1e6,
            });
        }
        Ok(())
    }

    /// The applied calibration artefact, when one is present.
    #[must_use]
    pub fn calibration(&self) -> Option<&CalibrationArtifact> {
        self.config.calibration.as_ref()
    }

    /// The per-node presence logit: the calibrated zone threshold when a valid
    /// calibration supplies one, the explicitly declared baseline when present,
    /// and the **unknown** logit `0` (odds 1:1, `p = 0.5`) otherwise.
    ///
    /// An uncalibrated core with no declared baseline refuses to decide rather
    /// than guessing with a synthetic-tuned constant (`sister-issues-core.md`
    /// S9): `p = 0.5` sits between `p_lo` and `p_hi`, so no occupancy is
    /// asserted.
    fn presence_logit(&self, zone: &ZoneId, node: NodeId, window_std: f64) -> f64 {
        if let Some(calibration) = self.config.calibration.as_ref() {
            if let (Some(zone_cal), Some(baseline)) = (
                calibration.zone_for_zone(zone),
                calibration.node_baseline(node),
            ) {
                return calibrated_logit(
                    window_std,
                    f64::from(baseline.amplitude_std_mean),
                    f64::from(zone_cal.detection.presence_threshold_db),
                    self.config.calibration_scale_floor_db,
                );
            }
        }
        match self.config.presence_motion_threshold {
            Some(threshold) => (window_std - threshold) / self.config.presence_motion_scale,
            // No baseline: unknown, never a decision.
            None => 0.0,
        }
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

    /// The coarse poses emitted so far, in emission order.
    #[must_use]
    pub fn poses(&self) -> &[Pose] {
        &self.recorded_poses
    }

    /// The whole-home summary for the most recently processed window. `None`
    /// before the first window closes.
    #[must_use]
    pub fn home(&self) -> Option<&HomeSummary> {
        self.home_history.last().map(|window| &window.home)
    }

    /// The per-window whole-home-summary history, in processing order
    /// (Slice 6).
    #[must_use]
    pub fn home_history(&self) -> &[HomeWindow] {
        &self.home_history
    }

    /// The per-window zone history, in processing order.
    #[must_use]
    pub fn zone_history(&self) -> &[ZoneWindow] {
        &self.zone_history
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

        // 1. Per-node evidence, grouped by zone.
        let mut by_zone: BTreeMap<ZoneId, Vec<NodeEvidence>> = BTreeMap::new();
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
            let Some(zone) = self.topology.zone_of(node).value.flatten() else {
                continue;
            };
            let feature = extract_motion(
                node,
                &sensing_frame.frames,
                &self.signal_config,
                &self.signal_cal,
            );
            // `None` is unknown, not zero: only sum and count nodes the signal
            // pipeline actually produced a feature for (`sister-issues-core.md`
            // S16).
            if let Some(energy) = feature.signal_motion_energy {
                signal_energy_total += energy;
                signal_nodes += 1;
            }
            presence_energy_total += feature.presence_energy;
            motion_cv_total += feature.amplitude_cv;
            motion_std_total += feature.amplitude_std;
            nodes_seen += 1;
            if sensing_frame.frames.len() < self.config.min_frames_per_node {
                continue;
            }
            let logit = self.presence_logit(zone, node, feature.amplitude_std);
            let presence_probability = fusion::sigmoid(logit);
            let weight = self.config.node_weight.clamp(0.0, 1.0) * self.sync.weight(node);
            let count_pmf = fusion::node_count_pmf(presence_probability, max_count);
            by_zone.entry(zone.clone()).or_default().push(NodeEvidence {
                node,
                zone: zone.clone(),
                presence_logit: logit,
                count_pmf,
                weight,
                frames: sensing_frame.frames.len(),
            });
        }
        let nodes_denominator = nodes_seen.max(1) as f64;

        // 2. Zone decisions, in canonical zone order, with edge-triggered events.
        let t_ms = (batch.start.0 / 1_000_000) as i64;
        let stamp = Timestamp::now(self.clock.as_ref());
        let mut emitted = 0usize;
        let zones = self.zones.clone();
        let mut zone_summaries: BTreeMap<ZoneId, ZoneSummary> = BTreeMap::new();

        for zone in &zones {
            let evidence = by_zone.remove(zone).unwrap_or_default();
            let covered = !evidence.is_empty();
            let probability = fusion::pooled_presence_probability(
                self.config.prior_logit,
                &evidence,
                self.config.logit_clamp,
            );
            let pmf = fusion::mix_count_pmf(&evidence, max_count);
            let (map_count, _) = pmf.map();

            let state = self.previous.entry(zone.clone()).or_default();
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
                    zone,
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
                    zone,
                    EventKind::CountChanged,
                    EventPayload::Count { people: count },
                    t_ms,
                )?;
                emitted += 1;
            }

            self.record(
                &format!("zone.presence.{}", zone.as_str()),
                MeasurementValue::Scalar(probability),
                "ratio",
                Grade::Statistical,
                stamp,
            )?;

            zone_summaries.insert(
                zone.clone(),
                if covered {
                    ZoneSummary::covered(zone.clone(), probability, pmf)
                } else {
                    ZoneSummary::uncovered(zone.clone(), max_count)
                },
            );
        }

        // 3. Zone escalation: zones compose into the whole home.
        let zone_aggregates: Vec<ZoneAggregate> = zone_summaries
            .values()
            .map(|zs| ZoneAggregate {
                zone: zs.zone.clone(),
                presence_probability: Some(zs.presence_probability),
                count: Some(zs.count),
                count_confidence: zs.count_pmf.map().1,
                count_pmf: Some(zs.count_pmf.clone()),
                nodes: vec![],
                attribution: None,
                conflicts: vec![],
            })
            .collect();
        emitted += self.process_home(&zone_aggregates, t_ms, stamp, max_count)?;

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

        if self.config.pose_enabled {
            emitted += self.process_pose(batch, stamp)?;
        }

        if self.config.vitals_enabled {
            emitted += self.process_vitals(batch, stamp)?;
        }

        Ok(emitted)
    }

    /// Aggregate this window's zone summaries into the whole home, apply home
    /// hysteresis, emit home-scoped events, and record home and whole-home
    /// attribution measurements (`fusion-sync.md` §2.5; `roadmap.md`, Slice 6).
    ///
    /// Home presence soft-ORs the zones, home count convolves the zone count
    /// PMFs, and the whole-home location names the strongest occupied zone,
    /// withheld when two zones are ambiguous. The hierarchy is read from the
    /// topology, so this is data-driven with no per-zone special case.
    /// Cross-zone results are reported and labelled, never gated
    /// (`evaluation.md` §5.3).
    fn process_home(
        &mut self,
        zones: &[ZoneAggregate],
        t_ms: i64,
        stamp: Timestamp,
        max_count: usize,
    ) -> Result<usize, SensingError> {
        let summary = aggregate_home(zones, self.config.home_gap, self.config.p_hi, max_count);
        let mut emitted = 0usize;

        let (occupied_changed, count_changed, occupied, count, probability) = {
            let state = &mut self.home_previous;
            let mut occupied_changed = false;
            if let Some(p) = summary.presence_probability {
                state.presence_probability = p;
                let desired_occupied = if state.occupied {
                    p >= self.config.p_lo
                } else {
                    p >= self.config.p_hi
                };
                occupied_changed = debounce_occupied(state, desired_occupied, self.config.t_hold);
            }
            let mut count_changed = false;
            if let Some(c) = summary.count {
                let desired_count = if state.occupied { c } else { 0 };
                count_changed = debounce_count(state, desired_count, self.config.t_hold);
            }
            (
                occupied_changed,
                count_changed,
                state.occupied,
                state.count,
                summary.presence_probability,
            )
        };

        if occupied_changed {
            if let Some(p) = probability {
                self.emit_home(
                    EventKind::PresenceChanged,
                    EventPayload::Presence {
                        occupied,
                        probability: p as f32,
                    },
                    t_ms,
                )?;
                emitted += 1;
            }
        }
        if count_changed {
            self.emit_home(
                EventKind::CountChanged,
                EventPayload::Count { people: count },
                t_ms,
            )?;
            emitted += 1;
        }

        if let Some(p) = summary.presence_probability {
            self.record(
                "home.presence",
                MeasurementValue::Scalar(p),
                "ratio",
                Grade::Statistical,
                stamp,
            )?;
        }
        if let Some(count) = summary.count {
            self.record(
                "home.count",
                MeasurementValue::Count(u64::from(count)),
                "count",
                Grade::Statistical,
                stamp,
            )?;
            self.record(
                "home.count.confidence",
                MeasurementValue::Scalar(summary.count_confidence),
                "ratio",
                Grade::Statistical,
                stamp,
            )?;
        }
        self.record(
            "home.attribution",
            MeasurementValue::Flag(summary.attribution.is_some()),
            "flag",
            Grade::Deterministic,
            stamp,
        )?;
        for conflict in &summary.conflicts {
            if let Conflict::AmbiguousZonesHome { candidates } = conflict {
                self.record(
                    "home.conflict.ambiguous_zones",
                    MeasurementValue::Count(candidates.len() as u64),
                    "count",
                    Grade::Deterministic,
                    stamp,
                )?;
            }
        }

        self.home_history.push(HomeWindow {
            t_ms,
            home: summary,
        });
        Ok(emitted)
    }

    /// Build and emit one edge-triggered whole-home-scoped event.
    fn emit_home(
        &mut self,
        kind: EventKind,
        payload: EventPayload,
        t_ms: i64,
    ) -> Result<(), SensingError> {
        let home = self.topology.home().id.clone();
        let kind_label = format!("{kind:?}");
        let event = Event {
            id: format!("{}:{kind_label}:home:{t_ms}", home.as_str()),
            home: home.clone(),
            t_unix_ms: t_ms,
            dedup_key: format!("{}:{kind_label}:home", home.as_str()),
            scope: EventScope::Home,
            kind,
            payload,
            witness: Witness::Measured,
        };
        event.validate()?;
        self.output.emit(&event)?;
        Ok(())
    }

    /// Estimate one coarse pose per zone from the window's per-node CSI.
    ///
    /// This is feature-level, within-zone fusion (`fusion-sync.md` §2): each
    /// node contributes scalar torso-scatterer ranges derived from its own CSI,
    /// weighted by its calibration quality and clock health, and
    /// [`crate::pose::estimate_zone_pose`] combines them into a rigid torso. No
    /// cross-node raw phase is combined (§3). A zone with fewer than two nodes
    /// — or one that cannot form a torso — yields no pose, never a fabricated
    /// one.
    fn process_pose(
        &mut self,
        batch: &WindowBatch,
        stamp: Timestamp,
    ) -> Result<usize, SensingError> {
        // Group this window's node envelopes by zone, carrying each node's
        // position and fusion weight.
        let mut by_zone: BTreeMap<ZoneId, Vec<ZonePoseEntry<'_>>> = BTreeMap::new();
        for sensing_frame in &batch.frames {
            let SensingScope::Node(node) = sensing_frame.scope else {
                continue;
            };
            let Some(zone) = self.topology.zone_of(node).value.flatten() else {
                continue;
            };
            let Some(position) = self.topology.position(node).value else {
                continue;
            };
            let quality = self.node_calibration_quality(node);
            let weight = fusion::node_fusion_weight(quality, self.sync.weight(node));
            by_zone.entry(zone.clone()).or_default().push((
                node,
                [f64::from(position[0]), f64::from(position[1])],
                weight,
                &sensing_frame.frames,
            ));
        }

        // Route pose through the `PoseModel` seam: the core never calls the DSP
        // directly (`sister-issues-core.md` S8). Without a model there is no
        // pose.
        let Some(model) = self.pose.clone() else {
            return Ok(0);
        };

        let t_ms = (batch.start.0 / 1_000_000) as i64;
        let mut emitted = 0usize;
        for (zone, entries) in &by_zone {
            let contributor_nodes: Vec<NodeId> = entries.iter().map(|e| e.0).collect();
            let series: Vec<PoseNodeSeries> = entries
                .iter()
                .map(|(node, position_m, weight, envelopes)| PoseNodeSeries {
                    node: *node,
                    position_m: *position_m,
                    weight: *weight,
                    envelopes: envelopes.to_vec(),
                })
                .collect();
            let request = PoseRequest {
                scope: DetectionScope::Zone(zone.clone()),
                contributor_nodes,
                subject: Some(SubjectId::new(0)),
                span: TimeSpan {
                    start_mono_ns: batch.start.0,
                    end_mono_ns: batch.end.0,
                    start_unix_ms: t_ms,
                    end_unix_ms: t_ms,
                },
                features: FeatureVector::new(Vec::new()),
                series,
            };
            if request.validate().is_err() {
                continue;
            }
            let Ok(outcomes) = model.infer(std::slice::from_ref(&request)) else {
                continue;
            };
            let Some(pose) = outcomes.into_iter().next().and_then(|outcome| outcome.pose) else {
                continue;
            };
            self.record(
                "pose.confidence",
                MeasurementValue::Scalar(f64::from(pose.score)),
                "ratio",
                Grade::Statistical,
                stamp,
            )?;
            self.recorded_poses.push(pose);
            emitted += 1;
        }
        if emitted > 0 {
            self.record(
                "pose.windows",
                MeasurementValue::Count(self.recorded_poses.len() as u64),
                "count",
                Grade::Deterministic,
                stamp,
            )?;
        }
        Ok(emitted)
    }

    /// The calibration quality `q_n` of a node (`fusion-sync.md` §2.3).
    ///
    /// When a valid calibration artefact is applied, its **per-node** quality
    /// (`NodeCalibration::quality`) supplies `q_n`: fusion weights each node by
    /// its own calibration quality, not by a whole-home proxy. A node with
    /// quality zero contributes nothing. A node absent from the artefact falls
    /// back to the global link coverage; with no artefact at all, the configured
    /// [`SensingConfig::pose_uncalibrated_quality`] applies, whose `0.0` default
    /// withholds an uncalibrated node from pose.
    fn node_calibration_quality(&self, node: NodeId) -> f64 {
        if let Some(artifact) = self.config.calibration.as_ref() {
            if let Some(calibrated) = artifact.nodes.iter().find(|n| n.node_id == node.0) {
                let q = f64::from(calibrated.quality.get());
                if q.is_finite() {
                    return q.clamp(0.0, 1.0);
                }
            }
            let q = artifact.quality.metrics.link_coverage;
            if q.is_finite() {
                return q.clamp(0.0, 1.0);
            }
        }
        self.config.pose_uncalibrated_quality
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

        // Append this window's per-node envelopes, grouped by zone, and note
        // which zones saw frames (so a silent zone is not re-estimated).
        let mut touched: std::collections::BTreeSet<ZoneId> = std::collections::BTreeSet::new();
        for sensing_frame in &batch.frames {
            let SensingScope::Node(node) = sensing_frame.scope else {
                continue;
            };
            let Some(zone) = self.topology.zone_of(node).value.flatten() else {
                continue;
            };
            touched.insert(zone.clone());
            self.vitals_first_seen.entry(zone.clone()).or_insert(
                sensing_frame
                    .frames
                    .first()
                    .map_or(batch.end.0, |env| env.host_rx_mono_ns),
            );
            let buffer = self.vitals_buffers.entry(zone.clone()).or_default();
            buffer.extend(sensing_frame.frames.iter().cloned());
        }

        let mut emitted = 0usize;
        let mut zone_vitals: BTreeMap<ZoneId, ZoneVitals> = BTreeMap::new();
        for zone in touched {
            // Trim the buffer to the trailing vitals span.
            if let Some(buffer) = self.vitals_buffers.get_mut(&zone) {
                let keep_from = buffer
                    .iter()
                    .position(|env| env.host_rx_mono_ns >= cutoff)
                    .unwrap_or(buffer.len());
                buffer.drain(0..keep_from);
            }
            let Some(first_seen) = self.vitals_first_seen.get(&zone).copied() else {
                continue;
            };
            // Ready once a full span has been observed since accumulation
            // began; a trimmed buffer spans slightly less than the window.
            if batch.end.0.saturating_sub(first_seen) < window_ns {
                continue;
            }
            let buffer = match self.vitals_buffers.get(&zone) {
                Some(buffer) if buffer.len() >= 2 => buffer.clone(),
                _ => continue,
            };
            let start_ns = buffer.first().map_or(0, |env| env.host_rx_mono_ns);
            let end_ns = buffer.last().map_or(0, |env| env.host_rx_mono_ns);
            if end_ns <= start_ns {
                continue;
            }

            // The same signal pipeline as presence, over the accumulated span.
            let Ok(chunk) = crate::bridge::to_signal_chunk(&buffer) else {
                continue;
            };
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
                zone: zone.clone(),
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

            zone_vitals.insert(
                zone.clone(),
                ZoneVitals {
                    zone: zone.clone(),
                    breathing: reading.breathing,
                    heart_rate: reading.heart_rate,
                },
            );

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
                self.emit_zone_vital(&zone, "breathing_rate", estimate.value, "brpm", t_ms)?;
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
                self.emit_zone_vital(&zone, "heart_rate", estimate.value, "bpm", t_ms)?;
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

        let t_ms = (batch.end.0 / 1_000_000) as i64;

        // Aggregate the zone vitals by highest-confidence selection
        // and emit zone-scoped vitals events (`fusion-sync.md` §2.2).
        let aggregated = aggregate_home_vitals(
            &zone_vitals.values().cloned().collect::<Vec<_>>(),
            self.config.vitals_floor_min_confidence,
        );
        if let Some((zone_id, estimate)) = &aggregated.breathing {
            self.emit_zone_vital(zone_id, "breathing_rate", estimate.value, "brpm", t_ms)?;
            emitted += 1;
            self.record(
                "vitals.zone.breathing.brpm",
                MeasurementValue::Scalar(f64::from(estimate.value)),
                "brpm",
                Grade::Statistical,
                stamp,
            )?;
        }
        if let Some((zone_id, estimate)) = &aggregated.heart_rate {
            self.emit_zone_vital(zone_id, "heart_rate", estimate.value, "bpm", t_ms)?;
            emitted += 1;
            self.record(
                "vitals.zone.heart.bpm",
                MeasurementValue::Scalar(f64::from(estimate.value)),
                "bpm",
                Grade::Statistical,
                stamp,
            )?;
        }

        Ok(emitted)
    }

    /// Emit one zone-scoped vitals event carrying a measured rate.
    fn emit_zone_vital(
        &mut self,
        zone: &ZoneId,
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
                zone.as_str(),
                metric,
                t_ms
            ),
            home,
            t_unix_ms: t_ms,
            dedup_key: format!(
                "{}:vital:{}:{}",
                self.topology.home().id.as_str(),
                zone.as_str(),
                metric
            ),
            scope: EventScope::Zone(zone.clone()),
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
        zone: &ZoneId,
        kind: EventKind,
        payload: EventPayload,
        t_ms: i64,
    ) -> Result<(), SensingError> {
        let home = self.topology.home().id.clone();
        let scope = EventScope::Zone(zone.clone());
        let kind_label = format!("{kind:?}");
        let event = Event {
            id: format!("{}:{kind_label}:{}:{t_ms}", home.as_str(), zone.as_str()),
            home,
            t_unix_ms: t_ms,
            dedup_key: format!(
                "{}:{kind_label}:{}",
                self.topology.home().id.as_str(),
                zone.as_str()
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
