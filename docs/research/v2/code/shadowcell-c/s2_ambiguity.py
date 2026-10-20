#!/usr/bin/env python
"""Round-2 T2 (character `c`): the ambiguity cascade of the ShadowCell mesh.

SIMULATION ONLY (NumPy model; not a measurement). Parameters printed below.

Parts
  A  single-link integer-fix probability (closed form) for wrap W = lambda/2.
  B  monostatic delay-estimate quality (ML delay fit on s1's residual-CSI
     model) -> the coarse stage of the target cascade.
  C  target-integer fixing over the monostatic mesh: P(success) vs number of
     pucks, coarse-range quality sigma_c and phase noise sigma_phi.
  D  mesh self-survey integer bootstrap from two-way-product ranges with FTM
     coarse priors (FTM only / +one taped link / anchor-grade coarse).

Outputs: printed tables, s2_ambiguity.csv, s2_ambiguity.png.

Run:  /home/matt/projects/aquila-v2-spec/sim-python s2_ambiguity.py
"""
import csv
from math import erf

import numpy as np

rng = np.random.default_rng(20261017)

C = 2.99792458e8
FC = 2.437e9
LAM = C / FC
W = LAM / 2.0                       # carrier-phase range wrap: 6.147 cm
PUCKS = np.array([
    [0.5, 0.5, 2.5], [7.5, 0.5, 2.5], [7.5, 5.5, 2.5], [0.5, 5.5, 2.5],
    [4.0, 0.5, 1.2], [4.0, 5.5, 1.2], [0.5, 3.0, 0.3], [7.5, 3.0, 0.3]])
TARGET = np.array([3.5, 2.8, 1.5])
ROOM_CENTRE = np.array([4.0, 3.0, 1.5])

print(f"parameters: fc=2.437 GHz, lambda={LAM:.4f} m, W=lambda/2={W:.4f} m")

# ------------------------------------------------- Part A: single-link P_fix
print("\n== Part A: single-link integer-fix probability, W=%.3f m ==" % W)
rowsA = []
for sc in [0.005, 0.01, 0.02, 0.03, 0.05, 0.1, 0.3, 0.5, 1.5]:
    v = erf(W / (2 * np.sqrt(2) * sc))
    rowsA.append((sc, v))
    print(f"  sigma_c={sc:6.3f} m -> P_fix={v*100:6.2f} %")

# ----------------------- Part B: monostatic delay-estimate quality (ML fit)
PT_DBM, G_DBI, NF_DB = 19.5, 2.0, 9.0
NS, DF, B_HZ = 56, 312.5e3, 20e6
N0_SUB_DBM = -174 + 10*np.log10(B_HZ) + NF_DB - 10*np.log10(NS)
PHI = (1 + np.sqrt(5)) / 2
verts = [(s1, s2, s3) for s1 in (+1, -1) for s2 in (+1, -1) for s3 in (+1, -1)]
for a in (+1, -1):
    for b in (+1, -1):
        verts += [(0, a/PHI, b*PHI), (a/PHI, b*PHI, 0), (a*PHI, 0, b*PHI)]
faces = np.array(verts, float)
faces = faces / np.linalg.norm(faces[0]) * 0.15

def echo_alpha(d, rcs, I):
    tgt = np.array([d, 0.0, 0.0])
    d0 = np.linalg.norm(tgt - faces[0])
    dj = np.linalg.norm(tgt - faces, axis=1)
    pe = (PT_DBM + 2*G_DBI + 20*np.log10(LAM) + 10*np.log10(rcs)
          - 30*np.log10(4*np.pi) - 20*np.log10(d0) - 20*np.log10(dj))
    return 10 ** ((pe - (PT_DBM - I)) / 20.0)

