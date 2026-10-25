# Sentori — self-hosting

Sentori runs on your own infrastructure: one app image, one Postgres, one
`docker compose up`. This guide covers self-hosted operations. Sentori Cloud uses
a separate control plane and explicit deployment, billing and region configuration.

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

## Instance health

The owner can run **Instance settings → Instance health** or, inside the app
container, `sentori-operator healthcheck`. The HTTP endpoint is
`GET /admin/api/instance/health`; anonymous users and members cannot read it,
and Cloud data-plane users must use their managed operations surface instead.

This read-only check shares its implementation with the operator CLI. It checks
database connectivity, exact migration versions/checksums, single-instance
identity, readable persistent storage, recent backup job metadata for the current
instance generation, and overdue lifecycle jobs. It does not run migrations,
write test blobs, send mail or contact outside services. SQL and connection errors
are reported without secrets or filesystem paths.

The versioned JSON report distinguishes `pass`, `warning`, `fail` and `unknown`.
CLI exit codes are 0, 1, 2 and 3 respectively; unhealthy diagnostics still produce
JSON, including when the database cannot be reached. A backup older than 24 hours
is a warning. A recent backup record is not proof that the archive still exists,
has an off-host copy, or can be restored. Recovery remains `unknown` until verified
separately; the overall report does not claim fully healthy while that is unknown.
Free disk space, write access, external TLS/reachability, SMTP and complete blob
integrity are outside this first read-only check. The existing **Environment
checks** action separately writes, reads and deletes a temporary blob for setup.

Run archive verification and a restore drill in an isolated instance to obtain
recovery evidence. Operational health never substitutes for the release testsuite,
candidate-image acceptance, or an actual old-version upgrade test.

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
| `SENTORI_MASTER_KEY` | Optional 64-character hexadecimal key for encrypted integration credentials and managed backups. Keep it outside database backups; losing or changing it makes existing credentials unreadable. No default key. |
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
| `SENTORI_BACKUP_DIR` | `/data/backups` in Compose; otherwise unset | Absolute path for encrypted backup archives. Requires filesystem blob storage and `SENTORI_MASTER_KEY`. Download completed archives to independent storage. |
| `SENTORI_ATTACHMENT_STORE` | `fs:/data/blobs` in Compose | Persistent blob root. For offline restore, use a new empty directory and then configure the server to read that directory. |
| `SENTORI_PORT` | `8080` | Host port for dashboard + ingest. |
| `SENTORI_VERSION` | `latest` | Pin an image tag instead of tracking latest. |
| `SENTORI_SMTP_HOST` | *(empty)* | Empty runs without email — everything else works and Settings shows the channel as not configured. See `SENTORI_SMTP_{PORT,USER,PASS,FROM,TLS}`. |
| `SENTORI_ARTIFACT_KEEP_RELEASES` | `20` | Keep symbolication artifacts for the newest N releases per project. `0` disables. |
| `SENTORI_EVENT_RETENTION_DAYS` | `90` | Delete raw events and their attachments after N days. `0` disables. |
| `SENTORI_DB_MAX_CONNECTIONS` | sqlx default (`10`) | Max Postgres connections the server pool opens. Raise it if `/metrics` shows `sentori_db_pool_in_use` pinned near `sentori_db_pool_size`; make sure Postgres' own `max_connections` has the headroom first. A zero or non-numeric value is ignored with a warning rather than honoured — a pool of 0 hands out no connections and would read as a database outage. |
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

Migrations run at boot. Before a major upgrade, preserve a consistent database
backup **and** the persistent application files. A database-only backup cannot
recover replay bytes or source artifacts.

### Upgrade from v4 to v5

The v5 schema keeps the original migrations 0001–0021 unchanged and adds new
metadata and tables. Existing owners, projects, ingestion tokens, events and file
references remain usable. New backup/migration features require persistent blob
storage and an instance master key; keep that key with your operational secrets.

