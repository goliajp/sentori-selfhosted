---
title: Release versioning
description: The `<app>@<version>+<build>` convention across web and mobile
---

# Release versioning

Sentori treats the **release** string as the join key between every
event and the artifacts that symbolicate it. Getting the format
consistent across web and mobile is what makes "click a frame, see
the source line" work reliably across an entire product.

## The format

```
<app>@<version>+<build>
```

| Part | Meaning |
|---|---|
| `app` | A short identifier for the codebase. Use one per ship-target: `myapp-web`, `myapp-ios`, `myapp-android`. |
| `version` | Semver or marketing version. The thing users see in App Store / "About" panel. |
| `build` | Anything that makes two builds at the same `version` distinguishable: a CI run number, a short git sha, a CFBundleVersion, a Gradle versionCode. |

Examples:

```
myapp-web@1.4.0+a1b2c3d
myapp-ios@1.4.0+1234
myapp-android@1.4.0+1234
sentori-dashboard@0.3.1+sha-deadbeef
```

The `+build` suffix is **mandatory** — without it, two different
artifacts produced from the same `version` would collide on the same
release row, and either symbolication or regression detection breaks.

## Web — Vite

`vite.config.ts`:

```ts
import { execSync } from 'node:child_process'
import { defineConfig } from 'vite'

const sha = execSync('git rev-parse --short HEAD').toString().trim()
const version = process.env.npm_package_version

export default defineConfig({
  define: {
    'import.meta.env.VITE_SENTORI_RELEASE': JSON.stringify(`myapp-web@${version}+${sha}`),
  },
})
```

Or just compose it from CI env in your `.env.production`:

```bash
VITE_SENTORI_RELEASE=myapp-web@1.4.0+${GITHUB_SHA:0:7}
```

## Web — Next.js

`next.config.js`:

```js
const { execSync } = require('node:child_process')
const sha = process.env.VERCEL_GIT_COMMIT_SHA?.slice(0, 7)
  ?? execSync('git rev-parse --short HEAD').toString().trim()

module.exports = {
  env: {
    NEXT_PUBLIC_SENTORI_RELEASE: `myapp@${process.env.npm_package_version}+${sha}`,
  },
}
```

`NEXT_PUBLIC_*` is the only env mechanism that reaches client
bundles, so server- and client-side init both see the same value.

## React Native — Expo

`app.config.ts`:

```ts
import 'dotenv/config'
import type { ExpoConfig } from 'expo/config'

export default (): ExpoConfig => ({
  name: 'MyApp',
  version: '1.4.0',                                  // Marketing version
  ios:     { buildNumber: '1234' },                  // CFBundleVersion
  android: { versionCode: 1234 },                    // Gradle versionCode
  extra: {
    sentoriRelease: `myapp@${process.env.npm_package_version}+${process.env.EAS_BUILD_ID ?? 'local'}`,
  },
})
```

Pull it at runtime:

```ts
import Constants from 'expo-constants'
import { sentori } from '@goliapkg/sentori-react-native'

sentori.init({
  token: '...',
  release: Constants.expoConfig?.extra?.sentoriRelease ?? 'myapp@0.0.0+local',
  // ...
})
```

## React Native — bare

iOS (`Info.plist` reading at runtime is fine, but the build step is
easier):

```bash
# In your iOS build script
BUILD=$(/usr/libexec/PlistBuddy -c "Print CFBundleVersion" "$INFOPLIST_FILE")
VERSION=$(/usr/libexec/PlistBuddy -c "Print CFBundleShortVersionString" "$INFOPLIST_FILE")
echo "SENTORI_RELEASE=myapp-ios@$VERSION+$BUILD" >> .env.production
```

Android (`build.gradle`):

```gradle
android {
  defaultConfig {
    versionCode 1234
    versionName "1.4.0"
    buildConfigField "String", "SENTORI_RELEASE",
      "\"myapp-android@${defaultConfig.versionName}+${defaultConfig.versionCode}\""
  }
}
```

## Why per-platform `app` names

You could in principle use `myapp@1.4.0+...` everywhere. Don't.
Different platforms have different artifact kinds (sourcemap vs
dSYM vs Proguard mapping), and the dashboard's release detail page
lists artifacts per release row. Mixing platforms under one
release pollutes the row with artifacts that only one of them
actually uses, and makes "which build is deployed where" harder to
read.

The convention is:

- `myapp-web` — browser bundle
- `myapp-ios` — iOS build
- `myapp-android` — Android build
- `myapp-node` — backend (if you instrument it)

## Regression detection

Sentori marks an issue `regressed` when:

1. It was previously `resolved` (with a `resolvedInRelease` set), AND
2. A new event lands with a release whose **app + version** is
   greater than `resolvedInRelease`.

Greater here uses semver ordering on `version`, and `+build` is
ignored for the comparison. So:

- Resolve in `myapp-web@1.4.0+a1b2c3d`
- Event arrives in `myapp-web@1.4.0+e5f6g7h` → **not regressed**
  (same version, presumably the same fix, different build)
- Event arrives in `myapp-web@1.4.1+...` → **regressed** (newer
  version means the fix should have been included)

This is why the `app` prefix matters. An iOS-only fix in
`myapp-ios@1.4.0` should not be flipped to `regressed` by an Android
event in `myapp-android@1.5.0` — they're different apps, even if
the marketing brand is the same.

## Don't change the convention mid-stream

If you ship `myapp@1.4.0+1` for a month and then switch to
`myapp-web@1.4.0+1`, every event under the new release is "new" to
the dashboard — issues won't merge across the rename, and the
regression detector won't see continuity. Settle on the convention
before launch, document it in a CONTRIBUTING.md, and treat changes
to it as a coordinated migration (resolve open issues against both
naming schemes for one release).
