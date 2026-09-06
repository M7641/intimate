# syntax=docker/dockerfile:1.7
#
# Multi-stage build : on compile dans une image rust complète, on copie le binaire
# dans une image Lambda minimale. Le runtime final ne contient ni cargo, ni rustc,
# ni les sources — typiquement ~80 MB compressé.

# ─── stage 1 : compilation ────────────────────────────────────────────────────
FROM --platform=linux/arm64 rust:1.83-bookworm AS builder
WORKDIR /build

# Cache des dépendances : on copie d'abord Cargo.toml seul, on builde un dummy
# pour figer le lockfile + télécharger les crates, puis on remplace par les vrais
# sources. Toute modification de src/ ne réinvalide pas la couche des deps.
COPY Cargo.toml ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src target/release/perch* target/release/deps/perch*

COPY src ./src
RUN cargo build --release

# ─── stage 2 : image Lambda finale ────────────────────────────────────────────
# `provided:al2023-arm64` est le runtime "custom" — Lambda y cherche /var/runtime/bootstrap.
# Lambda Web Adapter (extension) intercepte les invocations et les proxie vers
# le HTTP server local que bootstrap démarre.
FROM --platform=linux/arm64 public.ecr.aws/lambda/provided:al2023-arm64

# L'adapter est livré en tant qu'image scratch : on en copie juste le binaire.
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:0.9.0 \
     /lambda-adapter /opt/extensions/lambda-adapter

COPY --from=builder /build/target/release/perch /var/runtime/bootstrap

ENV PORT=8080 \
    RUST_LOG=info,tower_http=info \
    READINESS_CHECK_PATH=/healthz \
    AWS_LWA_INVOKE_MODE=buffered

# Pas de CMD : Lambda exécute /var/runtime/bootstrap par convention.
