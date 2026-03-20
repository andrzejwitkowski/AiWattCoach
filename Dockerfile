FROM rust:1.88-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY tests ./tests

RUN cargo build --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system appuser \
    && useradd --system --gid appuser --create-home --home-dir /app appuser

WORKDIR /app

COPY --from=builder /app/target/release/aiwattcoach /usr/local/bin/aiwattcoach

USER appuser

ENV APP_NAME=AiWattCoach
ENV SERVER_HOST=0.0.0.0
ENV SERVER_PORT=3000
# Set MONGODB_URI and MONGODB_DATABASE at runtime for the target environment.
EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD curl --fail --silent http://127.0.0.1:${SERVER_PORT}/health || exit 1

CMD ["aiwattcoach"]
