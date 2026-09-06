---
title: Troubleshooting
description: Common Sentori questions, what to check, and how to fix them
---

# Troubleshooting

Ten questions that come up over and over. Each one: what to check,
what to expect, what to fix.

## 1. Dashboard isn't seeing any events

**Diagnose**

```bash
# Smoke-test from the same machine the app runs on
curl -sI -X POST "$SENTORI_INGEST_URL/v1/events" \
  -H "Authorization: Bearer $SENTORI_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{}'
```

Look at the status code:

- `202` — the endpoint accepted a (malformed) event. Wiring is fine,
  problem is upstream of HTTP (init didn't run, capture isn't being
  called, hooks are stripped by a router).
- `400` — endpoint reachable, token OK, the test payload was just
  rejected for missing fields. Same conclusion as 202.
- `401` — token mismatch. Check it starts with `st_` and matches
  the project's token in Project settings → tokens.
- `429` — rate limit. Default is 1000 req/min/token; bump via
  `SENTORI_RATELIMIT_PER_TOKEN_RPS` (100/sec by default, over
  `SENTORI_RATELIMIT_WINDOW_SEC`), or wait for the window to pass.
- Connection refused / timeout — network. Confirm `SENTORI_INGEST_URL`
  is reachable from the app's network (not just your laptop).

**Fix**

Look at the SDK's stdout/console for `[sentori]` lines. The transport
logs every failure with the full URL and status; the test above
should be redundant if you can grep your logs.

## 2. Stack traces are minified

**Diagnose**

Open an Issue detail page. If the frames show files like
`index-DkkF.js` and functions like `Y`/`Z`/`a`, the sourcemap for
that release isn't loaded. Two reasons:

- **Source map not uploaded** — most common. Run:

  ```bash
  sentori-cli upload sourcemap \
    --release "myapp@1.2.3+456" \
    --token "$SENTORI_TOKEN" \
    --ingest-url "$SENTORI_INGEST_URL" \
    dist/assets/
  ```

- **Release name mismatch** — the upload's `--release` and the
  event's `release` field must match byte-for-byte (case-sensitive,
  including the `+build` suffix). Compare:

  ```bash
  # what the SDK sent
  curl ".../admin/api/projects/$PROJ/issues/$ISSUE/events" | jq '.[0].release'

  # what the upload labelled
  ls /data/artifacts/  # or check the release detail page
  ```

**Fix**

If the issue page now shows an "Unsymbolicated stack" banner above
the frames, click "Open release →" — the release detail page shows
which artifacts are present and the upload command to use if any
are missing.

Uploading it now is not too late. A successful upload re-reads the
crashes already stored for that release, so the stacks that came in
while the artifact was missing become readable too — you do not have
to wait for the bug to happen again. What it does **not** do is
re-group: an issue created from an unreadable stack and one created
from the readable version stay two issues, both now legible. If the
pass did not run (an upload from before server 2.12.0, or a failure
in the server log), run it by hand:

```bash
docker compose exec sentori sentori-server resymbolicate "myapp@1.2.3+456"
# or, for every release: drop the argument
```

It is idempotent — a frame that already carries its source window is
skipped — so running it again costs a read and changes nothing.

## 3. dSYM uploaded successfully but iOS frames still minified

**Diagnose**

The dashboard's release detail page lists slices with `arch` and
`uuid`. Compare the `uuid` to the event's frame:

```bash
# uuid the dSYM has
xcrun dwarfdump --uuid path/to/dSYM/Contents/Resources/DWARF/*

# uuid the event references — look at any frame's `debugId` field
```

If the uuids don't match, the dSYM was generated from a different
build than the one that crashed. Common causes:

- Debug build symbolicated against a release dSYM (or vice versa).
- Build cache wasn't cleaned; the dSYM is for the previous commit.
- Multi-architecture issue: you uploaded arm64 but the event came
  from a sim build (x86_64 / arm64-sim).

**Fix**

Find the matching dSYM:

```bash
# Spotlight indexes dSYMs locally
mdfind "com_apple_xcode_dsym_uuids == <event uuid>"
```

