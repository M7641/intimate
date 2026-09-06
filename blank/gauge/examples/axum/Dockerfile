# Multi-stage build for the Axum example: compile with the Rust toolchain, then
# copy just the binary into a slim runtime. So `gauge run` can measure it with
# exact cgroup accounting.
FROM rust:1-slim AS build
WORKDIR /src
COPY Cargo.toml ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=build /src/target/release/gauge-axum-example /usr/local/bin/app
# Bind all interfaces so a published port reaches the app.
ENV GAUGE_HOST=0.0.0.0
ENV PORT=8080
EXPOSE 8080
CMD ["/usr/local/bin/app"]