def delay_fit_mc(d=4.0, rcs=0.3, I=45.0, sig_phi_leak=0.02, n_real=500):
    """ML delay fit on one residual CSI frame; returns range bias, std [m]."""
    alpha = np.delete(echo_alpha(d, rcs, I), 0)[:19]
    tau = np.delete((d + np.linalg.norm(np.array([d, 0, 0]) - faces, axis=1)) / C, 0)[:19]
    k = np.arange(NS) - NS // 2
    n_sub = 10 ** ((N0_SUB_DBM - ((PT_DBM - I) - 10*np.log10(NS))) / 20)
    grid = np.linspace(0.05, 12.0, 2400)              # candidate ranges [m]
    Psub = np.exp(-2j*np.pi*np.outer(tau, k*DF))      # per-face echo phases
    Pgrid = np.exp(-2j*np.pi*np.outer(2*grid/C, k*DF))
    u = np.exp(1j*rng.uniform(0, 2*np.pi, 19))
    leak = u[:, None] * sig_phi_leak * np.ones((1, NS))
    est = np.zeros(n_real)
    for r in range(n_real):
        noise = n_sub/np.sqrt(2)*(rng.standard_normal((19, NS))
                                   + 1j*rng.standard_normal((19, NS)))
        res = alpha[:, None]*Psub + leak + noise
        S = np.einsum('gk,fk,fk->g', np.conj(Pgrid), res, np.conj(Psub))
        est[r] = grid[np.argmax(np.abs(S))]
    e = est - d
    return float(e.mean()), float(e.std())

print("\n== Part B: ML delay-estimate quality (design point d=4 m, rcs=0.3, I=45) ==")
print("  (one frame; leakage sigma_phi = the s1 residual-wander parameter)")
for spl in [0.0, 0.02, 0.05, 0.1]:
    bias, std = delay_fit_mc(sig_phi_leak=spl)
    print(f"  sigma_phi={spl:4.2f}: range bias={bias*100:+6.2f} cm  std={std*100:6.2f} cm")

# ------------------------------------- Part C: target integer fixing over mesh
def gn_trilaterate(x, ranges_to_pucks, puck_pos, iters=15):
    """Gauss-Newton trilateration, vectorised over trials."""
    for _ in range(iters):
        diff = x[:, None, :] - puck_pos[None, :, :]
        d = np.linalg.norm(diff, axis=2)
        J = diff / np.maximum(d, 1e-9)[:, :, None]
        resid = (ranges_to_pucks - d)[:, :, None]
        g = np.einsum('nij,njk->nik', J.transpose(0, 2, 1), resid)
        H = np.einsum('nij,nik->njk', J.transpose(0, 2, 1), J)
        x = x + np.linalg.solve(H + 1e-9*np.eye(3), g)[:, :, 0]
    return x

def fix_target_mc(n_puck, sigma_c, sig_phi_rad, n_trial=600, jitter=0.5):
    pucks = PUCKS[:n_puck]
    ok = 0
    err_all = []
    n_done = 0
    while n_done < n_trial:
        n = min(200, n_trial - n_done)
        tgt = TARGET[None, :] + jitter*rng.standard_normal((n, 3))
        d_true = np.linalg.norm(tgt[:, None, :] - pucks[None, :, :], axis=2)
        phi = (4*np.pi*d_true/LAM
               + sig_phi_rad*rng.standard_normal(d_true.shape)) % (2*np.pi)
        coarse = d_true + sigma_c*rng.standard_normal(d_true.shape)
        pr = phi*LAM/(4*np.pi)
        # honest init: room centre + 1 m jitter, then coarse trilateration
        x = gn_trilaterate(ROOM_CENTRE[None, :]
                           + 1.0*rng.standard_normal((n, 3)), coarse, pucks)
        n_int = np.zeros_like(coarse)
        for _ in range(12):
            d_hat = np.linalg.norm(x[:, None, :] - pucks[None, :, :], axis=2)
            n_int = np.round((pr - d_hat) / W)
            x = gn_trilaterate(x, pr - n_int*W, pucks)
        err = np.linalg.norm(x - tgt, axis=1)
        n_true = np.round((d_true - pr) / W)
        good = (np.abs(n_int - n_true).max(axis=1) == 0) & (err < 0.05)
        ok += good.sum()
        err_all.append(err)
        n_done += n
    return ok / n_done, np.concatenate(err_all)

