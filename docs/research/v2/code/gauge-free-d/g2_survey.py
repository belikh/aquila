#!/usr/bin/env python
"""Round-2 T2 (character `d`): geometry-first self-survey from two-way products.

Cascade (per node-pair, all 13 ISM channels, HT20 tones, K packets averaged):
  1  within-channel unwrap + slope fit  -> per-channel slope = -4 pi d/c
     (immune to per-channel chain offsets); median over channels -> d_hat1
  2  per-channel intercept (phase at channel centre) + sequential-difference
     unwrap across channels (steps ~ -4 pi d * 5 MHz / c, |.| < pi) -> global
     slope over 60 MHz -> d_hat2 (sensitive to per-channel offset residuals)
  3  weighted blend -> coarse d_hat; carrier integer fix (ambiguity lambda/2)
  4  weighted-LS anchor-free pose from the fixed ranges, scored by Procrustes.

Monte Carlo, simulation only (a NumPy model of the estimator).
Run:    /home/matt/projects/aquila-v2-spec/sim-python g2_survey.py
Output: g2_summary.csv, g2_figure.png
"""
import csv
import itertools

import numpy as np
import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

rng = np.random.default_rng(20261016)

C = 2.99792458e8
FC = 2.437e9
LAM = C / FC

NODES = np.array([
    [0.5, 0.5, 2.4],
    [7.5, 0.5, 1.4],
    [7.6, 4.4, 2.3],
    [4.0, 4.6, 1.0],
    [0.4, 4.0, 2.1],
    [3.9, 0.4, 1.0],
])                                     # 6 nodes, room 8 x 5 x 2.7 m
N = len(NODES)
PAIRS = list(itertools.combinations(range(N), 2))
D_TRUE = {p: float(np.linalg.norm(NODES[p[0]] - NODES[p[1]])) for p in PAIRS}

CH = 13
CH_CENTRES = 2.412e9 + 5e6 * np.arange(CH)
NTONE = 52
TONE_SPACING = 312.5e3

SIGMA_PHI = [0.10, 0.25, 0.50]         # per-packet product phase noise, rad
SIGMA_OFF = [0.0, 0.05, 0.20]          # per-channel chain-offset residual, rad
KPKT = [1, 4]                          # packets averaged per channel
N_MC = 150

rows = []


def unwrap_run(y):
    out = np.empty_like(y)
    out[0] = y[0]
    for k in range(1, len(y)):
        delta = (y[k] - out[k - 1] + np.pi) % (2 * np.pi) - np.pi
        out[k] = out[k - 1] + delta
    return out


def link_estimate(d, sigma_phi, sigma_off, kpkt, rng):
    """Return (d_hat1, d_hat2, d_coarse, d_fixed, fixed_ok, sigma_phi_est)."""
    slopes, intercepts = [], []
    for c in range(CH):
        f = CH_CENTRES[c] + (np.arange(NTONE) - (NTONE - 1) / 2) * TONE_SPACING
        ph = (-4.0 * np.pi * f * d / C
              + sigma_off * rng.standard_normal()          # per-channel offset
              + sigma_phi / np.sqrt(kpkt) * rng.standard_normal(NTONE))
        y = unwrap_run(ph)
        p = np.polyfit(f - f.mean(), y, 1)
        slopes.append(p[0])
        intercepts.append(p[1])          # phase at channel centre (x = 0)
    slopes = np.array(slopes)
    d1 = -np.median(slopes) * C / (4.0 * np.pi)
    # stage 2: cross-channel intercept differences, sequential unwrap
    ic = np.array(intercepts)
    steps = np.diff(ic)
    steps_u = (steps + np.pi) % (2 * np.pi) - np.pi          # each |.| < pi
    ic_u = np.concatenate([[ic[0]], ic[0] + np.cumsum(steps_u)])
    pc = np.polyfit(CH_CENTRES - CH_CENTRES.mean(), ic_u, 1)
    d2 = -pc[0] * C / (4.0 * np.pi)
    # blend: inverse-variance weights from analytic estimates
    sig1 = (sigma_phi / np.sqrt(kpkt)) * C / (4 * np.pi) / np.sqrt(
        CH * NTONE * (NTONE * TONE_SPACING) ** 2 / 12)
    sig2 = np.hypot(sigma_off, sigma_phi / np.sqrt(kpkt * NTONE)) * C / (4 * np.pi) / np.sqrt(
        CH * (CH * 5e6) ** 2 / 12)
    w1, w2 = 1 / sig1**2, 1 / sig2**2
    d_coarse = (w1 * d1 + w2 * d2) / (w1 + w2)
    # stage 3: carrier fix
    amb = LAM / 2.0
    phi_c = -4.0 * np.pi * FC * d / C + sigma_phi / np.sqrt(kpkt) * rng.standard_normal()
    d_carrier = (-(phi_c % (2 * np.pi))) / (4 * np.pi * FC / C)
    d_carrier = d_carrier % amb
    n_int = np.round((d_coarse - d_carrier) / amb)
    d_fixed = d_carrier + n_int * amb
    ok = abs(d_fixed - d) < amb / 4
    return d1, d2, d_coarse, d_fixed, ok, sigma_phi / np.sqrt(kpkt)


