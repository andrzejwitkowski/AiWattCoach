FROM oven/bun:1 AS frontend-builder

WORKDIR /app/frontend

COPY frontend/package.json frontend/bun.lock ./
RUN bun install --frozen-lockfile

COPY frontend ./

RUN bun run build

FROM rust:1.88-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl wget \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system appuser \
    && useradd --system --gid appuser --create-home --home-dir /app appuser

WORKDIR /app

COPY --from=builder /app/target/release/aiwattcoach /usr/local/bin/aiwattcoach
COPY --from=frontend-builder /app/frontend/dist ./frontend/dist

ENV APP_NAME=AiWattCoach
ENV SERVER_HOST=0.0.0.0
ENV SERVER_PORT=3002
ENV RUST_BACKTRACE=1
ENV RUST_LOG=info,axum=info,tower_http=info
ENV OTEL_SERVICE_NAME=aiwattcoach-backend

USER appuser

# Set MONGODB_URI and MONGODB_DATABASE at runtime for the target environment.
EXPOSE 3002

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD wget --quiet --tries=1 --spider http://127.0.0.1:${SERVER_PORT}/health || exit 1

CMD ["aiwattcoach"]
