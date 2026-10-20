#!/usr/bin/env python
"""Round-2 T1 (character `a`): the coherence gate — end-to-end chain.

Companion to `22-invention-{a,c,s,t,k}.md` and `ROUND2-BRIEF.md` (Finding 4).
Where the sibling `coherence-gate-f` script maps the M2 residual itself, this
script carries the gate *through* to what it buys: the two-way exchange
estimator, the face/packet averaging law, the ambiguity cascade, and a complete
simulated chain  conducted calibration -> fence coherence -> coherent 3D
localisation  with an error budget, plus a dedicated treatment of the missing
round-1 direction f (phase-swept beacon / RIPS-style hub protocol).

SIMULATION ONLY — a NumPy model of estimators and oscillator processes, not a
measurement of any device (per EXPERIMENT-ENV.md honesty rules). Parameters are
stated at each part. Run:
    /home/matt/projects/aquila-v2-spec/sim-python coherence_gate_chain.py
Outputs (kept small): gate_a_summary.csv, gate_a_fig1.png
"""
import csv
import numpy as np

rng = np.random.default_rng(20261016)  # reproducible

C = 2.99792458e8
FC = 2.437e9                    # Wi-Fi channel 6 centre (2,437 MHz)
LAM = C / FC                    # 0.12305 m
PPM = 1e-6
DF_RAW_20PPM = 20 * PPM * FC   # 48.74 kHz relative CFO at 20 ppm
TS_GRID = 12.5e-9               # ns RX timestamp latch grid (ESP-IDF)
# Estimator noise constants (used by Part A2's CSI-phase CFO ledger and Part B):
NSUB = 64
SNR_DB = 10.0
SIG_EST = 1 / np.sqrt(2 * NSUB * 10 ** (SNR_DB / 10))   # 0.028 rad
SIG_DNU = 0.2     # Hz, fine CFO from the CSI-phase ledger (A2), not timestamps
ROWS = []                       # ledger rows for CSV


def log(part, param, value, units, note=""):
    ROWS.append([part, param, f"{value:.6g}", units, note])


# =============================================================================
# Part A — holdover of the cross-node LO phase after CFO removal
#          + CFO estimation from the nanosecond RX timestamps
# =============================================================================
TAU_C = 1.0                      # correlation time of residual CFO, s (Part A)
SIG_F = [2.4, 10.0, 30.0, 100.0]  # residual-CFO 1-sigma, Hz
#   2.4 Hz   = 1 ppb ESP-PPB-class disciplined VCTCXO (grey literature)
#   10-100 Hz = post-CFO-removal residual (MegaMIMO / ROUND2-BRIEF Finding 4)
GAPS_MS = [0.1, 0.3, 1.0, 3.0, 8.0, 20.0, 100.0]


def ou_paths(sigma_f, tau_c, t_max, n_real):
    """Exact OU discretisation of residual-CFO-driven LO phase (rad)."""
    e = np.exp(-DT / tau_c)
    sd = sigma_f * np.sqrt(1 - e * e)
    th = np.empty((n_real, int(t_max / DT) + 1))
    f = sigma_f * rng.standard_normal(n_real)
    th[:, 0] = 0.0
    for k in range(th.shape[1] - 1):
        f = e * f + sd * rng.standard_normal(n_real)
        th[:, k + 1] = th[:, k] + 2 * np.pi * f * DT
    return th


def var_ou_integral(sigma_f, tau_c, delta):
    """Var[int_0^delta f dt] for OU(f; sigma_f, tau_c). Wolfram-verified."""
    return 2 * sigma_f**2 * tau_c * (delta - tau_c * (1 - np.exp(-delta / tau_c)))


NREAL = 400
DT = 2.5e-5                     # 25 us integration step (resolves 0.1 ms gaps)


def ou_paths(sigma_f, tau_c, t_max, n_real):
    """Exact OU residual-CFO-driven LO phase (rad); trapezoid integral."""
    e = np.exp(-DT / tau_c)
    sd = sigma_f * np.sqrt(1 - e * e)
    n_steps = int(round(t_max / DT))
    th = np.empty((n_real, n_steps + 1))
    f = sigma_f * rng.standard_normal(n_real)
    th[:, 0] = 0.0
    for k in range(n_steps):
        f_new = e * f + sd * rng.standard_normal(n_real)
        th[:, k + 1] = th[:, k] + 2 * np.pi * 0.5 * (f + f_new) * DT
        f = f_new
    return th


