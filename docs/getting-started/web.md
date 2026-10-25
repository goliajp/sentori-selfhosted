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

Stable `app.identifier` configuration requires browser SDK **1.1.0 or later**.

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
  app: { identifier: 'admin-panel' },
  environment: import.meta.env.MODE,
})
```

`app.identifier` is the stable browser app identity within the project; keep it
unchanged across releases. For a project containing both an API and a web UI,
use separate identifiers such as `api` and `admin-panel`. The configured App type
is Web API or Web App; browser telemetry still uses platform `web`.

The token belongs in the bundle. It only grants ingest, and every
browser SDK works this way — there is nothing to hide, and hiding it
behind a proxy costs you the origin of the report.

`init` installs:

- `error` and `unhandledrejection` listeners, both with
  `addEventListener` so whatever you already had keeps working
- click, navigation and request breadcrumbs, into a sixty-second ring
  that ships only when something goes wrong
- browser timing context in the error signal ring (LCP candidates, cumulative layout shifts and slow interactions); this is not a standards-compliant Core Web Vitals dashboard
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

### Filter expected permission failures

`init({ beforeSend })` can return `null` to drop an event. For an application
that intentionally treats a denied clipboard permission as an expected outcome:

```ts
sentori.init({
  token: import.meta.env.VITE_SENTORI_TOKEN,
  ingestUrl: import.meta.env.VITE_SENTORI_INGEST_URL,
  release: `web@${__APP_VERSION__}`,
  beforeSend(event) {
    const error = event.payload.error
    if (event.kind === 'error' && error && !error.stack?.length &&
        error.type === 'NotAllowedError' && /clipboard/i.test(error.message)) {
      return null
    }
    return event
  },
})
```

Match the actual type and message your browser reports. Keep the filter narrow:
a TypeError caused by missing `navigator.clipboard` is a separate application
error. The hook is synchronous; throwing or returning a Promise does not drop
the event. Handle expected permission denial at the clipboard call site when
possible so the application can show useful feedback.

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

**Default request breadcrumbs.** Resource timing exposes duration and size;
HTTP status visibility depends on the browser. The optional performance
collector's `networkErrors` setting adds bounded fetch/XHR connectivity and
HTTP 5xx reporting without reading bodies. It is disabled by default.

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

A 25 KiB gzipped budget covers the default bundle and the combined optional
performance bundle, measured on every build. A run of 550 SDK calls in a
real Chrome produces no long task — also measured on every build,
rather than promised here.

## Next

- [SDK reference](../../sdk/web/README.md)
- [Self-hosting](../self-hosting.md)
- [Protocol](../protocol.md)

## Real-user performance and browser checks

With Sentori 5.2.0 or later and Web SDK 1.2.0 or later, create a Web App with an explicit identifier in the intended project, then set
`app: { identifier: 'admin-panel' }` in `sentori.init`. Import the optional
performance entry and choose a sampling rate as shown in the
[SDK reference](../../sdk/web/README.md#optional-real-user-performance).
The dashboard's Web monitoring screen separates sampled real-user performance,
correlated JavaScript errors and isolated browser checks.

Percentiles require at least twenty metric samples and are separated by mobile
and desktop. Healthy sampled visits contribute to the error-rate denominator.
A missing metric, an unobserved interaction or insufficient samples is not a
successful measurement. SPA readiness is distinct from document LCP and INP.

Browser checks need an operator-installed runner. Configure the exact URL,
required public resource origins, a visible readiness selector and optional
click/wait/text assertions. Public policy checks rendered and raw metadata,
robots directives, canonical links, sitemap coverage and JSON-LD syntax.
Private policy checks anonymous protection and noindex; an operator may supply
a scoped test-account profile. These checks do not establish Google indexing
or search ranking. Lighthouse results are laboratory measurements: TBT is not
INP and laboratory LCP is not a real-user percentile.

Scheduled execution and alerts start disabled. Enable recurrence explicitly
(minimum fifteen minutes). Alerts require consecutive failed checks; runner,
network-policy and expired-authentication failures are monitoring errors, not
confirmed site outages. Existing incidents recover only after a passing check.
