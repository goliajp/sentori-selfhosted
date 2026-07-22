# @goliapkg/sentori-next

Next.js (App Router, ≥ 14) adapter for Sentori, built on
`@goliapkg/sentori-react` + `@goliapkg/sentori-javascript`.

## Install

```bash
bun add @goliapkg/sentori-next
# or
pnpm add @goliapkg/sentori-next
```

Set in `.env.local`:

```
NEXT_PUBLIC_SENTORI_TOKEN=st_pk_...
NEXT_PUBLIC_SENTORI_RELEASE=myapp@1.2.3
NEXT_PUBLIC_SENTORI_ENVIRONMENT=prod

# Optional — server-only token / release if you want to differentiate
# server traffic from browser traffic on the dashboard.
SENTORI_TOKEN=st_pk_...
SENTORI_RELEASE=myapp-server@1.2.3
SENTORI_ENVIRONMENT=prod
```

## Wire it up

### Server (instrumentation.ts at project root)

```ts
// instrumentation.ts
export { register, onRequestError } from '@goliapkg/sentori-next/instrumentation'
```

That's it — `register()` boots the SDK on Node start, and
`onRequestError` captures every server-side request error with route
+ method tags. The `register()` helper guards on `NEXT_RUNTIME ===
'nodejs'` so the edge runtime doesn't try to load Node-only deps.

### Client (app/layout.tsx)

```tsx
// app/layout.tsx
'use client'
import { clientInit, SentoriProvider } from '@goliapkg/sentori-next/client'

clientInit() // reads NEXT_PUBLIC_SENTORI_*

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html>
      <body>
        <SentoriProvider config={configFromEnv()}>{children}</SentoriProvider>
      </body>
    </html>
  )
}
```

### App Router error boundary (app/error.tsx)

```tsx
// app/error.tsx
'use client'
import { useReportNextError } from '@goliapkg/sentori-next/app-router'

export default function Error({
  error,
  reset,
}: {
  error: Error & { digest?: string }
  reset: () => void
}) {
  useReportNextError(error) // captureError once per error instance
  return (
    <div>
      <h2>Something went wrong</h2>
      <button onClick={reset}>Try again</button>
    </div>
  )
}
```

`useReportNextError` calls `captureError` once per error instance
and picks up Next's `error.digest` as a tag so the dashboard can
correlate the client report with the server error.

For a global catch-all, drop the same component into
`app/global-error.tsx`.

### Navigation breadcrumbs (any layout)

```tsx
// app/Shell.tsx — client wrapper mounted from app/layout.tsx
'use client'
import { useNextRouter } from '@goliapkg/sentori-next/app-router'

export function Shell({ children }: { children: React.ReactNode }) {
  useNextRouter() // nav breadcrumb on every pathname change
  return <>{children}</>
}
```

First mount does not emit a breadcrumb; only real pathname
transitions are recorded.

## What gets captured

| Path | Source tag |
|------|------------|
| Server route / API throw | `source=next.requestError`, `next.runtime=nodejs\|edge` |
| Component render error (App Router) | `source=react.errorBoundary` (via `<SentoriErrorBoundary>`) |
| Browser uncaught error / promise | `source=` (set by JS SDK hooks) |
| `useCaptureError(fn)` | per-call tags as you pass them |

## Edge runtime

`onRequestError` works in both Node and Edge runtimes — Next forwards
the same signature. `serverInit()` is Node-only because Edge lacks
`process.on(...)` for `uncaughtException`.

## Sub-paths

| Import | Use from |
|--------|----------|
| `@goliapkg/sentori-next/client` | App Router client components, `clientInit`, `SentoriProvider`, `<SentoriErrorBoundary>`, hooks |
| `@goliapkg/sentori-next/server` | `instrumentation.ts`, `serverInit`, `onRequestError` |
| `@goliapkg/sentori-next/instrumentation` | one-line `instrumentation.ts` re-export |
| `@goliapkg/sentori-next/app-router` | `useNextRouter`, `useReportNextError` — client-only App Router hooks |

## Versioning

Tracks the underlying SDKs:

- depends on `@goliapkg/sentori-react@0.1.0`
- depends on `@goliapkg/sentori-javascript@0.2.0`
- depends on `@goliapkg/sentori-core@0.1.0`

## License

Dual-licensed under [Apache-2.0](../../LICENSE-APACHE) OR
[MIT](../../LICENSE-MIT).
