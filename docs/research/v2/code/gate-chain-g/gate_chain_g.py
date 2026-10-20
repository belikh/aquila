#!/usr/bin/env python
"""Round-2, track T1, agent `g`: the coherence gate as an end-to-end chain.

Distinct from the sibling `f` (holdover/bracket/jitter decomposition): this
script simulates the *packet-level two-way exchange* (ratio + product with the
corrected shared CFO x reply-interval term), the ambiguity-resolution cascade
(FTM ppm seed -> dual-LTF within-frame probe -> burst unwrap), the falsifier
power analysis, and the complete chain sigma_phi -> integer ambiguity
resolution -> 3-D position error (CRLB + Monte Carlo), plus the reference-type
comparison (reciprocal exchange vs common-view beacon vs RIPS beat).

Simulation only: NumPy/SciPy models of estimators, not measurements. All
parameters are stated in the note and in each row below. Run:

    /home/matt/projects/aquila-v2-spec/sim-python gate_chain_g.py

Outputs (small): gate_chain_summary.csv, fig_gate.png
"""
import csv
import numpy as np
from scipy import stats

rng = np.random.default_rng(20261016)

C = 2.99792458e8
FC = 2.437e9            # channel 6 centre
LAM = C / FC            # 12.297 cm
PPM = 1e-6
DF_RAW = {              # raw relative CFO (both crystals at stated ppm, opposite)
    "10ppm": 10 * PPM * FC,    # 24.4 kHz
    "20ppm": 20 * PPM * FC,    # 48.7 kHz
    "40ppm": 40 * PPM * FC,    # 97.5 kHz
}
NS_GRID = 12.5e-9       # ns RX timestamp grid, s
TAU_LTF = 8e-6          # L-LTF -> HT-LTF separation (within-frame CFO probe), s
DELTA0 = 350e-6         # nominal reply interval (0.7 ms RTT at fixed MCS7-SGI), s
GATE = 0.5              # rad, coherence-gate threshold
SHARP = 0.1             # rad, "sharp" bar (sibling g's R1 pass bar)
SIG_M = 1 / np.sqrt(2 * 64 * 10 ** 0.6)     # 0.0442 rad, Ns=64 @ 6 dB/subcarrier
SIG_M_RATIO = np.sqrt(2) * SIG_M            # ratio of two CSI frames: 0.0625 rad

summary = []


def log(part, param, value, units):
    summary.append((part, param, f"{value:.6g}", units))
    print(f"[{part}] {param:62s} {value:12.5g} {units}")


# =============================================================================
# Part B: closed forms (Wolfram-derived, verified here)
# =============================================================================
print("=" * 100)
print("Part B: closed forms of the tracking layer")
print("=" * 100)


def riccati_ss(a, r, iters=3000):
    """Steady-state posterior of the 2-state KF. x=[theta(rad), rate(rad/step)],
    F=[[1,1],[0,1]], Q=a{{1/3,1/2},{1/2,1}}, a=(2pi)^2 q_f T^3 [rad^2], meas r [rad^2]."""
    F = np.array([[1.0, 1.0], [0.0, 1.0]])
    Q = a * np.array([[1 / 3, 1 / 2], [1 / 2, 1.0]])
    P = np.diag([1.0, 1.0])
    for _ in range(iters):
        Pp = F @ P @ F.T + Q
        K = Pp[:, :1] / (Pp[0, 0] + r)
        P = (np.eye(2) - K @ np.array([[1.0, 0.0]])) @ Pp
    return P


# (i) update-instant posterior saturates at r in the wander-dominated regime
for a_, r_, expect in [(100.0, 0.01, "p11->r"), (3.553e-5, 0.01, "0.00292 (Wolfram DARE check)"),
                       (1e6, 1e-6, "p11->r")]:
    P = riccati_ss(a_, r_)
    log("B", f"posterior p11 a={a_:g} r={r_:g} ({expect})", P[0, 0], "rad^2")

# (ii) bridge (bracket) interpolation: Var(mid) = a/48 + r/2 — MC verification
def bridge_mc(a, r, n=400000):
    return np.std(rng.normal(0, np.sqrt(a / 48), n) + rng.normal(0, np.sqrt(r / 2), n))


