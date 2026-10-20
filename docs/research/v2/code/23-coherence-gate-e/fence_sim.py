#!/usr/bin/env python3
"""CG-1 fence simulation: the cross-node phase residual sigma_phi(T_gap).

SIMULATION ONLY (per docs/research/v2/EXPERIMENT-ENV.md honesty rules): a
NumPy evaluation of the mathematics of the phase process and estimator. No
quantity here is measured on ESP32-S3 hardware; every parameter is an
assumption drawn from the round-1 envelope (see note 23-coherence-gate-e.md).
The E1-CG experiment is what measures the real ones.

Truth process: relative LO phase to the master, m(t) = 2*pi*integral of an
Ornstein-Uhlenbeck residual-frequency process f(t) with stationary std sigma_f
and correlation time tau_f (post-acquisition: mean CFO removed by the
bootstrap). Reference frames every T_ref give a noisy, wrapped measurement of
m(t) with per-frame CSI phase noise sigma_meas.

Estimator (CG-1 fence):
  * bracketed (ESP-PPB pattern): de-rotate a sensing frame by the pin average
    of the two bracketing reference frames, m_hat = (y(t-h)+y(t+h))/2;
  * extrapolated (causal): matched [phase, frequency] Kalman on reference
    frames, dead-reckon to the sensing frame;
  * holdover: fence stops; dead-reckon from the last filtered state.

Cross-node residual = sqrt(2) x per-node residual (the master cancels in the
difference of two nodes' tracks).

Run:  /home/matt/projects/aquila-v2-spec/sim-python fence_sim.py
"""
import os

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd
from scipy.signal import lfilter

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "out")
os.makedirs(OUT, exist_ok=True)

TWOPI = 2.0 * np.pi
DT = 2.5e-4            # s, fine grid (0.25 ms)
DUR = 10.0             # s simulated per realisation
NREAL = 40
SEED = 42
T_STOP = 6.0           # s: the fence stops here (holdover measurement)

SIGMA_F = [2.4, 10.0, 24.0, 100.0]      # Hz residual frequency instability
TAU_F = [0.2, 2.0]                      # s frequency-wander correlation time
SIGMA_M = [0.01, 0.03, 0.1, 0.3]        # rad per-frame CSI phase noise
T_REF = [0.002, 0.005, 0.02]            # s reference-frame period

H_BRACK = [2.5e-4, 5e-4, 1e-3, 2.5e-3, 5e-3, 1e-2]   # s bracket half-gap
T_EXTRA = [2.5e-4, 5e-4, 1e-3, 2.5e-3, 5e-3, 1e-2]   # s extrapolation gap
T_HOLD = [0.02, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0]       # s holdover


def wraprad(x):
    return (x + np.pi) % (2 * np.pi) - np.pi


def coherence_budget(T, sf, tauf):
    """Closed-form untracked phase growth B(T) in rad (Wolfram-verified:
    Var = 2 sf^2 tau^2 (x - 1 + exp(-x)), x = T/tau)."""
    T = np.asarray(T, dtype=float)
    x = T / tauf
    var = 2.0 * sf**2 * tauf**2 * (x - 1.0 + np.exp(-x))
    return TWOPI * np.sqrt(var)


def ou_frequency(sf, tauf, nstep, nreal, rng):
    """Stationary OU frequency on the fine grid, shape (nreal, nstep)."""
    a = np.exp(-DT / tauf)
    sd = sf * np.sqrt(1.0 - a * a)
    w = rng.standard_normal((nreal, nstep)) * sd
    f = lfilter([1.0], [1.0, -a], w, axis=1)
    f0 = rng.standard_normal(nreal) * sf
    decay = a ** np.arange(nstep)
    return f + f0[:, None] * decay[None, :]


def simulate_truth(sf, tauf, rng):
    nstep = int(round(DUR / DT))
    f = ou_frequency(sf, tauf, nstep, NREAL, rng)
    return f, TWOPI * DT * np.cumsum(f, axis=1)


def bracket_residual(phi, sm, h):
    """Pin-interpolation residual for bracket half-gap h (per node, rad).

    m_hat(t) = (y(t-h)+y(t+h))/2, y = phi + noise:
      r = phi(t) - (phi(t-h)+phi(t+h))/2 - (w1+w2)/2
    Process term from the truth path; noise term 0.5*sm^2 added analytically.
    Non-overlapping evaluation points.
    """
    step = int(round(h / DT))
    n = phi.shape[1]
    idx = np.arange(step, n - step, 2 * step)
    d = phi[:, idx] - 0.5 * (phi[:, idx - step] + phi[:, idx + step])
    d = wraprad(d)
    return np.sqrt(np.var(d) + 0.5 * sm * sm)


