#!/usr/bin/env python
"""Round-2 T1 (character `i`): the coherence gate from the bottom up.

Complements ../coherence-gate-f/m2_residual.py (top-down: assumes a residual-CFO
OU process).  This script derives the residual from physical clock noise
(power-law oscillator models anchored on measured ESP-PPB values), then runs
exchange -> tracker -> localisation.

  V  noise-synthesis validation (measured Allan deviation vs closed forms)
  A  gate map: sigma_phi(T_gap) per oscillator tier; killing values
  B  exchange simulation: strobe bursts -> Kalman [phi, f] -> sensing-cadence
     residual vs cycle time; hazard budget lines (jitter, artefact, walker)
  C  averaging law on two axes: faces (common-mode LO) x packets (wander-
     correlated): where does sqrt(N) fail?
  D  end-to-end chain: ambiguity-resolution success rate + single-epoch
     coherent localisation (position RMSE vs sigma_phi, with multipath arm)
  E  phase-swept beacon (missing direction f): estimator variance + airtime

Simulation only (NumPy model of oscillator physics + estimators; NOT a
measurement of any device).  Run with:
    /home/matt/projects/aquila-v2-spec/sim-python m2_from_crystals.py
Outputs (small): m2i_summary.csv, m2i_figures.png
"""
import csv
import numpy as np

rng = np.random.default_rng(20261016)
C = 2.99792458e8
FC = 2.437e9                    # channel 6 centre
LAM = C / FC
K2P = 2.0 * np.pi * FC          # rad of carrier phase per second of time error

# ---------------------------------------------------------------------------
# Oscillator tiers (one-sided power-law PSDs S_y(f) = h0 + h_-1/f + h_-2 f^-2):
#   white FM      h0   : sigma_y(tau) = sqrt(h0/(2 tau))
#   flicker FM    h_-1 : sigma_y(tau) = sqrt(h_-1 * 2 ln 2)   (floor)
#   rand-walk FM  h_-2 : sigma_y(tau) = sqrt(2 pi^2 h_-2 tau / 3)
# Anchors: ESP-PPB story.md (pessimistic cheap-XO sigma_y(1 s)=1e-8 white-FM;
# 0.5 ppm TCXO pair ~0.1 ppm; VCTCXO disciplined 1-0.1 ppb; 100+ ppb on
# airflow; 1 ppb/s same-PCB thermal gradient); corpus notes 12/17.
# ---------------------------------------------------------------------------
TIERS = {
    "S0-stock-pess":   (2.0e-16, 0.0,    0.0),      # sigma_y(1s)=1e-8  (ESP-PPB pessimistic)
    "S1-stock-typ":    (2.0e-18, 0.0,    0.0),      # sigma_y(1s)=1e-9  (XO hope; UNMEASURED)
    "S2-good-XO":      (1.8e-19, 6.5e-20, 0.0),     # 3e-10 WFM + 3e-10 flicker floor
    "T0-TCXO":         (2.0e-20, 7.2e-21, 0.0),     # 1e-10 flicker floor
    "V0-VCTCXO-1ppb":  (2.0e-22, 7.2e-23, 0.0),     # disciplined, 1 ppb
}
RW_CASES = {"quiet": 1.0e-11, "typical": 1.0e-10, "warmup": 1.0e-9, "airflow": 1.0e-8}


def h_m2_of(rw):
    """sigma_y,RW(tau)=rw*sqrt(tau)  ->  h_-2 = 3 rw^2/(2 pi^2)."""
    return 3.0 * rw * rw / (2.0 * np.pi * np.pi)