print("== A1: untracked holdover sigma_phi(T_gap) — MC vs closed form ==")
hold = {}
for sf in SIG_F:
    th = ou_paths(sf, TAU_C, 0.12, NREAL)
    hold[sf] = {}
    for g in GAPS_MS:
        i = int(round(g * 1e-3 / DT))
        mc = float(np.std(th[:, i] - th[:, 0], ddof=1))
        hold[sf][g] = mc
        cf = 2 * np.pi * np.sqrt(var_ou_integral(sf, TAU_C, g * 1e-3))
        log("A_holdover", f"sigma_f={sf:g}Hz,gap={g:g}ms", mc, "rad",
            f"closed-form {cf:.4f}")
print("\n  sigma_phi[rad] vs gap[ms]; rows = residual-CFO sigma")
print("  gap->  " + "".join(f"{g:>9g}" for g in GAPS_MS))
for sf in SIG_F:
    print(f"  {sf:5.1f}Hz " + "".join(f"{hold[sf][g]:9.3f}" for g in GAPS_MS))
print("  MC vs closed form at 1 ms: " +
      ", ".join(f"{sf:g}Hz:{hold[sf][1.0]:.3f}/{2*np.pi*np.sqrt(var_ou_integral(sf, TAU_C, 1e-3)):.3f}"
                for sf in SIG_F))
for sf in SIG_F:
    log("A_holdover", f"T* at 0.5 rad, sigma_f={sf:g}Hz",
        0.5 / (2 * np.pi * sf) * 1e3, "ms", "short-gap budget 2 pi sigma_f T")

# --- A3: coherence budget B = sigma_f * T_obs (shared ledger) + kill values ---
B_GATE = 0.5 / (2 * np.pi)      # 0.0796 cycles; short-gap law sigma_phi ~ 2 pi B
log("A_budget", "B gate (0.5 rad, short-gap)", B_GATE, "cycles",
    "sigma_phi ~ 2 pi sigma_f T_gap; B = sigma_f T_gap")
for sf in SIG_F:
    b20 = sf * 0.020
    log("A_budget", f"B @20ms,sigma_f={sf:g}Hz", b20, "cycles",
        "PASS" if b20 <= B_GATE else "FAIL")
log("A_budget", "killing sigma_f at 20 ms", B_GATE / 0.020, "Hz",
    "0.5 rad unbracketed at T_gap = 20 ms")
log("A_budget", "killing sigma_tau at 20 ppm",
    0.5 / (2 * np.pi * DF_RAW_20PPM) * 1e6, "us",
    "2 pi df_raw sigma_tau = 0.5 rad")

# --- A2: CFO precision from the ns RX timestamps (exchange timing ledger) ---
# Each side stamps frames on a 12.5 ns grid; latch jitter 25 ns 1-sigma
# (characterisation pending on S3; grid is ESTABLISHED from ESP-IDF).


def cfo_from_timestamps(n, Ts, sig_j=25.0e-9, n_real=4000):
    """LS-slope sigma of the RELATIVE CFO estimate from n stamped exchanges.
    The timestamp slope is fractional (s/s); multiply by fc for Hz."""
    t = np.arange(n) * Ts
    sig_t = np.sqrt(TS_GRID ** 2 / 12 + sig_j ** 2)
    y = sig_t * rng.standard_normal((n_real, n))
    tc = t - t.mean()
    slopes = (y @ tc) / float(np.sum(tc ** 2))     # fractional
    closed = sig_t * np.sqrt(12 / (n * (n * n - 1) * Ts ** 2)) * FC
    return float(np.std(slopes, ddof=1)) * FC, closed


print("\n== A2: relative-CFO precision from ns RX timestamps ==")
for n, Ts in [(10, 5e-3), (20, 5e-3), (50, 20e-3), (100, 10e-3)]:
    mc, cf = cfo_from_timestamps(n, Ts)
    print(f"  n={n:3d} over {n*Ts*1e3:5.0f} ms: sigma_dnu MC {mc:8.2f} Hz,"
          f" closed {cf:8.2f} Hz -> phase over 1 ms = {2*np.pi*mc*1e-3:.3f} rad")
    log("A_timestamp_cfo", f"n={n},Ts={Ts*1e3:g}ms", mc, "Hz",
        f"closed {cf:.2f}; phase/1ms={2*np.pi*mc*1e-3:.3f} rad")
