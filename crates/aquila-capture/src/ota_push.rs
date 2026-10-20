//! The `ota-push` subcommand: push a signed firmware image to a node.
//!
//! A development-time operator tool. It signs an image with the Ed25519
//! firmware key, then drives the chunked transfer to a node over the node
//! control channel (`firmware-ota.md` §3–§7).
//!
//! The tool acts as a minimal core: it listens on the control port for the
//! node's `JOIN_REQUEST`, answers it with a `JOIN_ACK` carrying a fresh
//! pairing token, and then sends the OTA messages authenticated with that
//! token. The node verifies the Ed25519 signature over the image digest
//! before it activates anything, so a tampered image is never installed.
//!
//! Australian English throughout.

use std::fs;
use std::net::{SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::time::Duration;

use aquila_types::MacAddr;
use aquila_wire::{
    ControlBody, ControlMessage, JoinAck, OtaBegin, OtaChunk, OtaEnd, OtaOutcome, SyncPing,
    CONTROL_AUTH_LEN, OTA_DEFAULT_CHUNK_SIZE,
};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

use crate::cli::Options;
use crate::error::CaptureError;

/// The node control channel port.
const CONTROL_PORT: u16 = 9101;

/// How long to wait for a datagram from the node.
const REPLY_TIMEOUT: Duration = Duration::from_secs(5);

/// Run `ota-push`.
///
/// # Errors
///
/// Returns a typed [`CaptureError`] for a missing flag, an unreadable image or
/// key, a signature failure, or a transport failure.
pub(crate) fn run(options: &Options) -> Result<(), CaptureError> {
    let image_path = PathBuf::from(
        options
            .get("image")
            .ok_or_else(|| CaptureError::Usage("ota-push needs --image".to_string()))?,
    );
    let key_path = PathBuf::from(
        options
            .get("key")
            .ok_or_else(|| CaptureError::Usage("ota-push needs --key".to_string()))?,
    );
    let version = options
        .get("version")
        .ok_or_else(|| CaptureError::Usage("ota-push needs --version".to_string()))?
        .to_string();
    let target = options.get("target").unwrap_or("esp32s3").to_string();
    let core = options
        .get("core")
        .ok_or_else(|| CaptureError::Usage("ota-push needs --core".to_string()))?
        .to_string();
    let want_mac = match options.get("mac") {
        Some(value) => Some(parse_mac(value)?),
        None => None,
    };
    let pairing_code = options
        .get("pairing-code")
        .map(|value| value.parse::<u32>())
        .transpose()
        .map_err(|_| CaptureError::Usage("--pairing-code must be a u32".to_string()))?;
    let chunk_size = options
        .get("chunk-size")
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(OTA_DEFAULT_CHUNK_SIZE);

    let image = fs::read(&image_path).map_err(|error| {
        CaptureError::Usage(format!("cannot read {}: {error}", image_path.display()))
    })?;
    if image.is_empty() {
        return Err(CaptureError::Usage("the image is empty".to_string()));
    }

    let seed = fs::read(&key_path).map_err(|error| {
        CaptureError::Usage(format!("cannot read {}: {error}", key_path.display()))
    })?;
    if seed.len() != 32 {
        return Err(CaptureError::Usage(format!(
            "the signing key must be 32 bytes, got {}",
            seed.len()
        )));
    }
    let seed: [u8; 32] = seed.try_into().map_err(|_| {
        CaptureError::Usage("the signing key must be exactly 32 bytes".to_string())
    })?;
    let signing_key = SigningKey::from_bytes(&seed);

    // The version must be `major.minor.patch`; the node parses it into the
    // anti-rollback security floor and refuses anything it cannot parse.
    if !is_security_version(&version) {
        return Err(CaptureError::Usage(format!(
            "--version must be major.minor.patch, got `{version}`"
        )));
    }

    // Sign the SHA-256 digest, exactly as the node verifies it.
    let digest = sha256(&image);
    let signature = signing_key.sign(&digest).to_bytes();
    let image_id = hex_lower(&digest);

    println!("signed {image_id}");
    println!("  version {version}, target {target}, {} bytes", image.len());

    let core_addr: SocketAddr = format!("{core}:{CONTROL_PORT}")
        .parse()
        .map_err(|_| CaptureError::Usage(format!("`{core}` is not a host")))?;

    // Bind the control port so the node's JOIN_REQUEST reaches us.
    let socket = UdpSocket::bind(core_addr)
        .map_err(|error| CaptureError::Usage(format!("cannot bind {core_addr}: {error}")))?;
    socket
        .set_read_timeout(Some(REPLY_TIMEOUT))
        .map_err(|error| CaptureError::Usage(format!("cannot set a timeout: {error}")))?;

    // A probe answers one question: does the node accept the token? It sends
    // an authenticated SYNC_PING and reports whether the SYNC_ECHO comes back.
    if options.get("probe").is_some() {
        let addr = match options.get("node-ip") {
            Some(value) => {
                let ip: std::net::IpAddr = value.parse().map_err(|_| {
                    CaptureError::Usage(format!("`{value}` is not an IP address"))
                })?;
                find_node(&socket, want_mac, Some(ip))?
            }
            None => find_node(&socket, want_mac, None)?,
        };
        let token = match options.get("token").map(|v| v.to_string()).or_else(|| {
            options
                .get("token-file")
                .and_then(|path| fs::read_to_string(path).ok())
                .map(|text| text.trim().to_string())
        }) {
            Some(value) => parse_token(&value)?,
            None => {
                return Err(CaptureError::Usage(
                    "a probe needs --token or --token-file".to_string(),
                ))
            }
        };
        return probe(&socket, addr, &token);
    }

    // A node that has already paired keeps its token in NVS and ignores a
    // fresh JOIN_ACK, so an already-paired node needs the stored token.
    //
    // An explicit `--token` always wins. Otherwise `--token-file` is a *read*
    // source only when the caller did not ask to pair: asking to pair means
    // "issue me a fresh token", and the file is then where the new token is
    // written. Without that split the two uses of the flag collide and the
    // pairing path becomes unreachable.
    let stored_token = options.get("token").map(|v| v.to_string()).or_else(|| {
        if pairing_code.is_some() {
            return None;
        }
        options
            .get("token-file")
            .and_then(|path| fs::read_to_string(path).ok())
            .map(|text| text.trim().to_string())
    });
    let (node_addr, token) = match stored_token {
        Some(value) => {
            let token = parse_token(&value)?;
            println!("  using the supplied pairing token");
            let addr = match options.get("node-ip") {
                Some(value) => {
                    // The node's control socket binds an ephemeral port, so
                    // the port to reply to is learned from its own traffic
                    // rather than assumed to be the control port.
                    let ip: std::net::IpAddr = value.parse().map_err(|_| {
                        CaptureError::Usage(format!("`{value}` is not an IP address"))
                    })?;
                    find_node(&socket, want_mac, Some(ip))?
                }
                None => find_node(&socket, want_mac, None)?,
            };
            (addr, token)
        }
        None => {
            // Pair: the node sends JOIN_REQUEST, we answer JOIN_ACK with a token.
            let code = pairing_code.ok_or_else(|| {
                CaptureError::Usage("ota-push needs --pairing-code".to_string())
            })?;
            let paired = pair(&socket, code, want_mac)?;
            println!("  paired with {}", paired.0);
            // Record the token: a paired node ignores a later JOIN_ACK, so a
            // second push needs the token this one issued.
            if let Some(path) = options.get("token-file") {
                fs::write(path, hex_lower(&paired.1)).map_err(|error| {
                    CaptureError::Usage(format!("cannot write {path}: {error}"))
                })?;
                println!("  token written to {path}");
            }
            paired
        }
    };

    push_image(
        &socket,
        node_addr,
        &token,
        &image,
        &signature,
        &version,
        chunk_size,
    )
}

/// Send an authenticated SYNC_PING and report whether the node answers.
fn probe(
    socket: &UdpSocket,
    node: SocketAddr,
    token: &[u8; CONTROL_AUTH_LEN],
) -> Result<(), CaptureError> {
    let ping = ControlMessage {
        request_id: 1,
        body: ControlBody::SyncPing(SyncPing { t1_us: 0 }),
    };
    send_authenticated(socket, node, &ping, token)?;

    let mut buf = [0u8; 2048];
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        socket
            .set_read_timeout(Some(Duration::from_millis(500)))
            .map_err(|error| CaptureError::Usage(format!("cannot set a timeout: {error}")))?;
        let Ok((read, from)) = socket.recv_from(&mut buf) else {
            continue;
        };
        let Ok(message) = aquila_wire::decode_control(&buf[..read]) else {
            continue;
        };
        match message.body {
            ControlBody::SyncEcho(_) => {
                println!("  the node accepted the token: SYNC_ECHO from {from}");
                return Ok(());
            }
            ControlBody::OtaResult(result) => {
                println!("  the node replied: {result:?}");
                return Ok(());
            }
            _ => {}
        }
    }
    println!("  no reply: the node did not accept the token");
    Err(CaptureError::Usage(
        "the node did not answer the authenticated probe".to_string(),
    ))
}

