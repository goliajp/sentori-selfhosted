# Migrating to @goliapkg/sentori-react-native 5.x

5.0 is the five-kind redesign: the SDK surface shrinks from 40+
verbs to **eight**, everything is synchronous and can never throw
into your app, and the server it talks to is the self-hosted
Sentori 2.x stack. This guide covers every 4.x API a host app is
likely to use, in the order a migration usually meets them.

```
sentori.init(config)   sentori.user(u)      sentori.context(patch)
sentori.error(err)     sentori.warn(name)   sentori.trace(name)
sentori.assert(n, ok)  sentori.probe(ref)
```

## 1. Install

```bash
bun add @goliapkg/sentori-react-native@^5.0.0
# Expo apps also bump the config plugin:
bun add @goliapkg/sentori-expo@^10.0.0
# Build pipelines:
bun add -d @goliapkg/sentori-cli@^1.0.0
```

`@goliapkg/sentori-core` 2.x comes in transitively. The native
layer (iOS/Android) is unchanged — no pod/gradle churn, an
existing dev client keeps working.

## 2. Tokens

4.x `st_pk_…` tokens do not work against the 2.x server. Mint a
fresh **ingest**-scope token in **Settings → Tokens** and swap it
into your config. Build-time tooling (sourcemap upload, probe
sync) needs a separate **api**-scope token — the ingest token
shipped inside the app is deliberately refused there.

## 3. `init`

The `capture.*` switch block is gone. Warn scenarios are detected,
not configured; replay is a single rolling-buffer knob.

```ts
// 4.x
init({
  capture: {
    longTaskMonitor: true,
    preCrashSentinel: true,
    replay: 'wireframe',
    sampleProfiler: false,
    screenshot: true,
    sessionTrail: true,
  },
  environment: 'prod',
  ingestUrl, token, release,
  onReady: (info) => { ... },
})

// 5.x
init({
  detect: {
    rageTap: true,
    longFreeze: true,
    slowColdStart: true,
    // slowApi stays opt-in
  },
  replaySeconds: 15,        // 0 disables; ships only on error/warn
  environment: 'prod',
  ingestUrl, token, release,
  logLevel: 'warn',
})
```

Removed with no replacement: `onReady`, `sampleProfiler`,
`sessionTrail` (the signal ring is always on), `launchCrashGuard`.
Cold-start timing is internal to the `slow_cold_start` detector —
`getColdStartMs()` no longer exists; drop it from log lines.

## 4. Verb mapping

| 4.x | 5.x |
|---|---|
| `sentori.captureException(err, { tags })` | `sentori.error(err, tags)` — data is a flat object |
| `sentori.captureMessage(msg)` | `sentori.warn(name, data)` — a named sub-health report |
| `sentori.track(name, props)` | `sentori.trace(name, props)` |
| `sentori.addBreadcrumb({ type, data })` | `sentori.trace(name, data, { quiet: true })` — ring-only |
| `sentori.setUser({ id, linkBy: { email }, name })` | `sentori.user({ id, email, name })` — flat shape |
| `sentori.setUser(null)` | `sentori.user(null)` |
| `sentori.setFeatureFlag(key, value)` | `sentori.context({ [key]: value })` |
| — | `sentori.assert(name, ok, data?)` — **new**: production assertion; failure reports, never halts |
| — | `sentori.probe(ref, data?)` — **new**: regression tripwire; plant in the branch that used to break |

Every verb returns immediately, never returns a Promise, and never
throws — the queue is the contract.

## 5. Components and helpers

| 4.x | 5.x |
|---|---|
| `<sentori.RageTapCapture>` | `import { RageTapCapture }` — named export, same wrapper role |
| `<FeedbackButton />` | removed |
| `sentori.registerMaskQuery(fn)` | removed — screenshot masking returns with the privacy pass |
| `useTraceNavigation(ref)` | unchanged (feeds `nav` signals + screen names on warns) |
| `triggerNativeCrash()` | unchanged (dev-panel helper) |
| `ErrorBoundary` | unchanged |
| `push.*` namespace | unchanged |

## 6. Build pipeline (CLI 1.x)

```bash
# Sourcemaps — upload failures print a friendly notice and exit 0
# so they never block a release; pass --strict to opt into a
# non-zero exit. Events received before the upload are
# re-symbolicated automatically afterwards.
sentori-cli upload sourcemap dist/main.jsbundle.map \
  --release "myapp@1.4.2+381" --token $SENTORI_API_TOKEN

# Probe tripwires — static-scans your source for sentori.probe()
# call sites and registers them against the release, so a silent
# probe is provable "fix holding".
sentori-cli probes sync --release "myapp@1.4.2+381" --dir src
```

## 7. Worked example

The first migration (Qualcomm insight-mobile) touched ten files
and took the shape above verbatim; the diff is a useful reference:
`captureException`→`error` in error reporters, `track`/
`addBreadcrumb`→`trace` in analytics shims, `setUser` flattening
in the auth listener, `setFeatureFlag`→one `context()` call at
boot, plus one new `probe()` planted on a previously-fixed bug's
failure branch.
