#!/usr/bin/env python3
"""S3 — reply-jitter x CFO (decisive quantity 2), CSMA tails, and averaging laws (quantity 3).

Simulation, NOT a measurement.
- The two-way product's only dynamic error is 2*pi*nu_rel*xi, xi = turnaround
  jitter of the reply (no TX hardware timestamp exists on ESP32-S3).
- With raw crystal offsets (nu ~ 24-49 kHz) the phase wraps for xi > ~10 us;
  with servo/disciplined nu the term collapses. Tables + a CSMA-tail Monte Carlo
  showing where sqrt(N) averaging of repeated exchanges survives and where it
  does not (wrapped/uniform tails), and what RTT gating recovers.
- Averaging laws: (i) burst strobes — LS fit of K frames in a window W, variance
  reduction vs K (sqrt(K) until wander curvature dominates);
  (ii) face averaging inside a conducted puck — i.i.d. artefact/thermal averages
  as sqrt(N_f), the LO/servo term is common-mode (does not average at all).
"""
import numpy as np, os

OUT = os.path.dirname(os.path.abspath(__file__))
FC = 2.442e9; C = 299_792_458.0; LAM = C/FC

def jitter_table():
    print("== S3(a) reply-jitter x CFO budget: max xi for sigma_phi_jit target ==")
    print("nu_rel (Hz)   ppm    xi_max(0.1 rad)  xi_max(0.3 rad)  xi_max(0.5 rad)")
    for nu, ppm in [(2.44, "1 ppb (VCTCXO+FTM)"), (24.4, "10 ppb"), (244, "0.1 ppm"),
                    (2442, "1 ppm"), (24420, "10 ppm (one crystal)"), (48840, "20 ppm (pair)")]:
        r1, r3, r5 = [t/(2*np.pi*nu) for t in (0.1, 0.3, 0.5)]
        print(f"{nu:9.0f}  {ppm:>18s}  {r1*1e6:12.1f} us {r3*1e6:12.1f} us {r5*1e6:12.1f} us")

def csma_tail_mc():
    print("\n== S3(b) CSMA-tail Monte Carlo (nu=48.84 kHz raw pair CFO, scheduled slots sigma=0.3 us) ==")
    rng = np.random.default_rng(9)
    nu = 48840.0
    Ks = [1, 4, 16, 64]
    print("p_defer | estimator | K=:" + " | ".join(f"{K:>8d}" for K in Ks) + "   (std of wrapped estimate, rad)")
    for p_defer, tau_d in [(0.0, 0.0), (0.2, 2e-3), (0.5, 2e-3)]:
        for est in ["mean", "median", "RTT-gate"]:
            xi = np.abs(rng.normal(0, 0.3e-6, (20000, 64)))
            defer = rng.random((20000, 64)) < p_defer
            xi = xi + defer*(rng.exponential(tau_d, (20000, 64)) + 0.3e-6)
            eps = 2*np.pi*nu*(xi - np.median(xi, axis=1, keepdims=True))
            sig = []
            for K in Ks:
                e = eps[:, :K]
                if est == "mean":
                    v = np.mean(e, axis=1)
                elif est == "median":
                    v = np.median(e, axis=1)
                else:
                    thr = np.quantile(xi[:, :K], 0.8, axis=1, keepdims=True)
                    w = (xi[:, :K] <= thr)
                    v = (e*w).sum(axis=1)/np.maximum(w.sum(axis=1), 1)
                sig.append(np.std((v + np.pi) % (2*np.pi) - np.pi))
            print(f"{p_defer:7.1f} | {est:9s} | " + " | ".join(f"{s:8.3f}" for s in sig))

def burst_averaging():
    print("\n== S3(c) burst-averaging law: LS-fit phase at window centre, K frames over W ==")
    rng = np.random.default_rng(4)
    sig_w, tau, sig_art, sig_m, sig_pn = 2*np.pi*50.0, 10.0, 0.17, 0.03, 0.045
    sig_z = np.sqrt(2)*np.sqrt(sig_m**2 + sig_art**2 + sig_pn**2)
    s2 = 2*sig_w**2/tau
    n_trial = 400
    print("W (ms) | K:  " + " | ".join(f"{K:>7d}" for K in [1, 2, 4, 8, 16]) +
          "  | sqrt(K) prediction at K=16")
    for W in [1e-3, 8e-3, 64e-3]:
        row = []
        for K in [1, 2, 4, 8, 16]:
            ts = np.linspace(-W/2, W/2, K)
            errs = np.empty(n_trial)
            for i in range(n_trial):
                nu0 = rng.normal(0, sig_w); th0 = rng.normal(0, np.pi)
                nu = nu0; th = th0; THs = []
                for j in range(K-1):
                    h = ts[j+1]-ts[j]
                    dnu = np.sqrt(s2*h)*rng.standard_normal()
                    A = 0.5*h*dnu + np.sqrt(s2*h**3/12)*rng.standard_normal()
                    THs.append(th); th = th + nu*h + A; nu = nu*(1-h/tau) + dnu
                THs.append(th)
                z = np.array(THs) + sig_z*rng.standard_normal(K)
                Aa = np.vstack([np.ones(K), ts]).T
                coef, *_ = np.linalg.lstsq(Aa, z, rcond=None)
                errs[i] = coef[0] - th0          # estimate at window centre ~ th0 + nu0*0 ... compare to truth at t=0
            row.append(np.std(errs))
        print(f"{W*1e3:6.1f} | " + " | ".join(f"{v:7.3f}" for v in row) + f"  | {row[0]/4:.3f}")

def face_averaging():
    print("\n== S3(d) face averaging inside a conducted puck ==")
    sig_art, sig_m, sig_pn = 0.17, 0.03, 0.045
    for N_f in [1, 4, 20]:
        iid = np.sqrt(sig_art**2 + sig_m**2 + sig_pn**2)/np.sqrt(N_f)
        common = np.sqrt(sig_art**2 + sig_pn**2)  # LO/servo common-mode part (worst case: artefact-like)
        print(f"N_f={N_f:3d}: i.i.d. thermal part {np.sqrt(sig_m**2)/np.sqrt(N_f):.4f} rad, "
              f"artefact+PN part {np.sqrt(sig_art**2+sig_pn**2)/np.sqrt(N_f):.4f} rad, "
              f"LO/servo common-mode part unchanged (no sqrt(N) gain)")

if __name__ == "__main__":
    jitter_table(); csma_tail_mc(); burst_averaging(); face_averaging()
