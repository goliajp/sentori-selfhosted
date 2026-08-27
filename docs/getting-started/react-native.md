---
title: Getting started — React Native
description: 5 minutes from `bun add` to your first React Native event in the Sentori dashboard
---

# React Native quickstart

Goal: 5 minutes from `bun add @goliapkg/sentori-react-native` to a
mobile error showing up in the Sentori dashboard.

## Prerequisites

- A React Native app, Expo or bare. Expo Go does not bundle native
  modules so iOS dSYM uploads / Android ANR detection require a
  custom dev client (`expo prebuild`) or a bare RN project.
- Xcode (macOS) or Android Studio if you need native debugging.
- A Sentori **token** and **ingest URL** — see the [React
  getting-started overview](../getting-started.md).

## 1. Install

```bash
bun add @goliapkg/sentori-react-native
# or
pnpm add @goliapkg/sentori-react-native

# Expo: also rebuild the native layer once.
bunx expo prebuild --clean
```

## 2. Configure & initialise

Initialise once before any other module that might throw. The
recommended spot is the very top of your entry file (`index.js` for
bare RN, `app/_layout.tsx` for Expo Router):

```tsx
// index.js (bare RN) or app/_layout.tsx (Expo Router)
import 'react-native-gesture-handler' // any existing top imports
import { sentori } from '@goliapkg/sentori-react-native'

sentori.init({
  token: 'st_...',
  release: 'myapp@1.0.0+123',          // your build number / commit
  environment: __DEV__ ? 'dev' : 'prod',
  ingestUrl: 'https://ingest.sentori.golia.jp',
})

// ... rest of your entry
```

The init call:

- installs JS global error / unhandledRejection hooks
- attaches a native crash handler (signal-style on iOS, Java
  exception handler on Android)
- starts the hang watchdog (iOS) / ANR detector (Android)

## 3. Capture your first error

```tsx
import { Button, View } from 'react-native'
import { sentori } from '@goliapkg/sentori-react-native'

export default function Home() {
  return (
    <View>
      <Button
        onPress={() => {
          throw new TypeError('hello sentori')
        }}
        title="Boom"
      />
      <Button
        onPress={() => {
          // Returns the event id the server will file this under.
          const id = sentori.error(new Error('manual capture'), {
            feature: 'checkout',
          })
          console.log('[sentori] reported', id)
        }}
        title="Capture manually"
      />
    </View>
  )
}
```

Tap **Boom** to trigger a render-phase throw; tap **Capture
manually** for an imperative report.

## 4. Confirm it arrived

Every verb returns the event id it filed under, so a script does not
have to open the dashboard to know whether the report landed:

```tsx
const id = sentori.error(new Error('smoke'))
// '01a0…' — the id the server will use for this event
```

The dashboard shows the issue on the Issues list within a few seconds.

If you would rather test the endpoint before wiring the SDK, the curl
in [getting-started](../getting-started.md#working-without-an-sdk)
answers `202` with `eventId` and `issueId`. Those two ids are the only
machine-readable receipt; an ingest-scope token cannot read anything
back, so a token that can also *query* issues has to be `api` scope.

If you don't see it:

- iOS sim: events go through the host's network — `localhost:8080`
  on the sim is the host's `localhost`.
- Android emulator: replace `localhost` with `10.0.2.2` so the
  emulator can reach your dev machine. Or use the LAN IP.
- Inspect Metro / native logs for `[sentori]` warnings — bad tokens,
  network failures, and HTTP 4xx all log there.

## 5. Source maps + native symbols (optional but recommended)

JS errors symbolicate from the bundle source map; iOS / Android
crashes from the dSYM / proguard mapping respectively. Upload after
each build:

```bash
# JS bundle map
sentori-cli upload sourcemap \
  --release "myapp@1.0.0+123" \
  --token "$SENTORI_TOKEN" \
  --ingest-url "$SENTORI_INGEST_URL" \
  ios/main.jsbundle.map android/app/build/.../index.android.bundle.map

# iOS dSYMs
sentori-cli upload dsym \
  --project "$PROJECT_ID" \
  --release "myapp@1.0.0+123" \
  --token "$SENTORI_ADMIN_TOKEN" \
  --api-url "$SENTORI_API_URL" \
  ios/build/.../dSYMs/

# Android Proguard mapping
sentori-cli upload mapping \
  --project "$PROJECT_ID" \
  --release "myapp@1.0.0+123" \
  --token "$SENTORI_ADMIN_TOKEN" \
  --api-url "$SENTORI_API_URL" \
  android/app/build/outputs/mapping/release/mapping.txt
```

**Upload failures do not fail your build.** That is deliberate — a
release should never be blocked because Sentori is having a bad day.
It also means a broken upload step is silent, so check afterwards
that what you expected actually landed:

```bash
sentori-cli artifacts check \
  --release "myapp@1.0.0+123" \
  --token "$SENTORI_TOKEN" \
  --api-url "$SENTORI_API_URL" \
  --expect sourcemap,dsym,proguard
```

This is the one command in the CLI that exits non-zero on purpose:
put it at the end of the release job. It asks the server what is
there rather than trusting that the upload ran — the failure it is
built for is an upload step that stopped being called at all, which
a local "we ran it" record cannot see. Restrict `--expect` to the
platforms you actually ship (an iOS-only app has no `proguard`).

Late uploads are not wasted: the server re-reads the events it has
already stored for that release, so fixing a gap recovers the stacks
that came in while it was open. It does not re-group them — an issue
created from an unreadable stack and one created from the readable
version stay two issues, both legible.

## 6. Next steps

- [SDK reference](../../sdk/react-native/README.md) — `<ErrorBoundary>`,
  breadcrumbs, navigation hook, hang/ANR detection knobs
- [Self-hosting](../self-hosting.md) — production deploy, SMTP
- [Protocol](../protocol.md) — wire format reference
