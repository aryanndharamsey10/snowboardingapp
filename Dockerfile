FROM rust:1-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY static/catalog.json ./static/catalog.json
RUN cargo build --locked --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates gosu && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/weekend-club /usr/local/bin/weekend-club
COPY static ./static
COPY scripts/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN groupadd --system --gid 10001 appuser && useradd --system --uid 10001 --gid appuser appuser && mkdir data && chown appuser:appuser data
ENV HOST=0.0.0.0 PORT=3000 DATABASE_PATH=/app/data/weekend-club.sqlite
EXPOSE 3000
VOLUME ["/app/data"]
ENTRYPOINT ["/bin/sh", "/usr/local/bin/docker-entrypoint.sh"]
CMD ["weekend-club"]