for a_, r_ in [(0.05, 0.01), (0.567, 0.01), (6.3, 0.05)]:
    log("B", f"bridge sigma a={a_:g} r={r_:g} closed form",
        np.sqrt(a_ / 48 + r_ / 2), "rad")
    log("B", f"bridge sigma a={a_:g} r={r_:g} MC", bridge_mc(a_, r_), "rad")

# (iii) posterior frequency (Hz) at the design point — governs the product's
# CFO x reply-interval correction
def post_freq_hz(sigma_res, tau_c, T_c, r):
    qf = 2 * sigma_res ** 2 / tau_c
    a_ = (2 * np.pi) ** 2 * qf * T_c ** 3
    P = riccati_ss(a_, r)
    return float(np.sqrt(P[1, 1]) / (2 * np.pi)), P


for T_c in [1e-3, 20e-3]:
    f_post, P = post_freq_hz(30.0, 1.0, T_c, SIG_M_RATIO ** 2)
    log("B", f"posterior residual-CFO sigma (30 Hz OU, Tc={T_c*1e3:g}ms)",
        f_post, "Hz")
    log("B", f"  -> product correction term 2pi f_post Delta0",
        2 * np.pi * f_post * DELTA0, "rad")


# =============================================================================
# Part A: the gate table (per-exchange product residual + between-fix coherence)
# =============================================================================
print("=" * 100)
print("Part A: the coherence gate, two distinct ledger quantities")
print("=" * 100)

SIG_RES = [3.0, 10.0, 30.0, 100.0, 300.0, 1000.0]
CAD = [0.7e-3, 1e-3, 2e-3, 5e-3, 10e-3, 20e-3]


def gate_terms(sigma_res, tau_c, T_c, sigma_jit_us, df_raw,
               sig_art=0.0, motion_hz=0.0, n_avg=4):
    """Two ledger quantities:
    (1) COHERENCE at the sensing frame, bracketed by reference exchanges:
        sigma_coh = sqrt( a/48 + r_eff/2 )   [a=(2pi)^2 qf Tc^3, qf=2 sig_res^2/tau_c]
        r_eff = sig_m_ratio^2/n_avg + (2 pi df_raw sigma_jit)^2
    (2) PRODUCT residual per exchange (the sensing observable itself):
        sigma_prod = sqrt[ (2 pi f_post Delta0)^2 + (2 pi df_raw sigma_jit)^2
                           + sig_m^2/n_avg + sig_art^2 + (2 pi motion_hz Delta0)^2 ]
    """
    qf = 2 * sigma_res ** 2 / tau_c
    a_ = (2 * np.pi) ** 2 * qf * T_c ** 3
    sj = sigma_jit_us * 1e-6
    jit = 2 * np.pi * df_raw * sj
    r_eff = (SIG_M_RATIO ** 2) / n_avg + jit ** 2
    sig_coh = np.sqrt(a_ / 48 + r_eff / 2)
    f_post, _ = post_freq_hz(sigma_res, tau_c, T_c, r_eff)
    sig_prod = np.sqrt((2 * np.pi * f_post * DELTA0) ** 2 + jit ** 2
                       + (SIG_M ** 2) / n_avg + sig_art ** 2
                       + (2 * np.pi * motion_hz * DELTA0) ** 2)
    return sig_coh, sig_prod


TAU_C = 1.0
print("\n(1) sigma_phi at a bracketed sensing frame [rad], 20 ppm, sigma_jit=0.33 us:")
print(f"{'T_c':>8s} | " + " | ".join(f"{s:g}Hz" for s in SIG_RES))
gate_tbl = {}
for T_c in CAD:
    row = [gate_terms(s, TAU_C, T_c, 0.33, DF_RAW["20ppm"])[0] for s in SIG_RES]
    gate_tbl[T_c] = row
    print(f"{T_c*1e3:7.2f}ms | " + " | ".join(f"{v:8.3f}" for v in row))
    for s, v in zip(SIG_RES, row):
        log("A", f"coh sigma_phi Tc={T_c*1e3:g}ms res={s:g}Hz", v, "rad")

