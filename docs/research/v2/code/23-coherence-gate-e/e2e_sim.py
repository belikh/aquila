#!/usr/bin/env python3
"""CG-1 end-to-end chain: coherence -> localisation CRLB, and the
phase-swept beacon (round-1 missing direction f) ranging CRB.

SIMULATION ONLY (EXPERIMENT-ENV.md honesty rules): Cramer-Rao arithmetic on a
fixed geometry. Precision (CRLB), not resolution and not ambiguity-resolved
accuracy — the ambiguity cascade is handled in the note.

Modes compared for a target at x0 in a 5 x 4 x 2.7 m room:
  (a) coherent fleet (cross-node sigma_phi from the fence): master n0
      illuminates; receivers n1..n5 measure the bistatic echo phase relative to
      the direct path (per-node LoSRef-style referencing kills the master's
      unknown TX phase);
  (b) single conducted puck: 20 faces on a 0.30 m icosahedron at the same
      corner, face 0 illuminates, faces 1..19 receive (bistatic, conducted);
  (c) products-only (no cross-node coherence): 15 node-pairs give the doubled
      bistatic echo phase (the two-way product's target term), noise =
      sigma_prod from product_sim.py.

Run:  /home/matt/projects/aquila-v2-spec/sim-python e2e_sim.py
"""
import os

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "out")
os.makedirs(OUT, exist_ok=True)

C = 3e8
FC = 2.44e9
LAM = C / FC
TWOPI = 2 * np.pi

NODES = np.array([
    [0.15, 0.15, 2.55],   # n0 master (fleet)
    [4.85, 0.15, 2.55],
    [4.85, 3.85, 2.55],
    [0.15, 3.85, 2.55],
    [2.50, 0.15, 1.00],
    [2.50, 3.85, 1.00],
])
X0 = np.array([2.5, 2.0, 1.2])


def ico_face_centres(r=0.15):
    """20 face centres of an icosahedron on a sphere of radius r."""
    t = (1 + 5**0.5) / 2
    # canonical 12 vertices
    raw = [(0, t, -1), (0, t, 1), (0, -t, -1), (0, -t, 1),
           (t, -1, 0), (t, 1, 0), (-t, -1, 0), (-t, 1, 0),
           (-1, 0, t), (1, 0, t), (-1, 0, -t), (1, 0, -t)]
    verts = [np.array(v, float) for v in raw]
    # face centres via convex hull of the 12 vertices
    from scipy.spatial import ConvexHull
    pts = np.array(verts)
    pts = pts / np.linalg.norm(pts, axis=1, keepdims=True)
    hull = ConvexHull(pts)
    centres = pts[hull.simplices].mean(axis=1)
    centres = centres / np.linalg.norm(centres, axis=1, keepdims=True)
    return centres * r


def fisher_pos(fun_obs, sigma):
    """3D position CRLB (rms, m) for observation functions fun_obs(x)->(M,)."""
    eps = 1e-4
    J = np.empty((len(fun_obs(X0)), 3))
    for j in range(3):
        xp, xm = X0.copy(), X0.copy()
        xp[j] += eps
        xm[j] -= eps
        J[:, j] = (fun_obs(xp) - fun_obs(xm)) / (2 * eps)
    fim = J.T @ J / sigma**2
    cov = np.linalg.inv(fim)
    return float(np.sqrt(np.trace(cov)))