print("  NOTE: timestamps alone reach ~21 Hz only after ~1 s; that is however")
print("  enough to unwrap 5 ms-spaced CSI phases (2 pi x 21 x 5 ms = 0.66 rad")
print("  < pi), after which the CSI PHASE slope supplies the fine CFO:")
# CSI-phase CFO: n frames, spacing Ts, per-frame phase noise sigma_est
for n, Ts in [(20, 5e-3), (100, 5e-3)]:
    s = SIG_EST * np.sqrt(12 / (n * (n * n - 1))) / Ts / (2 * np.pi)
    print(f"    CSI-phase CFO, n={n} over {n*Ts*1e3:3.0f} ms: sigma = {s:.3f} Hz"
          f" -> phase over 1 ms = {2*np.pi*s*1e-3:.4f} rad")
    log("A_timestamp_cfo", f"CSI-phase CFO n={n},Ts={Ts*1e3:g}ms", s, "Hz",
        f"phase/1ms={2*np.pi*s*1e-3:.4f} rad")

# =============================================================================
# Part B — the two-way exchange (ping + unique reply) as an estimator
# =============================================================================
# Product phase (LO-free): err per exchange =
#   2 pi df_raw dtau  (turnaround error x raw CFO; mean tau absorbed in the
#                      per-link calibration constant, so dtau = jitter only)
# + sqrt(2) x 2 pi sqrt(Var_ou(tau))   (LO wander of BOTH nodes over tau)
# + sqrt(2) x sigma_est               (two CSI phase estimates)
# Corrected path: dtau -> sqrt(asym^2 + grid^2/12), plus the CFO-estimator
# residual 2 pi sigma_dnu tau (sigma_dnu from A2).
# (NSUB/SNR_DB/SIG_EST/SIG_DNU are defined with the module constants above,
#  because Part A2's CSI-phase CFO ledger uses SIG_EST before Part B runs.)


def twoway_run(sigma_f=30.0, tau_c=1.0, df_raw=DF_RAW_20PPM,
               sig_tau_us=1.0, tail_p=0.10, tail_us=8.0, gate_us=None,
               sig_asym_ns=30.0, n_frames=2000):
    """MC of per-exchange errors: returns (raw product, corrected product,
    ratio-fix) 1-sigma in rad."""
    tau_nom = 1e-3
    wander = np.sqrt(2) * 2 * np.pi * np.sqrt(var_ou_integral(sigma_f, tau_c, tau_nom))
    mnoise = np.sqrt(2) * SIG_EST
    corr_jit = 2 * np.pi * df_raw * np.sqrt((sig_asym_ns * 1e-9) ** 2
                                            + TS_GRID ** 2 / 12)
    out = np.empty((n_frames, 3))
    for k in range(n_frames):
        dtau = rng.laplace(0, sig_tau_us * 1e-6 / np.sqrt(2))
        if rng.random() < tail_p:
            dtau += rng.choice([-1, 1]) * tail_us * 1e-6
        if gate_us is not None:                     # deferral gate (retry)
            while abs(dtau) > gate_us * 1e-6:
                dtau = rng.laplace(0, sig_tau_us * 1e-6 / np.sqrt(2))
                if abs(dtau) <= gate_us * 1e-6 and rng.random() < tail_p:
                    dtau = rng.choice([-1, 1]) * tail_us * 1e-6
        w = wander * rng.standard_normal()
        m = mnoise * rng.standard_normal()
        out[k, 0] = 2 * np.pi * df_raw * dtau + w + m          # raw product
        out[k, 1] = (corr_jit * rng.standard_normal()
                     + 2 * np.pi * SIG_DNU * tau_nom * rng.standard_normal()
                     + w + m)                                  # corrected product
        out[k, 2] = corr_jit * rng.standard_normal() + m        # ratio fix
    return tuple(float(np.std(out[:, j], ddof=1)) for j in range(3))


print("\n== B: two-way exchange (1 ms nominal reply, 20 ppm raw CFO, wander 30 Hz) ==")
raw = twoway_run()
print(f"  {'raw software reply (1 us laplace + 10% 8 us tail)':48s}"
      f" -> raw_product={raw[0]:6.3f} rad (CFO x jitter dominated)")