1. Record the currently running v4 image digest, database version, Compose settings,
   blob location and master key. Pin the intended v5 image version or digest; do not
   use a moving `latest` reference for the rehearsal.
2. During the maintenance window, stop application writes and background workers.
   With the standard Compose deployment, stop the `sentori` service while leaving
   `db` available. Create a PostgreSQL custom-format dump and an independent copy
   or storage snapshot of `sentori-data` from this same stopped state. Store the
   backup outside those volumes. If blob storage is outside `sentori-data`, include
   that location too. The v4 image does not contain the new recovery operator.
3. Restore the paired backup into isolated volumes and a separate database first.
   Start v5 there with the same instance master key and a test ingress. Verify owner
   and member access, existing SDK token scope, representative events, replay bytes,
   source artifacts and symbolication. Check the actual version from `/healthz`.
4. After the rehearsal passes, apply the pinned v5 image to the maintenance copy
   or planned production instance. Confirm migration completion before reopening
   traffic. Send an actual SDK event and verify its stored evidence. Retain the v4
   backup and exact previous image through the observation window.

For a rollback after v5 has accepted data, first close ingress and preserve a
verified v5 encrypted archive with the matching operator and recovery key. Restore
it into an isolated validation instance so the new data remains readable. Restore
the paired **pre-upgrade v4 database and blob backup** into separate volumes, run the
pinned v4 image, and check login, replay bytes and a new SDK event before routing
traffic to it. Keep the v5-only data for explicit reconciliation; restoring the old
snapshot does not include it. Do not point the old binary at the already-upgraded
database or delete the upgraded volumes to make rollback appear complete.

The release gate runs this sequence using the actual v4.1.1 binary and the current
candidate. It also backs up a sealed project with a queued migration job: restoring
that v5 archive cancels old migration state, erases its queued key and clears its
project fence before the operator reviews the recovered instance.

## Backups and recovery

The v5 image includes `/usr/local/bin/sentori-operator`. From a source checkout,
build `self-hosted/cli/Cargo.toml`; its output is named `sentori-cli`. This is the
**operator binary**, separate from the SDK artifact upload CLI with a similar name.
These commands require the v5 build; v4 images do not include this operator.

In **Self-hosted → Backups**, an owner downloads a newly generated recovery key,
confirms it is safely stored, and starts a backup. The server captures one consistent
Postgres snapshot and every referenced replay/attachment/source artifact, encrypts
all frames, then reads the archive back to verify its schema, counts and hashes.
Missing or corrupt referenced files fail the operation. No partial archive is offered
for download. Progress and cancellation survive page reloads; a crashed worker retries
with a new fenced attempt, up to three attempts. Recovery keys are temporarily sealed
with the instance master key and removed from active job storage on completion.

**Upload and verify** checks an existing recovery archive using its saved key. Browser
uploads are limited to 1 GiB; use the operator CLI for larger archives (up to 64 GiB).
The uploaded filename is never used as a storage path. A failed verification does not
produce a downloadable artifact. This action verifies contents; it does not restore them.

Download the archive to an independent machine or storage service. Compare its
SHA-256 with the value shown in the dashboard and run the independent verifier below.
A recorded download start does not establish that the file reached off-instance storage.
Keep **both** the recovery key and the original `SENTORI_MASTER_KEY` in separate secure
storage. Neither can be recovered from backup history. Archives contain user accounts,
sessions, SDK credentials and integration secrets; access is restricted to owners.
Database connection settings, SMTP/TLS configuration, environment variables and
Compose secrets are not archived. Keep the deployment configuration separately;
restoring rows and blobs does not recreate those external settings.

For command-line backup, with `DATABASE_URL` and the instance `SENTORI_MASTER_KEY`
provided through the operator's environment:

```bash
sentori-operator keygen --out recovery.key
sentori-operator dump --out backup.sentori --blob-dir /data/blobs --key-file recovery.key
sentori-operator verify --from backup.sentori --key-file recovery.key
```

