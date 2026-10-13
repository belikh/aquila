# Research — paired WiFi-CSI datasets and pretrained weights

**Status:** research (out-of-band, not v1 scope). Informed the model-strategy and evaluation specs.
**Last updated:** 2026-10-13. **Sources:** see `../reference/sources.json`.

## Question

What public data pairs WiFi CSI with pose/vitals ground truth, under what licences, and what
accuracy should Aquila consider realistic?

## Findings

- **MM-Fi (NeurIPS 2023 D&B)** — pairs CSI with RGB, infra1/infra2, depth, LiDAR and mmWave;
  2D/3D keypoints, 3D position, dense pose, action labels; ~320k synchronised frames, 40
  subjects, 4 environments, 27 actions. **Licence: CC BY-NC 4.0 (non-commercial).** CSI/radar/LiDAR
  via Google Drive/Baidu; **raw RGB withheld on request** for privacy.
- **Person-in-WiFi-3D** — CSI paired with **camera-derived 3D keypoints**; >97k frames, 7
  volunteers; reported 3D joint error 91.7 mm (1 person), 108.1 mm (2), 125.3 mm (3). Code
  MIT/Apache-2.0; **dataset licence not stated (unverified)**.
- **Widar3.0** — gesture CSI/DFS/BVP, 22 gestures; **no camera pose, no vitals**.
- **MetaFi** (arXiv 2208.10414), **X-Fi** (arXiv 2410.10167) — WiFi→pose with camera supervision;
  **licences/access unverified**.
- **CSI-Bench** (NeurIPS 2025, arXiv 2505.21866) — 461 h, 26 rooms, 35 users; includes **breathing**,
  fall, localisation; **no camera pose**; repo MIT, **data licence unverified**.
- **Wi-Pose** — no peer-reviewed CSI+camera-pose dataset found by that name (could not verify).
- **Permissively licensed CSI+camera pose for commercial use: none found.**

### Pretrained weights (ruvnet, Hugging Face)

- `ruvnet/wifi-densepose-pretrained` — **MIT**; encoder of ~9,280 params trained on **one
  self-collected overnight capture** (no pose, no camera); held-out temporal-triplet accuracy
  82.33%. README retracts an earlier "100% presence" claim.
- `ruvnet/wifi-densepose-mmfi-pose` — **CC BY-NC 4.0**; 2,320,869 params; torso-PCK@20 = 82.686
  in-domain on MM-Fi `random_split`; self-reported cross-subject ~64%, cross-environment ~17.5%;
  **zero downloads recorded.**

## Realistic targets

Standard splits are random (S1), leave-one-subject-out (S2), leave-one-room-out (S3). Metrics:
MPJPE / PA-MPJPE, PCK@20. Reported WiFi torso-PCK@20: CSI2Pose 68.41, MultiFormer 72.25. So:
**~65–75% in-domain, ~60–65% cross-subject, ~15–20% zero-shot cross-room.** WiFi is the weakest
modality and degrades sharply cross-domain.

## Implication for Aquila

Aquila is non-commercial, so NC data/weights are usable, but shipped models should train on
permissively-licensed or own data, and NC datasets used only as an isolated research benchmark.
Aquila's v1 bars (in-domain torso-PCK@20 ≥ 65%, presence F1 ≥ 0.9, breathing MAE ≤ 2 brpm) sit at
the honest lower edge of the literature; cross-room is reported, never headlined.
