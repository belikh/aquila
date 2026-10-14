//! `claims-verify` — validate the honest-claims registry (`testing.md` §7,
//! `repository-tooling.md` §10.2).
//!
//! The registry maps each `CLM-<slug>` id to its claim text, metric, status and
//! (when measured) a commit-keyed ledger entry. This check validates the
//! structure and internal consistency of the registry; the bijection with test
//! declarations is enforced by the harness itself. It never runs tests.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Deserialize;

/// The default registry path (repository root; `repository-tooling.md` §1).
const DEFAULT_REGISTRY: &str = "claims.toml";

/// The permitted claim statuses (`testing.md` §7).
const STATUSES: [&str; 3] = ["measured", "target", "future"];

/// The claims registry document.
#[derive(Debug, Deserialize)]
struct Registry {
    #[serde(default)]
    claim: Vec<Claim>,
}

/// One registered claim.
#[derive(Debug, Deserialize)]
struct Claim {
    /// The `CLM-<slug>` id.
    id: String,
    /// The claim text.
    #[serde(default)]
    text: String,
    /// The metric definition id local to the ledger.
    #[serde(default)]
    metric: String,
    /// `measured`, `target` or `future`.
    status: String,
    /// The commit-keyed ledger entry, required when `status = "measured"`.
    #[serde(default)]
    ledger: Option<String>,
}

/// Validate a parsed registry, returning every problem found.
fn validate(registry: &Registry) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for claim in &registry.claim {
        if !claim.id.starts_with("CLM-") || claim.id.len() <= "CLM-".len() {
            errors.push(format!("claim id `{}` must be `CLM-<slug>`", claim.id));
        }
        if !seen.insert(claim.id.as_str()) {
            errors.push(format!("duplicate claim id `{}`", claim.id));
        }
        if claim.text.trim().is_empty() {
            errors.push(format!("claim `{}` has no text", claim.id));
        }
        if claim.metric.trim().is_empty() {
            errors.push(format!("claim `{}` has no metric", claim.id));
        }
        if !STATUSES.contains(&claim.status.as_str()) {
            errors.push(format!(
                "claim `{}` has unknown status `{}` (expected one of {STATUSES:?})",
                claim.id, claim.status
            ));
        }
        if claim.status == "measured" && claim.ledger.as_deref().unwrap_or("").trim().is_empty() {
            errors.push(format!(
                "measured claim `{}` needs a commit-keyed ledger entry",
                claim.id
            ));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Run the verifier. Returns a one-line summary or the validation errors.
pub fn run(args: &[String]) -> Result<String, String> {
    let mut path = PathBuf::from(DEFAULT_REGISTRY);
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--registry" => {
                index += 1;
                path = PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| "--registry requires a path".to_string())?,
                );
            }
            other => return Err(format!("claims-verify: unknown argument `{other}`")),
        }
        index += 1;
    }

    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let registry: Registry = toml::from_str(&text)
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))?;

    match validate(&registry) {
        Ok(()) => Ok(format!(
            "claims-verify: OK — {} claims validated ({})",
            registry.claim.len(),
            path.display()
        )),
        Err(errors) => {
            let mut out = String::from("claims-verify: FAILED\n");
            for error in errors {
                out.push_str(&format!("  {error}\n"));
            }
            Err(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Registry {
        toml::from_str(text).expect("valid registry")
    }

    #[test]
    fn a_valid_registry_passes() {
        let registry = parse(
            r#"
[[claim]]
id = "CLM-torso-pck20"
text = "torso PCK@20 at or above the v1 bar"
metric = "pose.pck20.torso"
status = "target"

[[claim]]
id = "CLM-presence-f1"
text = "presence F1 at or above the v1 bar"
metric = "presence.f1"
status = "measured"
ledger = "9f2c41e"
"#,
        );
        assert!(validate(&registry).is_ok());
    }

    #[test]
    fn duplicate_and_measured_without_ledger_are_rejected() {
        let registry = parse(
            r#"
[[claim]]
id = "CLM-presence-f1"
text = "presence F1"
metric = "presence.f1"
status = "measured"

[[claim]]
id = "CLM-presence-f1"
text = "duplicate"
metric = "presence.f1"
status = "target"
"#,
        );
        let errors = validate(&registry).expect_err("must fail");
        assert!(errors.iter().any(|e| e.contains("duplicate")));
        assert!(errors.iter().any(|e| e.contains("ledger")));
    }

    #[test]
    fn a_bad_status_is_rejected() {
        let registry = parse(
            r#"
[[claim]]
id = "CLM-x"
text = "x"
metric = "x"
status = "probably"
"#,
        );
        assert!(validate(&registry).is_err());
    }
}