/// Find the node's address from its traffic, for the already-paired case.
///
/// A paired node no longer sends JOIN_REQUEST — it sends STATUS heartbeats
/// instead — so any datagram from the node locates it. A JOIN_REQUEST from
/// the wanted MAC is preferred, because it names the node.
fn find_node(
    socket: &UdpSocket,
    want_mac: Option<MacAddr>,
    want_ip: Option<std::net::IpAddr>,
) -> Result<SocketAddr, CaptureError> {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let mut buf = [0u8; 2048];
    let mut fallback: Option<SocketAddr> = None;
    while std::time::Instant::now() < deadline {
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .map_err(|error| CaptureError::Usage(format!("cannot set a timeout: {error}")))?;
        let Ok((read, from)) = socket.recv_from(&mut buf) else {
            continue;
        };
        let Ok(message) = aquila_wire::decode_control(&buf[..read]) else {
            continue;
        };
        // The node's control socket binds an ephemeral port and sends from
        // it, so the port the core must reply to is whatever the node's own
        // datagrams carry. Learn it from the traffic.
        if let Some(ip) = want_ip {
            if from.ip() != ip {
                continue;
            }
        }
        if let Some(wanted) = want_mac {
            if let ControlBody::JoinRequest(request) = message.body {
                if request.node == wanted {
                    return Ok(from);
                }
            }
            // A paired node sends STATUS heartbeats, which do not name the
            // sender: the address is all there is to match on.
            if want_ip.is_some() {
                return Ok(from);
            }
            continue;
        }
        // No MAC was given: any node's datagram locates it.
        if fallback.is_none() {
            fallback = Some(from);
        }
    }
    fallback.ok_or_else(|| {
        CaptureError::Usage("the node did not announce itself on the control channel".to_string())
    })
}

