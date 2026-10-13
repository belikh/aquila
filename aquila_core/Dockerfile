# syntax=docker/dockerfile:1
#
# =============================================================================
# PLACEHOLDER — NOT A VERIFIED BUILD.
# =============================================================================
#
# This Dockerfile is a scaffold for the Aquila Core add-on. It has NOT been
# built or run: the Rust + ONNX toolchain is absent from the environment in
# which this file was written, so no stage below has been exercised. Every
# build step is marked TODO with the intended command. Do not treat this as a
# working multi-stage build.
#
# Intended shape (home-assistant.md §3.1):
#   Stage 1  build the dashboard crate (`aquila-dashboard`) to WASM.
#   Stage 2  build the composition-root binary (`aquila-addon`) natively.
#   Stage 3  assemble the HA base image: the core binary, the WASM dashboard
#            bundle, and the bundled firmware image store under /data.
#
# Crate references (see docs/spec/interface-register.md):
#   - `aquila-addon`      composition root: wires adapters, serves the
#                         dashboard and ingress; depends on `aquila-api`.
#   - `aquila-dashboard`  Rust -> WASM management UI.
#   - `aquila-api`        private HTTP+WS control plane on :8099.
#   - `aquila-sensing`    the stateful core (pulled in transitively).
#
# The dashboard crate, `aquila-api` and `aquila-addon` are named in the
# interface register but are not yet present in the workspace's Cargo.toml
# members list; they must exist before this build can be real.

ARG BUILD_FROM
ARG AQUILA_VERSION=0.1.0

# -----------------------------------------------------------------------------
# Stage 1 — dashboard (Rust -> WASM)
# -----------------------------------------------------------------------------
FROM rust:1.98-bookworm AS dashboard-build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
# TODO(unverified): add the wasm target and build with trunk/wasm-pack.
#   RUN rustup target add wasm32-unknown-unknown
#   RUN cargo install trunk wasm-bindgen-cli --locked
#   RUN trunk build crates/aquila-dashboard/index.html --release
#       -> produces aquila_dashboard_bg.wasm + JS glue. The WASM MIME type on
#          the serving side MUST be application/wasm (home-assistant.md §3.1).
RUN echo "TODO: build crates/aquila-dashboard to WASM (not implemented)"

# -----------------------------------------------------------------------------
# Stage 2 — core (Rust -> native)
# -----------------------------------------------------------------------------
FROM rust:1.98-bookworm AS core-build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
# TODO(unverified): build the composition root. The ONNX runtime and the
#   sensing pipeline are memory-hungry; only amd64 and aarch64 are supported.
#   RUN apt-get update && apt-get install -y --no-install-recommends \
#         pkg-config libssl-dev
#   RUN cargo build --release -p aquila-addon
RUN echo "TODO: build crates/aquila-addon natively (not implemented)"

# -----------------------------------------------------------------------------
# Stage 3 — runtime image
# -----------------------------------------------------------------------------
FROM ${BUILD_FROM}
ARG AQUILA_VERSION

# TODO(unverified): copy the built artefacts from the previous stages.
#   COPY --from=core-build /src/target/release/aquila-addon /usr/bin/aquila-addon
#   COPY --from=dashboard-build /src/crates/aquila-dashboard/dist/ \
#        /usr/share/aquila/dashboard/

# TODO(unverified): bundle signed firmware images for the OTA store. Images are
#   Ed25519-signed and verified by the core before an offer (firmware-ota.md §4).
#   COPY firmware/ /usr/share/aquila/firmware/

# TODO(unverified): the add-on entrypoint. It must:
#   - read /data/options.json (Supervisor injects options),
#   - bind UDP 9100 (CSI ingest) and 9101 (node control),
#   - serve HTTP+WS+ingress on 8099 with the WASM dashboard,
#   - initialise /data (ledger, calibration store, drift state, model cache).
#   ENTRYPOINT ["/usr/bin/aquila-addon"]

# Intentionally left with no ENTRYPOINT: this placeholder is not runnable.
