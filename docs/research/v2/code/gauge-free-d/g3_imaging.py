#!/usr/bin/env python
"""Round-2 T2 (character `d`): gauge-free imaging — what "sharp" survives.

Compares, on the same 6-node room geometry and the same target model:
  (i)  amplitude-only RTI (Wilson-Patwari kernel; needs no geometry at all)
  (ii) coherent back-projection in the two-way-product domain
       ("reciprocal reference holography", missing direction j): the per-link
       differential product  p(t) p*(t0) / |p|  gives  2 h_p/h_d  with a KNOWN
       reference phase (surveyed direct path), so links sum coherently with
       no cross-node coherence.  The back-projected PSF across ~7 m room
       baselines at 2.44 GHz is ~2-9 cm wide, so the BP grid is 0.02 m.
  (iii) ideal one-way coherent back-projection (assumes cross-node carrier
        coherence at SIG_COH) — the coherence-first comparator; same PSF,
        half the person sensitivity (eps, not 2 eps).

Person model: point scatterer, echo amplitude ratio eps = |h_p/h_d| swept
-60..-14 dB.  Product phase noise sig_p (from g1) swept.  K frames averaged.

Monte Carlo, simulation only.
Run:    /home/matt/projects/aquila-v2-spec/sim-python g3_imaging.py
Output: g3_summary.csv, g3_figure.png
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
PAIRS = list(itertools.combinations(range(N), 2))
LINKS = np.array(PAIRS)
D_IJ = np.linalg.norm(NODES[LINKS[:, 0]] - NODES[LINKS[:, 1]], axis=1)

ROOM = (8.0, 5.0)
Z_T = 1.2
# fine grid for coherent BP (PSF ~ 2-9 cm)
STEP_F = 0.02
GXf, GYf = np.meshgrid(np.arange(0.2, ROOM[0] - 0.2 + 1e-9, STEP_F),
                       np.arange(0.2, ROOM[1] - 0.2 + 1e-9, STEP_F))
GPTS_F = np.column_stack([GXf.ravel(), GYf.ravel(), np.full(GXf.size, Z_T)])
# coarse grid for RTI (kernels ~ 1 m)
STEP_C = 0.2
GXc, GYc = np.meshgrid(np.arange(0.3, ROOM[0] - 0.3 + 1e-9, STEP_C),
                       np.arange(0.3, ROOM[1] - 0.3 + 1e-9, STEP_C))
GPTS_C = np.column_stack([GXc.ravel(), GYc.ravel(), np.full(GXc.size, Z_T)])

TEST_PTS = np.array([
    [2.0, 1.6, Z_T], [4.0, 2.5, Z_T], [6.0, 3.4, Z_T],
    [2.8, 3.8, Z_T], [5.4, 1.2, Z_T],
])

EPS_DB = [-60, -54, -48, -42, -36, -30, -24, -18, -14]
SIG_P = [0.10, 0.25]
SIG_COH = 0.25
KFRAMES = [1, 20]
N_MC = 40
ACQ_R = 0.30                     # "acquisition" radius, m


def geom(p):
    ri = np.linalg.norm(NODES[LINKS[:, 0]] - p, axis=1)
    rj = np.linalg.norm(NODES[LINKS[:, 1]] - p, axis=1)
    return ri, rj


def excess(p):
    return geom(p)[0] + geom(p)[1] - D_IJ


STEER_F = np.exp(-1j * K0 * np.array([excess(p) for p in GPTS_F]))

# RTI kernel on the coarse grid
def rti_kernel(p, extra=0.15):
    ri, rj = geom(p)
    inside = (ri + rj) <= (D_IJ + LAM + extra)
    return np.where(inside, 1.0 / np.sqrt(np.maximum(D_IJ, 0.1)), 0.0)


W_GRID = np.array([rti_kernel(p) for p in GPTS_C])      # (npts_c, nlinks)
A_RTI = W_GRID.T
PINV_RTI = np.linalg.solve(A_RTI.T @ A_RTI + 1e-2 * np.eye(A_RTI.shape[1]), A_RTI.T)


def rti_localise(y):
    return GPTS_C[int(np.argmax(PINV_RTI @ y))]


def bp_localise(m):
    return GPTS_F[int(np.argmax(np.abs(STEER_F @ np.conj(m))))]


rows = []
for eps_db in EPS_DB:
    for sig_p in SIG_P:
        for kf in KFRAMES:
            eps = 10 ** (eps_db / 20.0)
            res = {k: [] for k in ("rti", "bp", "ow")}
            for pt in TEST_PTS:
                th = -K0 * excess(pt)
                for _ in range(N_MC):
                    se = np.sqrt(2.0) * sig_p / np.sqrt(kf)
                    m = 2.0 * eps * np.exp(1j * th) \
                        + se / np.sqrt(2.0) * (rng.standard_normal(len(LINKS))
                                               + 1j * rng.standard_normal(len(LINKS)))
                    e = bp_localise(m)[:2] - pt[:2]
                    res["bp"].append(float(np.hypot(*e)))
                    w = rti_kernel(pt)
                    y = 3.0 * w / max(w.max(), 1e-9) \
                        + 0.5 / np.sqrt(kf) * rng.standard_normal(len(LINKS))
                    e = rti_localise(y)[:2] - pt[:2]
                    res["rti"].append(float(np.hypot(*e)))
                    se2 = np.sqrt(2.0) * SIG_COH / np.sqrt(kf)
                    m2 = eps * np.exp(1j * th) \
                        + se2 / np.sqrt(2.0) * (rng.standard_normal(len(LINKS))
                                                 + 1j * rng.standard_normal(len(LINKS)))
                    e = bp_localise(m2)[:2] - pt[:2]
                    res["ow"].append(float(np.hypot(*e)))
            row = dict(eps_db=eps_db, sig_p=sig_p, kframes=kf)
            for k, v in res.items():
                v = np.array(v)
                row[f"{k}_acq"] = float(np.mean(v < ACQ_R))
                row[f"{k}_m"] = float(np.mean(v))
                row[f"{k}_med"] = float(np.median(v))
            rows.append(row)

with open("g3_summary.csv", "w", newline="") as fh:
    w = csv.DictWriter(fh, fieldnames=list(rows[0].keys()))
    w.writeheader()
    for r in rows:
        w.writerow(r)

fig, axes = plt.subplots(1, 2, figsize=(9.0, 3.5))
for sig_p, mk in zip(SIG_P, ["o-", "s--"]):
    sel = [r for r in rows if r["sig_p"] == sig_p and r["kframes"] == 20]
    axes[0].plot([r["eps_db"] for r in sel], [r["bp_m"] for r in sel], mk,
                 label=f"product BP (sig_p {sig_p:g})")
    axes[1].plot([r["eps_db"] for r in sel], [r["bp_acq"] for r in sel], mk,
                 label=f"product BP (sig_p {sig_p:g})")
sel = [r for r in rows if r["sig_p"] == 0.25 and r["kframes"] == 20]
axes[0].plot([r["eps_db"] for r in sel], [r["ow_m"] for r in sel], "^-",
             label="one-way coherent (0.25 rad)")
axes[0].plot([r["eps_db"] for r in sel], [r["rti_m"] for r in sel], "v-",
             label="amplitude RTI")
axes[1].plot([r["eps_db"] for r in sel], [r["ow_acq"] for r in sel], "^-",
             label="one-way coherent")
axes[1].plot([r["eps_db"] for r in sel], [r["rti_acq"] for r in sel], "v-",
             label="amplitude RTI")
for ax in axes:
    ax.set_xlabel("person echo ratio |h_p/h_d| (dB)")
    ax.legend(fontsize=7)
axes[0].set_ylabel("2D localisation mean error (m)")
axes[0].set_yscale("log")
axes[0].axhline(0.5, color="k", ls=":", lw=1)
axes[1].set_ylabel("acquisition rate (err < 0.3 m)")
axes[1].set_ylim(0, 1.05)
axes[0].set_title("g3: imaging modes (6 nodes, 20 frames)")
axes[1].set_title("g3: acquisition rate")
fig.tight_layout()
fig.savefig("g3_figure.png", dpi=110)

for r in rows:
    print({k: (round(v, 3) if isinstance(v, float) else v) for k, v in r.items()})