Re-upload with the matching dSYM. Sentori dedupes by sha256, so
re-running the upload is a no-op if it's the same file.

## 4. token 401

The transport returned 401. The server's 401 response includes a
`hint` field telling you which check failed:

```bash
curl -i -X POST "$SENTORI_INGEST_URL/v1/events" \
  -H "Authorization: Bearer $SENTORI_TOKEN" -d '{}' | tail -5
```

Possible hints:

These are every hint the server sends, measured against a running
instance rather than written from memory. The four in this table
before 2026-08-27 were invented: none of those strings existed, and
two of them named an `sk_` token, a concept `protocol.md` records as
removed.

| Hint | Cause | Fix |
|---|---|---|
| `send \`Authorization: Bearer st_<token>\` header` | No Authorization header at all | Add the header |
| `Authorization header must be \`Bearer st_<token>\`` | Header present, `Bearer ` prefix missing | Include `Bearer ` and one space |
| `token must start with \`st_\`` | The value is not a Sentori token | Copy from Settings → Tokens, not from a chat snippet |
| `token starts with \`st_\` but is the wrong length …` | Truncated in transit | Copy the whole value; it is `st_` plus 26 characters |
| `token unknown or revoked` | Well-formed, not in this instance's table | Wrong instance, or the token was revoked. Mint a new one |
| `token has wrong scope for this endpoint` | `ingest` token on an endpoint needing `api` | Mint an `api`-scope token |

## 5. Regression didn't fire when I expected

**Diagnose**

Sentori marks an issue `regressed` when:

1. It was resolved with a `resolvedInRelease`, AND
2. A new event lands with `(app, version)` strictly greater than
   `resolvedInRelease`.

Build numbers (`+build` suffix) are ignored. So `myapp@1.4.0+1`
resolving and `myapp@1.4.0+2` re-occurring does **not** trigger
regression.

```bash
# what was the resolve set to?
curl "/admin/api/projects/$P/issues/$I" | jq '.resolvedInRelease'

# what release sent the new event?
curl "/admin/api/projects/$P/issues/$I/events" | jq '.[0].release'
```

**Fix**

If you want every build to count as a new release for regression
purposes, encode the build into the `version` portion (less ideal)
or accept the current semantics (more honest — you didn't ship a
new version, you just rebuilt).

If the dashboard says the issue is still `resolved` but you have
events from a newer release: the regression evaluator runs on a
1-minute cron; wait a minute and refresh. Persistent miss = bug,
file an issue.

## 6. CI builds are slow because of source-map upload

The `sentori-cli upload sourcemap` step is sequential and uploads
the entire directory. For very large bundles (10+ MB) it can take
30–60s.

**Fix**

- Run upload in parallel with deploy when safe (errors that hit
  before symbols arrive will just look minified until symbols
  catch up — usually a few seconds).
- Upload only the changed assets by diffing `dist/assets/` against
  a previous build's manifest. CLI rejects on size and dedupes by
  sha256 internally; uploading the whole dir is safe but wasteful.
- Increase the CI runner's network bandwidth (GitHub `ubuntu-latest`
  is usually 1 Gbps; `runs-on: ubuntu-22.04` is similar).

## 7. Local dev floods the dashboard

You're seeing 1000+ events from `environment: dev` in production
dashboard.

**Fix**

Skip init in dev:

```ts
// web
if (import.meta.env.MODE === 'production') {
  // wrap with SentoriProvider
}

// RN
if (!__DEV__) {
  sentori.init({ /* ... */ })
}
```

Or set up a separate `dev` project with its own token, and switch
between them via `.env.local` (untracked) vs `.env.production`
(tracked). See [Multi-environment](./archive/web-sdk/multi-environment.md)
for the full strategy.

## Still stuck?

- File an issue on [GitHub](https://github.com/goliajp/sentori/issues)
- Self-hosted: check `docker compose logs server` for warnings
- The dashboard's Audit log (Settings → Audit) records every config
  change in the project; sometimes "events stopped flowing" is
  "someone rotated the token an hour ago"
