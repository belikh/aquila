#!/usr/bin/env python
"""Round-2 T1 (character `f`): the coherence gate — M2 residual simulation.

Parts:
  A  holdover of the cross-node LO phase after per-frame CFO removal
     (OU residual-CFO process): unbracketed holdover vs exact mid-bracket
     interpolation vs a 2-state Kalman tracker at cadence.
  B  TX-instant jitter x CFO budget for two-way exchanges (uncorrected,
     ns-timestamp-corrected, deferral-gated).
  C  face/packet averaging law + modified CRB (MCRB) measurement floor.
  D  the shared coherence budget B, the combined design-point budget with a
     scene-dynamics (walker) term, and the killing parameter values.

SIMULATION ONLY: a NumPy model of the estimator stack, not a measurement of
any device (per docs/research/v2/EXPERIMENT-ENV.md). Parameters are sourced
from the round-1 corpus and cited in the note 23-coherence-gate-f.md.

Run:   /home/matt/projects/aquila-v2-spec/sim-python m2_residual.py
Out:   m2_summary.csv, m2_figure.png (small)
"""
import csv
import numpy as np

rng = np.random.default_rng(20261016)

C = 2.99792458e8
FC = 2.437e9                    # channel 6 centre
LAM = C / FC                    # 12.29 cm
PPM = 1e-6
DF_RAW = {"10ppm": 10 * PPM * FC, "20ppm": 20 * PPM * FC, "40ppm": 40 * PPM * FC}
F_D_ONEWAY = 1.0 / LAM          # 8.15 Hz per (m/s): one-way Doppler
F_D_RTW = 2.0 / LAM             # 16.3 Hz per (m/s): round-trip Doppler


# ----------------------------------------------------------------------------
# Part A: residual-CFO process (Ornstein-Uhlenbeck) and three estimators
# ----------------------------------------------------------------------------
SIG_F = [10.0, 30.0, 100.0]     # residual CFO 1-sigma, Hz (post per-frame removal)
TAU_C = 1.0                     # residual correlation time, s
GAP_MS = [0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0]


def ou_phase_batch(sigma_f, tau_c, n_real, n_steps, dt, rng):
    """Vectorised exact OU frequency; returns phase paths [rad]."""
    e = np.exp(-dt / tau_c)
    sd = sigma_f * np.sqrt(1.0 - e * e)
    f = sigma_f * rng.standard_normal(n_real)          # stationary start
    th = np.zeros((n_real, n_steps + 1))
    for k in range(n_steps):
        f_new = e * f + sd * rng.standard_normal(n_real)
        th[:, k + 1] = th[:, k] + 2.0 * np.pi * 0.5 * (f + f_new) * dt
        f = f_new
    return th


def holdover_mc(sigma_f, tau_c, gaps_ms, n_real=4000, dt=2.5e-5):
    """sigma_phi(delta): std of phase accrued over delta from a noiseless fix."""
    gaps = np.array(gaps_ms) * 1e-3
    n_steps = int(round(max(gaps) / dt))
    th = ou_phase_batch(sigma_f, tau_c, n_real, n_steps, dt, rng)
    idx = np.rint(gaps / dt).astype(int)
    return {g: float(np.std(th[:, i] - th[:, 0], ddof=1)) for g, i in zip(gaps_ms, idx)}


def holdover_closed(sigma_f, tau_c, d):
    """Var(int_0^d f dt) for stationary OU, returned in rad^2."""
    var_cycles = 2.0 * sigma_f**2 * tau_c * (d - tau_c * (1.0 - np.exp(-d / tau_c)))
    return (2.0 * np.pi) ** 2 * var_cycles


