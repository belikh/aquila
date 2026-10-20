#!/usr/bin/env python3
"""S1 — the coherence servo: sigma_phi(M2) vs strobe cadence on ESP32-S3 parameter ranges.

Simulation, NOT a measurement (per EXPERIMENT-ENV.md). Parameters and their
provenance are recorded in ../23-coherence-servo-j.md section 3. Pair-level
model: relative LO phase theta and relative CFO nu of two independent nodes.

Model (first-order exact for h << tau_w):
  nu_{k+1} = nu_k*(1 - h/tau_w) + dnu_noise,   Var[dnu] = s2*h
  theta_{k+1} = theta_k + nu_k*h + A_noise,
  Var[A] = s2*h^3/3, Cov[A,dnu] = s2*h^2/2,  s2 = 2*sigma_w_rad^2/tau_w
  (Ornstein-Uhlenbeck frequency wander with stationary std sigma_w, corr time tau_w;
   matches continuous white-FM process noise to O(h/tau_w).)
Kalman servo: state [theta, nu] (+[a] for 3-state variant), measurement z = theta + N(0, sigma_z^2),
  sigma_z^2 = 2*(sigma_m^2 + sigma_art^2 + sigma_pn^2)   (two nodes' independent per-frame noises)
  sigma_m  = per-frame common-phase thermal noise (modified CRB 1/sqrt(2 N_s rho))
  sigma_art = i.i.d. per-frame ESP32 CFO-correction artefact (UNMEASURED on S3; scenario)
  sigma_pn = integrated LO phase noise from L(f), computed below from the -120..-80 dBc/Hz corpus range.

sigma_phi(M2) at a sensing frame = prediction error of the servo at the frame time
  (interpolation = batch/smoother; holdover = causal)  (+) sqrt(2)*sqrt(sigma_art^2+sigma_pn^2)
  (the sensing frame's own irreducible per-frame artefact+PN at the two nodes).
"""
import numpy as np, csv, os

OUT = os.path.dirname(os.path.abspath(__file__))
FC = 2.442e9; C = 299_792_458.0; LAM = C/FC

def pn_sigma(Ldbc, f_lo=50.0, f_hi=10e6):
    """Integrated single-sideband phase noise (rad) between f_lo and f_hi (white FM approx)."""
    return np.sqrt(2.0 * 10.0**(Ldbc/10.0) * (f_hi - f_lo))

def frame_sigma(sig_m, sig_art, sig_pn):
    return np.sqrt(sig_m**2 + sig_art**2 + sig_pn**2)

def F_of(h, tau, n=2):
    if n == 2: return np.array([[1.0, h], [0.0, 1.0 - h/tau]])
    return np.array([[1.0, h, 0.5*h*h], [0.0, 1.0, h], [0.0, 0.0, 1.0]])

def Q_of(h, tau, sig_w_rad, n=2, qa=0.0):
    s2 = 2.0*sig_w_rad**2/tau
    if n == 2: return s2*np.array([[h**3/3.0, h**2/2.0], [h**2/2.0, h]])
    Q = np.zeros((3, 3)); Q[:2, :2] = s2*np.array([[h**3/3.0, h**2/2.0], [h**2/2.0, h]])
    # accel-state noise (for ramp tracking); cross terms with nu-noise omitted (small)
    Q[2, 2] = qa*h; return Q

def kf_update(P, R):
    S = P[0, 0] + R
    P = P - np.outer(P[:, 0], P[0, :])/S
    return 0.5*(P + P.T)

def steady_P(h, tau, sig_w_rad, sig_z, n=2, qa=0.0):
    F, Q, R = F_of(h, tau, n), Q_of(h, tau, sig_w_rad, n, qa), sig_z**2
    P = np.eye(n)*10.0
    for _ in range(6000):
        P = kf_update(F@P@F.T + Q, R)
    return P

def interp_sigma(h, tau, sig_w_rad, sig_z, n=2, qa=0.0):
    """Smoothed (batch) prediction std at the interval midpoint, bracket-limited."""
    F, Q, R = F_of(h, tau, n), Q_of(h, tau, sig_w_rad, n, qa), sig_z**2
    Pf = steady_P(h, tau, sig_w_rad, sig_z, n, qa)
    h2 = 0.5*h
    F2, Q2 = F_of(h2, tau, n), Q_of(h2, tau, sig_w_rad, n, qa)
    Pm = F2@Pf@F2.T + Q2                 # predicted to midpoint from t_k
    Pp = F2@Pm@F2.T + Q2                 # predicted to t_{k+1} from midpoint
    Pu = kf_update(Pp, R)                # updated at t_{k+1}
    G = Pm@F2.T@np.linalg.inv(Pp)        # RTS gain
    Ps = Pm + G@(Pu - Pp)@G.T
    return np.sqrt(Ps[0, 0])

