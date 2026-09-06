# Multi-stage build: one image, two binaries selected via command:
FROM rust:1.83-bookworm AS builder
WORKDIR /app

# Pre-warm the dependency layer: copy manifests + a stub source tree so cargo
# resolves and compiles the dep graph without touching real code. Editing a .rs
# file later only invalidates the final compile step, not this layer.
COPY Cargo.toml Cargo.lock* ./
COPY build.rs ./
COPY proto ./proto
RUN mkdir -p src/bin \
 && echo "fn main() {}" > src/bin/math_server.rs \
 && echo "fn main() {}" > src/bin/gateway.rs \
 && touch src/lib.rs \
 && cargo build --release --bins \
 && rm -rf src

# Real build
COPY src ./src
RUN touch src/lib.rs src/bin/math_server.rs src/bin/gateway.rs \
 && cargo build --release --bins

# Runtime stage: slim Debian, non-root, both binaries on PATH
FROM debian:bookworm-slim AS runtime
RUN useradd --system --uid 1000 --create-home app
USER app
COPY --from=builder /app/target/release/math_server /usr/local/bin/math_server
COPY --from=builder /app/target/release/gateway /usr/local/bin/gateway

# No ENTRYPOINT: docker-compose / k8s picks the binary via `command:`.
