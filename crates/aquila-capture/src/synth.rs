//! The `synth` subcommand: write a deterministic `.aqs` Session from a scene.
//!
//! Synthesis is delegated entirely to [`aquila_simulator::synthesise_session`],
//! so `synth` adds no behaviour of its own: the same scene, seed and virtual
//! origins produce byte-identical bytes on every machine
//! (`capture-and-firmware.md` §7.1), and a golden Session can be committed for
//! CI (`capture-and-firmware.md` §8.4).

use aquila_simulator::{synthesise_session, SceneScript, SimConfig};
use aquila_types::Slug;

use crate::cli::Options;
use crate::error::CaptureError;

/// Run `synth`.
///
/// # Errors
///
/// Returns a typed [`CaptureError`] for a missing flag, an unreadable scene, an
/// invalid scene or a write failure.
pub(crate) fn run(options: &Options) -> Result<(), CaptureError> {
    let out = options
        .get("out")
        .ok_or_else(|| CaptureError::Usage("synth needs --out".to_string()))?;
    let seed = parse_optional_u64(options, "seed", 1)?;
    let start_unix_ms = parse_optional_i64(options, "unix-ms", 0)?;
    let start_host_us = parse_optional_u64(options, "host-us", 0)?;
    let session = options.get_or("session", "synth");

    let script = match options.get("scene") {
        Some(path) => {
            let bytes = std::fs::read(path)?;
            SceneScript::from_json(&bytes)?
        }
        None => SceneScript::demo(),
    };

    let config = SimConfig::new(
        seed,
        Slug(session.to_string()),
        start_unix_ms,
        start_host_us,
    );
    let bytes = synthesise_session(&script, &config)?;
    std::fs::write(out, &bytes)?;

    println!(
        "synthesised scene {:?} (seed {seed}) to {out}; {} bytes, {} node(s)",
        script.name,
        bytes.len(),
        script.nodes.len(),
    );
    Ok(())
}

/// Parse an optional `u64` flag, falling back to `default`.
fn parse_optional_u64(options: &Options, name: &str, default: u64) -> Result<u64, CaptureError> {
    match options.get(name) {
        None => Ok(default),
        Some(value) => value.parse().map_err(|_| {
            CaptureError::Usage(format!("--{name} must be an integer, got {value:?}"))
        }),
    }
}

/// Parse an optional `i64` flag, falling back to `default`.
fn parse_optional_i64(options: &Options, name: &str, default: i64) -> Result<i64, CaptureError> {
    match options.get(name) {
        None => Ok(default),
        Some(value) => value.parse().map_err(|_| {
            CaptureError::Usage(format!("--{name} must be an integer, got {value:?}"))
        }),
    }
}
