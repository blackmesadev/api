# syntax=docker/dockerfile:1.7

FROM rust:1.94-slim-trixie AS builder

WORKDIR /app/api

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# COPY ./lib /app/lib

COPY ./Cargo.toml ./Cargo.lock ./
COPY ./src/main.rs ./src/main.rs

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo fetch

COPY ./src ./src

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/cargo-target \
    CARGO_TARGET_DIR=/cargo-target cargo build --release --bin mesa-api \
    && cp /cargo-target/release/mesa-api /usr/local/bin/mesa-api

FROM debian:trixie-slim AS runtime

WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3t64 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/bin/mesa-api /usr/local/bin/mesa-api

RUN useradd --system --uid 10001 --create-home mesaapi
USER mesaapi

EXPOSE 8080
CMD ["/usr/local/bin/mesa-api"]
