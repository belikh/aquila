#!/usr/bin/env python3
"""CG-1 bootstrap simulation: CFO acquisition from the delay (SFO) slope.

SIMULATION ONLY (EXPERIMENT-ENV.md honesty rules). Question: on an ESP32-S3
with no CFO register, can the raw relative CFO (up to ~48-98 kHz from +-10..20
ppm crystals) be acquired well enough that the fence's phase unwrapping locks?
Method under test: the per-frame delay (the CSI subcarrier-phase slope and/or
the ns RX timestamp) drifts at the relative ppm rate; a least-squares slope
over a bootstrap window T_b estimates the ppm error, hence the carrier CFO,
without any phase unwrapping.

Delay-observation model per frame:  tau_k = tau_0 + D*t_k + q_k, with
  q_k = uniform(+/-6.25 ns)  [12.5 ns RX-timestamp lattice]
        optionally + N(0, 12.5 ns)  [FFT-window "hunting" jitter]
The CSI-slope thermal contribution (~0.1 ns at HT40, 20 dB) is negligible and
omitted. CFO estimate:  f_hat = 2.44 GHz * slope.  The fence then unwraps iff
|residual rotation between reference frames| < pi, i.e. |Deltaf_error| <
1/(2 T_ref); a 3x safety margin is applied.

Run:  /home/matt/projects/aquila-v2-spec/sim-python bootstrap_sim.py
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

FC = 2.44e9
SEED = 42
NMC = 400

WINDOWS = [0.1, 0.25, 0.5, 1.0, 2.0]       # s bootstrap window
RATES = [50.0, 100.0, 250.0, 500.0]        # frames/s during bootstrap
MODELS = ["lattice", "lattice+hunt"]       # delay-noise model
T_REF = [0.002, 0.005, 0.02]               # s fence period to unlock into


def slope_sigma_q(model, rate, tw):
    """Std of the per-frame delay noise for the given model."""
    n = max(int(rate * tw), 2)
    if model == "lattice":
        return 6.25e-9 / np.sqrt(3)         # uniform +/- 6.25 ns
    return np.sqrt((6.25e-9 / np.sqrt(3))**2 + 12.5e-9**2)


def main():
    rng = np.random.default_rng(SEED)
    rows = []
    for model in MODELS:
        for rate in RATES:
            for tw in WINDOWS:
                n = max(int(rate * tw), 2)
                t = np.linspace(0.0, tw, n)
                sig_q = slope_sigma_q(model, rate, tw)
                # OLS slope variance: sig_q^2 / sum((t - tbar)^2)
                denom = np.sum((t - t.mean()) ** 2)
                var_slope = sig_q**2 / denom
                sig_slope = np.sqrt(var_slope)          # s/s (ppm)
                sig_df = FC * sig_slope                  # Hz
                # Monte-Carlo confirmation
                slopes = np.empty(NMC)
                for m in range(NMC):
                    q = rng.uniform(-6.25e-9, 6.25e-9, n)
                    if model == "lattice+hunt":
                        q = q + rng.standard_normal(n) * 12.5e-9
                    D = 20e-6                            # 20 ppm true offset
                    tau = 100e-9 + D * t + q
                    A = np.vstack([np.ones(n), t]).T
                    slopes[m] = np.linalg.lstsq(A, tau, rcond=None)[0][1]
                mc_sig_df = FC * np.std(slopes)
                row = dict(model=model, rate_hz=rate, window_s=tw, n_frames=n,
                           sigma_df_hz=sig_df, sigma_df_hz_mc=mc_sig_df)
                for tref in T_REF:
                    req = 1.0 / (2.0 * tref) / 3.0        # 3x safety margin
                    row[f"unlock_{tref*1e3:g}ms"] = sig_df < req
                rows.append(row)
    df = pd.DataFrame(rows)
    df.to_csv(os.path.join(OUT, "bootstrap_cfo.csv"), index=False)
    print(df.to_string(index=False))

    fig, ax = plt.subplots(figsize=(6.5, 4))
    for model in MODELS:
        for rate in [100.0, 500.0]:
            sub = df[(df.model == model) & (df.rate_hz == rate)]
            ax.plot(sub.window_s, sub.sigma_df_hz, "o-",
                    label=f"{model}, {rate:g} Hz")
    for tref, c in zip(T_REF, ["tab:red", "tab:orange", "tab:brown"]):
        ax.axhline(1.0 / (2.0 * tref) / 3.0, color=c, ls=":",
                   label=f"unwrap req x1/3 ($T_{{ref}}$={tref*1e3:g} ms)")
    ax.set_yscale("log")
    ax.set_xlabel("bootstrap window $T_b$ (s)")
    ax.set_ylabel(r"$\sigma_{\Delta\hat f}$ (Hz)")
    ax.set_title("Bootstrap CFO error from the delay (SFO) slope")
    ax.legend(fontsize=7)
    ax.grid(alpha=0.3)
    fig.tight_layout()
    fig.savefig(os.path.join(OUT, "fig2_bootstrap.png"), dpi=110)
    print("wrote fig2_bootstrap.png")


if __name__ == "__main__":
    main()
