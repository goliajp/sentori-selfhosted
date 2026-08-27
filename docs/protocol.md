# Sentori Protocol v1

> The wire contract between any SDK and the server. Both sides are
> real code — `sdk/core/src/types.ts` and
> `self-hosted/server/src/handlers/sdk/events.rs` — and this document
> describes them rather than the other way round. A field added on one
> side and not the other is caught by
> `scripts/check-wire-contracts.mjs` for the enumerated unions; the
> rest is on whoever edits.
>
> **This page described v0.x until 2026-08-10** — it documented
> `POST /v1/sessions`, `POST /v1/spans` and `POST /v1/spans:batch`
> (none of which exist), an event schema with `timestamp`,
> `breadcrumbs` and `tags` at the top level (the wire has `occurredAt`
> and a `payload` object), a `402 quota_exceeded` (billing is gone),
> and a size-limit table of constraints nothing enforced. Someone
> implementing against it built a client that 404s.

## Design principles

This protocol is intentionally **without legacy**. It does not maintain compatibility with Sentry, OpenTelemetry, or any other prior system. Notable choices:

- **camelCase** field names on the wire (idiomatic for JS/Swift/Kotlin clients; Rust server uses `serde rename_all = "camelCase"`).
- **Full words, never abbreviations** — `timestamp` not `ts`, `message` not `msg`, `function` not `fn`.
- **Single JSON event** — no envelope, no multipart, no streaming. One request = one or many events, all JSON.
- **Flat top-level structure** — no `contexts.{runtime, os, device, app, ...}` nesting tax.
- **Nested `cause`** for error chains, not `exceptions[]` arrays.
- **uuid-v7** for all client-generated IDs (RFC 9562, includes timestamp; sortable; modern).
- **ISO 8601 UTC, millisecond precision** for all timestamps.
- **Five kinds, one envelope** — `error`, `warn`, `trace`, `assert`,
  `probe`. There is no separate transaction, session or span object:
  a duration is a `trace` with a `ms` in its data, and the minute
  before an event travels as `payload.signals` rather than a
  breadcrumb list with its own schema.
- **Top-level is what the server routes on; everything else rides in
  `payload` untouched.** An SDK can add to `payload` without a server
  change or a migration.

## Versioning

API version lives in the URL path: `/v1/...`. Breaking changes ship as `/v2/`. Within a major version all changes are additive (new optional fields, new enum variants — clients ignore unknown).

## Endpoints

### `POST /v1/events`

