# @goliapkg/sentori-core

## 3.0.0

### Major Changes

- [`f1f1543`](https://github.com/goliajp/sentori-selfhosted/commit/f1f154344d7ff3ca07ba94cde21fa7c944eea3af) - The push API is one shape now, and the SDK calls it.

  `/v1/push/tokens` is `/v1/push/devices` — four things in this system
  are called a token and the thing being registered is a device. The
  register response is `{ spToken, isNew }`; `token_id` and `is_new` are
  gone. The ack posts to `/v1/push/deliveries/{deliveryId}/ack`, because
  the id it carries is one device's row and not the call that produced
  it.

  Every field on `/v1`, in both directions, is camelCase.

## 2.3.0

### Minor Changes

- [`d6d7698`](https://github.com/goliajp/sentori-selfhosted/commit/d6d7698e361e2d10735f66b2580add4574fb9d06) - `user()` takes traits, and a registered device follows the person.

  Registering for push and signing in happen in that order in every app
  with a login screen, and nothing updated the device row in between —
  so the row held no user for the life of the install, and a send aimed
  at that person matched no device and reported success. The device now
  re-registers by itself when the identity changes, only when it
  actually changed, and only for a device that had registered.

  `user({ id, traits })` carries attributes a push campaign can select
  on. They travel raw beside the hashed identity, so the identity stays
  unreadable and the selection stays possible; a call describes the
  person completely, so signing out clears them.

## 1.4.0

### Minor Changes

- [`8d07add`](https://github.com/goliajp/sentori-selfhosted/commit/8d07add988d737b7699299c26e3712c444660ca9) Thanks [@doracawl](https://github.com/doracawl)! - v2.26 — RN SDK auto-correlation pipeline.

  - New `BreadcrumbType` `'push'` for the auto-emitted breadcrumb when the SDK
    receives a push carrying `_sentori.msgId` (server injects in v2.25+).
    Same union shape on the server (`server/src/event.rs` `BreadcrumbType::Push`).
  - RN drain loop, on a notification with `_sentori.msgId`:
    - writes a `{ type: 'push', data: { msgId, title, body, opened, provider } }`
      breadcrumb so a later `captureException` shows the push that just arrived.
    - emits `sentori.push.received` (or `sentori.push.opened` for taps) through
      the existing track pipeline.
    - enqueues an ack POST to `/v1/push/sends/<msgId>/ack` — flushed every 5s
      in the background. Server-side `push_sends.acked_at` flips on first ack.
  - `sentori.push.setSessionContext(sessionId)` stamps the host's current
    session id on outgoing acks for v2.27 push×session BI correlation.
  - Zero host-app code change required to opt in. Payloads without
    `_sentori.msgId` (older server, non-Sentori sender) flow through
    unchanged — no breadcrumb, no track, no ack.

- [`cb1870e`](https://github.com/goliajp/sentori-selfhosted/commit/cb1870ebc23e515d3d94775536cf2dba2b406be3) Thanks [@doracawl](https://github.com/doracawl)! - v2.28 — Push rich-media (image) support.

  - New wire field `richMedia.imageUrl` on `/v1/push/send`. When set:
    - **Android (FCM):** server writes `message.notification.image`,
      FCM auto-renders the Android BigPicture style. Zero device-side
      work required.
    - **iOS (APNs):** server forces `aps.mutable-content: 1` and
      surfaces the URL under the reserved `sentori_attachment_url`
      key for a Notification Service Extension to download + attach.
    - **Web Push:** passes through under `data.sentori_attachment_url`
      for the host's Service Worker to use as `options.image`.
  - The Sentori Expo plugin now writes a minimal NSE Swift template +
    `Info.plist` to `ios/SentoriNSE/` on every `expo prebuild`. The
    one-time Xcode target wiring is documented in the recipe; the
    template downloads the URL with a 5 s timeout + attaches.
  - Opt out per-platform / per-template with `{ ios: false }` /
    `{ nse: false }` in `app.json` plugin props.
  - Legacy customers (no `richMedia` field) see identical v2.27
    behaviour. Hosts without the NSE target installed still receive
    the text-only notification — `mutable-content:1` is harmless when
    no extension is registered.

## 1.3.1

### Patch Changes

- [`5f0fa5b`](https://github.com/goliajp/sentori-selfhosted/commit/5f0fa5b525158cf024b99fa952716b19f1e447f1) Thanks [@doracawl](https://github.com/doracawl)! - fix(sdk): never emit `console.error` from runtime paths — host apps reading red `[sentori]` lines mistake them for their own app crashing and pull Sentori out. Downgraded to `console.warn`:

  - `sentori-core`: `logger.error(...)` now routes to `console.warn` in the default console emit path. Host-supplied log transports still receive the real `error` level so they can route it to their aggregator however they like.
  - `sentori-react`: `SentoriProvider` init-failure catch block.
  - `sentori-next`: `clientInit` and `serverInit` failure catch blocks.

  The runtimeMetrics flush-failure channel that triggered the original report runs through `reportInternal → logger.error → console.error` in the host's runtime; the `sentori-core` fix closes the channel at the source for every downstream SDK.

## 1.3.0

### Minor Changes

- [`d8e38b2`](https://github.com/goliajp/sentori-selfhosted/commit/d8e38b222a4a5b1d76362514e637bc996c805cc2) Thanks [@doracawl](https://github.com/doracawl)! - v2.8 — Web Push opt-in, Service Worker subscription, server-side send for Next.js.

  Second phase of the v2.7→v2.12 Push rollout. v2.7 shipped the
  server foundation (provider trait, dispatch cron, secrets-sealed
  credentials, `/v1/push/*` routes); v2.8 lights up the **Web Push**
  branch end-to-end:

  **`@goliapkg/sentori-core` (minor) — types**

  Adds `PushMessage`, `PushOptions`, `PushPriority`, `PushTicket`,
  `PushTicketStatus`, `PushReceipt`. These mirror the
  `/v1/push/send` wire shape and are re-exported from
  `@goliapkg/sentori-javascript` + `@goliapkg/sentori-next` so the
  matrix shares one canonical contract.

  **`@goliapkg/sentori-javascript` (minor) — browser registration**

  New `sentori.push.registerWeb({ vapidPublicKey, ... })` that walks
  the standard browser opt-in path:

  1. `Notification.requestPermission()`
  2. `navigator.serviceWorker.register(serviceWorkerUrl)` (default `/sentori-sw.js`)
  3. `pushManager.subscribe({ userVisibleOnly: true, applicationServerKey })`
  4. POST the subscription JSON to the Sentori ingest
     `/v1/push/tokens`, getting back an `ipt_*` device handle.
  5. Cache the handle in `localStorage` + bind Service Worker
     `postMessage` events to optional `onMessage` / `onTap`
     callbacks.

  `unregisterWeb()` does the reverse — DELETE the handle + unsubscribe
  locally. Both are no-ops when the browser doesn't support the Push API.

  **Default off** — the host app calls `registerWeb` when ready.
  Sentori never triggers a permission prompt on its own. Same opt-in
  principle as `trackAutoBreadcrumb`.

  **`@goliapkg/sentori-next` (minor) — server-side send**

  New `sentoriPush({ ingestUrl, token })` factory at
  `@goliapkg/sentori-next/push`. Returns a `{ send, sendBatch,
getReceipt, isSentoriPushToken }` client that wraps `/v1/push/send`
  and `/v1/push/receipts/{id}` with the Sentori-native wire shape.
  Pure `fetch`, no Node-only imports — safe under `runtime: 'edge'`
  in App Router server actions + middleware.

  `sendBatch` concurrency-caps at 8 parallel HTTP calls to keep the
  Sentori dispatcher's queue healthy on big fan-outs.

  **Recipe**

  `docs-site` gains `recipes/push-from-nextjs.md` — end-to-end walk
  through: VAPID key pair generation, admin REST upload of the
  encrypted credentials, Service Worker template, `'use client'`
  register flow, App Router server action send, and a troubleshooting
  matrix mapping push-server status codes to operator action.

  **Compatibility**

  Wire shape is unchanged from v2.7. Customers using raw REST against
  `/v1/push/*` keep working without code changes. The Sentori-native
  wire shape `PushMessage` and the Expo-compat endpoints both stay byte-
  compatible.

## 1.3.0

### Minor Changes

- [`d8e38b2`](https://github.com/goliajp/sentori-selfhosted/commit/d8e38b222a4a5b1d76362514e637bc996c805cc2) Thanks [@doracawl](https://github.com/doracawl)! - v2.8 — Web Push opt-in, Service Worker subscription, server-side send for Next.js.

  Second phase of the v2.7→v2.12 Push rollout. v2.7 shipped the
  server foundation (provider trait, dispatch cron, secrets-sealed
  credentials, `/v1/push/*` routes); v2.8 lights up the **Web Push**
  branch end-to-end:

  **`@goliapkg/sentori-core` (minor) — types**

  Adds `PushMessage`, `PushOptions`, `PushPriority`, `PushTicket`,
  `PushTicketStatus`, `PushReceipt`. These mirror the
  `/v1/push/send` wire shape and are re-exported from
  `@goliapkg/sentori-javascript` + `@goliapkg/sentori-next` so the
  matrix shares one canonical contract.

  **`@goliapkg/sentori-javascript` (minor) — browser registration**

  New `sentori.push.registerWeb({ vapidPublicKey, ... })` that walks
  the standard browser opt-in path:

  1. `Notification.requestPermission()`
  2. `navigator.serviceWorker.register(serviceWorkerUrl)` (default `/sentori-sw.js`)
  3. `pushManager.subscribe({ userVisibleOnly: true, applicationServerKey })`
  4. POST the subscription JSON to the Sentori ingest
     `/v1/push/tokens`, getting back an `ipt_*` device handle.
  5. Cache the handle in `localStorage` + bind Service Worker
     `postMessage` events to optional `onMessage` / `onTap`
     callbacks.

  `unregisterWeb()` does the reverse — DELETE the handle + unsubscribe
  locally. Both are no-ops when the browser doesn't support the Push API.

  **Default off** — the host app calls `registerWeb` when ready.
  Sentori never triggers a permission prompt on its own. Same opt-in
  principle as `trackAutoBreadcrumb`.

  **`@goliapkg/sentori-next` (minor) — server-side send**

  New `sentoriPush({ ingestUrl, token })` factory at
  `@goliapkg/sentori-next/push`. Returns a `{ send, sendBatch,
getReceipt, isSentoriPushToken }` client that wraps `/v1/push/send`
  and `/v1/push/receipts/{id}` with the Sentori-native wire shape.
  Pure `fetch`, no Node-only imports — safe under `runtime: 'edge'`
  in App Router server actions + middleware.

  `sendBatch` concurrency-caps at 8 parallel HTTP calls to keep the
  Sentori dispatcher's queue healthy on big fan-outs.

  **Recipe**

  `docs-site` gains `recipes/push-from-nextjs.md` — end-to-end walk
  through: VAPID key pair generation, admin REST upload of the
  encrypted credentials, Service Worker template, `'use client'`
  register flow, App Router server action send, and a troubleshooting
  matrix mapping push-server status codes to operator action.

  **Compatibility**

  Wire shape is unchanged from v2.7. Customers using raw REST against
  `/v1/push/*` keep working without code changes. The Sentori-native
  wire shape `PushMessage` and the Expo-compat endpoints both stay byte-
  compatible.

## 1.3.0

### Minor Changes

- [`d8e38b2`](https://github.com/goliajp/sentori-selfhosted/commit/d8e38b222a4a5b1d76362514e637bc996c805cc2) Thanks [@doracawl](https://github.com/doracawl)! - v2.8 — Web Push opt-in, Service Worker subscription, server-side send for Next.js.

  Second phase of the v2.7→v2.12 Push rollout. v2.7 shipped the
  server foundation (provider trait, dispatch cron, secrets-sealed
  credentials, `/v1/push/*` routes); v2.8 lights up the **Web Push**
  branch end-to-end:

  **`@goliapkg/sentori-core` (minor) — types**

  Adds `PushMessage`, `PushOptions`, `PushPriority`, `PushTicket`,
  `PushTicketStatus`, `PushReceipt`. These mirror the
  `/v1/push/send` wire shape and are re-exported from
  `@goliapkg/sentori-javascript` + `@goliapkg/sentori-next` so the
  matrix shares one canonical contract.

  **`@goliapkg/sentori-javascript` (minor) — browser registration**

  New `sentori.push.registerWeb({ vapidPublicKey, ... })` that walks
  the standard browser opt-in path:

  1. `Notification.requestPermission()`
  2. `navigator.serviceWorker.register(serviceWorkerUrl)` (default `/sentori-sw.js`)
  3. `pushManager.subscribe({ userVisibleOnly: true, applicationServerKey })`
  4. POST the subscription JSON to the Sentori ingest
     `/v1/push/tokens`, getting back an `ipt_*` device handle.
  5. Cache the handle in `localStorage` + bind Service Worker
     `postMessage` events to optional `onMessage` / `onTap`
     callbacks.

  `unregisterWeb()` does the reverse — DELETE the handle + unsubscribe
  locally. Both are no-ops when the browser doesn't support the Push API.

  **Default off** — the host app calls `registerWeb` when ready.
  Sentori never triggers a permission prompt on its own. Same opt-in
  principle as `trackAutoBreadcrumb`.

  **`@goliapkg/sentori-next` (minor) — server-side send**

  New `sentoriPush({ ingestUrl, token })` factory at
  `@goliapkg/sentori-next/push`. Returns a `{ send, sendBatch,
getReceipt, isSentoriPushToken }` client that wraps `/v1/push/send`
  and `/v1/push/receipts/{id}` with the Sentori-native wire shape.
  Pure `fetch`, no Node-only imports — safe under `runtime: 'edge'`
  in App Router server actions + middleware.

  `sendBatch` concurrency-caps at 8 parallel HTTP calls to keep the
  Sentori dispatcher's queue healthy on big fan-outs.

  **Recipe**

  `docs-site` gains `recipes/push-from-nextjs.md` — end-to-end walk
  through: VAPID key pair generation, admin REST upload of the
  encrypted credentials, Service Worker template, `'use client'`
  register flow, App Router server action send, and a troubleshooting
  matrix mapping push-server status codes to operator action.

  **Compatibility**

  Wire shape is unchanged from v2.7. Customers using raw REST against
  `/v1/push/*` keep working without code changes. The Sentori-native
  wire shape `PushMessage` and the Expo-compat endpoints both stay byte-
  compatible.

## 1.2.0

### Minor Changes

- [`c26c88c`](https://github.com/goliajp/sentori-selfhosted/commit/c26c88c690bb9260881651e0d787c5d5b4b87bc3) Thanks [@doracawl](https://github.com/doracawl)! - v2.3 W6.0 — silent-by-default SDK logger.

  The SDK now ships a centralised logger module (`logger.error/warn/info/debug`)
  gated by a single `LogLevel` setting. Default level is `'warn'`: a normal Sentori
  install adds zero `[sentori]` lines to the host's console under healthy
  operation. Real problems (transport sustained failure, native module not bound,
  SDK-internal exception) still surface.

  New init fields:

  - `init({ logLevel: 'silent' | 'error' | 'warn' | 'info' | 'debug' })` — gates
    the host-facing console output. Default `'warn'`.
  - `init({ onReady: (info) => ... })` — fires once after init completes with a
    shared `ReadyInfo` shape (`sdkVersion`, plus RN-only `coldStartMs` + `native`).
    Use this to know the SDK is live without scanning the console.

  New host APIs (re-exported from each SDK):

  - `setLogLevel(level)` / `getLogLevel()` — change the gate at runtime.
  - `setLogTransport(fn)` — route Sentori-internal lines into the host's own
    logger (Datadog, OpenTelemetry, etc.); pass `null` to restore console.
  - `logger` namespace + `LogLevel` / `LogTransport` types for hosts that want to
    produce subsystem-prefixed lines themselves.

  JS SDK additions (RN was already wired in prior commits): `logLevel`, `onReady`,
  and the new canonical `sample` field (alongside the existing `sampling`
  back-compat alias). No behaviour change beyond log silence; existing init calls
  keep working.

  Mechanical perf bench (`sdk/core/src/__tests__/perf.bench.ts`) extended with
  logger budgets — gated-out `logger.debug` < 1 µs/op, emit through transport
  < 5 µs/op, `setLogLevel` toggle < 1 µs/op. Baseline numbers recorded at
  `docs/perf-baselines/v2.2.1.md` for Phase 3 (W6.1) to diff against.

- [`f1559cb`](https://github.com/goliajp/sentori-selfhosted/commit/f1559cbad697cc23e286f8f5d68f172b182d7d58) Thanks [@doracawl](https://github.com/doracawl)! - v2.3 W6.1 — `beforeSend` hook + unified `withSpan` entry point.

  Two additive surface changes:

  **`init({ beforeSend })` — host PII scrub hook**

  A sync host-supplied function called once per event just before
  transport enqueue. Return the event (possibly mutated) to ship it,
  or `null` to drop it entirely. Use for application-specific PII
  scrubbing the SDK can't do automatically.

  NEVER rule applies: a throwing hook is caught, one-shot warned, and
  the SDK falls back to the unmodified event. A non-event return
  (typo, `undefined`, etc.) gets the same treatment. Server-side
  `privacy_lab` continues running regardless of whether `beforeSend`
  is configured — `beforeSend` is the host's own defence layer in
  front of the existing server scrubber.

  ```ts
  sentori.init({
    token: "st_pk_…",
    release: "myapp@1.0.0",
    beforeSend(event) {
      if (event.tags?.flow === "kyc") return null; // never ship KYC events
      return { ...event, user: undefined }; // strip user
    },
  });
  ```

  **`withSpan` — unified entry point per design §2.3**

  `withSpan` now overloads by first-argument type:

  - `withSpan(name: string, fn)` — high-level wrap helper. Opens a
    span, runs `fn`, ends the span. Same semantics as
    `withScopedSpan(name, fn)`.
  - `withSpan(span: SpanContextLike, fn)` — low-level active-span
    manager. Pushes the span onto the active-context stack so child
    spans inherit it. Same semantics as the prior `withSpan` export
    (and the new explicit name `withActiveSpan`).

  The pre-v2.3 export name `withSpan` continues to work via the new
  overload (dispatching on first-arg type), so `withSpan(span, fn)`
  call sites are source-compatible. The explicit name
  `withActiveSpan` is exported for hosts that prefer disambiguation.
  `withScopedSpan` remains exported as the explicit name for the
  high-level path.

  Tests: new RN `applyBeforeSend` dispatcher unit tests + JS SDK
  `beforeSend` end-to-end tests via the fetch mock + core
  `withSpan(name, fn)` overload coverage (5 new tests on top of the
  v2.2 spans suite).

  `BeforeSendHook` type is exported from `@goliapkg/sentori-core` and
  re-exported by both SDKs.

## 1.1.1

### Patch Changes

- [`cb55b42`](https://github.com/goliajp/sentori-selfhosted/commit/cb55b4216dc45d2ddb58ef5fae31307685f54ebe) Thanks [@doracawl](https://github.com/doracawl)! - v2.1.1 — web matrix runtime metrics wiring + perf budget CI

  **Web SDKs now ship the runtime-metrics surface**

  `@goliapkg/sentori-javascript`:

  - New `runtime-metrics.ts` flusher mirroring the RN module —
    drains core's ring every 30 s and POSTs to
    `/v1/runtime-metrics:batch` via the same transport shape (auth
    - `Sentori-Sdk` header + `keepalive: true`). On failure,
      rebuffers + self-reports through the circuit breaker per the
      NEVER rule.
  - `initSentori({ capture: { runtimeMetrics: true } })` opt-in
    starts the flusher. Defaults `false` in JS because the
    auto-instrument modules (FPS / heap / network bytes) are
    RN-only in 2.1.0; web hosts that want to push metrics today
    call `emitMetric()` directly from their own polling.
  - Re-exports `emitMetric` / `RuntimeMetricBuffer` /
    `drainRuntimeMetricsForFlush` / `rebufferRuntimeMetrics` /
    `flushRuntimeMetrics` / `startRuntimeMetricsTimer` /
    `stopRuntimeMetricsTimer` so framework adapters don't have
    to pull in `@goliapkg/sentori-core` directly.

  `@goliapkg/sentori-svelte` / `-vue` / `-solid`:

  - Re-export the runtime-metrics surface from
    `@goliapkg/sentori-javascript` (matching each package's
    existing `addBreadcrumb` / `captureException` re-export
    convention).

  `@goliapkg/sentori-react` / `-next`:

  - Not updated in this patch — these packages don't re-export
    capture surfaces from `@goliapkg/sentori-javascript` at the
    index level (their convention is to ship providers / hooks /
    components only). Hosts using React or Next can import
    `emitMetric` directly from `@goliapkg/sentori-javascript`.

  **Performance budget CI gate**

  - `.github/workflows/sdk-perf.yml` runs `sdk/core` perf bench on
    every push to `master` + every PR touching `sdk/core/**`,
    `sdk/react-native/**`, `sdk/javascript/**`, or the workflow
    itself. A regression in any hot path (uuid / sampling / span /
    breadcrumb / trail / emitMetric / drain) fails the suite.
  - New `sdk/core` bench entries:
    - `emitMetric (no tags) < 5 µs/op` — currently ~0.2 µs (25x margin)
    - `emitMetric (3 tags) < 10 µs/op` — currently ~0.3 µs (33x margin)
    - `drainRuntimeMetricsForFlush (300 pts) < 1000 µs` — currently ~48 µs (20x margin)

  The big margins give the bench room to absorb shared-runner
  variance without flaking; a real regression nudges times into
  the same order of magnitude as the budget and the test fails
  loudly.

  **Core patch**

  - `@goliapkg/sentori-core` gets a patch bump because the perf
    bench file was modified; no API change.

## 1.1.0

### Minor Changes

- v2.1 — runtime metrics auto-instrument suite (RN)

  **New (additive — purely opt-in via `capture.runtimeMetrics`,
  defaults `true`)**

  - `sentori.init({ capture: { runtimeMetrics: true } })` (default
    on) starts the RN auto-instrument suite. Drains the shared
    `@goliapkg/sentori-core` ring to `/v1/runtime-metrics:batch`
    every 30 s, coalesced with the existing event flush so the
    host app pays one round-trip instead of two.

  **Metrics emitted**

  | Name                                                            | Source                                           | Cost                                        |
  | --------------------------------------------------------------- | ------------------------------------------------ | ------------------------------------------- |
  | `runtime.cold_start_ms`                                         | One-shot at first paint                          | trivial                                     |
  | `runtime.fps.p50` / `runtime.fps.p95`                           | rAF rolling 5 s window                           | per-tick < 0.5 ms target                    |
  | `runtime.heap.{used,total,limit}_bytes`                         | `performance.memory` poll @ 30 s                 | trivial when present, silent no-op when not |
  | `runtime.route_nav_ms`                                          | inline emit from `useTraceNavigation` per screen | trivial                                     |
  | `runtime.network.bytes_sent` / `runtime.network.bytes_received` | fetch wrapper counters, 30 s emit                | 2 adds per fetch round-trip                 |

  **Core API surface (additive)**

  - `emitMetric(name, value, tags?)` — auto-instrument entry point.
    Same validation as the server: `^[a-z][a-z0-9_]*\.[a-z0-9_.]+$`
    name regex, value finite, tags ≤ 16. Malformed silently dropped
    per the NEVER rule.
  - `RuntimeMetricBuffer` — bounded ring (10k cap, FIFO drop).
    Module-scoped global + per-instance constructor for multi-org
    test fixtures.
  - `drainRuntimeMetricsForFlush()` — atomic drain for the per-SDK
    flusher; surfaces overflow drop count once via
    `reportInternal('runtime-metrics.ring_overflow', ...)`.
  - `rebufferRuntimeMetrics(points)` — failed-flush recovery,
    bounded by ring cap.

  **Server side (v2.1 W1, already in prod)**

  - `POST /v1/runtime-metrics:batch` token-gated endpoint, writes
    to `runtime_metrics_raw` (day-partitioned, 90 d retention).
  - 60 s rollup cron raw → `_1m` with 10 s late-arrival safety
    margin; hourly `_1m → _1h`; daily `_1h → _1d`.
  - Pre-computed count / sum / avg / p50 / p95 / p99 per
    (project, bucket, name, release, environment, device_class)
    so the dashboard BI panel reads percentiles directly.

  **Compatibility**

  - v2.0 SDK requests parse unchanged on the v2.1 server. The
    `server/tests/v20_compat.rs` suite (5 cases) pins this. The
    v1 compat suite (11 cases) also stays green.
  - Mixed v2.0 / v2.1 fleets are fully supported — no flag day.
    Hosts that stay on v2.0 skip runtime-metric reporting; their
    errors / traces / breadcrumbs land on the same server unchanged.

  **Performance bedrock** (the client zero-cost rule)

  - Per-tick budget < 0.5 ms on a Pixel-5-equivalent bench
    (stop-ship gate; CI workflow lands in W2 part 5).
  - Sustained main-thread cost < 1 %.
  - Total network < 500 KB per 60 s capture window (auto-instrument
    contributes ~2.4 KB / min, ~0.5 % of the budget).

  **Not yet in 2.1**

  - Web matrix auto-instrument wiring (sdk-javascript +
    sdk-react / -vue / -svelte / -solid). Core's `emitMetric` is
    available to web hosts; the framework adapters' auto-instrument
    hooks ship in 2.1.1.
  - `.github/workflows/sdk-perf.yml` per-tick budget CI workflow
    ships in 2.1.1 alongside the web matrix.

  See `docs/roadmap/v2.1.md` for the full L2 / W-checkpoint plan
  and `docs/design/v2-metrics.md` for the schema + capacity
  envelope rationale.

## 1.0.0

### Major Changes

- v2.0 — manual instrumentation v2 (W1–W4 closeout)

  The SDK gets its first major release since v1. Every change is
  either a rename (v1 aliases gone), a move (advanced surfaces
  behind subpath imports), or an additive new API. Wire format is
  forever back-compat with v1 — v1 SDK still reports against a
  v2 server and vice-versa. Migration is purely syntactic; estimated
  effort for a typical app is ~15 minutes. See the migration recipe
  at `docs.sentori.golia.jp/recipes/v1-to-v2-migration`.

  **Renamed (v1 aliases removed)**

  - `sentori.captureError(err)` → `sentori.captureException(err)`
  - `sentori.initSentori({ ... })` → `sentori.init({ ... })`
  - `span.finish()` → `span.end()`
  - Positional `addBreadcrumb('msg', { route })` → object-form
    `addBreadcrumb({ type, data })`
  - `Event` type → `SentoriEvent` (avoids DOM `Event` collision)
  - `SpanHandle` / `MomentHandle` types → `Span` / `Moment`

  **Moved (subpath imports — bundle hygiene)**

  - `FeedbackButton` → `import { FeedbackButton } from
'@goliapkg/sentori-react-native/feedback'` (top-level re-export
    retained for one release cycle)
  - `Sentry` compat layer → `import { Sentry } from
'@goliapkg/sentori-react-native/compat'` (already present in
    v1.x; reaffirmed here)

  **Additive — new in v2.0**

  - `sentori.captureMessage(msg, { level, tags })` — issues without
    a thrown `Error`. Lands in the Issues module with a 💬 icon
    next to thrown errors. Recipe:
    `docs.sentori.golia.jp/recipes/manual-issue`.
  - Formal `Span` / `Trace` surface — `startTrace(name)`,
    `startSpan(op, opts)`, `withSpan(span, fn)`, `withScopedSpan(op,
fn, opts)`. `Span` gains `.end()` / `.setAttribute()` /
    `.setStatus()` / `.recordException()` / `.isRecording()`,
    OTel-aligned. Recipes: `manual-trace`, `manual-span`.
  - `sentori.recordMetric(name, value, tags?, { parent: span })` —
    ties the metric point to its emitting span via `tags.span_id`,
    and the dashboard's trace detail view renders a **related
    metrics row** under that span. Recipe: `track-and-metrics`.
  - `init.capture.trackAutoBreadcrumb: true` — every
    `sentori.track(name, props)` also pushes a `{ type: 'track',
data: { name, props } }` breadcrumb, so a later
    `captureException` carries the customer journey. Defaults
    `false` to preserve v1 breadcrumb shape on upgrade; recommended
    `true` for new integrations.
  - `BreadcrumbType` union adds `'track'`; server `BreadcrumbType`
    enum adds matching `Track` variant.

  **Safety guarantee — NEVER rule**

  Every public `sentori.*` API is wrapped via `safeFn` /
  `safeAsync` (`sdk/core/src/safe.ts`); internal errors silently
  fail and optionally self-report via the circuit breaker. The host
  app never sees a thrown error, a rejected promise, a frame drop,
  a network failure, or anything else attributable to Sentori —
  the budgets are < 1 % main-thread sustained and < 5 ms per tick.

  **Server compatibility**

  v1 and v2 SDK requests parse cleanly against either v1 or v2
  server. Regression suites `server/tests/v1_compat.rs` (existing)
  and `server/tests/v20_compat.rs` (added with v2.1 W1) gate this.

  **Rollout**

  We dogfood the SDK on the SaaS dashboard. Recommended customer
  sequence: lockstep upgrade + run the codemod (15 min), opt into
  `trackAutoBreadcrumb`, adopt `captureMessage` / `withSpan` /
  `recordMetric({ parent })` for the cases v1 didn't fit. Mixed
  v1 / v2 fleets are supported indefinitely — there's no flag day.

## 0.10.0

### Minor Changes

- [`ff0be91`](https://github.com/goliajp/sentori-selfhosted/commit/ff0be919b7d5cc0a1ba84e00d6203218806c5450) Thanks [@doracawl](https://github.com/doracawl)! - v2.3 W6.2 — cross-project user lookup via client-side hashed identities

  Sentori never sees raw email / phone / OAuth sub anymore. Hosts
  can now correlate one user's events across multiple projects in
  the same org without sending PII to the server.

  ## SDK API

  ```ts
  sentori.setUser({
    id: "usr_internal_123", // unchanged
    name: "Lihao", // optional, display only
    linkBy: {
      // NEW — values hashed client-side
      email: "lihao@example.com",
      googleSub: "108293…",
      phone: "+81-90-1234-5678",
    },
  });
  ```

  The `linkBy` map accepts any (key_type → raw_value) pair. SDK
  internally:

  1. Normalises (email lowercase+trim, phone E.164-ish, etc).
  2. Hashes via `crypto.subtle.digest('SHA-256', …)` — 64-char hex.
  3. Discards raw value — never crosses the network.

  Wire format carries `user.linkHashes` (a hex-only map). Server
  validates each value matches `/^[a-f0-9]{64}$/` at ingest;
  malformed (e.g. accidental raw email) → 400 rejection.

  ## What ships in the dashboard

  A new **Users** module: operator types raw value into the search
  box → browser hashes → URL state and POST body carry hash only →
  server returns cross-project hit aggregates (events / issues /
  first-seen / last-seen per project). The raw value never persists
  in dashboard state, URL, browser history, or server logs.

  ## Privacy posture

  - Different orgs use different per-org salts → same email
    produces different stored fingerprints in different orgs.
    Cross-org correlation impossible by construction.
  - Server-side salt is loaded into memory; not present in event
    payload dumps. If both `events` and `identity_scopes` tables
    leak together, fingerprints are still unsalted-sha256 — not
    trivially reversible.
  - Hosts that don't call `setUser({ linkBy })` send no identity
    hashes; this feature has zero footprint when unused.

  ## SDK behaviour

  - `setUser` returns void synchronously (host stays single-line).
  - Hash work happens in background; commits to scope when ready.
  - If a `captureException` fires before the hash settles, the
    event ships without linkHashes for that call. Next event picks
    it up.
  - If `crypto.subtle` is unavailable (very old runtime), hash
    rejects silently and linkBy is dropped — NEVER rule, no host-
    visible failure.


- [`f4748cf`](https://github.com/goliajp/sentori-selfhosted/commit/f4748cf3f1030fb1df6fcc1f4bd5d6fd16d0aeca) Thanks [@doracawl](https://github.com/doracawl)! - v2.3 W6.0 — silent-by-default + structured ready signal

  **SDK is now silent on the host's console under normal operation.**
  Previously every SDK install produced ~6 `[sentori] …` console.warn
  lines on init + per-tick replay diagnostics + breadcrumb dumps in
  dev mode. Hosts seeing `[sentori]` in their metro now means
  Sentori has a real problem, not "Sentori is doing its job."

  ## New `init` options

  ```ts
  sentori.init({
    token: "st_pk_…",
    release: "myapp@1.2.3",

    // NEW: log gate — default 'warn', set 'silent' for total silence
    logLevel: "warn" | "silent" | "error" | "info" | "debug",

    // NEW: ready callback — replaces the console banner
    onReady: (info) => {
      // info.sdkVersion, info.coldStartMs, info.native.bound,
      // info.native.methods
    },
  });
  ```

  `onReady` fires once after init completes (setConfig + native bind
  probe + transport start all settled). Host uses this to know the
  SDK is live instead of scanning the console.

  ## New `setLogTransport`

  For hosts that want to route Sentori internal logs into their own
  log aggregator (Datadog / OpenTelemetry / Bugsnag / etc.):

  ```ts
  import { setLogTransport } from "@goliapkg/sentori-react-native";

  setLogTransport((level, tag, args) => {
    myLogger.log({ source: `sentori/${tag}`, level, args });
  });
  ```

  When set, console output is fully suppressed. Pass `null` to
  restore console output. If the transport throws, Sentori swallows
  (NEVER rule) and falls back to console for that line.

  ## Log routing changes

  | Old behaviour                                                                  | New behaviour                                           |
  | ------------------------------------------------------------------------------ | ------------------------------------------------------- |
  | `console.warn('[sentori] native module bound; methods: …')`                    | `logger.debug('native', …)` — needs `logLevel: 'debug'` |
  | `console.warn('[sentori] replay tick: FIRST INVOCATION')`                      | `logger.debug('replay', …)`                             |
  | `console.warn('[sentori] replay: scheduled …')`                                | `logger.debug('replay', …)`                             |
  | `console.warn('[sentori] breadcrumb: …')`                                      | `logger.debug('breadcrumb', …)`                         |
  | `console.warn('[sentori] captureException eventId=…')` (dev dump)              | `logger.debug('capture', …)`                            |
  | `console.warn('[sentori] heartbeat failed')`                                   | `logger.debug('heartbeat', …)` (transient network)      |
  | `console.warn('[sentori] transport failed: …')`                                | `logger.warn('transport', …)` (default-visible)         |
  | `console.warn('[sentori] screenshot threw')`                                   | `logger.warn('native', …)`                              |
  | `console.warn('[sentori] requireNativeModule threw')`                          | `logger.error('native', …)` (real problem)              |
  | `console.warn('[sentori] internal failure in <api>: …')`                       | `logger.error('internal', …)` (default-visible)         |
  | `console.log('sentori: initialized (dev) · cold N ms')` (one-shot init banner) | **removed** — surface via `onReady`                     |

  Net effect with default `logLevel: 'warn'`:

  - ✓ Silent on success path
  - ✓ Real problems still visible
  - ✓ Host can dial up to `'debug'` when debugging Sentori itself
  - ✓ Host can dial down to `'silent'` for CI / production-quiet hosts

  ## Why now

  User feedback (2026-05-23) on a host metro session showing 6
  `[sentori]` WARN lines for normal init. The Sentori principle is
  "免费的好处" — a free bonus must not pollute the host's runtime
  surface. Console warns from normal operation broke that contract.

  Part of the v2.3 SDK redesign;
  identity layer + Sentry compat layer follow in W6.1+.

## 0.9.0

### Minor Changes

- [`09c823f`](https://github.com/goliajp/sentori-selfhosted/commit/09c823f4bcc9216f7c14943480dff390bef7d9de) Thanks [@doracawl](https://github.com/doracawl)! - v2.0 W1 — `captureMessage` manual issue reporting.

  Adds the missing piece of the manual instrumentation story: `sentori.captureMessage(message, opts?)` lands an issue without forcing the caller to construct an `Error`. Routes to the Issues module in the dashboard, distinct from `track` (analytics) and `recordMetric` (numeric).

  ```ts
  sentori.captureMessage("Payment provider returned 500, used fallback", {
    level: "warning",
    tags: { feature: "checkout" },
  });
  ```

  What ships in W1:

  - **New top-level API**: `captureMessage(message, opts?)` on every framework SDK (RN, JS, Solid, Svelte, Vue). React / Next / Expo continue to access it via their underlying SDK.
  - **NEVER-rule foundation**: `safeFn` / `safeAsync` / `reportInternal` utilities in `@goliapkg/sentori-core`. Every public API can now be wrapped so internal failures silently fail and never propagate to the host app. `captureMessage` is the first API to land with this discipline; W2 backfills it across the rest.
  - **5-level severity**: `'fatal' | 'error' | 'warning' | 'info' | 'debug'`. Aligns with RFC 5424 / syslog; deliberately skips Sentry's redundant `'log'` level.
  - **`SamplingConfig.messages`**: per-event-class sampling rate. `null` / absent / `1.0` = keep all.
  - **`EventKind` adds `'message'`** as a union member. `Event.error` becomes optional (message-kind events carry `level` + `message` instead). TypeScript-strict exhaustive switches on `EventKind` may need a `case 'message':` added — additive change, no runtime impact on existing callers.
  - **Server**: `events.level` + `events.message` columns (nullable migration `0064_events_level_message.sql`). Server is forward-compatible with v1 SDK wire format — v1 SDK requests parse cleanly with the new fields absent. `tests/v1_compat.rs` locks the contract in.

  Self-hosted upgrade order: server first (run the new migration), then SDK. v1 SDK clients keep working against v2 server forever.

- [`cdddae4`](https://github.com/goliajp/sentori-selfhosted/commit/cdddae448347fe6fdb7ceeb87c9818b13a9844d0) Thanks [@doracawl](https://github.com/doracawl)! - v2.0 W2 — manual span / trace surface + scope tags (additive).

  Adds the Sentry / OTel-aligned manual instrumentation primitives. All v1.x APIs continue to work unchanged — these are net-new exports.

  **New Span methods on `SpanHandle`:**

  ```ts
  span.end({ status: "ok" }); // canonical name; `finish()` still works
  span.setAttribute("db.query", sql);
  span.setAttributes({ k1: "v1", k2: "v2" });
  span.setStatus("error", "timeout"); // stashes; applied at end()
  span.recordException(err); // attaches err shape to span.data.exception
  span.isRecording(); // true while not ended
  ```

  **New top-level functions:**

  ```ts
  sentori.startTrace('checkout-flow')           // explicit new trace root, auto-tags source=manual
  sentori.withScopedSpan('db.query', async (s) => {
    // span auto-ends with status from promise outcome
    return await db.query(...)
  })
  ```

  **Global scope tags:**

  ```ts
  sentori.setTag("rollout", "dark-mode-v2");
  sentori.setTags({ rollout: "dark-mode-v2", tier: "pro" });
  // every subsequent captureException / captureMessage merges these.
  // per-call extras.tags / opts.tags win on conflict.
  ```

  Scope tags persist across captures within a process. The `setUser(...)` API (already present) follows the same precedence rule — per-call user override > scope user.

  **What's still v1.x in this release:**

  `captureError` / `initSentori` (RN) aliases, `addBreadcrumb(type, data)` positional form, `Event` type name (vs `SentoriEvent`), and `SpanHandle.finish()` all stay first-class — no deprecation warnings yet. The clean v2 major cut (v2.0.0) removes the v1.x aliases all together; it lands in W4 alongside the docs / recipes / publish.
