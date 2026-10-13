//! The `provision` subcommand: write a node's NVS provisioning payload.
//!
//! The firmware persists network credentials, the core endpoint and the pairing
//! token in NVS (`capture-and-firmware.md` §9.3). This tool prepares the
//! **pre-pairing** document — network credentials and the core endpoint — as
//! JSON, for an operator to flash. Pairing itself happens on the control channel
//! (`capture-and-firmware.md` §6.3); the pairing token is issued there, not
//! here.
//!
//! No secret is ever logged: the payload is written to the named file only.

use serde::Serialize;

use crate::cli::Options;
use crate::error::CaptureError;

/// The JSON schema label of a provisioning document.
const SCHEMA: &str = "aquila-provisioning/1";

/// A node's pre-pairing provisioning payload.
#[derive(Debug, Serialize)]
struct Provisioning {
    /// The document schema label.
    schema: &'static str,
    /// The dense node id assigned in the home model.
    node: u16,
    /// The factory base MAC, lower-case hex.
    mac: String,
    /// The WiFi credentials the node associates with.
    wifi: Wifi,
    /// The core endpoint the node streams CSI to and is controlled from.
    core: Core,
    /// The short-lived pairing code, when one was supplied.
    pairing_code: Option<u32>,
}

/// The WiFi credentials a node associates with.
#[derive(Debug, Serialize)]
struct Wifi {
    /// The network SSID.
    ssid: String,
    /// The network password, when the network is not open.
    #[serde(skip_serializing_if = "Option::is_none")]
    password: Option<String>,
}

/// The core endpoint a node talks to.
#[derive(Debug, Serialize)]
struct Core {
    /// The core's hostname or address.
    host: String,
    /// The CSI data-plane port (`9100/udp`).
    data_port: u16,
    /// The node control-plane port (`9101/udp`).
    control_port: u16,
}

/// Run `provision`.
///
/// # Errors
///
/// Returns a typed [`CaptureError`] for a missing flag, a malformed MAC, an
/// invalid port or a write failure.
pub(crate) fn run(options: &Options) -> Result<(), CaptureError> {
    let out = options
        .get("out")
        .ok_or_else(|| CaptureError::Usage("provision needs --out".to_string()))?;
    let node = options
        .get("node")
        .ok_or_else(|| CaptureError::Usage("provision needs --node".to_string()))?
        .parse::<u16>()
        .map_err(|_| CaptureError::Usage("--node must be a u16".to_string()))?;
    let mac = options
        .get("mac")
        .ok_or_else(|| CaptureError::Usage("provision needs --mac".to_string()))?;
    let mac = normalise_mac(mac)?;
    let ssid = options
        .get("ssid")
        .ok_or_else(|| CaptureError::Usage("provision needs --ssid".to_string()))?
        .to_string();
    let host = options
        .get("core-host")
        .ok_or_else(|| CaptureError::Usage("provision needs --core-host".to_string()))?
        .to_string();
    let data_port = options.get_u16_or("core-data-port", aquila_wire::CSI_DATA_PORT)?;
    let control_port = options.get_u16_or("core-control-port", aquila_wire::CONTROL_PORT)?;
    let pairing_code = match options.get("pairing-code") {
        None => None,
        Some(value) => Some(
            value
                .parse::<u32>()
                .map_err(|_| CaptureError::Usage("--pairing-code must be a u32".to_string()))?,
        ),
    };

    let payload = Provisioning {
        schema: SCHEMA,
        node,
        mac,
        wifi: Wifi {
            ssid,
            password: options.get("password").map(str::to_string),
        },
        core: Core {
            host,
            data_port,
            control_port,
        },
        pairing_code,
    };
    let json = serde_json::to_vec_pretty(&payload)?;
    std::fs::write(out, &json)?;

    println!(
        "wrote provisioning for node {node} to {out} (core {}:{data_port}/udp data, {}:{control_port}/udp control)",
        payload.core.host, payload.core.host,
    );
    Ok(())
}

/// Validate and canonicalise a `aa:bb:cc:dd:ee:ff` MAC address.
fn normalise_mac(value: &str) -> Result<String, CaptureError> {
    let octets: Vec<&str> = value.split(':').collect();
    if octets.len() != 6 {
        return Err(CaptureError::Usage(format!(
            "--mac {value:?} must have six octets"
        )));
    }
    let mut out = String::with_capacity(17);
    for (index, octet) in octets.iter().enumerate() {
        let byte = u8::from_str_radix(octet, 16)
            .map_err(|_| CaptureError::Usage(format!("--mac octet {octet:?} is not hex")))?;
        if index > 0 {
            out.push(':');
        }
        out.push_str(&format!("{byte:02x}"));
    }
    Ok(out)
}
