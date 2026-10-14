# syntax=docker/dockerfile:1
#
# =============================================================================
# Aquila Core add-on image (home-assistant.md §3.1; repository-tooling.md §12).
#
# BUILD STATUS: **unbuilt in this repository's authoring environment** — no
# `docker`, `podman` or `buildah` is present on the machine where this file was
# written, so no stage below has been executed here. It is validated by
# inspection only; the release workflow (`.github/workflows/release.yml`) is what
# actually builds and pushes it. Treat the image as unbuilt until that workflow
# has run from a tag.
#
# KNOWN INCOMPLETE: `aquila-addon` (the composition root) currently wires the
# deterministic simulator demo and binds `127.0.0.1`, not the UDP CSI ingest
# (9100) and node-control (9101) planes or the browser-reachable interface. The
# EXPOSEd ports are therefore not yet bound by the binary. This is the real code
# at this commit, not a placeholder; wiring the live adapters is tracked work and
# changes no data-plane behaviour here.
#
# Shape:
#   Stage 1  build the dashboard crate (`aquila-dashboard`) to WASM with Trunk.
#   Stage 2  build the composition-root binary (`aquila-addon`) natively.
#   Stage 3  assemble the HA base image: the core binary + the dashboard bundle,
#            entrypoint the add-on, expose 8099 (HTTP+WS+ingress) and 9100/9101
#            (CSI ingest + node control, UDP).
#
# Reproducibility (repository-tooling.md §12): every base image is pinned by tag
# **and digest**; the Rust toolchain matches `rust-toolchain.toml`
# (1.98.1); the native build uses `cargo build --locked` and the dashboard build
# resolves from the committed `Cargo.lock`; Trunk and wasm-bindgen-cli are
# pinned. `BUILD_FROM` defaults to the amd64 HA base and is
# overridden per arch by `build.yaml` (local) and `release.yml` (GHCR, buildx).
# No `${HOME}` or secret is baked in.
#
# Crate references (see docs/spec/interface-register.md):
#   `aquila-addon`      composition root: wires adapters, serves the API.
#   `aquila-dashboard`  Rust -> WASM management UI.
#   `aquila-api`        private HTTP control plane on 8099.

ARG BUILD_FROM=ghcr.io/home-assistant/amd64-base-debian:bookworm@sha256:7da3f90a8e2fdc3095cc93b0dfb751c9c21356e11b30a6493c7ac15270b4e809
ARG AQUILA_VERSION=0.1.0
# The commit the image is built from; the core refuses to key its ledger to an
# unknown commit, so this is set at build time and surfaced as an env var.
ARG AQUILA_COMMIT=0000000000000000000000000000000000000000

# -----------------------------------------------------------------------------
# Stage 1 — dashboard (Rust -> WASM)
# -----------------------------------------------------------------------------
FROM rust:1.98.1-bookworm@sha256:93ce27a88655056a51dbdd8f5f2d7ddc071c7b0070fb288a37b5a285fc83971e AS dashboard-build
WORKDIR /src
# `gawk` backs the post-build CSP hook that externalises Trunk's inline
# bootstrap (crates/aquila-dashboard/hooks/externalise-inline-js.sh,
# `dashboard.md` §8). Trunk and wasm-bindgen-cli are pinned; the wasm-bindgen
# CLI version matches the `wasm-bindgen` crate in Cargo.lock (0.2.129).
RUN apt-get update \
    && apt-get install -y --no-install-recommends gawk \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add wasm32-unknown-unknown \
    && cargo install trunk --version 0.21.14 --locked \
    && cargo install wasm-bindgen-cli --version 0.2.129 --locked
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
# `trunk build` reads `crates/aquila-dashboard/Trunk.toml` and writes the WASM
# bundle, JS glue, CSS and `index.html` to `crates/aquila-dashboard/dist`
# (ticket #10, `dashboard.md` §2). Assets are emitted with relative URLs so they
# load under any HA ingress prefix.
RUN cd crates/aquila-dashboard && trunk build --release

# -----------------------------------------------------------------------------
# Stage 2 — core (Rust -> native)
# -----------------------------------------------------------------------------
FROM rust:1.98.1-bookworm@sha256:93ce27a88655056a51dbdd8f5f2d7ddc071c7b0070fb288a37b5a285fc83971e AS core-build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
# `--locked` fails the build rather than silently resolving a different
# dependency graph than the committed Cargo.lock.
RUN cargo build --release --locked -p aquila-addon

# -----------------------------------------------------------------------------
# Stage 3 — runtime image
# -----------------------------------------------------------------------------
FROM ${BUILD_FROM}
ARG AQUILA_VERSION
ARG AQUILA_COMMIT
LABEL org.opencontainers.image.title="Aquila Core" \
      org.opencontainers.image.version="${AQUILA_VERSION}" \
      org.opencontainers.image.revision="${AQUILA_COMMIT}" \
      org.opencontainers.image.source="https://github.com/belikh/aquila" \
      org.opencontainers.image.licenses="GPL-3.0-only"

COPY --from=core-build /src/target/release/aquila-addon /usr/local/bin/aquila-addon
COPY --from=dashboard-build /src/crates/aquila-dashboard/dist/ /usr/share/aquila/dashboard/

# 8099 HTTP+WS+ingress (TCP); 9100 CSI ingest and 9101 node control (UDP).
EXPOSE 8099
EXPOSE 9100/udp
EXPOSE 9101/udp

# `AQUILA_COMMIT` is required: the composition root refuses to key its ledger to
# an unknown commit. `RUST_LOG` and `AQUILA_DASHBOARD_DIR` are the intended
# runtime contract (the dashboard bundle is baked in here); the current
# composition root does not read them yet, and `/data` is the HA persistent
# volume it will use. Set as documentation of that contract, not as working
# configuration at this commit.
ENV AQUILA_COMMIT=${AQUILA_COMMIT} \
    AQUILA_DASHBOARD_DIR=/usr/share/aquila/dashboard \
    RUST_LOG=info

# The default run serves on 8099; HA Supervisor options are passed through the
# add-on environment.
ENTRYPOINT ["/usr/local/bin/aquila-addon"]
