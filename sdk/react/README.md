# @goliapkg/sentori-react

React 18 / 19 web SDK for [Sentori](https://sentori.golia.jp) —
`SentoriProvider`, `SentoriErrorBoundary`, and a Suspense-aware
fallback wrapper.

## Install

```sh
bun add @goliapkg/sentori-react @goliapkg/sentori-javascript
```

## Use

```tsx
import { SentoriProvider, SentoriErrorBoundary } from '@goliapkg/sentori-react'

function Root() {
  return (
    <SentoriProvider
      config={{
        token: 'st_pk_…',
        release: 'my-app@1.2.3',
        environment: 'prod',
      }}
    >
      <SentoriErrorBoundary fallback={<p>Something broke.</p>}>
        <App />
      </SentoriErrorBoundary>
    </SentoriProvider>
  )
}
```

Errors thrown in render or in effect cleanup get captured with the
React component stack attached.

→ Full guide: [sentori.golia.jp/docs/getting-started/react](https://sentori.golia.jp/docs/getting-started/react)
→ Sentry drop-in: [sentori.golia.jp/docs/sentry-compat](https://sentori.golia.jp/docs/sentry-compat)

## License

Dual-licensed under [Apache-2.0](../../LICENSE-APACHE) OR
[MIT](../../LICENSE-MIT).