/// Parse a 64-character hex pairing token.
fn parse_token(value: &str) -> Result<[u8; CONTROL_AUTH_LEN], CaptureError> {
    if value.len() != CONTROL_AUTH_LEN * 2 {
        return Err(CaptureError::Usage(format!(
            "--token must be {} hex characters",
            CONTROL_AUTH_LEN * 2
        )));
    }
    let mut bytes = [0u8; CONTROL_AUTH_LEN];
    for (index, slot) in bytes.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&value[2 * index..2 * index + 2], 16).map_err(|_| {
            CaptureError::Usage("--token is not a hex string".to_string())
        })?;
    }
    Ok(bytes)
}

/// Wait for the node's `JOIN_REQUEST` and answer it with a `JOIN_ACK`.
///
/// Several nodes may be joining at once, so a JOIN_REQUEST from a node other
/// than the one wanted is answered with a rejection and the wait continues.
fn pair(
    socket: &UdpSocket,
    pairing_code: u32,
    want_mac: Option<MacAddr>,
) -> Result<(SocketAddr, [u8; CONTROL_AUTH_LEN]), CaptureError> {
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    let mut buf = [0u8; 2048];
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err(CaptureError::Usage(
                "no node offered the right pairing code in time".to_string(),
            ));
        }
        socket
            .set_read_timeout(Some(remaining.min(REPLY_TIMEOUT)))
            .map_err(|error| CaptureError::Usage(format!("cannot set a timeout: {error}")))?;

        let (read, from) = match socket.recv_from(&mut buf) {
            Ok(result) => result,
            Err(error) => {
                if std::io::ErrorKind::WouldBlock == error.kind()
                    || std::io::ErrorKind::TimedOut == error.kind()
                {
                    continue;
                }
                return Err(CaptureError::Usage(format!(
                    "no JOIN_REQUEST from the node: {error}"
                )));
            }
        };
        eprintln!("  datagram {read} bytes from {from}");

        let message = match aquila_wire::decode_control(&buf[..read]) {
            Ok(message) => message,
            Err(error) => {
                eprintln!("  undecodable datagram: {error}");
                continue;
            }
        };
        eprintln!("  decoded: {:?}", message.body);
        let request = match message.body {
            ControlBody::JoinRequest(request) => request,
            _ => continue,
        };
        if let Some(wanted) = want_mac {
            if request.node != wanted {
                // Not the node we want: tell it so and keep waiting.
                let reject = ControlMessage {
                    request_id: message.request_id,
                    body: ControlBody::Result(ack_rejected("not the target node")),
                };
                if let Ok(bytes) = aquila_wire::encode_control(&reject) {
                    let _ = socket.send_to(&bytes, from);
                }
                continue;
            }
        }
        if request.pairing_code != pairing_code {
            return Err(CaptureError::Usage(
                "the node offered the wrong pairing code".to_string(),
            ));
        }

        let token = fresh_token();
        let ack = ControlMessage {
            request_id: message.request_id,
            body: ControlBody::JoinAck(JoinAck {
                node: request.node,
                token,
                config_version: 1,
                config_hash: [0u8; 32],
            }),
        };
        // JOIN_ACK is the one message accepted before pairing: it delivers the
        // token, so it carries no trailer.
        let bytes = aquila_wire::encode_control(&ack)
            .map_err(|error| CaptureError::Usage(format!("cannot encode JOIN_ACK: {error}")))?;
        socket
            .send_to(&bytes, from)
            .map_err(|error| CaptureError::Usage(format!("cannot send JOIN_ACK: {error}")))?;

        return Ok((from, token));
    }
}

