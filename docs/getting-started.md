---
title: Getting started
description: Pick the 5-minute quickstart that matches your stack
---

# Getting started

Sentori watches mobile apps. One quickstart, because there is one
supported stack:

| Stack | Quickstart |
|---|---|
| **React Native** (bare or Expo) | [getting-started/react-native](./getting-started/react-native.md) |

There were guides here for React, Next.js and Node until 2026-08-10.
They pointed at `@goliapkg/sentori-react` and friends — packages whose
source left this repo with the v1 redesign and which speak the v0.2
wire format, so following them produced an integration the current
server answers with `400 invalid_payload`. `@goliapkg/sentori-node`
never existed at all.

It assumes you already have:

- a **token** (`st_...`)
- an **ingest URL**

Both come from an instance you run: mint the token on the project's
Settings page, and the ingest URL is that instance's origin.

Don't have one yet? [Self-hosting](./self-hosting.md) — one
`docker compose up` on your own VM. There is no hosted signup;
Sentori is self-hosted, and `sentori.golia.jp` is GOLIA's own
instance rather than a service you can join.

## Working without an SDK

If you're prototyping or writing your own client, you can POST
directly to the ingest endpoint:

```bash
curl -X POST "$SENTORI_INGEST_URL/v1/events" \
  -H "Authorization: Bearer $SENTORI_TOKEN" \
  -H "Sentori-Sdk: curl/0.0.0" \
  -H "Content-Type: application/json" \
  -d '{
    "kind": "error",
    "occurredAt": "'"$(date -u +%FT%T.000Z)"'",
    "platform": "javascript",
    "release": "myapp@0.1.0+1",
    "environment": "dev",
    "payload": {
      "error": {
        "type": "TypeError",
        "message": "hello sentori",
        "stack": [{"file": "shell.ts", "line": 1, "inApp": true}]
      }
    }
  }'
```

A success is `202` with the ids the server assigned:

```json
{"eventId":"01a0…","issueId":"01a0…","isNewIssue":true,"regressed":false}
```

`issueId` is the machine-readable receipt — it is what to assert on in
a script, rather than opening the dashboard to look.

Three things this body gets right that are easy to get wrong, because
this page had all three wrong until 2026-08-27:

- **`occurredAt`, not `timestamp`.** It is required and has no alias.
  Sending `timestamp` drops it as an unknown field and the request
  fails deserialisation with `422 missing field occurredAt` — not the
  `400` you might expect, and not a message about the field you sent.
- **`error` goes inside `payload`.** So do `device` and `app`. Only
  `kind`, `occurredAt`, `platform`, `release`, `environment`,
  `userKey`, `name` and `surface` are top-level.
- **`id` is optional.** Send one only if you are deduplicating retries
  yourself.

See the [Protocol reference](./protocol.md) for the full schema.

## After you have events flowing

- [Notify Sentori of deploys](#deploy-pings) — one curl in CI keeps
  the release timeline accurate
- [Triage from CI with sentori-cli](#triage-from-ci) — `issue
  list / resolve / silence`
- [Make errors readable: upload source maps](#source-maps) —
  see `src/Foo.tsx:42` instead of `index.bundle:1:288432`

### Deploy pings

Add one line to your CI right after the build is uploaded:

```bash
curl -fsS -X POST "$SENTORI_INGEST_URL/v1/deploys" \
  -H "Authorization: Bearer $SENTORI_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"release\":\"myapp@$VERSION+$BUILD\",\"environment\":\"prod\"}"
```

Idempotent — re-running the same release just refreshes
`deployAt`, so flaky-CI retry is safe.

### Triage from CI

```bash
# Latest 20 active issues — one line per issue.
npx @goliapkg/sentori-cli issue list \
  --project "$PROJECT_ID" --status active --limit 20

# Mark resolved, tagging the fix release. The dashboard's regression
# detector flips it back to `regressed` if a matching event lands later.
npx @goliapkg/sentori-cli issue resolve <issue-uuid> \
  --project "$PROJECT_ID" \
  --in-release "myapp@1.2.4+457"

# Silence a known-noisy issue.
npx @goliapkg/sentori-cli issue silence <issue-uuid> \
  --project "$PROJECT_ID"
```

The admin token (`SENTORI_ADMIN_TOKEN`, `sk_` prefix) is in project
settings → tokens.

### Source maps

So a stack trace points at `src/Foo.tsx:42`, not `index.bundle:1:288432`.
After a release build, upload the source map tagged to the release —
**byte-for-byte the same string you pass to `init({ release })`**:

```bash
npx @goliapkg/sentori-cli@latest upload sourcemap \
  --release "myapp@$VERSION+$BUILD" --token "$SENTORI_TOKEN" \
  dist/assets/            # a build dir, or specific .map / .js files
```

The server symbolicates matching events at ingest and groups the issue
on the original-source frame. React Native (Hermes) needs the Metro
and Hermes maps composed first — see
[Source map upload](./recipes/sourcemap-upload.md) for the per-platform
steps and CI recipes (GitHub Actions / GitLab / Vercel / EAS).

## Reference

- [Protocol](./protocol.md) — wire format, if you're writing your
  own SDK or just curious
- [Self-hosting](./self-hosting.md) — production deploy, SMTP,
  backups, behind a reverse proxy
- [SDK — React Native](../sdk/react-native/README.md)
