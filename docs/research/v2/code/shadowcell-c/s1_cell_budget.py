#!/usr/bin/env python
"""Round-2 T2 (character `c`): ShadowCell — the self-shadowed quasi-monostatic
ESP32-S3 puck cell. Part 1: the self-interference (SI) gate.

SIMULATION ONLY (NumPy model of the estimator and channel; not a measurement).
All parameters are printed below. Numbers feed 23-shadowcell-c.md.

Model summary
  * One 20-face puck (circumradius 0.15 m). One face transmits an ESP-NOW HT20
    frame; the other 19 faces receive simultaneously (different chips, so no
    half-duplex violation). Total TX-to-RX-face isolation I is a parameter.
  * Per-face residual CSI after static background subtraction:
        r_j(k,t) = a_j e^{-j2pi k df tau_j} e^{j2pi f_D t}      (target echo)
                 + u_j * dphi_j(t)                              (leakage residual)
                 + n_j(k,t)                                     (thermal+est+ADC)
    in amplitude units normalised to |h_SI| = 1 (the static intra-puck direct
    path). dphi_j(t) is the post-subtraction relative-LO wander (OU process,
    per face independent; common-mode fraction kappa tested).
  * Processing: per-face delay matched filter at the true delay, Hann-windowed
    Doppler matched filter at the true f_D, coherent face sum with true
    steering (known-hypothesis detection SNR; search losses flagged in the note).
Outputs: printed tables, s1_cell_budget.csv, s1_cell.png.

Run:  /home/matt/projects/aquila-v2-spec/sim-python s1_cell_budget.py
"""
import csv
import numpy as np

rng = np.random.default_rng(20261016)

# ---------------------------------------------------------------- parameters
C = 2.99792458e8
FC = 2.437e9                    # Wi-Fi channel 6
LAM = C / FC                    # 0.1229 m
PT_DBM = 19.5                   # ESP32-S3 conducted TX power (datasheet class)
G_DBI = 2.0                     # face patch gain toward the target (flagged)
NF_DB = 9.0                     # receiver noise figure
B_HZ = 20e6                     # HT20
NS = 56                         # ESP32 HT20 CSI subcarriers
DF = 312.5e3                    # subcarrier spacing
N_F = 100                       # frames per 1 s dwell (100 Hz per-puck rate)
FS = 100.0                      # frame rate, Hz
TDWELL = N_F / FS               # 1.0 s
TAU_C = 0.5                     # OU correlation time of relative wander [s]
T_BASE = 2.0                    # background EMA time constant [s]
KAPPA = 0.0                     # common-mode fraction of wander (0 tested, 0.5 sens.)
SIGMA_Q = 2.0**-8 / np.sqrt(12) # 8-bit CSI quantisation, relative amplitude
RCS = [0.03, 0.1, 0.3, 1.0, 3.9]  # m^2; 3.9 = 20 cm trihedral (note d)
ISOL_DB = [24, 30, 35, 40, 45, 55, 65]
SIG_PHI = [0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2]
RANGES = [1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 10.0]
FD_WALK = 2 * 1.0 / LAM         # 1 m/s radial -> 16.3 Hz monostatic Doppler
SNR_DET_DB = 13.0               # detection threshold (design margin)

print("parameters:")
for k, v in [("fc", f"{FC/1e9:g} GHz"), ("lambda", f"{LAM:.4f} m"),
             ("Pt", f"{PT_DBM} dBm"), ("G", f"{G_DBI} dBi (each way)"),
             ("NF", f"{NF_DB} dB"), ("B", "20 MHz"), ("Ns", NS),
             ("frames/dwell", f"{N_F} @ {FS:g} Hz (T={TDWELL:g} s)"),
             ("OU tau_c", f"{TAU_C:g} s"), ("baseline EMA", f"{T_BASE:g} s"),
             ("kappa (common wander)", f"{KAPPA:g}"),
             ("f_D walk@1m/s", f"{FD_WALK:.1f} Hz")]:
    print(f"  {k:26s} {v}")

