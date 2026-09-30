# Scaling

Sentori runs as one server container and one Postgres container on one host. That is the whole topology, and most of what follows is about staying inside it rather than growing out of it.

> Rewritten 2026-09-08. The previous version sized Hetzner VMs that were never provisioned, added app VMs to a Caddy upstream pool that has one upstream, and tuned `org_quotas` — a SaaS table removed with the SaaS surface. Its three diagnostic steps queried a Grafana panel that drew nothing, a metric name the server does not emit, and an `events` partition tree that does not exist. Every number below is either read off the running system or named as an assumption.

## What you are actually running

`postgres-v1` + `server-v1`, `docker compose` on the app host, one Caddy upstream. No partitions on `events` — retention is a `DELETE` pass in `archive_worker`, not a partition drop. No queue in front of ingest; the per-token limiter and a 429 are the backpressure.

## Before adding anything

1. **Read `/metrics`.** `curl -s https://sentori.golia.jp/metrics | grep sentori_`. The four numbers that matter:
   - `sentori_ingest_total{status="accepted"}` — is traffic actually up?
   - `sentori_ingest_total{status="rejected"}` — a spike here **looks** like load and is usually an SDK regression sending malformed events. Do not add capacity to absorb broken clients.
   - `sentori_ingest_total{status="rate_limited"}` — the limiter is doing its job. This is not a capacity signal.
   - `sentori_db_pool_in_use` against `sentori_db_pool_size` — the one that leads somewhere (below).
2. **Check retention is real.** `SELECT count(*), min(received_at) FROM events;` — if the oldest row is older than `SENTORI_EVENT_RETENTION_DAYS`, the archive worker is not running and you have a retention bug, not a capacity problem. Check for `archive worker disabled` in the logs, and for `retention pass failed` warnings.
3. **Check the disk.** Attachments (replay frames especially) dominate volume, not rows.

## The pool

`sentori_db_pool_in_use / sentori_db_pool_size > 0.80` sustained is the `PgPoolNearSaturation` alert. The response:

```sh
# In /apps/sentori/.env, or the compose environment:
SENTORI_DB_MAX_CONNECTIONS=25
docker compose --env-file .env -f docker-compose.yml up -d server-v1
```

Confirm Postgres has the headroom first (`SHOW max_connections;` — default 100, shared with anything else on that instance). Unset it and the pool goes back to sqlx's default of 10.

A saturated pool is more often a slow query holding connections than genuine concurrency. `SELECT pid, now()-query_start AS age, left(query,80) FROM pg_stat_activity WHERE state='active' ORDER BY age DESC;` before raising the ceiling — a bigger pool against a slow query buys minutes.

## Vertical, then honestly

More RAM and faster disk on the Postgres side is the whole scaling story at this size, and it goes further than it sounds: no partitions, one primary, no replication to keep consistent.

Retention is the other lever, and it is the cheaper one. `SENTORI_EVENT_RETENTION_DAYS` at 90 with replay attachments is what fills disks. Halving it costs you evidence older than 45 days and nothing else — issue counters, first/last seen and the regression anchor are denormalized onto the issue row and survive the event deletion.

## What is not built

These are honest gaps, not planned work:

- **No horizontal path.** A second server container would need the rate limiter and ingest counters — both process-local — to become shared state, and there is no design for that. The limiter's own crate documents the seam (`RateBackend`); nothing implements a cross-process backend.
- **No autoscaler, no multi-region, no queue.**
- **No read replica.** One primary. Adding one is a real project, not a runbook step.

If you are hitting the ceiling of one host, that is a design conversation, and this file is not it.
