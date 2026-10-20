#!/usr/bin/env python3
"""
T1 — THE COHERENCE GATE, SIMULATED.
Round-2 research artefact for docs/research/v2/23-coherence-gate-h.md (Aquila v2).

Run:
    /home/matt/projects/aquila-v2-spec/sim-python docs/research/v2/code/t1-coherence-gate-h.py

Everything here is a SIMULATION/ANALYTIC DERIVATION, not a measurement
(EXPERIMENT-ENV.md honesty rules). Parameters are drawn from the round-1 note
set with the provenance stated in the note; the ESP32-S3-specific values
(sigma_rho, reply jitter, tau_nu) are UNMEASURED and are swept.

Outputs (small, per the disk-discipline rule):
    docs/research/v2/figures/t1-h-sigma-vs-gap.png
    docs/research/v2/figures/t1-h-chain.png
    docs/research/v2/code/t1-coherence-gate-h-results.csv
    stdout: markdown tables embedded in the note.
"""
from __future__ import annotations

import csv
import math
import os

import numpy as np

RNG_SEED = 20261016

HERE = os.path.dirname(os.path.abspath(__file__))
FIGDIR = os.path.abspath(os.path.join(HERE, "..", "figures"))
os.makedirs(FIGDIR, exist_ok=True)

C = 299_792_458.0          # m/s
FC = 2.437e9               # Hz, mid-band 2.4 GHz
LAM = C / FC               # ~12.29 cm

# Parameter provenance (full table in the note):
#   raw CFO: note 17 (2 ppm -> 4.8 kHz; measured S3 setup 10 ppm -> 24 kHz;
#            20 ppm -> 48.7 kHz)
#   residual CFO 10/100 Hz: note 12 (MegaMIMO measurements)
#   sigma_m: ML-Track 0.04 rad (RPi); ESP-PPB 0.175 rad typical (C3,
#            unrefereed); Intel B2B 0.117 rad
#   sigma_tau: WCNC 2023 RPi datum < 1 us; UNMEASURED on S3
GAPS_MS = [0.25, 0.5, 1, 2, 5, 10, 20, 50, 100]
SIGMA_RHO_A = 0.175        # rad; irreducible per-frame noise, regime A
ROWS: list[dict] = []


def log(section: str, msg: str) -> None:
    print(f"[{section}] {msg}")


# ============================================================================
# PART A — closed-form scalar budgets (analytic; provenance in the note)
# ============================================================================
def part_a() -> dict:
    out: dict = {}
    out["raw_cfo"] = [
        (ppm, ppm * 1e-6 * FC / 1e3, (0.25 / (ppm * 1e-6 * FC)) * 1e6)
        for ppm in (2, 10, 20, 40)
    ]
    # holdover with NO fence: sigma_phi = 2*pi*sigma_nu*T_gap
    out["kill_nu_20ms"] = {thr: thr / (2 * math.pi * 0.020) for thr in (0.5, 0.1)}
    out["holdover"] = {
        s: {g: 2 * math.pi * s * g * 1e-3 for g in GAPS_MS}
        for s in (0.244, 2.44, 10, 25, 100)
    }
    # two-way reply jitter x raw CFO
    out["jitter_kill"] = {
        (d, thr): thr / (2 * math.pi * d * 1e3) * 1e6
        for d in (4.8, 24, 48.7, 97.5) for thr in (0.5, 0.3, 0.1)
    }
    # ns-timestamp quantisation (12.5 ns) x raw CFO
    out["quant_rad"] = {d: 2 * math.pi * d * 1e3 * 12.5e-9 for d in (4.8, 24, 48.7)}
    # CRB: per-frame common phase from 64 subcarriers
    out["crb_frame"] = {db: 1 / math.sqrt(2 * 64 * 10 ** (db / 10)) for db in (5, 10, 20, 30)}
    # CFO fit CRB: Var(nu_hat) = 12 s_m^2 / ((2pi)^2 N Tw^2)
    out["cfo_fit_hz"] = {
        (R, Tw, sm): math.sqrt(12 * sm**2 / ((2 * math.pi) ** 2 * (R * Tw) * Tw**2))
        for R in (50, 200, 1000) for Tw in (0.1, 0.5) for sm in (0.04, 0.175, 0.6)
    }
    out["gain"] = {s: math.exp(-s * s / 2) for s in (0.1, 0.175, 0.3, 0.5, 1.0, 1.5)}
    out["budget_B"] = {(nu, t): nu * t * 1e-3 for nu in (0.244, 2.44, 10, 100) for t in (1, 5, 20, 100)}
    out["tgt_coh_ms"] = {v: LAM / (2 * v) * 1e3 for v in (0.005, 0.1, 0.5, 1.0, 5.0)}
    out["fence_duty"] = {R: R * 250e-6 for R in (50, 200, 1000)}
    return out


