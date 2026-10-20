#!/usr/bin/env python3
"""Aquila v2 round 2, Track T1 (the coherence gate), agent b.

Oscillator-level Monte-Carlo + closed-form simulation of the few-board
conducted-truth phase-residual experiment (M2'/E1 family), the TX-jitter
budget, the face/packet averaging law, the ambiguity-resolution cascade and
the coherent localisation error budget for a home-scale ESP32-S3 fleet.

Run with the worktree wrapper:
  /home/matt/projects/aquila-v2-spec/sim-python docs/research/v2/code/t1_gate_b.py

Outputs (small, per EXPERIMENT-ENV.md disk discipline):
  docs/research/v2/figures/23-t1-b-*.png   (5 figures)
  docs/research/v2/code/t1_gate_b_results.csv  (all scalar results)

This is a SIMULATION, not a measurement. See note 23-coherence-gate-b.md for
the provenance of every parameter ladder below.
"""

import csv
import os

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
from scipy.linalg import solve_discrete_are

HERE = os.path.dirname(os.path.abspath(__file__))
FIGDIR = os.path.normpath(os.path.join(HERE, "..", "figures"))
os.makedirs(FIGDIR, exist_ok=True)
RNG = np.random.default_rng(20261016)

C = 299_792_458.0
FC = 2.437e9          # channel 6 centre (note 12 uses 2.437 GHz)
LAM = C / FC

# ---------------------------------------------------------------- ladders ----
# beta_rel: effective RELATIVE Lorentzian linewidth (Hz) of the oscillator pair
# at the carrier: Var[dTheta(tau)] = 4*pi*beta_rel*tau (rad^2).
#   0.05 Hz : disciplined-best   (ESP-PPB "<1 deg best", ~1 ms)
#   0.5  Hz : disciplined-typical (ESP-PPB "<10 deg typical")
#   4    Hz : free-running optimistic (round-1 residual "10 Hz" reading)
#   40   Hz : MegaMIMO-class post-CFO residual ("100 Hz -> pi in 20 ms")
BETAS = [0.05, 0.5, 4.0, 40.0]
BETA_LABELS = ["0.05 Hz disc. best", "0.5 Hz disc. typical",
               "4 Hz free-run optimistic", "40 Hz MegaMIMO-class"]

# sigma_p: per-packet white common-phase noise (CSI estimation + non-invertible
# part of the receive-side CFO-correction artefact)
SIG_P = [0.03, 0.17, 0.5]

TGAPS_MS = [0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0]

DNU = 48.7e3          # 20 ppm differential CFO at 2.437 GHz (note 12)
SIG_DT = [0.3e-6, 1.6e-6, 16e-6]   # reply-instant jitter ladder (s)
SIG_DMEAS = 30e-9     # ns-RX-timestamp knowledge of the reply interval

RESULTS = {}


def rec(key, val):
    RESULTS[key] = val
    print(f"{key:<70s} {val}")


# =============================================================================
# 1. Closed-form gate curves: holdover and bracket vs gap, beta ladder
# =============================================================================
def sigma_hold(beta, T, sp):
    """One-sided holdover over gap T (last reference T before sensing)."""
    return np.sqrt(4 * np.pi * beta * T + sp ** 2)


def sigma_bracket(beta, T, sp):
    """Two-sided bracket: refs at +/-T/2, sensing at centre (Wolfram-checked)."""
    return np.sqrt(np.pi * beta * T + sp ** 2 / 2)


Tg = np.array(TGAPS_MS) * 1e-3
fig, ax = plt.subplots(figsize=(5.4, 3.7), dpi=110)
cols = ["#2166ac", "#4393c3", "#f4a582", "#b2182b"]
for beta, lab, col in zip(BETAS, BETA_LABELS, cols):
    ax.loglog(Tg * 1e3, sigma_bracket(beta, Tg, 0.03), color=col, label=f"bracket, {lab}")
    ax.loglog(Tg * 1e3, sigma_hold(beta, Tg, 0.03), color=col, ls="--", alpha=0.55)
ax.axhline(0.5, color="k", lw=0.8)
ax.axhline(0.3, color="k", lw=0.8, ls=":")
ax.axhline(1.5, color="k", lw=0.8, ls="--")
ax.text(0.105, 0.53, "gate 0.5 rad", fontsize=7)
ax.text(0.105, 0.315, "0.3 rad", fontsize=7)
ax.text(0.105, 1.56, "E1 fail 1.5 rad", fontsize=7)
ax.set_xlabel("reference-to-sensing gap T (ms)")
ax.set_ylabel(r"$\sigma_\varphi$ (rad)")
ax.set_title(r"Gate curves, $\sigma_p$=0.03 rad: bracket (solid) vs holdover (dashed)")
ax.legend(fontsize=6.5, loc="upper left")
ax.grid(alpha=0.3, which="both")
fig.tight_layout()
fig.savefig(os.path.join(FIGDIR, "23-t1-b-fig1-gate.png"))
plt.close(fig)


