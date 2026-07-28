# ── 阶段 1：依赖缓存（cargo-chef）──────────────────────────────────────────
FROM rust:1.88-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
# Lindera 嵌入词典在 build.rs 里用 reqwest 拉，易因 HTTP/2 失败；
# 用 curl 预下载到 LINDERA_CACHE，跳过网络下载。
ARG LINDERA_IPADIC_URL=https://Lindera.dev/mecab-ipadic-2.7.0-20250920.tar.gz
ARG LINDERA_KODIC_URL=https://Lindera.dev/mecab-ko-dic-2.1.1-20180720.tar.gz
ENV LINDERA_CACHE=/app/.lindera-cache
RUN mkdir -p /app/.lindera-cache/1.5.1 \
 && curl --http1.1 -fsSL --retry 10 --retry-all-errors --retry-delay 3 \
      -o /app/.lindera-cache/1.5.1/mecab-ipadic-2.7.0-20250920.tar.gz \
      "$LINDERA_IPADIC_URL" \
 && curl --http1.1 -fsSL --retry 10 --retry-all-errors --retry-delay 3 \
      -o /app/.lindera-cache/1.5.1/mecab-ko-dic-2.1.1-20180720.tar.gz \
      "$LINDERA_KODIC_URL"

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY static ./static
RUN cargo build --release

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -ms /bin/sh tokenizer
WORKDIR /app

COPY --from=builder /app/target/release/tokenizer-service /usr/local/bin/tokenizer-service

RUN mkdir -p /data && chown tokenizer:tokenizer /data
VOLUME ["/data"]

USER tokenizer
EXPOSE 8080

ENV TOKENIZER_BIND="0.0.0.0:8080" \
    TOKENIZER_DB_PATH="/data/tokenizer.db"

HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

ENTRYPOINT ["/usr/local/bin/tokenizer-service"]
