# Sentori self-hosted

Crash and warning reporting for mobile apps, on your own server.
One `docker compose` command runs the whole thing: a single app
image (API + dashboard) plus postgres.

## Quick start

```bash
git clone <this-repo> sentori && cd sentori/self-hosted/docker

cp .env.example .env
# Set the three required values:
#   POSTGRES_PASSWORD   — any strong secret
#   SENTORI_OWNER_EMAIL — your sign-in address
#   SENTORI_BASE_URL    — public origin (keep the default for a local try-out)

docker compose up -d
```

If you didn't set `SENTORI_OWNER_PASSWORD`, the first boot
generates one — grab it from the log:

```bash
docker compose logs sentori | grep password
```

Open `http://localhost:8080`, sign in as the owner, and you're on
the Inbox.

## Connect an app

1. **Settings → Projects**: create a project.
2. **Settings → Tokens**: mint an `ingest` token for it.
3. In the app:

```ts
import { sentori } from '@goliapkg/sentori-react-native';

sentori.init({
  token: 'st_...',                       // the ingest token
  ingestUrl: 'https://sentori.example.com',
  release: 'myapp@1.4.2',
});
```

Events group into issues on the Inbox; assertions, probes and
trace points appear under Instruments.

## HTTPS

TLS belongs to your reverse proxy — point it at port 8080 and put
the https origin in `SENTORI_BASE_URL` (it's used in email links):

```caddyfile
sentori.example.com {
    encode zstd gzip
    reverse_proxy localhost:8080
}
```

## Email notifications (optional)

Set the `SENTORI_SMTP_*` values in `.env` and restart. New issues
and regressions then mail the owner and assigned admins;
**Settings → Notifications** has per-project switches and a
test-email button. Without SMTP everything else works — the page
just shows the channel as not configured.

## Locked out?

```bash
docker compose exec sentori sentori-server reset-password you@example.com
```

Prints a fresh password (and signs out that account everywhere).
No SMTP required.

## Configuration

Everything lives in `.env` — see `.env.example` for the full
annotated list. Secret-bearing variables also accept a `_FILE`
variant pointing at a mounted secret file
(`SENTORI_SMTP_PASS_FILE`, ...).

Upgrades:

```bash
docker compose pull && docker compose up -d
```

Migrations run automatically at boot. Back up the `sentori-db`
volume before major version jumps.

## License

Apache-2.0 OR MIT. Copyright © GOLIA K.K.

---

This repo is a read-only mirror of the upstream sentori monorepo.
Issues / PRs are not accepted here.