def mc_gate(beta, T, sp, n_seed=192, dur=6.0, dt=2.5e-4):
    """MC check: bracket + holdover errors along a simulated phase walk."""
    n = int(dur / dt)
    inc_std = np.sqrt(4 * np.pi * beta * dt)
    half = int(T / (2 * dt))
    step = max(4 * int(T / dt), 8)
    err_b, err_h = [], []
    for _ in range(n_seed):
        W = np.cumsum(RNG.normal(0, inc_std, n))
        for c in range(step, n - step, step):
            if c + half < n:
                L = c - half
                R = c + half
                err_b.append(W[c] - ((W[L] + RNG.normal(0, sp)) + (W[R] + RNG.normal(0, sp))) / 2)
            Lh = c - 2 * half
            if Lh >= 0:
                err_h.append(W[c] - (W[Lh] + RNG.normal(0, sp)))
    return np.std(err_b), np.std(err_h)


for beta in (4.0, 40.0):
    for T_ms in (1.0, 20.0):
        T = T_ms * 1e-3
        sb_mc, sh_mc = mc_gate(beta, T, 0.17)
        rec(f"MC check bracket beta={beta}Hz T={T_ms}ms (MC / closed)",
            f"{sb_mc:.3f} / {sigma_bracket(beta, T, 0.17):.3f} rad")
        rec(f"MC check holdover beta={beta}Hz T={T_ms}ms (MC / closed)",
            f"{sh_mc:.3f} / {sigma_hold(beta, T, 0.17):.3f} rad")

# gate crossing table (max gap for 0.5 rad) and design points
for beta in BETAS:
    for sp in SIG_P:
        Th = (0.25 - sp ** 2) / (4 * np.pi * beta) if 0.25 > sp ** 2 else np.inf
        Tb = (0.25 - sp ** 2 / 2) / (np.pi * beta) if 0.25 > sp ** 2 / 2 else np.inf
        rec(f"max gap @0.5 rad, beta={beta}Hz sigma_p={sp}rad: holdover/bracket (ms)",
            f"{Th * 1e3:.2f} / {Tb * 1e3:.2f}")
for beta in BETAS:
    rec(f"design points beta={beta}Hz sp=0.17: hold@20ms/brk@2ms/brk@1ms (rad)",
        f"{sigma_hold(beta, 0.02, 0.17):.3f} / {sigma_bracket(beta, 0.002, 0.17):.3f}"
        f" / {sigma_bracket(beta, 0.001, 0.17):.3f}")
    rec(f"design points beta={beta}Hz F=20 faces: hold@20ms/brk@1ms (rad)",
        f"{np.sqrt(4 * np.pi * beta * 0.02 + 0.17 ** 2 / 20):.3f}"
        f" / {np.sqrt(np.pi * beta * 0.001 + 0.17 ** 2 / 40):.3f}")
# ESP-PPB claim mapping: claimed single-frame residual at a ~1 ms bracket,
# resolved into its two possible physical causes:
for deg, lab in ((1, "best"), (10, "typical")):
    sig = np.deg2rad(deg)
    beta_walk = sig ** 2 / (np.pi * 1e-3)          # if all walk (sigma_p=0)
    sp_white = sig * np.sqrt(2)                    # if all white (beta=0)
    rec(f"ESP-PPB <{deg} deg {lab} @1ms: implied beta_rel / implied sigma_p",
        f"{beta_walk:.2f} Hz / {sp_white:.3f} rad")

# =============================================================================
# 2. Kalman [phase, frequency] tracker: DARE steady state + MC check
# =============================================================================
def kalman_steady(beta, Tc, sp, sig_F=2 * np.pi * 25.0, tau_F=120.0):
    """Steady-state covariances for state [phase, freq(rad/s)] at cadence Tc.

    Phase: Wiener, Var[dt] = 4*pi*beta*dt. Freq: OU (stationary std sig_F,
    correlation time tau_F) modelling slow thermal CFO wander. Measurement:
    z = phase + N(0, sp^2). Returns (P_prior, P_post, S).
    """
    rho_f = np.exp(-Tc / tau_F)
    F = np.array([[1.0, Tc], [0.0, rho_f]])
    qf = sig_F ** 2 * (1 - rho_f ** 2)
    Q = np.array([[4 * np.pi * beta * Tc, 0.0], [0.0, qf]])
    H = np.array([[1.0], [0.0]])
    R = np.array([[sp ** 2]])
    # prediction Riccati: X = F X F' - (F X H)(R + H' X H)^-1 (H' X F) + Q
    Pm = solve_discrete_are(F.T, H, Q, R)            # prior covariance
    S = Pm[0, 0] + sp ** 2
    K = (Pm @ H) / S
    Pp0 = Pm - K @ (H.T @ Pm)
    return Pm, Pp0, S