log("B_twoway", "raw software reply", raw[0], "rad", "uncorrected product")
for label, gate in [("ns-timestamp corrected (asym 30 ns)", None),
                    ("corrected + 3 us deferral gate", 3.0),
                    ("corrected + 1 us deferral gate", 1.0)]:
    r = twoway_run(gate_us=gate, sig_asym_ns=30.0)
    print(f"  {label:48s} -> corr_product={r[1]:6.3f}  ratio_fix={r[2]:6.3f} rad")
    log("B_twoway", label, r[1], "rad", f"ratio {r[2]:.3f}")

for v in [0.5, 1.0]:                 # moving-scene non-reciprocity over tau
    sig_nr = 2 * 2 * np.pi * (v / LAM) * 1e-3
    print(f"  non-reciprocity @ {v} m/s radial, tau=1 ms: {sig_nr:.3f} rad (two-way)")
    log("B_nonrecip", f"v={v}m/s,tau=1ms", sig_nr, "rad", "two-way Doppler residue")

# =============================================================================
# Part C — face/packet averaging law
# =============================================================================
print("\n== C: face/packet averaging law (sigma_f=30 Hz, 1 ms gap, tau_c=1 s) ==")
sigma_wander = 2 * np.pi * np.sqrt(var_ou_integral(30.0, TAU_C, 1e-3))
sigma_artefact = 0.05    # per-frame CFO-compensation intercept, rad (ESP-PPB class)
sigma_bias = 0.10        # static per-face multipath/chain bias, rad (frozen)
Nfaces = [1, 2, 4, 8, 16, 20, 40]
Mframes = 4000
print(f"  components: wander={sigma_wander:.3f} artefact={sigma_artefact:.3f}"
      f" est={SIG_EST:.4f} bias(frozen)={sigma_bias:.3f} rad")
print("  (analytic: sigma_N = sqrt(wander^2 + artefact^2 + (est^2+bias^2)/N))")
print("  N | analytic | MC check | beta")
sn, prev = [], None
for n in Nfaces:
    # analytic snapshot sigma after averaging n faces (est + frozen bias
    # average across faces; wander and per-frame artefact do not)
    snap = float(np.sqrt(sigma_wander ** 2 + sigma_artefact ** 2
                         + (SIG_EST ** 2 + sigma_bias ** 2) / n))
    # MC check with independent draws per face; bias frozen within a run,
    # re-drawn 20x so the reported RMS reflects the ensemble bias sigma
    mc2 = []
    for _ in range(20):
        wander = sigma_wander * rng.standard_normal(Mframes)
        artef = sigma_artefact * rng.standard_normal(Mframes)
        est = SIG_EST * rng.standard_normal((Mframes, n))
        bias = sigma_bias * rng.standard_normal(n)
        per_frame = wander + artef + est.mean(axis=1) + bias.mean()
        mc2.append(float(np.sqrt(np.mean(per_frame ** 2))))
    mc = float(np.mean(mc2))
    beta = "-" if prev is None else f"{-np.log(snap/prev)/np.log(n/Nfaces[Nfaces.index(n)-1]):.2f}"
    sn.append(snap)
    print(f"  {n:3d} | {snap:8.4f} | {mc:8.4f} | {beta:>4}")
    log("C_averaging", f"N_faces={n}", snap, "rad", f"MC {mc:.4f}; beta {beta}")
    prev = snap
print("  -> beta starts at 0.5 only while est+bias dominates; here wander+artefact")
print("     dominate from N=1, so face averaging buys ~nothing at the gate.")
log("C_averaging", "wander floor at 1 ms", sigma_wander, "rad", "dominant term")

# =============================================================================
# Part D — the ambiguity cascade (FTM -> stitched ~83 MHz -> carrier phase)
# =============================================================================
LAM2 = LAM / 2
BAND = 2482e6 - 2402e6            # stitched 13x20 MHz Wi-Fi band (ch1-ch13)
RULER_BAND = C / (2 * BAND)        # two-way delay ruler of the synth band

print("\n== D: ambiguity cascade ==")
print(f"  two-way carrier ruler lam/2 = {LAM2*100:.2f} cm;"
      f" stitched-band ruler = {RULER_BAND:.3f} m")
