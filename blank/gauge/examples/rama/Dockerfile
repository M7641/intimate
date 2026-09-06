# Multi-stage build for the Rama example. Rama's `http-full` feature can pull in
# BoringSSL, which needs a C/C++ toolchain (cmake, perl, clang, go) to compile —
# so the builder stage installs those. The runtime stage stays slim.
FROM rust:1-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential cmake perl clang golang pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=build /src/target/release/gauge-rama-example /usr/local/bin/app
# Bind all interfaces so a published port reaches the app.
ENV GAUGE_HOST=0.0.0.0
ENV PORT=8080
EXPOSE 8080
CMD ["/usr/local/bin/app"]
