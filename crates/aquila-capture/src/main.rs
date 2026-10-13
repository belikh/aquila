#![forbid(unsafe_code)]

//! `aquila-capture` — the dev-only capture, synthesis, verification and
//! provisioning binary.
//!
//! This crate is the `aquila-capture` tool of `capture-and-firmware.md` §8.3
//! (interface register: composition root and tooling, exempt from the layer
//! rule). It is **not** part of the runtime; it exists so a developer can:
//!
//! - `record` — bind the CSI data port (`9100/udp`) and write a `.aqs` Session
//!   from live nodes, carrying one re-encoded `AQCI` datagram per CSI record.
//! - `synth` — write a deterministic `.aqs` Session from a scene script via
//!   [`aquila_simulator`], so the codec and the pipeline are exercised in CI
//!   without hardware.
//! - `verify` — validate a `.aqs` Session against its container invariants and
//!   digest trailer.
//! - `provision` — write a node's provisioning payload (network credentials and
//!   the core endpoint) to a JSON file. Pairing itself happens on the control
//!   channel (`capture-and-firmware.md` §6.3); this tool only prepares the NVS
//!   document an operator would flash.
//!
//! The wire bytes are owned by [`aquila_wire`]: every datagram written by
//! `record` is produced by `aquila_wire::encode_datagram`, so the firmware, the
//! Session and the core cannot drift.
//!
//! No hardware is required: `synth` plus one committed golden Session covers the
//! codec and the pipeline (`capture-and-firmware.md` §8.3).
//!
//! Australian English throughout (serialisation, behaviour, normalise).

mod cli;
mod error;
mod provision;
mod record;
mod synth;
mod verify;

use std::process::ExitCode;

use cli::{Flag, Options};
use error::CaptureError;

/// The process entry point.
fn main() -> ExitCode {
    // `aquila-capture <subcommand> [flags]`.
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("aquila-capture: {error}");
            if matches!(error, CaptureError::Usage(_)) {
                eprintln!();
                eprintln!("{USAGE}");
            }
            ExitCode::FAILURE
        }
    }
}

/// Parse and dispatch one command line.
///
/// # Errors
///
/// Returns a typed [`CaptureError`] for a usage problem, an I/O failure or a
/// domain failure from the invoked subcommand.
fn run(args: &[String]) -> Result<(), CaptureError> {
    let Some((command, rest)) = args.split_first() else {
        return Err(CaptureError::Usage("no subcommand given".to_string()));
    };

    match command.as_str() {
        "record" => match Options::parse(rest, RECORD_FLAGS, "record")? {
            Some(options) => options.run_record(),
            None => Ok(()),
        },
        "synth" => match Options::parse(rest, SYNTH_FLAGS, "synth")? {
            Some(options) => options.run_synth(),
            None => Ok(()),
        },
        "verify" => match Options::parse(rest, VERIFY_FLAGS, "verify")? {
            Some(options) => options.run_verify(),
            None => Ok(()),
        },
        "provision" => match Options::parse(rest, PROVISION_FLAGS, "provision")? {
            Some(options) => options.run_provision(),
            None => Ok(()),
        },
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            Ok(())
        }
        other => Err(CaptureError::Usage(format!("unknown subcommand {other:?}"))),
    }
}

/// The human-readable usage text.
const USAGE: &str = "\
aquila-capture — CSI Session capture, synthesis, verification and provisioning

USAGE:
    aquila-capture <SUBCOMMAND> [FLAGS]

SUBCOMMANDS:
    record     Bind the CSI data port and write a .aqs Session from live nodes
    synth      Write a deterministic .aqs Session from a scene script
    verify     Validate a .aqs Session container and its digest
    provision  Write a node NVS provisioning payload as JSON

Run a subcommand with --help to list its flags.";

/// The flags `record` accepts.
const RECORD_FLAGS: &[Flag] = &[
    Flag::required("out", "the .aqs path to write"),
    Flag::optional("bind", "the local address to bind (default 0.0.0.0)"),
    Flag::optional("port", "the CSI data port (default 9100)"),
    Flag::optional(
        "frames",
        "stop after writing this many frames (default 1000)",
    ),
    Flag::optional("nodes", "a comma-separated node-id filter (default: all)"),
    Flag::optional("session", "the session slug (default live-record)"),
    Flag::optional("home", "the home slug (default home)"),
];

/// The flags `synth` accepts.
const SYNTH_FLAGS: &[Flag] = &[
    Flag::required("out", "the .aqs path to write"),
    Flag::optional(
        "scene",
        "a scene-script JSON path (default: the demo scene)",
    ),
    Flag::optional("seed", "the RNG seed (default 1)"),
    Flag::optional("unix-ms", "the virtual wall-clock origin (default 0)"),
    Flag::optional("host-us", "the virtual host-clock origin (default 0)"),
    Flag::optional("session", "the session slug (default synth)"),
];

/// The flags `verify` accepts.
const VERIFY_FLAGS: &[Flag] = &[Flag::required("session", "the .aqs path to verify")];

/// The flags `provision` accepts.
const PROVISION_FLAGS: &[Flag] = &[
    Flag::required("out", "the JSON path to write"),
    Flag::required("node", "the dense node id"),
    Flag::required("mac", "the factory base MAC, aa:bb:cc:dd:ee:ff"),
    Flag::required("ssid", "the WiFi SSID"),
    Flag::required("core-host", "the core's hostname or IP"),
    Flag::optional("password", "the WiFi password"),
    Flag::optional("core-data-port", "the CSI data port (default 9100)"),
    Flag::optional("core-control-port", "the control port (default 9101)"),
    Flag::optional("pairing-code", "the short-lived pairing code"),
];

#[cfg(test)]
mod tests {
    /// The committed firmware header path, relative to this crate.
    const NODE_WIRE_HEADER: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../firmware/esp32-s3-csi-node/components/node_wire/include/node_wire.h"
    );

    /// Regenerate the committed firmware header from `aquila-wire`.
    ///
    /// Run with `cargo test -p aquila-capture emit_node_wire_header -- --ignored
    /// --nocapture`; the generated bytes are also asserted against the
    /// committed copy by [`committed_node_wire_header_is_current`].
    #[test]
    #[ignore = "regenerates the committed firmware header on demand"]
    fn emit_node_wire_header() {
        let path = std::env::var("NODE_WIRE_OUT")
            .unwrap_or_else(|_| "/tmp/opencode/node_wire.h".to_string());
        std::fs::write(&path, aquila_wire::generate_c_header())
            .expect("write the generated header");
        eprintln!("wrote {path}");
    }

    /// The committed header must match `aquila-wire::generate_c_header()`.
    ///
    /// This is the CI half of the "one layout, two languages" check
    /// (`capture-and-firmware.md` §9.4): the firmware's C header cannot silently
    /// drift from the Rust codec.
    #[test]
    fn committed_node_wire_header_is_current() {
        let committed = std::fs::read_to_string(NODE_WIRE_HEADER)
            .expect("the committed firmware header must exist");
        assert_eq!(
            committed,
            aquila_wire::generate_c_header(),
            "the committed node_wire.h is stale; regenerate it from generate_c_header()"
        );
    }
}