# ============================================================================
# PART B — the fence tracker Monte Carlo (core engine)
# ============================================================================
def fence_mc(R_ref, sigma_nu, tau_nu, sigma_m, a_ramp=0.0, p_pert=0.0,
             pert_amp=0.5, gate=True, ramp_state=False, t_end=20.0,
             n_real=200, seed=RNG_SEED):
    """Return {gap_ms: sigma_phi_track} for the fence prediction error.

    Truth: nu(t) = OU(sigma_nu, tau_nu) + deterministic thermal ramp a_ramp
    (Hz/s) starting at 30% of the run; phi = 2pi int nu dt, trapezoid at
    fence steps (integration error O(T/tau), <=2% for tau >= 1 s).
    Fence: z_k = phi_k + sigma_m n_k (+ perturbation on p_pert of frames).
    Tracker: Kalman on [phi, nu] (or [phi, nu, alpha]); datum = prediction
    error of the post-update state to t_k + T_gap.
    """
    r = np.random.default_rng(seed)
    T = 1.0 / R_ref
    K = int(t_end * R_ref)
    n = n_real
    a_ou = math.exp(-T / tau_nu)
    s_ou = sigma_nu * math.sqrt(1 - a_ou**2)

    nu = r.normal(0, sigma_nu, n)
    nu_det = np.zeros(n)
    phi = r.uniform(-math.pi, math.pi, n)

    nx = 3 if ramp_state else 2
    P0 = np.diag([(2 * math.pi * sigma_nu * T) ** 2 + sigma_m**2, sigma_nu**2])
    if ramp_state:
        P0 = np.diag([(2 * math.pi * sigma_nu * T) ** 2 + sigma_m**2,
                      sigma_nu**2, (0.1 * sigma_nu) ** 2])
    P = np.repeat(P0[None], n, axis=0)
    X = np.zeros((n, nx))                       # phase seeded by first fence frame
    seeded = False

    q = s_ou**2
    if ramp_state:
        F = np.array([[1, 2 * math.pi * T, math.pi * T * T],
                      [0, a_ou, T],
                      [0, 0, 1.0]])
        Q = np.zeros((3, 3))
        Q[0, 0] = (math.pi * T) ** 2 * q
        Q[0, 1] = Q[1, 0] = math.pi * T * q
        Q[1, 1] = q
        Q[2, 2] = (0.05 * sigma_nu / max(tau_nu, 1.0)) ** 2 * T
    else:
        F = np.array([[1, 2 * math.pi * T], [0, a_ou]])
        Q = np.array([[(math.pi * T) ** 2 * q, math.pi * T * q],
                      [math.pi * T * q, q]])
    I = np.eye(nx)
    Rm = sigma_m**2
    ramp_on = int(0.3 * K)
    gaps = np.array(GAPS_MS) * 1e-3
    errs = {g: [] for g in GAPS_MS}
    big = {g: [] for g in GAPS_MS}
    gated_frac = []
    acq = 300                                   # ungated acquisition frames
    cons_rej = np.zeros(n, int)
    reseeds = 0

    for k in range(K):
        # ---- truth step over [t_k, t_k+T] (trapezoid) ----
        nu_next = nu * a_ou + s_ou * r.normal(size=n)
        if a_ramp and k >= ramp_on:
            nu_det = nu_det + a_ramp * T
        phi = phi + 2 * math.pi * T * (nu + nu_next) / 2 \
            + 2 * math.pi * T * nu_det
        nu = nu_next
        # ---- fence measurement at t_{k+1} ----
        z = phi + sigma_m * r.normal(size=n)
        if not seeded:
            X[:, 0] = z                        # state now represents t_1
            seeded = True
            continue                           # no predict/update on the seed frame
        if p_pert:
            z = z + (r.random(n) < p_pert) * pert_amp * r.normal(size=n)
        # ---- Kalman predict ----
        Xp = X @ F.T
        Pp = F @ P @ F.T + Q
        S = Pp[:, 0, 0] + Rm
        y = z - Xp[:, 0]
        use_gate = gate and (k > acq)
        keep = np.abs(y) < 3 * np.sqrt(S) if use_gate else np.ones(n, bool)
        gated_frac.append(1 - keep.mean())
        # lockout detector: >25 consecutive rejections -> phase re-lock,
        # keep the frequency estimate, reset the covariance (firmware rule)
        cons_rej = np.where(keep, 0, cons_rej + 1)
        reseed = cons_rej > 25
        Kg = (Pp[:, :, 0] / S[:, None]) * keep[:, None]
        X = Xp + Kg * y[:, None]
        # simple update: P = (I - K H) Pp with H = [1,0,(0)]; K H puts K in COLUMN 0
        KH = np.zeros((n, nx, nx))
        KH[:, :, 0] = Kg
        P = (I[None] - KH) @ Pp
        P = 0.5 * (P + np.transpose(P, (0, 2, 1)))
        # ---- datum: prediction to t_{k+1} + gap ----
        if k > 400:
            nu_now = nu + nu_det
            for gi, gs in enumerate(gaps):
                if ramp_state:
                    pred = X[:, 0] + X[:, 1] * 2 * math.pi * gs \
                        + 0.5 * X[:, 2] * gs * gs
                else:
                    pred = X[:, 0] + X[:, 1] * 2 * math.pi * gs
                truth = phi + 2 * math.pi * (nu_now + 0.5 * a_ramp * gs) * gs
                e = truth - pred
                errs[GAPS_MS[gi]].append(float(np.std(e)))
                big[GAPS_MS[gi]].append(float(np.mean(np.abs(e) > 0.5)))
        X = X.copy(); nu = nu.copy(); phi = phi.copy()
    return ({g: float(np.mean(errs[g])) for g in GAPS_MS},
            {g: float(np.mean(big[g])) for g in GAPS_MS},
            float(np.mean(gated_frac)), reseeds / n)


