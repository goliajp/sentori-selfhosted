# @goliapkg/sentori-solid

SolidJS SDK for [Sentori](https://sentori.golia.jp) —
`SentoriProvider` + an `ErrorBoundary` that flushes the captured
exception with the component tree attached.

## Install

```sh
bun add @goliapkg/sentori-solid @goliapkg/sentori-javascript
```

## Use

```tsx
import { SentoriProvider, SentoriErrorBoundary } from '@goliapkg/sentori-solid'

function Root() {
  return (
    <SentoriProvider
      config={{
        token: 'st_pk_…',
        release: 'my-app@1.2.3',
        environment: 'prod',
      }}
    >
      <SentoriErrorBoundary fallback={(err, reset) => <p>Something broke.</p>}>
        <App />
      </SentoriErrorBoundary>
    </SentoriProvider>
  )
}
```

→ Full guide: [sentori.golia.jp/docs/sdk-solid](https://sentori.golia.jp/docs/sdk-solid)
→ Sentry drop-in: [sentori.golia.jp/docs/sentry-compat](https://sentori.golia.jp/docs/sentry-compat)

## License

Dual-licensed under [Apache-2.0](../../LICENSE-APACHE) OR
[MIT](../../LICENSE-MIT).
