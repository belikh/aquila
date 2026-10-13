# Research — node backhaul (CSI transport to the core)

**Status:** research. Decision taken: **WiFi UDP unicast** to the core; ESP-NOW and PoE dropped;
MQTT not used.
**Last updated:** 2026-10-13. **Sources:** see `../reference/sources.json`.

## Question

How should ~9+ ESP32 nodes across three floors deliver CSI (and timing) to a central HA-hosted core?

## Findings

- **esp-csi** leaves transport to the integrator; it recommends one dedicated broadcast sender with
  many receivers for multi-device accuracy.
- **ESPectre** runs detection on-device and reports motion via ESPHome/MQTT/Matter; broadcast CSI is
  explicitly unreliable — **unicast/multicast is required**.
- **ESPARGOS / esp-sdr** streams CSI over Ethernet/UDP; phase coherence exists only **within** an
  array; across arrays it needs a **shared 40 MHz + 2.4 GHz reference over coax** with equal-length
  cables and software compensation.
- **RuView** `stream_sender.c` sends frames by **UDP to :5005** (~20 Hz/channel, ~5 KB/s/node, 64
  subcarriers, 1 antenna); vitals 1 Hz.
- **In-home norms:** ESPHome uses a protobuf/TCP native API; Zigbee2MQTT assumes an MQTT broker;
  Frigate uses RTSP + MQTT; HA add-ons are containers and need `host_network`/raw sockets for LAN
  UDP/multicast.
- **Trade-offs:** CSI volume is trivial for IP (~15 KB/s/node, ~135 KB/s for 9). UDP is lossy but
  low-jitter (best for timestamping); TCP/MQTT adds retransmit and a broker hop (poor for timing);
  ESP-NOW is low-jitter but channel-locked and needs a gateway. **Phase coherence cannot be
  recovered in software** — it requires a shared RF reference.

## Implication for Aquila

Recommended primary was wired Ethernet/PoE UDP with per-floor aggregation; **PoE was ruled out on
cost**, so the settled backhaul is **WiFi UDP unicast** from each node to the core's static IP, with
per-node timestamps and TDM slots. **ESP-NOW dropped. MQTT not used** — the control plane runs over
the core's private HTTP+WS and a direct node control channel. Feature-level fusion only; the
coherent array is a future research track (see `esp32-as-sdr.md`).