N0_TOT_DBM = -174 + 10 * np.log10(B_HZ) + NF_DB        # -92 dBm in 20 MHz
N0_SUB_DBM = N0_TOT_DBM - 10 * np.log10(NS)            # per-subcarrier noise

# icosahedron face centres = dodecahedron vertices (all norm sqrt(3)), R=0.15 m
PHI = (1 + np.sqrt(5)) / 2
verts = []
for s1 in (+1, -1):
    for s2 in (+1, -1):
        for s3 in (+1, -1):
            verts.append((s1, s2, s3))
for a in (+1, -1):
    for b in (+1, -1):
        verts.append((0, a / PHI, b * PHI))
        verts.append((a / PHI, b * PHI, 0))
        verts.append((a * PHI, 0, b / PHI))
faces = np.array(verts, dtype=float)
faces = faces / np.linalg.norm(faces[0]) * 0.15        # circumradius 0.15 m
assert faces.shape == (20, 3)

# ------------------------------------------------------- Part A: echo power
def echo_power_dbm(d0, dj, rcs):
    """Quasi-bistatic radar equation, dBm (bistatic ~ monostatic here)."""
    return (PT_DBM + 2 * G_DBI + 20 * np.log10(LAM) + 10 * np.log10(rcs)
            - 30 * np.log10(4 * np.pi) - 20 * np.log10(d0) - 20 * np.log10(dj))

ISOL_REF_DB = 45.0  # reference isolation (for the Part A amplitude-ratio table)

def alpha_j(d, rcs, tx_face=0, isol_db=None):
    """Per-face echo amplitude relative to |h_SI|=1 for target at range d along +x."""
    tgt = np.array([d, 0.0, 0.0])
    d0 = np.linalg.norm(tgt - faces[tx_face])
    dj = np.linalg.norm(tgt - faces, axis=1)
    pe = echo_power_dbm(d0, dj, rcs)
    psi = PT_DBM - (ISOL_REF_DB if isol_db is None else isol_db)
    return 10 ** ((pe - psi) / 20.0), pe, dj

print("\n== Part A: target echo power and per-subcarrier SNR (thermal only) ==")
print("d[m]  Pe[dBm] rcs=0.3   SNRsub[dB]  |  Pe[dBm] rcs=3.9  SNRsub[dB]")
rowsA = []
for d in RANGES:
    _, pe03, _ = alpha_j(d, 0.3)
    _, pe39, _ = alpha_j(d, 3.9)
    pe03m, pe39m = float(np.mean(pe03)), float(np.mean(pe39))
    snr03 = pe03m - 10 * np.log10(NS) - N0_SUB_DBM
    snr39 = pe39m - 10 * np.log10(NS) - N0_SUB_DBM
    rowsA.append((d, pe03m, snr03, pe39m, snr39))
    print(f"{d:4.1f}  {pe03m:8.1f}  {snr03:8.1f}     |  {pe39m:8.1f}  {snr39:8.1f}")

# --------------------------------------------------- Part B: SI vs isolation
print("\n== Part B: self-interference vs isolation; linearity gate (SI<=-20 dBm) ==")
print("I[dB]  SI[dBm]  linear?  est-SNR[dB]  ADCq rel.   (est-SNR = SI_sub - noise_sub)")
rowsB = []
for I in ISOL_DB:
    si = PT_DBM - I
    si_sub = si - 10 * np.log10(NS)
    est = si_sub - N0_SUB_DBM
    print(f"{I:5d}  {si:7.1f}  {'OK' if si <= -20 else 'DEGRADED':8s} {est:8.1f}   {SIGMA_Q:.2e}")
    rowsB.append((I, si, est))

