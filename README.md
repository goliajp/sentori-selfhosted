# Sentori

> Self-hosted observability for mobile apps. Five kinds of signal,
> eight verbs, zero cost to the host app.

Sentori watches a React Native app the way a triager actually works:
**errors** (what broke), **warns** (where users hurt — rage taps,
long freezes, slow launches, detected automatically), **traces**
(what happened), **asserts** (what should hold — and never halts
production), and **probes** (did that bug come back). Events group
into issues; every issue is a self-contained case file — session
replay, the failing source line, the user's own actions, the minute
before — on a dense, fast dashboard.

One Rust binary + PostgreSQL. Your data never leaves your machines.

## What's in the box

| | What | Where |
|---|---|---|
| 📱 | **React Native SDK** | `sdk/react-native` — JS + Swift + Kotlin, Expo module (bare RN works too) |
| 🧩 | **Core** | `sdk/core` — types, wire protocol, the never-throw safety layer |
| 🔧 | **CLI** | `sdk/cli` — sourcemap / dSYM / Proguard / source-bundle upload, probe registry, MCP server for AI triage |
| ⚙️ | **Server** | `self-hosted/server` — Rust + axum, PostgreSQL 18 |
| 🖥️ | **Dashboard** | `webapp/` — React 19 SPA, baked into the server image |
| 🚀 | **Deploy** | `self-hosted/docker` — one compose file, distroless image |

## Use it from a React Native app (60 s)

```sh
bun add @goliapkg/sentori-react-native
cd ios && pod install --repo-update
```

```tsx
import { sentori } from '@goliapkg/sentori-react-native'

sentori.init({
  token: 'st_…',                       // ingest token, Settings → Tokens
  ingestUrl: 'https://sentori.your-domain.example',
  release: 'my-app@1.2.3',
  environment: 'production',
})

sentori.error(new Error('boom'))       // what broke
sentori.warn('pay.gateway-retry')      // where users hurt
sentori.trace('checkout.start')        // what happened
sentori.assert('total-positive', ok)   // what should hold (never halts)
sentori.probe('BUG-123')               // did that bug come back
```

Crashes (JS, `NSException`, Android uncaught), warn scenarios,
the behaviour timeline and wireframe replay are all automatic.
**Every call is synchronous and can never throw into your app** —
that contract, plus the full API (staged launch measurement, custom
breadcrumbs, visual replay with masking, backend availability), is
documented in
[`sdk/react-native/README.md`](sdk/react-native/README.md).

## Self-host

```sh
git clone https://github.com/goliajp/sentori-selfhosted
cd sentori-selfhosted/self-hosted/docker

cat > .env <<EOF
POSTGRES_PASSWORD=$(openssl rand -hex 16)
SENTORI_OWNER_EMAIL=you@example.com
SENTORI_OWNER_PASSWORD=changeme-12chars
SENTORI_BASE_URL=https://sentori.your-domain.example
EOF

docker compose up -d
open http://localhost:8080
```

Sign in as the owner, create a project, mint an ingest token, point
the SDK at your instance. TLS stays with your reverse proxy —
point it at `:8080`.

## Why it's different

- **Five kinds, one model.** The palette, the queue, the timeline
  and the instruments panel all speak the same five words. Learn
  eight verbs and you know the whole product.
- **Issues are case files.** Replay (wireframe always-on; pixel
  replay opt-in, with native-side masking so tagged views never
  exist in any frame), the failing line with surrounding source,
  the user's own actions with tap coordinates, and the minute
  before — one screen, no digging.
- **Asserts and probes.** Production assertions that never halt,
  aggregated into a liveness ledger; regression tripwires planted
  in the branch that used to break — a silent probe is proof the
  fix holds.
- **Objective importance.** Issues rank by breadth × depth (how
  many users × how hard each was hit), not raw event counts.
  Identity is hashed on-device; the server never sees a raw email.
- **The zero-cost contract.** < 1% main-thread budget, quiet
  network (nothing leaves until an error/warn fires), hard-bounded
  buffers, and no Sentori failure can ever reach the host app.
- **Slice by your own vocabulary.** `environment` is first-class;
  every `context` key (tenant, QA mode, build type…) becomes a
  queue dimension automatically — Sentori doesn't need to know
  what your keys mean.

## What Sentori explicitly does NOT do

- Sentry compatibility — protocol and SDK were designed from zero
- Web / browser platforms — mobile only, React Native first
- Multi-tenant SaaS — one instance, one team, your infrastructure

## Stack

- **Server** — Rust + axum, PostgreSQL 18, single distroless image
- **Dashboard** — React 19 + Vite + Tailwind v4, self-owned design
  tokens, dark/light, EN·中文·日本語
- **SDK** — TypeScript core + Swift / Kotlin natives as an Expo
  module; CLI in Node with an MCP server for AI-assisted triage

## License

Copyright © 2026 [GOLIA K.K.](https://golia.jp) Sentori is dual-licensed
under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.