from scipy.stats import norm  # noqa: E402
for sig_prior in [0.1, 0.3, 0.5, 1.0, 2.0, 3.5]:
    pc = float(2 * norm.cdf(LAM2 / 2 / sig_prior) - 1)
    pb = float(2 * norm.cdf(RULER_BAND / 2 / sig_prior) - 1)
    print(f"  prior sigma={sig_prior:4.1f} m: carrier-integer {pc*100:5.1f}%"
          f" | band-integer {pb*100:5.1f}%")
    log("D_cascade", f"prior={sig_prior:g}m", pc, "frac", f"band {pb:.3f}")


def band_sigma(ns=64, rho_db=10.0):
    """Two-way range precision from an 80 MHz stitched band (MCRB-class)."""
    return C / (2 * np.pi * BAND * np.sqrt(2 * ns * 10 ** (rho_db / 10)))


print("\n  cascade rung 2->3 (band -> carrier), per link:")
for rho_db in [0, 10, 20]:
    sb = band_sigma(64, rho_db)
    p2 = float(2 * norm.cdf(LAM2 / 2 / sb) - 1)
    print(f"    stitched-band range sigma = {sb*100:5.2f} cm at rho={rho_db:2d} dB"
          f" -> carrier-integer success {p2*100:5.1f}%")
    log("D_cascade", f"band->carrier, rho={rho_db}dB", p2, "frac",
        f"band range sigma {sb*100:.2f} cm")


def mean_integer_degeneracy(sigma_phi=0.15, sigma_prior=1.0, n_links=100,
                             n_real=20):
    """Carrier-only grid search: the spread cancels into per-link integers,
    so candidates a multiple of the ruler apart fit equally well. Count the
    near-minimal candidates (within 5% of the best residual) -> implied
    success = 1/count."""
    b0 = 5.0
    spread = rng.uniform(-0.15, 0.15, n_links)
    r0 = b0 + spread
    step = LAM2 / 2
    cand = np.arange(b0 - 2 * sigma_prior, b0 + 2 * sigma_prior, step)
    counts, oks = [], []
    for _ in range(n_real):
        phi = 4 * np.pi * r0 / LAM + sigma_phi * rng.standard_normal(n_links) \
            + 2 * np.pi * rng.integers(-40, 41, n_links)
        res = np.empty(len(cand))
        for k, b in enumerate(cand):
            rhat = b + spread
            n_int = np.round((phi - 4 * np.pi * rhat / LAM) / (2 * np.pi))
            res[k] = np.std(phi - 4 * np.pi * rhat / LAM - 2 * np.pi * n_int)
        nmin = int(np.sum(res < res.min() * 1.05))
        counts.append(nmin)
        # an arbitrary near-minimum is picked; success iff it is the true one
        oks.append(1.0 / nmin)
    return float(np.mean(counts)), float(np.mean(oks))


print("\n  mean-integer degeneracy (carrier only, rigid grid does NOT fix it):")
for sp in [0.5, 1.0, 2.0]:
    nmin, succ = mean_integer_degeneracy(0.15, sp)
    print(f"    prior +-{2*sp:.1f} m window: ~{nmin:.0f} near-equal minima"
          f" (one per {LAM2*1e3:.1f} cm ruler); implied success {succ*100:5.1f}%")
    log("D_degeneracy", f"prior=+-{2*sp:g}m", succ, "frac",
        f"~{nmin:.0f} near-equal minima")

# =============================================================================
# Part E — END-TO-END CHAIN: conducted calibration -> fence -> 3D localisation
# =============================================================================
# A conducted-coherent 20-face puck (icosahedron, 30 cm) is the sole TX; six
# satellite S3 nodes receive. Static field perfectly removed (baseline); the
# target's bistatic sum phase per (face s, node i):
#   phi_si = 2 pi (d(p, s) + d(p, i)) / lam + eps_puck + eps_i + nu_si
# eps_puck, eps_i: per-snapshot fence residuals (the M2 quantity); nu: CSI
# estimator noise. NOTE the *separability theorem*: phi's target part is
# exactly additively separable over (s, i), so if gauges are re-solved freely
# per snapshot the target is UNIDENTIFIABLE — checked numerically below. The
# global constant (U(1)) is projected out (unobservable); that also removes
# the TX-side eps_puck. Only eps_i propagates.
phi_g = (1 + np.sqrt(5)) / 2
verts = np.array([[0, 1, phi_g], [0, 1, -phi_g], [0, -1, phi_g], [0, -1, -phi_g],
                 [1, phi_g, 0], [1, -phi_g, 0], [-1, phi_g, 0], [-1, -phi_g, 0],
                 [phi_g, 0, 1], [-phi_g, 0, 1], [phi_g, 0, -1], [-phi_g, 0, -1]])
