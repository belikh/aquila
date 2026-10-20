"""Hardware interface for real ESP32-S3 CSI nodes.

When the 10 ESP32-S3 devices arrive, this module collects CSI from them over
the AQCI UDP data plane (port 9100) and feeds it into the same analysis
functions used by the pure-Python simulation. The analysis code (closure
phase, reciprocal gauge, ambient ledger) is unchanged — only the CSI source
swaps from csi_sim.measure_csi to a real UDP socket.

Design (from docs/spec/capture-and-firmware.md §5):
  * One CSI frame per UDP datagram, node -> core, unicast.
  * Default port 9100/udp, private-only.
  * 16-byte AQCI header + 28-byte body + 2*n_subcarriers int8 I/Q payload.
  * Header fields: magic "AQCI", wire_major/minor, msg_type, sc_format,
    node_id (u16), payload_len, band, channel, bandwidth_mhz, flags.
  * Body: sequence(u64), device_time_us(u64), boot_epoch(u16),
    subcarrier_offset(i16), n_subcarriers(u16), rssi_dbm(i8),
    noise_floor_dbm(i8), phy_flags(u16), reserved(u16),
    then i_q[0..2n] interleaved signed int8 Q,I (imaginary first).

Usage:
    # Discover nodes on the network.
    nodes = discover_nodes(timeout_s=5.0)
    # Collect a directed-loop capture.
    capture = collect_loop_capture(nodes, loop=("n0","n1","n2"), duration_s=10.0)
    # Analyse.
    from closure_phase import compute_closure_phase
    ...
"""

from __future__ import annotations

import socket
import struct
import threading
import time
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Tuple

AQCI_MAGIC = b"AQCI"
AQCI_DATA_PORT = 9100
AQCI_HEADER_SIZE = 16
AQCI_BODY_SIZE = 28
AQCI_MSG_TYPE_CSI_FRAME = 1
AQCI_SC_FORMAT_INT8_IQ = 1  # raw signed int8 I/Q, imaginary first


@dataclass
class CsiFrame:
    """One decoded AQCI CSI datagram."""

    node_id: int
    sequence: int
    device_time_us: int
    boot_epoch: int
    subcarrier_offset: int
    n_subcarriers: int
    centre_freq_hz: int
    bandwidth_mhz: int
    rssi_dbm: int
    noise_floor_dbm: int
    first_word_invalid: bool
    # Complex CSI per subcarrier: re/im in f32 (derived from int8 I/Q).
    csi: List[complex]
    host_arrival_us: int


@dataclass
class NodeInfo:
    """Discovered node on the mesh."""

    node_id: int
    mac: str
    ip: str
    first_seen: float
    last_seen: float
    n_frames: int


