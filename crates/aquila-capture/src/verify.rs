//! The `verify` subcommand: validate a `.aqs` Session container.
//!
//! Verification recomputes the record-stream digest and checks the trailer and
//! manifest invariants (`capture-and-firmware.md` §8.1–8.4), which is the same
//! check a CI job runs over the committed fixtures.

use std::path::Path;

use crate::cli::Options;
use crate::error::CaptureError;

/// Run `verify`.
///
/// # Errors
///
/// Returns a typed [`CaptureError`] when the container cannot be read or fails a
/// structural or digest check.
pub(crate) fn run(options: &Options) -> Result<(), CaptureError> {
    let path = options
        .get("session")
        .or_else(|| options.positional().first().map(String::as_str))
        .ok_or_else(|| CaptureError::Usage("verify needs --session <path>".to_string()))?;
    let report = aquila_session::verify(Path::new(path))?;
    println!(
        "verified {path}: session {:?}, container v{}, {} records, {} CSI frames, digest {:02x}{:02x}{:02x}{:02x}…",
        report.session_id,
        report.version,
        report.record_count,
        report.frames,
        report.digest[0],
        report.digest[1],
        report.digest[2],
        report.digest[3],
    );
    Ok(())
}
