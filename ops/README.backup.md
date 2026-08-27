# Sentori PG backup + log pipeline (Phase 16 sub-C)

## What's here

- `backup.sh` — `pg_dump --format=custom` → Cloudflare R2 (`daily/sentori-<stamp>.dump`), drops dumps older than `RETENTION_DAYS` (default 30).
- `restore.sh` — pulls the latest (or a specified) dump from R2 and restores into a fresh `$PG_DB`. Prompts for `yes` before dropping.
- `postgresql.archive.conf` — append-and-reload snippet that turns on continuous WAL archiving to R2 for ≤ 5-minute RPO.
- `vector.toml` — journald → Grafana Cloud Loki shipper.

## One-time setup on the PG VM

```sh
# rclone for the postgres user (so archive_command can call it)
sudo -u postgres rclone config create r2 s3 \
    provider=Cloudflare \
    access_key_id=<CF_R2_ACCESS_KEY> \
    secret_access_key=<CF_R2_SECRET> \
    endpoint=https://<account>.r2.cloudflarestorage.com \
    region=auto

# enable WAL archiving
sudo cat ops/postgresql.archive.conf >> /etc/postgresql/18/main/postgresql.conf
sudo systemctl reload postgresql

# install the daily dump cron
sudo install -m 750 ops/backup.sh /opt/sentori/backup.sh
sudo tee /etc/cron.d/sentori-backup <<'EOF'
PGPASSWORD=...
RCLONE_REMOTE=r2:sentori-backups
0 4 * * * postgres /opt/sentori/backup.sh >> /var/log/sentori-backup.log 2>&1
EOF
```

## One-time setup on the app VM (logs)

```sh
sudo apt install vector
sudo install -m 644 ops/vector.toml /etc/vector/vector.toml
# put LOKI_ENDPOINT / LOKI_USER / LOKI_PASSWORD in /etc/vector/.env
sudo systemctl enable --now vector
```

## Recovery drill (do this once before launch)

The drill only counts if you actually rebuild a fresh VM end-to-end:

1. `terraform apply` (or click) a new PG VM.
2. Install Postgres + rclone, point rclone at the same `r2:sentori-backups`.
3. `RCLONE_REMOTE=r2:sentori-backups PGPASSWORD=… ./restore.sh`
4. Optional: replay WAL up to a target time:
   ```sh
   # in /var/lib/postgresql/18/main/recovery.signal:
   restore_command = 'rclone copyto r2:sentori-backups/wal/%f %p'
   recovery_target_time = '2026-05-09 03:55:00 UTC'
   ```
5. Stand up a server pointing at the restored DB; spot-check the
   dashboard (orgs, projects, recent issues). Time the whole thing —
   anything over 30 minutes is too slow for our RTO.

Update `docs/runbook/backup-restore.md` with the actual minutes you
measured (Phase 16 sub-F).
