//! The dependency-DAG guardrail (`repository-tooling.md` §5).
//!
//! `dag-check` loads `tools/dag.toml` (the allow-list) and the workspace graph
//! from `cargo metadata --format-version 1 --no-deps`, then evaluates the six
//! rules of `repository-tooling.md` §5.1. The graph logic is a pure function so
//! it can be unit-tested without invoking Cargo.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;
use std::process::Command;

use serde::Deserialize;

/// One dependency edge's kind, as `cargo metadata` reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepKind {
    /// A normal `[dependencies]` edge.
    Normal,
    /// A `[dev-dependencies]` edge.
    Dev,
    /// A `[build-dependencies]` edge.
    Build,
}

/// One workspace member and its internal `aquila-*` (and tooling) edges.
#[derive(Debug, Clone)]
pub struct Member {
    /// The package name.
    pub name: String,
    /// The internal dependency edges, in the order Cargo reports them.
    pub deps: Vec<(String, DepKind)>,
}

/// The parsed `tools/dag.toml` allow-list.
#[derive(Debug, Deserialize)]
pub struct Dag {
    /// The allow-list revision.
    #[serde(default)]
    pub rev: u32,
    /// Registered members and their topological level.
    #[serde(default)]
    pub levels: BTreeMap<String, u32>,
    /// Exempt crates (composition root, tooling) and the reason.
    #[serde(default)]
    pub exempt: BTreeMap<String, String>,
    /// The allowed normal/build edges, keyed by depending crate.
    #[serde(default)]
    pub edges: BTreeMap<String, Vec<String>>,
}

impl Dag {
    /// Whether `name` is registered (in `[levels]` or `[exempt]`).
    #[must_use]
    pub fn registered(&self, name: &str) -> bool {
        self.levels.contains_key(name) || self.exempt.contains_key(name)
    }

    /// Whether `name` is exempt from the level and allow-list rules.
    #[must_use]
    pub fn is_exempt(&self, name: &str) -> bool {
        self.exempt.contains_key(name)
    }
}

/// A single guardrail violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// The depending crate.
    pub from: String,
    /// The dependency.
    pub to: String,
    /// A human-readable explanation.
    pub detail: String,
}

impl std::fmt::Display for Violation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DAG violation: {} -> {}\n  {}", self.from, self.to, self.detail)
    }
}

/// The summary returned by a successful check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The number of registered workspace members.
    pub members: usize,
    /// The number of internal dependency edges checked.
    pub edges: usize,
}

/// Evaluate the DAG rules over a set of members.
///
/// # Errors
///
/// Returns every violation found, in member order.
pub fn evaluate(dag: &Dag, members: &[Member]) -> Result<Report, Vec<Violation>> {
    let names: BTreeSet<&str> = members.iter().map(|m| m.name.as_str()).collect();
    let mut violations = Vec::new();
    let mut edge_count = 0usize;

    for member in members {
        if !dag.registered(&member.name) {
            violations.push(Violation {
                from: member.name.clone(),
                to: String::new(),
                detail: format!(
                    "`{}` is a workspace member but is not registered in tools/dag.toml \
                     ([levels] or [exempt]); owner: docs/adr/0003-interface-reconciliation.md",
                    member.name
                ),
            });
        }
        for (dep, kind) in &member.deps {
            if !names.contains(dep.as_str()) {
                continue;
            }
            edge_count += 1;
            let kind_str = match kind {
                DepKind::Normal => "normal",
                DepKind::Dev => "dev",
                DepKind::Build => "build",
            };

            if *kind != DepKind::Dev && dep == "aquila-testkit" {
                violations.push(Violation {
                    from: member.name.clone(),
                    to: dep.clone(),
                    detail: "aquila-testkit may appear only under [dev-dependencies] \
                             (repository-tooling.md §5.1 rule 5)"
                        .to_string(),
                });
                continue;
            }
            if *kind != DepKind::Dev && (dep == "xtask" || dep == "aquila-addon") {
                violations.push(Violation {
                    from: member.name.clone(),
                    to: dep.clone(),
                    detail: format!(
                        "{kind_str} dependency on `{dep}` is forbidden; it is never a product \
                         dependency (repository-tooling.md §5.1 rule 6)"
                    ),
                });
                continue;
            }

            if dag.is_exempt(&member.name) {
                continue;
            }

            let allowed = dag
                .edges
                .get(&member.name)
                .map(|list| list.iter().any(|d| d == dep))
                .unwrap_or(false);
            if *kind != DepKind::Dev && !allowed {
                violations.push(Violation {
                    from: member.name.clone(),
                    to: dep.clone(),
                    detail: format!(
                        "`{dep}` is not in tools/dag.toml [edges] for `{}`; a new edge requires \
                         an ADR (docs/adr/0003-interface-reconciliation.md)",
                        member.name
                    ),
                });
            }

            if let (Some(&from_level), Some(&to_level)) =
                (dag.levels.get(&member.name), dag.levels.get(dep))
            {
                if to_level >= from_level {
                    violations.push(Violation {
                        from: member.name.clone(),
                        to: dep.clone(),
                        detail: format!(
                            "levels: {dep}={to_level}, {}={from_level} (must be strictly lower; \
                             {kind_str} edge)",
                            member.name
                        ),
                    });
                }
            }
        }
    }

    if let Some(cycle) = find_cycle(dag, members) {
        violations.push(Violation {
            from: cycle.clone(),
            to: cycle,
            detail: "cycle in the workspace dependency graph (always fatal)".to_string(),
        });
    }

    if violations.is_empty() {
        Ok(Report {
            members: members.len(),
            edges: edge_count,
        })
    } else {
        Err(violations)
    }
}

