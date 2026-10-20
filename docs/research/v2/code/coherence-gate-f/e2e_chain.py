#!/usr/bin/env python
"""Round-2 T1 (character `f`): end-to-end chain — calibration -> coherence ->
localisation — Monte-Carlo, plus the protocol airtime ledger.

Chain (all in the hub puck's coordinate frame; room-frame alignment is a
Procrustes step outside this simulation and noted as such):

  1. CALIBRATION (self-survey): each anchor node's position in the hub frame
     from (a) FTM coarse range (sigma 1.5 m, note 17), refined by (b) the
     two-way 13-channel stitched delay (sigma 8 cm, note-m priors, carrier
     integers resolved at the design sigma_phi; slips killed by median-of-3),
     and (c) the PSB q-range bearing to the hub (sigma 0.18 deg at
     sigma_theta = 0.05 rad, from sweep_beacon.py E3; basin needs a <=10 deg
     prior from intra-puck coarse AoA or the previous fix).
  2. COHERENCE: per-sensing-frame cross-node phase fixes from the PSB ladder
     (sigma_phi = 0.056 rad design point from m2_residual.py D2; phase fix at
     the sensing frame = 0.075 rad from the E1 prediction law).
  3. LOCALISATION: a target node W is localised by Levenberg-Marquardt
     multilateration of two-way stitched ranges against the self-surveyed
     anchors, with and without the cross-node coherent phase-difference
     observable (differential ranges at ~2 mm noise; carrier-integer slips per
     pair at the design per-burst rate, down-weighted robustly).

Part G: airtime ledger of the superframe.

SIMULATION ONLY (NumPy estimator models; parameters cited from the round-1
corpus in the note). Multipath bias is entered as a bounded budget term and
labelled as such — it is not physically simulated.
Run:  /home/matt/projects/aquila-v2-spec/sim-python e2e_chain.py
Out:  e2e_summary.csv, e2e_figure.png
"""
import csv
import numpy as np

rng = np.random.default_rng(20261016)
C = 2.99792458e8
FC = 2.437e9
LAM = C / FC

HUB = np.array([4.0, 2.5, 1.6])                       # hub puck
ANCH = np.array([[0.4, 0.4, 1.9], [7.6, 0.5, 1.2],
                 [7.5, 4.6, 2.0], [0.5, 4.5, 0.9]])  # 4 anchors
N_ANC = len(ANCH)
PAIRS = [(i, j) for i in range(N_ANC) for j in range(i + 1, N_ANC)]
N_PAIR = len(PAIRS)
II = np.array([i for i, _ in PAIRS])
JJ = np.array([j for _, j in PAIRS])

# ladder prediction factor at 1 ms lag, K=8 frames, two bursts averaged
K, DT, ELL = 8, 100e-6, 1e-3
PRED_FAC = (1.0 + 3.0 * (2 * ELL + DT * (K - 1)) ** 2
            / (DT**2 * (K * K - 1))) / K / 2.0
SIG_N_FRAME = 0.05                                    # rad per frame
SIG_FIX = SIG_N_FRAME * np.sqrt(PRED_FAC)             # phase fix at sensing frame


def rand_targets(n):
    return np.column_stack([rng.uniform(0.6, 7.4, n), rng.uniform(0.6, 4.4, n),
                            rng.uniform(0.3, 2.2, n)])


print("== F: end-to-end chain Monte-Carlo ==")
N = 4000
targets = rand_targets(N)
TRUE_R = np.linalg.norm(targets[:, None, :] - ANCH[None, :, :], axis=2)

# --- stage 1: self-survey of anchors -----------------------------------------
SIG_RANGE, SIG_BEAR, SLIP_P = 0.08, np.deg2rad(0.18), 0.026

true_rel = ANCH - HUB
r_true = np.linalg.norm(true_rel, axis=1)
u = true_rel / r_true[:, None]
n_med = 3
rr = r_true[None, :] + SIG_RANGE * rng.standard_normal((n_med, N, N_ANC))
rr += (rng.random((n_med, N, N_ANC)) < SLIP_P) * rng.normal(0.0, 1.0,
                                                           (n_med, N, N_ANC))