def part_b():
    runs = [
        # label,              R,   sigma_nu, tau,  sigma_m, a_ramp, 3st, p_pert, gate
        ("f1kHz-tau10s",      1000, 100.0, 10.0, 0.175, 0.0, False, 0.0, True),
        ("f200Hz-tau10s",      200, 100.0, 10.0, 0.175, 0.0, False, 0.0, True),
        ("f50Hz-tau10s",        50, 100.0, 10.0, 0.175, 0.0, False, 0.0, True),
        ("f200Hz-tau1s",       200, 100.0,  1.0, 0.175, 0.0, False, 0.0, True),
        ("f200Hz-tau0p1s",     200, 100.0,  0.1, 0.175, 0.0, False, 0.0, True),
        ("f200Hz-tau100s",     200, 100.0, 100.0, 0.175, 0.0, False, 0.0, True),
        ("f200Hz-sm0p04",      200, 100.0, 10.0, 0.040, 0.0, False, 0.0, True),
        ("f200Hz-sm0p6",       200, 100.0, 10.0, 0.600, 0.0, False, 0.0, True),
        ("f200Hz-ramp8-2st",   200, 100.0, 10.0, 0.175, 8.0, False, 0.0, True),
        ("f200Hz-ramp8-3st",   200, 100.0, 10.0, 0.175, 8.0, True,  0.0, True),
        ("f200Hz-pert-ng",     200, 100.0, 10.0, 0.175, 0.0, False, 0.20, False),
        ("f200Hz-pert-g",      200, 100.0, 10.0, 0.175, 0.0, False, 0.20, True),
    ]
    results = []
    for label, R, snu, tau, sm, a, rst, pp, gate in runs:
        st, bg, gf, rs = fence_mc(R, snu, tau, sm, a_ramp=a, ramp_state=rst,
                                  p_pert=pp, gate=gate,
                                  t_end=10.0 if R == 1000 else 20.0)
        log("B", f"{label:18s} " + " ".join(f"{g}ms:{st[g]:.3f}" for g in (1, 2, 5, 20, 50, 100))
            + f"  P(>0.5rad@5ms)={bg[5]:.3f}"
            + (f"  gated={gf:.3f}" if pp else "")
            + (f"  reseeds/real={rs:.2f}" if rs > 0 else ""))
        for g in GAPS_MS:
            ROWS.append({"run": label, "gap_ms": g, "sigma_track_rad": st[g]})
        results.append((label, st))
    return results


