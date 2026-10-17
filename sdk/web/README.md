# @goliapkg/sentori-web

The Sentori SDK for the browser. One package, no framework adapters:
a React app and a Svelte app make the same five calls.

```bash
bun add @goliapkg/sentori-web
```

```ts
import { sentori } from '@goliapkg/sentori-web'

sentori.init({
  token: 'st_…',
  ingestUrl: 'https://sentori.example.com',
  release: 'web@1.4.0+220',
  environment: 'production',
})
```

That is the whole setup. `init` installs uncaught-error and
unhandled-rejection listeners, breadcrumbs, Web Vitals and a session
ping, and none of it replaces anything the page already had — every
listener is added with `addEventListener`, never by assigning
`window.onerror`.

## The five kinds

```ts
sentori.error(err, { cartId })        // something broke
sentori.warn('checkout.slow', { ms }) // something is wrong but survivable
sentori.trace('cart.opened')          // a breadcrumb
sentori.assert('total.positive', ok)  // an invariant; passes aggregate
sentori.probe('SEN-482')              // a tripwire: reaching it is the signal
```

Each returns the event id the server will file it under. None of them
throws, and all of them are no-ops before `init` — including if `init`
itself failed.

## Identity

```ts
sentori.user({ id: 'usr_123', email: null })
sentori.context({ tenant: 'acme' })
```

Only a salted hash of `id` (or `email`) travels. The raw values stay in
the page.

## Options

| option | type | default |
|---|---|---|
| `token` | `string` | required |
| `ingestUrl` | `string` | required |
| `release` | `string` | `''` |
| `environment` | `string` | `'production'` |
| `detect.uncaught` | `boolean` | `true` |
| `detect.breadcrumbs` | `boolean` | `true` |
| `detect.webVitals` | `boolean` | `true` |
| `replayScreens` | `boolean` | `false` |
| `beforeSend` | `(e) => e \| null` | — |
| `logLevel` | `'silent' \| 'error' \| 'warn' \| 'info' \| 'debug'` | `'silent'` |
| `backendHealthUrl` | `string` | — |

## Session replay

Off by default. Turn it on and register what to hide:

```ts
import { registerMaskQuery, sentori } from '@goliapkg/sentori-web'

sentori.init({ /* … */ replayScreens: true })
registerMaskQuery('.card-number', '[data-private]')
```

It records a **wireframe**, not the DOM and not pixels: every visible
element's box, what it is (text / image / input / button), its
background colour, and for text the number of characters. The text
itself never leaves the page, nor does an image's `src`, an input's
value, or any attribute. A masked subtree keeps its box and
contributes nothing else — not even a length, which would leak how
much was written there.

It is the same format the iOS and Android SDKs produce, so it plays in
the same viewer. That is also why it is not rrweb: rrweb records the
DOM, which is a different recording, a different player and a far
larger payload.

The ring holds the last sixty seconds in memory and is drained onto an
`error` or `warn` as an attachment. A page where nothing goes wrong
sends none of it.

## What it cannot do

**Default request breadcrumbs.** Resource timing exposes duration and size,
but HTTP status visibility depends on the browser. The default SDK does not
patch browser requests. Optional `networkErrors` in the performance entry
adds bounded fetch/XHR connectivity and HTTP 5xx reporting; it preserves the
original response or rejection and never reads response bodies.

To record a status, say so:

```ts
const res = await fetch(url)
sentori.trace('http', { url, status: res.status }, { quiet: true })
```

**Cross-origin script errors.** A `<script>` from another origin
without `crossorigin="anonymous"` gives the browser's opaque
`Script error.` with no stack and no file. Those are reported, marked
`opaque`, rather than dropped — dropping them makes an error budget
look healthier than it is. Add `crossorigin="anonymous"` to the tag and
`Access-Control-Allow-Origin` on the asset to get the real stack.

**Storage.** The offline queue uses `localStorage`. In a private
window, or where the person has blocked site data, there is none: the
SDK works and a failed batch is counted as dropped rather than kept.

## Your server must allow the origin

The ingest endpoint answers browser preflights out of the box on
Sentori 3.20 and later. On an older self-hosted server there is no
CORS, and every event from a page is refused before it is sent —
visibly in the page's console, invisibly to the server. Upgrade the
server before pointing a browser at it.

## Cost

The bundle is measured on every build, gzipped, the way a browser
receives it. The gate fails over 25 KiB for either the default entry or a
bundle combining it with the optional performance entry.

The main thread is measured too: a run that makes 550 SDK calls must
produce no `longtask` entry. That check runs in a real Chrome on every
build, not as a claim in this file.

## License

Apache-2.0 OR MIT

## Optional real-user performance

The separate `@goliapkg/sentori-web/performance` entry requires a server with
Sentori 5.2.0 or later and a registered Web App. Set `app.identifier` in
`sentori.init` to that app's exact identifier before starting the collector.

```ts
import { startWebMonitoring } from '@goliapkg/sentori-web/performance'

const monitoring = startWebMonitoring({
  sampleRate: 0.1,
  route: () => '/orders/:id',
  resourceErrors: true,
  networkErrors: true,
})

// The host router decides when its next screen is usable.
const navigationStarted = performance.now()
// After the screen becomes ready:
monitoring.routeReady('/orders/:id', navigationStarted)
// On consent withdrawal:
// monitoring.stop()
```

Collection is off until this entry is imported and started. `sampleRate` is
required and must be greater than zero and at most one. One sampling decision
covers each document or restored visit, including healthy visits. Official
`web-vitals` measurements report LCP, INP, CLS, FCP and TTFB; SPA `routeReady`
is a separate host-defined measurement. Unsupported values and lack of user
interaction remain missing, not zero. Never put account IDs or query strings
in route templates.

Resource and network error reporting are separately opt-in. Network reporting
captures connectivity failures and HTTP 5xx, ignores aborts, 4xx, opaque
responses and Sentori requests, and does not read bodies. Each source is capped
at ten reported errors per page. Ordinary JavaScript errors keep using the
existing error pipeline and exact-release source maps.

The pending visit queue is bounded to twenty records and 48 KiB, with an
optional local-storage copy expiring after one day. Uploads run at most every
thirty seconds, back off on failure, and attempt a final lifecycle flush;
browser shutdown can still lose data. `stop()` clears this collector's queue,
listeners, network patches and transport. The official metrics library retains
bounded document lifecycle observers; its callbacks stop reporting. A stopped
controller cannot be restarted in the same document.