def bracket_closed_rad(sigma_f, tau_c, T):
    """Exact mid-bracket error of linear interpolation between two fixes T apart.

    Var_e[cycles^2] = s2*tau*(T + (-3 - e^{-T/tau} + 4 e^{-T/(2 tau)})*tau)/2,
    derived and checked with Wolfram (note 23-coherence-gate-f.md, S3.1).
    """
    s2 = sigma_f**2
    var_cyc = 0.5 * s2 * tau_c * (T + (-3.0 - np.exp(-T / tau_c)
                                        + 4.0 * np.exp(-T / (2.0 * tau_c))) * tau_c)
    return 2.0 * np.pi * np.sqrt(max(var_cyc, 0.0))


def bracket_mc(sigma_f, tau_c, T, n_real=200000, dt=None):
    dt = dt if dt else min(2.5e-5, T / 200.0)
    n_steps = max(int(round(T / dt)), 2)
    th = ou_phase_batch(sigma_f, tau_c, n_real, n_steps, dt, rng)
    e_mid = th[:, n_steps // 2] - 0.5 * (th[:, 0] + th[:, n_steps])
    return float(np.std(e_mid, ddof=1))


def kalman_tracker(T, sigma_f, tau_c, sig_m, sig_fm, t_sim=6.0, n_real=400):
    """Vectorised 2-state [phase rad, freq Hz] tracker; phase + CFO fix each update.

    Returns (rms error at updates, rms error at worst prediction point 0.75T later).
    """
    steps = max(int(t_sim / T), 40)
    burn = steps // 4
    q_f = 2.0 * sigma_f**2 / tau_c                      # Hz^2/s
    qp = (2 * np.pi) ** 2 * q_f * T**3 / 3.0
    qpf = 2 * np.pi * q_f * T**2 / 2.0
    qff = q_f * T
    th = np.zeros(n_real)
    f = sigma_f * rng.standard_normal(n_real)
    th_hat = np.zeros(n_real)
    f_hat = np.zeros(n_real)
    pth, pthf, pf = (np.full(n_real, 0.25), np.zeros(n_real),
                     np.full(n_real, sigma_f**2))
    errs, errm = [], []
    for _ in range(steps):
        f = f + np.sqrt(qff) * rng.standard_normal(n_real)
        th = th + 2 * np.pi * f * T
        th_p = th_hat + 2 * np.pi * f_hat * T
        pth_p = pth + 4 * np.pi * pthf * T + 4 * np.pi**2 * pf * T**2 + qp
        pthf_p = pthf + 2 * np.pi * pf * T + qpf
        pf_p = pf + qff
        z1 = th + sig_m * rng.standard_normal(n_real)
        S1 = pth_p + sig_m**2
        r1 = z1 - th_p
        th_hat = th_p + pth_p / S1 * r1
        f_hat = f_hat + pthf_p / S1 * r1
        pth_n = pth_p - pth_p * pth_p / S1
        pthf_n = pthf_p - pth_p * pthf_p / S1
        pf_n = pf_p - pthf_p * pthf_p / S1
        z2 = f + sig_fm * rng.standard_normal(n_real)
        S2 = pf_n + sig_fm**2
        r2 = z2 - f_hat
        f_hat = f_hat + pf_n / S2 * r2
        pth_n2 = pth_n - pthf_n * pthf_n / S2
        pthf_n2 = pthf_n - pthf_n * pf_n / S2
        pf_n2 = pf_n - pf_n * pf_n / S2
        pth, pthf, pf = pth_n2, pthf_n2, pf_n2
        if _ >= burn:
            e = (th_hat - th + np.pi) % (2 * np.pi) - np.pi
            errs.append(e)
            em = (th_hat + 2 * np.pi * f_hat * 0.75 * T - th - 2 * np.pi * f * 0.75 * T)
            em = (em + np.pi) % (2 * np.pi) - np.pi
            errm.append(em)
    return (float(np.std(np.asarray(errs), ddof=1)),
            float(np.std(np.asarray(errm), ddof=1)))


print("== Part A: residual-CFO wander (tau_c = 1 s) ==")
hold = {s: holdover_mc(s, TAU_C, GAP_MS) for s in SIG_F}
print("unbracketed holdover sigma_phi[rad] after a noiseless fix:")
print("gap[ms]   " + "   ".join(f"{s:g} Hz" for s in SIG_F))
for g in GAP_MS:
    print(f"{g:7.1f}   " + "   ".join(f"{hold[s][g]:9.3f}" for s in SIG_F))
for s in SIG_F:
    dv = float(np.sqrt(holdover_closed(s, TAU_C, 1e-3)))
    print(f"check sigma_f={s:g} Hz, 1 ms: closed-form {dv:.4f} vs MC {hold[s][1.0]:.4f}")

BRACK_MS = [0.1, 0.5, 1.0, 5.0, 10.0, 20.0, 50.0, 100.0]
print("\nmid-bracket interpolation sigma_phi[rad] (exact OU closed form):")
brack = {s: {g: bracket_closed_rad(s, TAU_C, g * 1e-3) for g in BRACK_MS} for s in SIG_F}
print("T_ref[ms]  " + "   ".join(f"{s:g} Hz" for s in SIG_F))
for g in BRACK_MS:
    print(f"{g:8.1f}   " + "   ".join(f"{brack[s][g]:9.4f}" for s in SIG_F))
for Tms in [1.0, 20.0]:
    m = bracket_mc(30.0, TAU_C, Tms * 1e-3, n_real=200000)
    print(f"bracket MC check sigma_f=30 Hz, T={Tms:g} ms: MC {m:.5f} vs closed "
          f"{bracket_closed_rad(30.0, TAU_C, Tms*1e-3):.5f}")

CAD = [1, 2, 5, 10, 20, 50, 100, 200]      # update cadence, ms
K, DT = 8, 100e-6                           # ladder: 8 frames at 100 us spacing
SIG_N = 0.05                                # per-frame phase noise, rad
Stt = K * (K * K - 1) * DT * DT / 12.0
sig_fm = SIG_N / (2 * np.pi * np.sqrt(Stt))          # CFO fix per burst, Hz
print(f"\nladder CFO fix: K={K}, dt={DT*1e6:g} us, sig_n={SIG_N} rad"
      f" -> sig_fm={sig_fm:.1f} Hz per burst")
print("2-state tracker (phase fix 0.05 rad + ladder CFO fix) vs cadence, 30 Hz:")
track = {}
for Tms in CAD:
    at_up, mid = kalman_tracker(Tms * 1e-3, 30.0, TAU_C, 0.05, max(sig_fm, 1.0),
                                t_sim=min(6.0, 600 * Tms * 1e-3 + 1))
    track[Tms] = (at_up, mid)
    print(f"  T={Tms:5.1f} ms   at-update={at_up:.4f}   worst-between={mid:.4f}")

# ----------------------------------------------------------------------------
# Part B: TX-instant jitter x CFO (two-way exchanges only)
# ----------------------------------------------------------------------------
print("\n== Part B: two-way residual from TX-instant jitter (2 pi df_raw sig_tau) ==")


def jitter_sigma(df_raw, sig_tau):
    return float(2 * np.pi * df_raw * sig_tau)


rowsB = []
print("df_raw        sigma_tau[us]   sigma_phi[rad]")
for name, df in DF_RAW.items():
    for st in [0.1e-6, 0.3e-6, 1e-6, 3e-6]:
        sp = jitter_sigma(df, st)
        rowsB.append((name, df, st, sp))
        print(f"{name:6s} {df/1e3:7.1f}kHz   {st*1e6:7.2f}        {sp:8.3f}")

quant_ns = 12.5 / np.sqrt(12) * np.sqrt(2.0)     # difference of two 12.5 ns stamps
print(f"\nns-timestamp-corrected (residual = TXN asymmetry + quant {quant_ns:.2f} ns):")
rowsB2 = []
for name, df in DF_RAW.items():
    for asym in [1, 10, 30, 100]:
        st = np.sqrt(asym**2 + quant_ns**2) * 1e-9
        sp = jitter_sigma(df, st)
        rowsB2.append((name, df, asym, sp))
        print(f"  {name:6s} asym={asym:4d} ns -> sigma_phi={sp:8.4f} rad")

tm = rng.laplace(0.0, 1.0e-6, 400000)
tm = tm + rng.choice([0.0, 8e-6], size=tm.size, p=[0.9, 0.1])
df0 = DF_RAW["20ppm"]
print("\ndeferral gate (1 us Laplace turnaround + 10% 8 us tail, 20 ppm):")
rowsB3 = []
for gate_us in [None, 10, 5, 3, 1]:
    m = np.ones_like(tm, bool) if gate_us is None else np.abs(tm) <= gate_us * 1e-6
    sp = float(np.std(2 * np.pi * df0 * tm[m], ddof=1))
    rowsB3.append((str(gate_us), float(m.mean()), sp))
    print(f"  gate={str(gate_us):>4s} us  keep={m.mean()*100:5.1f}%  sigma_phi={sp:6.3f} rad")

# ----------------------------------------------------------------------------
# Part C: MCRB floor and the face/packet averaging law
# ----------------------------------------------------------------------------
print("\n== Part C: Modified CRB floor per reference event; averaging law ==")
rowsC = []
for ns in [56, 114]:
    for snr_db in [-6, 0, 6, 10, 16]:
        rho = 10 ** (snr_db / 10)
        s = 1 / np.sqrt(2 * ns * rho)
        rowsC.append((ns, snr_db, s))
        print(f"  Ns={ns:3d}  SNR={snr_db:3d} dB  sigma_MCRB={s:.4f} rad")


def avg_frac(N, rho_c):
    return np.sqrt((1.0 + (N - 1.0) * rho_c) / N)


print("\naveraging law sigma_N/sigma_1 (common-mode correlation rho_c):")
print("  N  | rho=0.0  rho=0.25  rho=0.5  rho=0.8  rho=1.0")
for N in [1, 2, 4, 8, 16, 20]:
    print(f" {N:3d} | " + "  ".join(f"{avg_frac(N, r):.3f}" for r in [0.0, 0.25, 0.5, 0.8, 1.0]))

# ----------------------------------------------------------------------------
# Part D: coherence budget B, design-point budget, killing values
# ----------------------------------------------------------------------------
print("\n== Part D: coherence budget B = df_res * T_obs [cycles] (0.08 cyc = 0.5 rad) ==")
for Tms in [0.1, 0.5, 1, 2, 5, 10, 20, 62]:
    b = [Tms * 1e-3 * df for df in [10, 30, 100]]
    print(f"T_obs={Tms:6.1f} ms   " + "  ".join(f"{x:7.4f}" for x in b))


def mcrb(ns, snr_db):
    return 1.0 / np.sqrt(2 * ns * 10 ** (snr_db / 10))


V_WALK = 1.0                                  # m/s walker
def scene_rad(gap_s, twoway=False):
    f_d = F_D_RTW if twoway else F_D_ONEWAY
    return 2 * np.pi * f_d * V_WALK * gap_s


# design points: (name, wander, meas/ sqrt(N_eff), two-way jitter, scene)
budget = [
    ("D1 stock S3: 1 ms brackets in 20 ms superframe, 4-frame avg,"
     " two-way jitter corrected (30 ns)",
     bracket_closed_rad(30.0, TAU_C, 1e-3), mcrb(56, 6) / np.sqrt(4),
     jitter_sigma(DF_RAW["20ppm"], 30e-9), scene_rad(1e-3, True)),
    ("D2 stock S3: broadcast-only fix (no two-way term), 1 ms brackets",
     bracket_closed_rad(30.0, TAU_C, 1e-3), mcrb(56, 6) / np.sqrt(4),
     0.0, scene_rad(1e-3, False)),
    ("D3 stock S3: 20 ms unbracketed holdover, walker present",
     np.sqrt(holdover_closed(30.0, TAU_C, 20e-3)), mcrb(56, 6) / np.sqrt(4),
     jitter_sigma(DF_RAW["20ppm"], 30e-9), scene_rad(20e-3, True)),
    ("D4 ESP-PPB-class VCTCXO (1 ppb), 20 ms unbracketed holdover, walker",
     np.sqrt(holdover_closed(2.44, 100.0, 20e-3)), mcrb(56, 6) / np.sqrt(4),
     jitter_sigma(DF_RAW["20ppm"], 30e-9), scene_rad(20e-3, True)),
    ("D5 hazard: uncorrected software reply jitter 1 us at 20 ppm, 1 ms brackets",
     bracket_closed_rad(30.0, TAU_C, 1e-3), mcrb(56, 6) / np.sqrt(4),
     jitter_sigma(DF_RAW["20ppm"], 1e-6), scene_rad(1e-3, True)),
    ("D6 hazard: residual CFO 100 Hz, 1 ms brackets (still bracketed!)",
     bracket_closed_rad(100.0, TAU_C, 1e-3), mcrb(56, 6) / np.sqrt(4),
     jitter_sigma(DF_RAW["20ppm"], 30e-9), scene_rad(1e-3, True)),
    ("D7 hazard: 20 ms brackets, static scene (no walker), 30 Hz",
     bracket_closed_rad(30.0, TAU_C, 20e-3), mcrb(56, 6) / np.sqrt(4),
     jitter_sigma(DF_RAW["20ppm"], 30e-9), 0.0),
    ("D8 hazard: 5 ms brackets with walker on the reference path (broadcast)",
     bracket_closed_rad(30.0, TAU_C, 5e-3), mcrb(56, 6) / np.sqrt(4),
     0.0, scene_rad(5e-3, False)),
]
print("\ncombined 1-sigma cross-node phase budget (quadrature); gate = 0.5 rad:")
budget_rows = []
for name, st, sm, sj, sc in budget:
    tot = float(np.sqrt(st**2 + sm**2 + sj**2 + sc**2))
    budget_rows.append((name, tot))
    print(f"  {name:76s} {tot:7.3f} rad  {'PASS' if tot <= 0.5 else 'FAIL'}")

print("\nkilling parameter values (single-term 0.5 rad lines):")
for df in [10, 30, 100]:
    print(f"  unbracketed holdover, sigma_f={df:3d} Hz -> T* ="
          f" {0.5/(2*np.pi*df)*1e3:7.2f} ms")
for df in [10, 30, 100]:
    lo, hi = 1e-5, 2.0
    for _ in range(60):
        mid = 0.5 * (lo + hi)
        if bracket_closed_rad(df, TAU_C, mid) < 0.5:
            lo = mid
        else:
            hi = mid
    print(f"  bracket gap,          sigma_f={df:3d} Hz -> T* = {lo*1e3:7.2f} ms")
for name, df in DF_RAW.items():
    print(f"  {name:6s} uncorrected reply jitter -> tau* ="
          f" {0.5/(2*np.pi*df)*1e6:6.2f} us")
print(f"  scene: 1 m/s walker -> 0.5 rad at broadcast bracket T* ="
      f" {0.5/(2*np.pi*F_D_ONEWAY)*1e3:5.2f} ms, two-way T* ="
      f" {0.5/(2*np.pi*F_D_RTW)*1e3:5.2f} ms")

# ----------------------------------------------------------------------------
# CSV + figure
# ----------------------------------------------------------------------------
with open("m2_summary.csv", "w", newline="") as fh:
    w = csv.writer(fh)
    w.writerow(["part", "param", "value", "units"])
    for s in SIG_F:
        for g in GAP_MS:
            w.writerow(["A_holdover", f"sigma_f={s:g}Hz,gap={g}ms", f"{hold[s][g]:.5f}", "rad"])
        for g in BRACK_MS:
            w.writerow(["A_bracket", f"sigma_f={s:g}Hz,Tref={g}ms", f"{brack[s][g]:.5f}", "rad"])
    for Tms in CAD:
        w.writerow(["A_tracker", f"sigma30Hz,T={Tms}ms,worst", f"{track[Tms][1]:.5f}", "rad"])
    for name, df, st, sp in rowsB:
        w.writerow(["B_jitter", f"{name},tau={st*1e6:g}us", f"{sp:.5f}", "rad"])
    for name, df, asym, sp in rowsB2:
        w.writerow(["B_jitter_corrected", f"{name},asym={asym}ns", f"{sp:.5f}", "rad"])
    for gate, keep, sp in rowsB3:
        w.writerow(["B_deferral", f"gate={gate}us", f"{sp:.5f}", "rad"])
    for ns, snr_db, s in rowsC:
        w.writerow(["C_mcrb", f"Ns={ns},SNR={snr_db}dB", f"{s:.5f}", "rad"])
    for name, tot in budget_rows:
        w.writerow(["D_budget", name, f"{tot:.5f}", "rad"])

try:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    fig, ax = plt.subplots(1, 3, figsize=(11, 3.4))
    for s in SIG_F:
        ax[0].loglog(GAP_MS, [hold[s][g] for g in GAP_MS], marker="o", ms=3,
                     label=f"holdover {s:g} Hz")
        ax[0].loglog(BRACK_MS, [brack[s][g] for g in BRACK_MS], marker="^", ms=3,
                     label=f"bracket {s:g} Hz")
    ax[0].loglog(GAP_MS, [scene_rad(g * 1e-3, True) for g in GAP_MS], "k:", lw=1,
                 label="scene (1 m/s, two-way)")
    ax[0].axhline(0.5, color="k", ls="--", lw=0.8, label="0.5 rad gate")
    ax[0].set_xlabel("reference gap [ms]")
    ax[0].set_ylabel(r"$\sigma_\phi$ [rad]")
    ax[0].set_title("A. wander: holdover vs bracket vs scene")
    ax[0].legend(fontsize=6)
    ax[0].grid(True, which="both", alpha=0.3)

    ax[1].semilogy(CAD, [track[T][1] for T in CAD], marker="s", ms=3,
                   label="tracker worst-between")
    ax[1].semilogy(BRACK_MS, [brack[30.0][g] for g in BRACK_MS], marker="^", ms=3,
                   label="bracket mid (30 Hz)")
    ax[1].axhline(0.5, color="k", ls="--", lw=0.8)
    ax[1].set_xlabel("update interval [ms]")
    ax[1].set_title(r"B. tracking ($\sigma_f$=30 Hz, fixes 0.05 rad)")
    ax[1].legend(fontsize=7)
    ax[1].grid(True, which="both", alpha=0.3)

    for name, df in DF_RAW.items():
        xs = np.array([0.1, 0.3, 1, 3]) * 1e-6
        ax[2].loglog(xs * 1e6, 2 * np.pi * df * xs, marker="o", ms=3, label=name)
        xs2 = np.array([1, 10, 30, 100]) * 1e-9
        ax[2].loglog(xs2 * 1e6, 2 * np.pi * df * xs2, marker="x", ms=3, alpha=0.5,
                     label=f"{name} corrected")
    ax[2].axhline(0.5, color="k", ls="--", lw=0.8)
    ax[2].set_xlabel(r"turnaround jitter $\sigma_\tau$ [$\mu$s]")
    ax[2].set_ylabel(r"$\sigma_\phi$ [rad]")
    ax[2].set_title("C. TX jitter $\\times$ CFO (two-way only)")
    ax[2].legend(fontsize=6)
    ax[2].grid(True, which="both", alpha=0.3)
    fig.tight_layout()
    fig.savefig("m2_figure.png", dpi=100)
    print("\nwrote m2_summary.csv and m2_figure.png")
except Exception as exc:  # noqa: BLE001
    print("figure skipped:", exc)
