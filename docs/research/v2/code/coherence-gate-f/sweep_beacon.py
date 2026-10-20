#!/usr/bin/env python
"""Round-2, direction f (T1): the Phased-Sweep Beacon (PSB) — a RIPS-style
hub protocol — simulated.

The hub puck is conducted-coherent (ESPARGOS pattern): its F faces share one
LO, so the hub can radiate a burst of K ESP-NOW frames with a KNOWN
carrier-phase ladder u_k between frames (per-face VCTCXO pulls a la ESP-PPB,
or alternating faces with a conducted-calibrated relative phase, or a plain
unknown-common-phase broadcast — the ladder degrades gracefully to note k's
strobe). Every listener regresses

    y_k = phi_m + 2 pi f_m t_k + u_k + n_k              (per face in use)

to a per-burst phase intercept phi_m and CFO f_m. Two listeners' intercepts
differ by the cross-node gauge fix; a listener's intercepts for two hub
FACES differ by a RIPS-style q-range (a path-difference observable that
cancels the listener's own LO — gauge-free self-localisation).

Parts:
  E1  ladder regression: intercept + prediction variance (Wolfram closed form
      vs Monte-Carlo), vs burst length K and per-frame noise.
  E2  CFO-correction artefact scenarios: constant per burst (absorbed into
      the intercept), random per frame (averages), per-frame full rotation (A1
      failure — kills), int8-quantised CFO intercept artefact.
  E3  q-range bearing solve from F hub faces: wrapped-phase Gauss-Newton with
      an FTM-scale prior; CRLB; success vs prior error and phase noise.
  E4  ambiguity cascade: FTM -> 13-channel stitched delay -> carrier phase;
      integer-resolution success rate vs sigma_phi.

SIMULATION ONLY (NumPy model of estimators; no device measured).
Run:  /home/matt/projects/aquila-v2-spec/sim-python sweep_beacon.py
Out:  sweep_summary.csv, sweep_figure.png
"""
import csv
import numpy as np

rng = np.random.default_rng(20261016)

C = 2.99792458e8
FC = 2.437e9
LAM = C / FC
DT_FRAME = 100e-6          # ESP-NOW frame spacing in a burst (ESP-PPB measured)


# ----------------------------------------------------------------------------
# E1: ladder regression — intercept and prediction variance
# ----------------------------------------------------------------------------
def ladder_fit_MC(K, dt, ell, sig_n, n_real=20000, rng=rng):
    """MC of LS line fit over K frames; returns (sig_intercept, sig_pred)."""
    t = np.arange(K) * dt
    tbar = t.mean()
    X = np.column_stack([np.ones(K), t - tbar])
    H = np.linalg.inv(X.T @ X) @ X.T            # (2, K) hat matrix
    ts = t[-1] + ell
    w_pred = np.array([1.0, ts - tbar])
    phi0 = rng.uniform(-np.pi, np.pi, n_real)
    f0 = rng.normal(0, 30.0, n_real)            # listener CFO vs hub, Hz
    n = sig_n * rng.standard_normal((n_real, K))
    Y = phi0[:, None] + 2 * np.pi * f0[:, None] * (t - tbar)[None, :] + n
    beta = Y @ H.T
    pred = beta @ w_pred
    true = phi0 + 2 * np.pi * f0 * (ts - tbar)
    return (float(np.std(beta[:, 0] - phi0, ddof=1)),
            float(np.std(pred - true, ddof=1)))


def ladder_closed(K, dt, ell):
    """Var_pred/sig_n^2 = (1 + 3 (2 ell + dt(K-1))^2 / (dt^2 (K^2-1)))/K
    (Wolfram-verified); intercept at burst centre has variance sig_n^2/K."""
    return (1.0 + 3.0 * (2 * ell + dt * (K - 1)) ** 2 / (dt**2 * (K * K - 1))) / K


