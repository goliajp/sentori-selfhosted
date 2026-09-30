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
# The registry and the target directory sit in BuildKit cache mounts
# rather than in a layer. Any source byte invalidates the COPY above, so
# without them every build downloads and recompiles all 387 dependencies
# — 303 s on the deploy host for a release whose own crates are a small
# part of that. A cache mount belongs to the builder, so this pays off
# where the builder persists (the deploy host, a developer's machine) and
# changes nothing on a throwaway runner. `sharing=locked` because cargo
# takes its own lock on target/, and two builds sharing the mount would
# otherwise queue inside cargo with nothing to show for it. The binary is
# copied out in the same RUN: nothing under a cache mount reaches the
# image.
RUN --mount=type=cache,id=sentori-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=sentori-server-target,target=/src/self-hosted/server/target,sharing=locked \
    cargo build --release --bin sentori-server \
    && cp target/release/sentori-server /usr/local/bin/sentori-server
# Skeleton for the /data volume: distroless has no shell, so the
# ownership must be baked in — a named volume adopts the image
# path's owner on first creation, and root-owned /data would make
# the nonroot server fail its blob-store init.
RUN mkdir -p /data-skel/blobs

# ── runtime: distroless cc (libc + libgcc only) ─────────────
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

COPY --from=builder /usr/local/bin/sentori-server /usr/local/bin/sentori-server
# Bundle the SPA. The server's fallback handler serves this
# directory; SENTORI_WEBAPP_DIST env-var can override.
COPY --from=webapp-builder /webapp/dist /app/webapp
COPY --from=builder --chown=nonroot:nonroot /data-skel /data

EXPOSE 8080
USER nonroot
ENTRYPOINT ["/usr/local/bin/sentori-server"]