def main():
    # ---- mode (a): coherent fleet --------------------------------------
    n0 = NODES[0]

    def fleet_obs(x):
        d_master = np.linalg.norm(x - n0)
        out = []
        for n in NODES[1:]:
            out.append(TWOPI * (d_master + np.linalg.norm(x - n)
                                - np.linalg.norm(n0 - n)) / LAM)
        return np.array(out)

    # ---- mode (b): single conducted puck --------------------------------
    c0 = np.array([0.30, 0.30, 2.30])          # puck centre near the corner
    faces = ico_face_centres(0.15) + c0

    def puck_obs(x):
        d0 = np.linalg.norm(x - faces[0])
        out = [TWOPI * (d0 + np.linalg.norm(x - f)
                        - np.linalg.norm(faces[0] - f)) / LAM
               for f in faces[1:]]
        return np.array(out)

    # ---- mode (c): products-only (15 pairs) ------------------------------
    pairs = [(i, j) for i in range(6) for j in range(i + 1, 6)]

    def prod_obs(x):
        return np.array([2 * TWOPI * (np.linalg.norm(x - NODES[i])
                                      + np.linalg.norm(x - NODES[j])) / LAM
                          for i, j in pairs])

    r_fleet = np.linalg.norm(X0 - n0)
    print(f"target-to-master range: {r_fleet:.2f} m; "
          f"fleet aperture ~ {np.linalg.norm(NODES.max(0)-NODES.min(0)):.1f} m")

    sigmas = [0.05, 0.1, 0.2, 0.3, 0.5, 1.0]
    rows = []
    for s in sigmas:
        rows.append(dict(sigma_rad=s,
                         fleet_m=fisher_pos(fleet_obs, s),
                         puck_m=fisher_pos(puck_obs, s),
                         products_m=fisher_pos(prod_obs, s)))
    e2e = pd.DataFrame(rows)
    e2e.to_csv(os.path.join(OUT, "e2e_loc_crlb.csv"), index=False)
    print(e2e.to_string(index=False))

    # ---- phase-swept beacon (direction f): delay CRB + airtime ----------
    sweep_rows = []
    for M in [3, 5, 13]:
        spacing = 5e6
        b_rms = spacing * np.sqrt((M**2 - 1) / 12.0)
        for s in [0.05, 0.1, 0.3]:
            sig_tau = s / (TWOPI * b_rms * np.sqrt(M))
            sweep_rows.append(dict(M=M, b_rms_MHz=b_rms / 1e6, sigma_rad=s,
                                   sigma_tau_ps=sig_tau * 1e12,
                                   sigma_d_mm=3e8 * sig_tau * 1e3))
    sweep = pd.DataFrame(sweep_rows)
    sweep.to_csv(os.path.join(OUT, "swept_beacon_crb.csv"), index=False)
    print("\nPhase-swept beacon (5 MHz channel spacing):")
    print(sweep.to_string(index=False))

    duty_rows = []
    tframe = 190e-6
    for rate in [0.5, 1, 5, 10]:
        duty_rows.append(dict(sweep_hz=rate,
                               duty_pct=100 * 13 * rate * tframe))
    print("\nSweep airtime (13 ch x 190 us HT40 frame):")
    print(pd.DataFrame(duty_rows).to_string(index=False))

    # fence + round + sweep duty at the CG-1 design point
    print("\nCG-1 airtime design point:")
    print(f"  fence 200 Hz x 190 us  = {100*200*tframe:.1f} %")
    print(f"  round 50 Hz x 6 frames = {100*50*6*tframe:.1f} %")
    print(f"  sweep  1 Hz x 13 ch    = {100*1*13*tframe:.2f} %")
    print(f"  total                  = {100*tframe*(200+300+13):.1f} %")

    # ---- figure 4 --------------------------------------------------------
    fig, ax = plt.subplots(figsize=(6.2, 4.2))
    ax.plot(e2e.sigma_rad, e2e.fleet_m * 100, "o-", label="coherent fleet (5 m)")
    ax.plot(e2e.sigma_rad, e2e.products_m * 100, "s--", label="products-only (no x-node)")
    ax.plot(e2e.sigma_rad, e2e.puck_m * 100, "^:", label="single puck (0.3 m)")
    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.invert_yaxis()
    ax.set_xlabel(r"per-observation phase noise (rad)")
    ax.set_ylabel("3D position CRLB (cm)")
    ax.set_title("end-to-end: localisation precision vs phase residual")
    ax.legend(fontsize=8)
    ax.grid(alpha=0.3)
    fig.tight_layout()
    fig.savefig(os.path.join(OUT, "fig4_e2e.png"), dpi=110)
    print("wrote fig4_e2e.png")


if __name__ == "__main__":
    main()
