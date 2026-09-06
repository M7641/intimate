# Containerization

> Status: **Reference** — guidelines for building and deploying the container image

## Dockerfile Pattern

```dockerfile
# ── Build stage ───────────────────────────────────────────────────
FROM rust:1.82-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin dv

# ── Runtime stage ─────────────────────────────────────────────────
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/dv /usr/local/bin/dv
COPY --from=builder /app/frontend/dist /app/frontend/dist

ENV RUST_ENV=production
EXPOSE 8050
ENTRYPOINT ["dv", "start"]
```

## Key Decisions

| Decision               | Rationale                                                                    |
| ---------------------- | ---------------------------------------------------------------------------- |
| Multi-stage build      | Keeps the runtime image small (~80 MB vs ~2 GB with the full Rust toolchain) |
| `debian:bookworm-slim` | Includes glibc (required by most Rust binaries) without unnecessary packages |
| `ca-certificates`      | Required for TLS connections to cloud data warehouses                        |
| `RUST_ENV=production`  | Enables JSON structured logging by default                                   |

## Health Probes (Kubernetes)

```yaml
livenessProbe:
  httpGet:
    path: /health/live
    port: 8050
  initialDelaySeconds: 5
  periodSeconds: 10

readinessProbe:
  httpGet:
    path: /health/ready
    port: 8050
  initialDelaySeconds: 10
  periodSeconds: 5
  failureThreshold: 3
```

## Environment Variables

All configuration is via environment variables. See `.env.example` for the full list. In Kubernetes, inject via ConfigMap (non-sensitive) and Secret (credentials).

## Building with OpenTelemetry

```dockerfile
RUN cargo build --release --bin dv --features otel
```
