# =============================================================================
# Build context = the Cargo WORKSPACE ROOT (cocoon/), NOT this app directory.
# The app crate inherits `[workspace.package]` / `[workspace.dependencies]` from
# the root Cargo.toml (`*.workspace = true`), so the whole workspace must be in
# the build context for `cargo build -p data_view` to find its workspace root.
# `data_view deploy` sets this up (artifact = workspace root, dockerfile_path =
# apps/data_view/Dockerfile). All paths below are workspace-relative.
# =============================================================================

# =============================================================================
# Stage 1 — Frontend builder (Bun + Vite)
# =============================================================================
FROM oven/bun:1 AS frontend-builder

WORKDIR /frontend

# Cache deps layer: only re-runs when lockfile changes
COPY apps/data_view/frontend/package.json apps/data_view/frontend/bun.lock ./
RUN bun install --frozen-lockfile

# Copy the rest of the frontend source and build
COPY apps/data_view/frontend/ ./
RUN bun run build


# =============================================================================
# Stage 2 — Rust builder (Cargo workspace)
# =============================================================================
FROM rust:bookworm AS rust-builder

# System deps for: DuckDB bundled (cmake, build-essential),
# PostgreSQL (libpq-dev), OpenSSL/native-tls (libssl-dev, pkg-config)
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    cmake \
    libpq-dev \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Workspace manifests + lockfile, then every member crate and app. The root
# Cargo.toml globs `crates/*` and `apps/*`, so all members must be present for
# the manifest to resolve — even the ones data_view doesn't depend on.
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY apps/ apps/

# Build only this app from the workspace.
RUN cargo build --release -p data_view


# =============================================================================
# Stage 3 — Runtime (minimal)
# =============================================================================
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    libpq5 \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Match existing convention: nimbus-user at UID 1467
RUN useradd -m -u 1467 -s /bin/sh nimbus-user

WORKDIR /app

# Copy the compiled binary (single shared workspace target/)
COPY --from=rust-builder /build/target/release/data_view ./data_view

# Copy the frontend bundle into the path expected by ServeDir::new("frontend/dist")
COPY --from=frontend-builder /frontend/dist ./frontend/dist/

# Set ownership
RUN chown -R nimbus-user:nimbus-user /app

USER nimbus-user

ENV RUST_ENV=production
ENV RUST_LOG=data_view=info,tower_http=info

EXPOSE 8050

ENTRYPOINT ["./data_view", "serve"]
