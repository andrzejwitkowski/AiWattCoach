# syntax=docker/dockerfile:1.7

ARG NODE_VERSION=22
ARG RUST_VERSION=1.88
ARG DEBIAN_CODENAME=bookworm

FROM node:${NODE_VERSION}-alpine AS frontend-deps

WORKDIR /app/frontend

COPY frontend/package.json frontend/package-lock.json ./

RUN --mount=type=cache,target=/root/.npm,id=aiwattcoach-npm-cache,sharing=locked \
    npm ci --cache /root/.npm --prefer-offline --no-audit

FROM frontend-deps AS frontend-builder

ARG VITE_API_BASE_URL=
ARG VITE_DEV_AUTH_ENABLED=false

ENV VITE_API_BASE_URL=${VITE_API_BASE_URL}
ENV VITE_DEV_AUTH_ENABLED=${VITE_DEV_AUTH_ENABLED}

COPY frontend/index.html ./
COPY frontend/tsconfig.json frontend/tsconfig.app.json frontend/tsconfig.node.json ./
COPY frontend/vite.config.ts ./
COPY frontend/public ./public
COPY frontend/src ./src

RUN npm run build

FROM rust:${RUST_VERSION}-slim-${DEBIAN_CODENAME} AS rust-base

ARG TARGETARCH

WORKDIR /app

ENV CARGO_HOME=/usr/local/cargo \
    CARGO_TARGET_DIR=/app/target \
    CARGO_NET_GIT_FETCH_WITH_CLI=true \
    CARGO_INCREMENTAL=0 \
    CARGO_PROFILE_RELEASE_STRIP=symbols \
    CC=clang \
    CXX=clang++ \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=clang \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=clang \
    RUSTFLAGS=-Clink-arg=-fuse-ld=lld

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt/lists,sharing=locked \
    apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates clang lld pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN --mount=type=cache,target=/usr/local/cargo/registry,id=aiwattcoach-cargo-registry-${TARGETARCH},sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,id=aiwattcoach-cargo-git-${TARGETARCH},sharing=locked \
    --mount=type=cache,target=/app/target,id=aiwattcoach-cargo-tools-${TARGETARCH},sharing=locked \
    cargo install cargo-chef --locked

FROM rust-base AS planner

COPY Cargo.toml Cargo.lock ./

RUN mkdir -p src && printf 'fn main() {}\n' > src/main.rs
RUN cargo chef prepare --recipe-path recipe.json

FROM rust-base AS chef

ARG TARGETARCH

COPY --from=planner /app/recipe.json recipe.json

RUN --mount=type=cache,target=/usr/local/cargo/registry,id=aiwattcoach-cargo-registry-${TARGETARCH},sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,id=aiwattcoach-cargo-git-${TARGETARCH},sharing=locked \
    --mount=type=cache,target=/app/target,id=aiwattcoach-cargo-target-${TARGETARCH},sharing=locked \
    cargo chef cook --release --locked --recipe-path recipe.json

FROM rust-base AS builder

ARG TARGETARCH

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN --mount=type=cache,target=/usr/local/cargo/registry,id=aiwattcoach-cargo-registry-${TARGETARCH},sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,id=aiwattcoach-cargo-git-${TARGETARCH},sharing=locked \
    --mount=type=cache,target=/app/target,id=aiwattcoach-cargo-target-${TARGETARCH},sharing=locked \
    cargo build --release --locked --bin aiwattcoach \
    && install -Dm755 /app/target/release/aiwattcoach /out/aiwattcoach

FROM debian:${DEBIAN_CODENAME}-slim AS runtime

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt/lists,sharing=locked \
    apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system appuser \
    && useradd --system --gid appuser --create-home --home-dir /app appuser

WORKDIR /app

COPY --from=builder /out/aiwattcoach /usr/local/bin/aiwattcoach
COPY --from=frontend-builder /app/frontend/dist ./frontend/dist

ENV APP_NAME=AiWattCoach \
    SERVER_HOST=0.0.0.0 \
    SERVER_PORT=3002 \
    RUST_BACKTRACE=1 \
    RUST_LOG=info,axum=info,tower_http=info \
    OTEL_SERVICE_NAME=aiwattcoach-backend

USER appuser

# Set MONGODB_URI and MONGODB_DATABASE at runtime for the target environment.
EXPOSE 3002

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD wget --quiet --tries=1 --spider http://127.0.0.1:${SERVER_PORT}/health || exit 1

CMD ["aiwattcoach"]
