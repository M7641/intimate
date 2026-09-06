# ── Stage 1: build the frontend (Vite → static dist/) ──────────────
# Node is a BUILD-time tool only. The output is plain static files;
# nothing from this stage reaches the final image except dist/.
FROM node:22-alpine AS frontend
WORKDIR /app
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

# ── Stage 2: build a fully static Rust binary ──────────────────────
# rust:alpine's default target is *-linux-musl, which links statically
# (crt-static is on for musl). The result needs no libc and no dynamic
# loader at runtime — which is precisely what lets the final image be
# `scratch`. musl-dev provides the C headers a few crates compile against.
FROM rust:alpine AS backend
RUN apk add --no-cache musl-dev
WORKDIR /app
COPY backend/ ./
RUN cargo build --release

# ── Stage 3: scratch — nothing but the binary and the static files ─
# No OS, no shell, no package manager. The attack surface is the two
# files we copy in. A numeric USER needs no /etc/passwd entry.
FROM scratch
COPY --from=backend /app/target/release/nix-stack-backend /nix-stack-backend
COPY --from=frontend /app/dist /dist
ENV STATIC_DIR=/dist
EXPOSE 3000
USER 1000:1000
ENTRYPOINT ["/nix-stack-backend"]