class CsiCollector:
    """UDP listener that collects AQCI CSI datagrams from all nodes.

    Runs a background socket thread; callers poll get_frames() or use
    collect_for(seconds) to grab a time-bounded capture.
    """

    def __init__(self, bind_port: int = AQCI_DATA_PORT) -> None:
        self.bind_port = bind_port
        self._sock: Optional[socket.socket] = None
        self._running = False
        self._thread: Optional[threading.Thread] = None
        self._lock = threading.Lock()
        self._frames: List[CsiFrame] = []
        self._nodes: Dict[int, NodeInfo] = {}
        self._dropped = 0
        self._start_time: Optional[float] = None

    def start(self) -> None:
        if self._running:
            return
        self._sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self._sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self._sock.bind(("0.0.0.0", self.bind_port))
        self._sock.settimeout(0.5)
        self._running = True
        self._start_time = time.time()
        self._thread = threading.Thread(target=self._recv_loop, daemon=True)
        self._thread.start()

    def stop(self) -> None:
        self._running = False
        if self._thread is not None:
            self._thread.join(timeout=2.0)
        if self._sock is not None:
            self._sock.close()
            self._sock = None

    def _recv_loop(self) -> None:
        assert self._sock is not None
        while self._running:
            try:
                data, addr = self._sock.recvfrom(65535)
            except socket.timeout:
                continue
            except OSError:
                break
            frame = self._decode_datagram(data, addr)
            if frame is None:
                self._dropped += 1
                continue
            now = time.time()
            frame.host_arrival_us = int(now * 1_000_000)
            with self._lock:
                self._frames.append(frame)
                ni = self._nodes.get(frame.node_id)
                if ni is None:
                    self._nodes[frame.node_id] = NodeInfo(
                        node_id=frame.node_id,
                        mac="",
                        ip=addr[0],
                        first_seen=now,
                        last_seen=now,
                        n_frames=1,
                    )
                else:
                    ni.last_seen = now
                    ni.n_frames += 1

    def _decode_datagram(self, data: bytes, addr: Tuple[str, int]) -> Optional[CsiFrame]:
        if len(data) < AQCI_HEADER_SIZE + AQCI_BODY_SIZE:
            return None
        magic = data[0:4]
        if magic != AQCI_MAGIC:
            return None
        wire_major = data[4]
        wire_minor = data[5]
        msg_type = data[6]
        sc_format = data[7]
        node_id = struct.unpack_from("<H", data, 8)[0]
        payload_len = struct.unpack_from("<H", data, 10)[0]
        band = data[12]
        channel = data[13]
        bandwidth_mhz = data[14]
        flags = data[15]
        first_word_invalid = bool(flags & 0x01)

        if msg_type != AQCI_MSG_TYPE_CSI_FRAME:
            return None
        if sc_format != AQCI_SC_FORMAT_INT8_IQ:
            return None

        body = data[AQCI_HEADER_SIZE : AQCI_HEADER_SIZE + AQCI_BODY_SIZE]
        sequence = struct.unpack_from("<Q", body, 0)[0]
        device_time_us = struct.unpack_from("<Q", body, 8)[0]
        boot_epoch = struct.unpack_from("<H", body, 16)[0]
        subcarrier_offset = struct.unpack_from("<h", body, 18)[0]
        n_subcarriers = struct.unpack_from("<H", body, 20)[0]
        rssi_dbm = struct.unpack_from("<b", body, 22)[0]
        noise_floor_dbm = struct.unpack_from("<b", body, 23)[0]
        # phy_flags at 24, reserved at 26

        iq_payload = data[AQCI_HEADER_SIZE + AQCI_BODY_SIZE :]
        expected_iq_bytes = 2 * n_subcarriers
        if len(iq_payload) < expected_iq_bytes:
            return None

        # Derive centre frequency from channel + band (2.4 or 5 GHz).
        centre_freq_hz = self._channel_to_freq(band, channel, bandwidth_mhz)

        # Decode interleaved int8 Q,I (imaginary first) to complex f32.
        csi: List[complex] = []
        for k in range(n_subcarriers):
            q = iq_payload[2 * k]
            i_val = iq_payload[2 * k + 1]
            # Convert signed int8 to f32, divide by IQ_COUNTS_PER_UNIT (~128).
            # Spec: re = i_q[2k+1]/IQ_COUNTS_PER_UNIT, im = i_q[2k]/IQ_COUNTS_PER_UNIT.
            csi.append(
                complex(
                    i_val / 128.0,
                    q / 128.0,
                )
            )

        return CsiFrame(
            node_id=node_id,
            sequence=sequence,
            device_time_us=device_time_us,
            boot_epoch=boot_epoch,
            subcarrier_offset=subcarrier_offset,
            n_subcarriers=n_subcarriers,
            centre_freq_hz=centre_freq_hz,
            bandwidth_mhz=bandwidth_mhz,
            rssi_dbm=rssi_dbm,
            noise_floor_dbm=noise_floor_dbm,
            first_word_invalid=first_word_invalid,
            csi=csi,
            host_arrival_us=0,  # set by caller
        )

    @staticmethod
    def _channel_to_freq(band: int, channel: int, bandwidth_mhz: int) -> int:
        """Map Wi-Fi channel to centre frequency (Hz)."""
        if band == 0:  # 2.4 GHz
            if 1 <= channel <= 13:
                return 2412 + (channel - 1) * 5
            if channel == 14:
                return 2484
        elif band == 1:  # 5 GHz
            # 5 GHz: 5000 + 5*channel for most channels.
            return 5000 + 5 * channel
        return 2_412_000_000

    def get_frames(self, clear: bool = True) -> List[CsiFrame]:
        with self._lock:
            frames = list(self._frames)
            if clear:
                self._frames.clear()
        return frames

    def nodes(self) -> Dict[int, NodeInfo]:
        with self._lock:
            return dict(self._nodes)

    def collect_for(self, seconds: float) -> List[CsiFrame]:
        """Collect frames for `seconds` and return them."""
        deadline = time.time() + seconds
        frames: List[CsiFrame] = []
        while time.time() < deadline:
            remaining = deadline - time.time()
            batch = self.get_frames(clear=True)
            frames.extend(batch)
            if remaining > 0:
                time.sleep(min(0.05, remaining))
        return frames