def hold_sigma(delta, h, tau, sig_w_rad, sig_z, n=2, qa=0.0):
    """Causal holdover: prediction std delta beyond the last strobe (steady state)."""
    F, Q = F_of(delta, tau, n), Q_of(delta, tau, sig_w_rad, n, qa)
    Pf = steady_P(h, tau, sig_w_rad, sig_z, n, qa)
    P = F@Pf@F.T + Q
    return np.sqrt(P[0, 0])

# ---------------------------------------------------------------- MC validation
def mc_case(T_s, tau_w, sig_w_hz, sig_z, T_run=60.0, n_trial=16, seed=3,
            ramp_hz_s=None, ramp_t0=None, ramp_t1=None, n=2, qa=0.0):
    """Full forward Kalman + RTS smoother Monte Carlo; returns midpoint RMSE.
    ONE truth trajectory is simulated at half the strobe cadence; strobes are
    every second sample, so the true phase at the midpoints is exact."""
    rng = np.random.default_rng(seed)
    sig_w_rad = 2*np.pi*sig_w_hz
    s2 = 2.0*sig_w_rad**2/tau_w
    K = int(T_run/T_s)
    hh = 0.5*T_s; K2 = 2*K
    rr = 2*np.pi*ramp_hz_s if ramp_hz_s else 0.0
    nu = rng.normal(0, sig_w_rad, n_trial)
    th = rng.normal(0, np.pi, n_trial)
    TH2 = np.empty((K2, n_trial))
    sdnu = np.sqrt(s2*hh)
    for k in range(K2):
        TH2[k] = th
        dnu = sdnu*rng.standard_normal(n_trial)
        A = 0.5*hh*dnu + np.sqrt(s2*hh**3/12.0)*rng.standard_normal(n_trial)
        if rr and ramp_t0 is not None and ramp_t0 <= (k+1)*hh <= ramp_t1:
            dnu = dnu + rr*hh
        th = th + nu*hh + A
        nu = nu*(1 - hh/tau_w) + dnu
    TH = TH2[1::2]                                   # strobe samples: index k at t=(k+1)*T_s
    z = TH + sig_z*rng.standard_normal((K, n_trial))
    # forward Kalman
    F, Q, R = F_of(T_s, tau_w, n), Q_of(T_s, tau_w, sig_w_rad, n, qa), sig_z**2
    x = np.zeros((n, n_trial)); x[0] = z[0]
    P = np.eye(n)*10.0; xf_seq = []; Ps_seq = []
    for k in range(K):
        x = F@x; P = F@P@F.T + Q
        y = z[k] - x[0]
        S = P[0, 0] + R
        x = x + np.outer(P[:, 0]/S, y)
        P = P - np.outer(P[:, 0], P[0, :])/S
        P = 0.5*(P + P.T)
        xf_seq.append(x.copy()); Ps_seq.append(P.copy())
    # RTS backward
    xs = [None]*K; xs[-1] = xf_seq[-1]
    for k in range(K-2, -1, -1):
        Pp = F@Ps_seq[k]@F.T + Q
        G = Ps_seq[k]@F.T@np.linalg.inv(Pp)
        xs[k] = xf_seq[k] + G@(xs[k+1] - F@xf_seq[k])
    # smoothed midpoint estimates: between strobe k and k+1 (truth at TH2 index 2k+2)
    est_mid = np.array([xs[k][0] + xs[k][1]*0.5*T_s for k in range(K-1)])
    mids = TH2[2:K2-1:2]
    err = est_mid - mids
    return dict(rmse_mid=np.sqrt(np.mean(err[20:, :]**2)))

