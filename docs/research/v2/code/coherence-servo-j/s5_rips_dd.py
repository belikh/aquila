#!/usr/bin/env python3
"""S5 — dedicated treatment of the missing round-1 direction (f): RIPS-style /
phase-swept-beacon hub protocols, mapped onto the strobe servo.

Simulation, NOT a measurement. On a single-channel TDM radio the RIPS beat
measurement becomes a *cross-receiver, cross-transmitter double difference (DD)*
of per-frame phases taken from two TX slots (nodes A and B) at slot gap
Delta_slot. Expanding the one-way phase model

  phi_{i<-A}(t) = -2*pi/lambda * r_Ai + [theta_i(t) - psi_A(t)] + hw_iA + noise

the DD  D = (phi_iA - phi_iB) - (phi_jA - phi_jB)  cancels:
  - both TX LO phases psi_A, psi_B exactly (common to both receivers);
  - the receiver LO *instantaneous* phases theta_i, theta_j (they appear as
    differences over the slot gap only);
  - nothing else. The residual LO term is [theta_i(t_A)-theta_i(t_B)] -
    [theta_j(t_A)-theta_j(t_B)] = the pair relative-phase evolution over
    Delta_slot — exactly the quantity the coherence servo predicts. Hence
    DD error = sqrt(2)*sigma_phi,servo(Delta_slot) (+ 2*frame noise terms).

We verify the cancellation numerically (random LO trajectories), quantify
sigma_DD vs gap, and use DDs as a *calibration-constant-free coarse geometry*
seed for the ambiguity cascade (one unknown node position; LS solve).
"""
import numpy as np, os

OUT = os.path.dirname(os.path.abspath(__file__))
FC = 2.442e9; C = 299_792_458.0; LAM = C/FC

def lo_trajectory(T, dt=1e-3, sig_w_hz=50.0, tau_w=10.0, rng=None):
    """Random-walk-ish (OU) LO phase trajectory, rad. sigma in pair-Hz equivalent."""
    sw = 2*np.pi*sig_w_hz; s2 = 2*sw**2/tau_w
    n = int(T/dt)
    nu = rng.normal(0, sw); th = rng.uniform(-np.pi, np.pi)
    out = np.empty(n)
    for k in range(n):
        out[k] = th
        dnu = np.sqrt(s2*dt)*rng.standard_normal()
        th = th + nu*dt + 0.5*dt*dnu + np.sqrt(s2*dt**3/12)*rng.standard_normal()
        nu = nu*(1-dt/tau_w) + dnu
    return out

def dd_mc(gap_ms, sigma_art=0.17, sig_m=0.03, sig_pn=0.045, n_trial=400, seed=1,
          sig_w_hz=50.0, tau_w=10.0, servo_pred=True):
    """MC of the DD observable: returns std of (DD - geometric prediction)."""
    rng = np.random.default_rng(seed)
    # fixed 2-D canonical geometry (metres): TXs A,B; receivers i,j
    xA, xB, xi, xj = np.array([0,0]), np.array([4,0]), np.array([1,3]), np.array([3,3])
    rA = lambda x: np.linalg.norm(x-xA); rB = lambda x: np.linalg.norm(x-xB)
    geo = (rA(xi)-rB(xi)) - (rA(xj)-rB(xj))
    geo_phase = -2*np.pi/LAM*geo
    errs = np.empty(n_trial)
    # per-node servo prediction error over the gap (sigma_pred node ~ artefact+PN floor)
    sig_frame = np.sqrt(sig_m**2 + sigma_art**2 + sig_pn**2)
    sig_servo_node = sig_frame
    for t in range(n_trial):
        # LO phases of i and j at the two slot times
        tr_i = lo_trajectory(gap_ms*1e-3 + 1e-3, sig_w_hz=sig_w_hz, tau_w=tau_w, rng=rng)
        tr_j = lo_trajectory(gap_ms*1e-3 + 1e-3, sig_w_hz=sig_w_hz, tau_w=tau_w, rng=rng)
        psi_A, psi_B = rng.uniform(-np.pi, np.pi, 2)          # TX initial phases (must cancel)
        hw = rng.uniform(-np.pi, np.pi, 4)                     # hw_iA, hw_iB, hw_jA, hw_jB
        th_iA, th_iB = tr_i[0], tr_i[-1]
        th_jA, th_jB = tr_j[0], tr_j[-1]
        if servo_pred:
            # the servo predicts the evolution over the gap; residual = per-node prediction error
            th_iB = th_iA + (tr_i[-1]-tr_i[0]) + rng.normal(0, sig_servo_node)
            th_jB = th_jA + (tr_j[-1]-tr_j[0]) + rng.normal(0, sig_servo_node)
        phi_iA = (-2*np.pi/LAM*rA(xi)) + (th_iA - psi_A) + hw[0] + rng.normal(0, sig_frame)
        phi_iB = (-2*np.pi/LAM*rB(xi)) + (th_iB - psi_B) + hw[1] + rng.normal(0, sig_frame)
        phi_jA = (-2*np.pi/LAM*rA(xj)) + (th_jA - psi_A) + hw[2] + rng.normal(0, sig_frame)
        phi_jB = (-2*np.pi/LAM*rB(xj)) + (th_jB - psi_B) + hw[3] + rng.normal(0, sig_frame)
        D = (phi_iA - phi_iB) - (phi_jA - phi_jB)
        pred = geo_phase - (hw[0]-hw[1]-hw[2]+hw[3])         # hardware DD constant, known after one-time cal
        errs[t] = np.angle(np.exp(1j*(D - pred)))
    return np.std(errs), geo

