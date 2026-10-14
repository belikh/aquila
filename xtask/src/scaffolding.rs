//! The no-committed-agent-scaffolding guardrail (`repository-tooling.md` §8).
//!
//! `check-scaffolding` runs `git ls-files -z` and fails, listing offending
//! paths, if any **tracked** path matches a pattern in
//! `tools/no-agent-scaffolding.txt`. It never modifies the tree.

use std::path::Path;
use std::process::Command;

/// The patterns checked when `tools/no-agent-scaffolding.txt` is absent.
const DEFAULT_PATTERNS: &[&str] = &[".claude", ".claude-flow", ".swarm"];

/// Run the guardrail. Returns a one-line success summary or the offending
/// paths.
pub fn run() -> Result<String, String> {
    let root = workspace_root()?;
    let patterns = load_patterns(&root);
    let tracked = tracked_paths(&root)?;

    let mut offenders: Vec<String> = tracked
        .into_iter()
        .filter(|path| patterns.iter().any(|pattern| matches(pattern, path)))
        .collect();
    offenders.sort();
    offenders.dedup();

    if offenders.is_empty() {
        Ok(format!(
            "check-scaffolding: OK — no tracked agent scaffolding ({} patterns)",
            patterns.len()
        ))
    } else {
        let mut out = String::from("check-scaffolding: FAILED — tracked agent scaffolding:\n");
        for path in offenders {
            out.push_str(&format!("  {path}\n"));
        }
        Err(out)
    }
}

/// Whether `path` matches `pattern`.
///
/// A trailing `*` is a prefix wildcard; a directory pattern matches the
/// directory and everything beneath it.
fn matches(pattern: &str, path: &str) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return false;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return path.starts_with(prefix);
    }
    let base = pattern.trim_end_matches('/');
    path == base || path.starts_with(&format!("{base}/"))
}

/// Load the pattern list, falling back to the defaults.
fn load_patterns(root: &Path) -> Vec<String> {
    let path = root.join("tools/no-agent-scaffolding.txt");
    match std::fs::read_to_string(&path) {
        Ok(text) => text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(str::to_string)
            .collect(),
        Err(_) => DEFAULT_PATTERNS.iter().map(|s| (*s).to_string()).collect(),
    }
}

/// The repository root: the parent of this tooling crate.
fn workspace_root() -> Result<std::path::PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "xtask has no parent directory".to_string())
}

/// Every git-tracked path, via `git ls-files -z`.
fn tracked_paths(root: &Path) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("cannot run `git ls-files`: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "`git ls-files` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output
        .stdout
        .split(|b| *b == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| String::from_utf8_lossy(entry).into_owned())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_patterns_match_their_subtree_only() {
        assert!(matches(".claude", ".claude"));
        assert!(matches(".claude/", ".claude/settings.json"));
        assert!(matches(".claude-flow", ".claude-flow/x"));
        assert!(matches(".swarm", ".swarm/agents/a"));
        assert!(matches("CLAUDE.md", "CLAUDE.md"));
        assert!(matches(".aider*", ".aider.tags.cache.v3"));
        assert!(!matches(".claude", ".claude-flow/x"));
        assert!(!matches(".claude", "crates/aquila-types/src/lib.rs"));
        assert!(!matches("CLAUDE.md", "docs/CLAUDE.md"));
    }
}