The verifier needs no database. `keygen` creates a new private key file and refuses
to replace an existing file. Capture coordinates with physical blob garbage collection
and uses a repeatable-read database transaction; ingest can continue during capture.
Keep the matching operator image/binary with the backup: schema migration checksums
and column types must match exactly. A logical `export` is selective NDJSON and does
not contain referenced blob bytes; it is not a recovery backup or a secret-free Cloud
migration package.

### Project migration packages

A project package uses a separate, explicitly scoped mode. It includes selected
projects, issues, events, replay/attachment bytes, releases/source artifacts, sessions,
tasks/activity and probe/assert data. Historical authors are represented only by source
UUID and display name, for explicit identity mapping at import time. It excludes
passwords, emails, account roles, membership grants, authentication sessions, SDK tokens,
push devices/tokens, integration credentials, webhook/health URLs, active automation,
pending jobs and deliveries. New tables and columns are excluded unless added to the
format's positive allowlist. The instance master key is not needed.

```bash
sentori-operator keygen --out migration.key
sentori-operator package-projects --project PROJECT_UUID --out project.sentori --blob-dir /data/blobs --key-file migration.key
sentori-operator verify --from project.sentori --key-file migration.key
```

Repeat `--project` for each selected project (maximum 1,000). Unknown or duplicate
project IDs fail; the verifier checks child references against the declared scope.
Telemetry payloads, user-authored notes and source/attachment bytes are preserved;
this is **not content redaction**. Keep the encrypted package and its key protected.
The instance `restore` command rejects this mode. The managed migration below uses
additional archive bindings for the migration, target identity, generation and copy
phase. An unbound `package-projects` archive cannot be uploaded into that workflow.

### Move projects to Cloud or another instance

Open **Project migration** from **Self-hosted** on the source and from the target
workspace's instance settings. Both sides require their own authenticated owner.
The source must be self-hosted; a Cloud workspace can receive a migration. Both
runtimes need persistent filesystem blob storage and a configured master key. The
master key protects active job keys and the replacement token used for forwarding;
it is not copied between instances.

1. On the target, save its instance identity file. On the source, choose **Start a
   migration**, load that file and select the projects. Save the resulting migration
   descriptor and load it with **Receive a migration** on the target. Check both
   instance IDs and project scope. This file contains no credentials.
2. On the source, **Export initial copy**. Save the generated recovery key off-instance
   and confirm that it has been saved. Download the completed encrypted package.
   The source remains writable during this copy.
3. On the target, **Import initial copy**, selecting that package and its matching
   key. Import checks the archive's schema, identity bindings, references, row counts,
   checksums and blob bytes before publishing it in one database transaction.
   Imported projects remain fenced against writes. Unrelated target projects are
   preserved. Cloud project and artifact capacity apply; historical imports do not
   spend current monthly event or ingestion-byte allowance.
4. Prepare the rollback plan described below. On the source, **Seal source writes**.
   Sealing waits for admitted writes to finish, then fences all selected projects at
   the database boundary. SDK requests receive a retryable 503 until forwarding is
   configured. Export the **final copy** and save its matching key. The final package
   is a complete reconciliation of the sealed source, so changes, deletions and late
   events since the initial copy are included.
5. Import that final package on the target. It replaces only this migration's fenced
   staging projects. After successful verification, choose **Activate target** and
   record the data preservation plan. Activation admits new writes. Create **new**
   target ingestion tokens after the final import, then update SDK endpoints,
   project IDs and tokens. Tokens created on the staging copy are removed by final
   reconciliation. Recreate member grants, Push credentials, integrations and
   automation settings explicitly; none are carried in the project package.
6. Wait for every imported project to receive a new SDK event. The page shows events
   actually observed by the target after activation. Historical timestamps cannot
   satisfy this check. **Verify received events**, acknowledge the SDK configuration
   change, then **Complete migration**.
