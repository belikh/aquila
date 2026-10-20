"""10-node ESP32-S3 geometry and provisioning configuration for the 14-day
campaign (INTEGRATION.md §3).

Single room, ~4–6 m × 4–6 m, stable furniture, non-coplanar node placement.
Antenna phase-centre positions surveyed to ~2 cm with tape/laser — deliberate
survey input for this experiment, NOT zero-survey commissioning.

Australian English spelling throughout.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Dict, List

# ---------------------------------------------------------------------------
# Hardware configuration (ESP32-S3, 2.4 GHz only)
# ---------------------------------------------------------------------------

CHANNEL = 6               # 2.4 GHz, fixed for the whole campaign
BANDWIDTH_MHZ = 20        # HT20 first; HT40 only after HT20 baseline stable
DATA_PORT = 9100          # AQCI CSI data plane, UDP
CONTROL_PORT = 9101       # AQCC node control plane, UDP

# Node IDs are dense u16, unique per node, provisioned via AQUILA_NODE_ID Kconfig.
NODE_IDS = list(range(10))  # 0 .. 9

# ---------------------------------------------------------------------------
# Node geometry (metres, relative to room origin)
#
# Nodes 1–4: four room perimeter positions at ~0.3 m height.
# Nodes 5–8: corresponding perimeter positions at ~2.2–2.5 m height.
# Nodes 9–10: opposite mid-wall positions at ~1.2–1.5 m height.
# Non-coplanar; antenna orientation recorded separately; photographed.
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class NodePlacement:
    node_id: int
    x_m: float
    y_m: float
    z_m: float
    label: str  # human-readable, e.g. "perimeter-low-NW"


NODE_PLACEMENTS: List[NodePlacement] = [
    # Low tier (~0.3 m), perimeter.
    NodePlacement(0,  0.30,  0.30,  0.30, "perimeter-low-NW"),
    NodePlacement(1,  5.70,  0.30,  0.30, "perimeter-low-NE"),
    NodePlacement(2,  0.30,  5.70,  0.30, "perimeter-low-SW"),
    NodePlacement(3,  5.70,  5.70,  0.30, "perimeter-low-SE"),
    # High tier (~2.2–2.5 m), corresponding perimeter positions.
    NodePlacement(4,  0.30,  0.30,  2.35, "perimeter-high-NW"),
    NodePlacement(5,  5.70,  0.30,  2.35, "perimeter-high-NE"),
    NodePlacement(6,  0.30,  5.70,  2.35, "perimeter-high-SW"),
    NodePlacement(7,  5.70,  5.70,  2.35, "perimeter-high-SE"),
    # Mid-wall (~1.2–1.5 m), opposite positions.
    NodePlacement(8,  3.00,  0.30,  1.30, "mid-wall-N"),
    NodePlacement(9,  3.00,  5.70,  1.30, "mid-wall-S"),
]

# Map node_id -> placement for fast lookup.
PLACEMENT_BY_ID: Dict[int, NodePlacement] = {n.node_id: n for n in NODE_PLACEMENTS}

# ---------------------------------------------------------------------------
# Provisioning payload template (written to each node via Kconfig / NVS)
# ---------------------------------------------------------------------------

def provisioning_payload(node_id: int, ssid: str, password: str,
                         core_host: str) -> dict:
    """Return the JSON payload to provision one node.

    Fields map to main/Kconfig.projbuild options in
    firmware/esp32-s3-csi-node/main/Kconfig.projbuild.
    """
    return {
        "node_id": node_id,
        "wifi_ssid": ssid,
        "wifi_password": password,
        "core_host": core_host,
        "core_data_port": DATA_PORT,
        "core_control_port": CONTROL_PORT,
        "channel": CHANNEL,
        "bandwidth_mhz": BANDWIDTH_MHZ,
    }


def provisioning_summary(ssid: str, core_host: str) -> str:
    """Human-readable provisioning summary for the operator."""
    lines = [
        "Aquila 10-node provisioning summary",
        f"  SSID: {ssid}",
        f"  Core host: {core_host}:{DATA_PORT} (data) / {CONTROL_PORT} (control)",
        f"  Channel: {CHANNEL} (2.4 GHz), bandwidth: {BANDWIDTH_MHZ} MHz (HT20 first)",
        "",
        "Node placements:",
    ]
    for n in NODE_PLACEMENTS:
        lines.append(
            f"  Node {n.node_id:2d} ({n.label:18s}): "
            f"({n.x_m:5.2f}, {n.y_m:5.2f}, {n.z_m:4.2f}) m"
        )
    return "\n".join(lines)


if __name__ == "__main__":
    print(provisioning_summary("AquilaHome", "192.168.1.100"))
