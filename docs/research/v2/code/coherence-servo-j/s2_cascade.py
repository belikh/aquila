#!/usr/bin/env python3
"""S2 — the ambiguity cascade: unwrap/cycle-slip rates and range-integer success rates.

Simulation, NOT a measurement. Two parts:

(A) Phase cascade (the servo's unwrap problem). Raw relative CFO is tens of kHz
(+-10..20 ppm at 2.442 GHz), so phase between strobes wraps hundreds of times.
The servo must know the relative CFO to |dnu| < pi/(2*pi... ) before it can
track: unwrap condition |dnu*T_s| < pi. A two-scale "cadence vernier"
(phase-swept beacon: strobe pairs T_long apart) acquires CFO; the fast tracker
must then hold it against frequency wander (sigma_w, tau_w). We Monte-Carlo
the cycle-slip probability over a 120 s run for T_s x sigma_w x sigma_art, and
the vernier acquisition failure probability for T_long.

(B) Range-integer cascade: FTM (~1 m bias) -> stitched delay over ~83.5 MHz
(ambiguity c/2B = 1.79 m) -> two-way carrier phase (ambiguity lambda/2 = 6.1 cm).
Stage-A seed comes either from FTM averaging (multipath bias does not average)
or from the RIPS-style double difference (s5_rips_dd.py; bias-free, servo-noise
limited). Delay-stage precision is artefact-immune because the artefact is a
common phase across subcarriers (does not perturb the slope) — verified here.
"""
import numpy as np, os
from s1_servo import F_of, Q_of, steady_P   # shared servo model (same directory)

OUT = os.path.dirname(os.path.abspath(__file__))
FC = 2.442e9; C = 299_792_458.0; LAM = C/FC

# ---------------------------------------------------------------- (A) unwrap / slips
def acq_fail(T_s, sigma_w_hz, sigma_art, T_long, tau_w=10.0, sig_m=0.03, sig_pn=0.045, n=20000, seed=11):
    """Vernier acquisition failure probability (vectorised).
    The vernier measures the AVERAGE CFO over T_long; the unwrap needs the CFO at the
    first fast strobe (the window's end) to |dnu| < 1/(2 T_s). Error = vernier noise
    + (nu(end) - nu_avg) OU fluctuation."""
    rng = np.random.default_rng(seed)
    sig_z = np.sqrt(2.0)*np.sqrt(sig_m**2 + sigma_art**2 + sig_pn**2)
    dnu_acq = np.sqrt(2)*sig_z/(2*np.pi*T_long)                       # vernier two-frame noise (Hz)
    sig2 = sigma_w_hz**2
    var_avg = 2*sig2/T_long**2*(tau_w*T_long - tau_w**2*(1-np.exp(-T_long/tau_w)))
    cov_end_avg = sig2*tau_w/T_long*(1-np.exp(-T_long/tau_w))
    var_end_minus_avg = sig2 + var_avg - 2*cov_end_avg                # OU exact
    dnu = rng.normal(0, np.sqrt(dnu_acq**2 + var_end_minus_avg), n)
    return np.mean(np.abs(dnu) > 1.0/(2*T_s))

def slip_rate_analytic(T_s, sigma_w_hz, sigma_art, tau_w=10.0, sig_m=0.03, sig_pn=0.045,
                       T_run=120.0, margin=0.9):
    """Cycle-slip rate of the steady-state [theta,nu] servo. The innovation is
    ~N(0, S) with S = P^-_thth + R; a slip is |innov| > margin*pi (unwrap failure)."""
    from scipy.stats import norm
    sig_z = np.sqrt(2.0)*np.sqrt(sig_m**2 + sigma_art**2 + sig_pn**2)
    F, Q = F_of(T_s, tau_w), Q_of(T_s, tau_w, 2*np.pi*sigma_w_hz)
    P = steady_P(T_s, tau_w, 2*np.pi*sigma_w_hz, sig_z)
    Pm = F@P@F.T + Q
    S = Pm[0, 0] + sig_z**2
    p_step = 2*norm.sf(margin*np.pi/np.sqrt(S))
    K = T_run/T_s
    return p_step*K, np.sqrt(S)                      # expected slips per run, innovation std