# ---------------------------------------- OU helper: post-subtraction wander
def ou_residual(n_real, n_face, n_t, dt, tau_c, t_base, sigma_target, rng):
    """OU wander after EMA background subtraction; rescaled so the residual
    std equals sigma_target. Returns array (n_real, n_face, n_t)."""
    e = np.exp(-dt / tau_c)
    sd = np.sqrt(1.0 - e * e)
    f = np.zeros((n_real, n_face))
    th = np.zeros((n_real, n_face, n_t))
    ema = np.zeros((n_real, n_face))
    k_e = dt / t_base
    for t in range(n_t):
        f = e * f + sd * rng.standard_normal((n_real, n_face))
        ema = ema + k_e * (f - ema)
        th[:, :, t] = f - ema
    # rescale: burn-in handled by caller; use last 30% for std estimate
    tail = th[:, :, int(0.7 * n_t):]
    gain = sigma_target / np.std(tail)
    return th * gain, gain

DT_OU = 0.05          # 20 Hz enough for <1 Hz-scale wander
WARM = int(200.0 / DT_OU)   # 200 s warm-up so the EMA reaches steady state

def doppler_hann(n):
    w = np.hanning(n + 2)[1:-1]     # non-zero endpoints avoided
    return w

H = doppler_hann(N_F)
H1 = H.sum()                      # coherent echo gain over the dwell
H2 = (H**2).sum()
print(f"\nDoppler MF: sum(hann)={H1:.1f}, sum(hann^2)={H2:.2f} -> "
      f"frame gain {10*np.log10(H1**2/H2):.1f} dB over {N_F} frames")

def leak_proj_std(f_d, sigma_target, n_real=2000):
    """Std of the leakage time-series after the Doppler MF, per unit wander,
    for a smooth OU residual with the requested post-subtraction std."""
    th, _ = ou_residual(n_real, 1, WARM + N_F, DT_OU, TAU_C, T_BASE, sigma_target, rng)
    seg = th[:, 0, -N_F:]                          # (n_real, N_F)
    t = np.arange(N_F) / FS
    proj = (seg * H * np.exp(-2j * np.pi * f_d * t)).sum(axis=1)
    return np.std(np.abs(proj) + 0j) if False else np.std(proj)

print("\n== Part C0: leakage Doppler projection (std of Hann-weighted sum) ==")
LEAK = {}
for f_d, tag in [(0.0, "static"), (FD_WALK, "walker")]:
    s = leak_proj_std(f_d, 1.0)
    LEAK[tag] = s
    print(f"  f_D={f_d:5.1f} Hz ({tag:6s}): |proj| std = {s:.2f}  (vs echo gain {H1:.1f})")

