#!/usr/bin/env python
"""Round-2 T2 (character `d`): the end-to-end chain, checked numerically.

  calibration  : g2's product self-survey -> per-link direct-path phase known
                 to sigma_angle_h = k0 * sigma_survey (+ chain-asymmetry drift)
  coherence    : s's T2 "manufactured coherence" — a hub one-way frame lets
                 each receiver infer its own relative-LO phase from the
                 SURVEYED channel; residual = survey error (+) acq (+) OU
                 residual-CFO accrual over the transfer-to-use gap
  localisation : coherent near-field multistatic CRB of a person echo, with
                 the transferred phase error sigma_coh; compared with the
                 gauge-free product-domain BP of g3.

Also: static-scene (reciprocal reference holography) precision — delay-sum
positioning of a wall specular from the self-convolution peaks of the
two-way product, sigma from the stitched band.

Monte Carlo + Fisher, simulation only.
Run:    /home/matt/projects/aquila-v2-spec/sim-python g4_chain.py
Output: g4_summary.csv, g4_figure.png
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
K0 = 2.0 * np.pi / LAM

NODES = np.array([
    [0.5, 0.5, 2.4],
    [7.5, 0.5, 1.4],
    [7.6, 4.4, 2.3],
    [4.0, 4.6, 1.0],
    [0.4, 4.0, 2.1],
    [3.9, 0.4, 1.0],
])
N = len(NODES)
PT = np.array([4.0, 2.5, 1.2])          # person
HUB = 0                                  # node 0 is the hub

SIGMA_SURVEY = [0.003, 0.01, 0.03]       # m, range sigma from g2 (3 mm..3 cm)
SIGMA_ASYM = [0.05, 0.15]               # rad, chain-asymmetry residual
SIGMA_ACQ = [0.05, 0.17]                 # rad, per-packet acquisition phase
SIG_F = [10.0, 30.0, 100.0]              # Hz, residual CFO (OU, tau = 1 s)
TAU_C = 1.0
GAP_MS = [0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0]

rows = []


def sigma_drift_cross(sigma_f, tau_c, delta):
    var_one = 2.0 * sigma_f**2 * tau_c * (delta - tau_c * (1.0 - np.exp(-delta / tau_c)))
    return 2.0 * np.pi * np.sqrt(2.0 * var_one)


# --- Part 1: transfer residual sigma_coh vs gap, for the design grid --------
for s_surv in SIGMA_SURVEY:
    s_ang = K0 * s_surv                  # survey phase knowledge, rad
    for s_asym in SIGMA_ASYM:
        for s_acq in SIGMA_ACQ:
            for sf in SIG_F:
                for g in GAP_MS:
                    s_drift = sigma_drift_cross(sf, TAU_C, g * 1e-3)
                    # transfer frame acq (1x) + sensing frame acq (1x)
                    s_coh = float(np.sqrt(s_ang**2 + s_asym**2
                                           + 2.0 * s_acq**2 + s_drift**2))
                    rows.append(dict(part="transfer", survey_cm=s_surv * 100,
                                     asym=s_asym, acq=s_acq, resf_Hz=sf,
                                     gap_ms=g, sigma_coh=s_coh))

# --- Part 2: coherent near-field multistatic CRB vs sigma_coh ---------------
# observable: person echo phase at each node j from a hub transmission:
#   phi_j = k0 (r_H,p + r_p,j) + const ; Jacobian rows: k0*(u_Hp + u_pj)
# (u_Hp common to all rows; u_pj varies) — 3D Fisher for position.
u_Hp = (PT - NODES[HUB]) / np.linalg.norm(PT - NODES[HUB])
J = np.zeros((N - 1, 3))
jj = 0
for j in range(N):
    if j == HUB:
        continue
    u_pj = (NODES[j] - PT) / np.linalg.norm(NODES[j] - PT)
    J[jj] = K0 * (u_Hp + u_pj)
    jj += 1

SIG_COH_GRID = [0.05, 0.1, 0.2, 0.3, 0.5, 0.7, 1.0, 1.5, 2.0, 3.0]
for s_coh in SIG_COH_GRID:
    F = J.T @ J / s_coh**2
    cov = np.linalg.inv(F)
    # add the hub-leg range nuisance? The hub->person leg is common; position
    # CRB above already accounts for it via the Jacobian.
    sig3d = float(np.sqrt(np.trace(cov)))
    rows.append(dict(part="crb", survey_cm=np.nan, asym=np.nan, acq=np.nan,
                     resf_Hz=np.nan, gap_ms=np.nan, sigma_coh=s_coh,
                     crb3d_mm=sig3d * 1000))

# gauge-free comparator: person seen per link via products; the same Fisher
# but with 2x phase sensitivity and product noise sqrt(2) sigma_p:
# effective per-measurement sigma = sigma_p / sqrt(2) (2x signal, sqrt2 noise)
# -> scale J rows by 2 and noise by sqrt(2): net SNR gain 2x amplitude.
for s_p in [0.10, 0.25]:
    F = (2.0 * J).T @ (2.0 * J) / (np.sqrt(2.0) * s_p) ** 2
    cov = np.linalg.inv(F)
    rows.append(dict(part="crb_product", survey_cm=np.nan, asym=np.nan,
                     acq=np.nan, resf_Hz=np.nan, gap_ms=np.nan,
                     sigma_coh=s_p, crb3d_mm=float(np.sqrt(np.trace(cov))) * 1000))

# --- Part 3: static-scene delay-sum positioning (reciprocal holography) -----
# a wall specular at W; delay sums S_ij = (r_i,W + r_W,j)/c measured with
# sigma from the stitched band; position via Gauss-Newton over 15 links.
W = np.array([4.0, 5.0, 1.3])           # specular on the far wall
PAIRS = list(itertools.combinations(range(N), 2))
SIG_SUM = [0.02, 0.05, 0.10]            # m, delay-sum sigma (stitched band)
for s_sum in SIG_SUM:
    errs = []
    for _ in range(300):
        X = W + 0.3 * rng.standard_normal(3)
        for _it in range(30):
            r = []
            for (a, b) in PAIRS:
                pred = (np.linalg.norm(NODES[a] - X) + np.linalg.norm(X - NODES[b]))
                obs = (np.linalg.norm(NODES[a] - W) + np.linalg.norm(W - NODES[b]))
                r.append(obs - pred)
            r = np.array(r)
            Jw = np.zeros((len(PAIRS), 3))
            for k, (a, b) in enumerate(PAIRS):
                ua = (X - NODES[a]) / np.linalg.norm(X - NODES[a])
                ub = (NODES[b] - X) / np.linalg.norm(NODES[b] - X)
                Jw[k] = ua + ub
            step, *_ = np.linalg.lstsq(Jw, r + s_sum * rng.standard_normal(len(PAIRS)),
                                       rcond=None)
            X = X + step
        errs.append(float(np.linalg.norm(X - W)))
    rows.append(dict(part="static_specular", survey_cm=np.nan, asym=np.nan,
                     acq=np.nan, resf_Hz=np.nan, gap_ms=np.nan,
                     sigma_coh=np.nan, note=f"sig_sum={s_sum} m",
                     crb3d_mm=float(np.sqrt(np.mean(np.square(errs))) * 1000)))

with open("g4_summary.csv", "w", newline="") as fh:
    fieldn = ["part", "survey_cm", "asym", "acq", "resf_Hz", "gap_ms",
              "sigma_coh", "crb3d_mm", "note"]
    w = csv.DictWriter(fh, fieldnames=fieldn)
    w.writeheader()
    for r in rows:
        w.writerow(r)

# figure: transfer residual vs gap (design point) and CRB vs sigma_coh
fig, axes = plt.subplots(1, 2, figsize=(9.0, 3.4))
ax = axes[0]
for sf, ls in zip(SIG_F, ["o-", "s--", "^:"]):
    sel = [r for r in rows if r["part"] == "transfer" and r["resf_Hz"] == sf
           and r["survey_cm"] == 0.3 and r["asym"] == 0.15 and r["acq"] == 0.17]
    ax.plot([r["gap_ms"] for r in sel], [r["sigma_coh"] for r in sel], ls,
            label=f"resCFO {sf:g} Hz")
ax.axhline(0.5, color="r", ls=":", lw=1, label="0.5 rad gate")
ax.axhline(0.3, color="k", ls=":", lw=1, label="0.3 rad target")
ax.set_xscale("log"); ax.set_yscale("log")
ax.set_xlabel("transfer-to-use gap (ms)")
ax.set_ylabel("transferred-coherence residual (rad)")
ax.set_title("g4: manufactured coherence (T2)")
ax.legend(fontsize=7)

ax = axes[1]
sel = [r for r in rows if r["part"] == "crb"]
ax.plot([r["sigma_coh"] for r in sel], [r["crb3d_mm"] for r in sel], "o-",
        label="coherent one-way (sigma_coh)")
sel = [r for r in rows if r["part"] == "crb_product"]
ax.plot([r["sigma_coh"] for r in sel], [r["crb3d_mm"] for r in sel], "s",
        label="gauge-free product (sig_p)")
ax.set_xscale("log"); ax.set_yscale("log")
ax.set_xlabel("per-measurement phase noise (rad)")
ax.set_ylabel("3D position CRB (mm)")
ax.set_title("g4: near-field multistatic CRB, person at 3.9 m")
ax.legend(fontsize=7)
fig.tight_layout()
fig.savefig("g4_figure.png", dpi=110)

tr = [r for r in rows if r["part"] == "transfer"]
print("transfer rows:", len(tr))
for r in rows:
    if r["part"] in ("crb", "crb_product", "static_specular") or (
        r["part"] == "transfer" and r["gap_ms"] in (1.0, 5.0, 20.0)
        and r["acq"] == 0.17):
        print({k: (round(v, 4) if isinstance(v, float) else v) for k, v in r.items() if v == v})