print("\n== Part C: target integer-fix success vs (N_pucks, sigma_c, sigma_phi) ==")
print("  sigma_phi: 0.03 rad (0.29 mm), 0.1 rad (0.98 mm), 0.3 rad (2.9 mm)")
rowsC = []
for n_puck in [4, 6, 8]:
    for sc in [0.007, 0.02, 0.05, 0.2, 0.75]:
        line = []
        for sp in [0.03, 0.1, 0.3]:
            p, _ = fix_target_mc(n_puck, sc, sp)
            line.append(p)
            rowsC.append((n_puck, sc, sp, p))
        print(f"  N={n_puck} sigma_c={sc:5.3f} m: " +
              "  ".join(f"P={v*100:6.1f}%" for v in line))

# --------------------------- Part D: mesh self-survey integer bootstrap (FTM)
def kabsch(P, Q):
    Pc, Qc = P - P.mean(0), Q - P.mean(0)
    U, _, Vt = np.linalg.svd(Pc.T @ Qc)
    d = np.sign(np.linalg.det(Vt.T @ U.T))
    R = Vt.T @ np.diag([1, 1, d]) @ U.T
    return (R @ Pc.T).T + Q.mean(0)

def classical_mds(D2):
    n = D2.shape[0]
    Jm = np.eye(n) - np.ones((n, n))/n
    B = -0.5 * Jm @ (D2**2) @ Jm
    w, v = np.linalg.eigh(B)
    idx = np.argsort(w)[::-1][:3]
    return v[:, idx] * np.sqrt(np.maximum(w[idx], 0))

def gn_survey(x, pair_ranges, ii, jj, iters=20):
    n = x.shape[0]
    for _ in range(iters):
        d = np.linalg.norm(x[ii] - x[jj], axis=1)
        J = np.zeros((len(ii), 3*n))
        for a, (i, j) in enumerate(zip(ii, jj)):
            u_ = (x[i] - x[j]) / max(d[a], 1e-9)
            J[a, 3*i:3*i+3] = u_
            J[a, 3*j:3*j+3] = -u_
        dx = np.linalg.lstsq(J, pair_ranges - d, rcond=None)[0]
        x = x + dx.reshape(n, 3)
    return x

def survey_mc(sigma_ftm, sigma_phi_rad=0.1, n_node=6, n_trial=150,
              taped=None, anchor_sigma=None):
    nodes = PUCKS[:n_node]
    ii, jj = np.triu_indices(n_node, 1)
    d_true = np.linalg.norm(nodes[ii] - nodes[jj], axis=1)
    ok = 0
    rmse = []
    for _ in range(n_trial):
        phi = (4*np.pi*d_true/LAM
               + sigma_phi_rad*rng.standard_normal(len(ii))) % (2*np.pi)
        coarse = d_true + sigma_ftm*rng.standard_normal(len(ii))
        if anchor_sigma is not None:
            coarse = d_true + anchor_sigma*rng.standard_normal(len(ii))
        if taped is not None:
            a, b = taped
            m = int(np.where((ii == a) & (jj == b))[0][0])
            coarse[m] = d_true[m] + 0.01*rng.standard_normal()
        x0 = classical_mds(coarse)          # honest init: coarse MDS only
        x = gn_survey(x0, coarse, ii, jj)
        n_int = np.zeros(len(ii))
        for _ in range(12):
            d_hat = np.linalg.norm(x[ii] - x[jj], axis=1)
            n_int = np.round((phi*LAM/(4*np.pi) - d_hat) / W)
            x = gn_survey(x, phi*LAM/(4*np.pi) - n_int*W, ii, jj)
        n_true = np.round((d_true - phi*LAM/(4*np.pi)) / W)
        err = np.linalg.norm(kabsch(x, nodes) - nodes, axis=1)
        ok += int((np.abs(n_int - n_true).max() == 0) and (err.mean() < 0.03))
        rmse.append(err.mean())
    return ok / n_trial, float(np.mean(rmse))