verts = verts / np.linalg.norm(verts[0])
edge = 2 / np.sqrt(1 + phi_g ** 2) * 1.0        # edge length after normalising
centres = []
for a in range(12):                            # 20 triangular face centres
    for b in range(a + 1, 12):
        for c in range(b + 1, 12):
            if (np.linalg.norm(verts[a] - verts[b]) < edge * 1.01 and
                np.linalg.norm(verts[b] - verts[c]) < edge * 1.01 and
                np.linalg.norm(verts[a] - verts[c]) < edge * 1.01):
                centres.append(verts[[a, b, c]].mean(axis=0))
faces = np.array(centres)
faces = faces / np.linalg.norm(faces, axis=1, keepdims=True) * 0.15
faces = faces + np.array([0.2, 0.2, 2.2])       # puck centre, room corner
nodes = np.array([[0.2, 5.8, 2.0], [7.8, 0.2, 2.4], [7.8, 5.8, 1.8],
                  [4.0, 0.2, 2.6], [4.0, 5.8, 1.4], [7.8, 3.0, 0.4]])
p_true = np.array([4.0, 3.0, 1.2])
NS, NI = len(faces), len(nodes)
NO = NS * NI
assert NS == 20, f"expected 20 faces, got {NS}"

s_idx, i_idx = np.meshgrid(np.arange(NS), np.arange(NI), indexing="ij")
s_idx, i_idx = s_idx.ravel(), i_idx.ravel()
fs = faces[s_idx]
ni = nodes[i_idx]


def sum_dist(p):
    return np.linalg.norm(p - fs, axis=1) + np.linalg.norm(p - ni, axis=1)


def jac_p(p):
    d1 = np.linalg.norm(p - fs, axis=1)
    d2 = np.linalg.norm(p - ni, axis=1)
    u1 = (p - fs) / d1[:, None]
    u2 = (p - ni) / d2[:, None]
    return 2 * np.pi / LAM * (u1 + u2)


print("\n== E0: separability theorem (free gauges kill one-way phase sensing) ==")
G = np.hstack([np.eye(NS)[s_idx], np.eye(NI)[i_idx]])   # gauge design (s+i model)
J = jac_p(p_true)
Qg, _ = np.linalg.qr(G)
Portho = np.eye(NO) - Qg @ Qg.T
resid = np.linalg.norm(Portho @ J, axis=0)
print(f"  gauge-subspace dimension: {np.linalg.matrix_rank(G)} of {NO} obs")
print(f"  |projection of J_p off the gauge span| per column: {resid}")
print(f"  -> max {resid.max():.2e} (machine zero): target signature lies INSIDE"
      " the gauge span; free-gauge re-solve is unidentifiable.")
log("E_separability", "max |P_orth J_p|", float(resid.max()), "-",
    "one-way sum phase additively separable")

# frozen-gauge GLS CRLB: unknowns (p, global const); covariance =
# eps_i (per-RX, common across faces) + nu.
print("\n== E1: frozen-gauge chain: CRLB and Monte-Carlo vs sigma_phi ==")
F = J.T @ J
def gls_crlb(sigma_phi, sigma_n=SIG_EST):
    Sig = np.eye(NO) * sigma_n ** 2
    for i in range(NI):
        m = i_idx == i
        Sig[np.ix_(m, m)] += sigma_phi ** 2
    SigI = np.linalg.inv(Sig)
    Jg = np.hstack([J, np.ones((NO, 1))])         # + global constant
    Finv = np.linalg.inv(Jg.T @ SigI @ Jg)
    return float(np.sqrt(np.trace(Finv[:3, :3])))