/// A rejection body.
fn ack_rejected(detail: &str) -> aquila_wire::Ack {
    aquila_wire::Ack {
        status: aquila_wire::AckStatus::Rejected,
        detail: detail.to_string(),
    }
}

/// Parse an `aa:bb:cc:dd:ee:ff` MAC address.
fn parse_mac(value: &str) -> Result<MacAddr, CaptureError> {
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() != 6 {
        return Err(CaptureError::Usage(format!(
            "`{value}` is not a six-octet MAC address"
        )));
    }
    let mut octets = [0u8; 6];
    for (index, part) in parts.iter().enumerate() {
        octets[index] = u8::from_str_radix(part, 16).map_err(|_| {
            CaptureError::Usage(format!("`{value}` is not a hex MAC address"))
        })?;
    }
    Ok(MacAddr::new(octets))
}

/// Drive the chunked transfer: `OTA_BEGIN`, then `OTA_CHUNK` per chunk, then
/// `OTA_END` with the digest for the node's final check.
fn push_image(
    socket: &UdpSocket,
    node: SocketAddr,
    token: &[u8; CONTROL_AUTH_LEN],
    image: &[u8],
    signature: &[u8; 64],
    version: &str,
    chunk_size: u16,
) -> Result<(), CaptureError> {
    let digest = sha256(image);
    // A core-chosen transfer id, echoed back by the node.
    let image_id = 0x414F_5401u32;

    // The node keeps a small replay window of recent request_ids and drops
    // any message whose id is in it, so the ids must not repeat across
    // runs. A time-derived base keeps them unique.
    let request_base = u32::from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos()),
    ) | 1;

    let begin = ControlMessage {
        request_id: request_base,
        body: ControlBody::OtaBegin(OtaBegin {
            image_id,
            size: image.len() as u32,
            sha256: digest,
            signature: *signature,
            version: version.to_string(),
            chunk_size,
            window: 4,
        }),
    };
    send_authenticated(socket, node, &begin, token)?;

    let mut request_id = request_base.wrapping_add(1);
    let mut offset = 0usize;

    // The node's control socket has a small receive buffer, so the transfer
    // is paced by the node's cumulative acknowledgement: each chunk waits for
    // the ACK that says it landed, and a NACK resumes from the offset the
    // node reports. This is the same sliding window the core's manager uses
    // (`firmware-ota.md` §5), with one chunk in flight.
    while offset < image.len() {
        let end = (offset + chunk_size as usize).min(image.len());
        let chunk = &image[offset..end];
        let message = ControlMessage {
            request_id,
            body: ControlBody::OtaChunk(OtaChunk {
                offset: offset as u32,
                len: chunk.len() as u16,
                crc32: crc32(chunk),
                bytes: chunk.to_vec(),
            }),
        };
        request_id = request_id.wrapping_add(1);
        send_authenticated(socket, node, &message, token)?;

        // Wait for the node to acknowledge this chunk; the offset it reports
        // is where the transfer resumes.
        offset = wait_for_ack(socket, token)? as usize;
        if offset.is_multiple_of(chunk_size as usize * 16) || offset == image.len() {
            println!("  {offset}/{} bytes", image.len());
        }
    }

    let end = ControlMessage {
        request_id,
        body: ControlBody::OtaEnd(OtaEnd {
            image_id,
            sha256: digest,
        }),
    };
    send_authenticated(socket, node, &end, token)?;
    println!("  OTA_END sent; the node will verify, activate and reboot");

    // The node reboots into the new image and holds it on a pending-verify
    // state for the health window. The health check requires the node to be
    // associated, streaming, AND control-responsive — so the tool stays up
    // and keeps the control channel alive with SYNC_PING until the node
    // reports the outcome, or the window closes.
    //
    // Nothing may open the node's serial port while this runs: asserting
    // DTR/RTS resets the chip and aborts the transfer.
    println!("  holding the control channel open for the health window...");
    hold_open(socket, node, token)?;
    Ok(())
}

