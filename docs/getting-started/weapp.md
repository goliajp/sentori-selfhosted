---
title: Getting started — WeChat mini program
description: Sentori in a mini program, and the one platform constraint you cannot work around
---

# WeChat mini program

## Before anything else: the domain

WeChat only lets a mini program call domains on its request
allowlist, and a domain only gets on that list with an ICP filing
(备案). `sentori.golia.jp` cannot be added.

**So a mini program can only report to a Sentori instance you host on
a filed domain in mainland China.** There is no flag that changes
this, and no proxy that avoids it — the allowlist is checked by the
platform, not by the network. [Self-hosting](../self-hosting.md) is
the whole of the answer.

Add your ingest URL under 开发管理 → 开发设置 → 服务器域名 → request
合法域名 first. Until it is there every request fails with
`request:fail url not in domain list`.

## 1. Install

```bash
npm install @goliapkg/sentori-weapp
```

Then 工具 → 构建 npm in the devtools, and `"usingComponents"` aside,
nothing else changes.

## 2. Initialise in `onLaunch`

```js
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

Early, and before anything else takes a reference to `wx.request` —
the request breadcrumbs wrap whatever is there when `init` runs, so a
copy taken earlier stays unwrapped.

## 3. Identify the person

```js
wx.login({
  success: ({ code }) => {
    // … exchange the code with your own backend for an openid
    sentori.user({ id: openid, email: null })
  },
})
```

Only a salted SHA-256 travels. This runtime has no `crypto.subtle`,
so the digest is computed in plain JavaScript and checked against
`crypto.subtle`'s own answers, so the same person is one person
across your app and your mini program.

## 4. Report what you catch

```js
try {
  await pay()
} catch (err) {
  sentori.error(err, { orderId })
}
```

Uncaught errors, unhandled rejections, a missing page, a memory
warning and a subpackage that would not load are all captured without
a call.

## What you get that no other platform has

| Handler | What it means |
|---|---|
| `onPageNotFound` | someone reached a route that is not in the build |
| `onMemoryWarning` | the system is asking for memory back, and may kill you next |
| `onLazyLoadError` | a subpackage did not download, so a screen never appeared |

These are reported as `warn` with the names `weapp.pageNotFound`,
`weapp.memoryWarning` and `weapp.lazyLoadError`.

## Source maps

The devtools produce them per build. Upload per release:

```bash
export SENTORI_API_URL=https://sentori.your-domain.cn   # YOUR instance
export SENTORI_TOKEN=st_...

npx @goliapkg/sentori-cli@latest upload sourcemap \
  --api-url "$SENTORI_API_URL" \
  --release "weapp@1.4.0+220" \
  --token "$SENTORI_TOKEN" \
  ./dist
```

Every page's entry file in a mini program is called `index.js`, so the
server matches maps by how much of the **path** they share with a
frame, not by filename. Point the CLI at the build directory and it
keeps that structure: `dist/pages/cart/index.js.map` is stored as
`pages/cart/index.js.map`, which is what makes two pages
distinguishable.

## Cost

16.9 KB raw. Raw rather than gzipped, because the 2 MB main-package
limit counts uncompressed bytes. If that matters at your size, the
package can go in a subpackage.

## Next

- [SDK reference](../../sdk/weapp/README.md)
- [Self-hosting](../self-hosting.md) — required here, not optional