def mc_chain(sigma_phi, sigma_n=SIG_EST, n_real=300):
    """Frozen-gauge MC: eps_i is UNKNOWN per-node error of the fence fix;
    eps_puck is the TX-side residual, absorbed by the free global constant
    (U(1) projection). The estimator knows only the nominal gauge."""
    errs = []
    for _ in range(n_real):
        eps = sigma_phi * rng.standard_normal(NI)
        eps_puck = sigma_phi * rng.standard_normal()   # TX-side residual
        nu = sigma_n * rng.standard_normal(NO)
        meas = (2 * np.pi * sum_dist(p_true) / LAM) + eps[i_idx] + eps_puck + nu
        meas = (meas + np.pi) % (2 * np.pi) - np.pi
        p = p_true + 0.005 * rng.standard_normal(3)      # 5 mm warm start
        for _ in range(5):                              # Gauss-Newton
            r = meas - 2 * np.pi * sum_dist(p) / LAM
            r = (r + np.pi) % (2 * np.pi) - np.pi
            Jg = np.hstack([jac_p(p), np.ones((NO, 1))])
            dp, *_ = np.linalg.lstsq(Jg, r, rcond=None)
            p = p + dp[:3]
        errs.append(np.linalg.norm(p - p_true))
    return float(np.sqrt(np.mean(np.square(errs))))


sigmas = [0.05, 0.1, 0.2, 0.3, 0.5, 1.0, 2.0, 3.0]
print("  sigma_phi[rad] | CRLB [cm] | MC [cm]")
for s in sigmas:
    cb = gls_crlb(s)
    mc = mc_chain(s)
    print(f"  {s:12.2f} | {cb*100:9.2f} | {mc*100:7.2f}")
    log("E_e2e", f"sigma_phi={s:g} rad", mc, "m", f"CRLB {cb:.4f} m")
log("E_e2e", "dCRLB/dsigma_phi (0.3 rad)", gls_crlb(0.31) - gls_crlb(0.29), "m/rad", "")
print(f"  10 cm bar: sigma_phi* ~ {0.10 / gls_crlb(1.0):.2f} rad (CRLB scaling;"
      " wrap-limited beyond ~3 rad)")
log("E_e2e", "sigma_phi* at 10 cm", 0.10 / gls_crlb(1.0), "rad", "LoS, no multipath")

# =============================================================================
# Part F — missing direction f: phase-swept beacon / RIPS-style hub protocol
# =============================================================================
print("\n== F: RIPS-style hub protocol (two hub TXs, q-invariant) ==")
db = 30e3                                     # inter-TX carrier offset
t = np.arange(0, 0.002, 1e-7)
lo_errs = rng.uniform(-np.pi, np.pi, 200)
spread = []
for le in lo_errs:
    bb = (np.exp(1j * (2 * np.pi * db / 2 * t + 0.3 - le)) +
          np.exp(1j * (-2 * np.pi * db / 2 * t + 1.1 - le)))
    ph = np.angle(np.sum(bb * np.exp(-1j * 2 * np.pi * db / 2 * t)))
    resid = np.angle(np.exp(1j * (ph - (0.3 - le))))       # compare mod 2 pi
    spread.append(resid)
print(f"  F1 LO-invariance: beat-phase error spread {np.std(spread):.2e} rad"
      f" over 200 random LO phases (expect ~0)")
log("F_rips", "beat-phase LO invariance", float(np.std(spread)), "rad",
    "common-mode algebra")


def beat_phase_sigma(rho_db, n_samp):
    return float(np.sqrt(2 / (10 ** (rho_db / 10) * n_samp)))


print("  F2 beat-phase sigma -> range-difference error (250 kSa/s decimated I/Q):")
for rho_db in [0, 10, 20, 30]:
    row = []
    for T_int in [1e-3, 1e-2, 1e-1]:
        n_s = T_int * 250e3
        s = beat_phase_sigma(rho_db, n_s)
        rd = LAM / (2 * np.pi) * s
        row.append(f"T={T_int*1e3:4.0f} ms: {rd*1e3:6.3f} mm")
        log("F_rips", f"rho={rho_db}dB,T={T_int*1e3:g}ms", s, "rad",
            f"range-diff err {rd*1e3:.3f} mm")
    print(f"    rho={rho_db:2d} dB  " + "  ".join(row))
print("  F3 cross-receiver alignment cost (12.5 ns grid) at the beat frequency:")
for fb in [3e3, 30e3, 60e3]:
    print(f"    beat {fb/1e3:4.0f} kHz: {2*np.pi*fb*12.5e-9*1e3:7.3f} mrad per grid step")
    log("F_rips", f"alignment,beat={fb/1e3:g}kHz", 2 * np.pi * fb * 12.5e-9,
        "rad", "per 12.5 ns")