def kalman_holdover(beta, Tc, sp, Th):
    """Prediction-error std over horizon Th after the last update."""
    _, Pp0, _ = kalman_steady(beta, Tc, sp)
    var = (Pp0[0, 0] + 2 * Th * Pp0[0, 1] + Th ** 2 * Pp0[1, 1]
           + 4 * np.pi * beta * Th)
    return np.sqrt(var), np.sqrt(Pp0[1, 1]) / (2 * np.pi)  # sigma_phi, sigma_f [Hz]


CADENCES_MS = [0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0]
HORIZONS_MS = [0.5, 1, 2, 5, 10, 20, 50, 100]
sp_k = 0.17
for beta_k in BETAS:
    for Tc in CADENCES_MS:
        _, _, S = kalman_steady(beta_k, Tc * 1e-3, sp_k)
        sh, sf = kalman_holdover(beta_k, Tc * 1e-3, sp_k, 20e-3)
        rec(f"Kalman beta={beta_k}Hz sp=0.17 cad={Tc}ms: innov/hold@20ms/sig_f",
            f"{np.sqrt(S):.3f} rad / {sh:.3f} rad / {sf:.2f} Hz")

fig, axes = plt.subplots(1, 2, figsize=(7.8, 3.3), dpi=110, sharey=True)
for beta, col in zip(BETAS, cols):
    ys = [kalman_holdover(beta, Tc * 1e-3, 0.17, 20e-3)[0] for Tc in CADENCES_MS]
    axes[0].loglog(CADENCES_MS, ys, "o-", color=col, label=f"$\\beta$={beta} Hz")
for Tc, col in zip([0.5, 2.0, 20.0], cols[:3]):
    hs = np.array(HORIZONS_MS) * 1e-3
    ys = [kalman_holdover(4.0, Tc * 1e-3, 0.17, h)[0] for h in hs]
    axes[1].loglog(hs * 1e3, ys, "o-", color=col, ms=3, label=f"cadence {Tc} ms")
axes[0].axhline(0.5, color="k", lw=0.8)
axes[1].axhline(0.5, color="k", lw=0.8)
axes[0].set_xlabel("update cadence (ms)")
axes[0].set_ylabel(r"holdover 20 ms $\sigma_\varphi$ (rad)")
axes[0].set_title(r"20 ms holdover vs cadence ($\sigma_p$=0.17)")
axes[0].legend(fontsize=6.5)
axes[1].set_xlabel("holdover horizon (ms)")
axes[1].set_title(r"prediction error vs horizon ($\beta$=4 Hz)")
axes[1].legend(fontsize=6.5)
axes[0].grid(alpha=0.3, which="both")
axes[1].grid(alpha=0.3, which="both")
fig.tight_layout()
fig.savefig(os.path.join(FIGDIR, "23-t1-b-fig2-kalman.png"))
plt.close(fig)


def mc_kalman2(beta, Tc, sp, n_seed=96, dur=12.0, sig_F=2 * np.pi * 25.0, tau_F=120.0):
    """Vectorised KF Monte-Carlo: innovation std + 20 ms holdover std."""
    rho_f = np.exp(-Tc / tau_F)
    n = int(dur / Tc)
    qphi = 4 * np.pi * beta * Tc
    qf = sig_F ** 2 * (1 - rho_f ** 2)
    ph_t = RNG.uniform(-np.pi, np.pi, n_seed)
    f_t = 2 * np.pi * DNU * RNG.normal(size=n_seed) + sig_F * RNG.normal(size=n_seed)
    ph_h = ph_t.copy()
    f_h = np.zeros(n_seed)
    P = np.tile(np.eye(2) * 1e6, (n_seed, 1, 1))
    innov = []
    hold_errs = []
    for k in range(n):
        ph_t = ph_t + f_t * Tc + RNG.normal(0, np.sqrt(qphi), n_seed)
        f_t = f_t * rho_f + RNG.normal(0, np.sqrt(qf), n_seed)
        z = ph_t + RNG.normal(0, sp, n_seed)
        ph_h = ph_h + f_h * Tc
        f_h = f_h * rho_f
        P00 = P[:, 0, 0] + Tc * (P[:, 0, 1] + P[:, 1, 0]) + Tc ** 2 * P[:, 1, 1] + qphi
        P01 = (P[:, 0, 1] + Tc * P[:, 1, 1]) * rho_f
        P11 = P[:, 1, 1] * rho_f ** 2 + qf
        P[:, 0, 0], P[:, 0, 1], P[:, 1, 0], P[:, 1, 1] = P00, P01, P01, P11
        Sx = P[:, 0, 0] + sp ** 2
        K0 = P[:, 0, 0] / Sx
        K1 = P[:, 1, 0] / Sx
        inno = z - ph_h
        if k > 200:
            innov.append(inno.std())
            ph_pred = (ph_h + K0 * inno) + (f_h + K1 * inno) * 0.02
            ph_true = ph_t + f_t * 0.02 + RNG.normal(0, np.sqrt(4 * np.pi * beta * 0.02), n_seed)
            hold_errs.append((ph_pred - ph_true) ** 2)
        ph_h = ph_h + K0 * inno
        f_h = f_h + K1 * inno
        # post-update covariance (uses pre-update P00/P01 only)
        P00 = P[:, 0, 0]
        P01 = P[:, 0, 1]
        P11 = P[:, 1, 1]
        P[:, 0, 0] = P00 - K0 * P00
        P[:, 0, 1] = P01 - K0 * P01
        P[:, 1, 0] = P01 - K1 * P00
        P[:, 1, 1] = P11 - K1 * P01
    return float(np.mean(innov)), float(np.sqrt(np.mean(hold_errs)))