print("== E1: ladder regression (u_k known; sensing frame at lag ell) ==")
print(" K   ell[ms]  sig_n | MC intercept  closed int | MC pred   closed pred")
e1_rows = []
for K in [4, 8, 16]:
    for ell_ms in [0.0, 1.0, 5.0]:
        si, sp = ladder_fit_MC(K, DT_FRAME, ell_ms * 1e-3, 0.05)
        cl = np.sqrt(ladder_closed(K, DT_FRAME, ell_ms * 1e-3)) * 0.05
        cli = 0.05 / np.sqrt(K)
        e1_rows.append((K, ell_ms, 0.05, si, sp, cl))
        print(f"{K:3d}  {ell_ms:7.1f}   0.05 |   {si:.4f}      {cli:.4f}   |"
              f"  {sp:.4f}     {cl:.4f}")

# ----------------------------------------------------------------------------
# E2: CFO-correction artefact scenarios
# ----------------------------------------------------------------------------
print("\n== E2: artefact scenarios (K=8, dt=100 us, ell=1 ms, sig_n=0.05) ==")
K, ell, sig_n = 8, 1e-3, 0.05
t = np.arange(K) * DT_FRAME
tbar = t.mean()
X = np.column_stack([np.ones(K), t - tbar])
H = np.linalg.inv(X.T @ X) @ X.T
ts = t[-1] + ell
w_pred = np.array([1.0, ts - tbar])
n_real = 20000
phi0 = rng.uniform(-np.pi, np.pi, n_real)
f0 = rng.normal(0, 30.0, n_real)
base = phi0[:, None] + 2 * np.pi * f0[:, None] * (t - tbar)[None, :]
noise = sig_n * rng.standard_normal((n_real, K))

art_const = rng.uniform(-0.2, 0.2, n_real)[:, None] * np.ones((1, K))
art_rand = 0.1 * rng.standard_normal((n_real, K))
art_rot = rng.uniform(-np.pi, np.pi, (n_real, K))
# int8-quantised CFO estimate artefact: removed-phase error 2 pi df_err T_ref
CFO_STEP = 2 * 48.7e3 / 255                  # int8 over +-48.7 kHz, Hz
cfo_meas = np.round(f0 / CFO_STEP) * CFO_STEP
T_REF = 100e-6                               # unknown internal window; mid-case
art_quant = (2 * np.pi * (f0 - cfo_meas) * T_REF)[:, None] * np.ones((1, K))

scen = {
    "none (noise only)": noise,
    "(a) constant artefact +-0.2 rad": art_const + noise,
    "(b) random per-frame 0.1 rad": art_rand + noise,
    "(c) A1 fail: U(-pi,pi)/frame": art_rot + noise,
    "(d) int8-CFO quant. artefact (T_ref=100us)": art_quant + noise,
}
e2_rows = []
for name, extra in scen.items():
    Y = base + extra
    beta = Y @ H.T
    pred = beta @ w_pred
    true = phi0 + 2 * np.pi * f0 * (ts - tbar)
    si = float(np.std(beta[:, 0] - phi0, ddof=1))
    sp = float(np.std(pred - true, ddof=1))
    e2_rows.append((name, si, sp))
    print(f"  {name:40s}  intercept sigma={si:7.4f}   pred sigma={sp:7.4f}")

# ----------------------------------------------------------------------------
# E3: q-range bearing from F hub faces (RIPS-style, listener-side)
# ----------------------------------------------------------------------------
print("\n== E3: q-range bearing solve (20 dodecahedron phase centres, r=15 cm) ==")
PHI = (1 + np.sqrt(5)) / 2
DOD = [(s1, s2, s3) for s1 in (1, -1) for s2 in (1, -1) for s3 in (1, -1)]
DOD += [(0, s1 / PHI, s2 * PHI) for s1 in (1, -1) for s2 in (1, -1)]
DOD += [(s1 / PHI, s2 * PHI, 0) for s1 in (1, -1) for s2 in (1, -1)]
DOD += [(s1 * PHI, 0, s2 / PHI) for s1 in (1, -1) for s2 in (1, -1)]
X_FACES = np.array(DOD) * (0.15 / np.sqrt(3.0))     # circumradius 15 cm
F = X_FACES.shape[0]
print(f"  faces: {F}, max lever arm {np.max(np.linalg.norm(X_FACES, axis=1))*2*100:.1f} cm")


def wrap(x):
    return (x + np.pi) % (2 * np.pi) - np.pi