# ============================================================================
# PART C — averaging law: faces N_f and packets K
# ============================================================================
def part_c():
    comps = {"sigma_track": 0.06,   # fence datum, T_gap <= 2.5 ms (part B)
             "sigma_frame": 0.175,  # per-frame irreducible (regime A)
             "sigma_face": 0.30,    # per-face impairment (assumed, worst-ish)
             "sigma_mp": 0.08}      # multipath-bias wander (assumed)
    common2 = comps["sigma_track"]**2 + comps["sigma_mp"]**2 + comps["sigma_frame"]**2
    law, indep = [], []
    for N in range(1, 41):
        v = common2 + comps["sigma_face"]**2 / N
        law.append((N, math.sqrt(v)))
        indep.append(comps["sigma_face"] / math.sqrt(N))
    n_star = comps["sigma_face"]**2 / common2
    klaw = []
    floor2 = comps["sigma_track"]**2 + comps["sigma_mp"]**2
    for K in range(1, 13):
        v = floor2 + comps["sigma_frame"]**2 / K + comps["sigma_face"]**2 / (4 * K)
        klaw.append((K, math.sqrt(v)))
    k_star = comps["sigma_frame"]**2 / floor2
    return {"comps": comps, "law": law, "indep": indep, "n_star": n_star,
            "klaw": klaw, "k_star": k_star}


# ============================================================================
# PART D — end-to-end chain: coherence -> coherent localisation
# ============================================================================
def part_d():
    nodes = np.array([
        [0.2, 0.2, 2.5], [2.5, 0.2, 2.6], [4.8, 0.2, 2.5],
        [0.2, 3.8, 2.4], [4.8, 3.8, 2.6], [2.5, 3.8, 2.5],
    ])
    tgt = np.array([2.0, 1.5, 1.0])
    g = np.linspace(-0.3, 0.3, 61)
    GX, GY, GZ = np.meshgrid(g, g, g, indexing="ij")
    grid = np.stack([GX.ravel(), GY.ravel(), GZ.ravel()], axis=1) + tgt

    d_t = np.linalg.norm(nodes - tgt, axis=1)
    # effective lateral aperture: 2 x RMS extent of nodes projected on the
    # plane perpendicular to the target direction (from the array centroid)
    cen = nodes.mean(axis=0)
    u = (tgt - cen) / np.linalg.norm(tgt - cen)
    rel = nodes - cen
    proj = rel - np.outer(rel @ u, u)
    D_eff = 2 * float(np.sqrt(np.mean(np.sum(proj**2, axis=1))))
    r_bar = float(np.mean(d_t))

    # multipath ghost: strongest single wall bounce per link, -6.9 dB (0.45)
    walls = [np.array([2.5, 0.0, 1.3]), np.array([2.5, 4.0, 1.3]),
             np.array([0.0, 2.0, 1.3]), np.array([5.0, 2.0, 1.3])]
    ghost_L = np.array([
        min(np.linalg.norm(tgt - w) + np.linalg.norm(w - xn) for w in walls)
        for xn in nodes
    ])
    wall_amp = 0.45

    los = np.exp(-1j * 2 * math.pi * d_t / LAM)
    gh = wall_amp * np.exp(-1j * 2 * math.pi * ghost_L / LAM)
    dd = np.linalg.norm(nodes[None, :, :] - grid[:, None, :], axis=2)
    steer = np.exp(1j * 2 * math.pi * dd / LAM)
    dh0 = los + gh

    steps = {0: 61 * 61, 1: 61, 2: 1}

    def peak_pos(dh):
        B = np.abs((steer * dh[None, :]).sum(axis=1))
        idx = int(np.argmax(B))
        p = grid[idx].copy()
        for ax in (0, 1, 2):
            li, ri = idx - steps[ax], idx + steps[ax]
            if li >= 0 and ri < len(grid):
                y0, y1, y2 = B[li], B[idx], B[ri]
                den = y0 - 2 * y1 + y2
                if abs(den) > 1e-12:
                    delta = 0.5 * (y0 - y2) / den
                    if abs(delta) <= 1:
                        p[ax] += 0.01 * delta
        return p

    # multipath bias: position of the sigma=0 peak vs the true target
    p_bias = peak_pos(dh0)
    mp_bias = float(np.linalg.norm(p_bias - tgt))

    sigma_list = [0.0, 0.05, 0.1, 0.175, 0.35, 0.7, 1.4]
    n_mc = 240
    prec, rmse, crb = [], [], []
    for s in sigma_list:
        e_prec, e_tot = [], []
        for _ in range(n_mc):
            ph = rng.normal(0, s, 6) if s > 0 else np.zeros(6)
            p = peak_pos(dh0 * np.exp(1j * ph))
            e_prec.append(np.linalg.norm(p - p_bias))
            e_tot.append(np.linalg.norm(p - tgt))
        prec.append(float(np.sqrt(np.mean(np.square(e_prec)))))
        rmse.append(float(np.sqrt(np.mean(np.square(e_tot)))))
        crb.append(r_bar * LAM * s / (2 * math.pi * D_eff) if s > 0 else 0.0)
        log("D", f"sigma={s:5.3f} rad -> precision {prec[-1]*100:5.2f} cm, "
                f"total RMSE {rmse[-1]*100:6.2f} cm, CRLB scale {crb[-1]*100:5.2f} cm")
    return {"sigma": sigma_list, "prec_cm": [x * 100 for x in prec],
            "rmse_cm": [x * 100 for x in rmse], "crb_cm": [x * 100 for x in crb],
            "mp_bias_cm": mp_bias * 100, "D_eff": D_eff, "r_bar": r_bar}


