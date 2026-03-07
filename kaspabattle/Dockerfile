# ─── Stage 1: Install cargo-chef for dependency caching ───────────────────────
FROM rust:1.80-bookworm AS chef
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --locked
WORKDIR /app

# ─── Stage 2: Compute recipe (dependency fingerprint) ─────────────────────────
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ─── Stage 3: Build dependencies (cached layer) ───────────────────────────────
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

# Cache deps — only invalidated when Cargo.toml/Cargo.lock change
RUN cargo chef cook --release --recipe-path recipe.json -p battle-api

# Copy workspace source and build the final binary
COPY . .
RUN cargo build --release -p battle-api

# ─── Stage 4: Minimal runtime image ────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -m -s /sbin/nologin battle
WORKDIR /app

# Copy binary
COPY --from=builder --chown=battle:battle /app/target/release/battle-api ./battle-api

# Copy SQL migrations (run at startup)
COPY --chown=battle:battle migrations ./migrations

# Copy startup entrypoint
COPY --chown=battle:battle docker-entrypoint.sh ./docker-entrypoint.sh
RUN chmod +x ./docker-entrypoint.sh

USER battle
EXPOSE 8080

HEALTHCHECK --interval=15s --timeout=5s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:8080/api/v1/lobbies || exit 1

ENTRYPOINT ["./docker-entrypoint.sh"]