def kalman_track(phi, ftrue, sf, tauf, sm, tref, rng):
    """Matched [phase, freq] Kalman over reference frames up to T_STOP.

    Exact discrete OU transition over tref (units: rad, Hz):
      f' = a f + n_f,            Var(n_f)  = sf^2 (1-a^2)
      m' = m + 2pi*tau*(1-a) f + n_m,
           Var(n_m) = (2pi)^2 sf^2 (2 tau^2 (x-1+e^-x) - tau^2 (1-a)^2)
           Cov(n_f, n_m) = 2pi sf^2 tau (1-a)^2
    Measurement: y = wrap(m + w), w ~ N(0, sm^2), H = [1, 0].

    Caveat the simulation exposes: per-frame sampling aliases the frequency
    state modulo 1/tref (a per-frame phase advance > pi is indistinguishable
    from a slower rotation plus a 2*pi cycle slip), so f_err_std flags the
    alias-locked regime; extrapolated de-rotation inherits 2*pi*k/tref*T_gap
    of error there, while bracketed de-rotation does not.
    """
    a = np.exp(-tref / tauf)
    x = tref / tauf
    var_nf = sf**2 * (1.0 - a * a)
    var_nm = (TWOPI**2) * sf**2 * (2 * tauf**2 * (x - 1.0 + np.exp(-x))
                                   - tauf**2 * (1 - a) ** 2)
    cov_fm = TWOPI * sf**2 * tauf * (1 - a) ** 2
    F = np.array([[1.0, TWOPI * tauf * (1 - a)], [0.0, a]])
    Q = np.array([[var_nm, cov_fm], [cov_fm, var_nf]])

    frame_step = int(round(tref / DT))
    last_frame = int(T_STOP / DT)
    idxs = np.arange(0, last_frame, frame_step)
    nframes = len(idxs)

    st = np.zeros((NREAL, 2))
    st[:, 1] = rng.standard_normal(NREAL) * sf
    P = np.zeros((NREAL, 2, 2))
    P[:, 0, 0] = np.pi**2
    P[:, 1, 1] = sf**2
    st_hist = np.zeros((nframes, NREAL, 2))
    slips = 0
    warm = int(0.5 / tref)

    meas = phi[:, idxs] + rng.standard_normal((NREAL, nframes)) * sm
    meas = wraprad(meas)

    for k in range(nframes):
        st = st @ F.T                                   # (N,2)
        P = F @ P @ F.T + Q                             # (N,2,2)
        innov = wraprad(meas[:, k] - st[:, 0])
        S = P[:, 0, 0] + sm * sm
        if k >= warm:
            slips += int(np.sum(np.abs(innov) > np.pi / 2))
        K = P[:, :, 0] / S[:, None]                      # (N,2)
        st = st + K * innov[:, None]
        P = P - K[:, :, None] * P[:, :1, :]              # (I - K H) P
        st_hist[k] = st

    # frequency-state error vs truth (reveals alias lock: |err| ~ 1/tref)
    nh = nframes // 2
    ferr = st_hist[nh:, :, 1] - ftrue[:, idxs[nh:]].T
    return {"idxs": idxs, "st": st_hist,
            "slipfrac": slips / (max(nframes - warm, 1) * NREAL),
            "f_err_std": float(np.std(ferr))}
