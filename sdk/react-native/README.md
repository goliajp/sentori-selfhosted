# @goliapkg/sentori-react-native

React Native SDK for [Sentori](https://sentori.golia.jp) — JS layer
+ iOS Swift + Android Kotlin native, distributed as an Expo module
(works on bare RN too).

## Install

```sh
bun add @goliapkg/sentori-react-native
cd ios && pod install --repo-update
```

## Use

```tsx
import { sentori } from '@goliapkg/sentori-react-native'

sentori.init({
  token: 'st_pk_…',
  release: 'my-app@1.2.3',
  environment: 'prod',
  capture: {
    replay: { mode: 'wireframe', hz: 1 },
  },
})

sentori.captureException(new Error('boom'))
```

Auto-wired:

- JS `error` / `unhandledrejection` global hooks
- iOS `NSException` handler primed by `init`
- Android uncaught exception handler primed by `init`
- Native screenshot + view-tree capture under JS-supplied mask IDs
- Hang watchdog (main blocked > 2s emits `kind: "anr"`)
- Wireframe replay sampler (60 slots × ~120 bytes/frame at idle)

## Cross-project user lookup (PII-safe)

```ts
sentori.setUser({ linkBy: { email: user.email } })
```

Identity is hashed on-device with a per-org salt. The server never
sees the raw email / phone.

## Sentry compat drop-in

```ts
import * as Sentry from '@goliapkg/sentori-react-native/compat'

Sentry.init({ dsn: 'st_pk_…' })
Sentry.captureException(err)
```

→ Full guide: [sentori.golia.jp/docs/getting-started/react-native](https://sentori.golia.jp/docs/getting-started/react-native)
→ Privacy contract: [sentori.golia.jp/docs/privacy/identity](https://sentori.golia.jp/docs/privacy/identity)
→ Sentry translation: [sentori.golia.jp/docs/sentry-compat](https://sentori.golia.jp/docs/sentry-compat)

## License

Dual-licensed under [Apache-2.0](../../LICENSE-APACHE) OR
[MIT](../../LICENSE-MIT).
