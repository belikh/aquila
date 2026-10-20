#!/usr/bin/env python
"""Round-2 T2 (character `d`): gauge-free observable algebra under ESP32-like
nuisance phases.

Question: which carrier-phase observables survive the two contested ESP32-S3
nuisances — (i) the post-CFO-removal residual LO phase (the M2 family, modelled
as an Ornstein-Uhlenbeck residual-CFO process) and (ii) the per-packet
acquisition phase phi_acq (gist regime: i.i.d. per packet; ESP-PPB regime:
small/stable)?  Observables:

  A  one-way cross-node residual at sensing-cadence gaps (coherence-first gate)
     — closed form Var[phase of OU-integral] + i.i.d. acq; MC spot-check
  B  two-way product residual vs reply interval (+ TX-jitter x CFO tolerance)
     — same closed form (cross-node accrual over the reply interval)
  C  directed-triangle closure residual vs slot spacing, raw and with a
     per-node window-mean CFO ledger (N=6, all 40 directed triangles) — MC

Simulation only: a NumPy model of the estimator, not a measurement.
Run:    /home/matt/projects/aquila-v2-spec/sim-python g1_observables.py
Output: g1_summary.csv, g1_figure.png
"""
import csv
import itertools

import numpy as np
import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

rng = np.random.default_rng(20261016)

C = 2.99792458e8
FC = 2.437e9                      # channel 6 centre
LAM = C / FC
NSUB = 52                          # HT20 subcarriers used for CSI
RHO_DB = 20.0                      # per-subcarrier post-average SNR
SIG_TH = 1.0 / np.sqrt(2.0 * NSUB * 10.0 ** (RHO_DB / 10.0))   # thermal floor

SIG_F = [1.0, 10.0, 30.0, 100.0]   # residual CFO 1-sigma per node, Hz (post-removal)
TAU_C = 1.0                        # OU correlation time, s
SIG_ACQ = [0.0, 0.05, 0.17, 1.0]   # i.i.d. per-packet acquisition phase, rad
N_MC = 2000
DT = 1.0e-4                         # OU integration step, s

GAPS_MS = [5.0, 10.0, 20.0, 50.0, 200.0]     # A: sensing-cadence gaps
REPLY_MS = [0.2, 1.0, 3.0, 10.0, 20.0]       # B: reply intervals
SLOT_MS = [1.0, 2.0, 5.0, 10.0]              # C: slot spacings


def sigma_drift_cross(sigma_f, tau_c, delta):
    """Std of cross-node relative PHASE accrual over gap `delta` (rad).

    Per node the residual LO phase is 2*pi * integral of an OU(s) process:
    Var[2 pi int_0^d f dt] = (2 pi)^2 * 2 s^2 tau (d - tau (1 - e^{-d/tau})).
    Cross-node difference doubles the variance.
    """
    var_one = 2.0 * sigma_f**2 * tau_c * (delta - tau_c * (1.0 - np.exp(-delta / tau_c)))
    return 2.0 * np.pi * np.sqrt(2.0 * var_one)


def mc_drift_cross(sigma_f, tau_c, delta, n=1000, dt=1e-4, rng=rng):
    """MC spot-check of sigma_drift_cross."""
    n_steps = int(round(delta / dt))
    e = np.exp(-dt / tau_c)
    sd = sigma_f * np.sqrt(1.0 - e * e)
    f = sigma_f * rng.standard_normal((n, 2))     # stationary init
    th = np.zeros((n, 2))
    for _ in range(n_steps):
        th = th + 2.0 * np.pi * f * dt
        f = e * f + sd * rng.standard_normal((n, 2))
    rel = th[:, 0] - th[:, 1]
    return float(np.std(rel, ddof=1))


rows = []

