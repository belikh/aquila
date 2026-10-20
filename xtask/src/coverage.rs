//! `coverage-floor` — parse an LCOV report and enforce a line-coverage floor
//! (`repository-tooling.md` §10.2, `testing.md` §6).
//!
//! Coverage is a floor, not a goal. This check reads the LCOV record written by
//! `cargo llvm-cov --lcov`, sums the `LF:` (lines found) and `LH:` (lines hit)
//! records, and fails when the whole-workspace line percentage is below the
//! configured floor. It never runs the coverage tool itself.

use std::path::PathBuf;

/// The default LCOV record path.
const DEFAULT_LCOV: &str = "lcov.info";

/// The per-module floor file, when present (`tools/coverage-floor.txt`).
const FLOOR_FILE: &str = "tools/coverage-floor.txt";

/// Lines found and hit, summed across every LCOV record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Coverage {
    /// Total lines instrumented (`LF`).
    pub found: u64,
    /// Total lines executed (`LH`).
    pub hit: u64,
}

impl Coverage {
    /// The line-coverage percentage, or `0.0` when nothing was instrumented.
    #[must_use]
    pub fn percent(&self) -> f64 {
        if self.found == 0 {
            0.0
        } else {
            (self.hit as f64 / self.found as f64) * 100.0
        }
    }
}

/// Sum the `LF:`/`LH:` records of an LCOV report.
#[must_use]
pub fn parse_lcov(text: &str) -> Coverage {
    let mut coverage = Coverage::default();
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("LF:") {
            coverage.found += value.trim().parse::<u64>().unwrap_or(0);
        } else if let Some(value) = line.strip_prefix("LH:") {
            coverage.hit += value.trim().parse::<u64>().unwrap_or(0);
        }
    }
    coverage
}

/// Run the check. Returns a one-line summary or a floor violation.
pub fn run(args: &[String]) -> Result<String, String> {
    let mut lcov = PathBuf::from(DEFAULT_LCOV);
    let mut floor: Option<f64> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--lcov" => {
                index += 1;
                lcov = PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| "--lcov requires a path".to_string())?,
                );
            }
            "--floor" => {
                index += 1;
                let raw = args
                    .get(index)
                    .ok_or_else(|| "--floor requires a percentage".to_string())?;
                floor = Some(
                    raw.parse::<f64>()
                        .map_err(|_| format!("invalid --floor value `{raw}`"))?,
                );
            }
            other => return Err(format!("coverage-floor: unknown argument `{other}`")),
        }
        index += 1;
    }

    let floor = match floor {
        Some(value) => value,
        None => read_floor().unwrap_or(0.0),
    };

    let text = std::fs::read_to_string(&lcov)
        .map_err(|error| format!("cannot read {}: {error}", lcov.display()))?;
    let coverage = parse_lcov(&text);
    let percent = coverage.percent();

    if percent + f64::EPSILON < floor {
        Err(format!(
            "coverage-floor: FAILED — {percent:.2}% line coverage is below the {floor:.2}% floor \
             ({} / {} lines)",
            coverage.hit, coverage.found
        ))
    } else {
        Ok(format!(
            "coverage-floor: OK — {percent:.2}% line coverage ({} / {} lines), floor {floor:.2}%",
            coverage.hit, coverage.found
        ))
    }
}

/// Read the configured floor from `tools/coverage-floor.txt`, if present.
///
/// The file holds one number; blank lines and `#` comments are ignored.
fn read_floor() -> Option<f64> {
    let root = crate::workspace_root().ok()?;
    let text = std::fs::read_to_string(root.join(FLOOR_FILE)).ok()?;
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))?
        .parse::<f64>()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lcov_records_are_summed() {
        let text = "\
TN:
SF:/src/a.rs
LF:100
LH:80
end_of_record
SF:/src/b.rs
LF:50
LH:45
end_of_record
";
        let coverage = parse_lcov(text);
        assert_eq!(coverage.found, 150);
        assert_eq!(coverage.hit, 125);
        assert!((coverage.percent() - 83.333_333).abs() < 0.01);
    }

    #[test]
    fn an_empty_report_is_zero_not_a_divide_by_zero() {
        assert_eq!(parse_lcov("").percent(), 0.0);
    }
}