/// Detect a cycle in the internal dependency graph (normal, build and dev
/// edges all count).
fn find_cycle(_dag: &Dag, members: &[Member]) -> Option<String> {
    let names: BTreeSet<&str> = members.iter().map(|m| m.name.as_str()).collect();
    let adjacency: BTreeMap<&str, Vec<&str>> = members
        .iter()
        .map(|m| {
            let deps: Vec<&str> = m
                .deps
                .iter()
                .map(|(d, _)| d.as_str())
                .filter(|d| names.contains(d))
                .collect();
            (m.name.as_str(), deps)
        })
        .collect();

    let mut state: BTreeMap<&str, u8> = BTreeMap::new();
    let mut stack: Vec<&str> = Vec::new();
    for name in &names {
        if dfs_cycle(name, &adjacency, &mut state, &mut stack) {
            return stack.first().map(|s| (*s).to_string());
        }
    }
    None
}

fn dfs_cycle<'a>(
    node: &'a str,
    adjacency: &BTreeMap<&'a str, Vec<&'a str>>,
    state: &mut BTreeMap<&'a str, u8>,
    stack: &mut Vec<&'a str>,
) -> bool {
    match state.get(node).copied().unwrap_or(0) {
        1 => return true, // back edge: cycle
        2 => return false,
        _ => {}
    }
    state.insert(node, 1);
    stack.push(node);
    if let Some(deps) = adjacency.get(node) {
        for dep in deps {
            if dfs_cycle(dep, adjacency, state, stack) {
                return true;
            }
        }
    }
    stack.pop();
    state.insert(node, 2);
    false
}

/// Run the guardrail end to end: load `tools/dag.toml`, read `cargo metadata`,
/// evaluate. Returns a one-line success summary or the violations rendered.
pub fn run() -> Result<String, String> {
    let root = workspace_root()?;
    let dag_path = root.join("tools/dag.toml");
    let dag_text = std::fs::read_to_string(&dag_path)
        .map_err(|e| format!("cannot read {}: {e}", dag_path.display()))?;
    let dag: Dag = toml::from_str(&dag_text)
        .map_err(|e| format!("cannot parse {}: {e}", dag_path.display()))?;

    let members = read_members(&root)?;
    match evaluate(&dag, &members) {
        Ok(report) => Ok(format!(
            "dag-check: OK — {} members, {} internal edges, rev {}",
            report.members, report.edges, dag.rev
        )),
        Err(violations) => {
            let mut out = String::from("dag-check: FAILED\n");
            for violation in violations {
                out.push_str(&format!("{violation}\n"));
            }
            Err(out)
        }
    }
}

/// The workspace root: the parent of this tooling crate.
fn workspace_root() -> Result<std::path::PathBuf, String> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "xtask has no parent directory".to_string())
}

