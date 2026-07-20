# @goliapkg/sentori-expo

Expo adapter for Sentori — Config Plugin marker, runtime init helper
that reads `expo-application`, and an EAS post-build hook for source
map uploads. Built on `@goliapkg/sentori-react-native@>=0.2.0`.

## Install

```bash
bunx expo install @goliapkg/sentori-expo @goliapkg/sentori-react-native expo-application
```

## Wire it up

### 1. app.json

```json
{
  "expo": {
    "plugins": ["@goliapkg/sentori-expo"]
  }
}
```

The plugin is currently a marker — `@goliapkg/sentori-react-native`
ships its own `expo-module.config.json`, podspec, and Android gradle,
so Expo Modules autolinking handles the native side. The plugin entry
gives us a stable extension point for future native config (SDK
version banner, opt-in crash-handler tuning, etc.) without changing
your `app.json`.

### 2. App.tsx

```tsx
import * as Application from 'expo-application'
import { initSentoriExpo } from '@goliapkg/sentori-expo'

initSentoriExpo({
  application: Application,
  token: process.env.EXPO_PUBLIC_SENTORI_TOKEN!,
})

export default function App() { /* ... */ }
```

`initSentoriExpo`:
- Derives the release string `applicationId@version+build` from
  `expo-application`.
- Defaults the environment to `dev`/`prod` via the RN `__DEV__` flag.
- Defaults the ingest URL to the public SaaS endpoint.

You can override any of those — see `InitOptions`.

If you're not on Expo's managed workflow, omit `application` and pass
`release` explicitly:

```tsx
initSentoriExpo({
  release: 'myapp@1.2.3+42',
  token: process.env.EXPO_PUBLIC_SENTORI_TOKEN!,
})
```

### 3. EAS source map upload (optional, recommended)

Add to `eas.json`:

```json
{
  "build": {
    "production": {
      "hooks": {
        "postPublish": [
          {
            "config": "@goliapkg/sentori-expo/eas-post-build",
            "options": {
              "release": "$EAS_BUILD_RELEASE"
            }
          }
        ]
      }
    }
  }
}
```

The hook shells out to `@goliapkg/sentori-cli upload sourcemap` — install
it as a dev dep:

```bash
bun add -D @goliapkg/sentori-cli
```

> Status: Phase 22 sub-A lands the actual `upload sourcemap` and
> `upload dsym` CLI subcommands. Until then the hook logs a warning
> and exits 0 — adopt the wiring now and the upload works
> transparently when you next bump `@goliapkg/sentori-cli`.

## What `initSentoriExpo` does under the hood

```
@goliapkg/sentori-expo
  └── reads expo-application metadata
  └── calls @goliapkg/sentori-react-native init({ token, release, ... })
        └── starts the JS-layer global error / promise / network hooks
        └── primes the native iOS / Android crash handlers
```

The full feature list (breadcrumbs, capture, network instrumentation,
native crash capture) lives in `@goliapkg/sentori-react-native` —
`-expo` only adds the auto-config + EAS plumbing.

## License

Dual-licensed under [Apache-2.0](../../LICENSE-APACHE) OR
[MIT](../../LICENSE-MIT).