7. For older clients, configure **Forward old clients** on the source for each project
   with the target's public HTTPS origin and a newly issued target ingestion token.
   The source verifies the target instance, generation, project and final archive
   before storing the replacement token encrypted. Its existing source token stays
   on the source. Events, batches, attachments and deploy telemetry are forwarded;
   admin APIs and Push are not. Redirects and private destinations are refused.
   Target failures become retryable 503 responses to old clients.

Each upload is limited to **1 GiB**. Select smaller project groups where possible.
A single project's package over this limit requires an operator-assisted migration;
there is no browser import bypass. Keep the source and target on the same schema
version until migration completes. Restarted jobs recover from durable leases;
failed or cancelled jobs erase the stored recovery key. A retry after a lost response
uses the exact command ID, version and file while the dialog remains open. After
closing or reloading, inspect the persisted job before starting another command.

Project, issue, release and historical author identities are mapped deterministically.
Event and session IDs stay stable so queued SDK retries deduplicate against imported
history. Archived authors are display-only identities with unusable credentials and
no grants. Migrated tasks do not retain automatic completion authority.

Forwarding expires after 1–90 days and can be renewed on the same page. Watch its
success/failure counters, last request and expiry. Counts are HTTP requests, not
individual events. Completion does **not** stop the source or its forwarding routes.
Keep the source available for the rollout and offline-client retry horizon; an
expired route returns retryable 503. Do not shut it down solely because one recent
request succeeded.

### Migration rollback plan

Before sealing, record the responsible operator, where independent encrypted
backups and their keys are held, the allowed outage and how new target data will be
preserved and reconciled. Before cutover, **Cancel migration** can discard a target's
fenced staging copy without changing the source. After sealing or activation, the
UI deliberately offers no simple unfence or endpoint-reversal action.

After activation, the target may contain events, sessions, attachments, task edits
and release changes absent from the source. Reopening the old source without
reconciling those changes loses data or creates divergent histories. If rollback is
needed:

1. Stop admitting traffic at the target ingress and pause applications/automation
   that write to it. Keep old-client source forwarding closed or retrying during
   this window; do not resume source writes.
2. Preserve a verified encrypted backup of the target database **and referenced
   blobs**, plus an independent sealed-source backup. For a Cloud target, coordinate
   this recovery with the Cloud operator. Preserve the migration receipts, project
   mapping, manifests and hashes, final package, keys and the SDK cutover time.
3. Work in an isolated recovery target. Compare source and target records using the
   migration identity map and stable event/session IDs. Reconcile target-only rows,
   edits, deletions and referenced files; decide explicitly how conflicting changes
   are resolved. Verify counts, checksums, replay bytes, releases and authorization.
   There is no automated reverse-import command in this release.
4. Validate the recovered instance with an actual SDK event and required replay and
   symbol checks. Only then reconfigure client endpoints, issue appropriate new
   credentials and reopen traffic under the operator's recovery plan. Retain both
   original instances and archives until the recovery is accepted.

Never clear `projects.migration_fence` manually to make a failed migration appear
complete. Offline whole-instance restore invalidates active transfer jobs and
forwarding routes; review migration topology and target data before resuming traffic.

### Restore to a new target

Prepare an isolated target with a **new empty database** and the same operator version.
Stop the target server before restoring. Point `DATABASE_URL` to that empty target and
provision the original `SENTORI_MASTER_KEY` separately. Put the archive and recovery key
in operator-readable locations; choose a blob directory that does not exist yet:

```bash
sentori-operator verify --from backup.sentori --key-file recovery.key
sentori-operator migrate
sentori-operator restore --from backup.sentori --key-file recovery.key --blob-dir /data/restored
```

