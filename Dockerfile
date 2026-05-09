FROM rust:1.89-bookworm AS base
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN rustup target add wasm32-unknown-unknown && \
    cargo install cargo-chef --locked && \
    cargo install dioxus-cli --version "^0.7" --locked

FROM base AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM base AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json && \
    cargo chef cook --release --target wasm32-unknown-unknown --recipe-path recipe.json
COPY . .
RUN dx build --release

FROM nvcr.io/nvidia/cuda:11.7.0-base-ubuntu22.04 AS runtime
WORKDIR /app
ENV IP=0.0.0.0 \
    PORT=8080
COPY --from=builder /app/target/dx/tensorrt-converter/release/web/server ./server
COPY --from=builder /app/target/dx/tensorrt-converter/release/web/public/ ./public/
EXPOSE 8080
ENTRYPOINT ["./server"]