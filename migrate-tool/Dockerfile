# syntax=docker/dockerfile:1.7
#
# sentori-migrate — one-shot ETL binary (legacy prod DB → v0.2 DB).
# Standalone workspace; no core/ dependency.
#
# Build context = repo root:
#   docker build -f migrate-tool/Dockerfile -t sentori-migrate:dev .
#
# Run inside the compose network so both DBs resolve by service name:
#   docker run --rm --network sentori_default sentori-migrate:dev \
#     --src postgres://sentori:PW@sentori-postgres:5432/sentori \
#     --dst postgres://sentori:PW@postgres-v2:5432/sentori --dry-run

FROM rust:1.97-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src

COPY migrate-tool/ /src/migrate-tool/

WORKDIR /src/migrate-tool
RUN cargo build --release --bin sentori-migrate

FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

COPY --from=builder /src/migrate-tool/target/release/sentori-migrate /usr/local/bin/sentori-migrate

USER nonroot
ENTRYPOINT ["/usr/local/bin/sentori-migrate"]
