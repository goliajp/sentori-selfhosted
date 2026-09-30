# Sentori

Crash and error reporting for mobile and web apps, designed to be run
by the team that reads it. One wire format, five SDKs, five verbs, and
a server you can host yourself with one compose file.

```ts
import { sentori } from '@goliapkg/sentori-web'

sentori.init({ token: 'st_…', ingestUrl: 'https://sentori.example.com' })
sentori.error(new Error('checkout failed'))
```

Nothing above returns a promise or throws. That is a rule, not an
accident: the five verbs are synchronous and silent, because an
instrument that can break the app it watches gets removed from the
app it watches.

## Start here

1. [Getting started](getting-started.md) — install, first event, and a
   `curl` that a gate executes so it cannot rot.
2. Your runtime:
   [React Native](getting-started/react-native.md) ·
   [browser](getting-started/web.md) ·
   [WeChat mini program](getting-started/weapp.md)
3. [Protocol](protocol.md) — the wire schema, batching, token format,
   sourcemap upload, cross-origin requests.
4. [Error reference](errors.md) — every code this server sends,
   generated from the handlers.
5. [Using the dashboard](dashboard.md) — which screen answers which
   question, and how the queue is ordered.
6. [Troubleshooting](troubleshooting.md) — the failure modes people
   actually hit, with the hints the server really sends.

A running instance also serves [/llms.txt](../webapp/public/llms.txt),
which carries enough to send a first event without following any link.

## The five verbs

| Verb | For |
|---|---|
| `error` | something broke and the user can tell |
| `warn` | something broke and the user cannot tell yet |
| `trace` | a step worth seeing in the timeline before a crash |
| `assert` | an invariant you expect to hold in production |
| `probe` | a tripwire in code you believe is now unreachable |

They are the same five in every SDK, and the server stores them under
one schema. There is no separate metrics pipeline, no second SDK for
performance, and no Sentry compatibility layer.

## SDK reference

Each page is the API surface that ships with its package:

- [Sentori for Swift](sdk-swift.md) — native iOS and tvOS, without
  React Native.
- [Sentori for Kotlin](sdk-kotlin.md) — native Android, without React
  Native.
- [`@goliapkg/sentori-react-native`](../sdk/react-native/README.md) —
  React Native and Expo.
- [`@goliapkg/sentori-web`](../sdk/web/README.md) — the browser, any
  framework or none.
- [`@goliapkg/sentori-weapp`](../sdk/weapp/README.md) — WeChat mini
  programs. Needs an instance on a domain with an ICP filing; the
  platform will not call anything else.

## Running it

- [Self-hosting](self-hosting.md) — environment variables, backup and
  restore, Postgres upgrade notes.
- [Teams, projects, ownership](teams.md) — accounts, roles, project
  assignment.
- [Scaling](runbook/scaling.md) — what to read before adding capacity,
  and what this topology does not do.
- [CLI authentication](runbook/cli-auth.md) — which token each command
  wants.

## Recipes

- [Source map upload from CI](recipes/sourcemap-upload.md)
- [Release versioning](recipes/release-versioning.md)

## Wire formats

- [Replay encoding v2](replay-encoding-v2.md) — the replay attachment
  format: a keyframe plus deltas, one NDJSON line per frame. Read this
  before writing anything that produces or consumes a replay.

## What is not here

Pages describing versions that no longer exist are not kept. The
CHANGELOG is the history.
