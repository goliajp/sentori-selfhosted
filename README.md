# Sentori

> Error tracking, built React-first. Self-hostable.

First-class SDKs for **React, React Native, Next.js, Vue, Svelte,
and SolidJS** — and a camelCase wire protocol any other platform
can speak. Dense Linear-style dashboard, single Rust binary
deploy. Self-host on a VM today; managed SaaS at
[sentori.golia.jp](https://sentori.golia.jp).

![iOS showcase hero](marketing/assets/showcase-hero.png)

---

## What's in the box

| | What | Where |
|---|---|---|
| 📡 | **SDKs** | `sdk/` — `@goliapkg/sentori-{core,javascript,react,react-native,next,vue,svelte,solid,expo}` |
| ✋ | **Manual instrumentation** | `sentori.captureMessage` / `startTrace` / `startSpan` / `withScopedSpan` / `track` / `recordMetric(..., { parent })` / `addBreadcrumb` — one first-class API per signal. See `docs-site` recipes: `manual-issue`, `manual-trace`, `manual-span`, `manual-moment`, `track-and-metrics`, `manual-breadcrumb`, `v1-to-v2-migration` |
| 🔁 | **Sentry compat** | `@goliapkg/sentori-{javascript,react-native,…}/compat` — drop-in `import * as Sentry from "@goliapkg/sentori-react-native/compat"` |
| 🖥️ | **Dashboard** | `web/` — React 19 + Vite + Tailwind v4 SPA, served at `/main` |
| 🚀 | **iOS showcase** | `apps/ios-showcase/` — SwiftUI 6 / iOS 26 native demo |
| ⚙️ | **Server** | `server/` — Rust + axum 0.8, PostgreSQL 18, Valkey |
| 🔧 | **CLIs** | `cli/` (Rust, source-map upload) + `sdk/cli/` (Node, issue triage + MCP) |
| 📚 | **Docs site** | `docs-site/` — Astro Starlight at [sentori.golia.jp/docs](https://sentori.golia.jp/docs) |

---

## Use it from a React Native app (60 s)

```sh
bun add @goliapkg/sentori-react-native
cd ios && pod install --repo-update
```

```tsx
import { sentori } from '@goliapkg/sentori-react-native'

sentori.init({
  token: '<your project token>',  // st_pk_…
  release: 'my-app@1.2.3',
  environment: 'prod',
  // ingestUrl is optional — defaults to https://ingest.sentori.golia.jp.
  // Override only if you're self-hosting.
  capture: { replay: { mode: 'wireframe', hz: 1 } },
})

// Optional — privacy-preserving cross-project user lookup. Identity
// is hashed on-device; the server never sees raw email / phone.
sentori.setUser({ linkBy: { email: user.email } })
```

Errors thrown anywhere in JS, iOS `NSException`s, Android uncaught
exceptions, fetch breadcrumbs, native crash files, and the wireframe
replay ring are all flushed automatically on `captureException`. No
extra plumbing.

→ Full guide: [sentori.golia.jp/docs/getting-started/react-native](https://sentori.golia.jp/docs/getting-started/react-native).

---

## Migrating from Sentry?

```ts
// One-line drop-in. Translates Sentry.init / captureException /
// captureMessage / setUser / setTag / addBreadcrumb. A console.warn
// fires once per non-translatable call so you know what to migrate
// natively when you're ready.
import * as Sentry from '@goliapkg/sentori-react-native/compat'

Sentry.init({ dsn: 'st_pk_…' })  // dsn is your Sentori token; ingest URL is bundled
Sentry.captureException(err)
```

→ Migration guide: [sentori.golia.jp/docs/migration-v1-to-v2](https://sentori.golia.jp/docs/migration-v1-to-v2)
→ Translation table: [sentori.golia.jp/docs/sentry-compat](https://sentori.golia.jp/docs/sentry-compat)

---

## Self-host

```sh
git clone https://github.com/goliajp/sentori
cd sentori

cat > .env <<EOF
SENTORI_DEV_TOKEN=st_pk_dev0000000000000000000000
SENTORI_ADMIN_PASSWORD=changeme
SENTORI_SESSION_SECRET=$(openssl rand -hex 32)
SENTORI_PG_PASSWORD=$(openssl rand -hex 16)
SENTORI_BASE_URL=https://sentori.your-domain.example
EOF

docker compose up -d
open https://sentori.your-domain.example/login
```

Sign in with `SENTORI_ADMIN_PASSWORD`. Full guide:
[sentori.golia.jp/docs/self-hosting](https://sentori.golia.jp/docs/self-hosting).

---

## What's React-first about it

- **First-class hooks for every supported framework.** Not "JS SDK
  + framework adapter" — the React / Vue / Svelte / Solid bindings
  are written against each framework's own primitives (Error
  Boundaries, `setup()`, `$capture_error`, error context).
- **Wireframe replay**, not raster session replay. 60 slots ×
  ~120 bytes/frame at idle. Renders as SVG rects in the dashboard,
  scrub-able prop-by-prop. No pixel-PII leak.
- **Cross-project user lookup with on-device identity hashing.**
  `linkBy: { email }` → SHA-256 of `salt || "email:" || email`
  ships to the server. The server stores per-org-salted
  fingerprints. Operator can look up "what errors did this user
  hit across all our projects" without ever seeing the raw email.
- **Silent + LLM-friendly.** `logLevel: 'silent'` config + flat
  type definitions + structured `onReady` callback so Claude /
  Cursor / Aider can generate correct integration code on the
  first try. Sentori SDK failures are swallowed and self-reported
  via a circuit breaker — host code never sees a stack trace
  from inside Sentori.
- **Free bonus, never a burden.** < 1% main-thread budget on
  mid-end devices, < 500 KB per `captureException`. NEVER rule:
  Sentori SDK failures must never cause host-app perf or network
  hiccups.

---

## Roadmap

- **v0.1 – v0.9** — self-hostable single-binary baseline. Done.
  Capture / dashboard / source maps / privacy classifier / hang
  watchdog / mobile vitals / screenshot + view-tree + state /
  session trail / wireframe replay sampler.
- **v1.0** — Replay scrubber + fiber tree diff, intent-cluster
  view of breadcrumb paths, iOS showcase as the open-source
  front door. Done.
- **v2.x** — Polyglot SDKs (React / Next / Vue / Svelte / Solid),
  Sentry compat layer, cross-project user lookup with PII-safe
  identity, single-domain consolidation. Done in v2.4.
- **v3.0+** — Android showcase, distributed trace replay
  RN → backend, AI-assisted root-cause hints.

---

## Stack

- **Backend** — Rust + axum 0.8 + PostgreSQL 18 + Valkey
- **Dashboard** — React 19.1 + Vite + Tailwind v4 + jotai + react-query
- **SDKs** — TypeScript core + per-framework bindings. Native
  Swift / Kotlin reusable as a pod / Gradle module without the
  Expo wrapper (see `apps/ios-showcase/`).
- **CLI** — `sentori-cli` for source-map upload (Rust); Node CLI
  in `sdk/cli/` for issue triage + Model Context Protocol server.
- **Showcase** — SwiftUI 6, iOS 26 deployment, MeshGradient +
  SF Symbol animations + Liquid Glass.

---

## What Sentori explicitly does NOT do

- Sentry wire-protocol compatibility (the `/compat` API translates
  Sentry **client SDK calls**, not the on-wire envelope format —
  Sentori uses a single JSON event per request, no envelopes)
- Raster session replay (we do wireframe instead — smaller, no
  pixel-PII leak, RN-tree-native)
- Native signal-based crashes (SIGSEGV outside `NSException`)
- Multi-tenant SaaS billing (coming after v3.0)

---

## License

Copyright © 2026 [GOLIA K.K.](https://golia.jp) Sentori is dual-licensed
under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.
