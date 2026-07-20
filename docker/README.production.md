# Sentori production deploy

Phase 16 baseline. The dev `docker-compose.yml` at the repo root is
single-host and runs Postgres in a sidecar container; production splits
the stack across:

- 1 **app VM** (Hetzner CCX23 or similar) running this compose file:
  Caddy + 2 server containers (blue/green) + Valkey + the web SPA image.
- 1 **PG VM** (Hetzner CPX21) running PostgreSQL 18 with WAL archiving
  to Cloudflare R2 (Phase 16 sub-C).
- Cloudflare DNS (orange cloud for `sentori.<tld>` → Pages, grey cloud
  for `app.` / `api.` / `ingest.` / `docs.` / `cdn.` / `status.` → app
  VM).

## File layout

- `production-compose.yml` — service graph (server-blue + server-green
  for blue/green deploys, Valkey, web, Caddy).
- `Caddyfile` — TLS termination, security headers, CORS, blue/green
  reverse-proxy upstreams.

## Required env

Load via `sops` + an age key (Phase 16 sub-D); never check secrets in.

```
SENTORI_DOMAIN=sentori.golia.jp
SENTORI_BASE_URL=https://sentori.golia.jp
SENTORI_VERSION=v0.2.0          # default :latest is fine for staging

SENTORI_DEV_TOKEN=...
SENTORI_ADMIN_PASSWORD=...
SENTORI_SESSION_SECRET=...      # ≥ 64 random bytes
DATABASE_URL=postgres://sentori:...@<pg-vm-private-ip>:5432/sentori

SENTORI_SMTP_HOST=...
SENTORI_SMTP_PORT=587
SENTORI_SMTP_USER=...
SENTORI_SMTP_PASS=...
SENTORI_SMTP_FROM=sentori@golia.jp
```

## Day-zero deploy

1. Provision the app VM with Docker installed, open ports 80/443/443udp.
2. Provision the PG VM, run migration `0001_..0009_*.sql` from
   `server/migrations/`. Confirm `psql` reachable from app VM only.
3. Cloudflare DNS: add A records for `app` / `api` / `ingest` (grey
   cloud — Caddy handles TLS).
4. Copy `production-compose.yml` and `Caddyfile` to the app VM.
5. Place secrets in `/etc/sentori/.env`, then:
   ```
   docker compose -f production-compose.yml --env-file /etc/sentori/.env pull
   docker compose -f production-compose.yml --env-file /etc/sentori/.env up -d
   ```
6. Caddy will request certificates the first time each subdomain
   is hit. `docker logs caddy -f` to watch ACME.

## Day-N deploy (blue/green)

Push a new tag, then on the app VM:

```
docker compose -f production-compose.yml pull server-blue
docker compose -f production-compose.yml up -d --no-deps server-blue
# observe metrics + Better Stack uptime for a few minutes
docker compose -f production-compose.yml pull server-green
docker compose -f production-compose.yml up -d --no-deps server-green
```

Caddy's `lb_policy ip_hash` keeps a given client's session pinned to
one upstream while the other rolls.

## Rollback

```
SENTORI_VERSION=<previous-tag> docker compose -f production-compose.yml \
  --env-file /etc/sentori/.env up -d --no-deps server-blue server-green
```