def main():
    rng = np.random.default_rng(SEED)
    rows = []
    for sf in SIGMA_F:
        for tauf in TAU_F:
            f, phi = simulate_truth(sf, tauf, rng)
            for sm in SIGMA_M:
                brack = {h: bracket_residual(phi, sm, h) for h in H_BRACK}
                for tref in T_REF:
                    kt = kalman_track(phi, f, sf, tauf, sm, tref, rng)
                    idxs, st_hist = kt["idxs"], kt["st"]
                    row = dict(sigma_f=sf, tau_f=tauf, sigma_meas=sm,
                               t_ref=tref, slip_frac=kt["slipfrac"],
                               f_err_std=kt["f_err_std"])
                    for h in H_BRACK:
                        row[f"brack_{h*1e3:g}ms"] = np.sqrt(2.0) * brack[h]
                    # extrapolation residuals (causal), skip warmup, subsample
                    k0 = int(1.0 / tref) + 1
                    k1 = len(idxs) - int(0.5 / tref) - 1
                    ks = np.arange(k0, max(k1, k0 + 1), 4)
                    for tg in T_EXTRA:
                        if tg <= tref:
                            steps = int(round(tg / DT))
                            pred = (st_hist[ks, :, 0] + TWOPI * st_hist[ks, :, 1] * tg).T
                            obs = phi[:, idxs[ks] + steps]
                            row[f"extra_{tg*1e3:g}ms"] = \
                                np.sqrt(2.0) * np.std(wraprad(obs - pred))
                        else:
                            row[f"extra_{tg*1e3:g}ms"] = np.nan
                    # holdover: dead-reckon from the last filtered state
                    last = st_hist[-1]                  # (NREAL, 2)
                    for th in T_HOLD:
                        t_true = idxs[-1] + int(round(th / DT))
                        if t_true >= phi.shape[1]:
                            row[f"hold_{th:g}s"] = np.nan
                            continue
                        pred = last[:, 0] + TWOPI * last[:, 1] * th
                        row[f"hold_{th:g}s"] = \
                            np.sqrt(2.0) * np.std(wraprad(phi[:, t_true] - pred))
                    row["B_1ms"] = np.sqrt(2.0) * float(coherence_budget(1e-3, sf, tauf))
                    row["B_20ms"] = np.sqrt(2.0) * float(coherence_budget(2e-2, sf, tauf))
                    rows.append(row)
            print(f"done sf={sf} tau={tauf}")
    df = pd.DataFrame(rows)
    df.to_csv(os.path.join(OUT, "fence_sigma_phi.csv"), index=False)

    # ---- figure 1: the decisive curves -------------------------------
    fig, axes = plt.subplots(1, 2, figsize=(11, 4.2))
    ax = axes[0]
    sub = df[(df.sigma_meas == 0.1) & (df.tau_f == 2.0) & (df.t_ref == 0.005)]
    for sf in SIGMA_F:
        r = sub[sub.sigma_f == sf].iloc[0]
        ts = [t for t in T_EXTRA if t <= 0.005]
        ys = [r[f"extra_{t*1e3:g}ms"] for t in ts]
        ax.plot(np.array(ts) * 1e3, ys, "o-", label=f"$\\sigma_f$={sf:g} Hz")
        yh = [r[f"hold_{t:g}s"] for t in T_HOLD]
        ax.plot(np.array(T_HOLD) * 1e3, yh, "o--", alpha=0.5)
    ax.axhline(0.5, color="k", ls=":", lw=1)
    ax.text(300, 0.56, "0.5 rad gate")
    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.set_xlabel("gap since last reference frame (ms)")
    ax.set_ylabel(r"cross-node $\sigma_\phi$ (rad)")
    ax.set_title("extrapolate (solid) / holdover (dashed)\n"
                 r"$\sigma_m$=0.1 rad, $T_{ref}$=5 ms, $\tau_f$=2 s")
    ax.legend(fontsize=8)
    ax.grid(alpha=0.3)

    ax = axes[1]
    subb = df[(df.tau_f == 0.2) & (df.sigma_f == 100.0)]
    for sm in SIGMA_M:
        r = subb[subb.sigma_meas == sm].iloc[0]
        ax.plot(np.array(H_BRACK) * 1e3,
                [r[f"brack_{h*1e3:g}ms"] for h in H_BRACK], "s-",
                label=f"$\\sigma_m$={sm:g} rad")
    ax.axhline(0.5, color="k", ls=":", lw=1)
    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.set_xlabel("bracket half-gap h (ms)")
    ax.set_ylabel(r"cross-node $\sigma_\phi$ (rad)")
    ax.set_title("bracketed de-rotation\n"
                 r"$\sigma_f$=100 Hz, $\tau_f$=0.2 s (worst case)")
    ax.legend(fontsize=8)
    ax.grid(alpha=0.3)
    fig.tight_layout()
    fig.savefig(os.path.join(OUT, "fig1_fence.png"), dpi=110)
    print("wrote fig1_fence.png")

    # ---- closed-form check of the bracketed residual ------------------
    print("\nClosed-form bracket check (sigma_f=100 Hz, tau=0.2 s, h=1 ms, sm=0.03):")
    h = 1e-3
    B1 = float(coherence_budget(h, 100.0, 0.2))
    B2 = float(coherence_budget(2 * h, 100.0, 0.2))
    proc = np.sqrt(max(B1**2 - B2**2 / 4.0, 0.0))
    print(f"  closed form: {np.sqrt(2)*np.hypot(proc, 0.03/np.sqrt(2)):.4f} rad")
    r = df[(df.sigma_f == 100.0) & (df.tau_f == 0.2) & (df.sigma_meas == 0.03)].iloc[0]
    print(f"  simulated:   {r['brack_1ms']:.4f} rad")

    # ---- headline table for the note ----------------------------------
    cols = ["sigma_f", "tau_f", "sigma_meas", "t_ref", "f_err_std", "slip_frac",
            "brack_1ms", "extra_1ms", "hold_0.05s", "hold_0.5s", "B_20ms"]
    sel = df[(df.t_ref.isin([0.005, 0.02])) & (df.sigma_meas == 0.03)][cols]
    print("\nDesign-point table (sigma_meas=0.03 rad):")
    print(sel.to_string(index=False))


if __name__ == "__main__":
    main()
