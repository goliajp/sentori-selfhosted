# Sentori self-hosted

Run sentori on your own server. One docker compose command,
postgres + server + bundled SPA in a single image.

## Quick start

```bash
git clone <this-repo>
cd sentori-selfhosted

cp self-hosted/docker/.env.example .env
# Edit POSTGRES_PASSWORD + SENTORI_SESSION_SECRET +
# SENTORI_BOOTSTRAP_OWNER_EMAIL + SENTORI_BOOTSTRAP_OWNER_PASSWORD

docker compose -f self-hosted/docker/docker-compose.yml up -d

# Open http://localhost:8080
# Sign in with the bootstrap owner credentials
```

Or pull the prebuilt image:

```bash
docker run -d --name sentori-pg \
  -e POSTGRES_PASSWORD=changeme -p 5432:5432 \
  postgres:18-alpine

docker run -d --name sentori \
  -e SENTORI_DATABASE_URL='postgres://postgres:changeme@host.docker.internal:5432/postgres' \
  -e SENTORI_SESSION_SECRET="$(openssl rand -base64 24 | head -c 32)" \
  -e SENTORI_BOOTSTRAP_OWNER_EMAIL='you@example.com' \
  -e SENTORI_BOOTSTRAP_OWNER_PASSWORD='change-me-please' \
  -p 8080:8080 \
  ghcr.io/goliajp/sentori/sentori-server:latest
```

## HTTPS in production

Put any reverse proxy in front of port 8080:

```caddyfile
sentori.example.com {
    encode zstd gzip
    reverse_proxy localhost:8080
}
```

Keep `SENTORI_COOKIE_SECURE=1` (default) so the session cookie
sets the `Secure` flag.

## Env vars

See `self-hosted/docker/.env.example` for the full list with
inline docs. Required:

- `POSTGRES_PASSWORD`
- `SENTORI_SESSION_SECRET` — `openssl rand -base64 24 | head -c 32`
- `SENTORI_BOOTSTRAP_OWNER_EMAIL` + `SENTORI_BOOTSTRAP_OWNER_PASSWORD`
  (first boot only)

Optional:

- `SENTORI_ATTACHMENT_STORE=fs:/data/blobs` (default — persistent
  blob volume)
- `SENTORI_PUSH_WORKER_ENABLED=1` (default — background push dispatcher)
- `SENTORI_COOKIE_SECURE=1` (default — flip to 0 for local-dev HTTP)
- `SENTORI_SAASADMIN_USER_IDS=<uuid,uuid>` (limits /admin/api/saas/*
  visibility; leave unset on single-workspace self-hosted)

## SDK integration

```ts
import { init } from '@sentori/core';
init({
  token: 'st_pk_...',          // from /tokens page in dashboard
  ingestUrl: 'https://sentori.example.com',
});
```

The token format `st_pk_<26 base32>` is a permanent contract.

## License

Apache-2.0 OR MIT. Copyright © GOLIA K.K.

---

This repo is a read-only mirror of the upstream sentori monorepo.
Issues / PRs are not accepted here.
