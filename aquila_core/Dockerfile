# syntax=docker/dockerfile:1
#
# =============================================================================
# Aquila Core add-on image.
#
# UNVERIFIED IN THIS REPOSITORY. The Rust + WASM toolchain and Docker are absent
# from the environment in which this file was written, so no stage below has
# actually been built or run. The steps are the real commands, not `echo TODO`
# placeholders, but treat the image as unbuilt until a CI build (ticket #11)
# exercises it. Two things are known-incomplete and are marked inline:
#   1. the dashboard crate is a host-buildable stub, not yet a Trunk bundle
#      (ticket #10), so the dashboard stage emits a clearly-labelled placeholder
#      when there is no `index.html` to build; and
#   2. the `aquila-addon` composition root currently wires the simulator demo,
#      not the UDP ingest/control planes, so the EXPOSEd UDP ports are not yet
#      bound by the binary.
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
RUN rustup target add wasm32-unknown-unknown \
    && cargo install trunk wasm-bindgen-cli --locked
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
# The dashboard is a stub until ticket #10 lands; build it when a Trunk entry
# point exists, otherwise emit a labelled placeholder so the image still builds.
# `trunk build` writes the WASM bundle and JS glue to crates/aquila-dashboard/dist.
RUN if [ -f crates/aquila-dashboard/index.html ]; then \
        trunk build crates/aquila-dashboard/index.html --release; \
    else \
        mkdir -p crates/aquila-dashboard/dist && \
        printf '%s\n' \
          '<!doctype html><meta charset="utf-8">' \
          '<title>Aquila</title>' \
          '<p>Aquila dashboard is not built in this image (ticket #10).</p>' \
          > crates/aquila-dashboard/dist/index.html; \
    fi

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