def qrange_bearing_mc(sig_th, prior_rad, R, n_real=400, iters=10, rng=rng):
    """Wrapped-phase Gauss-Newton for the listener bearing (az, el) at range R.

    Observations: per-face phase differences vs face 0 (q-range phases),
    wrapped. Init: truth + Gaussian prior error. Returns (success fraction
    within 2 deg, rms bearing error of successes [rad], CRLB sigma [rad]).
    """
    az_t = rng.uniform(0, 2 * np.pi, n_real)
    el_t = np.arcsin(rng.uniform(-0.6, 0.6, n_real))
    u_t = np.column_stack([np.cos(el_t) * np.cos(az_t),
                           np.cos(el_t) * np.sin(az_t), np.sin(el_t)])
    p_t = u_t * R
    D = np.linalg.norm(p_t[:, None, :] - X_FACES[None, :, :], axis=2)
    q = D - D[:, :1]
    obs = wrap((2 * np.pi / LAM) * q + sig_th * rng.standard_normal(q.shape))
    az = wrap(az_t + prior_rad * rng.standard_normal(n_real))
    el = np.clip(el_t + prior_rad * rng.standard_normal(n_real), -1.3, 1.3)
    eps = 1e-6

    def model(az_, el_):
        u = np.column_stack([np.cos(el_) * np.cos(az_), np.cos(el_) * np.sin(az_),
                             np.sin(el_)])
        p = u * R
        Dm = np.linalg.norm(p[:, None, :] - X_FACES[None, :, :], axis=2)
        return (2 * np.pi / LAM) * (Dm - Dm[:, :1])

    for _ in range(iters):
        res = wrap(obs - model(az, el))
        J = np.empty((n_real, F, 2))
        m0 = model(az, el)
        for i_c, (da, de) in enumerate([(eps, 0.0), (0.0, eps)]):
            m1 = model(az + da, el + de)
            J[:, :, i_c] = wrap(m1 - m0) / eps
        JtJ = np.einsum("nfi,nfj->nij", J, J)
        Jtr = np.einsum("nfi,nf->ni", J, res)
        step = np.linalg.solve(JtJ + 1e-8 * np.eye(2), Jtr[:, :, None])[:, :, 0]
        step = np.clip(step, -0.3, 0.3)
        az = wrap(az + step[:, 0])
        el = np.clip(el + step[:, 1], -1.3, 1.3)
    err = np.sqrt(wrap(az - az_t) ** 2 + (el - el_t) ** 2)
    ok = err < np.deg2rad(2.0)
    rms = float(np.sqrt(np.mean(err[ok] ** 2))) if ok.any() else np.nan
    # CRLB: Fisher of the unwrapped model at truth
    m0 = (2 * np.pi / LAM) * q
    Jc = np.empty((n_real, F, 2))
    for i_c, (da, de) in enumerate([(1.0, 0.0), (0.0, 1.0)]):
        u2 = np.column_stack([np.cos(el_t + de * eps) * np.cos(az_t + da * eps),
                              np.cos(el_t + de * eps) * np.sin(az_t + da * eps),
                              np.sin(el_t + de * eps)])
        p2 = u2 * R
        D2 = np.linalg.norm(p2[:, None, :] - X_FACES[None, :, :], axis=2)
        Jc[:, :, i_c] = ((2 * np.pi / LAM) * (D2 - D2[:, :1]) - m0) / eps
    fish = np.einsum("nfi,nfj->nij", Jc, Jc) / sig_th**2
    crlb = float(np.sqrt(np.mean([np.trace(np.linalg.inv(f)) for f in fish])))
    return float(ok.mean()), rms, crlb


print("  sig_th[rad]  prior[deg]  R[m]   success  rms_err[deg]  CRLB[deg]")
e3_rows = []
for sig_th in [0.02, 0.05, 0.10, 0.20]:
    s, r_, c_ = qrange_bearing_mc(sig_th, np.deg2rad(17.0), 3.0)
    e3_rows.append((sig_th, 17.0, 3.0, s, r_, c_))
    print(f"  {sig_th:9.3f}  {17.0:8.0f}  {3.0:4.1f}   {s:6.1%}  "
          f"{np.rad2deg(r_):9.3f}  {np.rad2deg(c_):9.3f}")
