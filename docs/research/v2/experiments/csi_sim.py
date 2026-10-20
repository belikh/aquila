"""Pure-Python CSI simulator for closure-phase invention testing.

Models a mesh of ESP32-S3-style nodes, each with an independent local
oscillator and free-running clock. Generates synthetic per-subcarrier CSI
for directed transmitter->receiver links, including:

  * per-node LO phase (random, time-varying via clock drift)
  * per-link static multipath (empty-home baseline)
  * moving-scatterer path-length perturbations (breathing-scale + gross motion)
  * thermal noise + CFO/sampling offsets

No numpy: stdlib only (math, cmath, random). Runs on any Python 3.

This is the common ground-truth generator used by every invention test
(closure phase, reciprocal gauge, ambient ledger, clockless anchor).
"""

from __future__ import annotations

import cmath
import math
import random
from dataclasses import dataclass, field
from typing import Dict, List, Tuple

SPEED_OF_LIGHT = 299_792_458.0  # m/s


@dataclass
class Node:
    """One fixed Wi-Fi sensing node with an independent LO + clock."""

    node_id: str
    position: Tuple[float, float, float]  # (x, y, z) metres
    lo_freq_hz: float  # nominal LO frequency (independent per node)
    clock_drift_ppm: float  # fractional frequency offset vs nominal
    phase_noise_std_rad: float  # per-frame LO phase jitter (radians)

    # Internal state (advanced each frame).
    _lo_phase: float = field(init=False, default=0.0)
    _clock_offset_s: float = field(init=False, default=0.0)

    def __post_init__(self) -> None:
        self._lo_phase = random.uniform(0.0, 2.0 * math.pi)
        self._clock_offset_s = 0.0

    def advance(self, dt_s: float) -> None:
        """Advance LO phase and clock by dt_s seconds."""
        # LO rotates at lo_freq + drift.
        self._lo_phase += 2.0 * math.pi * self.lo_freq_hz * dt_s
        # Wrap to [-pi, pi] using modulo (fast; no iterative subtraction).
        self._lo_phase = math.fmod(self._lo_phase, 2.0 * math.pi)
        if self._lo_phase > math.pi:
            self._lo_phase -= 2.0 * math.pi
        elif self._lo_phase < -math.pi:
            self._lo_phase += 2.0 * math.pi
        self._clock_offset_s += self.clock_drift_ppm * 1e-6 * dt_s

    def lo_phase_rad(self) -> float:
        return self._lo_phase


@dataclass
class Scatterer:
    """A passive reflector (person, limb, object) that perturbs paths."""

    scatterer_id: str
    position: Tuple[float, float, float]  # metres
    # Motion model: p(t) = p0 + A*sin(2*pi*f*t + phi) along an axis.
    axis: Tuple[float, float, float]  # unit direction of motion
    amplitude_m: float  # peak displacement (metres)
    freq_hz: float  # motion frequency (Hz)
    phase_rad: float  # initial phase
    # Radar cross-section proxy: how strongly it perturbs each link (0..1).
    strength: float = 0.5

    def position_at(self, t_s: float) -> Tuple[float, float, float]:
        disp = self.amplitude_m * math.sin(2.0 * math.pi * self.freq_hz * t_s + self.phase_rad)
        ax, ay, az = self.axis
        px, py, pz = self.position
        return (px + ax * disp, py + ay * disp, pz + az * disp)


@dataclass
class LinkSpec:
    """A directed transmitter->receiver link on one subcarrier grid."""

    tx_node_id: str
    rx_node_id: str
    centre_freq_hz: float
    bandwidth_hz: float
    n_subcarriers: int
    # Static multipath: list of (amplitude, path_length_m) for empty-home baseline.
    static_paths: List[Tuple[float, float]] = field(default_factory=list)


def subcarrier_frequencies(centre_hz: float, bandwidth_hz: float, n: int) -> List[float]:
    """Return OFDM subcarrier frequencies (Hz) centred on centre_hz."""
    return [centre_hz - bandwidth_hz / 2.0 + (k + 0.5) * bandwidth_hz / n for k in range(n)]


def path_length_m(a: Tuple[float, float, float], b: Tuple[float, float, float]) -> float:
    """Euclidean distance between two 3D points."""
    dx = a[0] - b[0]
    dy = a[1] - b[1]
    dz = a[2] - b[2]
    return math.sqrt(dx * dx + dy * dy + dz * dz)


def measure_csi(
    nodes: Dict[str, Node],
    spec: LinkSpec,
    scatterers: List[Scatterer],
    f_k: float,
    t_s: float,
    noise_std: float = 0.05,
    cfo_hz: float = 0.0,
    sampling_offset_s: float = 0.0,
    include_lo: bool = True,
) -> complex:
    """Synthesise one complex CSI coefficient H[k] for link tx->rx at f_k, time t.

    Model:
        H[k] = sum_p  a_p * exp(-j*2*pi*f_k*tau_p) * exp(j*(theta_T - theta_R))
             + noise
    where tau_p = path_length_p / c for each static + scatterer path.

    The LO term exp(j*(theta_T - theta_R)) is the per-node phase that closure
    phase is designed to cancel.
    """
    tx = nodes[spec.tx_node_id]
    rx = nodes[spec.rx_node_id]

    # Build the set of paths: static multipath + one path per scatterer
    # (tx -> scatterer -> rx two-bounce model).
    paths: List[Tuple[float, float]] = list(spec.static_paths)  # (amplitude, length_m)
    tx_pos = tx.position
    rx_pos = rx.position
    for sc in scatterers:
        sp = sc.position_at(t_s)
        # Two-bounce: tx -> scatterer -> rx.
        length = path_length_m(tx_pos, sp) + path_length_m(sp, rx_pos)
        # Amplitude falls with total path length; scaled by scatterer strength.
        amp = sc.strength / (1.0 + length / 3.0)
        paths.append((amp, length))

    # Coherent sum over paths.
    total = 0.0 + 0.0j
    for amp, length in paths:
        tau = length / SPEED_OF_LIGHT
        # Propagation phase at this subcarrier.
        prop_phase = -2.0 * math.pi * f_k * tau
        # Add CFO-induced rotation (common to all paths on this link).
        cfo_phase = -2.0 * math.pi * cfo_hz * t_s
        # Sampling-time offset adds a linear-in-f phase ramp.
        sto_phase = -2.0 * math.pi * f_k * sampling_offset_s
        total += amp * cmath.exp(1j * (prop_phase + cfo_phase + sto_phase))

    # Per-node LO phase: transmitter and receiver each contribute.
    # When include_lo is False, the LO terms are removed — this isolates the
    # propagation/scatterer component and is what closure phase is designed to
    # cancel anyway (LO terms telescope to zero around a directed loop).
    if include_lo:
        lo_phase = tx.lo_phase_rad() - rx.lo_phase_rad()
        total *= cmath.exp(1j * lo_phase)

    # Thermal noise (complex, circular).
    if noise_std > 0.0:
        total += complex(
            random.gauss(0.0, noise_std),
            random.gauss(0.0, noise_std),
        )

    return total


def link_phase_rad(h: complex) -> float:
    """Measured phase of a complex CSI coefficient, wrapped to (-pi, pi]."""
    return cmath.phase(h)  # returns (-pi, pi]


def wrap_to_pm_pi(phase_rad: float) -> float:
    while phase_rad > math.pi:
        phase_rad -= 2.0 * math.pi
    while phase_rad < -math.pi:
        phase_rad += 2.0 * math.pi
    return phase_rad
