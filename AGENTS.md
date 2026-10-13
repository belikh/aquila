# Aquila

Aquila — a sixth sense for invisible fields. WiFi CSI sensing, rebuilt as a
tidy, measurable, test-driven pipeline.

Prose (docs, comments, issues, commit messages) uses Australian English spelling.

## Build and shells

There is no `cargo` on the bare PATH. Always work inside a dev shell.

- `nix develop` (or `nix-shell`) — Rust 1.98.1 **with the `wasm32-unknown-unknown`
  target**, plus `trunk`, `wasm-bindgen-cli`, `binaryen`, `wasm-tools`, `gcc`,
  `make`, `pkg-config`.
- `nix develop .#ha` — Python 3 with `aiohttp`, `voluptuous`, `pytest`,
  `pytest-asyncio`, `pyyaml`, and Home Assistant, for the HACS component.
- Test everything: `nix develop --command cargo test --workspace`.
- Build the dashboard: `nix run .#dashboard`
  (equivalently `nix develop --command bash -c 'cd crates/aquila-dashboard && trunk build --release'`).
- CI and scripts must use the same shells; do not assume a global toolchain.

## Documentation layout

All reference material lives under `docs/`. Read `docs/README.md` for the full map.

- `CONTEXT.md` — the domain glossary. Read first; use its vocabulary exactly.
- `docs/spec/` — the specification. Index at `docs/spec/README.md`.
- `docs/adr/` — decisions; immutable once accepted.
- `docs/research/` — investigations. Advisory and out-of-band; they never define scope.
- `docs/reference/` — external resources (datasets, papers, tools, licences). `sources.json` is machine-readable.
- `docs/plan/` — the wayfinder map and tickets (ticket number == GitHub issue number).
- `docs/agents/` — skill configuration and the **orchestration policy** (`orchestration.md`).

Every spec claim cites `docs/reference/`; research never overrides an ADR — surface the conflict.

## Agent skills

### Issue tracker

Issues and specs live as GitHub issues on `belikh/aquila`. See `docs/agents/issue-tracker.md`.

### Triage labels

The five canonical triage roles use their default label strings. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `CONTEXT.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
