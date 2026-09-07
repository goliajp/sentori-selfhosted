# @goliapkg/sentori-react-native

React Native SDK for [Sentori](https://sentori.golia.jp) — the
self-hosted crash + warning monitor for mobile apps. JS layer +
iOS Swift + Android Kotlin native, distributed as an Expo module
(works on bare RN too).

Eight methods on `sentori` are the whole reporting API — `init`,
`user`, `context`, and the five kinds `error` / `warn` / `trace` /
`assert` / `probe`. The package also exports `ErrorBoundary`,
`launch`, `pushSignal`, `registerMaskQuery`, `useTraceNavigation`,
`RageTapCapture` and `triggerNativeCrash`, which are wiring rather
than reporting.

Upgrading from 4.x? See [MIGRATION.md](./MIGRATION.md).

## The zero-cost contract

Sentori must only ever be a free upgrade for your app:

- **Every call is synchronous, O(1), and can never throw.** Not on
  garbage input, not when the network is down, not when the server
  is gone. You never need a try/catch around a Sentori call.
- `init()` failure (bad token, missing URL) degrades every verb to
  a no-op with one console.warn — never a crash, never a red box.
- Nothing leaves the device until an error/warn actually fires;
  then one batched request carries the event with its context.
- Buffers (signal ring, replay rings, offline queue) are all
  hard-bounded.

## Install

```sh
bun add @goliapkg/sentori-react-native
cd ios && pod install --repo-update
```

## Use

```tsx
import { sentori } from '@goliapkg/sentori-react-native'

sentori.init({
  token: 'st_…',                        // ingest token, Settings → Tokens
  ingestUrl: 'https://sentori.example.com',
  release: 'my-app@1.2.3',
  environment: 'production',            // the DEPLOYMENT environment
  replayScreens: true,                  // opt-in visual replay (v5.1)
  backendHealthUrl: 'https://api.example.com/healthz', // v5.2, see below
})

sentori.user({ id, email, name })        // drives breadth × depth stats
sentori.context({ build: 'release', tenant: 'acme' }) // ambient tags;
                                         // every key becomes a queue
                                         // slicing dimension on the board

sentori.error(new Error('boom'))         // what broke
sentori.warn('pay.gateway-retry', data)  // where users hurt
sentori.trace('checkout.start', data)    // what happened
sentori.assert('total-positive', ok)     // what should hold (never halts)
sentori.probe('BUG-123', data)           // did that bug come back
```

### `init()` options

Every field of `InitConfig`, checked against the type by
`scripts/check-sdk-doc-options.mjs` — a new option cannot ship
undocumented.

| Option | Type | Required | Default |
|---|---|---|---|
| `token` | `string` | yes | — |
| `ingestUrl` | `string` | yes | — |
| `release` | `string` | no | `''` — set it, or stacks cannot symbolicate |
| `environment` | `string` | no | `''` |
| `detect` | `{ rageTap?, longFreeze?, slowColdStart?, slowApi? }` | no | rageTap / longFreeze / slowColdStart on, `slowApi` off |
| `replaySeconds` | `number` | no | 60 |
| `replayScreens` | `boolean` | no | `false` — opt-in; frames may hold user content |
| `backendHealthUrl` | `string` | no | — |
| `logLevel` | `'debug' \| 'error' \| 'info' \| 'silent' \| 'warn'` | no | `'warn'` |
| `beforeSend` | `(event) => event \| null` | no | — return `null` to drop |

Auto-wired (no configuration):

- JS `error` / `unhandledrejection` global hooks
- iOS `NSException` + Android uncaught-exception handlers
- Warn scenario detectors: rage taps, long freezes, slow cold start
  (slow API stays opt-in) — tune with `init({ detect })`
- Signal ring: the last 60 s of taps (with coordinates), navigation,
  http and traces ride along on every error/warn as the "what the
  user was doing" timeline
- B-type replay: rolling wireframe buffer, shipped only when an
  error/warn actually fires (`init({ replaySeconds })`); opt-in
  pixel replay via `replayScreens: true`, with native-side masking
  (`registerMaskQuery`) so tagged views never exist in any frame
- Cold-start measurement with a pre-warm guard: processes started
  in the background (FCM, JobScheduler, iOS prewarming) are flagged
  and never counted as slow starts

Identity is hashed on-device with a salted hash — the server never
sees the raw email.

## Launch measurement (v5.6)

One number ending at `init()` cannot represent user-perceived
launch. Stage it instead:

```tsx
sentori.launch.mark('bootstrap')     // optional waypoints, anywhere
sentori.launch.mark('first-tree')
sentori.launch.complete()            // when YOUR app counts as usable
```

`complete()` emits one `app.launch` trace with segment durations
(native span → each waypoint → complete). The dashboard's
Instruments page aggregates p50/p90/p95 per release from it. Apps
that never call the marks keep the plain cold-start behaviour.

## Custom breadcrumbs (v5.5)

Feed your own context into the signal ring — same fire-and-forget
guarantees as the verbs:

```tsx
import { pushSignal } from '@goliapkg/sentori-react-native'

// in your API interceptor — the one place that knows which
// requests matter (Sentori deliberately does not patch fetch/XHR):
pushSignal('http', { method, url, status, ms })  // status 0 = no response
```

`http` entries render as request lines on the case timeline; any
other kind renders with its data as key=value pairs.

## Backend availability (v5.2)

`init({ backendHealthUrl })` — the URL rides along with event
batches (the app itself never pings anything); the Sentori server
probes it once a minute and shows uptime + latency on the project
card.

## React extras

```tsx
import {
  ErrorBoundary,        // React idiom for the error verb
  RageTapCapture,       // wrap your root to feed the rage-tap detector
  useTraceNavigation,   // pass your react-navigation ref
  registerMaskQuery,    // nativeIDs to black out in every captured frame
} from '@goliapkg/sentori-react-native'
```

## Build pipeline

`@goliapkg/sentori-cli` uploads sourcemaps, dSYMs, Proguard maps
and native source bundles (failures never block a release —
friendly notice, exit 0) and registers `probe()` tripwires per
release. See MIGRATION.md §6.

## License

Dual-licensed under [Apache-2.0](../../LICENSE-APACHE) OR
[MIT](../../LICENSE-MIT).
