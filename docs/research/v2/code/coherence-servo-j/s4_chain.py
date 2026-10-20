#!/usr/bin/env python3
"""S4 — end-to-end chain: calibration -> coherence -> localisation (Monte Carlo + CRLB).

Simulation, NOT a measurement. A conducted-coherent hub puck H broadcasts strobes
(servo) and sensing frames; S single-antenna S3 satellites receive. A static
point fiducial (corner reflector) at unknown position p scatters the sensing
frame back to each satellite. Two estimation routes are compared at equal
per-frame noise:

Route C (servo-coherent one-way fusion): node i's sensing-frame phase
  psi_i = -2*pi/lam * d_i(p) + (theta_i - theta_H)(t_s) + alpha_i + noise,
  d_i(p) = |x_H - p| + |p - x_i|. The servo removes the LO term to sigma_phi;
  alpha_i (hardware chain phase) is calibrated to sigma_c. A common global
  phase cancels in differences against a reference node.

Route G (gauge-free products, no servo): the clutter-normalised two-way
  product of pair (i,j) carries the target term at doubled sensitivity with
  foci (x_i, x_j), noise = sqrt(2)*frame noise (+ jitter x CFO term, parameterised).

Outputs: 3-D RMSE vs sigma_phi for both routes, CRLB cross-check, calibration
bias effect, and the sigma_phi at which coherent fusion stops beating the
gauge-free tier (~0.3 m) — the 'required vs optional' answer with numbers.
"""
import numpy as np, os
from scipy.optimize import least_squares

OUT = os.path.dirname(os.path.abspath(__file__))
FC = 2.442e9; C = 299_792_458.0; LAM = C/FC
rng = np.random.default_rng(21)

# geometry: 6 x 5 x 3 m room
X_H = np.array([0.3, 2.5, 2.8])
SATS = [np.array([5.7, 0.4, 2.0]), np.array([5.7, 4.6, 1.2]), np.array([3.0, 4.6, 2.2]),
        np.array([0.4, 0.4, 1.0]), np.array([3.0, 0.4, 2.6])]
TARGETS = [np.array([2.5, 2.3, 1.1]), np.array([4.5, 3.8, 0.9]), np.array([1.5, 1.2, 1.6])]

def d_i(xp, xi): return np.linalg.norm(X_H - xp) + np.linalg.norm(xp - xi)

def crlb_route_C(sigma_phi, nodes, xp, sig_c=0.0):
    """CRLB of p (3 unknowns + 1 nuisance global phase) from node phases, sigma_phi each."""
    Nn = len(nodes)
    ds = np.array([d_i(xp, x) for x in nodes])
    # measurement: psi_i - psi_ref (differences); build Jacobian of differences wrt p
    ref = 0
    J = []
    for a in range(Nn):
        if a == ref: continue
        ga = (xp - X_H)/np.linalg.norm(X_H - xp) + (xp - nodes[a])/np.linalg.norm(xp - nodes[a])
        gr = (xp - X_H)/np.linalg.norm(X_H - xp) + (xp - nodes[ref])/np.linalg.norm(xp - nodes[ref])
        J.append(ga - gr)
    J = -2*np.pi/LAM*np.array(J)                 # Nn-1 rows
    tot = 2*(sigma_phi**2 + sig_c**2)            # each difference carries TWO nodes' errors
    cov = np.linalg.inv(J.T@J/tot)               # differences kill the nuisance
    return np.sqrt(np.trace(cov))

def mc_route_C(sigma_phi, nodes, xp, sig_c=0.0, n_trial=160, seed=1):
    rng = np.random.default_rng(seed)
    ref = 0
    ds = np.array([d_i(xp, x) for x in nodes])
    errs, fails = [], 0
    Nn = len(nodes)
    for t in range(n_trial):
        # calibration residuals: constant bias within the trial; estimator assumes alpha = 0
        alpha = rng.normal(0, sig_c, Nn) if sig_c > 0 else np.zeros(Nn)
        psi = -2*np.pi/LAM*ds + alpha + rng.normal(0, sigma_phi, Nn) + rng.normal(0, 0.2)  # + global phase
        def resid2(p):
            dd = np.array([d_i(p, x) for x in nodes])
            v = (-2*np.pi/LAM*dd) - psi
            return np.angle(np.exp(1j*(v[1:] - v[0])))
        p0 = xp + rng.normal(0, 0.4, 3)
        r = least_squares(resid2, p0, method="lm", xtol=1e-10)
        e = np.linalg.norm(r.x - xp)
        if e > 1.0: fails += 1
        else: errs.append(e)
    return np.sqrt(np.mean(np.array(errs)**2)) if errs else np.nan, fails/n_trial