def frames_by_node(frames: List[CsiFrame]) -> Dict[int, List[CsiFrame]]:
    """Group frames by node id."""
    grouped: Dict[int, List[CsiFrame]] = {}
    for f in frames:
        grouped.setdefault(f.node_id, []).append(f)
    return grouped


def phase_per_subcarrier(frame: CsiFrame) -> List[float]:
    """Measured phase per subcarrier, wrapped to (-pi, pi]."""
    import cmath

    return [cmath.phase(c) for c in frame.csi]


def closure_phase_from_frames(frames_a: List[CsiFrame],
                              frames_b: List[CsiFrame],
                              frames_c: List[CsiFrame],
                              subcarrier_index: int = 32) -> float:
    """Compute Jennison closure phase for loop A->B->C->A at one subcarrier.

    We match frames by nearest device_time_us (the links are directed
    transmissions, so frame timestamps differ). Each link contributes its
    measured subcarrier phase.
    """
    import cmath

    def nearest(frames: List[CsiFrame], target_us: int) -> Optional[CsiFrame]:
        if not frames:
            return None
        return min(frames, key=lambda f: abs(f.device_time_us - target_us))

    if not frames_a or not frames_b or not frames_c:
        return 0.0

    # Use the A->B frame as the anchor.
    anchor = frames_a[0]
    fb = nearest(frames_b, anchor.device_time_us)
    fc = nearest(frames_c, anchor.device_time_us)
    if fb is None or fc is None:
        return 0.0

    # Link phases at the chosen subcarrier.
    phi_ab = cmath.phase(frames_a[subcarrier_index % len(frames_a.csi)].csi
                         [subcarrier_index % len(frames_a.csi)]) if frames_a else 0.0
    # Simpler: take phase of the specific subcarrier from each frame.
    sa = frames_a[0].csi[subcarrier_index % len(frames_a[0].csi)]
    sb = fb.csi[subcarrier_index % len(fb.csi)]
    sc_ = fc.csi[subcarrier_index % len(fc.csi)]
    phase_sum = cmath.phase(sa) + cmath.phase(sb) + cmath.phase(sc_)
    # Wrap.
    while phase_sum > 3.14159:
        phase_sum -= 6.28318
    while phase_sum < -3.14159:
        phase_sum += 6.28318
    return phase_sum


def discover_nodes(timeout_s: float = 5.0) -> Dict[int, NodeInfo]:
    """Briefly listen and return discovered nodes."""
    collector = CsiCollector()
    collector.start()
    try:
        time.sleep(timeout_s)
    finally:
        collector.stop()
    return collector.nodes()


if __name__ == "__main__":
    import sys

    print("Aquila v2 CSI hardware interface (no hardware connected).")
    print("Run on the core host with ESP32-S3 nodes streaming to port 9100.")
    nodes = discover_nodes(timeout_s=2.0)
    if not nodes:
        print("No nodes discovered (expected without hardware).")
    else:
        for nid, ni in nodes.items():
            print(f"  node {nid}: {ni.ip}, {ni.n_frames} frames")
    sys.exit(0)