One event. Body is a single [event object](#event-schema).

Response `202 Accepted`:

```json
{
  "eventId":    "019fe8a0-83a3-70a0-8624-8c4120c06bd5",
  "issueId":    "019fe8a0-83a5-7a60-bc7d-92f16566c7ea",
  "isNewIssue": true,
  "regressed":  false
}
```

`isNewIssue` and `regressed` are what the SDK's own diagnostics and a
CI smoke test read; nothing about delivery depends on them.

### `POST /v1/events:batch`

Up to **200** events in one request, plus two optional side channels.

```json
{
  "events": [ /* event objects */ ],
  "assertStats": [
    { "name": "cart.total matched the server", "release": "app@1.2.3+45",
      "passDelta": 4120, "failDelta": 2 }
  ],
  "backendHealthUrl": "https://api.example.com/healthz"
}
```

- `assertStats` — client-side aggregate of assertion passes. An
  assertion that passes forty thousand times must not become forty
  thousand events; only failures are events, and the passes arrive as
  a delta.
- `backendHealthUrl` — remembered per project and probed server-side
  once a minute. The app never pings it.

Response `200 OK`, one outcome per event, in order:

```json
{ "accepted": 2, "outcomes": [ { "eventId": "…", "issueId": "…" },
                               { "error": "invalid_payload", "detail": "…" } ] }
```

A malformed event in a batch does not fail the batch — its slot
carries the error and the rest are accepted. A device with one bad
event should not lose the other nineteen.

### `POST /v1/events/{event_id}/attachments/{kind}`

Binary evidence for an event that has already been accepted. `kind`
is one of `logTail`, `replay`, `screens`, `screenshot`,
`sessionTrail`, `stateSnapshot`, `viewTree` — the set a database
CHECK constraint accepts, so anything else is refused here with a
usable message rather than by Postgres.

Sent separately from the event on purpose: the event is small and
must land, the evidence is large and may not.

### `POST /v1/deploys`
Deploy hook (Phase 23 sub-C). Called by CI right after a build reaches users so the
dashboard's release timeline knows when each version went live. Auth uses the same
public token as ingest; rate-limit shares the ingest budget.

Body:

```json
{
  "release":     "myapp@1.2.3+456",
  "environment": "prod",
  "deployedAt":  "2026-05-10T18:30:00Z"
}
```

- `release` — required, ≤ 200 chars; should match the SDK's `release` config exactly.
- `environment` — optional, ≤ 64 chars; mirrors the runtime field but is not enforced.
- `deployedAt` — optional RFC 3339 timestamp. Defaults to server `now()`. Use this for
  backfilling historical deploys or pinning to the CI step time.

Response (`201 Created`):

```json
{
  "release":   "myapp@1.2.3+456",
  "deployAt":  "2026-05-10T18:30:00Z",
  "releaseId": "019e10..."
}
```

Idempotent — re-calling with the same `release` refreshes `deployAt` on the existing
`releases` row instead of creating a duplicate. The `created_at` column stays at the
row's first-touch moment (often when the first event for that release arrived).

Audit row `release.deployed` is recorded with payload `{project_id, release,
environment, deploy_at}`. CI is the actor — there is no user attribution because
token auth has no user identity.

Trailing slashes are not significant.

### `POST /v1/releases/{release}/artifacts`
Symbolication artifact upload — the source map, dSYM slice or R8 mapping that turns a
minified or stripped frame back into source. Needs an **api-scope** token, not the
public one shipped inside the app: whoever can replace a release's source map can make
every stack in it symbolicate to whatever they choose.

`multipart/form-data` with two fields:

- `kind` — one of `sourcemap`, `dsym`, `proguard`, `srcbundle`.
- `file` — the artifact. Gzip is inflated server-side, so `foo.map.gz` stores as
  `foo.map`. The **filename is data**: dSYM slices are matched to a crashing frame by
  the debug id embedded in it (`MyApp.app-arm64-<uuid>`).

The release row is created if it does not exist — maps are produced at build time,
usually before the app has ever run and announced its deploy, so requiring the row
first would make the ordering a trap.

Response (`201 Created`):

```json
{
  "id":              "019e10...",
  "kind":            "dsym",
  "name":            "MyApp.app-arm64-E63A748C-3F0E-302D-95EC-8DA5B55C97D9",
  "content_hash":    "990f6675...",
  "size_bytes":      304857600,
  "debug_id":        "E63A748C3F0E302D95EC8DA5B55C97D9",
  "first_seen":      true,
  "content_changed": true
}
```

- `debug_id` — the id read back out of the stored name, `null` when the name carries
  none (a source map has no debug id). This is the value a crashing frame is matched
  against, so it is what answers "is this the build that shipped?".
- `first_seen` — no artifact of this kind and name existed on the release before.
- `content_changed` — the stored bytes differ from what was there.

Both flags exist for the re-upload case: re-archiving a build does not guarantee the
same debug id, and an uploader with no way to tell cannot know whether the re-upload
accomplished anything. `first_seen: true` on a dSYM means the server had never held
that slice.

The unique key is `(release, kind, name)`, so a re-upload replaces rather than
accumulating near-duplicates a symbolicator would have to choose between.

### `GET /v1/releases/{release}/artifacts`
What actually landed. Same api-scope token as the upload — reading back what your own
token just wrote is strictly weaker than writing it, and it needs no dashboard session
or project UUID, so a release job can ask.

Response (`200 OK`):

```json
{
  "release": "myapp@1.2.3+456",
  "known":   true,
  "kinds":   { "sourcemap": 1, "dsym": 2, "proguard": 0, "srcbundle": 0 },
  "missing": ["proguard", "srcbundle"],
  "artifacts": [
    {
      "kind":        "dsym",
      "name":        "MyApp.app-arm64-E63A748C-3F0E-302D-95EC-8DA5B55C97D9",
      "debugId":     "E63A748C3F0E302D95EC8DA5B55C97D9",
      "contentHash": "571e3b3d...",
      "sizeBytes":   304857600,
      "createdAt":   "2026-08-09T17:33:49.239248Z"
    }
  ]
}
```

- `kinds` carries **every** kind with a count, zeros included: a gate wants to test a
  number, not the absence of a key.
- `known` is false when this instance has never heard the release name at all. A typo
  in the release string and a release nobody uploaded to look identical otherwise, and
  only one of them is fixed by uploading.

Scoped to the token's project: two projects with a release of the same name see only
their own.

`sentori-cli artifacts check --release <r> --expect sourcemap,dsym` wraps this and is
the one CLI command that exits non-zero on purpose — uploads stay lenient so Sentori
can never block a release, which means a broken upload step is silent and something
has to be allowed to notice.

### Conventions

Every field on `/v1`, in both directions, is **camelCase** — `appUserId`, `spToken`,
`sentAt`, `providerOutcome`. Paths are lowercase with hyphens where a segment needs
two words (`expo-compat`). Timestamps are RFC3339 and end in `At`. Errors are
`{"error": "<code>"}`, plus `detail` and `field` where there is one.

Two ids, two words: **`sendId`** is one call to `POST /v1/push/sends`; **`deliveryId`**
is one device's row inside it. They shared the word `send` until 3.0.0, and passing
one where the other belonged was a 404 with no hint which.

### `POST /v1/push/devices`
Register this device so Sentori can reach it. Ingest-scope token — the same one the
app already ships with.

```json
{
  "kind":        "apns",
  "nativeToken": "9f2c…",
  "env":         "sandbox",
  "userKey":     "ca010ec7…",
  "installId":   "6b1f…",
  "metadata":    { "appVersion": "4.2.1", "channel": "store" },
  "traits":      { "plan": "pro", "locale": "ja-JP" }
}
```

- `kind` is `apns` | `fcm` | `webpush` | `hcm` | `mipush`. **Not `provider`** — the
  React Native SDK sent that name for a year and earned a 422 for every registration
  it ever attempted.
- `env` is `sandbox` | `production`, and only APNs has the distinction: a token minted
  against one host is rejected by the other. FCM is a single host, so an `env` there
  would be a claim about a split that does not exist.
- `userKey` is the same salted-nothing identity hash every event carries
  (`SHA-256` of the normalised id, computed on the device). With it, an issue can
  reach the people it happened to; without it the device receives broadcasts only.
  The dashboard shows the difference as "N devices, M addressable".
- `metadata` is yours, stored verbatim and shown per device in Push ▸ Devices.
  Needs server ≥ 2.22.0 — before that the field reached neither the SDK's request nor
  the server's struct, and every row read `{}`.
- `traits` is what you know about the **person**, and what a send selects on: plan,
  cohort, org. Kept apart from `metadata`, which is about the build — otherwise a
  channel called `pro` answers a send aimed at the pro plan. Unlike `userKey` these
  travel raw, which is the point of the pair: the identity stays unreadable and the
  attributes stay selectable. Put nothing identifying here.
  Omitting `traits` keeps what the row has; sending `{}` clears them, which is what
  signing out sends. Needs server ≥ 2.28.0.
- `installId` is yours to mint once per install and keep. It is the upsert key, so a
  rotated vendor token updates this device instead of creating a second one under a
  new address. Never echoed back — the address a caller sends to is the row id, and an
  identifier that can claim a row must not be one that travels through logs.

Response (`202 Accepted`):

```json
{ "spToken": "019ff080-2aeb-7e30-aba1-4431b296d120", "isNew": true }
```

`spToken` is the `device_tokens` row id — a **bare uuid**. It carried an `ipt_`
prefix in v0.2. Anything routing on that prefix silently sends to the wrong place
after upgrading, so accept both while old devices are still registered.

Re-registering the same `(project, kind, nativeToken)` updates in place and un-revokes;
`env`, `userKey` and `metadata` are kept when the new call omits them. Calling this on
every launch is the intended usage.

### `DELETE /v1/push/devices/{spToken}`
Revoke. Ingest-scope. `202 Accepted` with `{"status":"revoked"}`, and idempotent — a
handle that is already revoked or was never yours is not an error worth failing a
sign-out over.

### `POST /v1/push/sends`
Send. **api-scope** token: an ingest token is in every copy of your app, and a
credential that can send notifications to your users is not one to hand out.

```json
{
  "spTokens": ["019ff080-…"],
  "payload":  { "title": "Back in stock", "body": "…" },
  "idempotencyKey": "restock-2026-08-11-u123"
}
```

`idempotencyKey` dedups **per device per key**: retrying the same send with the same
key queues nothing extra, and `queued` comes back as the number actually added. It had
the wrong grain until server 2.30.0 — the index was per project per key, so one send to
more than one device collided with itself and the whole call answered 500. A key was
only ever safe for an audience of exactly one.

`payload` is passed to the vendor verbatim. Response (`202 Accepted`):

```json
{ "sendId": "01a0…", "queued": 128, "capped": false }
```

`sendId` is the id **for this call** — one id however large the audience — and what
`GET /v1/push/sends/{sendId}` reports on. A background worker drains the queue every
5 s, retries, and quarantines a token the vendor rejects permanently.

`capped` is true when the audience was larger than one call will take (100 000), so
somebody did not get it. There is no array of per-device ids: `/deliveries` walks
those, and returning them inline was megabytes of uuid for a large send.

#### Who it goes to

Six ways, and they are one mechanism. `spTokens` / `tokenIds` / `nativeTokens` /
`topic` name devices; `appUserId` / `traits` / `audience` name people. Several device
modes together are a union. Only one of the three people modes may be given at a
time — a caller who sets two meant one of them, and guessing produces a send that
goes somewhere plausible and wrong; write them as one `audience` instead.

```json
{ "appUserId": "usr_123", "payload": { … } }
```

The id is hashed here the way the device hashed it, so it is compared against the
same value. Shorthand for `{"audience": {"user": "usr_123"}}`.

```json
{ "traits": { "plan": "pro", "locale": "ja-JP" }, "payload": { … } }
```

Every trait must match. Shorthand for an `audience` of equalities.

```json
{
  "audience": { "all": [
    { "trait":  "plan",       "in": ["pro", "team"] },
    { "device": "appVersion", "versionGte": "4.2" },
    { "any": [ { "trait": "locale", "is": "ja-JP" },
               { "trait": "org",    "is": "acme"  } ] },
    { "not": { "trait": "churned", "is": true } }
  ] },
  "payload": { … }
}
```

A tree, not a string: the dashboard's condition editor builds and reads a structure,
and a grammar would mean writing a parser and then the same structure behind it.

- **Groups** — `all`, `any`, `not`. An `any` with no branches matches nothing, which
  is what an editor holds the moment someone adds an or-group before filling it in.
- **Leaves** name one of `trait` (the person, from `user()`), `device` (the build,
  from `register()`), `user` / `userKey` (the identity), or `issue`.
- `{"issue": "<issue id>"}` selects everyone that issue happened to — the devices
  whose identity matches a row in the issue's hit table, which ingest writes. It is
  what "tell the people who hit this that it is fixed" compiles to, and the dashboard
  links to it from the issue's impact line. An id from another project selects
  nothing.
- **Comparisons** — `is`, `isNot`, `in`, `exists`, `prefix`, `gte` / `gt` / `lte` /
  `lt` (numbers), and `versionGte` / `versionGt` / `versionLte` / `versionLt`.
- **Use the version operators for versions.** As text `4.10.0` sorts *below* `4.2`,
  which is wrong for the first time on the day you ship your tenth minor release.
  `4.2` and `4.2.0` are the same version; a build suffix is ignored; anything
  unparseable is left out rather than swept in.
- At most 64 conditions, nested at most 8 deep. Explicit id lists are capped at 1 000
  — more than that is an audience, not a list.

An audience selects only live devices, and only ones whose `traits` were set: a device
that registered before `sentori.user()` ran carries none. The SDKs re-register by
themselves when the person changes, so ordering `user()` and `register()` no longer
matters (native ≥ 1.7.0).

### `POST /v1/push/audience/count`
How many devices an audience selects, without sending to it. **api-scope**, same body
as a send's targeting (`appUserId` / `traits` / `audience`), answers `{"matched": n}`.

The same compiled query the send runs with `count(*)` in front, so it is not an
estimate of what a send would do. The dashboard has counted before sending since
audiences existed and refuses to send to a number nobody read; a backend had no way
to — the preview is behind a browser session — so the one caller that sends on a timer
was the one that could not find out how large a condition had grown. Added in 2.31.0.

### `GET /v1/push/sends/{sendId}`
What happened to one call. **api-scope.** One poll answers it, whatever the size.

```json
{
  "sendId":  "01a0…",
  "state":   "in_flight",
  "createdAt": "2026-08-15T04:00:00Z",
  "lastSentAt": "2026-08-15T04:00:07Z",
  "counts":  { "total": 128, "queued": 12, "sent": 110, "failed": 6, "delivered": 74 },
  "reasons": [ { "reason": "BadDeviceToken", "count": 5 } ]
}
```

- **`state`** is `in_flight` while anything is still queued, `done` when nothing is.
  It says whether to keep polling — not whether everything arrived.
- **`sent` is not `delivered`.** `sent` means the vendor accepted it; `delivered`
  means the device told us, which only happens for apps whose SDK acks. Reading one
  as the other is how an integrator concludes a notification arrived while the phone
  was off. `delivered` is a subset of `sent`, and a zero there is not evidence of
  non-delivery.
- `404` for an id nobody minted.

### `GET /v1/push/sends/{sendId}/deliveries`
The rows behind the aggregate, one per device. **api-scope.**

`?status=failed` narrows to one state, `?limit=` (≤ 1000) sizes the page, and
`?cursor=` takes the previous page's `nextCursor`. Keyset, not offset: walking a
hundred thousand rows with an offset makes the last page cost a hundred thousand rows
to skip. `nextCursor` is absent on the last page rather than present-and-empty, so a
caller never asks for a page to find out there is none.

```json
{
  "deliveries": [
    { "deliveryId": "…", "spToken": "…", "status": "failed", "provider": "apns",
      "providerOutcome": "410", "error": "BadDeviceToken",
      "sentAt": "…", "deliveredAt": null, "retryCount": 3 }
  ],
  "nextCursor": "…"
}
```

### `GET /v1/push/deliveries/{deliveryId}` · `POST /v1/push/deliveries/{deliveryId}/ack`
One device's row, by the id from `deliveries[].deliveryId` — the same fields the
listing returns, because it is the same row. `404` when there is no such row.

The ack is what makes `delivered` mean anything: the SDK posts it when a notification
arrives, with an optional `{"ackSessionId": "…"}` so opening the same one twice
records once.

### `POST /v1/push/expo-compat/send` · `GET /v1/push/expo-compat/receipts/{send_id}`
Expo's server wire shape, in and out, in front of the same pipeline. A backend written
against `expo-server-sdk` can point at Sentori without changing its call sites. This
is the *server* shape only — the client side is `sentori.push.register()`.

### Topics and preferences
`POST` / `DELETE /v1/push/devices/{spToken}/topics[/{topic}]` subscribe and
unsubscribe; `GET` / `PUT /v1/push/users/{user_key}/preferences[/{category}]` read and
set per-category opt-outs. Both ingest-scope.

## Authentication and headers

| Header | Required | Example |
|---|---|---|
| `Authorization: Bearer <token>` | **yes** | `Bearer st_ahrpgkcuc…` |
| `Content-Type: application/json` | **yes** | `multipart/form-data` on the artifact and attachment routes only |
| `Sentori-Sdk` | no | `react-native/5.7.0` — who is reporting; the server records it and nothing branches on it |

The event's `id` is the idempotency key: re-sending the same id
returns `202` with the original `issueId`, `isNewIssue: false`, and
changes nothing — no second row, no second count against the issue.
That is the case a mobile SDK hits whenever a response is lost in
transit, so it is a success rather than a conflict. There is no
`Idempotency-Key` header.

(Until server 2.18.0 a resend was a primary-key violation surfaced as
`500 ingest_failed` — which this document tells SDKs to retry, so it
failed three more times and logged a dropped batch, for an event
already safely stored.)

The server does not negotiate a wire compatibility shim. `/v1` is
additive within its major version (see
[compatibility promises](#compatibility-promises)); an SDK that needs
a different shape needs `/v2`.

## Token and ingest URL

### Token format

`st_<random>` — one prefix, two scopes.

- **ingest** — what ships inside your app. It can post events and
  attachments and nothing else.
- **api** — what your CI holds. It uploads symbolication artifacts,
  reads back what landed, and drives the triage API.

The scope is a property of the token on the server, not of its text:
you cannot tell them apart by looking, and an ingest token on an
artifact upload is refused with `403 admin_token_required`. That
refusal matters — whoever can replace a release's source map can make
every stack in it symbolicate to whatever they choose, and the ingest
token is in the hands of everyone who has the app.

(The v0 `st_pk_…` / `sk_…` split is gone; a 4.x `st_pk_` token is
refused by this server.)

The token alone identifies the project — there is no project id in
any URL or header.

The token alone identifies a project — there is no separate project ID in URLs or headers.

### Ingest URL

The SDK takes two **independent** configuration fields, never combined into a single URL:

```ts
sentori.init({
  token: 'st_01j5y9z3vk8x4rmt2pcqjf7nw9',
  release: 'myapp@1.2.3+456',
  ingestUrl: 'https://sentori.example.com', // required — your instance
});
```

There is no default. `init()` without an `ingestUrl` logs one warning
and leaves every verb a no-op, because the alternative — falling back
to some other instance's host — would send an app's crashes to
strangers.

`ingestUrl` is the origin of the instance you run:

```ts
sentori.init({
  token: 'st_...',
  release: 'myapp@1.2.3+456',
  ingestUrl: 'https://sentori.your-company.com',
});
```

Environment variables: `SENTORI_TOKEN`, `SENTORI_INGEST_URL`.

### No DSN URL

Sentori does **not** use Sentry's `https://<key>@<host>/<id>` DSN format:
1. URL-embedded tokens leak whenever a logging framework records request URLs.
2. Token rotation should be independent of host change.
3. Two `.env` variables are clearer than parsing a DSN string.

Documentation **must not** use the term "DSN". Always say "token + ingest URL".

## Response codes

| Code | Meaning | Body |
|---|---|---|
| `202 Accepted` | single event accepted | `{ "eventId", "issueId", "isNewIssue", "regressed" }` |
| `200 OK` | batch processed (per-event outcomes inside) | `{ "accepted", "outcomes": [...] }` |
| `400 Bad Request` | the event could not be read | `{ "error": "invalid_payload", "detail": "<what>" }` |
| `401 Unauthorized` | missing, malformed or unknown token | `{ "error": "unauthorized", "hint": "<what is likely wrong>" }` |
| `403 Forbidden` | right token, wrong scope — e.g. an ingest token on an artifact upload | `{ "error": "admin_token_required", "hint": "…" }` |
| `413 Payload Too Large` | body over the route's limit | `{ "error": "too_large", "max": …, "got": … }` |
| `429 Too Many Requests` | rate limited | `{ "error": "rate_limited", "retryAfterMs": 1000 }` |
| `500 Internal Server Error` | server fault; retry with backoff | `{ "error": "ingest_failed" }` |

There is no `402`. Sentori has no quota to exhaust.

On `5xx` an SDK SHOULD back off exponentially — 1s, 2s, 4s, at most
three tries — and then drop rather than grow a queue without bound.
On `400` it MUST NOT retry: the same bytes will fail the same way.

## Event schema

Top-level fields are what the server routes, fingerprints and filters
on. Everything else rides in `payload`, stored as sent.

| Field | Type | Required | Notes |
|---|---|---|---|
| `id` | string (uuid-v7) | no | client-minted; the server mints one when absent |
| `kind` | `error` \| `warn` \| `trace` \| `assert` \| `probe` | **yes** | the five kinds |
| `occurredAt` | string (RFC 3339) | **yes** | when it happened, not when it was sent |
| `platform` | `javascript` \| `ios` \| `android` | **yes** | anything else is a `400` |
| `release` | string | no | `<name>@<version>+<build>`. Empty is accepted and costs you symbolication, regression anchoring and the release spread |
| `environment` | string | no | free text; `production` / `staging` / whatever you deploy |
| `name` | string | no | the `warn` / `trace` / `assert` name, or the `probe` ref. Part of the fingerprint for those kinds |
| `surface` | object | no | `{ screen, element }` — where it happened. Fingerprint input for `warn` |
| `userKey` | string | no | salted identity hash, computed **client-side**. The server never sees an identifier it could reverse |
| `payload` | object | **yes** | below |

### `payload`

| Field | Type | Notes |
|---|---|---|
| `error` | Error | required in practice for `kind: error` — it is what gets fingerprinted and symbolicated |
| `device` | Device | os, model, locale, screen, memory, battery, network |
| `app` | App | `{ version, build, framework: { name, version } }` |
| `signals` | array\<Signal\> | the minute before, see below |
| `data` | object | the verb's `data` argument, error instances already serialised |
| `context` | object | ambient flags and tags patched via `sentori.context()`; the dashboard slices on these |
| *(anything else)* | | stored untouched — this is how an SDK adds a field without a server release |

### Signal

The rolling context that replaces the breadcrumb list. It ships
inside `payload.signals` when an `error` or `warn` goes out, and
nowhere else — a quiet app sends none of it.

```json
{ "t": -9.0, "kind": "http",
  "data": { "method": "POST", "url": "/v1/pay/token", "status": 504, "ms": 30012 } }
```

- `t` — seconds relative to the event, negative for before.
- `kind` — free string, no server-side enum. Produced automatically:
  `nav`, `tap`, `lifecycle`, `freeze`, `push`. Hosts push their own
  with `pushSignal(kind, data)`.
- Dashboard conventions: `http` with `{ method, url, status, ms }`
  renders as a request line and colours a failure; `trace` is for
  quiet business breadcrumbs. Any other kind renders its data as
  `k=v`.

The SDK deliberately does **not** monkey-patch `fetch`/`XHR`. A host's
own interceptor is the right producer, and wrapping the network stack
of an app you do not own is not a free thing to do.

## Error schema

```json
{
  "type": "TypeError",
  "message": "Cannot read property 'id' of undefined",
  "stack": [ /* Frame */ ],
  "cause": { "type": "NetworkError", "message": "…", "stack": [ … ] }
}
```

`cause` nests rather than flattening into an `exceptions[]` array —
the chain is a chain, and the useful frame is usually several
`wrap and rethrow` layers above the throw site.

## Frame schema

| Field | Type | Notes |
|---|---|---|
| `file` | string | as the runtime reported it; rewritten to the source path when a map resolves it |
| `function` | string | |
| `line` / `column` | number | 1-indexed line, 0-indexed column — what every JS engine reports |
| `inApp` | boolean | your code rather than a dependency. Fingerprints are built from in-app frames, so a library upgrade does not mint new issues |
| `absolutePath` | string | |

Server-set once a map resolves the frame: `symbolicated: true`,
`minifiedFile` / `minifiedLine` / `minifiedColumn` (kept so a later
pass can re-resolve), and `preContext` / `contextLine` / `postContext`
when the map embedded `sourcesContent`.

## Batch wrapper

See [`POST /v1/events:batch`](#post-v1eventsbatch). `events` is
required; `assertStats` and `backendHealthUrl` are optional side
channels on the same request rather than endpoints of their own —
a quiet SDK should make one request, not three.

## Size limits

| Item | Limit | Enforced by |
|---|---|---|
| events per batch | 200 | `400 batch_too_large` |
| artifact upload, on the wire | 256 MB | route body limit; bigger artifacts arrive gzipped |
| artifact, decompressed | 512 MB | `413 too_large` |
| event / batch body | axum's default request limit | `413` |

That is the whole list. Earlier versions of this page tabulated
per-event caps on breadcrumbs, stack frames, cause depth and tag
sizes; none of them were ever enforced, and a documented limit nobody
checks is worse than none — it reads as a guarantee.

## Rate limits

Per **token**, in-memory, sliding window, defaults tunable by env:

| Setting | Default |
|---|---|
| `SENTORI_RATELIMIT_PER_TOKEN_RPS` | 100 events/sec |
| `SENTORI_RATELIMIT_WINDOW_SEC` | 1 |
| `SENTORI_RATELIMIT_DISABLED` | off |

The auth surface has its own, per IP: `SENTORI_AUTH_RATELIMIT_PER_IP`
(10) over `SENTORI_AUTH_RATELIMIT_WINDOW_SEC` (300) — a brute-force
response that stays loud without throttling a legitimate SDK burst.

Over the limit is `429` with `retryAfterMs`. The SDK MUST NOT retry
sooner.

## Examples

### A JS error

```json
{
  "kind": "error",
  "occurredAt": "2026-08-09T22:00:00.000Z",
  "platform": "android",
  "release": "myapp@1.2.3+456",
  "environment": "production",
  "userKey": "a91f3c02deadbeefa91f3c02deadbeef",
  "payload": {
    "error": {
      "type": "TypeError",
      "message": "Cannot read property 'id' of undefined",
      "stack": [
        { "file": "index.android.bundle", "line": 1, "column": 289430,
          "function": "e", "inApp": true }
      ]
    },
    "device": { "os": "Android", "osVersion": "15", "model": "Pixel 8",
                "locale": "ja-JP", "network": "wifi" },
    "app": { "version": "1.2.3", "build": "456",
             "framework": { "name": "react-native", "version": "0.85.3" } },
    "signals": [
      { "t": -24.0, "kind": "nav", "data": { "from": "Cart", "to": "Checkout" } },
      { "t": -11.1, "kind": "tap", "data": { "target": "Pay now", "x": 195, "y": 788 } },
      { "t": -9.0,  "kind": "http",
        "data": { "method": "POST", "url": "/v1/pay/token", "status": 504, "ms": 30012 } }
    ],
    "context": { "tenant": "acme", "plan": "pro" }
  }
}
```

The stack arrives minified. With a source map on the release it is
rewritten at ingest — before fingerprinting, so a column shift in a
rebuild cannot mint a new issue — and again by a later pass if the map
arrives after the crash did.

### A warn

No error object; `name` and `surface` are the fingerprint.

```json
{
  "kind": "warn",
  "occurredAt": "2026-08-09T22:00:00.000Z",
  "platform": "ios",
  "release": "myapp@1.2.3+456",
  "environment": "production",
  "name": "dead_button",
  "surface": { "screen": "Checkout", "element": "PayButton" },
  "payload": { "data": { "taps": 3, "withinMs": 1200 } }
}
```

### An assert failure

Passes never travel as events — they arrive as `assertStats` deltas on
a batch. Only the failure is an event.

```json
{
  "kind": "assert",
  "occurredAt": "2026-08-09T22:00:00.000Z",
  "platform": "ios",
  "release": "myapp@1.2.3+456",
  "environment": "production",
  "name": "cart.total matched the server",
  "payload": { "data": { "local": 4200, "server": 4150 } }
}
```

### A probe

A tripwire in a branch that was supposed to be dead. A probe that
never fires is the point; one that does says a fix stopped holding.

```json
{
  "kind": "probe",
  "occurredAt": "2026-08-09T22:00:00.000Z",
  "platform": "android",
  "release": "myapp@1.2.3+456",
  "environment": "production",
  "name": "checkout-double-charge",
  "payload": { "data": { "orderId": "o_8812" } }
}
```

## Compatibility promises

Within `/v1/`:

- The server SHALL NOT remove existing fields nor change their types.
- The server MAY add new optional fields; SDKs MUST ignore unknown fields.
- The server MAY add new enum variants; SDKs MUST treat unknown variants as `"other"` (or equivalent fallback).
- The SDK MAY omit any field marked "required: no".
- Breaking changes ship under `/v2/` with a 12-month overlap with `/v1/`.

## Document history

- **v0** — 2026-05-09 — initial draft.
- **v0.1.x** — 2026-05-10 — audit webhook, `POST /v1/deploys`,
  `POST /v1/sessions`.
- **v1** — 2026-08-10 — rewritten against the shipped code. Removed
  `POST /v1/sessions`, `POST /v1/spans`, `POST /v1/spans:batch`, the
  span and breadcrumb schemas, the `402` quota response, the
  unenforced size-limit table, the never-built audit webhook and a
  header table describing negotiation the server does not do.
  Documented the attachment endpoint, the five-kind event, `payload`,
  signals, and the limits that exist.
