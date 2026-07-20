# @goliapkg/sentori-javascript

Vanilla JavaScript / Node SDK for [Sentori](https://sentori.golia.jp) —
error tracking with sub-millisecond startup and a NEVER-burden-host
budget.

## Install

```sh
bun add @goliapkg/sentori-javascript
# npm install @goliapkg/sentori-javascript
```

## Use

```ts
import { sentori } from '@goliapkg/sentori-javascript'

sentori.init({
  token: 'st_pk_…',
  release: 'my-app@1.2.3',
  environment: 'prod',
})

try {
  doWork()
} catch (err) {
  sentori.captureException(err)
}
```

Global `window.onerror` + `unhandledrejection` hooks are wired
automatically; you only need to call `captureException` when you
want to attach extra context (tags / fingerprint / user).

## Sentry compat drop-in

```ts
// Replace `import * as Sentry from '@sentry/browser'` with:
import * as Sentry from '@goliapkg/sentori-javascript/compat'

Sentry.init({ dsn: 'st_pk_…' })
Sentry.captureException(err)
```

→ Full docs: [sentori.golia.jp/docs/getting-started/node](https://sentori.golia.jp/docs/getting-started/node)
→ Sentry translation: [sentori.golia.jp/docs/sentry-compat](https://sentori.golia.jp/docs/sentry-compat)

## License

Dual-licensed under [Apache-2.0](../../LICENSE-APACHE) OR
[MIT](../../LICENSE-MIT).
