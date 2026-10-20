# v2 experiment environment

Status: provisioned for the invention rounds. Companion to `BRIEF.md`,
`ROUND2-BRIEF.md` and the round-1 notes. Australian English.

## The Python environment: `sim-python`

Use the wrapper at the worktree root. It sets the two nix runtime libraries the
virtualenv needs (`libstdc++` for numpy's C extension, `libz` for zlib-ng) and
runs the venv's Python 3.12:

```bash
/home/matt/projects/aquila-v2-spec/sim-python -c "import numpy, scipy, sympy, skrf; print('ok')"
/home/matt/projects/aquila-v2-spec/sim-python your_script.py
```

**Do not call `.venv/bin/python` directly** — it fails to import numpy
(`libstdc++.so.6: cannot open shared object file`).

**Installed:** `numpy`, `scipy`, `sympy`, `pandas`, `matplotlib`, `scikit-learn`,
`scikit-rf`, `h5py`, `zarr`, `networkx`, `trimesh`, `pint`, `uncertainties`,
`rich`, `tqdm`, `joblib`. Everything is a PyPI wheel on Python 3.12.

## Why not the Nix shell for Python

This flake's `nixpkgs` is a **`path` flake input**, so the upstream binary cache
does not apply: every Python package compiles from source, and several nixpkgs
test-suites fail in this environment (e.g. `anyio`'s TLS tests), which cascades
and blocks the whole closure. Attempting it cost ~15 minutes and ~7 GB before
failing. The pip virtualenv is therefore the supported path.

`nix develop .#sim` still exists and is **tooling only** — it provides the Rust
toolchain, `jq`, `unzip`, `gnuplot`, `graphviz` and `make`, and is fast to enter.

## What to use, by task

| Task | Tool |
| --- | --- |
| Link budgets, CRBs, estimator error | `sympy` + `numpy` in `.venv` |
| Symbolic derivations, closed-form bounds, units, entity lookups | **Wolfram MCP tools** (`WolframLanguageEvaluator`, `WolframContext`, `WolframAlpha`) |
| Array factors, beam patterns, near-field curvature | `numpy` + `matplotlib` |
| S-parameters, networks | `scikit-rf` |
| Filters, spectrograms, CFO estimators | `scipy.signal` |
| Clustering, PCA, regression | `scikit-learn` |
| Geometry | `trimesh`, `networkx` |
| Deterministic CSI with ground truth | the repo's own simulator: `cargo run -p aquila-capture -- synth` (needs `.#sim` for cargo) |
| Figures | `matplotlib` (preferred) or `gnuplot` from `.#sim` |

## Not provisioned

GNU Radio, openems (FDTD), octave, ngspice and PyTorch are **not** available:
each would build from source here (GNU Radio pulls OpenCV) for tens of minutes
and several GB, and the host disk is tight. If a claim needs FDTD or an SDR
flowgraph, say so explicitly and compute an analytic bound instead — do not
pretend a simulation was run.

## Honesty rules for anything produced here

- A simulation is not a measurement. Label it accordingly; a NumPy model is
  `ESTABLISHED` only for the mathematics it evaluates, not for the device.
- Report the parameters used (frequency, bandwidth, geometry, SNR), and keep
  scripts reproducible so another agent can rerun them.
- If a claim cannot be checked with what is here, say so — do not dress a guess
  as a computation.
- **Disk discipline:** keep artefacts small (scalars, small CSVs, figures). The
  host filled its filesystem once already, which silently broke writes; it has
  ~6 GB free and no more to spare.