for sigma_phi in SIGMA_PHI:
    for sigma_off in SIGMA_OFF:
        for kpkt in KPKT:
            e1, e2, ec = [], [], []
            fixed_all = []
            pose_err = []
            for _ in range(N_MC):
                per_link = {}
                for p in PAIRS:
                    d1, d2, dc, df, ok, _ = link_estimate(
                        D_TRUE[p], sigma_phi, sigma_off, kpkt, rng)
                    per_link[p] = (d1, d2, dc, df, ok)
                    if p == (0, 1):
                        e1.append(d1 - D_TRUE[p])
                        e2.append(d2 - D_TRUE[p])
                        ec.append(dc - D_TRUE[p])
                fixed_all.append(np.mean([v[4] for v in per_link.values()]))
                obs = [(p, v[3]) for p, v in per_link.items() if v[4]]
                if len(obs) < 12:
                    pose_err.append(np.nan)
                    continue
                X = NODES + 0.05 * rng.standard_normal(NODES.shape)
                for _it in range(40):
                    RX = np.array([np.linalg.norm(X[a] - X[b]) for (a, b), _ in obs])
                    r = np.array([dd - rr for (_, dd), rr in zip(obs, RX)])
                    J = np.zeros((len(obs), 3 * N))
                    for k, ((a, b), _) in enumerate(obs):
                        u = (X[a] - X[b]) / RX[k]
                        # distance Jacobian (d|xa-xb|/dx): +u at a, -u at b;
                        # GN step solves Jf * step = (obs - predicted)
                        J[k, 3 * a:3 * a + 3] = u
                        J[k, 3 * b:3 * b + 3] = -u
                    # minimum-norm GN step: stays in the row space, so the
                    # 6-dim rigid gauge never accumulates a divergent update
                    step, *_ = np.linalg.lstsq(J, r, rcond=None)
                    X = (X.reshape(-1) + step).reshape(N, 3)
                Xc = X - X.mean(0)
                Tc = NODES - NODES.mean(0)
                U, S, Vt = np.linalg.svd(Xc.T @ Tc)
                Xa = Xc @ (U @ Vt)
                pose_err.append(float(np.sqrt(((Xa - Tc) ** 2).sum(1).mean())))
            rows.append(dict(
                sigma_phi=sigma_phi, sigma_off=sigma_off, kpkt=kpkt,
                d1_rmse_cm=float(np.sqrt(np.mean(np.square(e1))) * 100),
                d2_rmse_cm=float(np.sqrt(np.mean(np.square(e2))) * 100),
                coarse_rmse_cm=float(np.sqrt(np.mean(np.square(ec))) * 100),
                fix_rate=float(np.mean(fixed_all)),
                pose_rmse_cm=float(np.nanmean(pose_err)) * 100,
                pose_p90_cm=float(np.nanpercentile(pose_err, 90)) * 100))

rows.append(dict(sigma_phi=np.nan, sigma_off=np.nan, kpkt=np.nan,
                 d1_rmse_cm=np.nan, d2_rmse_cm=np.nan, coarse_rmse_cm=np.nan,
                 fix_rate=np.nan, pose_rmse_cm=np.nan, pose_p90_cm=np.nan,
                 note=(f"carrier amb={LAM/2*100:.2f} cm; per-node range sigma = "
                       f"LAM*sigma_phi/(4 pi): "
                       f"{[round(LAM*s/(4*np.pi)*100, 3) for s in SIGMA_PHI]} cm")))
rows.append(dict(sigma_phi=np.nan, sigma_off=np.nan, kpkt=np.nan,
                 d1_rmse_cm=np.nan, d2_rmse_cm=np.nan, coarse_rmse_cm=np.nan,
                 fix_rate=np.nan, pose_rmse_cm=np.nan, pose_p90_cm=np.nan,
                 note=("geometry: 6 nodes 8x5x2.7 m; 15 links; 13 ch x 52 tones; "
                       "within-channel unwrap step 4 pi d 312.5 kHz / c = "
                       f"{4*np.pi*12*312.5e3/C:.3f} rad/tone at 12 m")))

with open("g2_summary.csv", "w", newline="") as fh:
    w = csv.DictWriter(fh, fieldnames=["sigma_phi", "sigma_off", "kpkt", "d1_rmse_cm",
                                       "d2_rmse_cm", "coarse_rmse_cm", "fix_rate",
                                       "pose_rmse_cm", "pose_p90_cm", "note"])
    w.writeheader()
    for r in rows:
        w.writerow(r)

fig, axes = plt.subplots(1, 2, figsize=(8.6, 3.4))
ax = axes[0]
for k in KPKT:
    sel = [r for r in rows if r["kpkt"] == k and r["sigma_phi"] == 0.25]
    ax.plot([r["sigma_off"] for r in sel], [r["fix_rate"] for r in sel],
            "o-", label=f"K={k} pkt, prod phase 0.25 rad")
ax.set_xlabel("per-channel offset residual (rad)")
ax.set_ylabel("carrier-fix success rate")
ax.set_ylim(0, 1.05)
ax.legend(fontsize=7)
ax.set_title("g2: cascade fix rate")
ax = axes[1]
for k in KPKT:
    sel = [r for r in rows if r["kpkt"] == k and r["sigma_off"] == 0.05]
    ax.plot([r["sigma_phi"] for r in sel], [r["pose_rmse_cm"] for r in sel],
            "s-", label=f"K={k} pkt, off 0.05 rad")
ax.set_xlabel("per-packet product phase noise (rad)")
ax.set_ylabel("node pose RMSE (cm)")
ax.legend(fontsize=7)
ax.set_title("g2: anchor-free pose accuracy")
fig.tight_layout()
fig.savefig("g2_figure.png", dpi=110)

for r in rows:
    print({k: (round(v, 3) if isinstance(v, float) else v) for k, v in r.items()})
