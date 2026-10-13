# Research — plugin / extension architectures

**Status:** research. Decision taken: **Aquila ships no plugin surface; all extensions are in-tree.**
Recorded here because the question will recur and this is the considered background.
**Last updated:** 2026-10-13. **Sources:** see `../reference/sources.json`.

## Question

If Aquila ever exposes an extension mechanism for the sensing pipeline, what is the lowest-regret choice?

## Findings

- **In-process WASM** — `wasmtime` (Apache-2.0 WITH LLVM-exception) is the reference Rust host:
  Cranelift, fuel metering, epoch interruption, `StoreLimits`, and WASI Preview 2. `wasmer` (MIT)
  offers pluggable engines; `wasmi` is a pure-Rust interpreter (deterministic, no JIT). The WASM
  **Component Model + WIT worlds** makes capability scoping *structural*: a component receives only
  the interfaces you link. Real precedents: Envoy proxy-wasm, Shopify Functions.
- **Native dynamic plugins** — `abi_stable` (MIT/Apache-2.0) gives Rust-to-Rust FFI with load-time
  layout checking but "assumes a benign source", cannot unload, and does not sandbox; `stabby`,
  `libloading` are leaner/rawer. Only for fully-trusted Rust.
- **Out-of-process** — gRPC / Unix sockets / MQTT isolate fully but add marshalling; used by
  Frigate (separate process + MQTT), ESPHome/Rhasspy (services/containers).
- **Extism** — **BSD-3-Clause** (not MIT), ~5.8k stars, wraps wasmtime; language-agnostic PDKs with
  host-controlled HTTP/vars/limits. Good ergonomics, less runtime control, one more dependency layer.
- **Home Assistant** — custom components are in-process Python (full trust); add-ons are separate
  Docker containers over Supervisor. A CSI-pipeline plugin cannot live inside HA; it lives in the
  core and surfaces state to HA.

## Implication for Aquila

**v1: no plugin surface.** The brief settled on *in-tree only, no plugins ever*; this research
exists so the decision is documented, not accidental. If ever revisited, the recommendation is
in-process WASM on the Component Model (`wasmtime`) with WIT capability worlds and no ambient WASI,
batching at the ABI boundary; never native dylibs for third-party code.
