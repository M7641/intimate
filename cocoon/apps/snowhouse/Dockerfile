# =============================================================================
# Build context = the Cargo WORKSPACE ROOT (cocoon/), NOT this app directory.
# The app crate inherits `[workspace.package]` / `[workspace.dependencies]` from
# the root Cargo.toml (`*.workspace = true`), so the whole workspace must be in
# the build context for `cargo build -p snowhouse` to find its workspace root.
# `snowhouse deploy` sets this up (artifact = workspace root, dockerfile_path =
# apps/snowhouse/Dockerfile). All paths below are workspace-relative.
# =============================================================================

# =============================================================================
# Stage 1 — Frontend builder (Bun + Vite)
# =============================================================================
FROM oven/bun:1 AS frontend-builder

WORKDIR /frontend

# Cache deps layer: only re-runs when lockfile changes
COPY apps/snowhouse/frontend/package.json apps/snowhouse/frontend/bun.lock ./
RUN bun install --frozen-lockfile

# Copy the rest of the frontend source and build
COPY apps/snowhouse/frontend/ ./
RUN bun run build


# =============================================================================
# Stage 2 — Rust builder (Cargo workspace)
# =============================================================================
FROM rust:bookworm AS rust-builder

# System deps for: cmake + build-essential (compile aws-lc-sys, the crypto
# backend behind rustls/reqwest), OpenSSL/native-tls (libssl-dev, pkg-config).
# No libpq — the Snowflake connector is REST and rust-postgres is pure Rust.
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    cmake \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Workspace manifests + lockfile, then every member crate and app. The root
# Cargo.toml globs `crates/*` and `apps/*`, so all members must be present for
# the manifest to resolve — even the ones snowhouse doesn't depend on.
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY apps/ apps/

# Build only this app from the workspace.
RUN cargo build --release -p snowhouse


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
COPY --from=rust-builder /build/target/release/snowhouse ./snowhouse

# Copy the frontend bundle into the path expected by ServeDir::new("frontend/dist")
COPY --from=frontend-builder /frontend/dist ./frontend/dist/

# Set ownership
RUN chown -R nimbus-user:nimbus-user /app

USER nimbus-user

ENV RUST_ENV=production
ENV RUST_LOG=snowhouse=info,tower_http=info
# Tag every Snowflake query with this app as the source, for cost attribution
# in QUERY_HISTORY.QUERY_TAG.
ENV SNOWFLAKE_QUERY_TAG_SOURCE=snowhouse_application

# Snowflake auth. In the default OAuth mode the connector needs only API_KEY at
# runtime — it fetches account/token/warehouse/database/schema from the Nimbus
# connections API. Nimbus injects API_KEY as a build arg at deploy time; we promote
# it to a runtime env var.
# NOTE: this bakes API_KEY into an image layer — acceptable only because the
# image stays private to the Nimbus registry.
ARG API_KEY
ENV API_KEY=$API_KEY

EXPOSE 8050

ENTRYPOINT ["./snowhouse", "serve"]