inn_mc, hold_mc = mc_kalman2(4.0, 2e-3, 0.17)
_, _, S = kalman_steady(4.0, 2e-3, 0.17)
sh_cf, _ = kalman_holdover(4.0, 2e-3, 0.17, 20e-3)
rec("Kalman MC check beta=4Hz cad=2ms sp=0.17: innovation MC/DARE",
    f"{inn_mc:.3f} / {np.sqrt(S):.3f} rad")
rec("Kalman MC check beta=4Hz cad=2ms: 20 ms holdover MC/DARE",
    f"{hold_mc:.3f} / {sh_cf:.3f} rad")

# =============================================================================
# 3. Two-way (ping + unique reply) budget with TX-jitter
# =============================================================================
def sigma_twoway(Delta, sig_dt, beta, sp, sig_f, dnu=DNU, F=20, M=1):
    """Sum-carrier-phase (product) error std for a burst of M exchanges.

    Delta: reply interval (s). sig_dt: reply-instant uncertainty. sig_f:
    residual relative-frequency error of the tracker (Hz). F: faces per puck.
    M: exchanges averaged in the burst. The product is LO-free, so every
    white term (jitter, walk-within-exchange, measurement) averages as /M;
    the residual-CFO x known-interval term is common across the burst.
    Walk within one exchange: Var = 4*pi*beta*Delta (LO evolution over the
    two legs, both oscillators).
    """
    jit = 2 * np.pi * dnu * sig_dt                 # reply-instant x true CFO
    nomin = 2 * np.pi * sig_f * Delta              # residual CFO x interval
    tmeas = 2 * np.pi * dnu * SIG_DMEAS            # interval knowledge
    walk = 4 * np.pi * beta * Delta                # per exchange, independent
    meas = 2 * sp ** 2 / F                         # two CSI estimates, F faces
    return np.sqrt((jit ** 2 + walk + meas) / M + nomin ** 2 + tmeas ** 2)


DELTAS_MS = [0.1, 0.3, 0.7, 1.0, 3.0, 10.0, 20.0]
fig, ax = plt.subplots(figsize=(5.4, 3.5), dpi=110)
beta_tw = 4.0
sp_tw = 0.17
sig_f_tw = kalman_holdover(4.0, 2e-3, 0.17, 20e-3)[1]    # residual freq error (Hz)
D = np.array(DELTAS_MS) * 1e-3
for sdt, lab, col in zip(SIG_DT, ["0.3 us", "1.6 us", "16 us"], cols[:3]):
    y1 = [sigma_twoway(d, sdt, beta_tw, sp_tw, sig_f_tw, F=20, M=1) for d in D]
    y10 = [sigma_twoway(d, sdt, beta_tw, sp_tw, sig_f_tw, F=20, M=10) for d in D]
    ax.loglog(DELTAS_MS, y1, "o-", color=col, label=f"jitter {lab}, M=1")
    ax.loglog(DELTAS_MS, y10, "s--", color=col, alpha=0.6, label=f"jitter {lab}, M=10")
ax.axhline(0.5, color="k", lw=0.8)
ax.axvline(0.7, color="grey", lw=0.6, ls=":")
ax.text(0.73, 0.05, "fixed-rate ESP-NOW RTT ~0.7 ms", fontsize=6.5, color="grey", rotation=90)
ax.set_xlabel("reply interval $\\Delta$ (ms)")
ax.set_ylabel(r"$\sigma_\varphi$ of two-way product (rad)")
ax.set_title(rf"Two-way budget: $\beta$=4 Hz, $\sigma_p$=0.17 rad, 20 faces/puck")
ax.legend(fontsize=6.5)
ax.grid(alpha=0.3, which="both")
fig.tight_layout()
fig.savefig(os.path.join(FIGDIR, "23-t1-b-fig3-twoway.png"))
plt.close(fig)