def geometry_seed_mc(sigma_dd, n_obs=30, n_trial=400, seed=3):
    """LS fix of one unknown node position from n_obs DD observables (2-D).
    Returns std of position error (m)."""
    rng = np.random.default_rng(seed)
    xA = np.array([0,0]); xB = np.array([4,0]); xj = np.array([3,3])
    x_true = np.array([1.0, 3.0])
    errs = []
    for t in range(n_trial):
        # simulate DDs with different virtual receiver pairs? Only one unknown node i:
        # observables = DDs of (i, j) over different TX pairs: vary B's slot positions (5 TXs)
        txs = [np.array([0,0]), np.array([4,0]), np.array([2,4.5]), np.array([-1,2]), np.array([5,2])]
        obs = []
        for a_idx in range(len(txs)):
            for b_idx in range(a_idx+1, len(txs)):
                if a_idx == b_idx: continue
                xa, xb = txs[a_idx], txs[b_idx]
                g = (np.linalg.norm(x_true-xa)-np.linalg.norm(x_true-xb)) \
                    - (np.linalg.norm(xj-xa)-np.linalg.norm(xj-xb))
                obs.append((xa, xb, -2*np.pi/LAM*g + rng.normal(0, sigma_dd)))
        from scipy.optimize import least_squares
        def resid(p):
            return [np.angle(np.exp(1j*((-2*np.pi/LAM*((np.linalg.norm(p-xa)-np.linalg.norm(p-xb))
                -(np.linalg.norm(xj-xa)-np.linalg.norm(xj-xb)))) - o[2]))) for xa, xb, o in [(o[0], o[1], o) for o in obs]]
        x0 = x_true + rng.normal(0, 0.5, 2)                 # FTM-class init
        res = least_squares(lambda p: resid(p), x0, method="lm")
        errs.append(np.linalg.norm(res.x - x_true))
    return np.median(errs), np.mean(errs)

def main():
    print("== S5 RIPS-style double difference (missing direction f) ==")
    print("(a) DD noise vs slot gap (servo-predicted evolution, sigma_w=50 Hz):")
    for gap in [0.1, 0.5, 1, 2, 5, 10, 20]:
        s, _ = dd_mc(gap)
        print(f"  gap={gap:5.1f} ms: sigma_DD = {s:.3f} rad")
    print("\n(b) invariance check: random TX phases must cancel (gap=20 ms, servo OFF):")
    s_no, _ = dd_mc(20.0, servo_pred=False)
    print(f"  with raw evolution included (no servo): DD std vs known-constant prediction = {s_no:.3f} rad "
          f"(should be ~uniform/pi*sqrt... i.e. wrapped, ~1.8 rad, NOT the geometric signal)")
    print("\n(c) geometry seed: unknown-node LS fix from 10 TX-pair DDs:")
    for gap in [1, 5, 20]:
        s_dd, _ = dd_mc(gap)
        med, mean = geometry_seed_mc(s_dd)
        print(f"  gap={gap:4.0f} ms: sigma_DD={s_dd:.3f} rad -> position error median {med*100:5.1f} cm, "
              f"mean {mean*100:5.1f} cm")

if __name__ == "__main__":
    main()