# --- Part A: one-way cross-node residual at sensing-cadence gaps -------------
for sf in SIG_F:
    for sa in SIG_ACQ:
        for g in GAPS_MS:
            d = g * 1e-3
            s = np.hypot(sigma_drift_cross(sf, TAU_C, d), np.sqrt(2.0) * sa)
            rows.append(dict(part="A_oneway", sigma_f_Hz=sf, sigma_acq_rad=sa,
                             param=f"gap_ms={g:g}", sigma_rad=float(s)))

# MC spot-check of the closed form (sf=30 Hz, gap=20 ms)
mc = mc_drift_cross(30.0, TAU_C, 0.02)
cf = sigma_drift_cross(30.0, TAU_C, 0.02)
rows.append(dict(part="A_mc_check", sigma_f_Hz=30.0, sigma_acq_rad=0.0,
                 param="gap_ms=20", sigma_rad=float(mc)))
rows.append(dict(part="A_cf_check", sigma_f_Hz=30.0, sigma_acq_rad=0.0,
                 param="gap_ms=20", sigma_rad=float(cf)))

# --- Part B: two-way product -----------------------------------------------
for sf in SIG_F:
    for sa in SIG_ACQ:
        for r in REPLY_MS:
            d = r * 1e-3
            # cross-node accrual over the reply interval + one acquisition at
            # each node (sqrt(2) i.i.d. acq)
            s = np.hypot(sigma_drift_cross(sf, TAU_C, d), np.sqrt(2.0) * sa)
            rows.append(dict(part="B_product", sigma_f_Hz=sf, sigma_acq_rad=sa,
                             param=f"reply_ms={r:g}", sigma_rad=float(s)))

# TX-jitter x CFO: reply-interval tolerance for a 0.3 rad product error
DF_RAW = 48e3      # Hz, +-20 ppm at 2.437 GHz
for df, tag in [(DF_RAW, "raw48kHz"), (100.0, "res100Hz"), (10.0, "res10Hz")]:
    tol = 0.3 / (2.0 * np.pi * df)
    rows.append(dict(part="B_jitter_tol", sigma_f_Hz=df, sigma_acq_rad=np.nan,
                     param=f"cfo_{tag}", sigma_rad=tol))

# --- Part C: directed-triangle closures, with per-node CFO ledger -----------
N_NODES = 6
TRIS = []
for c in itertools.combinations(range(N_NODES), 3):
    TRIS.append((c[0], c[1], c[2]))
    TRIS.append((c[0], c[2], c[1]))
TRIS = np.array(TRIS)
IJK = TRIS  # (T,3)

def ledger_mc(sf, sa, slot_ms, n_real=400):
    """Vectorised MC: closures at slots 0/s/2s; ledger = per-node window-mean CFO."""
    slot = slot_ms * 1e-3
    n_steps = int(round(3.0 * slot / DT))
    e = np.exp(-DT / TAU_C)
    sd = sf * np.sqrt(1.0 - e * e)
    f = sf * rng.standard_normal((n_real, N_NODES))   # stationary init
    th = np.zeros((n_real, N_NODES))
    fpath = np.zeros((n_real, N_NODES, n_steps))
    for k in range(n_steps):
        f = e * f + sd * rng.standard_normal((n_real, N_NODES))
        fpath[:, :, k] = f
        th = th + 2.0 * np.pi * f * DT
    # th accumulated from t=0; slots at 0, s, 2s -> phases at those indices
    i2 = int(round(slot / DT)); i3 = 2 * i2
    # re-walk to capture th at the slot indices (cheap second pass, vectorised)
    th = np.zeros((n_real, N_NODES))
    th_at = {}
    for k in range(n_steps):
        th = th + 2.0 * np.pi * fpath[:, :, k] * DT
        if k + 1 == i2:
            th_at[1] = th.copy()
        if k + 1 == i3:
            th_at[2] = th.copy()
    th0 = np.zeros((n_real, N_NODES))
    barf = fpath.reshape(n_real, N_NODES, -1)[:, :, :i3].mean(axis=2)
    ii, jj, kk = IJK[:, 0], IJK[:, 1], IJK[:, 2]
    # closure = (th_i0 - th_j0) + (th_j1 - th_k1) + (th_k2 - th_i2) + noise
    cl = (th0[:, ii] - th0[:, jj]) + (th_at[1][:, jj] - th_at[1][:, kk]) \
        + (th_at[2][:, kk] - th_at[2][:, ii])
    cl = cl + sa * np.sqrt(3.0) * rng.standard_normal(cl.shape)
    pred = 2.0 * np.pi * (barf[:, jj] * slot + barf[:, kk] * slot - 2.0 * barf[:, ii] * slot)
    return (float(np.std(cl, ddof=1)), float(np.std(cl - pred, ddof=1)),
            float(np.std(pred, ddof=1)))