/// Read the workspace members and their internal dependency edges from Cargo.
fn read_members(root: &Path) -> Result<Vec<Member>, String> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("cannot run `cargo metadata`: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "`cargo metadata` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("cannot parse cargo metadata JSON: {e}"))?;

    // Package ids are opaque (`path+file:///...#0.0.0`) in current Cargo, so
    // match members by id rather than by parsing the id string.
    let member_ids: HashSet<String> = value["workspace_members"]
        .as_array()
        .ok_or_else(|| "cargo metadata has no workspace_members".to_string())?
        .iter()
        .filter_map(|id| id.as_str())
        .map(str::to_string)
        .collect();

    let packages = value["packages"]
        .as_array()
        .ok_or_else(|| "cargo metadata has no packages".to_string())?;

    let member_packages: Vec<&serde_json::Value> = packages
        .iter()
        .filter(|package| {
            package["id"]
                .as_str()
                .is_some_and(|id| member_ids.contains(id))
        })
        .collect();
    let member_names: HashSet<String> = member_packages
        .iter()
        .filter_map(|package| package["name"].as_str().map(str::to_string))
        .collect();

    let mut members = Vec::new();
    for package in member_packages {
        let Some(name) = package["name"].as_str() else {
            continue;
        };
        let mut deps = Vec::new();
        if let Some(deps_array) = package["dependencies"].as_array() {
            for dep in deps_array {
                let Some(dep_name) = dep["name"].as_str() else {
                    continue;
                };
                if !member_names.contains(dep_name) {
                    continue;
                }
                let kind = match dep["kind"].as_str() {
                    Some("dev") => DepKind::Dev,
                    Some("build") => DepKind::Build,
                    _ => DepKind::Normal,
                };
                deps.push((dep_name.to_string(), kind));
            }
        }
        members.push(Member {
            name: name.to_string(),
            deps,
        });
    }
    members.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(members)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dag() -> Dag {
        let text = r#"
rev = 1
[levels]
"aquila-types" = 0
"aquila-sensing" = 5
"aquila-api" = 7
[exempt]
"aquila-addon" = "composition root"
[edges]
"aquila-types" = []
"aquila-sensing" = ["aquila-types"]
"aquila-api" = ["aquila-types", "aquila-sensing"]
"#;
        toml::from_str(text).expect("valid test dag")
    }

    fn member(name: &str, deps: &[(&str, DepKind)]) -> Member {
        Member {
            name: name.to_string(),
            deps: deps.iter().map(|(d, k)| ((*d).to_string(), *k)).collect(),
        }
    }

    #[test]
    fn a_legal_graph_passes() {
        let members = vec![
            member("aquila-types", &[]),
            member("aquila-sensing", &[("aquila-types", DepKind::Normal)]),
            member(
                "aquila-api",
                &[
                    ("aquila-types", DepKind::Normal),
                    ("aquila-sensing", DepKind::Normal),
                ],
            ),
            member("aquila-addon", &[("aquila-api", DepKind::Normal)]),
        ];
        let report = evaluate(&dag(), &members).expect("legal graph");
        assert_eq!(report.members, 4);
        assert_eq!(report.edges, 4);
    }

    #[test]
    fn an_unregistered_member_fails() {
        let members = vec![member("aquila-rogue", &[])];
        let violations = evaluate(&dag(), &members).expect_err("unregistered");
        assert!(violations.iter().any(|v| v.from == "aquila-rogue"));
    }

    #[test]
    fn an_unlisted_edge_fails() {
        let members = vec![
            member("aquila-types", &[]),
            member("aquila-sensing", &[("aquila-types", DepKind::Normal)]),
            // api -> something not in its allow-list (and up/down levels aside).
            member(
                "aquila-api",
                &[
                    ("aquila-types", DepKind::Normal),
                    ("aquila-sensing", DepKind::Normal),
                ],
            ),
            // Make an edge from sensing to api: not allowed, and upward.
            member(
                "aquila-sensing",
                &[
                    ("aquila-types", DepKind::Normal),
                    ("aquila-api", DepKind::Normal),
                ],
            ),
        ];
        let violations = evaluate(&dag(), &members).expect_err("upward edge");
        assert!(violations.iter().any(|v| v.from == "aquila-sensing"
            && v.to == "aquila-api"
            && v.detail.contains("strictly lower")));
    }

    #[test]
    fn a_normal_dependency_on_testkit_fails() {
        let mut d = dag();
        d.levels.insert("aquila-testkit".to_string(), 0);
        let members = vec![
            member("aquila-types", &[]),
            member("aquila-testkit", &[]),
            member("aquila-sensing", &[("aquila-testkit", DepKind::Normal)]),
        ];
        let violations = evaluate(&d, &members).expect_err("testkit normal dep");
        assert!(violations
            .iter()
            .any(|v| v.detail.contains("dev-dependencies")));
    }

    #[test]
    fn a_cycle_fails() {
        let mut d = dag();
        d.edges
            .insert("aquila-types".to_string(), vec!["aquila-sensing".to_string()]);
        let members = vec![
            member("aquila-types", &[("aquila-sensing", DepKind::Normal)]),
            member("aquila-sensing", &[("aquila-types", DepKind::Normal)]),
        ];
        let violations = evaluate(&d, &members).expect_err("cycle");
        assert!(violations.iter().any(|v| v.detail.contains("cycle")));
    }
}