There is no overwrite mode. A live target server, occupied target, wrong key,
incompatible schema, invalid path, truncated archive or missing content rejects the
restore. Rows are restored in one transaction; a precommit failure rolls them back
and removes that operation's new blob directory. If a connection fails during COMMIT,
inspect the target before retrying: the result may be uncertain, and the new blob files
are deliberately preserved.

Set `SENTORI_ATTACHMENT_STORE=fs:/data/restored` on the target and start its server.
The target receives a **new instance ID** and a new generation, recording the archive
ID and source ID as provenance. It starts in `restore_validation` mode: all automatic
workers are paused. Sign in, open an existing replay, and ingest a disposable test event
to check source mapping. Saved backup-history files are marked unavailable because
managed backup archives are not recursively included in the archive. Monitors are
paused and pending outbound work is cancelled or marked uncertain.

Once validation is complete, stop the old instance and the restored target server.
Use the values returned by `restore` (also available in `GET /admin/api/instance`):

```bash
sentori-operator resume-recovery --instance-id TARGET_UUID --generation TARGET_GENERATION \
  --archive-id ARCHIVE_UUID --confirm-old-instance-stopped
```

Review integration destinations and SDK routing **before** resuming. Restart the
target after the command succeeds. Background workers resume, and integrations that
were enabled in the backup can send for new events. Monitors remain disabled until
explicitly reviewed and enabled. Unrouted notification events are closed and pending
outbound work from validation is cancelled before resume, so a test event cannot
unexpectedly generate a later delivery.

The operator CLI runs in the v5 image via
`docker compose run --rm --no-deps --entrypoint /usr/local/bin/sentori-operator sentori …`.
For recovery, run it in a **separate Compose project with fresh volumes**, start only its
`db` service, and mount the archive and key read-only. Never point this command at the
source database. Do not use `docker compose down -v` on the source.

Manage backup retention and offsite scheduling according to your recovery needs.
Completed archives currently remain in `SENTORI_BACKUP_DIR` until the operator removes
them; an interrupted process may also leave temporary encrypted files. Keep sufficient
free space and remove orphaned files only while the backup worker is stopped.
Existing `ops/backup.sh` and `ops/restore.sh` are deployment-specific scripts described
in `ops/README.backup.md`; they are not the v5 archive format.

### Reset with an independent recovery copy

From **Self-hosted → Overview → Reset instance**, type the exact instance name, ID
and generation, enter the current owner password, and acknowledge the data removal.
The server returns a private reset permit that must be used within ten minutes.
Download it; preparing a permit does not remove data. A newer permit replaces the
previous one for that owner. Logout, password rotation, session expiry or loss of
the owner role invalidates it. A restored instance cannot use the source's permits.

Provision `/recovery` on independently mounted storage that survives losing the
instance, and keep a private recovery key separately at `/keys/recovery.key`.
Generate a new key with `sentori-operator keygen --out /keys/recovery.key` if needed.
Mount the downloaded permit where the operator command can read it. The paths
below are examples: use the real blob root and actual independent mount paths.
The tool rejects recovery paths inside the blob root; the operator's
`--confirm-independent-storage` attests the storage failure boundary, which cannot
be established from a filesystem path alone.

Stop **every** server replica and other operator command using this database;
leave PostgreSQL running. Configure the same database connection and original
`SENTORI_MASTER_KEY` for the operator. Copy the exact command generated by the UI:

```bash
sentori-operator reset --permit-file reset.permit \
  --instance-id INSTANCE_UUID --generation GENERATION --instance-name 'INSTANCE_NAME' \
  --blob-dir /data/blobs --backup /recovery/reset.sentori \
  --journal /recovery/reset.json --key-file /keys/recovery.key \
  --confirm-independent-storage
```

For Compose, stop `sentori`, then use `docker compose run --rm --no-deps` with the
operator entrypoint shown above and the independent storage/key/permit mounts.
Mounts and ownership must allow the packaged, non-root operator to write the backup
and journal. `docker compose exec` cannot operate on the stopped service.