# ============================================================================
# PART E — f-direction: phase-swept / RIPS-style beacon
# ============================================================================
def part_e():
    out = {}
    # (i) RIPS-style two-tone beat: the beat is LO-IMMUNE. LO leakage into the
    # beat phase = 2 pi df sigma_t (sigma_t = sampling-time jitter), i.e. the
    # carrier phase is suppressed by df/f_c.
    out["beat_rad"] = {(df * 1e-3, st * 1e9): 2 * math.pi * df * st
                       for df in (1e3, 10e3, 1e6) for st in (20e-9, 100e-9, 1e-6)}
    # (ii) differential/swept beacon: within-receiver face differencing cancels
    # the LO common phase; the geometry term is computable only to the
    # receiver-position accuracy: dphi = 2 pi dp / lambda (worst case, radial).
    out["pos_bias_rad"] = {dp: 2 * math.pi * dp * 1e-3 / LAM
                           for dp in (1, 3, 10, 32.6, 100)}          # mm
    out["pos_kill_mm"] = 0.5 / (2 * math.pi / LAM) * 1e3
    # group-delay CANNOT certify the geometry term: 1 ns = 2 pi fc * 1e-9 rad
    out["delay_ns_rad"] = 2 * math.pi * FC * 1e-9
    Brms = 40e6 / (2 * math.sqrt(3))
    out["crb_tau_ns"] = math.sqrt(2) / (2 * math.pi * Brms * math.sqrt(2 * 64 * 100)) * 1e9
    # (iii) swept-beacon fit datum: 20-face puck at 3 m, geometry known;
    # LS fit of [x, y, offset] from per-face phases; fit noise only.
    R0, pr, nf = 3.0, 0.15, 20
    ang = np.linspace(0, 2 * math.pi, nf, endpoint=False)
    faces = np.stack([pr * np.cos(ang), pr * np.sin(ang), np.zeros(nf)], axis=1)
    rx = np.array([R0, 0.0, 0.0])
    d_face = np.linalg.norm(faces - rx, axis=1)
    ph_geom = -2 * math.pi * d_face / LAM
    A = np.stack([2 * math.pi / LAM * faces[:, 0],
                  2 * math.pi / LAM * faces[:, 1],
                  np.ones(nf)], axis=1)
    AtA_inv = np.linalg.inv(A.T @ A)
    out["sweep_fit_rad"] = {}
    for sm in (0.04, 0.175, 0.6):
        n_mc = 400
        sig = []
        for _ in range(n_mc):
            z = ph_geom + rng.normal(0, sm, nf)
            coef, *_ = np.linalg.lstsq(A, z, rcond=None)
            resid = z - A @ coef
            sig.append(AtA_inv[2, 2] * np.var(resid, ddof=3))
        out["sweep_fit_rad"][sm] = math.sqrt(float(np.mean(sig)))
    return out


