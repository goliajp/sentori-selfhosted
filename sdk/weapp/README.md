# @goliapkg/sentori-weapp

The Sentori SDK for WeChat mini programs.

```bash
npm install @goliapkg/sentori-weapp
```

```js
// app.js
import { sentori } from '@goliapkg/sentori-weapp'

App({
  onLaunch() {
    sentori.init({
      token: 'st_…',
      ingestUrl: 'https://sentori.your-domain.cn',
      release: 'weapp@1.4.0+220',
      environment: 'production',
    })
  },
})
```

## Your ingest URL must be a filed domain

WeChat only lets a mini program call domains on its request allowlist,
and a domain can only go on that list if it has an ICP filing (备案).
`sentori.golia.jp` cannot be added, so **the only way to use Sentori
from a mini program is an instance you host on a filed domain in
mainland China**. That is a property of the platform, not a setting in
this SDK.

Add your ingest URL under 开发管理 → 开发设置 → 服务器域名 → request
合法域名 before shipping. Until it is there, every event fails with
`request:fail url not in domain list` and nothing reaches the server.

## The five kinds

```js
sentori.error(err, { cartId })
sentori.warn('checkout.slow', { ms })
sentori.trace('cart.opened')
sentori.assert('total.positive', ok)
sentori.probe('SEN-482')
```

Same five verbs, same signatures and same wire as the React Native,
Swift, Kotlin and browser SDKs. A team shipping an app and a mini
program learns one vocabulary.

## What it captures on its own

- `wx.onError` — uncaught errors. The platform hands these over as one
  flattened **string**, so the type, the message and the stack are
  parsed back out of it. Without that every crash in your mini program
  would arrive under one title called `Error`.
- `wx.onUnhandledRejection`
- `wx.onPageNotFound` — a route that is not in the build
- `wx.onMemoryWarning` — the system asking for memory back
- `wx.onLazyLoadError` — a subpackage that would not download

The last three have no equivalent in any other Sentori SDK. They are
real user-facing failures here and invisible everywhere else.

## Requests

`wx.request` is wrapped, because there is no `PerformanceObserver` and
no other way to see a request. Unlike the browser SDK, the **status
code is real** — `wx.request` hands it back.

The wrapper calls the original first and returns its value unchanged,
and our work happens inside your callbacks wrapped in a try/catch. If
our patch breaks, `wx.request` behaves exactly as it did.

Call `init` before anything else takes its own reference to
`wx.request`, or that copy stays unwrapped.

## Identity

```js
sentori.user({ id: user.openid, email: null })
```

Only a salted SHA-256 of the id travels. There is no `crypto.subtle`
in this runtime, so the digest is computed in plain JavaScript — and
held to `crypto.subtle`'s own answers on a thousand random strings,
because a hash that is only self-consistent turns one person into two
across your app and your mini program.

## Storage

The offline queue uses `wx.setStorageSync`. A single key is capped at
1 MB by the platform and the whole store at 10 MB; the queue is capped
well under that and a write that fails counts the batch as dropped
rather than throwing on a path you cannot see.

## Cost

16.9 KB raw, measured on every build. Raw, not gzipped — WeChat's
2 MB main-package limit counts uncompressed bytes, so a gzipped number
would be one the platform does not use. The gate fails over 100 KB.

## What is not checked

The gate drives this SDK against a real server through a `wx` shim:
real HTTP, real storage semantics, the handlers called by hand. It
does **not** run inside WeChat's own JavaScript engine — that needs a
phone or the devtools, and no CI has either.

## License

Apache-2.0 OR MIT