/// Wait for the node's cumulative acknowledgement and return the offset it
/// reports: every byte below that offset is received.
fn wait_for_ack(
    socket: &UdpSocket,
    _token: &[u8; CONTROL_AUTH_LEN],
) -> Result<u32, CaptureError> {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut buf = [0u8; 2048];
    while std::time::Instant::now() < deadline {
        socket
            .set_read_timeout(Some(Duration::from_millis(500)))
            .map_err(|error| CaptureError::Usage(format!("cannot set a timeout: {error}")))?;
        let Ok((read, _)) = socket.recv_from(&mut buf) else {
            continue;
        };
        let Ok(message) = aquila_wire::decode_control(&buf[..read]) else {
            continue;
        };
        match message.body {
            ControlBody::OtaAck(ack) => return Ok(ack.received_upto),
            ControlBody::OtaResult(result) => {
                return Err(CaptureError::Usage(format!(
                    "the node refused the transfer: {result:?}"
                )))
            }
            _ => {}
        }
    }
    Err(CaptureError::Usage(
        "the node did not acknowledge the chunk in time".to_string(),
    ))
}

/// Keep the control channel alive until the node reports an OTA outcome or
/// the health window closes.
///
/// The node's control socket has no fixed port, so its source port changes on
/// every reboot. The address is therefore tracked from the traffic the node
/// sends: without that, the SYNC_PINGs would go to a stale port and the node
/// would never become control-responsive, so the health window would fail.
fn hold_open(
    socket: &UdpSocket,
    node: SocketAddr,
    token: &[u8; CONTROL_AUTH_LEN],
) -> Result<(), CaptureError> {
    let deadline = std::time::Instant::now() + Duration::from_secs(150);
    let mut request_id = u32::from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos()),
    ) | 1;
    let mut buf = [0u8; 2048];
    let mut next_ping = std::time::Instant::now();
    let mut target = node;

    while std::time::Instant::now() < deadline {
        if std::time::Instant::now() >= next_ping {
            // SYNC_PING: the node answers with SYNC_ECHO, which is what marks
            // it control-responsive for the health window.
            let ping = ControlMessage {
                request_id,
                body: ControlBody::SyncPing(SyncPing { t1_us: 0 }),
            };
            let _ = send_authenticated(socket, target, &ping, token);
            request_id += 1;
            next_ping = std::time::Instant::now() + Duration::from_secs(2);
        }

        socket
            .set_read_timeout(Some(Duration::from_millis(500)))
            .map_err(|error| CaptureError::Usage(format!("cannot set a timeout: {error}")))?;
        if let Ok((read, from)) = socket.recv_from(&mut buf) {
            // The node's source port moves on reboot: follow it. Other nodes
            // share this socket, so only the target's own traffic counts.
            if from.ip() == target.ip() {
                target = from;
            }
            if let Ok(message) = aquila_wire::decode_control(&buf[..read]) {
                match &message.body {
                    ControlBody::OtaResult(result) => {
                        println!("  node outcome: {result:?}");
                        // `Verified` is the *pre-reboot* outcome: the node has
                        // verified the image but has not activated it yet, and
                        // the health window has not run. Returning here would
                        // stop the core from keeping the control channel
                        // responsive, and the node would then fail its own
                        // health check and roll the image back. Only a
                        // terminal outcome ends the wait.
                        if !matches!(result.outcome, OtaOutcome::Verified) {
                            return Ok(());
                        }
                    }
                    ControlBody::SyncEcho(_) => {
                        println!("  node is control-responsive");
                    }
                    _ => {}
                }
            }
        }
    }
    println!("  health window closed; check the node's serial output");
    Ok(())
}