for sf in SIG_F:
    for sa in SIG_ACQ:
        for s_ms in SLOT_MS:
            s_raw, s_cor, s_pred = ledger_mc(sf, sa, s_ms)
            rows.append(dict(part="C_closure_raw", sigma_f_Hz=sf, sigma_acq_rad=sa,
                             param=f"slot_ms={s_ms:g}", sigma_rad=s_raw))
            rows.append(dict(part="C_closure_ledger", sigma_f_Hz=sf, sigma_acq_rad=sa,
                             param=f"slot_ms={s_ms:g}", sigma_rad=s_cor))

rows.append(dict(part="thermal_floor", sigma_f_Hz=np.nan, sigma_acq_rad=np.nan,
                 param=f"NSUB={NSUB},rho_dB={RHO_DB}", sigma_rad=SIG_TH))

with open("g1_summary.csv", "w", newline="") as fh:
    w = csv.DictWriter(fh, fieldnames=["part", "sigma_f_Hz", "sigma_acq_rad", "param", "sigma_rad"])
    w.writeheader()
    for r in rows:
        w.writerow(r)

# --- figure -----------------------------------------------------------------
fig, axes = plt.subplots(1, 2, figsize=(9.0, 3.4))
ax = axes[0]
for sa in SIG_ACQ:
    ys = [next(r["sigma_rad"] for r in rows if r["part"] == "A_oneway"
                and r["sigma_acq_rad"] == sa and r["param"] == f"gap_ms={g:g}"
                and r["sigma_f_Hz"] == 10.0) for g in GAPS_MS]
    ax.plot(GAPS_MS, ys, "o-", label=f"acq={sa:g} rad")
ax.axhline(0.5, color="k", ls=":", lw=1)
ax.axhline(0.3, color="r", ls=":", lw=1)
ax.set_xscale("log"); ax.set_yscale("log")
ax.set_xlabel("sensing-frame gap (ms)"); ax.set_ylabel("cross-node residual (rad)")
ax.set_title("A: one-way, residual CFO 10 Hz")
ax.legend(fontsize=7)

ax = axes[1]
for sa in SIG_ACQ:
    ys = [next(r["sigma_rad"] for r in rows if r["part"] == "C_closure_ledger"
               and r["sigma_acq_rad"] == sa and r["param"] == f"slot_ms={s:g}"
               and r["sigma_f_Hz"] == 30.0) for s in SLOT_MS]
    ax.plot(SLOT_MS, ys, "s-", label=f"acq={sa:g} rad")
ax.axhline(0.5, color="k", ls=":", lw=1)
ax.axhline(0.3, color="r", ls=":", lw=1)
ax.set_xscale("log"); ax.set_yscale("log")
ax.set_xlabel("slot spacing (ms)"); ax.set_ylabel("closure residual (rad)")
ax.set_title("C: closure + ledger, residual CFO 30 Hz")
ax.legend(fontsize=7)
fig.tight_layout()
fig.savefig("g1_figure.png", dpi=110)
print("done; rows:", len(rows))
print("thermal floor rad:", SIG_TH)
print("A mc/cf check:", mc, cf)
