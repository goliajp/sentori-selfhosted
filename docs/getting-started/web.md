---
title: Getting started — browser
description: Sentori in a web page, in one call
---

# Browser

One package for every framework. A React app, a Svelte app and a page
with no framework at all make the same five calls, so there is nothing
to pick.

## Prerequisites

- A Sentori **token** (`st_…`, scope `ingest`) and an **ingest URL** —
  see the [getting-started overview](../getting-started.md).
- A Sentori server on **4.0.0 or later**. Older servers send no CORS
  headers, so a browser refuses the request before it is made and
  reports it only to the page's console.

## 1. Install

```bash
bun add @goliapkg/sentori-web
# or
npm install @goliapkg/sentori-web
```

## 2. Initialise

As early as your entry file allows — anything that throws before this
runs is not reported, because nothing is listening yet.

```ts
import { sentori } from '@goliapkg/sentori-web'

sentori.init({
  token: import.meta.env.VITE_SENTORI_TOKEN,
  ingestUrl: import.meta.env.VITE_SENTORI_INGEST_URL,
  release: `web@${__APP_VERSION__}`,
  environment: import.meta.env.MODE,
})
```

The token belongs in the bundle. It only grants ingest, and every
browser SDK works this way — there is nothing to hide, and hiding it
behind a proxy costs you the origin of the report.

`init` installs:

- `error` and `unhandledrejection` listeners, both with
  `addEventListener` so whatever you already had keeps working
- click, navigation and request breadcrumbs, into a sixty-second ring
  that ships only when something goes wrong
- LCP, CLS and INP from the browser's own measurements
- a session per page view, ended when the page is hidden

## 3. Report something

```ts
try {
  await checkout()
} catch (err) {
  sentori.error(err, { cartId })
}
```

Uncaught errors need no call at all. The explicit verb is for the ones
you catch and still want to know about.

## 4. Identify the person

```ts
sentori.user({ id: user.id, email: null })
sentori.context({ tenant: 'acme', plan: 'pro' })
```

Only a salted hash travels. `context` is for attributes you want to
slice by; nothing there is hashed, so keep identities out of it.

## 5. Source maps

Upload them per release so a minified stack becomes a readable one:

```bash
export SENTORI_API_URL=https://sentori.example.com   # YOUR instance
export SENTORI_TOKEN=st_...

npx @goliapkg/sentori-cli@latest upload sourcemap \
  --api-url "$SENTORI_API_URL" \
  --release "web@1.4.0" \
  --token "$SENTORI_TOKEN" \
  ./dist
```

The `--release` here and the `release` you pass to `init` must be the
same string. They are matched literally.

Late uploads are not wasted — the server re-reads events already
stored for that release.

## Session replay (optional)

```ts
import { registerMaskQuery, sentori } from '@goliapkg/sentori-web'

sentori.init({ /* … */ replayScreens: true })
registerMaskQuery('.card-number', '[data-private]')
```

A wireframe, not a screenshot: boxes, what each one is, its colour,
and for text how many characters. No words, no image sources, no input
values. A masked subtree is a plain rectangle with nothing inside it.

Held in memory for sixty seconds and attached to an `error` or `warn`.
Nothing leaves the page otherwise.

## What you do not get

**Status codes on request breadcrumbs.** They come from
`PerformanceObserver`, which records a URL, a duration and a size, and
no HTTP status. Chromium exposes `responseStatus`; no other engine
does. Reporting one requires our code to sit in the path of every
request your page makes, and a bug there would break your app rather
than our reporting.

If you want the status, say so where you already have it:

```ts
const res = await fetch(url)
sentori.trace('http', { url, status: res.status }, { quiet: true })
```

**Stacks from cross-origin scripts.** Without
`crossorigin="anonymous"` on the tag and
`Access-Control-Allow-Origin` on the asset, the browser gives
`Script error.` and nothing else. Those still arrive, marked `opaque`,
because dropping them would make your error count look better than it
is.

## Cost

9.5 KB gzipped, measured on every build. A run of 550 SDK calls in a
real Chrome produces no long task — also measured on every build,
rather than promised here.

## Next

- [SDK reference](../../sdk/web/README.md)
- [Self-hosting](../self-hosting.md)
- [Protocol](../protocol.md)