print("\n== Part D: mesh self-survey integer bootstrap (6 nodes, 15 links) ==")
rowsD = []
for sftm in [0.5, 1.0, 2.0]:
    p, e = survey_mc(sftm)
    rowsD.append((f"FTM sigma={sftm:g} m", p, e))
    print(f"  FTM-only sigma={sftm:g} m: P_survey={p*100:5.1f}%  shape RMSE={e*100:6.1f} cm")
p, e = survey_mc(1.0, taped=(0, 1))
rowsD.append(("FTM 1 m + one taped link", p, e))
print(f"  FTM 1 m + one taped link (1 cm): P_survey={p*100:5.1f}%  shape RMSE={e*100:6.1f} cm")
for anch in [0.03, 0.05, 0.10]:
    p, e = survey_mc(1.0, anchor_sigma=anch)
    rowsD.append((f"anchor-grade coarse {anch:g} m", p, e))
    print(f"  anchor-grade coarse sigma={anch:g} m: P_survey={p*100:5.1f}%  shape RMSE={e*100:6.1f} cm")

# ---------------------------------------------------------------- write CSV
with open("s2_ambiguity.csv", "w", newline="") as fh:
    w = csv.writer(fh)
    w.writerow(["part", "params", "value", "units"])
    for sc, v in rowsA:
        w.writerow(["A_single_link", f"sigma_c={sc:g}", f"{v:.5f}", "prob"])
    for n_puck, sc, sp, p in rowsC:
        w.writerow(["C_target_fix", f"N={n_puck},sc={sc:g},sp={sp:g}", f"{p:.4f}", "prob"])
    for name, p, e in rowsD:
        w.writerow(["D_survey", name, f"{p:.4f}", "prob"])
        w.writerow(["D_survey_rmse", name, f"{e:.5f}", "m"])

try:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    fig, ax = plt.subplots(1, 2, figsize=(9, 3.4))
    for n_puck in [4, 6, 8]:
        for sp, mk in [(0.03, "o"), (0.1, "s"), (0.3, "^")]:
            xs = [sc for (n, sc, s, _) in rowsC if n == n_puck and s == sp]
            ys = [p*100 for (n, sc, s, p) in rowsC if n == n_puck and s == sp]
            ax[0].semilogx(xs, ys, marker=mk, ms=3,
                           label=f"N={n_puck}" if sp == 0.03 else None)
    ax[0].set_xlabel("coarse-range sigma [m]")
    ax[0].set_ylabel("P(fix) [%]")
    ax[0].set_title("C. target integer fixing (sp=0.03/0.1/0.3 rad)")
    ax[0].legend(fontsize=7)
    ax[0].grid(alpha=0.3)
    names = [r[0] for r in rowsD]
    vals = [r[1]*100 for r in rowsD]
    ax[1].bar(range(len(vals)), vals)
    ax[1].set_xticks(range(len(vals)))
    ax[1].set_xticklabels(names, rotation=45, ha="right", fontsize=5.5)
    ax[1].set_ylabel("P(survey) [%]")
    ax[1].set_title("D. mesh self-survey bootstrap")
    ax[1].grid(alpha=0.3, axis="y")
    fig.tight_layout()
    fig.savefig("s2_ambiguity.png", dpi=110)
    print("\nwrote s2_ambiguity.csv and s2_ambiguity.png")
except Exception as exc:  # noqa: BLE001
    print("figure skipped:", exc)