The tool acquires the offline instance lock and freezes database writes and blob
garbage collection. It creates a **new** complete encrypted backup on the specified
storage, reads it back to verify it, and persists an external journal before deleting
any rows. An older downloaded archive cannot authorize discarding newer events.
Missing blob bytes, wrong keys, schema mismatch, unavailable storage or an occupied
output path abort the operation. A crash before the clearing transaction commits
leaves the database intact and preserves any completed archive/journal for inspection.

Reset clears all application data, accounts, sessions, credentials and queued work.
It preserves the instance ID and name, increments the generation and returns the
instance to uninitialized state. It removes the blob store's recognized content and
interrupted temporary writes. Unknown files or symbolic links require inspection;
the tool never recursively deletes an arbitrary directory. Encrypted backup-history
files outside the blob root, database infrastructure/WAL/backups, and deployment
configuration are retained. This is an application reset, not physical secure erasure.

After the clearing transaction commits, the server and ordinary operator commands
refuse to run until blob cleanup and the independent completion receipt are durable.
If the command stops or reports an I/O error, preserve the archive, key and journal:

```bash
sentori-operator finish-reset --journal /recovery/reset.json --key-file /keys/recovery.key
```

This continuation checks the journal against the pending operation and verifies the
archive again before finishing cleanup. It does not need an unexpired permit after
the clearing transaction has committed. It is idempotent for the same completed
operation and generation. Do not remove `reset_pending` manually or edit the journal.
The external `reset.complete.json` receipt and new instance audit retain the operation,
old/new generation, initiating owner ID, archive ID and archive SHA-256.

Restart only after success. Configure the owner credentials for the new setup,
run the environment check, initialize again, and issue new SDK and integration
credentials. Existing deployment environment settings still apply at bootstrap.
To undo a reset, restore the independent archive into a **new empty** target using
the recovery procedure above, and validate the retained events and replay bytes
before resuming workers or redirecting clients.

## Before you call it production

- [ ] TLS terminating in front, `SENTORI_BASE_URL` set to the https
      origin.
- [ ] `POSTGRES_PASSWORD` not the example value; owner password
      changed from the generated one.
- [ ] A verified database-and-blob backup stored off this host,
      its recovery and master keys stored separately, and a restore drill.
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


## Signed task automation (v5 development)

The task workspace supports a generic signed webhook adapter for repository, CI and
deployment events. It does not establish a GitHub or Jira account connection.
Configure the repository and deployment environment from **Tasks → Task automation**
as an owner. The server needs `SENTORI_MASTER_KEY`; generate a random key with
`openssl rand -hex 32`, save it outside the database and provide the same key to
all server processes. The project signing key is shown once and encrypted in the
database. Put that signing key in your CI secret store.

Send JSON to `/hooks/tasks/{project_id}` with `X-Sentori-Timestamp` containing the
current Unix timestamp in seconds. Set `X-Sentori-Signature` to `sha256=` followed
by the lowercase hexadecimal HMAC-SHA256 of `timestamp + "." + raw_request_body`,
using the displayed signing key as UTF-8 bytes. The request timestamp must be
within five minutes of the server; retries may use a new signature and timestamp
with the same body and `deliveryId`. Never change a delivery's body when retrying.

```json
{
  "deliveryId": "ci-run-184-unit-and-e2e",
  "taskId": "019e358a-adac-7000-8000-000000000001",
  "repository": "acme/checkout",
  "commit": "0123456789abcdef0123456789abcdef01234567",
  "kind": "ci.completed",
  "occurredAt": "2026-10-10T00:00:00Z",
  "unit": "passed",
  "e2e": "passed"
}
```

All events require `deliveryId`, `taskId`, `repository`, full 40- or 64-hex `commit`,
`kind`, and the actual `occurredAt` timestamp (future event times are rejected). Supported kinds are `work.started`,
`pr.opened` / `pr.merged` (positive integer `number`), `ci.completed` (`unit` and
`e2e` are `passed` or `failed`), and `release.deployed` (`release` and the configured
`environment`). The body is limited to 16 KiB. Rejected signatures never update
work items. Reusing an ID with another body returns 409.