# =============================================================================
# Part G — airtime of the fence + round at home scale
# =============================================================================
print("\n== G: airtime ledger ==")
T_FRAME = 130e-6            # ~100 B ESP-NOW HT20 MCS0 incl. preamble/SIFS
N_NODES = 8
round_t = (1 + N_NODES) * T_FRAME
for rate in [10, 20, 50]:
    duty = round_t * rate
    print(f"  round of {N_NODES} nodes @ {rate} Hz: {round_t*1e3:.2f} ms/round,"
          f" duty {duty*100:4.1f}%")
    log("G_airtime", f"round@{rate}Hz", duty, "frac", f"{round_t*1e3:.2f} ms/round")
for ref_rate in [200, 500, 1000]:
    duty = T_FRAME * ref_rate
    print(f"  reference strobe @ {ref_rate} Hz (hub only): duty {duty*100:4.1f}%")
    log("G_airtime", f"strobe@{ref_rate}Hz", duty, "frac", "hub broadcast")
for sense_rate in [20, 50, 100]:
    duty = 2 * T_FRAME * sense_rate          # bracket pair per sensing frame
    print(f"  bracket pair per sensing frame @ {sense_rate} Hz: duty {duty*100:4.1f}%")
    log("G_airtime", f"bracket@sense={sense_rate}Hz", duty, "frac",
        "ref frame before+after each sensing frame")
print(f"  target coherence time lam/(2v) @ 1 m/s: {LAM/2*1e3:.1f} ms")
log("G_ledger", "target coherence time @1 m/s", LAM / 2, "s", "")

# =============================================================================
# write ledger CSV + figure
# =============================================================================
with open("gate_a_summary.csv", "w", newline="") as fh:
    w = csv.writer(fh)
    w.writerow(["part", "param", "value", "units", "note"])
    w.writerows(ROWS)
print(f"\nwrote gate_a_summary.csv ({len(ROWS)} rows)")

try:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    fig, ax = plt.subplots(1, 3, figsize=(11, 3.3))
    for sf in SIG_F:
        ax[0].loglog(GAPS_MS, [hold[sf][g] for g in GAPS_MS], marker="o", ms=3,
                     label=f"{sf:g} Hz")
    ax[0].axhline(0.5, color="k", ls="--", lw=0.8, label="0.5 rad gate")
    ax[0].axhline(0.3, color="r", ls=":", lw=0.8, label="0.3 rad target")
    ax[0].set_xlabel("reference-to-sensing gap [ms]")
    ax[0].set_ylabel(r"$\sigma_\phi$ [rad]")
    ax[0].set_title("A. holdover after CFO removal")
    ax[0].legend(fontsize=7)
    ax[0].grid(True, which="both", alpha=0.3)

    mc_list = [mc_chain(s) * 100 for s in sigmas]
    ax[1].loglog(sigmas, mc_list, marker="s", ms=3, label="frozen gauges (MC)")
    ax[1].loglog(sigmas, [gls_crlb(s) * 100 for s in sigmas], ls=":", lw=1.2,
                 label="GLS CRLB")
    ax[1].axhline(10, color="r", ls="--", lw=0.8, label="10 cm bar")
    ax[1].set_xlabel(r"fence residual $\sigma_\phi$ [rad]")
    ax[1].set_ylabel("3D RMSE [cm]")
    ax[1].set_title("E. coherence $\\to$ localisation")
    ax[1].legend(fontsize=7)
    ax[1].grid(True, which="both", alpha=0.3)

    ax[2].loglog(Nfaces, sn, marker="o", ms=3, label="snapshot avg")
    ax[2].loglog(Nfaces, [sn[0] / np.sqrt(n) for n in Nfaces], ls="--", lw=1,
                 label=r"$\sqrt{N}$ ideal")
    ax[2].axhline(sigma_wander, color="k", ls=":", lw=1,
                  label=f"wander floor {sigma_wander:.3f} rad")
    ax[2].set_xlabel("N faces averaged")
    ax[2].set_ylabel(r"$\sigma_\phi$ [rad]")
    ax[2].set_title("C. face averaging law")
    ax[2].legend(fontsize=7)
    ax[2].grid(True, which="both", alpha=0.3)
    fig.tight_layout()
    fig.savefig("gate_a_fig1.png", dpi=110)
    print("wrote gate_a_fig1.png")
except Exception as exc:  # noqa: BLE001
    print("figure skipped:", exc)