print("\nsame, unbracketed (20 ms holdover, no reference frames, worst case):")
for s in SIG_RES:
    v = 2 * np.pi * s * 20e-3 / np.sqrt(3)   # OU holdover rms over 20 ms
    log("A", f"unbracketed holdover sigma_phi res={s:g}Hz 20ms", v, "rad")

print("\n(2) product residual per exchange [rad] (sigma_jit sweep, 20 ppm, res=30Hz):")
for sj in [0.1, 0.33, 1.0, 3.0, 10.0]:
    for T_c in [1e-3, 20e-3]:
        v = gate_terms(30.0, TAU_C, T_c, sj, DF_RAW["20ppm"])[1]
        log("A", f"prod sigma_phi sj={sj:g}us Tc={T_c*1e3:g}ms", v, "rad")

print("\nkilling parameter values (single-term crossings):")
for name, df in DF_RAW.items():
    log("A", f"sigma_jit crossing 0.5 rad @ {name}",
        0.5 / (2 * np.pi * df) * 1e6, "us")
    log("A", f"sigma_jit crossing 0.1 rad @ {name}",
        0.1 / (2 * np.pi * df) * 1e6, "us")
for T_c in [1e-3, 20e-3]:
    for bar, nm in [(GATE, "0.5"), (SHARP, "0.1")]:
        # bridge wander term alone: sigma_res such that sqrt(a/48)=bar
        qf = (bar ** 2 * 48) / ((2 * np.pi) ** 2 * T_c ** 3)
        log("A", f"residual-CFO crossing {nm} rad (wander), Tc={T_c*1e3:g}ms",
            np.sqrt(qf * TAU_C / 2), "Hz")

# motion (non-reciprocity) term over the exchange gap
v = 1.0
f_D = 2 * v / LAM
log("A", "worst-case bistatic Doppler at 1 m/s", f_D, "Hz")
log("A", "motion bias over 0.7 ms exchange", 2 * np.pi * f_D * DELTA0, "rad")
log("A", "motion bias over 20 ms cycle", 2 * np.pi * f_D * 20e-3, "rad")


# =============================================================================
# Part C: ambiguity-resolution cascade
# =============================================================================
print("=" * 100)
print("Part C: ambiguity cascade (FTM ppm seed -> dual-LTF probe -> burst unwrap)")
print("=" * 100)
sig_ltf1 = np.sqrt(2) * SIG_M / (2 * np.pi * TAU_LTF)
log("C", "dual-LTF per-frame CFO sigma", sig_ltf1, "Hz")
log("C", "dual-LTF unambiguous CFO range", 1 / (2 * TAU_LTF), "Hz")
log("C", "FTM seed max error (1 ppm resolution)", 0.5 * 2.4e3, "Hz (<< probe range: seed always resolves the probe branch)")
for K in [4, 16, 64]:
    log("C", f"dual-LTF CFO sigma after {K} frames", sig_ltf1 / np.sqrt(K), "Hz")


def cascade_success(sigma_res, tau_c, T_b, K=16, n_mc=100000, rng=rng):
    """P(stage-2 unwrap succeeds): stage-1 (dual-LTF, K frames) error + OU wander
    over one 20 ms cycle must stay inside |df| < 1/(2 T_b)."""
    lim = 1 / (2 * T_b)
    est_err = rng.normal(0, sig_ltf1 / np.sqrt(K), n_mc)
    e = 20e-3 / tau_c
    wander = sigma_res * np.sqrt(1 - np.exp(-e)) * rng.standard_normal(n_mc)
    return float(np.mean(np.abs(est_err + wander) < lim))


for T_b in [0.7e-3, 1e-3, 2e-3, 5e-3, 20e-3]:
    for sres in [30.0, 100.0]:
        p = cascade_success(sres, TAU_C, T_b)
        log("C", f"unwrap success T_b={T_b*1e3:g}ms res={sres:g}Hz", p, "frac")


# =============================================================================
# Part D: end-to-end chain: products -> integer resolution -> position
# =============================================================================
print("=" * 100)
print("Part D: the chain: coherence -> integer resolution -> 3D position")
print("=" * 100)