rec("residual freq error used (Hz, beta=4Hz cad=2ms)", f"{sig_f_tw:.2f}")
for sdt in SIG_DT:
    for M in (1, 10):
        y = sigma_twoway(0.7e-3, sdt, beta_tw, sp_tw, sig_f_tw, F=20, M=M)
        rec(f"two-way sigma @Delta=0.7ms jitter={sdt * 1e6:.1f}us M={M} F=20",
            f"{y:.3f} rad")
for Dms in (0.7, 3.0, 20.0):
    y = sigma_twoway(Dms * 1e-3, 1.6e-6, beta_tw, sp_tw, sig_f_tw, F=20, M=1)
    rec(f"two-way sigma @Delta={Dms}ms jitter=1.6us F=20 M=1", f"{y:.3f} rad")
for beta in BETAS:
    y = sigma_twoway(0.7e-3, 1.6e-6, beta, sp_tw, sig_f_tw, F=20, M=1)
    rec(f"two-way sigma @Delta=0.7ms jitter=1.6us beta={beta}Hz", f"{y:.3f} rad")
rec("max single-exchange reply jitter for 0.5 rad @48.7 kHz (us)",
    f"{(0.5 / (2 * np.pi * DNU)) * 1e6:.2f}")
rec("max reply jitter for 0.5 rad @48.7 kHz with M=10 (us)",
    f"{(0.5 / (2 * np.pi * DNU)) * 1e6 * np.sqrt(10):.1f}")

# =============================================================================
# 4. Face/packet averaging law (exact finite-M walk variance + MC)
# =============================================================================
def walk_avg_var(beta, M, Tc):
    """Var[mean of M Wiener samples at spacing Tc - W(centre)] (exact)."""
    a = 4 * np.pi * beta
    t = np.arange(M) * Tc
    tc = (M - 1) * Tc / 2
    cov = a * (np.minimum(t[:, None], t[None, :]) - np.minimum(t, tc)[:, None]
               - np.minimum(t, tc)[None, :] + tc)
    return cov.sum() / M ** 2


def sigma_avg(beta, M, Tc, sp, F=1):
    return np.sqrt(walk_avg_var(beta, M, Tc) + sp ** 2 / (M * F))


Ms = np.array([1, 2, 4, 8, 16, 32, 64, 128])
fig, ax = plt.subplots(figsize=(5.4, 3.5), dpi=110)
Tc_a = 1e-3
for beta, col in zip([4.0, 40.0], cols[2:]):
    for F, ls in ((1, "--"), (20, "-")):
        ys = [sigma_avg(beta, M, Tc_a, 0.17, F) for M in Ms]
        ax.loglog(Ms, ys, ls, color=col, marker="o", ms=3, label=f"$\\beta$={beta} Hz, F={F}")
ax.loglog(Ms, 0.17 / np.sqrt(Ms * 20), ":", color="k", label=r"pure white, F=20")
ax.set_xlabel("packets averaged per fix, M (cadence 1 ms)")
ax.set_ylabel(r"$\sigma_\varphi$ of averaged fix (rad)")
ax.set_title("Averaging law: white falls as $1/\\sqrt{MF}$, walk floor grows")
ax.legend(fontsize=6.5)
ax.grid(alpha=0.3, which="both")
fig.tight_layout()
fig.savefig(os.path.join(FIGDIR, "23-t1-b-fig4-averaging.png"))
plt.close(fig)