Automatic completion requires a merged PR, both checks passing, and deployment
of the same commit to the configured environment, occurring no earlier than the
merge. Evidence may arrive out of order; the most recent work/PR event chooses the
active commit, and old-commit CI cannot replace its evidence. Changing integration
configuration starts a new evidence version. A regression or manual stage change
sets a server-side time barrier so older evidence cannot close the task again.
Manual completion is recorded separately and does not assert verification.

Task comments, state changes, audit records and notifications commit together.
Personal inbox delivery is retried from the database outbox and deduplicated per
recipient and source event. Project access and preferences are rechecked during
delivery, and current access is checked again when reading old notifications.


## HTTP monitors and outbound integrations

The Status page configures public HTTP health checks and shows incident history,
the latest 60 samples and availability over the last 30 days for the current
configuration. Checks run every 60 seconds to 24 hours, with a 1–30 second timeout
and a configurable 1–10 consecutive-failure threshold. One successful check
recovers an incident. Editing configuration starts a new sample window; pausing
stops scheduling. Samples expire after 30 days; incident history remains available.

Only HTTP/HTTPS targets on ports 80/443 that resolve entirely to public addresses
are supported. Each DNS answer and redirect is checked; requests pin validated IPs
and ignore proxy environment variables. Private networks and metadata endpoints
are blocked. Checks follow at most three redirects, reject HTTPS downgrades, and
read at most 64 KiB. Existing SDK `backendHealthUrl` checks use these rules too.
Internal-network monitoring agents are not yet available.

The Integrations page supports email via the configured SMTP server, signed
HTTPS webhooks, GitHub issue creation and Jira Cloud issue creation. An instance
vault master key is required to save settings. Preserve that key separately from
the database: encrypted credentials cannot be recovered without it. Reads return
configuration summaries; secret tokens and full webhook URLs are not returned.
Leaving an existing credential blank retains it, while changing an endpoint
requires a new credential. Existing project webhooks are encrypted in place when
the vault is available; their full URL remains owner-only for compatibility.

Saving a configuration does not send anything. **Send test performs a real write**:
GitHub and Jira create a real issue. Task exports copy the reviewed task version's
title and description. Automatic event subscriptions start empty. Selecting new
issues, regressions, task assignments/updates or monitor down/recovery events
turns on those deliveries; GitHub/Jira create a new issue per event. This is
outbound delivery, not bidirectional task synchronization.

The delivery history distinguishes queued, running, retrying, delivered, failed,
cancelled and uncertain results. Known retryable webhook failures get at most five
attempts with the same delivery ID. Outbound writes never follow redirects. An
ambiguous native issue creation, SMTP send, or worker interruption stays uncertain
until an owner checks the destination and explicitly accepts a possible duplicate
before retrying. Configuration changes, revoked access, personal opt-outs and
instance generation changes are checked again before dispatch. Webhooks continue
to work when SMTP is not configured. Notification events and delivery history are
currently retained for auditing; account for their growth when sizing storage.

Signed webhook requests include `X-Sentori-Delivery`, `X-Sentori-Timestamp` (Unix
seconds) and `X-Sentori-Signature`. Verify the latter as `sha256=` plus lowercase
hex HMAC-SHA256 of `timestamp + "." + raw_body`, using the configured signing secret
as UTF-8 bytes. Verify a bounded timestamp window and deduplicate by delivery ID;
retries preserve the ID but may have a fresh timestamp and signature.

## Optional isolated Web browser runner

Real-user visit ingestion runs in the server. Active browser and Lighthouse
checks additionally need a dedicated Linux Docker host with Node.js 24 or later.
Install the runner from the same qualified source as the server. Keep this host
separate from sensitive application services; each target page executes inside
a disposable, non-root container with no host mounts, Docker socket or server
API token. The coordinator needs permission to create and remove containers.