rr = np.median(rr, axis=0)
tang = rng.standard_normal((N, N_ANC, 3))
tang -= np.einsum("nak,ak->na", tang, u)[:, :, None] * u[None, :, :]
tang /= np.linalg.norm(tang, axis=2, keepdims=True)
u_noisy = u[None, :, :] + SIG_BEAR * tang
u_noisy /= np.linalg.norm(u_noisy, axis=2, keepdims=True)
anc_pos = HUB[None, None, :] + rr[:, :, None] * u_noisy      # (N, A, 3)
anc_err = np.sqrt(np.mean(np.sum((anc_pos - ANCH[None, :, :]) ** 2, axis=2)))
ANC_SELF = anc_pos.copy()      # S2c temporarily replaces anc_pos; restore after
print(f"  stage 1 self-survey: anchor position RMS = {anc_err*100:5.1f} cm"
      f"  (stitched range sigma {SIG_RANGE*100:.0f} cm, q-range bearing"
      f" {np.rad2deg(SIG_BEAR):.2f} deg, median-of-3 vs slips)")


def linear_init(anchor_pos, rr):
    a0 = anchor_pos[:, 0]
    A = 2 * (anchor_pos[:, 1:] - a0[:, None, :])
    b = (rr[:, :1] ** 2 - rr[:, 1:] ** 2
         - np.sum(a0 ** 2, axis=1)[:, None]
         + np.sum(anchor_pos[:, 1:] ** 2, axis=2))
    return np.linalg.solve(A, b[:, :, None])[:, :, 0]


def lm_solve(anchor_pos, rr, diffs, robust, iters=20):
    n = len(rr)
    x = linear_init(anchor_pos, rr)
    lam = np.full(n, 1.0)
    ones = np.ones(n)

    def cost(x):
        d = np.linalg.norm(x[:, None, :] - anchor_pos, axis=2)
        r = d - rr
        w = 1.0 / (1.0 + (r / 0.35) ** 2) if robust else np.ones_like(r)
        c = np.sum(w * r * r, axis=1)
        if diffs is not None:
            for k, (i, j) in enumerate(PAIRS):
                rij = (d[:, i] - d[:, j]) - diffs[:, k]
                wij = (1.0 / (1.0 + (rij / (3 * LAM)) ** 2)) if robust else ones
                c = c + wij * rij * rij
        return c, d

    c0, d = cost(x)
    for _ in range(iters):
        w = 1.0 / (1.0 + ((d - rr) / 0.35) ** 2) if robust else np.ones_like(d)
        J = (x[:, None, :] - anchor_pos) / np.maximum(d, 0.05)[:, :, None]
        WJ = J * w[:, :, None]
        JtJ = np.einsum("nrk,nrl->nkl", WJ, J)
        Jtr = np.einsum("nrk,nr->nk", WJ, d - rr)
        if diffs is not None:
            for k, (i, j) in enumerate(PAIRS):
                rij = (d[:, i] - d[:, j]) - diffs[:, k]
                wij = (1.0 / (1.0 + (rij / (3 * LAM)) ** 2)) if robust else ones
                gi = (x - anchor_pos[:, i]) / np.maximum(d[:, i], 0.05)[:, None]
                gj = (x - anchor_pos[:, j]) / np.maximum(d[:, j], 0.05)[:, None]
                G = wij[:, None] * (gi - gj)
                JtJ = JtJ + np.einsum("nk,nl->nkl", G, G)
                Jtr = Jtr + np.einsum("nk,n->nk", G, rij)
        for _ in range(8):
            dx = -np.linalg.solve(JtJ + lam[:, None, None] * np.eye(3),
                                  Jtr[:, :, None])[:, :, 0]
            xn = x + dx
            cn, dn = cost(xn)
            better = cn < c0
            lam = np.where(better, lam / 3, lam * 10)
            x = np.where(better[:, None], xn, x)
            c0 = np.where(better, cn, c0)
            d = np.where(better[:, None], dn, d)
            if not better.any():
                break
        if np.max(np.abs(dx)) < 1e-4:
            break
    return x


