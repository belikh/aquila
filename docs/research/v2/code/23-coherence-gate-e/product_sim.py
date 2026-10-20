#!/usr/bin/env python3
"""CG-1 two-way product budget and the averaging law.

SIMULATION ONLY (EXPERIMENT-ENV.md honesty rules).

(a) Two-way product residual. For a reciprocal exchange (A pings at t1, B
replies at t3; the reply interval is T_reply = t3 - t2 known only to the
software/queueing accuracy sigma_j), the product phase
    angle(h_AB h_BA) = 2 angle(h) + 2 pi Deltaf_raw T_reply + hw
cancels both LOs exactly. After correcting with the tracked Deltaf_hat and the
nominal reply interval, the residual budget is
    sigma_prod^2 = 2 sigma_meas^2                  (two CSI phase measurements)
                 + (2 pi Deltaf_raw sigma_j)^2     (reply jitter x raw CFO)
                 + (2 pi sigma_Dfhat T_reply)^2     (CFO-track error x interval)
(The motion non-reciprocity term 2 pi f_D T_reply is reported separately: for
the static-scene survey it is zero; during sensing it *is* the Doppler signal.)

(b) Averaging law. Faces on a conducted reference: each face's de-rotated
phase noise = common-mode part (reference distribution + common LO) + independent
part; sigma(N) = sqrt(sigma_c^2 + sigma_i^2 / N). Packets: M exchanges average
the iid part (jitter x CFO is iid across exchanges).

Run:  /home/matt/projects/aquila-v2-spec/sim-python product_sim.py
"""
import os

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "out")
os.makedirs(OUT, exist_ok=True)

TWOPI = 2 * np.pi
SEED = 42
NMC = 20000

DF_RAW = [24.4e3, 48.8e3, 97.6e3]      # Hz: 10 / 20 / 40 ppm total difference
SIGMA_J = [0.1e-6, 0.3e-6, 1.0e-6, 3.0e-6, 10e-6]
SIGMA_MEAS = [0.03, 0.1, 0.3]         # rad per-frame CSI phase noise
T_REPLY = [0.5e-3, 1e-3, 2e-3, 5e-3]  # s reply interval
SIGMA_DF = 10.0                        # Hz tracked-CFO error fed to the correction
BARS = [0.3, 0.5]                      # rad pass bars

# averaging law parameters (faces on the conducted reference)
SIG_C = [0.005, 0.02, 0.05]            # rad common-mode floor
SIG_I = [0.05, 0.1, 0.3]               # rad independent per-face noise
NFACES = 20


