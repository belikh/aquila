# Aquila

Aquila is a sixth sense for invisible fields: WiFi CSI sensing rebuilt as a
tidy, measurable, test-driven pipeline. ESP32-S3 nodes stream raw channel-state
information to a Rust core that runs as a Home Assistant add-on; a HACS custom
component projects the results — presence, occupancy, person count and vitals —
into Home Assistant entities.

The project's premise is that a home sensor is worth trusting only if every
number it states is **measured, reproducible and honestly graded**. Every claim
carries one of five evidence grades; a headline claim may be **Gated** only. See
the claims table in [`docs/README.md`](docs/README.md) and
[`claims.toml`](claims.toml) (a machine-checked bijection).

## Getting started

- **[Install and upgrade](docs/INSTALL.md)** — add-on, HACS component, node
  provisioning, first calibration, OTA firmware updates.
- **[Documentation index](docs/README.md)** — specs, ADRs, research and the
  claims table.
- **[Glossary](CONTEXT.md)** — the canonical vocabulary used throughout.
- **[Roadmap](docs/spec/roadmap.md)** — the delivery slices and what "v1
  shipped" means.

## What is here

- `crates/` — the Rust workspace: the sensing core, the private control-plane
  API and the WASM management dashboard (one crate graph, DAG-checked).
- `firmware/` — the ESP32-S3 CSI node firmware (ESP-IDF, C), including the OTA
  agent.
- `custom_components/aquila/` — the HACS custom component (Python).
- `aquila_core/` — the Home Assistant add-on packaging (Dockerfile, config).
- `docs/` — the specification, ADRs, plan and research notes.

## v1 status

This repository is **not yet "v1 shipped"**. The roadmap defines v1 as passing
the three accuracy bars *in-domain on a reference deployment* (Linux host, Home
Assistant OS, ~9+ ESP32-S3 nodes over three floors), with those results
evidenced by commit-keyed ledger rows (`docs/spec/roadmap.md`, "What v1 shipped
means"). Those bars are hardware-gated and are still `target` in `claims.toml`;
nothing here presents them as achieved.

**Verifiably done in this repository:**

- **Workspace** — the Rust workspace builds and tests green
  (`cargo test --workspace`), and `cargo xtask dag-check` enforces the crate
  allow-list.
- **Dashboard** — the Leptos management dashboard compiles to WASM with Trunk.
- **Firmware** — the ESP32-S3 firmware compiles with ESP-IDF v5.5.2
  (`nix develop .#esp`), including the A/B OTA agent.
- **OTA path** — the core OTA control channel and signed image store, the
  firmware OTA agent, and the Home Assistant `update` entity are implemented and
  unit-tested.
- **HA component** — the HACS integration's Python tests run under
  `nix develop .#ha`.
- **Specs and ADRs** — the specification set and ADRs 0001–0006 are settled for
  v1.
- **Ledger** — the commit-keyed measurement ledger and the honest-claims
  bijection (`cargo xtask claims-verify`) are in place.

**Remaining, and hardware-gated (cannot be verified in this repository):**

- **Accuracy bars** — in-domain torso PCK@20, presence F1 and breathing MAE must
  be measured on real nodes and recorded as ledger rows.
- **Flashing and on-device behaviour** — the firmware is compile-verified only;
  it has not been flashed, observed streaming, or run an OTA.
- **Add-on image build** — `aquila_core/Dockerfile` is validated by inspection
  only, because no container engine (`docker`/`podman`/`buildah`) is available in
  the authoring environment; the image is built by the release workflow.
- **HA-instance tests** — the component is tested without a running Home
  Assistant; a clean-install gate on a real HA OS host is outstanding.
- **Release** — the release automation exists but no `v1.0.0` tag or release has
  been created; that waits on the gates above.

## Licence

GPL-3.0-only. The project is non-commercial; non-commercial datasets and weights
are usable but must never be redistributed as if permissive.