Build and qualify the image before setting `SENTORI_WEB_RUNNER_IMAGE`:

```sh
python3 self-hosted/web-runner/qualify.py --build
python3 self-hosted/web-runner/qualify.py --test
```

The build prints a content-addressed test image tag. Use the qualified immutable
image ID in the service configuration. The fixture covers healthy/broken pages,
public/private SEO, scoped test credentials, blocked destinations and lab budgets.
Install pinned coordinator dependencies with `bun install --frozen-lockfile
--production` from `self-hosted/web-runner`, then run `node worker.mjs` under your
service manager. The source layout must retain `scripts/lib/headless-chrome.mjs`.

| Variable | Purpose |
| --- | --- |
| `SENTORI_WEB_RUNNER_TOKEN` | Dedicated random secret of at least 32 characters; configure the same value on the server and coordinator. The server also accepts its `_FILE` variant. Never use an SDK/project token. |
| `SENTORI_WEB_RUNNER_API_URL` | Coordinator's server origin; default `http://127.0.0.1:8080`. Use HTTPS across hosts. |
| `SENTORI_WEB_RUNNER_IMAGE` | Required qualified Docker image ID for individual jobs. |
| `SENTORI_WEB_RUNNER_NETWORK` | Docker network for the job containers; default `bridge`. |
| `SENTORI_WEB_RUNNER_LOCATION` | Operator-supplied measurement location label; default `unspecified`. |
| `SENTORI_WEB_RUNNER_AUTH_FILE` | Optional private JSON file of project/app-scoped test identities. |
| `SENTORI_WEB_RUNNER_LOCK` | Optional absolute path to `bench-lock` on a shared runner; each job takes a separate heavy lock with a 30-second wait limit. |

The coordinator passes only the selected test identity into a browser job over
stdin. Store the configuration and identity file with owner-only permissions.
An identity file contains an array of objects shaped as follows; use a dedicated
least-privilege test account and rotate its session normally:

```json
[
  {
    "projectId": "00000000-0000-0000-0000-000000000001",
    "appId": "00000000-0000-0000-0000-000000000002",
    "name": "admin-test",
    "storageState": { "cookies": [], "origins": [] }
  }
]
```

`storageState` uses Playwright's cookie/local-storage format. Cookie domains and
local-storage origins must match the configured target origin; another project's
profile with the same name cannot be used. Profiles are never sent to the server.
Expired credentials produce a monitoring error and do not establish an outage.

Each job is limited to one CPU, 1 GiB memory, 256 processes and a read-only root
filesystem with a 512 MiB temporary directory. The browser-side proxy checks all
DNS answers, pins the selected public address, and caps connections/bytes.
Private and link-local destinations remain blocked even when explicitly listed
as allowed origins. Public policy inspects indexing prerequisites only; neither
it nor private policy confirms Google indexing. No checks or schedules are
created merely by installing the runner.


### Trusted reverse proxies and audit source IPs

By default Sentori uses the TCP peer address and ignores forwarded headers.
When deployed behind a reverse proxy, set `SENTORI_TRUSTED_PROXIES` to a
comma-separated list of that proxy's exact IPv4 or IPv6 addresses as observed
by the server. Hostnames, CIDRs and wildcard trust are not accepted. For those
peers only, the last `X-Forwarded-For` entry is used; malformed or missing values
fall back to the TCP peer. Do not trust an entire container or tailnet network.
The proxy must replace or append the actual peer address at the end of the
header.
If container networking maps every connection to one gateway address, first
restrict backend access to the trusted proxy. Trusting that shared gateway
without an access restriction would also trust direct clients' forged headers.

New audit records contain `ip`; historical records and background operations
without an originating request retain `null`. Authentication failures include
the attempted account but never the submitted password or reset token.