def main():
    rng = np.random.default_rng(SEED)

    # ---- (a) product residual: analytic + Monte-Carlo ------------------
    rows = []
    for df_raw in DF_RAW:
        for sj in SIGMA_J:
            for sm in SIGMA_MEAS:
                for tr in T_REPLY:
                    var = (2 * sm**2 + (TWOPI * df_raw * sj)**2
                           + (TWOPI * SIGMA_DF * tr)**2)
                    sig = np.sqrt(var)
                    # Monte-Carlo confirmation (static channel)
                    n = np.sqrt(2.0) * sm * rng.standard_normal(NMC) \
                        + TWOPI * df_raw * sj * rng.standard_normal(NMC) \
                        + TWOPI * SIGMA_DF * tr * rng.standard_normal(NMC)
                    rows.append(dict(df_raw_kHz=df_raw / 1e3, sigma_j_us=sj * 1e6,
                                     sigma_meas=sm, T_reply_ms=tr * 1e3,
                                     sigma_prod=sig, sigma_prod_mc=np.std(n)))
    prod = pd.DataFrame(rows)
    prod.to_csv(os.path.join(OUT, "product_budget.csv"), index=False)

    # killing sigma_j: 2 pi df_raw sigma_j = bar  (jitter term alone)
    kill = []
    for df_raw in DF_RAW:
        for bar in BARS:
            kill.append(dict(df_raw_kHz=df_raw / 1e3, bar_rad=bar,
                             sigma_j_kill_us=(bar / (TWOPI * df_raw)) * 1e6))
    killdf = pd.DataFrame(kill)
    killdf.to_csv(os.path.join(OUT, "product_jitter_kill.csv"), index=False)
    print("Killing reply jitter (jitter term alone reaches the bar):")
    print(killdf.pivot(index="df_raw_kHz", columns="bar_rad",
                        values="sigma_j_kill_us").to_string())

    # averaging over M static exchanges (iid terms)
    avg_rows = []
    sm, df_raw, tr = 0.03, 48.8e3, 1e-3
    for sj in [0.3e-6, 1.0e-6, 3.0e-6]:
        base = np.sqrt(2 * sm**2 + (TWOPI * df_raw * sj)**2
                       + (TWOPI * SIGMA_DF * tr)**2)
        for M in [1, 2, 4, 8, 16]:
            avg_rows.append(dict(sigma_j_us=sj * 1e6, M=M,
                                 sigma_prod_M=base / np.sqrt(M)))
    avgdf = pd.DataFrame(avg_rows)

    # ---- (b) face-averaging law ----------------------------------------
    law_rows = []
    for sc in SIG_C:
        for si in SIG_I:
            for N in range(1, NFACES + 1):
                sig = np.sqrt(sc**2 + si**2 / N)
                law_rows.append(dict(sigma_c=sc, sigma_i=si, N=N, sigma_N=sig))
    law = pd.DataFrame(law_rows)
    law.to_csv(os.path.join(OUT, "face_averaging.csv"), index=False)

    # Monte-Carlo confirmation of the law at one point
    sc, si = 0.02, 0.1
    N = 20
    common = rng.standard_normal(NMC) * sc
    indep = rng.standard_normal((NMC, N)) * si
    sig_mc = np.std(common + indep.sum(axis=1) / N)
    print(f"\nFace-averaging MC check (sc={sc}, si={si}, N={N}): "
          f"mc={sig_mc:.4f} rad, closed form={np.sqrt(sc**2 + si**2/N):.4f} rad")

    # ---- figure 3 -------------------------------------------------------
    fig, axes = plt.subplots(1, 2, figsize=(11, 4.2))
    ax = axes[0]
    for sm in SIGMA_MEAS:
        sub = prod[(prod.df_raw_kHz == 48.8) & (prod.sigma_meas == sm)
                   & (prod.T_reply_ms == 1.0)]
        ax.plot(sub.sigma_j_us, sub.sigma_prod, "o-",
                label=f"$\\sigma_m$={sm:g} rad")
    ax.axhline(0.5, color="k", ls=":", lw=1)
    ax.axhline(0.3, color="k", ls=":", lw=0.7)
    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.set_xlabel(r"reply-interval jitter $\sigma_j$ ($\mu$s)")
    ax.set_ylabel(r"$\sigma_{prod}$ (rad)")
    ax.set_title("two-way product residual ($\\Delta f_{raw}$=48.8 kHz, $T_{reply}$=1 ms)")
    ax.legend(fontsize=8)
    ax.grid(alpha=0.3)

    ax = axes[1]
    for sc in SIG_C:
        for si in [0.1]:
            sub = law[(law.sigma_c == sc) & (law.sigma_i == si)]
            ax.plot(sub.N, sub.sigma_N, "o-", label=f"$\\sigma_c$={sc:g}, $\\sigma_i$={si:g}")
    ax.plot(range(1, 21), 0.1 / np.sqrt(np.arange(1, 21)), "k:", lw=1,
            label="pure $1/\\sqrt{N}$")
    ax.set_xlabel("faces N (conducted reference)")
    ax.set_ylabel(r"$\sigma$ of face-averaged phase (rad)")
    ax.set_title("face/packet averaging law")
    ax.legend(fontsize=7)
    ax.grid(alpha=0.3)
    fig.tight_layout()
    fig.savefig(os.path.join(OUT, "fig3_product_avg.png"), dpi=110)
    print("wrote fig3_product_avg.png")

    # headline numbers for the note
    print("\nDesign points (sigma_meas=0.03 rad, T_reply=1 ms, sigma_Df=10 Hz):")
    print(prod[(prod.sigma_meas == 0.03) & (prod.T_reply_ms == 1.0)]
          [["df_raw_kHz", "sigma_j_us", "sigma_prod"]].to_string(index=False))
    print("\nProduct averaging over M static exchanges (sj=1us, df=48.8 kHz):")
    print(avgdf[avgdf.sigma_j_us == 1.0].to_string(index=False))
    print("\nFace averaging, N=20:")
    print(law[law.N == 20].pivot(index="sigma_c", columns="sigma_i",
                                 values="sigma_N").to_string())


if __name__ == "__main__":
    main()
