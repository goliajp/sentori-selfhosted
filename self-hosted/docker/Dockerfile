# syntax=docker/dockerfile:1.7
#
# Sentori self-hosted server — multi-stage rust → distroless cc.
# Now bundles the compiled webapp SPA so a single image is the
# whole self-hosted stack (paired with postgres in compose).
#
# Build context = repo root. Image goal: < 100 MB
# (server bin + tiny static SPA).
#
# Usage:
#   docker build -f self-hosted/docker/Dockerfile -t sentori-selfhosted:dev .

# ── webapp builder ──────────────────────────────────────────
# Compiles the React/Vite SPA to /webapp/dist
FROM oven/bun:1-debian AS webapp-builder
WORKDIR /webapp
COPY webapp/package.json webapp/bun.lock /webapp/
RUN bun install --frozen-lockfile
COPY webapp/ /webapp/
RUN bun run build
# Expected output: /webapp/dist (Vite default)

# ── rust builder ────────────────────────────────────────────
FROM rust:1.97-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src

COPY core/ /src/core/
COPY self-hosted/ /src/self-hosted/

WORKDIR /src/self-hosted/server
RUN cargo build --release --bin sentori-server

# ── runtime: distroless cc (libc + libgcc only) ─────────────
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

COPY --from=builder /src/self-hosted/server/target/release/sentori-server /usr/local/bin/sentori-server
# Bundle the SPA. The server's fallback handler serves this
# directory; SENTORI_WEBAPP_DIST env-var can override.
COPY --from=webapp-builder /webapp/dist /app/webapp

EXPOSE 8080
USER nonroot
ENTRYPOINT ["/usr/local/bin/sentori-server"]