def mc_route_G(sigma_frame, nodes, xp, sig_cpair=0.0, n_trial=160, seed=2, jit_term=0.0):
    """Gauge-free products between all satellite pairs (no hub, no servo)."""
    rng = np.random.default_rng(seed)
    pairs = [(a, b) for a in range(len(nodes)) for b in range(a+1, len(nodes))]
    errs, fails = [], 0
    for t in range(n_trial):
        # per-pair calibration residuals, constant within trial; estimator assumes 0
        alpha = rng.normal(0, sig_cpair, len(pairs)) if sig_cpair > 0 else np.zeros(len(pairs))
        meas = []
        for k, (a, b) in enumerate(pairs):
            db = np.linalg.norm(nodes[a]-xp) + np.linalg.norm(xp-nodes[b])
            m = -4*np.pi/LAM*db + alpha[k] + rng.normal(0, np.sqrt(2)*sigma_frame) + rng.normal(0, jit_term)
            meas.append(m)
        meas = np.array(meas)
        def resid2(p):
            v = []
            for k, (a, b) in enumerate(pairs):
                db = np.linalg.norm(nodes[a]-p) + np.linalg.norm(p-nodes[b])
                v.append(-4*np.pi/LAM*db - meas[k])
            return np.angle(np.exp(1j*np.array(v)))
        p0 = xp + rng.normal(0, 0.4, 3)
        r = least_squares(resid2, p0, method="lm", xtol=1e-10)
        e = np.linalg.norm(r.x - xp)
        if e > 1.0: fails += 1
        else: errs.append(e)
    return np.sqrt(np.mean(np.array(errs)**2)) if errs else np.nan, fails/n_trial

def main():
    print("== S4 end-to-end chain: 3-D RMSE (m) vs per-node sigma_phi ==")
    print("nodes = hub + 5 satellites; fiducial at 3 positions; cal sigma_c = 0.1 rad")
    grid = [0.05, 0.1, 0.174, 0.25, 0.35, 0.5, 0.75, 1.0, 1.5]
    print(f"{'sigma_phi':>9s} | {'RMSE C (MC)':>12s} | {'CRLB C':>8s} | {'failC':>6s} | "
          f"{'RMSE G (MC)':>12s} | {'failG':>6s}")
    for s in grid:
        ec, fc_ = [], 0.0
        cc = []
        for xp in TARGETS:
            a, b = mc_route_C(s, SATS, xp, sig_c=0.1); ec.append(a); fc_ += b/len(TARGETS)
            cc.append(crlb_route_C(s, SATS, xp, sig_c=0.1))
        eg, fg = [], 0.0
        for xp in TARGETS:
            # route G noise: frame noise only (jitter term 0): compare at equal per-frame noise
            a, b = mc_route_G(s, SATS, xp, sig_cpair=0.1); eg.append(a); fg += b/len(TARGETS)
        print(f"{s:9.3f} | {np.mean(ec):12.3f} | {np.mean(cc):8.3f} | {fc_:6.2f} | "
              f"{np.mean(eg):12.3f} | {fg:6.2f}")
    print("\n-- route G with raw-CFO reply jitter term (2*pi*nu*sigma_xi), nu=48.84 kHz --")
    for sxi in [1e-6, 30e-6, 200e-6]:
        jit = 2*np.pi*48840.0*sxi
        eg, fg = [], 0.0
        for xp in TARGETS:
            a, b = mc_route_G(0.174, SATS, xp, sig_cpair=0.1, jit_term=jit); eg.append(a); fg += b/len(TARGETS)
        print(f"sigma_xi={sxi*1e6:6.0f} us -> jit term {jit:6.3f} rad: RMSE G = {np.mean(eg):.3f} m, fail {fg:.2f}")
    print("\n-- calibration bias sweep (route C, sigma_phi=0.174) --")
    for sc in [0.0, 0.1, 0.3, 0.7]:
        ec = []
        for xp in TARGETS:
            a, _ = mc_route_C(0.174, SATS, xp, sig_c=sc); ec.append(a)
        print(f"sigma_c={sc:.2f} rad -> RMSE C = {np.mean(ec):.3f} m")

    # figure
    import matplotlib; matplotlib.use("Agg"); import matplotlib.pyplot as plt
    fig, ax = plt.subplots(figsize=(5.0, 3.4), dpi=130)
    xs, yc, yg, ycr = [], [], [], []
    for s in grid:
        ec, cc, eg = [], [], []
        for xp in TARGETS:
            ec.append(mc_route_C(s, SATS, xp, sig_c=0.1)[0])
            cc.append(crlb_route_C(s, SATS, xp, sig_c=0.1))
            eg.append(mc_route_G(s, SATS, xp, sig_cpair=0.1)[0])
        xs.append(s); yc.append(np.mean(ec)); ycr.append(np.mean(cc)); yg.append(np.mean(eg))
    ax.plot(xs, yc, "o-", label="coherent (servo), MC", ms=3)
    ax.plot(xs, ycr, "--", label="coherent, CRLB", lw=1)
    ax.plot(xs, yg, "s-", label="gauge-free products, MC", ms=3, color="tab:green")
    ax.axhline(0.3, color="k", lw=0.8); ax.text(0.06, 0.32, "gauge-free tier ~0.3 m", fontsize=7)
    ax.set_xscale("log"); ax.set_yscale("log")
    ax.set_xlabel(r"per-node $\sigma_\varphi$ (rad)"); ax.set_ylabel("3-D RMSE (m)")
    ax.legend(fontsize=7); fig.tight_layout()
    fig.savefig(os.path.join(OUT, "s4_chain_rmse.png"))
    print("\nwrote s4_chain_rmse.png")

if __name__ == "__main__":
    main()