def localise(use_phase, sigma_range=0.08, slip_p=0.026, multipath_bias=0.0,
             anchor_pos=None):
    if anchor_pos is None:
        anchor_pos = anc_pos
    # two-way stitched ranges, median-of-3 across superframes vs slips
    rr = TRUE_R[None, :, :] + sigma_range * \
        rng.standard_normal((n_med, N, N_ANC))
    rr += (rng.random((n_med, N, N_ANC)) < slip_p) * \
        rng.normal(0.0, 1.0, (n_med, N, N_ANC))
    rr = np.median(rr, axis=0)
    if multipath_bias:
        rr = rr + rng.uniform(-multipath_bias, multipath_bias, (N, N_ANC))
    diffs = None
    if use_phase:
        sd = LAM * np.sqrt(2.0) * SIG_FIX / (2 * np.pi)      # ~2 mm
        dv = TRUE_R[:, II] - TRUE_R[:, JJ] + sd * rng.standard_normal((N, N_PAIR))
        bad = (rng.random((N, N_PAIR)) < slip_p) & \
              (rng.random((N, N_PAIR)) < slip_p)
        diffs = dv + bad * rng.normal(0.0, LAM, (N, N_PAIR))
    x = lm_solve(anc_pos, rr, diffs, robust=True)
    e3 = np.linalg.norm(x - targets, axis=1)
    e_h = np.linalg.norm((x - targets)[:, :2], axis=1)
    e_v = np.abs((x - targets)[:, 2])
    return (float(np.sqrt(np.mean(e3 ** 2))), float(np.percentile(e3, 95)),
            float(np.sqrt(np.mean(e_h ** 2))), float(np.sqrt(np.mean(e_v ** 2))))


scen = [
    ("S1 design: stitched ranges only (8 cm, median-of-3)",
     dict(use_phase=False)),
    ("S2 design + coherent phase differences (mm differential)",
     dict(use_phase=True)),
    ("S2b S2 + bounded multipath bias +-5 cm (budget term)",
     dict(use_phase=True, multipath_bias=0.05)),
    ("S3 sigma_phi=0.30 rad: integers fail (22%), delay-only 22 cm",
     dict(use_phase=False, sigma_range=0.22, slip_p=0.775)),
    ("S4 cascade dead (E-1 red): FTM-only multilateration",
     dict(use_phase=False, sigma_range=1.5, slip_p=0.0)),
    # S2c last: it swaps in perfect anchors, and running it in-list-order would
    # consume RNG draws that the scenarios above it expect (see ANC_SELF reset).
    ("S2c S2 with perfectly surveyed anchors (isolates coherence floor)",
     dict(use_phase=True, perfect_anchors=True)),
]
f_rows = []
print(f"  (phase fix at sensing frame {SIG_FIX:.4f} rad ->"
      f" differential-range noise {LAM*np.sqrt(2)*SIG_FIX/(2*np.pi)*1000:.1f} mm)")
print("  scenario:                                                  RMS3D    P95    RMSxy  RMSz")
for name, kw in scen:
    anc_pos = np.tile(ANCH[None, :, :], (N, 1, 1)) if kw.get("perfect_anchors") \
        else ANC_SELF
    rms, p95, rms_h, rms_v = localise(kw["use_phase"],
                                      sigma_range=kw.get("sigma_range", 0.08),
                                      slip_p=kw.get("slip_p", 0.026),
                                      multipath_bias=kw.get("multipath_bias", 0.0))
    f_rows.append((name, rms, p95, rms_h, rms_v))
    print(f"  {name:58s} {rms*100:6.1f} {p95*100:6.1f}  {rms_h*100:6.1f}"
          f" {rms_v*100:6.1f}  [cm]")

sigma_eff = np.sqrt(SIG_RANGE**2 + anc_err**2)
print(f"\n  budget check S1: per-link sigma_eff = sqrt(8^2 + {anc_err*100:.1f}^2)"
      f" = {sigma_eff*100:.1f} cm; z-DOP of this anchor layout dominates the"
      f" 3D RMS (see RMSz column)")

# ----------------------------------------------------------------------------
# Part G: airtime ledger
# ----------------------------------------------------------------------------
print("\n== G: airtime ledger (superframe = 20 ms) ==")
T_SUPER, K_BURST, T_FRAME, T_RTT = 20e-3, 8, 100e-6, 0.7e-3
duty_burst = K_BURST * T_FRAME / T_SUPER
duty_tw = T_RTT / T_SUPER
print(f"  PSB bursts: {K_BURST} x {T_FRAME*1e6:.0f} us every {T_SUPER*1e3:.0f} ms"
      f" -> {duty_burst:.1%} duty (bursts double as sensing illumination)")