def mc_avg(beta, M, Tc, sp, n_seed=192, dur=3.0, dt=1e-4):
    n = int(dur / dt)
    inc = np.sqrt(4 * np.pi * beta * dt)
    step = int(Tc / dt)
    errs = []
    for _ in range(n_seed):
        W = np.cumsum(RNG.normal(0, inc, n))
        for c in range(step * (M - 1), n, step * M):
            seg = W[c - step * (M - 1): c + 1: step]
            w = W[c - step * (M - 1) // 2]
            errs.append(seg.mean() + RNG.normal(0, sp / np.sqrt(M)) - w)
    return np.std(errs)


mc_a = mc_avg(4.0, 16, 1e-3, 0.17)
cf_a = sigma_avg(4.0, 16, 1e-3, 0.17)
rec("MC check averaging law beta=4Hz M=16 Tc=1ms sp=0.17 (MC / closed)",
    f"{mc_a:.3f} / {cf_a:.3f} rad")
for beta in BETAS:
    for F in (1, 20):
        ys = [sigma_avg(beta, M, 1e-3, 0.17, F) for M in Ms]
        rec(f"best M @1ms cadence & sigma, beta={beta}Hz F={F}",
            f"M={Ms[int(np.argmin(ys))]}, sigma={min(ys):.3f} rad")

# =============================================================================
# 5. Ambiguity cascade + localisation error budget
# =============================================================================
PUCKS = np.array([
    [0.4, 0.4, 2.2], [7.6, 0.4, 2.2], [0.4, 4.6, 2.2], [7.6, 4.6, 2.2],
    [4.0, 0.4, 0.4], [4.0, 4.6, 0.4]])   # last two at shelf height (z-leverage)
TARGET = np.array([4.0, 2.5, 1.2])
r_true = np.linalg.norm(PUCKS - TARGET, axis=1)
K = len(PUCKS)
SIG_FTM = 2.0          # m; S3 FTM 2.4 GHz HT20 (notes 15/17)
B_STITCH = 83e6        # 13 x 20 MHz overlapping channels (Zubow; note t M6)
LAMBDA2 = LAM / 2      # two-way sum-CP range ambiguity
STITCH_WRAP = C / (2 * B_STITCH)   # 1.81 m
WRAP20 = C / (2 * 20e6)            # intra-channel delay ambiguity: 7.5 m
B20 = 0.10             # intra-channel first-path bias knob (m)
SIG20_NOISE = C / 2 / (2 * np.pi * (20e6 / np.sqrt(12)) * np.sqrt(2 * 100 * 56))


def gn_position(ranges, P, iters=15):
    """Gauss-Newton multilaterion from K ranges, vectorised over trials.
    Step clipping at 0.5 m keeps the fit from diverging on corrupt ranges."""
    for _ in range(iters):
        d = np.linalg.norm(P[:, None, :] - PUCKS[None, :, :], axis=2)
        J = (P[:, None, :] - PUCKS[None, :, :]) / (d[:, :, None] + 1e-9)
        A = J.transpose(0, 2, 1) @ J + 1e-6 * np.eye(3)
        b = (J.transpose(0, 2, 1) @ (ranges - d)[:, :, None])
        dp = np.linalg.solve(A, b)[:, :, 0]
        n = np.linalg.norm(dp, axis=1)
        m = n > 0.5
        if m.any():
            dp[m] *= (0.5 / n[m])[:, None]
        P = P + dp
    return P


def stitched_noise():
    Brms = B_STITCH / np.sqrt(12)
    rho_tot = 100 * 728                       # 20 dB/subcarrier x 728 sc
    return C / 2 / (2 * np.pi * Brms * np.sqrt(2 * rho_tot))


S_NOISE = stitched_noise()


def cascade_mc(sig_phi_eff, s_bias, n=6000, seed=1234):
    """Four-rung ambiguity cascade: FTM -> CH20 delay -> 83 MHz stitched ->
    two-way carrier bins -> LS position.

    Returns (per-link all-rung success, all-links joint success,
    position RMSE over all trials, position RMSE on joint-success trials).
    """
    rng = np.random.default_rng(seed)
    n_blk, blk = n // 200, 200
    ok_link = ok_all = 0
    pos_err = []
    pos_err_ok = []
    for _ in range(n_blk):
        # rung 1: FTM LS position (cold start near room centre)
        ftm = r_true + rng.normal(0, SIG_FTM, (blk, K))
        P0 = np.tile(np.array([4.0, 2.5, 1.3]), (blk, 1)) + rng.normal(0, 1.0, (blk, 3))
        P0 = gn_position(ftm, P0)
        r_pred = np.linalg.norm(P0[:, None, :] - PUCKS[None, :, :], axis=2)
        # rung 2: intra-channel (20 MHz) delay, mod 7.5 m, unwrapped by FTM
        d20 = np.mod(r_true + rng.normal(0, SIG20_NOISE, (blk, K))
                     + rng.normal(0, B20, (blk, K)), WRAP20)
        r20 = d20 + np.round((r_pred - d20) / WRAP20) * WRAP20
        ok20 = np.abs(r20 - r_true) < WRAP20 / 2
        # rung 3: stitched 83 MHz delay, mod 1.81 m, unwrapped by rung 2
        d83 = np.mod(r_true + rng.normal(0, S_NOISE, (blk, K))
                     + rng.normal(0, s_bias, (blk, K)), STITCH_WRAP)
        r83 = d83 + np.round((r20 - d83) / STITCH_WRAP) * STITCH_WRAP
        ok83 = np.abs(r83 - r_true) < STITCH_WRAP / 2
        # rung 4: two-way carrier phase, mod 6.15 cm, unwrapped by rung 3
        car = r_true + rng.normal(0, LAM * sig_phi_eff / (4 * np.pi), (blk, K))
        rr = car + np.round((r83 - car) / LAMBDA2) * LAMBDA2
        okc = np.abs(rr - r_true) < LAMBDA2 / 2
        allok = ok20 & ok83 & okc
        ok_link += allok.sum()
        ok_all += allok.all(axis=1).sum()
        P1 = gn_position(rr, P0.copy())
        e = np.linalg.norm(P1 - TARGET, axis=1)
        pos_err.append(e)
        pos_err_ok.append(e[allok.all(axis=1)])
    pos_err = np.concatenate(pos_err)
    pos_err_ok = (np.concatenate(pos_err_ok) if pos_err_ok else np.array([np.nan]))
    return (ok_link / (n * K), ok_all / n,
            np.sqrt(np.mean(pos_err ** 2)), float(np.sqrt(np.mean(pos_err_ok ** 2))))


fig, axes = plt.subplots(1, 2, figsize=(7.8, 3.3), dpi=110)
biases = [0.005, 0.01, 0.03, 0.05, 0.10, 0.30]
phis = [0.05, 0.1, 0.3, 0.5, 1.0]
rates = {p: [] for p in phis}
for p in phis:
    for b in biases:
        lk, al, rm, rmok = cascade_mc(p, b, n=3000)
        rates[p].append(al)
        rec(f"cascade bias={b}m sig_phi={p}rad: link/all/RMSE/RMSE|ok",
            f"{lk:.3f}/{al:.3f}/{rm * 100:.0f}cm/{rmok * 100:.1f}cm")
for p in phis:
    axes[0].loglog(biases, rates[p], "o-", label=f"$\\sigma_\\varphi$={p} rad")
axes[0].set_xlabel("stitched-layer multipath bias (m)")
axes[0].set_ylabel("end-to-end bin-fix success rate")
axes[0].axhline(0.95, color="k", lw=0.7, ls=":")
axes[0].legend(fontsize=6.5)
axes[0].grid(alpha=0.3, which="both")

# warm-start CRLB vs sigma_phi (carrier ranges only, noise-limited)
sig_phis = np.linspace(0.02, 1.0, 25)


def pos_crlb(sig_r):
    J = np.zeros((K, 3))
    for k in range(K):
        J[k] = (TARGET - PUCKS[k]) / r_true[k] / sig_r
    return np.sqrt(np.trace(np.linalg.inv(J.T @ J)))


crlb = [pos_crlb(LAM * p / (4 * np.pi)) for p in sig_phis]
axes[1].loglog(sig_phis, np.array(crlb) * 100, label="CRLB (noise only)")
rng2 = np.random.default_rng(7)
for bias_c in (0.005, 0.02):
    ph_pts, errs = [], []
    for p in sig_phis[::4]:
        rr = (r_true + rng2.normal(0, LAM * p / (4 * np.pi), (3000, K))
              + rng2.normal(0, bias_c, (3000, K)))
        P = np.tile(TARGET, (3000, 1)) + rng2.normal(0, 0.005, (3000, 3))
        P = gn_position(rr, P, iters=4)
        ph_pts.append(p)
        errs.append(np.sqrt(np.mean(np.linalg.norm(P - TARGET, axis=1) ** 2)))
    axes[1].loglog(ph_pts, np.array(errs) * 100, "s--",
                   label=f"MC + {bias_c * 100:.0f} cm link bias")
axes[1].set_xlabel(r"$\sigma_\varphi$ per two-way exchange (rad)")
axes[1].set_ylabel("3D position RMSE (cm)")
axes[1].set_title("Warm start (bins pre-resolved), K=6 pucks")
axes[1].legend(fontsize=6.5)
axes[1].grid(alpha=0.3, which="both")
fig.tight_layout()
fig.savefig(os.path.join(FIGDIR, "23-t1-b-fig5-cascade.png"))
plt.close(fig)

rec("carrier-phase range noise per link @ sig_phi=0.3 rad (mm)",
    f"{LAM * 0.3 / (4 * np.pi) * 1e3:.1f}")
rec("stitched-delay CRLB range noise @83 MHz, 20 dB, 728 sc (mm)",
    f"{S_NOISE * 1e3:.1f}")
rec("stitched unambiguous range c/2B (m)", f"{STITCH_WRAP:.2f}")
rec("two-way carrier ambiguity lambda/2 (cm)", f"{LAMBDA2 * 100:.2f}")
rec("CH20 delay noise, 56 sc, 20 dB (cm)", f"{SIG20_NOISE * 100:.2f}")

# per-rung diagnostics + graceful degradation at one operating point
def cascade_diag(sig_phi_eff, s_bias, n=4000, seed=99):
    rng = np.random.default_rng(seed)
    blk = 200
    ok = np.zeros(3)
    ftm_pos_err = []
    st_pos_err = []
    for _ in range(n // blk):
        ftm = r_true + rng.normal(0, SIG_FTM, (blk, K))
        P0 = np.tile(np.array([4.0, 2.5, 1.3]), (blk, 1)) + rng.normal(0, 1.0, (blk, 3))
        P0 = gn_position(ftm, P0)
        r_pred = np.linalg.norm(P0[:, None, :] - PUCKS[None, :, :], axis=2)
        ftm_pos_err.append(np.linalg.norm(P0 - TARGET, axis=1))
        d20 = np.mod(r_true + rng.normal(0, SIG20_NOISE, (blk, K))
                     + rng.normal(0, B20, (blk, K)), WRAP20)
        r20 = d20 + np.round((r_pred - d20) / WRAP20) * WRAP20
        ok[0] += (np.abs(r20 - r_true) < WRAP20 / 2).sum()
        d83 = np.mod(r_true + rng.normal(0, S_NOISE, (blk, K))
                     + rng.normal(0, s_bias, (blk, K)), STITCH_WRAP)
        r83 = d83 + np.round((r20 - d83) / STITCH_WRAP) * STITCH_WRAP
        ok[1] += (np.abs(r83 - r_true) < STITCH_WRAP / 2).sum()
        # graceful mode: position from stitched ranges only
        Ps = gn_position(r83, P0.copy())
        st_pos_err.append(np.linalg.norm(Ps - TARGET, axis=1))
        car = r_true + rng.normal(0, LAM * sig_phi_eff / (4 * np.pi), (blk, K))
        rr = car + np.round((r83 - car) / LAMBDA2) * LAMBDA2
        ok[2] += (np.abs(rr - r_true) < LAMBDA2 / 2).sum()
    return (ok / (n * K), np.sqrt(np.mean(np.concatenate(ftm_pos_err) ** 2)),
            np.median(np.concatenate(st_pos_err)),
            np.sqrt(np.mean(np.concatenate(st_pos_err) ** 2)))


for s_bias in (0.01, 0.03):
    oks, ftm_rmse, st_med, st_rmse = cascade_diag(0.3, s_bias)
    rec(f"rung diag bias={s_bias}m: ok20/ok83/okc per-link; FTMpos RMSE; "
        f"stitched-mode med/RMSE",
        f"{oks[0]:.4f}/{oks[1]:.4f}/{oks[2]:.4f}; {ftm_rmse:.2f}m; "
        f"{st_med * 100:.1f}cm/{st_rmse * 100:.0f}cm")

# =============================================================================
# 6. Airtime ledger
# =============================================================================
def frame_time_s(payload_b, mbps):
    return 40e-6 + (payload_b + 60) * 8 / (mbps * 1e6)


rec("HT20 frame time MCS0 127B (us)", f"{frame_time_s(127, 6.5) * 1e6:.0f}")
rec("HT20 frame time MCS7-SGI 127B (us)", f"{frame_time_s(127, 65) * 1e6:.0f}")
for N in (2, 8, 20):
    for rate, lab in ((6.5, "MCS0"), (65, "MCS7")):
        cyc = (N + 3) * (frame_time_s(127, rate) + 16e-6)
        for hz in (10, 20, 50):
            rec(f"airtime N={N} {lab} cadence={hz}Hz cycle={cyc * 1e3:.1f}ms",
                f"{cyc * hz * 100:.1f}%")

# =============================================================================
# 7. RIPS-style phase-swept beacon (missing direction f) numbers
# =============================================================================
for rho_db in (0, 10, 20, 30):
    for Tw in (1e-3, 10e-3):
        Ns = int(Tw * 250e3)                    # esp-sdr S3 decimated I/Q rate
        rho = 10 ** (rho_db / 10)
        sb = 1 / np.sqrt(Ns * rho)
        rec(f"RIPS beat-phase sigma rho={rho_db}dB Tw={Tw * 1e3:.0f}ms",
            f"{sb:.4f} rad -> q-noise {LAM * sb / (2 * np.pi) * 1e3:.2f} mm")
rec("RIPS cross-node time alignment for 0.1 rad beat phase @48.7 kHz (us)",
    f"{0.1 / (2 * np.pi * DNU) * 1e6:.2f}")

# =============================================================================
# 8. Ledger: coherence budget, target coherence time
# =============================================================================
for dnu_res in (1, 10, 100):
    for T_obs in (0.02, 1.0):
        rec(f"B_det = dnu_res*T_obs = {dnu_res}Hz x {T_obs}s (cycles)",
            f"{dnu_res * T_obs:.2f}")
for beta in BETAS:
    for T in (1e-3, 20e-3):
        rec(f"sigma_phi wiener beta={beta}Hz T={T * 1e3:.0f}ms (rad)",
            f"{np.sqrt(4 * np.pi * beta * T):.4f}")
for v in (1.0, 0.5, 0.1):
    rec(f"target coherence time lambda/2v @ v={v} m/s (ms)", f"{LAM / (2 * v) * 1e3:.0f}")

# =============================================================================
# write CSV
# =============================================================================
out = os.path.join(HERE, "t1_gate_b_results.csv")
with open(out, "w", newline="") as f:
    w = csv.writer(f)
    w.writerow(["quantity", "value"])
    for k, v in RESULTS.items():
        w.writerow([k, v])
print(f"\nwrote {out} and 5 figures to {FIGDIR}")