def room_case(n_nodes):
    if n_nodes == 6:
        pos = np.array([[0.2, 0.2, 2.3], [3.0, 0.2, 2.3], [5.8, 0.2, 2.3],
                        [0.2, 3.8, 2.3], [3.0, 3.8, 2.3], [5.8, 3.8, 2.3]])
    elif n_nodes == 8:
        pos = np.array([[0.2, 0.2, 2.3], [3.0, 0.2, 2.3], [5.8, 0.2, 2.3],
                        [0.2, 3.8, 2.3], [3.0, 3.8, 2.3], [5.8, 3.8, 2.3],
                        [0.2, 2.0, 0.3], [5.8, 2.0, 0.3]])
    else:
        pos = np.array([[0.2, 0.2, 2.3], [5.8, 0.2, 2.3],
                        [0.2, 3.8, 2.3], [5.8, 3.8, 2.3]])
    return pos, np.array([3.2, 1.9, 1.0])


def crlb_position(pos, tgt, sig_phi, n_avg=1):
    """CRLB of target position from product-phase range-sums.
    sigma_L = lam * sigma_phi / (2 pi sqrt(n_avg)); Fisher over all node pairs."""
    sig_L = LAM * sig_phi / (2 * np.pi) / np.sqrt(n_avg)
    J = np.zeros((3, 3))
    for i in range(len(pos)):
        for j in range(i + 1, len(pos)):
            g = (tgt - pos[i]) / np.linalg.norm(tgt - pos[i]) + \
                (tgt - pos[j]) / np.linalg.norm(tgt - pos[j])
            J += np.outer(g, g) / sig_L ** 2
    C = np.linalg.inv(J)
    return float(np.sqrt(np.trace(C)))


for n_nodes in [4, 6, 8]:
    pos, tgt = room_case(n_nodes)
    for sig_phi in [0.05, 0.1, 0.3, 0.5, 1.0]:
        log("D", f"position CRLB N={n_nodes} nodes sig_phi={sig_phi:g}rad",
            crlb_position(pos, tgt, sig_phi), "m")


def integer_resolution(n_nodes, sig_phi, sig_cal, prior_m=1.5,
                       n_mc=200, n_false=20000, rng=rng):
    """RTK-style joint integer resolution by position-candidate testing.
    Per link: phi_obs = -2 pi L/lam + cal_err + noise (cal_err ~ N(0,sig_cal),
    static-field calibration quality; noise ~ N(0,sig_phi)).
    Success = the true position passes the chi-square consistency test at 95%
    AND no uniformly sampled false candidate in the prior volume passes it.
    (Analytic expected-false count also reported, as the MC is sampled.)"""
    pos, tgt = room_case(n_nodes)
    pairs = [(i, j) for i in range(len(pos)) for j in range(i + 1, len(pos))]
    E = len(pairs)
    thr = stats.chi2.ppf(0.95, E)
    sig_eff = np.sqrt(sig_phi ** 2 + sig_cal ** 2)
    # analytic expected false matches in the prior volume:
    # per-link phase tolerance: solve sum_{e in E} (dphi_e/sig_eff)^2 <= thr for
    # iid uniform phases is not analytic; use the per-link 1-D match probability
    # at the chi-square threshold via effective tol: tol = sig_eff*sqrt(thr/E)
    tol = sig_eff * np.sqrt(thr / E)
    p1 = 2 * tol / (2 * np.pi)          # |wrap phase| < tol for a random position
    n_cells = (4 / 3 * np.pi * prior_m ** 3) / (LAM / 4) ** 3
    exp_false = n_cells * p1 ** E
    # Monte Carlo
    wins = 0
    for _ in range(n_mc):
        d1 = np.array([np.linalg.norm(pos[i] - tgt) for i, j in pairs])
        d2 = np.array([np.linalg.norm(pos[j] - tgt) for i, j in pairs])
        phi_obs = (-2 * np.pi * (d1 + d2) / LAM + rng.normal(0, sig_cal, E)
                   + rng.normal(0, sig_phi, E))
        # true-position statistic
        res = np.angle(np.exp(1j * phi_obs - 1j * (-2 * np.pi * (d1 + d2) / LAM)))
        chi2_true = float(np.sum(res ** 2)) / sig_eff ** 2
        if chi2_true > thr:
            continue
        # false candidates
        p = tgt + rng.uniform(-prior_m, prior_m, (n_false, 3))
        ok = np.ones(n_false, bool)
        for (i, j) in pairs:
            dd1 = np.sqrt(np.sum((pos[i] - p) ** 2, axis=1))
            dd2 = np.sqrt(np.sum((pos[j] - p) ** 2, axis=1))
            dphi = np.angle(np.exp(1j * phi_obs[pairs.index((i, j))]
                                   + 1j * (2 * np.pi * (dd1 + dd2) / LAM)))
            ok &= (dphi ** 2) < 9 * sig_eff ** 2
        if not np.any(ok):
            wins += 1
    return wins / n_mc, exp_false


