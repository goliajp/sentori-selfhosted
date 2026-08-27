# Sentori — self-hosting

Sentori is self-hosted first: one image, one Postgres, one
`docker compose up`. There is no hosted tier to fall back to, so this
page is the deployment story rather than a footnote to one.

**The quickstart lives with the compose file it describes:**
[`self-hosted/README.md`](../self-hosted/README.md). That file ships
inside the OSS mirror next to `docker-compose.yml` and `.env.example`,
which is where someone deploying actually is. This page covers what a
README should not carry — the env reference, what the retention knobs
do, the reverse proxy, and what to check before calling an instance
production.

> This page described the v0.1 stack until 2026-08-10 — service names
> `server` / `postgres`, port 8000, a `notification_recipients` table
> — none of which exist any more, while nine other pages linked here
> as the canonical guide. Two documents describing one deployment is
> how that happens; hence the single pointer above.

## What runs

```
                        ┌──────────────────┐
   browser ──HTTPS──▶   │  reverse proxy   │   (your TLS terminator)
   SDK     ──HTTPS──▶   └────────┬─────────┘
                                 │ HTTP :8080
                        ┌────────▼─────────┐
                        │  sentori         │  axum server + bundled SPA
                        │  (one container) │  ingest, dashboard, /api
                        └────────┬─────────┘
                                 │
                        ┌────────▼─────────┐      ┌──────────────┐
                        │  db (postgres 18)│      │ sentori-data │
                        └──────────────────┘      │ volume       │
                                                  │ blobs:       │
                                                  │ replays,     │
                                                  │ sourcemaps,  │
                                                  │ dSYMs        │
                                                  └──────────────┘
```

Two services, named `sentori` and `db` — the names matter, they are
what `docker compose exec` wants. The dashboard, the ingest API and
the `/api` surface are all one process on one port; there is no
separate web container to keep in step.

## Required configuration

Three values in `.env`. Everything else has a working default.

| Variable | What it is |
|---|---|
| `POSTGRES_PASSWORD` | Database password. The compose file wires it into the DSN for both services. |
| `SENTORI_OWNER_EMAIL` | The owner account, reconciled on every boot — changing it renames the owner. |
| `SENTORI_BASE_URL` | Public origin. Used in email links and shown to SDK users; with a proxy, the **https** origin. |

Leave `SENTORI_OWNER_PASSWORD` empty and the first boot generates one
and prints it:

```bash
docker compose logs sentori | grep password
```

Every secret-bearing variable also accepts a `_FILE` variant naming a
mounted file (`SENTORI_SMTP_PASS_FILE`, `SENTORI_DATABASE_URL_FILE`,
…). Direct env wins when both are set.

## Optional configuration

| Variable | Default | What it does |
|---|---|---|
| `SENTORI_PORT` | `8080` | Host port for dashboard + ingest. |
| `SENTORI_VERSION` | `latest` | Pin an image tag instead of tracking latest. |
| `SENTORI_SMTP_HOST` | *(empty)* | Empty runs without email — everything else works and Settings shows the channel as not configured. See `SENTORI_SMTP_{PORT,USER,PASS,FROM,TLS}`. |
| `SENTORI_ARTIFACT_KEEP_RELEASES` | `20` | Keep symbolication artifacts for the newest N releases per project. `0` disables. |
| `SENTORI_EVENT_RETENTION_DAYS` | `90` | Delete raw events and their attachments after N days. `0` disables. |
| `RUST_LOG` | `info,sqlx=warn` | Log filter. |

### What retention does and does not delete

Both knobs delete **evidence**, never **history**.

- Artifact retention reclaims sourcemaps, dSYMs and proguard maps for
  releases older than the newest N, plus any blob nothing references.
  Release rows survive, so resolve-anchoring and regression detection
  keep working on releases whose maps are long gone.
- Event retention removes raw events and their attachments (replays,
  screenshots). Issues — the aggregates you triage — keep their
  counters, first/last seen, timelines and regression anchors
  forever.

An issue from a year ago still tells you it happened 4,000 times to
900 people and when it came back. What it stops being able to show is
one particular occurrence's stack and replay.

## Behind a reverse proxy

Point the proxy at `:8080` and put the https origin in
`SENTORI_BASE_URL`. Sentori terminates no TLS itself.

Two things the proxy must not do:

- **Do not buffer uploads to a small limit.** Symbolication artifacts
  are tens to hundreds of megabytes; the server accepts 256 MB on the
  wire. A proxy with a 10 MB body cap rejects them, and because it
  usually resets the connection rather than answering, the CLI sees a
  bare network error rather than a 413.
- **Do not strip `Authorization`.** Both ingest and the `/api`
  surface authenticate with a bearer token.

## Operating

The one-shot commands (password reset, re-symbolication, artifact
verification, the pre-2.9.0 issue split) are listed in
[`self-hosted/README.md`](../self-hosted/README.md#operator-commands).

Upgrades:

```bash
docker compose pull && docker compose up -d
```

Migrations run at boot. Back up the `sentori-db` volume before a
major version jump.

## Backups

Postgres is the source of truth; the `sentori-data` volume holds
blobs (replays, screenshots, symbolication artifacts).

```bash
# nightly
docker compose exec -T db pg_dump -U sentori --format=custom sentori \
  > backups/sentori-$(date +%F).dump

# restore
docker compose exec -T db pg_restore -U sentori -d sentori --clean \
  < backups/sentori-2026-08-10.dump
```

The blob volume is worth backing up too, but it degrades gracefully:
symbolication artifacts are re-uploadable from CI, and a lost replay
costs one issue's evidence rather than the issue.

For the operational side — schedules, offsite copies, and the drill
that proves a restore actually works — see
[`runbook/backup-restore.md`](./runbook/backup-restore.md).

## Before you call it production

- [ ] TLS terminating in front, `SENTORI_BASE_URL` set to the https
      origin.
- [ ] `POSTGRES_PASSWORD` not the example value; owner password
      changed from the generated one.
- [ ] A nightly `pg_dump` running somewhere that is not this host,
      and a restore you have actually performed once.
- [ ] `SENTORI_EVENT_RETENTION_DAYS` set to what your disk can hold.
      Replays dominate the volume.
- [ ] SMTP configured, and the Settings page's test email received —
      without it, new-issue and regression notifications go nowhere
      and nothing says so.
- [ ] An ingest token per app, an api-scope token per CI. The ingest
      token ships inside your app; the api token uploads artifacts
      and must not.
- [ ] `sentori-cli artifacts check` in the release pipeline. Uploads
      exit 0 on failure by design, so this is the step allowed to go
      red when a symbol upload silently stopped running.
