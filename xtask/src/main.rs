//! `xtask` — Aquila repository tooling (`repository-tooling.md` §5, §8).
//!
//! Subcommands:
//!
//! - `dag-check` — enforce the `aquila-*` dependency allow-list in
//!   `tools/dag.toml` against `cargo metadata` (no upward or unlisted edges, no
//!   cycles, every member registered).
//! - `check-scaffolding` — fail if agent scaffolding (`.claude`,
//!   `.claude-flow`, `.swarm`) is git-tracked.

#![forbid(unsafe_code)]

mod dag;
mod scaffolding;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("dag-check") => dag::run(),
        Some("check-scaffolding") => scaffolding::run(),
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

/// Print usage to stdout.
fn print_usage() {
    println!("xtask — Aquila repository tooling\n");
    println!("USAGE:\n    cargo run -p xtask -- <SUBCOMMAND>\n");
    println!("SUBCOMMANDS:");
    println!("    dag-check           enforce the dependency allow-list in tools/dag.toml");
    println!("    check-scaffolding   fail if agent scaffolding is git-tracked");
    println!("    help                print this message");
}