def rho_leak(d):
    """Flat leakage through the delay matched filter at the target delay."""
    tau = 2 * d / C
    k = np.arange(NS) - (NS // 2)
    return np.abs(np.exp(-2j * np.pi * k * DF * tau).sum()) / NS

print("\ndelay-MF leakage projection rho_l(d):")
for d in RANGES:
    print(f"  d={d:4.1f} m  rho_l={rho_leak(d):.3f}")

# --------------------------------------------- Part C: detection SNR (grid)
def snr_out_analytic(d, rcs, I, sig_phi, f_d, n_face=19, kappa=KAPPA,
                     margin_db=0.0):
    a, pe, _ = alpha_j(d, rcs, isol_db=I)
    a = np.delete(a, 0)[:n_face]           # exclude the TX face
    psi_sub = (PT_DBM - I) - 10 * np.log10(NS)
    n_sub = 10 ** ((N0_SUB_DBM - psi_sub) / 20)      # thermal+est rel. amp
    n_eff = np.sqrt(n_sub**2 + SIGMA_Q**2)
    rl = rho_leak(d)
    tag = "static" if f_d == 0 else "walker"
    sig_echo = H1 * a.sum()
    # independent part of the wander averages over faces; common part does not
    var_ind = n_face * rl**2 * (1 - kappa) * (sig_phi * LEAK[tag]) ** 2
    var_com = (n_face * rl) ** 2 * kappa * (sig_phi * LEAK[tag]) ** 2
    var_th = n_face * n_eff**2 / NS * H2
    # the echo's own phase wander (same relative-LO process multiplies the
    # whole frame): multiplicative noise, sets an SNR ceiling ~ (H1/LEAK)^2
    var_self = (a.sum() * sig_phi * LEAK[tag]) ** 2
    snr = sig_echo**2 / (var_ind + var_com + var_th + var_self)
    return 10 * np.log10(snr) - margin_db

def snr_out_mc(d, rcs, I, sig_phi, f_d, n_real=400, n_face=19, kappa=KAPPA):
    """Monte-Carlo validation of the analytic model at the scalar
    post-delay-matched-filter layer (the subcarrier-level cross-check is
    snr_out_mc_full below)."""
    a, _, dj = alpha_j(d, rcs)
    a = np.delete(a, 0)[:n_face]
    tau = (d + dj) / C                      # per-face bistatic delays (TX face 0)
    tau = np.delete(tau, 0)[:n_face]
    psi_sub = (PT_DBM - I) - 10 * np.log10(NS)
    n_sub = 10 ** ((N0_SUB_DBM - psi_sub) / 20)
    n_eff = np.sqrt(n_sub**2 + SIGMA_Q**2)
    t = np.arange(N_F) / FS

    th, _ = ou_residual(n_real, n_face, WARM + N_F, DT_OU, TAU_C, T_BASE,
                        sig_phi, rng)
    dphi = th[:, :, -N_F:]                          # (real, face, frame)
    if kappa > 0:
        common = th[:, :1, -N_F:]
        dphi = np.sqrt(1 - kappa) * dphi + np.sqrt(kappa) * np.broadcast_to(
            common, dphi.shape)
    u = np.exp(1j * rng.uniform(0, 2 * np.pi, (1, n_face, 1)))  # leakage phase
    ph_steering = np.exp(-2j * np.pi * FC * tau)[None, :, None]
    ph_dop = np.exp(2j * np.pi * f_d * t)[None, None, :]

    def run(echo_on):
        # scalar-layer path (validated against the full-subcarrier path once):
        z = (echo_on * a[None, :, None] * ph_steering * ph_dop) \
            + (u * dphi * rho_leak(d)) \
            + (n_eff / np.sqrt(NS)) * (rng.standard_normal(dphi.shape)
                                       + 1j * rng.standard_normal(dphi.shape))
        S = (z * H[None, None, :] * np.exp(-2j * np.pi * f_d * t)[None, None, :]
             ).sum(axis=2)                       # Doppler MF -> (real, face)
        S = (S * np.conj(ph_steering[:, :, 0])).sum(axis=1)
        return S

    S1 = run(True)
    S0 = run(False)
    return 10 * np.log10(np.abs(S1.mean())**2 / S0.var())

# full-subcarrier validation at one design point (slower, fewer realisations)
def snr_out_mc_full(d, rcs, I, sig_phi, f_d, n_real=40, n_face=19):
    a, _, dj = alpha_j(d, rcs)
    a = np.delete(a, 0)[:n_face]
    tau = (d + dj) / C
    tau = np.delete(tau, 0)[:n_face]
    k = np.arange(NS) - (NS // 2)
    psi_sub = (PT_DBM - I) - 10 * np.log10(NS)
    n_sub = 10 ** ((N0_SUB_DBM - psi_sub) / 20)
    n_eff = np.sqrt(n_sub**2 + SIGMA_Q**2)
    t = np.arange(N_F) / FS
    th, _ = ou_residual(n_real, n_face, WARM + N_F, DT_OU, TAU_C, T_BASE,
                        sig_phi, rng)
    dphi = th[:, :, -N_F:]
    u = np.exp(1j * rng.uniform(0, 2 * np.pi, (1, n_face, 1)))
    # subcarrier-phase matrices
    Psub = np.exp(-2j * np.pi * DF * np.outer(tau, k))          # (face, k)
    Psteer = np.exp(-2j * np.pi * FC * tau)                     # (face,)
    out_e, out_0 = [], []
    for echo_on in (True, False):
        acc = []
        for r in range(n_real):
            n = n_eff / np.sqrt(2) * (rng.standard_normal((n_face, NS, N_F))
                + 1j * rng.standard_normal((n_face, NS, N_F)))
            leak = u[0, :, 0][:, None, None] * dphi[r][:, None, :] * np.ones((1, NS, 1))
            ech = (a[:, None, None] * Psub[:, :, None]
                   * np.exp(2j * np.pi * f_d * t)[None, None, :]
                   * Psteer[:, None, None]) if echo_on else 0
            rcs_mat = ech + leak + n
            z = (rcs_mat * np.conj(Psub)[:, :, None]).mean(axis=1)  # delay MF
            S = (z * H[None, :] * np.exp(-2j * np.pi * f_d * t)[None, :]
                 ).sum(axis=1)
            acc.append((S * np.conj(Psteer)).sum())
        arr = np.array(acc)
        (out_e if echo_on else out_0).append(arr)
    S1, S0 = out_e[0], out_0[0]
    return 10 * np.log10(np.abs(S1.mean())**2 / S0.var())

print("\n== Part C: detection SNR_out [dB] (known-hypothesis; threshold %.0f dB) =="
      % SNR_DET_DB)
design = (4.0, 0.3, 45.0)
print(f"design point: d={design[0]} m, rcs={design[1]} m2, I={design[2]} dB")
grid_static, grid_walk = {}, {}
for I in [30, 35, 40, 45, 55]:
    for sp in SIG_PHI:
        grid_static[(I, sp)] = snr_out_analytic(design[0], design[1], I, sp, 0.0)
        grid_walk[(I, sp)] = snr_out_analytic(design[0], design[1], I, sp, FD_WALK)
print(" I[dB] sig_phi  SNR_static  SNR_walker")
for (I, sp) in sorted(grid_static):
    print(f"{I:5d}  {sp:6.3f}  {grid_static[(I, sp)]:9.1f}  {grid_walk[(I, sp)]:9.1f}")

print("\nMC cross-checks (scalar-layer and full-subcarrier) at design point:")
for sp in [0.02, 0.05, 0.1]:
    mc = snr_out_mc(design[0], design[1], design[2], sp, 0.0)
    print(f"  sig_phi={sp:5.2f}: analytic {snr_out_analytic(design[0], design[1], design[2], sp, 0.0):6.1f} dB"
          f"   MC {mc:6.1f} dB")
mc_full = snr_out_mc_full(design[0], design[1], design[2], 0.05, 0.0)
print(f"  full-subcarrier MC (sig_phi=0.05, static): {mc_full:.1f} dB vs "
      f"analytic {snr_out_analytic(design[0], design[1], design[2], 0.05, 0.0):.1f} dB")

print("\nkappa sensitivity (common-mode wander does not average over faces):")
for kap in [0.0, 0.5, 1.0]:
    print(f"  kappa={kap:3.1f}: SNR_static(d=4, rcs=0.3, I=45, sp=0.02) = "
          f"{snr_out_analytic(4, 0.3, 45, 0.02, 0.0, kappa=kap):.1f} dB")

print("\ntau_c sensitivity of the walker-regime leakage projection:")
for tc in [0.01, 0.1, 0.5]:
    def _leak(f_d, tc):
        th, _ = ou_residual(2000, 1, WARM + N_F, DT_OU, tc, T_BASE, 1.0, rng)
        seg = th[:, 0, -N_F:]
        t = np.arange(N_F) / FS
        return np.std((seg * H * np.exp(-2j * np.pi * f_d * t)).sum(axis=1))
    s_w = _leak(FD_WALK, tc)
    s_s = _leak(0.0, tc)
    print(f"  tau_c={tc:5.2f} s: LEAK_static={s_s:6.2f}  LEAK_walker={s_w:6.2f}"
          f"  -> walker floor suppression {20*np.log10(s_s/s_w):.0f} dB"
          f"  echo-self ceiling {20*np.log10(H1/s_w):.0f} dB @ sp=1")

# ---------------------------------- Part D: min detectable RCS and max range
print("\n== Part D: min detectable RCS [m2] at SNR=13 dB (static, binding regime) ==")
print(" d[m] | " + " | ".join(f"sp={sp:g}" for sp in [0.005, 0.02, 0.05, 0.1]))
rowsD = []
for d in RANGES:
    row = []
    for sp in [0.005, 0.02, 0.05, 0.1]:
        lo, hi = 1e-4, 100.0
        if snr_out_analytic(d, lo, 45.0, sp, 0.0) > SNR_DET_DB:
            row.append(lo)
            continue
        for _ in range(60):
            mid = np.sqrt(lo * hi)
            if snr_out_analytic(d, mid, 45.0, sp, 0.0) < SNR_DET_DB:
                lo = mid
            else:
                hi = mid
        row.append(np.sqrt(lo * hi))
    rowsD.append((d, row))
    print(f" {d:4.1f} | " + " | ".join(f"{v:9.4f}" for v in row))

print("\nmax detection range [m] (walker, SNR=13 dB; ideal vs -10 dB realism margin):")
for rcs in [0.1, 0.3, 1.0]:
    row = []
    for sp in [0.02, 0.05, 0.1]:
        lo_i, hi_i = 0.5, 30.0
        lo_m, hi_m = 0.5, 30.0
        for _ in range(50):
            mid = 0.5 * (lo_i + hi_i)
            if snr_out_analytic(mid, rcs, 45.0, sp, FD_WALK) > SNR_DET_DB:
                lo_i = mid
            else:
                hi_i = mid
            mid = 0.5 * (lo_m + hi_m)
            if snr_out_analytic(mid, rcs, 45.0, sp, FD_WALK, margin_db=10.0) > SNR_DET_DB:
                lo_m = mid
            else:
                hi_m = mid
        row.append((lo_i, lo_m))
    print(f"  rcs={rcs:4.2f} m2: " + "  ".join(
        f"sp={sp:g}: {vi:4.1f}/{vm:4.1f} m" for sp, (vi, vm) in zip([0.02, 0.05, 0.1], row)))

# ------------------------------------------ Part E: the killing parameter lines
print("\n== Part E: killing lines (single-parameter, others at design values) ==")
for rcs in [0.1, 0.3]:
    lo, hi = 20.0, 80.0
    for _ in range(50):
        mid = 0.5 * (lo + hi)
        if snr_out_analytic(4.0, rcs, mid, 0.02, 0.0) > SNR_DET_DB:
            hi = mid
        else:
            lo = mid
    print(f"  rcs={rcs}: I_min(d=4 m, sp=0.02, static) = {0.5*(lo+hi):.0f} dB")
    lo, hi = 1e-3, 1.0
    for _ in range(60):
        mid = np.sqrt(lo * hi)
        if snr_out_analytic(4.0, rcs, 45.0, mid, 0.0) > SNR_DET_DB:
            lo = mid
        else:
            hi = mid
    print(f"  rcs={rcs}: sig_phi_max(d=4 m, I=45 dB, static) = {np.sqrt(lo*hi):.3f} rad")
    lo, hi = 1e-3, 1.0
    for _ in range(60):
        mid = np.sqrt(lo * hi)
        if snr_out_analytic(6.0, rcs, 45.0, mid, FD_WALK) > SNR_DET_DB:
            lo = mid
        else:
            hi = mid
    print(f"  rcs={rcs}: sig_phi_max(d=6 m, I=45 dB, walker) = {np.sqrt(lo*hi):.3f} rad")

# ---------------------------------------------------------------- write CSV
with open("s1_cell_budget.csv", "w", newline="") as fh:
    w = csv.writer(fh)
    w.writerow(["part", "params", "value", "units"])
    for d, pe, s, pe9, s9 in rowsA:
        w.writerow(["A_echo", f"d={d:g},rcs=0.3", f"{pe:.2f}", "dBm"])
        w.writerow(["A_echo", f"d={d:g},rcs=0.3", f"{s:.2f}", "dB_sub"])
        w.writerow(["A_echo", f"d={d:g},rcs=3.9", f"{pe9:.2f}", "dBm"])
        w.writerow(["A_echo", f"d={d:g},rcs=3.9", f"{s9:.2f}", "dB_sub"])
    for I, si, est in rowsB:
        w.writerow(["B_SI", f"I={I}", f"{si:.2f}", "dBm"])
    for (I, sp), v in sorted(grid_static.items()):
        w.writerow(["C_snr", f"I={I},sp={sp:g},static", f"{v:.2f}", "dB"])
        w.writerow(["C_snr", f"I={I},sp={sp:g},walker", f"{grid_walk[(I, sp)]:.2f}", "dB"])
    for d, row in rowsD:
        for sp, v in zip([0.005, 0.02, 0.05, 0.1], row):
            w.writerow(["D_minrcs", f"d={d:g},sp={sp:g}", f"{v:.5f}", "m2"])

try:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    fig, ax = plt.subplots(1, 3, figsize=(11, 3.4))
    dd = [d for d, _ in rowsD]
    for i, sp in enumerate([0.005, 0.02, 0.05, 0.1]):
        ax[0].loglog(dd, [r[i] for r in [row for _, row in rowsD]], marker="o", ms=3,
                     label=f"sp={sp:g} rad")
    ax[0].set_xlabel("range [m]"); ax[0].set_ylabel("min detectable RCS [m2]")
    ax[0].axhline(0.1, color="k", ls="--", lw=0.8, label="small adult, literature")
    ax[0].set_title("D. min RCS @13 dB (static, I=45 dB)")
    ax[0].legend(fontsize=7); ax[0].grid(True, which="both", alpha=0.3)
    Is = [30, 35, 40, 45, 55]
    for sp in [0.01, 0.05, 0.1]:
        ax[1].plot(Is, [grid_static[(I, sp)] for I in Is], marker="s", ms=3, label=f"static sp={sp:g}")
        ax[1].plot(Is, [grid_walk[(I, sp)] for I in Is], marker="^", ms=3, ls="--",
                   label=f"walker sp={sp:g}")
    ax[1].axhline(SNR_DET_DB, color="k", ls="--", lw=0.8)
    ax[1].set_xlabel("isolation I [dB]"); ax[1].set_ylabel("SNR_out [dB]")
    ax[1].set_title("C. detection SNR (d=4 m, rcs=0.3)")
    ax[1].legend(fontsize=6); ax[1].grid(alpha=0.3)
    for rcs in [0.1, 0.3, 3.9]:
        dd2 = np.linspace(1, 10, 40)
        ax[2].semilogy(dd2, [10 ** ((echo_power_dbm(d, d, rcs) - (PT_DBM - 45)) / 20)
                             * 1 for d in dd2], label=f"rcs={rcs}")
    ax[2].set_xlabel("range [m]"); ax[2].set_ylabel("|echo| / |SI| at I=45 dB")
    ax[2].set_title("A. echo-to-SI amplitude ratio")
    ax[2].legend(fontsize=7); ax[2].grid(alpha=0.3)
    fig.tight_layout()
    fig.savefig("s1_cell.png", dpi=110)
    print("\nwrote s1_cell_budget.csv and s1_cell.png")
except Exception as exc:  # noqa: BLE001
    print("figure skipped:", exc)
