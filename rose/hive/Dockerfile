# ── Stage 1: build a fully static Rust binary ──────────────────────
# rust:alpine targets *-linux-musl, which links statically. A static binary
# needs no libc and no dynamic loader at runtime, which is what lets the final
# image be `scratch`. Smaller image → faster pull → faster pod cold-start, and
# cold-start latency is exactly what bites when an autoscaler adds replicas
# during a traffic spike.
FROM rust:alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /app

# Pre-warm the dependency layer: manifests + a stub main so cargo compiles the
# dep graph without our source. Editing main.rs later only reruns the final,
# fast compile step, not the whole tree.
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

# Real build.
COPY src ./src
RUN touch src/main.rs && cargo build --release

# ── Stage 2: scratch — nothing but the binary ──────────────────────
# No OS, no shell, no package manager: the only attack surface is our binary.
# A numeric USER needs no /etc/passwd entry, so we run as non-root for free.
FROM scratch
COPY --from=build /app/target/release/hive /hive
EXPOSE 3000
USER 1000:1000
ENTRYPOINT ["/hive"]