print(f"  two-way round: one node per superframe ({T_RTT*1e6:.0f} us)"
      f" -> {duty_tw:.1%}")
for n_nodes in [4, 8, 20]:
    print(f"  N={n_nodes:2d}: total duty {(duty_burst+duty_tw):.1%};"
          f" two-way refresh every {n_nodes*T_SUPER*1e3:.0f} ms")
t_coh = LAM / 2.0
print(f"  target coherence time lambda/2v = {t_coh*1e3:.1f} ms at 1 m/s"
      f" (> superframe {T_SUPER*1e3:.0f} ms: OK)")
print(f"  a 1 m/s walker biases a broadcast reference fix by 0.5 rad after"
      f" {0.5/(2*np.pi*(1.0/LAM))*1e3:.1f} ms")

# ----------------------------------------------------------------------------
# CSV + figure
# ----------------------------------------------------------------------------
with open("e2e_summary.csv", "w", newline="") as fh:
    w = csv.writer(fh)
    w.writerow(["part", "param", "value", "units"])
    w.writerow(["F_selfsurvey", "anchor_pos_rms", f"{anc_err:.5f}", "m"])
    w.writerow(["F_sigfix", "phase_fix_at_sensing_frame", f"{SIG_FIX:.5f}", "rad"])
    for name, rms, p95, rms_h, rms_v in f_rows:
        w.writerow(["F_localise_rms", name, f"{rms:.5f}", "m"])
        w.writerow(["F_localise_p95", name, f"{p95:.5f}", "m"])
        w.writerow(["F_localise_rmsxy", name, f"{rms_h:.5f}", "m"])
        w.writerow(["F_localise_rmsz", name, f"{rms_v:.5f}", "m"])
    w.writerow(["G_duty", "psb_bursts", f"{duty_burst:.5f}", "frac"])
    w.writerow(["G_duty", "tw_one_per_superframe", f"{duty_tw:.5f}", "frac"])
    w.writerow(["G_target_coh", "lambda_half_v_1ms", f"{t_coh:.5f}", "s"])

try:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    fig, ax = plt.subplots(1, 2, figsize=(9.5, 3.5))
    names = [r[0].split(":")[0] for r in f_rows]
    vals = [r[1] * 100 for r in f_rows]
    vals_h = [r[3] * 100 for r in f_rows]
    yp = np.arange(len(vals))
    ax[0].barh(yp + 0.18, vals, height=0.35, color="#3b6", label="3D RMS")
    ax[0].barh(yp - 0.18, vals_h, height=0.35, color="#79c", label="horizontal RMS")
    ax[0].set_yticks(yp)
    ax[0].set_yticklabels(names, fontsize=6)
    ax[0].set_xlabel("target error [cm]")
    ax[0].set_title("F. end-to-end chain: localisation error")
    ax[0].grid(True, axis="x", alpha=0.3)
    ax[0].axvline(10, color="r", ls="--", lw=0.8, label="10 cm")
    ax[0].legend(fontsize=7)

    ax[1].axis("off")
    rows = [
        ["self-survey (anchors)", f"{anc_err*100:.1f} cm"],
        ["ledger sigma_phi (design)", "0.056 rad"],
        ["localisation 3D RMS (S2)", f"{f_rows[1][1]*100:.1f} cm"],
        ["localisation horiz RMS (S2)", f"{f_rows[1][3]*100:.1f} cm"],
        ["localisation 3D RMS (S3)", f"{f_rows[3][1]*100:.1f} cm"],
        ["FTM-only fallback (S4)", f"{f_rows[4][1]*100:.1f} cm"],
        ["airtime duty (N=4)", f"{(duty_burst+duty_tw):.1%}"],
        ["target coherence time", f"{t_coh*1e3:.0f} ms @1 m/s"],
    ]
    tbl = ax[1].table(cellText=rows, colLabels=["quantity", "value"],
                      loc="center", cellLoc="left")
    tbl.auto_set_font_size(False)
    tbl.set_fontsize(8)
    tbl.scale(1, 1.4)
    ax[1].set_title("G. ledger")
    fig.tight_layout()
    fig.savefig("e2e_figure.png", dpi=100)
    print("\nwrote e2e_summary.csv and e2e_figure.png")
except Exception as exc:  # noqa: BLE001
    print("figure skipped:", exc)
