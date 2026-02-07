# Stage 1: Build
FROM rust:1.83-slim-bookworm AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY backend/ ./

# Build dependencies first (cacheable layer)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release && \
    cp target/release/kanban /usr/local/bin/kanban

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

RUN useradd -m -s /bin/bash appuser
WORKDIR /app

COPY --from=builder /usr/local/bin/kanban /app/kanban

# Data directory for SQLite
RUN mkdir -p /app/data && chown appuser:appuser /app/data
VOLUME /app/data

ENV DATABASE_PATH=/app/data/kanban.db
ENV ROCKET_ADDRESS=0.0.0.0
ENV ROCKET_PORT=8000

USER appuser
EXPOSE 8000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -sf http://localhost:8000/api/v1/health || exit 1

CMD ["./kanban"]