for (n_nodes, sig_phi, sig_cal) in [(6, 0.1, 0.1), (6, 0.3, 0.1), (6, 0.3, 0.3),
                                    (6, 0.5, 0.3), (4, 0.3, 0.1), (8, 0.3, 0.3)]:
    p_ok, ef = integer_resolution(n_nodes, sig_phi, sig_cal)
    log("D", f"integer-resolution success N={n_nodes} sphi={sig_phi} scal={sig_cal}",
        p_ok, "frac")
    log("D", f"  expected false candidates (analytic)", ef, "count")

# differential (motion) mode: no integers needed
for sig_phi in [0.1, 0.3, 0.5]:
    log("D", f"differential range-sum precision per link (sphi={sig_phi})",
        LAM * sig_phi / (2 * np.pi), "m")


# =============================================================================
# Part E: reference types under motion; RIPS beat
# =============================================================================
print("=" * 100)
print("Part E: reference-type comparison; RIPS beat-phase CRB")
print("=" * 100)
log("E", "target coherence time lam/(2v) at 1 m/s", LAM / (2 * v), "s")
for gap_ms in [0.7, 1, 5, 20, 61]:
    log("E", f"motion bias over gap={gap_ms:g} ms", 2 * np.pi * f_D * gap_ms * 1e-3, "rad")

# RIPS beat-phase CRB: complex tone, K samples, per-sample SNR rho: 1/sqrt(K rho)
for K, rho_db in [(256, 10), (80000, 10), (80000, 0), (80000, -10)]:
    rho = 10 ** (rho_db / 10)
    crb = 1 / np.sqrt(K * rho)
    # MC check for the two smaller ones
    if K == 256:
        w = (rng.normal(0, np.sqrt(0.5), (4000, K))
             + 1j * rng.normal(0, np.sqrt(0.5), (4000, K)))
        t = np.arange(K) - (K - 1) / 2
        s = np.exp(1j * (2 * np.pi * 7.3e-3 * t + 0.7))
        y = np.sqrt(rho) * s + w
        ph = np.angle(y @ np.conj(s))
        log("E", f"beat-phase sigma MC K={K} rho={rho_db}dB",
            float(np.std(np.angle(np.exp(1j * (ph - np.angle(np.sqrt(rho) * s @ np.conj(s))))))), "rad")
    log("E", f"beat-phase CRB K={K} rho={rho_db}dB", crb, "rad")

# ledger: ambiguity cascade scales
log("E", "range resolution per epoch, 40 MHz", C / 2 / 40e6, "m")
log("E", "range resolution, 13-ch stitched (83.5 MHz)", C / 2 / 83.5e6, "m")
log("E", "range-sum ambiguity (product phase)", LAM, "m")
log("E", "monostatic-equivalent ambiguity (R = L/2)", LAM / 2, "m")


# =============================================================================
# Part F: falsifier power analysis + airtime ledger
# =============================================================================
print("=" * 100)
print("Part F: falsifier power analysis and airtime")
print("=" * 100)


def n_samples(sig_a, sig_b, alpha=0.05, power=0.90):
    for n in range(4, 60000):
        thr = sig_a ** 2 * stats.chi2.ppf(1 - alpha, n - 1) / (n - 1)
        if stats.chi2.sf(thr * (n - 1) / sig_b ** 2, n - 1) >= power:
            return n
    return -1


