//! Unit tests for the measurement module.
//!
//! These exercise the public interface only: the record round-trip and identity,
//! the sink adapters over a real temporary file, and the diff verdicts and
//! downgrade rules. Determinism is supplied by a [`FixedClock`] implementing the
//! `aquila-types` `Clock` seam; no wall-clock is read anywhere in this module.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use aquila_types::{Clock, CoreTime};

use crate::*;

/// A clock pinned to one instant, for deterministic timestamp tests.
struct FixedClock(CoreTime);

impl Clock for FixedClock {
    fn now(&self) -> CoreTime {
        self.0
    }
}

/// A sink that always refuses a record, to prove [`TeeSink`] fails closed.
struct FailingSink;

impl MetricSink for FailingSink {
    fn record(&mut self, _measurement: &Measurement) -> Result<(), MetricsError> {
        Err(MetricsError::Invalid("refused".to_string()))
    }
}

/// A metric that returns a fixed measurement.
struct ConstantMetric(Measurement);

impl Metric for ConstantMetric {
    fn id(&self) -> MetricName {
        self.0.name.clone()
    }

    fn compute(&self) -> Measurement {
        self.0.clone()
    }
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_path(tag: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "aquila-metrics-{}-{}-{}.jsonl",
        std::process::id(),
        tag,
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    path
}

fn name(value: &str) -> MetricName {
    MetricName::new(value).expect("valid metric name")
}

fn commit() -> CommitSha {
    CommitSha::new("a".repeat(40)).expect("valid commit")
}

fn measurement(name: &str, grade: Grade, value: MeasurementValue, unit: &str) -> Measurement {
    Measurement {
        name: self::name(name),
        version: 1,
        value,
        unit: Unit::new(unit).expect("valid unit"),
        samples: 1,
        seed: None,
        commit: commit(),
        dirty: false,
        host: HostFingerprint::new("linux-x86_64-test").expect("valid host"),
        tools: ToolVersions::new(),
        authority: Authority::Measured,
        grade,
        timestamp: Timestamp::from_unix_ms(1_700_000_000_000),
    }
}

fn deterministic(name: &str, count: u64) -> Measurement {
    measurement(
        name,
        Grade::Deterministic,
        MeasurementValue::Count(count),
        "count",
    )
}

fn digest(name: &str, value: &str) -> Measurement {
    measurement(
        name,
        Grade::Deterministic,
        MeasurementValue::Digest(value.to_string()),
        "sha256",
    )
}

fn statistical(name: &str, value: f64) -> Measurement {
    measurement(
        name,
        Grade::Statistical,
        MeasurementValue::Scalar(value),
        "ms",
    )
}

fn gated(name: &str, value: f64, unit: &str) -> Measurement {
    measurement(name, Grade::Gated, MeasurementValue::Scalar(value), unit)
}

fn delta_for<'a>(deltas: &'a [Delta], metric: &str) -> &'a Delta {
    deltas
        .iter()
        .find(|delta| delta.name.as_str() == metric)
        .expect("delta present")
}

#[test]
fn record_serialises_and_parses_round_trip() {
    let record = measurement(
        "accuracy.presence_f1",
        Grade::Gated,
        MeasurementValue::Scalar(0.912),
        "ratio",
    );

    let json = record.to_canonical_json().expect("serialise");
    let again = record.to_canonical_json().expect("serialise again");
    assert_eq!(json, again, "canonical serialisation must be byte-stable");
    assert!(!json.contains('\n'), "a ledger line carries no newline");

    let parsed = Measurement::from_canonical_json(&json).expect("parse");
    assert_eq!(parsed, record, "round trip must preserve the record");
    assert_eq!(parsed.id(), record.id());
    assert!(record.validate().is_ok());

    // The tagged value never coerces a digest through f64.
    let digest_record = digest("determinism.pipeline.sha256", "9f2c");
    let parsed_digest =
        Measurement::from_canonical_json(&digest_record.to_canonical_json().unwrap()).unwrap();
    assert_eq!(
        parsed_digest.value,
        MeasurementValue::Digest("9f2c".to_string())
    );
}