for prior_deg in [5.0, 10.0, 30.0, 45.0, 60.0]:
    s, r_, c_ = qrange_bearing_mc(0.05, np.deg2rad(prior_deg), 3.0)
    e3_rows.append((0.05, prior_deg, 3.0, s, r_, c_))
    print(f"  {0.05:9.3f}  {prior_deg:8.0f}  {3.0:4.1f}   {s:6.1%}  "
          f"{np.rad2deg(r_):9.3f}  {np.rad2deg(c_):9.3f}")
for R in [1.0, 8.0]:
    s, r_, c_ = qrange_bearing_mc(0.05, np.deg2rad(17.0), R)
    e3_rows.append((0.05, 17.0, R, s, r_, c_))
    print(f"  {0.05:9.3f}  {17.0:8.0f}  {R:4.1f}   {s:6.1%}  "
          f"{np.rad2deg(r_):9.3f}  {np.rad2deg(c_):9.3f}")

# ----------------------------------------------------------------------------
# E4: ambiguity cascade: FTM -> stitched delay -> carrier phase
# ----------------------------------------------------------------------------
print("\n== E4: delay-integer resolution (13 channels x 5 MHz, FTM prior 1.5 m) ==")
FCH = 2412e6 + 5e6 * np.arange(13)          # 2412..2472 MHz
D_TRUE = 5.0
SIG_FTM = 1.5


def cascade_mc(sig_phi, n_real=4000, rng=rng):
    """P(carrier integer correct) for the stitched-delay -> carrier cascade.

    One-way ranging model: wrapped phases phi_c = 2 pi f_c tau mod 2 pi.
    Stage A: FTM prior (sigma 1.5 m) picks the 5 MHz branch (30 m window).
    Stage B: ML delay from the 13 wrapped phases over +-3 sigma_FTM window.
    Stage C: carrier integer at f_c correct iff |tau_hat - tau| < 0.5/f_c.
    """
    tau = D_TRUE / C
    ftm = tau + SIG_FTM / C * rng.standard_normal(n_real)
    ph = wrap(2 * np.pi * FCH[None, :] * tau
              + sig_phi * rng.standard_normal((n_real, 13)))
    grid = np.linspace(-3 * SIG_FTM / C, 3 * SIG_FTM / C, 3001)
    tau_hat = np.zeros(n_real)
    best = np.full(n_real, np.inf)
    for i0 in range(0, len(grid), 100):
        g = grid[i0:i0 + 100]
        tau_c = ftm[:, None, None] + g[None, None, :]          # (n,1,100)
        r = wrap(ph[:, :, None] - 2 * np.pi * FCH[None, :, None] * tau_c)
        sse = np.einsum("ncg,ncg->ng", r, r)
        idx = np.argmin(sse, axis=1)
        cand = sse[np.arange(n_real), idx]
        take = cand < best
        best = np.where(take, cand, best)
        tau_hat = np.where(take, ftm + grid[i0 + idx], tau_hat)
    ok_hi = np.abs(tau_hat - tau) * FCH[-1] < 0.5
    ok_lo = np.abs(tau_hat - tau) * FCH[0] < 0.5
    return float(ok_hi.mean()), float(ok_lo.mean()), float(np.std(tau_hat - tau) * C)


print("  sig_phi[rad]  P(int ok @2472MHz)  P(int ok @2412MHz)  sig_tauhat[cm]")
e4_rows = []
for sig_phi in [0.01, 0.02, 0.05, 0.07, 0.10, 0.15, 0.20, 0.30]:
    p1, p2, st = cascade_mc(sig_phi)
    e4_rows.append((sig_phi, p1, p2, st))
    print(f"  {sig_phi:9.3f}       {p1:8.1%}             {p2:8.1%}        {st*100:6.2f}")

Brms = np.sqrt(np.sum((FCH - FCH.mean()) ** 2) / len(FCH))
print(f"  analytic B_rms={Brms/1e6:.1f} MHz; sig_tau = sig_phi/(2 pi B_rms sqrt(13)):")
for sig_phi in [0.02, 0.05, 0.10]:
    st_ = sig_phi / (2 * np.pi * Brms * np.sqrt(13)) * C
    print(f"    sig_phi={sig_phi:.2f} -> sig_tau={st_*100:5.2f} cm"
          f"  (carrier integers need < {0.5 / FCH[-1] * C * 100:.2f} cm)")