# ------------------------------------------------------------------ tables
def main():
    print("== S1 coherence servo: sigma_phi(M2) vs cadence ==")
    sig_pn = pn_sigma(-100.0)                     # -100 dBc/Hz mid of corpus range
    sig_pn_lo, sig_pn_hi = pn_sigma(-120.0), pn_sigma(-80.0)
    print(f"sigma_pn from L(f): -120 dBc/Hz -> {sig_pn_lo:.4f} rad, -100 -> {sig_pn:.4f}, -80 -> {sig_pn_hi:.4f}")
    sig_m = 1.0/np.sqrt(2*52*10.0)                # 10 dB per-subcarrier SNR, 52 subcarriers (HT20)
    print(f"sigma_m (mCRB, 10 dB, 52 sc) = {sig_m:.4f} rad")

    T_grid = [0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0]
    rows = []
    for sw in [10.0, 50.0, 100.0]:                # residual relative CFO, pair level (Hz)
        for sa in [0.02, 0.17, 0.35, 0.5, 1.0]:   # per-node i.i.d. artefact (rad)
            sz = np.sqrt(2.0)*frame_sigma(sig_m, sa, sig_pn)
            for Ts in T_grid:
                si = interp_sigma(Ts, 10.0, 2*np.pi*sw, sz)
                sh = hold_sigma(0.02, Ts, 10.0, 2*np.pi*sw, sz)      # 20 ms causal holdover
                tot_i = np.sqrt(si**2 + 2*(sa**2 + sig_pn**2))        # + sensing-frame artefact
                rows.append(dict(sw=sw, sa=sa, Ts=Ts, sig_z=sz, s_interp=si,
                                 s_hold20=sh, M2_interp=tot_i))
    with open(os.path.join(OUT, "s1_table.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys())); w.writeheader(); w.writerows(rows)

    print("\n-- sigma_phi(M2), batch/smoother at strobe midpoint (rad) --")
    print("sigma_w\\T_s  " + "  ".join(f"{t*1000:6.0f}m" for t in T_grid))
    for sw in [10.0, 50.0, 100.0]:
        for sa in [0.02, 0.17, 0.5]:
            vals = [r["M2_interp"] for r in rows if r["sw"] == sw and r["sa"] == sa]
            print(f"{sw:5.0f}Hz a={sa:.2f} " + "  ".join(f"{v:6.3f}" for v in vals))
    print("\n-- causal 20 ms holdover prediction std (rad), artefact NOT included --")
    for sw in [10.0, 50.0, 100.0]:
        for sa in [0.02, 0.17, 0.5]:
            vals = [r["s_hold20"] for r in rows if r["sw"] == sw and r["sa"] == sa]
            print(f"{sw:5.0f}Hz a={sa:.2f} " + "  ".join(f"{v:6.3f}" for v in vals))

    print("\n-- MC validation (sentinel cases) --")
    for sw, sa, Ts in [(50.0, 0.17, 0.02), (50.0, 0.17, 0.005), (10.0, 0.5, 0.02), (100.0, 0.02, 0.1)]:
        sz = np.sqrt(2.0)*frame_sigma(sig_m, sa, sig_pn)
        ana = interp_sigma(Ts, 10.0, 2*np.pi*sw, sz)
        mc = mc_case(Ts, 10.0, sw, sz)["rmse_mid"]
        print(f"sw={sw:5.0f}Hz sa={sa:.2f} Ts={Ts*1000:5.0f}ms  analytic sigma_pred={ana:.4f}  MC={mc:.4f}")

    print("\n-- killing values: parameter that makes sigma_phi(M2)=0.5 rad at cadence Ts --")
    # (a) artefact floor: sigma_phi = sqrt(2)*sqrt(sa^2+sig_pn^2) (servo error negligible at Ts<=50 ms)
    from scipy.optimize import brentq
    f_a = lambda sa: np.sqrt(2*(sa**2 + sig_pn**2)) - 0.5
    print(f"sigma_art* (i.i.d., per node, any cadence) = {brentq(f_a, 0.01, 2.0):.3f} rad")
    # (b) max cadence for each sigma_w at sa=0.17 (servo-limited leg)
    for sw in [10.0, 50.0, 100.0]:
        sa = 0.17; sz = np.sqrt(2.0)*frame_sigma(sig_m, sa, sig_pn)
        g = lambda Ts: np.sqrt(interp_sigma(Ts, 10.0, 2*np.pi*sw, sz)**2 + 2*(sa**2 + sig_pn**2)) - 0.5
        try:
            Ts_kill = brentq(g, 0.02, 5.0)
            print(f"sigma_w={sw:5.0f} Hz: cadence ceiling (batch) = {Ts_kill*1000:.0f} ms")
        except ValueError:
            print(f"sigma_w={sw:5.0f} Hz: no ceiling below 5 s (artefact dominates)")
    # (c) holdover: max delta at sa=0.17, sw=50 Hz
    sa = 0.17; sz = np.sqrt(2.0)*frame_sigma(sig_m, sa, sig_pn)
    for Ts in [0.02, 0.1]:
        g = lambda d: np.sqrt(hold_sigma(d, Ts, 10.0, 2*np.pi*50.0, sz)**2 + 2*(sa**2 + sig_pn**2)) - 0.5
        try:
            print(f"holdover ceiling (causal, sw=50Hz, Ts={Ts*1000:.0f}ms) = {brentq(g, Ts, 10.0)*1000:.0f} ms")
        except ValueError:
            print(f"holdover ceiling (causal, sw=50Hz, Ts={Ts*1000:.0f}ms): never crosses 0.5 rad "
                  f"(sigma at Ts = {g(Ts)+0.5:.2f} rad) -> causal holdover of a full interval fails the gate")

    print("\n-- tau_w sensitivity and servo coherence budget at Ts=20 ms, sigma_art=0.17 --")
    sa = 0.17; sz = np.sqrt(2.0)*frame_sigma(sig_m, sa, sig_pn)
    for tau in [3.0, 10.0, 60.0]:
        for sw in [50.0, 100.0]:
            si = interp_sigma(0.02, tau, 2*np.pi*sw, sz)
            sh = hold_sigma(0.02, 0.02, tau, 2*np.pi*sw, sz)
            print(f"tau_w={tau:4.0f}s sw={sw:5.0f}Hz: batch M2={np.sqrt(si**2+2*(sa**2+sig_pn**2)):.3f} rad, "
                  f"causal(20ms)={np.sqrt(sh**2+2*(sa**2+sig_pn**2)):.3f} rad")
    print("servo CFO knowledge (Delta_nu_res) and coherence budget, tau_w=10 s, Ts=20 ms:")
    for sw in [10.0, 50.0, 100.0]:
        Pf = steady_P(0.02, 10.0, 2*np.pi*sw, sz)
        dnu = np.sqrt(Pf[1, 1])/(2*np.pi)
        print(f"  sw={sw:5.0f} Hz: Delta_nu_res = {dnu:5.2f} Hz; B(1 s) = {dnu:5.2f} cycles; "
              f"T_coh(0.5 rad) = {0.5/(2*np.pi*dnu)*1000:5.0f} ms; target coherence lambda/2v = {LAM/2/1.0*1000:.0f} ms")

    print("\n-- ramp (temperature step) MC: 2-state vs 3-state, Ts=20 ms, sw=10 Hz --")
    for n, qa in [(2, 0.0), (3, 5.6e3)]:
        for r in [0.0, 12.0, 122.0]:
            res = mc_case(0.02, 10.0, 10.0, np.sqrt(2.0)*frame_sigma(sig_m, 0.17, sig_pn),
                          T_run=120.0, ramp_hz_s=r, ramp_t0=60.0, ramp_t1=90.0, n=n, qa=qa)
            print(f"n_state={n} ramp={r:6.1f} Hz/s  MC rmse_mid={res['rmse_mid']:.4f} rad")

    # figure: sigma_phi vs cadence
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    fig, ax = plt.subplots(figsize=(5.2, 3.4), dpi=130)
    Tfg = np.logspace(np.log10(0.001), np.log10(2.0), 40)
    for sw, ls in [(10.0, "-"), (50.0, "--"), (100.0, ":")]:
        for sa, col in [(0.17, "tab:blue"), (0.5, "tab:red")]:
            sz = np.sqrt(2.0)*frame_sigma(sig_m, sa, sig_pn)
            ys = [np.sqrt(interp_sigma(T, 10.0, 2*np.pi*sw, sz)**2 + 2*(sa**2 + sig_pn**2)) for T in Tfg]
            ax.plot(Tfg*1000, ys, ls, color=col, label=f"$\\sigma_w$={sw:.0f} Hz, $\\sigma_{{art}}$={sa}")
    ax.axhline(0.5, color="k", lw=0.8); ax.text(1.2, 0.53, "gate: 0.5 rad", fontsize=7)
    ax.set_xscale("log"); ax.set_yscale("log")
    ax.set_xlabel("strobe cadence $T_s$ (ms)"); ax.set_ylabel(r"$\sigma_\varphi$(M2) (rad)")
    ax.legend(fontsize=6, loc="upper left"); fig.tight_layout()
    fig.savefig(os.path.join(OUT, "s1_sigma_phi_vs_cadence.png"))
    print("\nwrote s1_table.csv, s1_sigma_phi_vs_cadence.png")

if __name__ == "__main__":
    main()