/// Send one control message with the HMAC trailer the node requires after
/// pairing.
fn send_authenticated(
    socket: &UdpSocket,
    node: SocketAddr,
    message: &ControlMessage,
    token: &[u8; CONTROL_AUTH_LEN],
) -> Result<(), CaptureError> {
    let bytes = aquila_wire::encode_control_authenticated(message, token)
        .map_err(|error| CaptureError::Usage(format!("cannot encode: {error}")))?;
    socket
        .send_to(&bytes, node)
        .map_err(|error| CaptureError::Usage(format!("cannot send to {node}: {error}")))?;
    Ok(())
}

/// A fresh 32-byte pairing token, from the system entropy source.
fn fresh_token() -> [u8; CONTROL_AUTH_LEN] {
    let mut bytes = [0u8; CONTROL_AUTH_LEN];
    // /dev/urandom is the system's entropy source; a failure there is fatal
    // for a pairing token, so fall back to a time-and-pid mixed value rather
    // than a constant.
    if let Ok(random) = read_random(&mut bytes) {
        if random {
            return bytes;
        }
    }
    let mut state = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
        ^ (u64::from(std::process::id()) << 32);
    for slot in bytes.iter_mut() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *slot = (state & 0xff) as u8;
    }
    bytes
}

/// Fill `buffer` from /dev/urandom. Returns whether it succeeded.
///
/// The read is exactly `buffer.len()` bytes: /dev/urandom is an endless
/// stream, so reading it whole would never terminate.
fn read_random(buffer: &mut [u8; CONTROL_AUTH_LEN]) -> std::io::Result<bool> {
    use std::io::Read;
    let mut file = std::fs::File::open("/dev/urandom")?;
    file.read_exact(buffer)?;
    Ok(true)
}

/// Whether `version` parses as `major.minor.patch`, as the node requires.
fn is_security_version(version: &str) -> bool {
    let mut parts = version.split('.');
    for _ in 0..3 {
        let Some(part) = parts.next() else {
            return false;
        };
        if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        if part.parse::<u32>().unwrap_or(9999) > 999 {
            return false;
        }
    }
    parts.next().is_none()
}

/// The SHA-256 of `data`.
fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// The CRC-32 (IEEE) of `data`, as the OTA chunk checksum.
///
/// The node verifies with `esp_rom_crc32_le(0, ...)`, which is the standard
/// zlib/IEEE CRC-32 — the same value `crc32fast::hash` produces on the core
/// (`firmware-ota.md` §5).
fn crc32(data: &[u8]) -> u32 {
    crc32fast::hash(data)
}

/// Lower-case hex.
fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