for (a, b) in [(0.1, 0.3), (0.3, 1.0), (0.05, 0.1), (0.1, 0.5)]:
    log("F", f"windows to discriminate sigma={a} vs {b} (alpha .05, power .90)",
        n_samples(a, b), "windows")

frame_us = 300
for n_nodes in [6, 10, 20]:
    for T_c in [20e-3, 5e-3, 1e-3]:
        duty = (n_nodes - 1) * 2 * frame_us * 1e-6 / T_c
        log("F", f"airtime duty, star N={n_nodes} Tc={T_c*1e3:g}ms", duty * 100, "%")
for n_nodes in [6, 10]:
    duty = n_nodes * (n_nodes - 1) / 2 * 2 * frame_us * 1e-6 / 20e-3
    log("F", f"airtime duty, all-pairs N={n_nodes} Tc=20ms", duty * 100, "%")

# coherence budget B = dnu * T_obs [cycles]
for T_obs in [0.7e-3, 20e-3]:
    for dnu in [30, 100]:
        log("F", f"B = dnu*T_obs = {dnu}Hz*{T_obs*1e3:g}ms", dnu * T_obs, "cycles")

# =============================================================================
with open("gate_chain_summary.csv", "w", newline="") as fh:
    w = csv.writer(fh)
    w.writerow(["part", "param", "value", "units"])
    w.writerows(summary)
print(f"\nwrote gate_chain_summary.csv ({len(summary)} rows)")

try:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    fig, ax = plt.subplots(1, 3, figsize=(11, 3.3))
    for s in SIG_RES[:5]:
        ax[0].loglog([t * 1e3 for t in CAD],
                     [gate_terms(s, TAU_C, t, 0.33, DF_RAW["20ppm"])[0] for t in CAD],
                     marker="o", ms=3, label=f"residual {s:g} Hz")
    ax[0].axhline(GATE, color="k", ls="--", lw=0.8, label="gate 0.5 rad")
    ax[0].axhline(SHARP, color="r", ls=":", lw=0.8, label="sharp 0.1 rad")
    ax[0].set_xlabel("bracket cadence $T_c$ [ms]")
    ax[0].set_ylabel(r"$\sigma_\phi$ (sensing frame) [rad]")
    ax[0].set_title("A. gate: bracketed coherence\n(20 ppm, $\\sigma_{jit}$=0.33 µs)")
    ax[0].legend(fontsize=6)
    ax[0].grid(True, which="both", alpha=0.3)

    sp = np.array([0.05, 0.1, 0.2, 0.3, 0.5, 1.0])
    for n_nodes in [4, 6, 8]:
        pos, tgt = room_case(n_nodes)
        ax[1].loglog(sp, [crlb_position(pos, tgt, x) for x in sp],
                     marker="s", ms=3, label=f"N={n_nodes} nodes")
    ax[1].set_xlabel(r"$\sigma_\phi$ [rad]")
    ax[1].set_ylabel("3-D position CRLB [m]")
    ax[1].set_title("D. chain: coherence -> position")
    ax[1].legend(fontsize=7)
    ax[1].grid(True, which="both", alpha=0.3)

    gaps = np.array([0.1, 0.3, 0.7, 1, 2, 5, 10, 20, 61]) * 1e-3
    ax[2].semilogy(gaps * 1e3, 2 * np.pi * f_D * gaps, marker="o", ms=3,
                   label="motion bias $2\\pi f_D T$")
    ax[2].axhline(GATE, color="k", ls="--", lw=0.8)
    ax[2].axhline(SHARP, color="r", ls=":", lw=0.8)
    ax[2].axvline(0.7, color="g", ls="-.", lw=0.8, label="0.7 ms exchange")
    ax[2].set_xlabel("reference-to-sensing gap [ms]")
    ax[2].set_ylabel("bias [rad]")
    ax[2].set_title("E. motion bias (v = 1 m/s)")
    ax[2].legend(fontsize=6)
    ax[2].grid(True, which="both", alpha=0.3)
    fig.tight_layout()
    fig.savefig("fig_gate.png", dpi=110)
    print("wrote fig_gate.png")
except Exception as exc:  # noqa: BLE001
    print("figure skipped:", exc)
