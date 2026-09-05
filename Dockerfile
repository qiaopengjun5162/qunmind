# syntax=docker/dockerfile:1

ARG RUST_VERSION=1

FROM rust:${RUST_VERSION}-bookworm AS builder

WORKDIR /workspace

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY docs/assets/wechat ./docs/assets/wechat
COPY src ./src

RUN cargo build --locked --release --bin qunmind

FROM debian:bookworm-slim AS runtime

LABEL org.opencontainers.image.title="QunMind"
LABEL org.opencontainers.image.description="Rust WeChat group AI mind backend"
LABEL org.opencontainers.image.source="https://github.com/qiaopengjun5162/qunmind"
LABEL org.opencontainers.image.licenses="MIT"

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --uid 10001 qunmind

WORKDIR /app

COPY --from=builder /workspace/target/release/qunmind /usr/local/bin/qunmind
COPY config.docker.example.toml /app/config.example.toml

USER qunmind

ENV RUST_LOG=info

ENTRYPOINT ["/usr/local/bin/qunmind"]
CMD ["--config", "/app/config.toml"]
