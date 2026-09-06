# =============================================================================
# Build context = the Cargo WORKSPACE ROOT (cocoon/), NOT this app directory.
# The app crate inherits `[workspace.package]` / `[workspace.dependencies]` from
# the root Cargo.toml (`*.workspace = true`), so the whole workspace must be in
# the build context for `cargo build -p redhouse` to find its workspace root.
# `redhouse deploy` sets this up (artifact = workspace root, dockerfile_path =
# apps/redhouse/Dockerfile). All paths below are workspace-relative.
# =============================================================================

# =============================================================================
# Stage 1 — Frontend builder (Bun + Vite)
# =============================================================================
FROM oven/bun:1 AS frontend-builder

WORKDIR /frontend

# Cache deps layer: only re-runs when lockfile changes
COPY apps/redhouse/frontend/package.json apps/redhouse/frontend/bun.lock ./
RUN bun install --frozen-lockfile

# Copy the rest of the frontend source and build
COPY apps/redhouse/frontend/ ./
RUN bun run build


# =============================================================================
# Stage 2 — Rust builder (Cargo workspace)
# =============================================================================
FROM rust:bookworm AS rust-builder

# System deps for: cmake + build-essential (compile aws-lc-sys, the crypto
# backend behind rustls/reqwest), OpenSSL/native-tls (libssl-dev, pkg-config).
# (rust-postgres is pure Rust, so no libpq is needed for the Redshift connector.)
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    cmake \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Workspace manifests + lockfile, then every member crate and app. The root
# Cargo.toml globs `crates/*` and `apps/*`, so all members must be present for
# the manifest to resolve — even the ones redhouse doesn't depend on.
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY apps/ apps/

# Build only this app from the workspace.
RUN cargo build --release -p redhouse


# =============================================================================
# Stage 3 — Runtime (minimal)
# =============================================================================
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Match existing convention: nimbus-user at UID 1467
RUN useradd -m -u 1467 -s /bin/sh nimbus-user

WORKDIR /app

# Copy the compiled binary (single shared workspace target/)
COPY --from=rust-builder /build/target/release/redhouse ./redhouse

# Copy the frontend bundle into the path expected by ServeDir::new("frontend/dist")
COPY --from=frontend-builder /frontend/dist ./frontend/dist/

# Set ownership
RUN chown -R nimbus-user:nimbus-user /app

USER nimbus-user

ENV RUST_ENV=production
ENV RUST_LOG=redhouse=info,tower_http=info

# Redshift username/password. Nimbus injects these as build args at deploy time;
# we promote them to runtime env vars for the connector (postgres.rs reads
# REDSHIFT_USERNAME / REDSHIFT_PASSWORD). Host / port / database come from the
# webapp's runtime config.
# NOTE: this bakes the credentials into an image layer — acceptable only because
# the image stays private to the Nimbus registry.
ARG REDSHIFT_USERNAME
ARG REDSHIFT_PASSWORD
ENV REDSHIFT_USERNAME=$REDSHIFT_USERNAME \
    REDSHIFT_PASSWORD=$REDSHIFT_PASSWORD

EXPOSE 8050

ENTRYPOINT ["./redhouse", "serve"]