def synth_relative(nb, n, dt, h0, hm1, hm2, r):
    """Batch-synthesise relative fractional-frequency y and phase phi.

    Two-sided PSDs = h/2.  phi[k] = K2P * trapezoid integral of y.
    Shapes: y (nb, n+1), phi (nb, n+1).
    """
    n0, n1, nff = h0 / 2.0, hm2 / 2.0, hm1 / 2.0
    y = r.standard_normal((nb, n + 1)) * np.sqrt(n0 / dt) if h0 > 0 else np.zeros((nb, n + 1))
    if hm2 > 0:
        y = y + np.cumsum(r.standard_normal((nb, n + 1)) * np.sqrt(n1 * dt), axis=1)
    if hm1 > 0:
        # band-limited flicker via FFT shaping (independent per batch member)
        m = n // 2 + 1
        f = np.fft.rfftfreq(n, dt)[1:]
        spec = r.standard_normal((nb, m - 1)) * np.sqrt(nff / f) * np.sqrt(n / dt)
        full = np.concatenate([np.zeros((nb, 1)), spec], axis=1)
        y_ff = np.fft.irfft(full, n, axis=1)[:, : n + 1]
        if y_ff.shape[1] < n + 1:                    # pad to the n+1 sample grid
            y_ff = np.concatenate([y_ff, y_ff[:, -1:]], axis=1)
        # renormalise so the flat ADEV part matches sqrt(h_-1 * 2 ln 2)
        target = np.sqrt(hm1 * 2.0 * np.log(2.0))
        sc = np.std(y_ff[:, :: max(1, n // 50)], axis=1)
        sc[sc < 1e-30] = 1.0
        y = y + y_ff * (target / sc)[:, None]
    phi = np.zeros((nb, n + 1))
    np.cumsum(K2P * 0.5 * (y[:, :-1] + y[:, 1:]) * dt, axis=1, out=phi[:, 1:])
    return y, phi


def adev_batch(phi, dt, taus):
    """Mean overlapping Allan deviation across a batch of phase series."""
    x = phi / K2P
    out = {}
    for tau in taus:
        m = int(round(tau / dt))
        if m < 1 or 2 * m >= x.shape[1]:
            continue
        d2 = x[:, 2 * m:] - 2 * x[:, m:-m] + x[:, :-2 * m]
        out[tau] = float(np.sqrt(np.mean(d2 * d2) / (2.0 * tau * tau)))
    return out


def sigma_y_theory(h0, hm1, hm2, tau):
    return np.sqrt(h0 / (2.0 * tau) + hm1 * 2.0 * np.log(2.0)
                   + (2.0 * np.pi**2 / 3.0) * hm2 * tau)


# ===========================================================================
# Part V: validate the noise synthesis
# ===========================================================================
print("== Part V: noise-synthesis validation (MC Allan deviation vs theory) ==")
DT = 25.0e-6
N_RUN = 400_000                        # 10 s per realisation
TAUS = [1e-4, 1e-3, 1e-2, 1e-1, 1.0]
val_rows = []
for tier, (h0, hm1, hm2b) in TIERS.items():
    hm2 = h_m2_of(RW_CASES["typical"])
    _, phi = synth_relative(4, N_RUN, DT, h0, hm1, hm2, rng)
    meas = adev_batch(phi, DT, TAUS)
    for tau in TAUS:
        pred = float(sigma_y_theory(h0, hm1, hm2, tau))
        val_rows.append((tier, tau, meas.get(tau, np.nan), pred))
        if tau in (1e-3, 1e-1):
            print(f"  {tier:16s} tau={tau:7.1e}  MC={meas.get(tau, np.nan):.3e}  theory={pred:.3e}")

# ===========================================================================
# Part A: gate map  --  sigma_phi(T_gap) per tier
# ===========================================================================
print("\n== Part A: gate map sigma_phi(T_gap) [rad] (MC, n=300) ==")
GAPS = np.array([0.1, 0.3, 1.0, 3.0, 10.0, 20.0, 50.0, 100.0])   # ms


def gate_mc(h0, hm1, hm2, gaps_ms, n_real=300, dt=DT, nper=120):
    tmax = max(gaps_ms) * 1e-3
    n = int(tmax / dt)
    idx = np.round(gaps_ms * 1e-3 / dt).astype(int)
    _, phi = synth_relative(n_real, n, dt, h0, hm1, hm2, rng)
    d = phi[:, idx] - phi[:, [0]]
    return d.std(axis=0, ddof=1)


print(f"{'tier':16s} " + " ".join(f"{g:7.1f}ms" for g in GAPS))
gate_rows = []
for tier, (h0, hm1, hm2b) in TIERS.items():
    hm2 = h_m2_of(RW_CASES["typical"])
    v = gate_mc(h0, hm1, hm2, GAPS)
    print(f"  {tier:14s} " + " ".join(f"{x:8.3f}" for x in v))
    gate_rows.append((tier, "typical", v))
    hm2a = h_m2_of(RW_CASES["airflow"])
    va = gate_mc(h0, hm1, hm2a, GAPS, n_real=300)
    gate_rows.append((tier, "airflow", va))
# untracked deterministic offsets for context (removable by CFO tracking)
print("\n  untracked deterministic offset (for context; tracking removes it):")
for df0 in [12.0e3, 48.7e3]:
    v = 2 * np.pi * df0 * GAPS * 1e-3
    print(f"   dF0={df0/1e3:4.1f} kHz  " + " ".join(f"{x:8.2f}" for x in v))

print(f"\n  gate identity: sigma_phi = 2 pi fc sigma_x(T)  ->  0.5 rad <=> "
      f"time error {0.5 / K2P * 1e12:.1f} ps")
for tier, (h0, hm1, _) in TIERS.items():
    if h0 > 0:
        tstar = (0.5 / (K2P * np.sqrt(h0 / 2.0))) ** 2
        print(f"  {tier:16s} WFM-only 0.5-rad crossing: T* = {tstar * 1e3:8.3f} ms")

# ===========================================================================
# Part B: exchange simulation (strobe bursts -> Kalman -> sensing residual)
# ===========================================================================
print("\n== Part B: strobe-exchange simulation (8-frame bursts @100 us; "
      "per-frame meas 0.52 rad) ==")
SIG_MEAS = 0.52            # rad; ESP-PPB board-6 single-frame CSI phase noise (~30 deg)
N_BURST, T_SEP = 8, 100.0e-6
CYCLES_MS = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0]
T_SIM = 12.0
NB = 16                    # batch of realisations


def run_exchange(h0, hm1, hm2, cycle, walker=False, n_real=NB):
    """Vectorised [phi, f] Kalman on burst-mean phase; prediction error at the
    sensing instant (mid-cycle).  Observation times = burst frame instants."""
    dt = DT
    n_tot = int(T_SIM / dt)
    _, phi = synth_relative(n_real, n_tot, dt, h0, hm1, hm2, rng)
    # burst schedule (frames within a burst, bursts every `cycle`)
    frame_off = np.arange(N_BURST) * T_SEP
    n_bur = int(T_SIM / cycle)
    t_frame = (np.arange(n_bur)[:, None] * cycle + frame_off[None, :]).ravel()  # (n_bur*8)
    k_frame = np.round(t_frame / dt).astype(int)
    k_frame = k_frame[k_frame < n_tot]
    n_b = len(k_frame) // N_BURST
    k_frame = k_frame[: n_b * N_BURST].reshape(n_b, N_BURST)
    t_centre = (np.arange(n_b) * cycle + 3.5 * T_SEP)
    k_centre = np.round(t_centre / dt).astype(int)
    k_centre = np.minimum(k_centre, n_tot - 1)
    # observations: burst frames with per-frame measurement noise
    Z = phi[:, k_frame] + SIG_MEAS * rng.standard_normal((n_real, n_b, N_BURST))
    zm = Z.mean(axis=2)                                    # (n_real, n_b)
    # empirical burst-mean observation variance (measurement + intra-burst wander)
    err_obs = zm - phi[:, k_centre]
    R = float(np.mean(err_obs ** 2))
    # Kalman [phi(rad), f(Hz)] at burst cadence
    x = np.zeros((n_real, 2))
    P = np.repeat(np.diag([1.0, 10.0 ** 2])[None], n_real, axis=0)
    F = np.array([[1.0, 2 * np.pi * cycle], [0.0, 1.0]])
    qf = FC ** 2 * (hm2 / 2.0)                             # Hz^2/s (RWFM)
    Q = np.array([
        [K2P**2 * (h0 / 2.0) * cycle + (2 * np.pi) ** 2 * qf * cycle**3 / 3.0
         + (K2P * np.sqrt(hm1 * 2 * np.log(2)) * cycle * 0.7) ** 2,
         (2 * np.pi) * qf * cycle**2 / 2.0],
        [(2 * np.pi) * qf * cycle**2 / 2.0, qf * cycle],
    ])
    errs = []
    burn = 8
    t_last = np.zeros(n_real)
    for b in range(n_b):
        # predict (per-burst step; first step uses actual dt)
        dtk = np.where(b == 0, t_centre[0], cycle)
        Fb = np.array([[1.0, 2 * np.pi * float(dtk) if b == 0 else 2 * np.pi * cycle],
                       [0.0, 1.0]])
        x = x @ Fb.T
        P = Fb @ P @ Fb.T + Q
        # update
        innov = zm[:, b] - x[:, 0]
        innov = (innov + np.pi) % (2 * np.pi) - np.pi
        S = P[:, 0, 0] + R
        Kk = np.stack([P[:, 0, 0] / S, P[:, 1, 0] / S], axis=1)
        x = x + Kk * innov[:, None]
        P = P - np.einsum("bi,bj->bij", Kk, P[:, :, 0])
        # sensing instant: half a cycle after the burst centre
        if b >= burn:
            dt_s = cycle / 2.0
            pred = x[:, 0] + 2 * np.pi * x[:, 1] * dt_s
            kc = np.minimum(np.round((t_centre[b] + dt_s) / dt).astype(int), n_tot - 1)
            e = pred - phi[:, kc]
            if walker:
                e = e + 4 * np.pi * 1.0 * dt_s / LAM * rng.standard_normal(n_real)
            errs.append((e + np.pi) % (2 * np.pi) - np.pi)
    errs = np.concatenate(errs)
    return float(np.std(errs, ddof=1)), R


print(f"{'tier':16s} " + " ".join(f"{c:7.1f}ms" for c in CYCLES_MS) + "   (burst-mean R in rad^2)")
partB_rows = []
for tier, (h0, hm1, _) in TIERS.items():
    hm2 = h_m2_of(RW_CASES["typical"])
    row, rrow = {}, {}
    for c_ms in CYCLES_MS:
        row[c_ms], rrow[c_ms] = run_exchange(h0, hm1, hm2, c_ms * 1e-3)
    print(f"  {tier:14s} " + " ".join(f"{row[c]:8.3f}" for c in CYCLES_MS)
          + "   " + " ".join(f"{rrow[c]:5.3f}" for c in CYCLES_MS))
    partB_rows.append((tier, row))

# hazard arms on the decisive tier S1
print("\nhazard arms (tier S1-stock-typ):")
h0, hm1, _ = TIERS["S1-stock-typ"]
hm2 = h_m2_of(RW_CASES["typical"])
base, _ = run_exchange(h0, hm1, hm2, 1e-3)
print(f"  baseline, 1 ms cycles                     : {base:6.3f} rad")
air, _ = run_exchange(h0, hm1, h_m2_of(RW_CASES["airflow"]), 1e-3)
print(f"  airflow event (RW 1e-8/sqrt s), 1 ms      : {air:6.3f} rad")
air20, _ = run_exchange(h0, hm1, h_m2_of(RW_CASES["airflow"]), 20e-3)
print(f"  airflow event, 20 ms cycles               : {air20:6.3f} rad")
typ20, _ = run_exchange(h0, hm1, hm2, 20e-3)
print(f"  (same, typical thermal, 20 ms)            : {typ20:6.3f} rad")
wk, _ = run_exchange(h0, hm1, hm2, 2e-3, walker=True)
print(f"  walker 1 m/s non-reciprocity, 2 ms cycles : {wk:6.3f} rad")
# analytic budget lines (two-way reply arm)
print("\nanalytic two-way reply budget lines:")
for df0, tau_asym in [(12.0e3, 30e-9), (48.7e3, 30e-9), (12.0e3, 1e-6), (48.7e3, 1e-6)]:
    print(f"  dF0={df0/1e3:4.1f} kHz, reply-timing sigma={tau_asym*1e9:6.0f} ns -> "
          f"{2*np.pi*df0*tau_asym:6.3f} rad")
print("  CFO-quantisation artefact 2 pi (g/sqrt12) dT_eff:")
for g in [100.0, 400.0, 1600.0]:
    print(f"   g={g:6.0f} Hz, dT_eff=20 us -> {2*np.pi*(g/np.sqrt(12))*20e-6:6.3f} rad")

# ===========================================================================
# Part C: averaging law on two axes (faces x packets)
# ===========================================================================
print("\n== Part C: averaging law, sigma_est/sigma_single (tier S1) ==")
h0, hm1, _ = TIERS["S1-stock-typ"]
hm2 = h_m2_of(RW_CASES["typical"])
print("  faces share the node LO exactly (wander common-mode); packets share")
print("  wander over the burst window. Rows N_p packets, cols N_f faces.")
sig_faces = SIG_MEAS
nf_cols = [1, 4, 20]
np_rows = [1, 2, 4, 8, 16, 32]
print("        " + " ".join(f"Nf={n:6d}" for n in nf_cols))
C_tab = []
for n_p in np_rows:
    tw = (n_p - 1) * T_SEP
    n = int(1.0 / DT)
    _, phi = synth_relative(200, n, DT, h0, hm1, hm2, rng)
    k0 = rng.integers(100, n - 1000, size=(200,))
    ks = np.clip(k0[:, None] + np.arange(n_p)[None, :] * int(T_SEP / DT), 0, n)
    ph = phi[np.arange(200)[:, None], ks]                    # (200, n_p)
    # truth at burst centre
    kc = np.clip(k0 + int(tw / 2 / DT), 0, n)
    truth = phi[np.arange(200), kc]
    vals = []
    for n_f in nf_cols:
        # measurement noise: independent per face; wander common to faces
        z_face = ph + sig_faces / np.sqrt(n_f) * rng.standard_normal(ph.shape)
        zm = z_face.mean(axis=1)
        vals.append(float(np.std(zm - truth, ddof=1)))
    C_tab.append((n_p, vals))
    print(f"  Np={n_p:3d} " + " ".join(f"{v/sig_faces:7.3f}" for v in vals) +
          "   (abs: " + " ".join(f"{v:5.3f}" for v in vals) + " rad)")

# ===========================================================================
# Part D: end-to-end chain (ambiguity + single-epoch coherent localisation)
# ===========================================================================
print("\n== Part D: end-to-end chain: sum-CP cascade -> coherent 3D position ==")
NODES = np.array([
    (0.5, 0.5, 1.0), (5.5, 0.5, 2.0), (5.5, 3.5, 1.0),
    (0.5, 3.5, 2.0), (3.0, 0.5, 2.6), (3.0, 3.5, 0.4),
])
LINKS = [(0, 3), (1, 4), (2, 5), (0, 5), (1, 3), (2, 4)]
AMB = LAM / 2.0                       # 6.12 cm sum-CP ambiguity
XT0 = np.array([3.0, 2.0, 1.6])


def rho(x, a, b):
    return np.linalg.norm(x - NODES[a]) + np.linalg.norm(x - NODES[b])


# --- D1: integer-ambiguity resolution vs prior width (cells) and sigma_phi
# The sum-CP observable per link is rho_k mod AMB.  Given a prior position of
# quality ~W cells, the integers are searched in a +/-W window; success =
# the least-squares integer vector is the true one AND separated from the
# runner-up.  A 1.5 m FTM-only prior spans ~25 cells and is NOT searched
# exhaustively here (the motion/stitched-delay cascade is the documented
# route); W = 1..3 represents a motion- or angle-gated prior.
print("\nD1: integer-ambiguity lock rate (6 links; window = prior quality):")


def gn_positions(M, x_init):
    """Vectorised Gauss-Newton: positions for metric-range rows M (m, 6)."""
    x = np.repeat(x_init[None], M.shape[0], axis=0).copy()
    for _ in range(8):
        H = np.zeros((M.shape[0], 6, 3))
        pred = np.zeros_like(M)
        for li, (a, b) in enumerate(LINKS):
            ua = x - NODES[a]
            da = np.linalg.norm(ua, axis=1, keepdims=True)
            ua = ua / da
            ub = x - NODES[b]
            db = np.linalg.norm(ub, axis=1, keepdims=True)
            ub = ub / db
            H[:, li] = ua + ub
            pred[:, li] = da[:, 0] + db[:, 0]
        r = M - pred
        htr = np.einsum("mlj,ml->mj", H, r)          # H^T r per row
        dx = np.einsum("mli,mlj->mij", H, H)         # H^T H per row
        step = np.einsum("mij,mj->mi", np.linalg.inv(dx), htr)
        x = x + step
    res = np.sum((M - np.stack([np.linalg.norm(x - NODES[a], axis=1)
                                + np.linalg.norm(x - NODES[b], axis=1)
                                for a, b in LINKS], axis=1)) ** 2, axis=1)
    return x, res


amb_rows = []
rho_true = np.array([rho(XT0, a, b) for a, b in LINKS])
for sig_phi in [0.1, 0.3, 0.5]:
    sig_d = LAM * sig_phi / (4 * np.pi)
    for n_ep in [1, 5, 20]:
        # multi-epoch averaging: integers are static; per-epoch noise averages
        sig_ep = sig_d / np.sqrt(n_ep)
        for W in ([1, 2] if n_ep == 1 else [1]):
            grid = np.arange(-W, W + 1)
            cand = np.stack(np.meshgrid(*[grid] * 6, indexing="ij"),
                            axis=-1).reshape(-1, 6)
            ok = 0
            n_try = 20
            for _ in range(n_try):
                eps = sig_ep * rng.standard_normal(6)
                f_obs = rho_true + eps - np.floor(rho_true / AMB) * AMB
                n_true = np.floor(rho_true / AMB).astype(int)
                M = f_obs[None, :] + (n_true[None, :] + cand) * AMB
                x, res = gn_positions(M, XT0 + 0.05 * rng.standard_normal(3))
                i0 = int(np.argmin(res))
                best_is_truth = np.all(cand[i0] == 0)
                # operational uniqueness: runner-up position must be >10 cm away
                order = np.argsort(res)
                ru = 1
                while ru < len(order) and np.linalg.norm(x[order[ru]] - x[i0]) < 0.10:
                    ru += 1
                uniq = ru < len(order)
                if best_is_truth and uniq:
                    ok += 1
            amb_rows.append((sig_phi, n_ep, W, ok / n_try))
            print(f"  sigma_phi={sig_phi:4.2f} epochs={n_ep:2d} window ±{W} cell: "
                  f"lock {ok/n_try*100:5.0f}%")
print("  (an FTM-only 1.5 m prior spans ±25 cells: needs the motion or")
print("   stitched-delay cascade -- flagged as residual unknown, not simulated)")

# --- D2: single-epoch coherent localisation (ambiguity assumed locked)
print("\nD2: single-epoch position RMSE vs sigma_phi (ambiguity locked, 300 MC):")


def loc_epoch(sigma_phi, bias=0.0, x_true=XT0, n_real=300, iters=20):
    sig_d = LAM * sigma_phi / (4 * np.pi)     # per-link delta/metric-range noise
    errs = []
    for _ in range(n_real):
        b = bias * rng.standard_normal(6) if bias > 0 else np.zeros(6)
        obs = np.array([rho(x_true, a, b) for a, b in LINKS]) + b \
            + sig_d * rng.standard_normal(6)
        x = x_true + 1.5 * rng.standard_normal(3)
        for _ in range(iters):                # Gauss-Newton (damped restart)
            H = np.zeros((6, 3))
            for li, (a, b_) in enumerate(LINKS):
                ua = x - NODES[a]; da = np.linalg.norm(ua); ua = ua / da
                ub = x - NODES[b_]; db = np.linalg.norm(ub); ub = ub / db
                H[li] = ua + ub
            r = obs - np.array([rho(x, a, b_) for a, b_ in LINKS])
            dx = np.linalg.lstsq(H, r, rcond=None)[0]
            x = x + dx
        errs.append(np.linalg.norm(x - x_true))
    return float(np.sqrt(np.mean(np.square(errs))))


loc_rows = []
for s in [0.05, 0.1, 0.2, 0.3, 0.5, 1.0]:
    r0 = loc_epoch(s)
    rb = loc_epoch(s, bias=0.03)             # 3 cm per-link multipath bias arm
    loc_rows.append((s, r0, rb))
    print(f"  sigma_phi={s:4.2f} rad -> RMSE {r0*100:5.1f} cm  "
          f"(with 3 cm/link bias arm: {rb*100:5.1f} cm)")

# CRLB / GDOP at room centre
Hf = np.zeros((6, 3))
for li, (a, b) in enumerate(LINKS):
    ua = XT0 - NODES[a]; ua /= np.linalg.norm(ua)
    ub = XT0 - NODES[b]; ub /= np.linalg.norm(ub)
    Hf[li] = ua + ub
cov = np.linalg.inv(Hf.T @ Hf)
gdop = float(np.sqrt(np.trace(cov)))
print(f"  GDOP at room centre = {gdop:.2f}; "
      f"CRLB: sigma_pos = {gdop * LAM/(4*np.pi) * 100:.2f} cm x sigma_phi[rad]")

# coherent-imaging gain loss and pedestal
print("\n  coherent sum gain loss / sidelobe pedestal (K equal links):")
for K in [6, 20, 190]:
    for s in [0.1, 0.3, 0.5, 1.0]:
        g = np.exp(-s * s)
        if s in (0.3, 0.5):
            print(f"   K={K:3d} sigma={s:.1f}: gain {10*np.log10(g):+5.2f} dB, "
                  f"pedestal {10*np.log10((1-g)/K):6.1f} dB")

# MCRB-limited measurement-noise variant (would the 30-deg anchor matter?)
def run_exchange_meas(h0, hm1, hm2, cycle, sig_meas):
    global SIG_MEAS
    keep = SIG_MEAS
    SIG_MEAS = sig_meas
    out = run_exchange(h0, hm1, hm2, cycle)
    SIG_MEAS = keep
    return out


print("\nmeasurement-noise anchor comparison (tier S1, 2 ms cycles):")
s_a, _ = run_exchange_meas(h0, hm1, hm2, 2e-3, 0.52)
s_b, _ = run_exchange_meas(h0, hm1, hm2, 2e-3, 0.06)
print(f"  per-frame 0.52 rad (ESP-PPB board-6 anchor): {s_a:6.3f} rad")
print(f"  per-frame 0.06 rad (MCRB, 56 subc, ~6 dB)  : {s_b:6.3f} rad")

# ===========================================================================
# Part E: phase-swept beacon (missing direction f)
# ===========================================================================
print("\n== Part E: phase-swept beacon (direction f) ==")
print("  common-view pairwise difference of ONE frame cancels the hub phase")
print("  exactly (same physical TX event); per-pair relative-phase noise:")
for nb in [1, 4, 8, 16]:
    s_pair = np.sqrt(2.0) * SIG_MEAS / np.sqrt(nb)
    print(f"   burst N={nb:2d}: sigma_phi(pair) = {s_pair:6.3f} rad")
print("  the sweep adds: known per-frame phase code -> frame association,")
print("  per-frame self-consistency test, unambiguous differencing.")
print("  RIPS-style dual-TX beat: relative LO phase in the baseband beat-phase")
print("  difference; needs concurrent adjacent-channel TX + wideband RX (raw")
print("  I/Q via ESP-SDR) -> SPECULATIVE on S3; CSI path cannot see the beat.")
print("\n  airtime per reference cycle (300 us frames):")
for N_fleet in [4, 8, 16, 32]:
    print(f"   fleet N={N_fleet:2d}: two-way O(N) = {N_fleet*0.3:5.2f} ms; "
          f"beacon O(1) = 0.30 ms")

# ===========================================================================
# outputs
# ===========================================================================
with open("m2i_summary.csv", "w", newline="") as fh:
    w = csv.writer(fh)
    w.writerow(["part", "series", "x", "value", "units"])
    for tier, tau, mc, an in val_rows:
        w.writerow(["V_adev", tier, tau, f"{mc:.6e}", "sigma_y"])
    for tier, scene, v in gate_rows:
        for g, val in zip(GAPS, v):
            w.writerow(["A_gate", f"{tier}-{scene}", g, f"{val:.5f}", "rad"])
    for tier, row in partB_rows:
        for c, val in row.items():
            w.writerow(["B_exchange", tier, c, f"{val:.5f}", "rad"])
    for n_p, vals in C_tab:
        for n_f, v in zip(nf_cols, vals):
            w.writerow(["C_averaging", f"Np={n_p}", n_f, f"{v:.5f}", "rad"])
    for s, n_ep, W, rate in amb_rows:
        w.writerow(["D1_ambiguity", f"epochs={n_ep},W={W}", s, f"{rate:.3f}", "frac"])
    for s, r0, rb in loc_rows:
        w.writerow(["D2_localisation", "rmse", s, f"{r0:.5f}", "m"])
        w.writerow(["D2_localisation", "rmse_bias3cm", s, f"{rb:.5f}", "m"])

try:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    fig, ax = plt.subplots(1, 3, figsize=(12, 3.5))
    for tier, scene, v in gate_rows:
        if scene == "typical":
            ax[0].loglog(GAPS, v, marker="o", ms=3, label=tier)
    ax[0].axhline(0.5, color="k", ls="--", lw=0.8, label="0.5 rad gate")
    ax[0].set_xlabel("gap $T$ [ms]"); ax[0].set_ylabel(r"$\sigma_\phi$ [rad]")
    ax[0].set_title("A. gate map (MC)"); ax[0].legend(fontsize=6)
    ax[0].grid(True, which="both", alpha=0.3)
    for tier, row in partB_rows:
        ax[1].loglog(CYCLES_MS, [row[c] for c in CYCLES_MS], marker="s", ms=3, label=tier)
    ax[1].axhline(0.5, color="k", ls="--", lw=0.8)
    ax[1].set_xlabel("strobe cycle [ms]")
    ax[1].set_title(r"B. sensing-cadence residual (50% duty)")
    ax[1].legend(fontsize=6); ax[1].grid(True, which="both", alpha=0.3)
    ax[2].loglog([s for s, _, _ in loc_rows], [r * 100 for _, r, _ in loc_rows],
                 marker="^", ms=4, label="white phase noise")
    ax[2].loglog([s for s, _, _ in loc_rows], [rb * 100 for _, _, rb in loc_rows],
                 marker="v", ms=4, label="+3 cm/link multipath bias")
    ax[2].set_xlabel(r"$\sigma_\phi$ [rad]"); ax[2].set_ylabel("position RMSE [cm]")
    ax[2].set_title("D. end-to-end (ambiguity locked)")
    ax[2].legend(fontsize=7); ax[2].grid(True, which="both", alpha=0.3)
    fig.tight_layout()
    fig.savefig("m2i_figures.png", dpi=110)
    print("\nwrote m2i_summary.csv and m2i_figures.png")
except Exception as exc:  # noqa: BLE001
    print("figure skipped:", exc)