#[test]
fn identity_excludes_timestamp_and_recording_is_idempotent() {
    let record = deterministic("structure.crates.count", 13);

    let mut restamped = record.clone();
    restamped.timestamp = Timestamp::from_unix_ms(1_800_000_000_000);
    assert_ne!(restamped, record);
    assert_eq!(
        restamped.id(),
        record.id(),
        "timestamp must not participate in identity"
    );

    let mut changed = record.clone();
    changed.value = MeasurementValue::Count(14);
    assert_ne!(
        changed.id(),
        record.id(),
        "a changed value is a new identity"
    );

    // An in-memory sink collapses a repeat to one row.
    let mut memory = InMemoryMetricSink::new();
    memory.record(&record).expect("record");
    memory.record(&record).expect("record again");
    memory.record(&restamped).expect("re-stamped record");
    assert_eq!(memory.len(), 1, "identical records collapse to one row");
    assert!(memory.contains(&record.id()));

    // The JSONL ledger does the same, and survives a reopen.
    let path = temp_path("idempotent");
    {
        let mut ledger = JsonLedgerSink::open(&path).expect("open ledger");
        ledger.record(&record).expect("record");
        ledger.record(&record).expect("record again");
        ledger.flush().expect("flush");
        assert_eq!(ledger.len(), 1);
    }
    {
        let mut ledger = JsonLedgerSink::open(&path).expect("reopen ledger");
        assert_eq!(ledger.len(), 1, "reopen reads the existing identity");
        ledger.record(&restamped).expect("re-record");
        ledger.flush().expect("flush");
    }
    let contents = std::fs::read_to_string(&path).expect("read ledger");
    assert_eq!(contents.lines().count(), 1, "one canonical row");
    assert!(contents.ends_with('\n'), "JSONL lines are LF-terminated");
    std::fs::remove_file(&path).expect("clean up");
}

#[test]
fn ledger_appends_in_recording_order() {
    let first = deterministic("a.first", 1);
    let second = deterministic("b.second", 2);
    let path = temp_path("ordering");

    {
        let mut ledger = JsonLedgerSink::open(&path).expect("open ledger");
        assert!(ledger.is_empty());
        ledger.record(&first).expect("record first");
        ledger.record(&second).expect("record second");
        ledger.flush().expect("flush");
    }

    let contents = std::fs::read_to_string(&path).expect("read ledger");
    let names: Vec<String> = contents
        .lines()
        .map(|line| {
            Measurement::from_canonical_json(line)
                .expect("parse ledger line")
                .name
                .as_str()
                .to_string()
        })
        .collect();
    assert_eq!(names, vec!["a.first", "b.second"]);

    {
        let mut ledger = JsonLedgerSink::open(&path).expect("reopen ledger");
        ledger.record(&first).expect("duplicate record");
        ledger.flush().expect("flush");
    }
    let contents = std::fs::read_to_string(&path).expect("read ledger");
    assert_eq!(contents.lines().count(), 2, "duplicates are not appended");
    std::fs::remove_file(&path).expect("clean up");
}

#[test]
fn sink_adapters_tee_and_flush_over_a_temp_file() {
    let record = deterministic("structure.docs.ratio", 3);
    let path = temp_path("tee");

    let ledger = JsonLedgerSink::open(&path).expect("open ledger");
    let mut tee = TeeSink::new(vec![Box::new(ledger), Box::new(InMemoryMetricSink::new())]);
    assert_eq!(tee.len(), 2);
    tee.record(&record).expect("tee record");
    tee.flush().expect("tee flush");

    let contents = std::fs::read_to_string(&path).expect("read ledger");
    assert_eq!(contents.lines().count(), 1);
    std::fs::remove_file(&path).expect("clean up");

    // The tee fails closed if any required sink refuses the record.
    let mut failing = TeeSink::new(vec![
        Box::new(InMemoryMetricSink::new()),
        Box::new(FailingSink),
    ]);
    let error = failing.record(&record).expect_err("tee must fail closed");
    assert!(matches!(error, MetricsError::RequiredSink(_)));
    assert!(failing.flush().is_ok(), "the good sink still flushes");
}

#[test]
fn record_metric_writes_once_through_the_metric_seam() {
    let record = deterministic("structure.crates.count", 13);
    let metric = ConstantMetric(record.clone());
    assert_eq!(metric.id(), record.name);

    let mut sink = InMemoryMetricSink::new();
    let recorded = record_metric(&mut sink, &metric).expect("record metric");
    assert_eq!(recorded.id(), record.id());
    assert_eq!(sink.len(), 1);

    // A second computation with identical inputs adds no row.
    record_metric(&mut sink, &metric).expect("record again");
    assert_eq!(sink.len(), 1);
}

#[test]
fn timestamp_comes_from_the_clock_seam() {
    let clock = FixedClock(CoreTime::from_nanos(1_700_000_000_000_000_000));
    assert_eq!(Timestamp::now(&clock).as_unix_ms(), 1_700_000_000_000);
}

#[test]
fn diff_reports_deterministic_and_membership_verdicts() {
    let baseline = vec![
        digest("determinism.pipeline.sha256", "9f2c"),
        deterministic("structure.unsafe.count", 0),
        deterministic("only.in.baseline", 1),
    ];
    let head = vec![
        digest("determinism.pipeline.sha256", "7a01"),
        deterministic("structure.unsafe.count", 0),
        deterministic("only.in.head", 2),
    ];

    let deltas = diff(&baseline, &head);
    assert_eq!(
        delta_for(&deltas, "determinism.pipeline.sha256").verdict,
        Verdict::Changed
    );
    assert_eq!(
        delta_for(&deltas, "structure.unsafe.count").verdict,
        Verdict::Unchanged
    );
    assert_eq!(delta_for(&deltas, "only.in.head").verdict, Verdict::Added);
    assert_eq!(
        delta_for(&deltas, "only.in.baseline").verdict,
        Verdict::Removed
    );

    // Output is ordered canonically by metric name.
    let names: Vec<&str> = deltas.iter().map(|delta| delta.name.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted);
}