def slip_mc_fast(T_s, sigma_w_hz, sigma_art, T_run=30.0, n_trial=48, seed=13, tau_w=10.0,
                 sig_m=0.03, sig_pn=0.045):
    """Vectorised MC validation of the slip rate, using the steady-state Kalman gains."""
    rng = np.random.default_rng(seed)
    sig_z = np.sqrt(2.0)*np.sqrt(sig_m**2 + sigma_art**2 + sig_pn**2)
    K = int(T_run/T_s); sw_rad = 2*np.pi*sigma_w_hz; s2 = 2*sw_rad**2/tau_w
    F, Q = F_of(T_s, tau_w), Q_of(T_s, tau_w, sw_rad)
    P = steady_P(T_s, tau_w, sw_rad, sig_z)
    Pm = F@P@F.T + Q; S = Pm[0, 0] + sig_z**2
    k_theta, k_nu = Pm[0, 0]/S, Pm[1, 0]/S          # steady-state Kalman gains
    nu = rng.normal(0, sw_rad, n_trial); th = rng.uniform(-np.pi, np.pi, n_trial)
    th_hat = th.copy(); nu_hat = nu + rng.normal(0, 2*np.pi*1.0, n_trial)   # 1 Hz acq error
    slips = np.zeros(n_trial, bool)
    sdnu = np.sqrt(s2*T_s); sdA = np.sqrt(s2*T_s**3/12.0)
    for k in range(K):
        dnu = sdnu*rng.standard_normal(n_trial)
        A = 0.5*T_s*dnu + sdA*rng.standard_normal(n_trial)
        th = th + nu*T_s + A; nu = nu*(1 - T_s/tau_w) + dnu
        z = th + sig_z*rng.standard_normal(n_trial)
        th_pred = th_hat + nu_hat*T_s                    # advance state first
        innov = np.angle(np.exp(1j*(z - th_pred)))
        slips |= np.abs(innov) > 0.9*np.pi
        th_hat = th_pred + k_theta*innov
        nu_hat = nu_hat + k_nu*innov
    return np.mean(slips)

def part_a():
    print("== S2(A) unwrap / cycle-slip: acquisition failure & expected slips per 120 s run ==")
    Ts_grid = [0.001, 0.005, 0.02, 0.05, 0.1, 0.2]
    print("T_s (ms) | " + " | ".join(f"sw={sw:.0f} Hz" for sw in [10.0, 50.0, 100.0]) +
          "   (acq_fail @T_long=1 s, expected slips, innovation std)")
    for Ts in Ts_grid:
        cells = []
        for sw in [10.0, 50.0, 100.0]:
            af = acq_fail(Ts, sw, 0.17, T_long=1.0)
            es, ss = slip_rate_analytic(Ts, sw, 0.17)
            cells.append(f"{af:5.3f}, {es:5.3f}, {ss:5.2f}")
        print(f"{Ts*1000:7.0f}  | " + " | ".join(cells))
    print("\nT_long sensitivity (T_s=20 ms, sw=50 Hz): acq_fail")
    for Tl in [0.1, 0.5, 1.0, 2.0, 5.0]:
        print(f"  T_long={Tl:4.1f} s: acq_fail = {acq_fail(0.02, 50.0, 0.17, Tl):.3f}")
    print("\nsigma_art sensitivity (T_s=20 ms, sw=50 Hz, T_long=1 s): acq_fail, expected slips/120 s")
    for sa in [0.02, 0.17, 0.5, 1.0]:
        es, ss = slip_rate_analytic(0.02, 50.0, sa)
        print(f"  sigma_art={sa:.2f}: acq_fail={acq_fail(0.02, 50.0, sa, 1.0):.3f}, slips={es:.3f}, innov={ss:.2f} rad")
    print("\nMC validation of slip rate (30 s runs, 48 trials):")
    for Ts, sw in [(0.02, 50.0), (0.05, 50.0), (0.02, 10.0)]:
        mc = slip_mc_fast(Ts, sw, 0.17)
        es, _ = slip_rate_analytic(Ts, sw, 0.17, T_run=30.0)
        print(f"  Ts={Ts*1000:.0f} ms sw={sw:.0f} Hz: MC slip fraction = {mc:.3f} "
              f"(analytic expected slips/30 s = {es:.3f} -> P>=1 = {1-np.exp(-es):.3f})")

