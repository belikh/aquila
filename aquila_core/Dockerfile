# syntax=docker/dockerfile:1
#
# =============================================================================
# Aquila Core add-on image.
#
# UNVERIFIED IN THIS REPOSITORY. The Rust + WASM toolchain and Docker are absent
# from the environment in which this file was written, so no stage below has
# actually been built or run. The steps are the real commands, not `echo TODO`
# placeholders, but treat the image as unbuilt until a CI build (ticket #11)
# exercises it. One thing is known-incomplete and is marked inline: the
# `aquila-addon` composition root currently wires the simulator demo, not the
# UDP ingest/control planes, so the EXPOSEd UDP ports are not yet bound by the
# binary.
#
# Shape (home-assistant.md §3.1):
#   Stage 1  build the dashboard crate (`aquila-dashboard`) to WASM with Trunk.
#   Stage 2  build the composition-root binary (`aquila-addon`) natively.
#   Stage 3  assemble the HA base image: the core binary + the dashboard bundle,
#            entrypoint the add-on, expose 8099 (HTTP+WS+ingress) and 9100/9101
#            (CSI ingest + node control, UDP).
#
# Crate references (see docs/spec/interface-register.md):
#   `aquila-addon`      composition root: wires adapters, serves the API.
#   `aquila-dashboard`  Rust -> WASM management UI.
#   `aquila-api`        private HTTP control plane on 8099.

ARG BUILD_FROM
ARG AQUILA_VERSION=0.1.0

# -----------------------------------------------------------------------------
# Stage 1 — dashboard (Rust -> WASM)
# -----------------------------------------------------------------------------
FROM rust:1.98-bookworm AS dashboard-build
WORKDIR /src
# `gawk` backs the post-build CSP hook that externalises Trunk's inline
# bootstrap (crates/aquila-dashboard/Trunk.toml, `dashboard.md` §8).
RUN apt-get update \
    && apt-get install -y --no-install-recommends gawk \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add wasm32-unknown-unknown \
    && cargo install trunk wasm-bindgen-cli --locked
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
FROM rust:1.98-bookworm AS core-build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
RUN cargo build --release -p aquila-addon

# -----------------------------------------------------------------------------
# Stage 3 — runtime image
# -----------------------------------------------------------------------------
FROM ${BUILD_FROM}
ARG AQUILA_VERSION
LABEL org.opencontainers.image.title="Aquila Core" \
      org.opencontainers.image.version="${AQUILA_VERSION}" \
      org.opencontainers.image.licenses="GPL-3.0-only"

COPY --from=core-build /src/target/release/aquila-addon /usr/local/bin/aquila-addon
COPY --from=dashboard-build /src/crates/aquila-dashboard/dist/ /usr/share/aquila/dashboard/

# 8099 HTTP+WS+ingress (TCP); 9100 CSI ingest and 9101 node control (UDP).
EXPOSE 8099
EXPOSE 9100/udp
EXPOSE 9101/udp

# Read options from /data/options.json (Supervisor) and serve on 8099. The
# binary defaults to port 8099 and reads its data directory from the add-on
# environment.
ENV AQUILA_DASHBOARD_DIR=/usr/share/aquila/dashboard \
    RUST_LOG=info

ENTRYPOINT ["/usr/local/bin/aquila-addon"]
