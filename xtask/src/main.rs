//! `xtask` — Aquila repository tooling (`repository-tooling.md` §5, §8, §10.2).
//!
//! Subcommands:
//!
//! - `dag-check` — enforce the `aquila-*` dependency allow-list in
//!   `tools/dag.toml` against `cargo metadata` (no upward or unlisted edges, no
//!   cycles, every registered name an actual member).
//! - `check-scaffolding` — fail if agent scaffolding (`.claude`,
//!   `.claude-flow`, `.swarm`) is git-tracked.
//! - `coverage-floor` — parse an LCOV record and enforce the line-coverage
//!   floor (`testing.md` §6).
//! - `claims-verify` — validate the honest-claims registry (`testing.md` §7).

#![forbid(unsafe_code)]

mod claims;
mod coverage;
mod dag;
mod scaffolding;

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("dag-check") => dag::run(),
        Some("check-scaffolding") => scaffolding::run(),
        Some("coverage-floor") => coverage::run(&args[1..]),
        Some("claims-verify") => claims::run(&args[1..]),
        Some("-h") | Some("--help") | None => {
            print_usage();
            return;
        }
        Some(other) => {
            eprintln!("xtask: unknown subcommand `{other}`\n");
            print_usage();
            std::process::exit(2);
        }
    };

    match result {
        Ok(message) => println!("{message}"),
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    }
}

/// The workspace root: the parent of this tooling crate.
pub(crate) fn workspace_root() -> Result<PathBuf, String> {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(std::path::Path::to_path_buf)
        .ok_or_else(|| "xtask has no parent directory".to_string())
}

/// Print usage to stdout.
fn print_usage() {
    println!("xtask — Aquila repository tooling\n");
    println!("USAGE:\n    cargo run -p xtask -- <SUBCOMMAND>\n");
    println!("SUBCOMMANDS:");
    println!("    dag-check           enforce the dependency allow-list in tools/dag.toml");
    println!("    check-scaffolding   fail if agent scaffolding is git-tracked");
    println!("    coverage-floor      enforce the line-coverage floor from an LCOV record");
    println!("    claims-verify       validate claims.toml (the honest-claims registry)");
    println!("    help                print this message");
}