# ---------------------------------------------------------------- (B) range cascade
def stage_B_delay_sigma(n_ch=13, n_sc=52, B_ch=20e6, snr_db=10.0, n_trial=400, seed=5):
    """Stitched-delay precision via LS slope fit across n_ch x n_sc subcarriers.
    Per-subcarrier phase noise = 1/sqrt(2*rho) (thermal only): the CFO-correction
    artefact is a COMMON phase per frame and cancels in the slope — demonstrated
    by re-running with a random common phase per frame."""
    rng = np.random.default_rng(seed)
    B_eff = (n_ch-1)*5e6 + B_ch
    f = np.concatenate([(np.arange(n_sc) - n_sc/2)*(B_ch/n_sc) + ch*5e6 for ch in range(n_ch)])
    f = f - f.mean()
    tau = 40e-9                       # true 12 m range
    sc_sig = 1/np.sqrt(2*10**(snr_db/10))
    def run(common_art):
        sig = np.empty(n_trial)
        for i in range(n_trial):
            art = rng.normal(0, 0.17) if common_art else 0.0
            ph = -2*np.pi*f*tau + art + sc_sig*rng.standard_normal(f.size)
            A = np.vstack([np.ones_like(f), f]).T
            coef, *_ = np.linalg.lstsq(A, ph, rcond=None)
            sig[i] = -coef[1]/(2*np.pi)          # delay estimate (s); slope = -2*pi*tau
        return np.std(sig)*C                    # delay std in metres
    s_noart = run(False); s_art = run(True)
    return s_noart, s_art, B_eff

def cascade_success(sig_B, seed=7, n_trial=20000, n_ftm=100):
    """Stage A (FTM seed, bias U(-1,1) m, noise 0.3/sqrt(n)); stage B integer (amb 1.79 m);
    stage C carrier integer (amb 6.1 cm, needs |e_B| < 3.07 cm)."""
    rng = np.random.default_rng(seed)
    bias = rng.normal(0, 1.5, n_trial)                   # FTM metre-class bias (note 15)
    seed_err = bias + rng.normal(0, 0.3/np.sqrt(n_ftm), n_trial)
    okA = np.abs(seed_err) < 0.5*C/((13-1)*5e6+20e6)     # half of stitched ambiguity
    eB = rng.normal(0, sig_B, n_trial)
    okB = np.abs(eB) < LAM/4                             # carrier-integer handover
    return np.mean(okA), np.mean(okB), np.mean(okA & okB)

def part_b():
    print("\n== S2(B) range-integer cascade ==")
    s_noart, s_art, B_eff = stage_B_delay_sigma()
    print(f"B_eff = {B_eff/1e6:.1f} MHz; stitched-delay sigma (no artefact) = {s_noart*100:.2f} cm, "
          f"(with 0.17 rad common artefact) = {s_art*100:.2f} cm  -> artefact-immune: {s_art < 1.3*s_noart}")
    pA, pB, pAB = cascade_success(s_noart)
    print(f"P(stage A: FTM seed inside half-ambiguity) = {pA:.3f}")
    print(f"P(stage B->C: stitched handover) = {pB:.4f}")
    print(f"P(cascade closes, FTM-seeded) = {pAB:.3f}")
    # RIPS-DD-seeded variant: seed error = sigma_DD-geometry (from s5): 0.15 m typical, bias-free
    rng = np.random.default_rng(2)
    for s_dd in [0.05, 0.15, 0.30]:
        ok = np.abs(rng.normal(0, s_dd, 20000)) < 0.5*C/B_eff
        print(f"P(cascade closes, RIPS-DD seed sigma={s_dd:.2f} m) = {np.mean(ok):.4f}")

if __name__ == "__main__":
    part_a(); part_b()