# ----------------------------------------------------------------------------
# CSV + figure
# ----------------------------------------------------------------------------
with open("sweep_summary.csv", "w", newline="") as fh:
    w = csv.writer(fh)
    w.writerow(["part", "param", "value", "units"])
    for K, ell_ms, sn, si, sp, cl in e1_rows:
        w.writerow(["E1_intercept", f"K={K},ell={ell_ms}ms", f"{si:.5f}", "rad"])
        w.writerow(["E1_pred", f"K={K},ell={ell_ms}ms", f"{sp:.5f}", "rad"])
        w.writerow(["E1_pred_closed", f"K={K},ell={ell_ms}ms", f"{cl:.5f}", "rad"])
    for name, si, sp in e2_rows:
        w.writerow(["E2_intercept", name, f"{si:.5f}", "rad"])
        w.writerow(["E2_pred", name, f"{sp:.5f}", "rad"])
    for sig_th, prior_deg, R, s, r_, c_ in e3_rows:
        w.writerow(["E3_success", f"sig={sig_th},prior={prior_deg},R={R}",
                    f"{s:.4f}", "frac"])
        w.writerow(["E3_rms_deg", f"sig={sig_th},prior={prior_deg},R={R}",
                    f"{np.rad2deg(r_):.5f}", "deg"])
        w.writerow(["E3_crlb_deg", f"sig={sig_th},prior={prior_deg},R={R}",
                    f"{np.rad2deg(c_):.5f}", "deg"])
    for sig_phi, p1, p2, st in e4_rows:
        w.writerow(["E4_int_ok_hi", f"sig_phi={sig_phi}", f"{p1:.5f}", "frac"])
        w.writerow(["E4_int_ok_lo", f"sig_phi={sig_phi}", f"{p2:.5f}", "frac"])
        w.writerow(["E4_sig_tau_cm", f"sig_phi={sig_phi}", f"{st*100:.5f}", "cm"])

try:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    fig, ax = plt.subplots(1, 3, figsize=(11, 3.4))
    ells = np.linspace(0, 5e-3, 40)
    for K, ls in [(4, "--"), (8, "-"), (16, ":")]:
        ax[0].semilogy(ells * 1e3, np.sqrt(ladder_closed(K, DT_FRAME, ells)) * 0.05,
                       ls, label=f"K={K}")
    ax[0].set_xlabel("extrapolation lag ell [ms]")
    ax[0].set_ylabel(r"prediction $\sigma_\phi$ [rad]")
    ax[0].set_title("E1. ladder prediction noise")
    ax[0].legend(fontsize=7)
    ax[0].grid(True, which="both", alpha=0.3)

    sp_x = [r[0] for r in e4_rows]
    ax[1].semilogy(sp_x, [max(r[1], 1e-4) for r in e4_rows], "o-",
                   label="P(int ok) @2472 MHz")
    ax[1].semilogy(sp_x, [max(r[2], 1e-4) for r in e4_rows], "s--",
                   label="P(int ok) @2412 MHz")
    ax[1].axhline(0.99, color="k", ls=":", lw=0.8)
    ax[1].axvline(0.056, color="r", ls="--", lw=0.8, label="design pt 0.056 rad")
    ax[1].set_xlabel(r"per-channel $\sigma_\phi$ [rad]")
    ax[1].set_title("E4. carrier-integer success vs phase noise")
    ax[1].legend(fontsize=7)
    ax[1].grid(True, which="both", alpha=0.3)

    sigs = [0.02, 0.05, 0.10, 0.20]
    crlbs = [np.rad2deg(qrange_bearing_mc(s, np.deg2rad(17), 3.0, n_real=80)[2])
             for s in sigs]
    ax[2].loglog(sigs, crlbs, "o-")
    ax[2].set_xlabel(r"q-range noise $\sigma_\vartheta$ [rad]")
    ax[2].set_ylabel("bearing CRLB [deg]")
    ax[2].set_title("E3. PSB q-range bearing (R=3 m)")
    ax[2].grid(True, which="both", alpha=0.3)
    fig.tight_layout()
    fig.savefig("sweep_figure.png", dpi=100)
    print("\nwrote sweep_summary.csv and sweep_figure.png")
except Exception as exc:  # noqa: BLE001
    print("figure skipped:", exc)