# ============================================================================
# MAIN
# ============================================================================
def main():
    rng_global = np.random.default_rng(RNG_SEED)
    globals()["rng"] = rng_global

    A = part_a()
    print("=== PART A: closed-form budgets ===")
    print("raw CFO (ppm, kHz, tau_pi/2 us):",
          [(r[0], round(r[1], 1), round(r[2], 2)) for r in A["raw_cfo"]])
    print("kill sigma_nu @20 ms holdover:", {k: round(v, 3) for k, v in A["kill_nu_20ms"].items()}, "Hz")
    for s, curve in A["holdover"].items():
        print(f"holdover sigma_phi @ sigma_nu={s} Hz:",
              {g: round(v, 3) for g, v in curve.items()})
    print("jitter kill (kHz, rad)->sigma_tau us:",
          {k: round(v, 3) for k, v in A["jitter_kill"].items()})
    print("quantisation rad:", {k: round(v, 5) for k, v in A["quant_rad"].items()})
    print("per-frame CRB rad:", {k: round(v, 4) for k, v in A["crb_frame"].items()})
    print("CFO fit CRB Hz:", {k: round(v, 4) for k, v in A["cfo_fit_hz"].items()})
    print("E[cos phi]:", {k: round(v, 3) for k, v in A["gain"].items()})
    print("B = dnu*T_obs:", {k: round(v, 4) for k, v in A["budget_B"].items()})
    print("tgt coherence ms:", {k: round(v, 1) for k, v in A["tgt_coh_ms"].items()})
    print("fence duty:", {k: round(v, 4) for k, v in A["fence_duty"].items()})

    print("\n=== PART B: fence Monte Carlo ===")
    Bres = part_b()

    print("\n=== PART C: averaging law ===")
    Cc = part_c()
    print("components (rad):", {k: round(v, 3) for k, v in Cc["comps"].items()})
    print("N* (faces):", round(Cc["n_star"], 2), " K* (packets):", round(Cc["k_star"], 2))
    print("sigma(Nf=1,4,20):", [round(Cc["law"][N - 1][1], 3) for N in (1, 4, 20)])
    print("sigma(K=1,3,12 @Nf=4):", [round(Cc["klaw"][K - 1][1], 3) for K in (1, 3, 12)])

    print("\n=== PART D: end-to-end chain ===")
    D = part_d()
    print(f"geometry: D_eff={D['D_eff']:.2f} m, r_bar={D['r_bar']:.2f} m")

    print("\n=== PART E: f-direction ===")
    E = part_e()
    print("beat LO leakage rad:", {k: round(v, 6) for k, v in E["beat_rad"].items()})
    print("position-error -> datum bias (mm: rad):",
          {k: round(v, 3) for k, v in E["pos_bias_rad"].items()},
          "kill:", round(E["pos_kill_mm"], 2), "mm")
    print(f"1 ns of delay error = {E['delay_ns_rad']:.1f} rad; "
          f"group-delay CRB (40 MHz, 64 sc, 20 dB) = {E['crb_tau_ns']:.3f} ns "
          f"-> {2 * math.pi * FC * E['crb_tau_ns'] * 1e-9:.2f} rad")
    print("swept-beacon fit datum (geometry known):",
          {k: round(v, 4) for k, v in E["sweep_fit_rad"].items()})

    # ---------------- figures ----------------
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    fig, ax = plt.subplots(figsize=(6.4, 4.2), dpi=110)
    styles = {"f1kHz-tau10s": ("C0", "fence 1 kHz"),
              "f200Hz-tau10s": ("C1", "fence 200 Hz"),
              "f50Hz-tau10s": ("C2", "fence 50 Hz"),
              "f200Hz-tau1s": ("C3", "fence 200 Hz, tau_nu=1 s"),
              "f200Hz-ramp8-2st": ("C4", "fence 200 Hz, ramp 8 Hz/s"),
              "f200Hz-sm0p04": ("C5", "fence 200 Hz, sm=0.04")}
    for label, st in Bres:
        if label in styles:
            col, lab = styles[label]
            ax.plot(GAPS_MS, [st[g] for g in GAPS_MS], marker="o", ms=3,
                    lw=1.4, color=col, label=lab)
    for label, st in Bres:
        if label == "f200Hz-tau10s":
            ax.plot(GAPS_MS, [math.sqrt(st[g] ** 2 + SIGMA_RHO_A ** 2) for g in GAPS_MS],
                    marker="s", ms=3, lw=1.2, ls="--", color="C1",
                    label="datum = track + sigma_rho(0.175)")
    for s_nu, col, lab in ((10, "C6", "no fence, sigma_nu=10 Hz"),
                           (100, "C7", "no fence, sigma_nu=100 Hz")):
        ax.plot(GAPS_MS, [2 * math.pi * s_nu * g * 1e-3 for g in GAPS_MS],
                ls=":", color=col, lw=1.3, label=lab)
    ax.axhline(0.5, color="k", lw=0.8, ls="-.")
    ax.axhline(0.1, color="k", lw=0.6, ls="-.")
    ax.text(0.28, 0.55, "0.5 rad gate", fontsize=7)
    ax.text(0.28, 0.105, "0.1 rad", fontsize=7)
    ax.set_xscale("log"); ax.set_yscale("log")
    ax.set_xlabel("data-to-reference gap T_gap (ms)")
    ax.set_ylabel("cross-node phase residual sigma_phi (rad)")
    ax.set_ylim(5e-3, 30)
    ax.legend(fontsize=6.5, loc="upper left")
    ax.set_title("T1 coherence gate: fence-tracked residual (SIMULATION)")
    fig.tight_layout()
    fig.savefig(os.path.join(FIGDIR, "t1-h-sigma-vs-gap.png"))
    plt.close(fig)

    fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(9.6, 3.8), dpi=110)
    sx = [s for s in D["sigma"] if s > 0]
    ax1.loglog(sx, [r for s, r in zip(D["sigma"], D["rmse_cm"]) if s > 0],
               marker="o", label="total RMSE incl. multipath (MC)")
    ax1.loglog(sx, [p for s, p in zip(D["sigma"], D["prec_cm"]) if s > 0],
               marker="^", label="precision about biased peak (MC)")
    ax1.loglog(sx, [c for s, c in zip(D["sigma"], D["crb_cm"]) if s > 0],
               ls="--", label="r*lam*sigma/(2 pi D) scale")
    ax1.axhline(D["mp_bias_cm"], color="gray", lw=0.8, ls=":",
                label=f"multipath floor ({D['mp_bias_cm']:.1f} cm)")
    ax1.set_xlabel("datum sigma_phi (rad)"); ax1.set_ylabel("3D error (cm)")
    ax1.legend(fontsize=7); ax1.set_title("coherence -> localisation")
    ax2.semilogx([l[0] for l in Cc["law"]], [l[1] for l in Cc["law"]],
                 marker="o", ms=3, label="total datum")
    ax2.semilogx([l[0] for l in Cc["law"]], Cc["indep"], ls="--",
                 label="pure 1/sqrt(N) on face noise")
    ax2.axvline(Cc["n_star"], color="k", lw=0.8, ls=":")
    ax2.set_xlabel("faces averaged N_f"); ax2.set_ylabel("sigma (rad)")
    ax2.legend(fontsize=7); ax2.set_title("averaging law (K=1)")
    fig.tight_layout()
    fig.savefig(os.path.join(FIGDIR, "t1-h-chain.png"))
    plt.close(fig)

    with open(os.path.join(HERE, "t1-coherence-gate-h-results.csv"), "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["run", "gap_ms", "sigma_track_rad"])
        for r in ROWS:
            w.writerow([r["run"], r["gap_ms"], r["sigma_track_rad"]])

    print("\nartefacts written:", FIGDIR)


if __name__ == "__main__":
    main()
