//! `claims-verify` — validate the honest-claims registry and enforce the
//! documentation bijection (`testing.md` §7, `repository-tooling.md` §10.2).
//!
//! The registry (`claims.toml`) maps each `CLM-<slug>` id to its claim text,
//! metric and status. `docs/README.md` carries the human-readable claims table.
//! This check validates the registry's internal consistency **and** requires
//! that the registry ids and the table's ids are the same set: a registry claim
//! with no table row, or a table row with no registry entry, fails the gate.
//! It never runs tests, and (unlike the intended design) it does not yet parse
//! test/bench `CLM-` declarations.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Deserialize;

/// The default registry path (repository root; `repository-tooling.md` §1).
const DEFAULT_REGISTRY: &str = "claims.toml";

/// The default documentation claims-table path (`testing.md` §7).
const DEFAULT_TABLE: &str = "docs/README.md";

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

/// Extract every `CLM-<slug>` id that appears in the documentation table.
///
/// The scan is deliberately dependency-free: it finds a literal `CLM-` and then
/// consumes the slug alphabet (`[A-Za-z0-9-]`). It runs over the whole document
/// rather than a parsed markdown table, so a claim id quoted anywhere in the
/// table counts as a row.
fn table_claim_ids(text: &str) -> BTreeSet<String> {
    const NEEDLE: &[u8] = b"CLM-";
    let bytes = text.as_bytes();
    let mut ids = BTreeSet::new();
    let mut i = 0;
    while i + NEEDLE.len() <= bytes.len() {
        if &bytes[i..i + NEEDLE.len()] == NEEDLE {
            let start = i;
            let mut j = i + NEEDLE.len();
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-') {
                j += 1;
            }
            ids.insert(text[start..j].to_string());
            i = j;
        } else {
            i += 1;
        }
    }
    ids
}

/// Enforce the bijection between registry ids and table ids.
fn enforce_bijection(
    registry_ids: &BTreeSet<String>,
    table_ids: &BTreeSet<String>,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    for id in registry_ids.difference(table_ids) {
        errors.push(format!(
            "registry claim `{id}` has no row in the claims table"
        ));
    }
    for id in table_ids.difference(registry_ids) {
        errors.push(format!("claims-table row `{id}` has no registry entry"));
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
    let mut table_path = PathBuf::from(DEFAULT_TABLE);
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
            "--readme" | "--table" => {
                index += 1;
                table_path = PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| "--readme requires a path".to_string())?,
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
    let table_text = std::fs::read_to_string(&table_path)
        .map_err(|error| format!("cannot read {}: {error}", table_path.display()))?;

    if let Err(errors) = validate(&registry) {
        return Err(format_errors(errors));
    }

    let registry_ids: BTreeSet<String> = registry
        .claim
        .iter()
        .map(|claim| claim.id.clone())
        .collect();
    let table_ids = table_claim_ids(&table_text);
    if let Err(errors) = enforce_bijection(&registry_ids, &table_ids) {
        return Err(format_errors(errors));
    }

    Ok(format!(
        "claims-verify: OK — {} claims validated; registry and table are a bijection ({}, {})",
        registry.claim.len(),
        path.display(),
        table_path.display()
    ))
}

/// Render a list of errors behind the standard `FAILED` header.
fn format_errors(errors: Vec<String>) -> String {
    let mut out = String::from("claims-verify: FAILED\n");
    for error in errors {
        out.push_str(&format!("  {error}\n"));
    }
    out
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

    #[test]
    fn table_ids_are_extracted_from_markdown() {
        let text = "\
| Claim id | Metric |\n\
| --- | --- |\n\
| `CLM-torso-pck20` | `accuracy.torso_pck_at_20` |\n\
| `CLM-presence-f1` | `accuracy.presence_f1` |\n";
        let ids = table_claim_ids(text);
        assert!(ids.contains("CLM-torso-pck20"));
        assert!(ids.contains("CLM-presence-f1"));
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn bijection_rejects_orphans_on_both_sides() {
        let registry: BTreeSet<String> = ["CLM-a".to_string()].into_iter().collect();
        let table: BTreeSet<String> = ["CLM-b".to_string()].into_iter().collect();
        let errors = enforce_bijection(&registry, &table).expect_err("must fail");
        assert!(errors.iter().any(|e| e.contains("no row")));
        assert!(errors.iter().any(|e| e.contains("no registry entry")));
        assert!(enforce_bijection(&registry, &registry).is_ok());
    }
}