#[test]
fn diff_reports_statistical_tolerance_and_direction() {
    let within = diff(
        &[statistical("system.latency.p99.ingest", 41.2)],
        &[statistical("system.latency.p99.ingest", 43.0)],
    );
    assert_eq!(
        delta_for(&within, "system.latency.p99.ingest").verdict,
        Verdict::WithinTolerance
    );

    let regressed = diff(
        &[statistical("system.latency.p99.ingest", 41.2)],
        &[statistical("system.latency.p99.ingest", 50.0)],
    );
    assert_eq!(
        delta_for(&regressed, "system.latency.p99.ingest").verdict,
        Verdict::Regressed
    );

    let improved = diff(
        &[statistical("system.latency.p99.ingest", 50.0)],
        &[statistical("system.latency.p99.ingest", 40.0)],
    );
    assert_eq!(
        delta_for(&improved, "system.latency.p99.ingest").verdict,
        Verdict::Improved
    );

    let unchanged = diff(
        &[statistical("unknown.metric.without.direction", 10.0)],
        &[statistical("unknown.metric.without.direction", 10.0)],
    );
    assert_eq!(
        delta_for(&unchanged, "unknown.metric.without.direction").verdict,
        Verdict::WithinTolerance
    );
}

#[test]
fn diff_reports_gated_verdicts_with_margin() {
    let deltas = diff(
        &[
            gated("accuracy.presence_f1", 0.912, "ratio"),
            gated("accuracy.breathing.mae_brpm", 1.7, "brpm"),
        ],
        &[
            gated("accuracy.presence_f1", 0.898, "ratio"),
            gated("accuracy.breathing.mae_brpm", 1.6, "brpm"),
        ],
    );

    let f1 = delta_for(&deltas, "accuracy.presence_f1");
    assert_eq!(f1.verdict, Verdict::GateFail);
    assert_eq!(f1.bar, Some(0.9));
    assert!((f1.margin.expect("margin") - -0.002).abs() < 1e-9);

    let breathing = delta_for(&deltas, "accuracy.breathing.mae_brpm");
    assert_eq!(breathing.verdict, Verdict::GatePass);
    assert_eq!(breathing.bar, Some(2.0));
}

#[test]
fn diff_applies_the_downgrade_rules() {
    // A dirty head is labelled on the delta.
    let mut dirty_head = deterministic("structure.crates.count", 13);
    dirty_head.dirty = true;
    let dirty = diff(
        &[deterministic("structure.crates.count", 13)],
        &[dirty_head],
    );
    let dirty_delta = delta_for(&dirty, "structure.crates.count");
    assert!(dirty_delta.downgrades.contains(&Downgrade::DirtyHead));
    assert!(dirty_delta.is_downgraded());

    // A host-sensitive metric across differing hosts is informational.
    let baseline = statistical("micro.instr.ingest.parse", 812_000.0);
    let mut head = statistical("micro.instr.ingest.parse", 809_000.0);
    head.host = HostFingerprint::new("linux-x86_64-other").expect("valid host");
    let mismatch = diff(&[baseline], &[head]);
    let mismatch_delta = delta_for(&mismatch, "micro.instr.ingest.parse");
    assert_eq!(mismatch_delta.verdict, Verdict::HostMismatch);
    assert!(mismatch_delta.downgrades.contains(&Downgrade::HostMismatch));

    // A version change is uncomparable, never a regression.
    let mut bumped = deterministic("structure.crates.count", 13);
    bumped.version = 2;
    let versioned = diff(&[deterministic("structure.crates.count", 13)], &[bumped]);
    let versioned_delta = delta_for(&versioned, "structure.crates.count");
    assert_eq!(versioned_delta.verdict, Verdict::Uncomparable);
    assert!(versioned_delta
        .downgrades
        .contains(&Downgrade::VersionChanged));
}

#[test]
fn value_and_unit_validation_rejects_malformed_records() {
    assert!(Unit::new("").is_err());
    assert!(HostFingerprint::new("").is_err());
    assert!(CommitSha::new("short").is_err());
    assert!(CommitSha::new("z".repeat(40)).is_err());
    assert!(CommitSha::new("a".repeat(40)).is_ok());

    let mut non_finite = deterministic("structure.crates.count", 1);
    non_finite.value = MeasurementValue::Scalar(f64::NAN);
    assert!(non_finite.validate().is_err());

    let mut no_samples = deterministic("structure.crates.count", 1);
    no_samples.samples = 0;
    assert!(no_samples.validate().is_err());

    let mut empty_digest = digest("determinism.pipeline.sha256", "");
    empty_digest.samples = 1;
    assert!(empty_digest.validate().is_err());
}
