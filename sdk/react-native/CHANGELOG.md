# @goliapkg/sentori-react-native

## 7.0.1

### Patch Changes

- Android: a failed batch the spill could not take is counted rather than lost
  
  `persist` had two silent paths — no spill directory, which is the
  in-memory-only mode, and a write that throws, a full disk being the
  ordinary cause — and the file's own cap trimmed the oldest without
  counting them. The batch had already left the queue, so it existed
  nowhere, and the next envelope reported the gap as quiet. All three
  count now, through the same `droppedEvents` the in-memory cap has always
  used. iOS carries the matching fix.

## 7.0.0

### Major Changes

- [`f1f1543`](https://github.com/goliajp/sentori-selfhosted/commit/f1f154344d7ff3ca07ba94cde21fa7c944eea3af) - The push API is one shape now, and the SDK calls it.

  `/v1/push/tokens` is `/v1/push/devices` — four things in this system
  are called a token and the thing being registered is a device. The
  register response is `{ spToken, isNew }`; `token_id` and `is_new` are
  gone. The ack posts to `/v1/push/deliveries/{deliveryId}/ack`, because
  the id it carries is one device's row and not the call that produced
  it.

  Every field on `/v1`, in both directions, is camelCase.

### Patch Changes

- Updated dependencies [[`f1f1543`](https://github.com/goliajp/sentori-selfhosted/commit/f1f154344d7ff3ca07ba94cde21fa7c944eea3af)]:
  - @goliapkg/sentori-core@3.0.0

## 6.4.1

### Patch Changes

- [`7a80569`](https://github.com/goliajp/sentori-selfhosted/commit/7a805692373ae2258babde0f278455ab3925a931) - Nothing the host does inside a push handler can reach back into the app.

  The handlers are the host's own code running inside our loop, and one
  that threw used to take the rest of the batch with it, then the tick,
  then every later tick as an unhandled rejection. A throwing `onToken`
  reported a successful registration as a failure, and a throwing
  `onError` made `register()` reject — breaking the one contract it
  documents.

  The iron rule's gate covered the five event verbs and mentioned push
  nowhere. It does now: a server that answers 500, nonsense or nothing,
  a native module that throws, and garbage passed to every push verb.

## 6.4.0

### Minor Changes

- [`8dd96a2`](https://github.com/goliajp/sentori-selfhosted/commit/8dd96a2a955945d969a986a6351fb416b05dedd6) - A rotated vendor token no longer moves the device's address.

  Registration from this package was keyed on the vendor's token alone,
  so when the token rotated the server wrote a new row under a new
  spToken and every backend holding the old one was addressing nothing.
  Both native layers already routed a rotation into the native SDK,
  which declines to act on a device the native side never registered —
  which, for a React Native app, is every device.

  Registration now carries an installId, and the drain loop that already
  runs at 1 Hz reports a token it has not seen before. Foreground only,
  once per token, and a failed report is retried on the next change
  rather than silencing every later one.

## 6.3.0

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

### Patch Changes

- Updated dependencies [[`d6d7698`](https://github.com/goliajp/sentori-selfhosted/commit/d6d7698e361e2d10735f66b2580add4574fb9d06)]:
  - @goliapkg/sentori-core@2.3.0

## 6.2.1

### Patch Changes

- Fix a crash when push is touched from a process that is not an app

  `UNUserNotificationCenter.current()` raises
  `NSInternalInconsistencyException` — "bundleProxyForCurrentProcess is
  nil" — in any process without an app bundle: a unit-test target that
  links this SDK, a command-line tool, some extension hosts. An
  Objective-C exception cannot be caught in Swift, so it terminated the
  host outright.

  That is the SDK crashing the app rather than reporting the app's
  crash. Any customer running unit tests with Sentori linked would have
  hit it.

  All three call sites are guarded now — the permission read, the
  permission request, and the delegate installation — on whether the
  main bundle is an `.app` or `.appex`. Anything else reports the
  permission as `unavailable`, which the JS side already treats as
  `no-transport`.

  Found because the native package's own test target called it and the
  runner died mid-suite. The first fix guarded on
  `bundleIdentifier != nil`, which is true inside an xctest host and
  raised anyway; the exception message named the real cause and it took
  a second read to see it.

## 6.2.0

### Minor Changes

- [`f32b977`](https://github.com/goliajp/sentori-selfhosted/commit/f32b977fcbe956ba2db2f59db231e01368c4449a) - Two dead push options, and a `push: false` switch for the config plugin

  `PushRegisterOptions.linkHash` is gone. It targeted a column migration
  0012 removed, nothing read it, and its doc comment still called it the
  way the server reaches a specific user across their devices — so a host
  that followed the comment instead of calling `sentori.user()` got a
  successful registration that was never addressable, with nothing
  reporting why.

  `PushRegisterOptions.metadata` now works. It reached neither the
  request body nor the server, while `device_tokens.metadata` sat at
  `{}` since the table was created. It is stored, kept across
  re-registration when a later call omits it, and shown per device in
  Settings ▸ Push — needs server ≥ 2.22.0.

  `@goliapkg/sentori-expo` accepts `push: false`, which removes our
  `FirebaseMessagingService` from the merged Android manifest so
  `expo-notifications` can own delivery. Set it per EAS profile when only
  some environments use Sentori push. Error and performance capture are
  unaffected; this switches delivery, not the SDK.

## 6.1.0

### Minor Changes

- [`634754c`](https://github.com/goliajp/sentori-selfhosted/commit/634754cd4a64246bc60266cbf1b459d864944686) - Track the current Expo SDK, and declare a peer range that contains it

  6.0.0 declared `expo-modules-core >=56.0.0 <57.0.0`. This repo built
  and tested against 55.0.25, and the current Expo SDK ships 57. Nothing
  occupied that window — not our own example app, not a new user's
  project. `npm install @goliapkg/sentori-expo` answered ERESOLVE and
  installed nothing, which is the hardest possible way to fail the one
  thing this SDK has to be.

  Nothing was watching. peerDependencies are strings that no build step
  reads, and the example app is wired with `workspace:*`, so it never
  resolved the published range at all.

  The baseline is now the latest stable Expo, and stays there:

  |                              | was     | now     |
  | ---------------------------- | ------- | ------- |
  | expo                         | 55.0.24 | 57.0.12 |
  | react-native                 | 0.83.6  | 0.86.2  |
  | react                        | 19.2.0  | 19.2.3  |
  | expo-modules-core (resolved) | 55.0.25 | 57.0.10 |

  Peer ranges now say what we actually build against — `expo-modules-core

  > =57.0.0`, `expo >=57.0.0`, `react-native >=0.86.0`. **This raises the
  > floor**: Expo 55 and 56 are no longer declared supported. They were not
  > tested at any point, and 6.0.0 could not be installed on 57 regardless.

  `@goliapkg/sentori-expo` also drops its `expo-application` peer. It
  never imported the module — the host passes it in and the SDK reads a
  structural type, which is deliberate and documented in `index.ts`. An
  optional peer for something we never require is a claim with nothing
  behind it, and the new gate below is what noticed.

  `scripts/check-peer-ranges.mjs` asserts every declared peer range
  contains the version this repo installs, and fails when a peer is
  declared that nothing here installs at all. It refuses range syntax it
  cannot parse rather than passing it — a range a checker cannot read is
  one it must not bless.

## 6.0.0

### Major Changes

- [`c32d6c2`](https://github.com/goliajp/sentori-selfhosted/commit/c32d6c2da6988be9c7078a456abeff99ab8998cc) - `sentori.push.register()` never throws

  A denied push permission is an ordinary answer, not an exception. An
  opt-in that throws inside someone's `useEffect` is precisely the
  failure this SDK's contract with its host app is written against, and
  this was the one place in the SDK that did it.

  `register()` now resolves to a discriminated union — there is no
  `catch` branch left to forget:

  ```ts
  const r = await sentori.push.register();
  if (r.ok) use(r.ipt);
  else
    switch (r.reason /* ... */) {
    }
  ```

  `reason` is one of `permission-denied`, `no-transport`,
  `token-timeout`, `server-rejected`, `not-initialised`. Each asks the
  host for something different, which is the only reason to distinguish
  them — a declined permission should never be retried on a timer, while
  a token timeout reasonably should.

  Two of those were one branch until now: a missing native module
  reported itself as a denied permission, so an Expo Go build and a user
  tapping "Don't Allow" produced the same message.

  **Breaking**: the resolved value was `{ ipt }` and is now
  `{ ok: true, ipt } | { ok: false, reason, message }`. Destructuring
  `const { ipt } = await register()` no longer type-checks.

  Also in this release: the first tests this package has ever had for
  push. There were none, which is how three unbalanced INSERTs on the
  server and two wrong field names in this file went a year without
  anyone noticing that registration had never once succeeded.

## 4.0.0

### Major Changes

- [`2769816`](https://github.com/goliajp/sentori-selfhosted/commit/2769816170298cc5a2cc6d5f29f33c2296c256f9) Thanks [@doracawl](https://github.com/doracawl)! - Drop Expo SDK 55 / React Native 0.81 support; require Expo SDK 56+
  or bare RN 0.82+.

  Cascade from `@goliapkg/sentori-expo@8.0.0` (drops Expo SDK 55) so
  the Sentori React Native SDK matrix is aligned on a single Expo SDK
  version. Insight, our primary dogfood user, is on RN 0.85.3 (Expo
  SDK 56 environment), and the support window moves to match.

  **Peer range tightening:**

  | Peer                | 3.x                  | 4.0.0                               |
  | ------------------- | -------------------- | ----------------------------------- |
  | `expo-modules-core` | `">=55.0.0 <57.0.0"` | `">=56.0.0 <57.0.0"`                |
  | `react-native`      | `">=0.81.0"`         | `">=0.82.0"` (Expo 56 pins RN 0.82) |
  | `react`             | `">=19"`             | unchanged                           |

  **For Expo SDK 55 hosts or bare RN 0.81 hosts**: pin
  `@goliapkg/sentori-react-native@^3.1.0`. The 3.x line continues to
  receive critical-bug backports.

  **No runtime API change.** Every public surface (`sentori.init`,
  `captureException` / `captureMessage`, span / trace APIs,
  `recordMetric`, breadcrumbs, native crash handlers, push registration,
  session-trail / replay capture, fetch + react-navigation tracing)
  behaves identically on the new peer range. The native iOS / Android
  modules use only APIs stable on Expo modules-core 56.x.

  **Companion package not bumped automatically:**
  `@goliapkg/sentori-expo@8.0.0` declares
  `@goliapkg/sentori-react-native: ">=3.1.0"`, so Expo 56 hosts can run
  either RN SDK 3.x or 4.x (both satisfy the peer). For new
  installations on Expo 56 we recommend pinning RN SDK 4.x for the
  narrower-and-more-explicit Expo 56 support window.

## 3.1.0

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

### Patch Changes

- [`9746100`](https://github.com/goliajp/sentori-selfhosted/commit/97461007dfb23059fbf0d85e02b1e0e70752e098) Thanks [@doracawl](https://github.com/doracawl)! - Fix Android build break on the Kotlin K2 compiler (Expo SDK 56).

  `SentoriModule.kt` used a bare `return@Function` inside no-argument
  `Function {}` lambdas (`pushRegister`, `pushUnregister`). Under K2 these
  bind to the no-arg overload whose body is typed `() -> Any?`, where a bare
  `return` is inferred as `Unit` and rejected:

  ```
  e: SentoriModule.kt: Return type mismatch: expected 'Any?', actual 'Unit'.
  ```

  This was a hard compile error that failed `:compileDebugKotlin` and blocked
  the entire host app's Android build. Rewrote the early-out as idiomatic
  `appContext.reactContext?.let { ... }` so the lambda's last expression is the
  only return path. Also applied the same shape to `startAnrWatchdog` (which
  took a parameter and so still compiled, but carried the identical latent
  hazard) for consistency and future-proofing.

- Updated dependencies [[`8d07add`](https://github.com/goliajp/sentori-selfhosted/commit/8d07add988d737b7699299c26e3712c444660ca9), [`cb1870e`](https://github.com/goliajp/sentori-selfhosted/commit/cb1870ebc23e515d3d94775536cf2dba2b406be3)]:
  - @goliapkg/sentori-core@1.4.0

## 3.0.0

### Major Changes

- v2.17 — Drop Expo SDK 50 / 51 / 52 / 53 / 54 support; target Expo 55+ (RN 0.81+).

  `@expo/config-plugins` now ships synced to the Expo SDK version
  (`~55.0.x` for Expo 55, `~56.0.x` for Expo 56) — the prior
  `^9 || ^10` dep range was broken from Expo 54 onward and effectively
  made `@goliapkg/sentori-expo` uninstallable on any current Expo SDK.

  This release aligns peer + dependency ranges with what Expo actually
  ships today:

  **`@goliapkg/sentori-expo` (major — 5.0.0 → 6.0.0)**

  - `peerDependencies.expo`: `">=50"` → `">=55.0.0 <57.0.0"`
  - `peerDependencies.expo-application`: `">=5"` → `">=55.0.0 <57.0.0"`
  - `peerDependencies.react-native`: `">=0.74"` → `">=0.81.0"`
  - `peerDependencies.@goliapkg/sentori-react-native`: `">=2.2.0"` → `">=3.0.0"`
  - `dependencies.@expo/config-plugins`: `"^9 || ^10"` → `">=55.0.0 <57.0.0"`

  **`@goliapkg/sentori-react-native` (major — 2.x → 3.0.0)**

  Cascade — Expo SDK 55+ ships `expo-modules-core` versioned with the
  SDK (no longer the standalone `2.x` line), so the peer range has to
  move with it.

  - `peerDependencies.expo-modules-core`: `">=2.0"` → `">=55.0.0 <57.0.0"`
  - `peerDependencies.react`: `">=18"` → `">=19"`
  - `peerDependencies.react-native`: `">=0.74"` → `">=0.81.0"`

  **Support window**

  Currently-supported Expo SDKs: **55** (Aug 2025, RN 0.81) and
  **56** (Nov 2025, RN 0.82). Older Expo apps stay on
  `@goliapkg/sentori-expo@5.x` / `@goliapkg/sentori-react-native@2.x`,
  which keep the existing Push notification setup the v2.7–v2.12
  series shipped.

  **No runtime code change.** Every config-plugin API
  (`withInfoPlist` / `withEntitlementsPlist` / `withAndroidManifest` /
  `withProjectBuildGradle` / `withAppBuildGradle` / `withDangerousMod` /
  `withPlugins` / `AndroidConfig.Permissions.addPermission`) used in
  `app.plugin.js` has been stable since `@expo/config-plugins@4`; the
  v2.11 Push plugin runs unchanged on the new range.

### Minor Changes

- [`4e12ddf`](https://github.com/goliajp/sentori-selfhosted/commit/4e12ddfb8b87ace79521e9f2a2363e2d0bd79b20) Thanks [@doracawl](https://github.com/doracawl)! - v2.10 — Android FCM push notification opt-in for React Native.

  Fourth phase of the v2.7→v2.12 Push rollout. v2.9 shipped the iOS
  branch; v2.10 lights the Android FCM branch behind the same JS
  surface — `sentori.push.register({...})` now resolves to an `ipt_*`
  handle on Android too.

  **Native — Kotlin**

  - New `android/src/main/java/com/sentori/SentoriPushNotifications.kt`
    — static singleton owning the 32-slot FIFO buffers (token /
    notifications / taps), runtime POST_NOTIFICATIONS permission flow
    for Android 13+, and the FCM token retrieval via
    `Class.forName("FirebaseMessaging").getInstance().getToken()` so
    the SDK works whether or not Firebase is on the host's classpath.
  - New `SentoriFirebaseMessagingService.kt` extends
    `FirebaseMessagingService` and routes `onNewToken` /
    `onMessageReceived` into `SentoriPushNotifications`. Manifest-
    registered in the SDK's `AndroidManifest.xml`; manifest merger
    combines it with the host app's manifest.
  - `SentoriModule.kt` gains 5 ModuleDefinition exports identical to
    iOS: `pushGetStatus`, `pushRequestPermission`, `pushRegister`,
    `pushUnregister`, `pushDrainState`.
  - `android/build.gradle` adds `compileOnly` on
    `com.google.firebase:firebase-messaging:24.0.3` so non-push hosts
    pay nothing. The host app declares the runtime Firebase dep + the
    `com.google.gms.google-services` plugin themselves.
  - `AndroidManifest.xml` declares
    `<uses-permission android:name="android.permission.POST_NOTIFICATIONS"/>`
    and the FirebaseMessagingService.

  **JS — `src/push.ts` cross-platform**

  - `detectPlatform()` reads `Platform.OS` from `react-native` (with
    test-only `__setPlatformForTests` hook).
  - `registerWithServer` now sends `provider: 'fcm'` and omits `env`
    on Android. iOS still sends `provider: 'apns'` + `env: 'sandbox' |
'production'` keyed by `__DEV__`.
  - All other public surface (`register`, `unregister`, `getCachedIpt`,
    `getStatus`, `requestPermission`) is unchanged — host app code is
    platform-agnostic.

  **Recipe**

  `docs-site/src/content/docs/recipes/push-from-react-native-android.md`
  walks the `google-services.json` placement, gradle deps, plugin
  application, register flow, send via `@goliapkg/sentori-next/push`,
  and a troubleshooting matrix for the FCM\_\* status codes the server
  dispatcher surfaces.

  **Tests**

  One new bun test in `src/__tests__/push.test.ts` covers the Android
  branch — asserts `provider: 'fcm'` is sent and `env` is omitted.
  Full RN test suite 186 pass / 0 fail.

  **Compatibility**

  Wire shape unchanged from v2.7 / v2.8 / v2.9. Android device tokens
  land in the same `device_tokens` table with `provider: 'fcm'`. Raw
  REST customers keep working with no changes.

  v2.11 — Expo config plugin + dashboard Push module — lands next.

- [`cd4aa8e`](https://github.com/goliajp/sentori-selfhosted/commit/cd4aa8e58491d846b3ea575a02aac761791c72bc) Thanks [@doracawl](https://github.com/doracawl)! - v2.18 — `expo-notifications` drop-in shim at `@goliapkg/sentori-react-native/expo-compat`.

  90% drop-in. Customers migrating from `expo-notifications` change
  ONE line:

  ```diff
  - import * as Notifications from 'expo-notifications'
  + import * as Notifications from '@goliapkg/sentori-react-native/expo-compat'
  ```

  …and the rest of their client-side code keeps compiling. Type
  shapes (`Notification` / `NotificationResponse` /
  `NotificationContent` / `NotificationRequest`) match
  `expo-notifications` byte-for-byte; constants
  (`AndroidImportance` / `IosAuthorizationStatus` /
  `DEFAULT_ACTION_IDENTIFIER` / `SchedulableTriggerInputTypes`) are
  re-exported with identical values; the listener registry +
  1 Hz drain loop mirror the upstream's event semantics.

  **Covered today** (just-works after the import swap):

  - `getPermissionsAsync` / `requestPermissionsAsync` (incl. iOS
    sub-options — `allowProvisional` falls back to regular auth, with
    a debug-log note)
  - `getDevicePushTokenAsync` (raw APNs / FCM token)
  - `getExpoPushTokenAsync` (returns native token wrapped in the same
    envelope shape; **server-side change required** — POST to Sentori
    ingest instead of exp.host)
  - `addNotificationReceivedListener` (foreground)
  - `addNotificationResponseReceivedListener` (tap)
  - `addPushTokenListener` (rotation)
  - `setNotificationHandler` (handler runs; presentation-override
    flags are a follow-up)
  - `unregisterForNotificationsAsync`

  **Throws today** (each error message points at the recipe section
  that documents the workaround):

  `scheduleNotificationAsync` + 7 trigger types · `setBadgeCountAsync`
  / `getBadgeCountAsync` · `setNotificationChannelAsync` + channel
  groups · `setNotificationCategoryAsync` + interactive actions ·
  `useLastNotificationResponse` / `getLastNotificationResponseAsync` ·
  `subscribeToTopicAsync` / `unsubscribeFromTopicAsync` ·
  `registerTaskAsync` · `dismissNotificationAsync` /
  `dismissAllNotificationsAsync` / `getPresentedNotificationsAsync`

  The unsupported list is the follow-up minor backlog — every entry
  needs a native module surface we don't have yet, but nothing here
  is a permanent gap.

  **Server-side migration** documented in the recipe at
  `docs-site/src/content/docs/recipes/migrate-from-expo-notifications.md`
  — covers the `exp.host` → Sentori ingest swap, the device-token
  shape change (`ExponentPushToken[...]` → `ipt_<uuid>`), and the
  credential upload flow.

  **Coexistence** — `expo-notifications` AND
  `@goliapkg/sentori-react-native/expo-compat` can run side-by-side in
  the same app; they register separately with the OS push service.
  Useful for a 1-week cut-over where you compare delivery rates.

  **Compatibility** — additive subpath export. The existing
  `sentori.push.*` namespace is unchanged. Wire shape with
  `/v1/push/*` is the same as v2.7–v2.12 Push series.

- [`fd81428`](https://github.com/goliajp/sentori-selfhosted/commit/fd81428f380da7bbadbae24eccc9270b1b59144a) Thanks [@doracawl](https://github.com/doracawl)! - v2.9 — iOS push notification opt-in for React Native.

  Third phase of the v2.7→v2.12 Push rollout. v2.7 shipped the server
  foundation (APNs + FCM providers + dispatch cron + secrets-sealed
  credentials + `/v1/push/*` routes); v2.8 shipped Web Push end-to-end;
  v2.9 lights the iOS APNs branch for `@goliapkg/sentori-react-native`.

  **New surface — `sentori.push.*`**

  ```ts
  const { ipt } = await sentori.push.register({
    linkHash: '...',
    onMessage: (m) => { ... },
    onTap: (data) => { ... },
  })
  await sentori.push.unregister()
  sentori.push.getCachedIpt() // ipt_... | null
  await sentori.push.getStatus() // 'granted' | 'denied' | 'notDetermined' | ...
  await sentori.push.requestPermission()
  ```

  **Native iOS module**

  A new `SentoriPushNotifications.swift` owns:

  - UNUserNotificationCenter delegate (foreground + tap)
  - AppDelegate method swizzle for
    `application:didRegisterForRemoteNotificationsWithDeviceToken:` +
    the failure variant. Idempotent; opt-out via Info.plist
    `Sentori.disableAppDelegateSwizzle = YES`.
  - In-memory buffers (32-slot FIFO each) for the token, foreground
    notifications, and tap responses. JS drains them via a 1 Hz loop
    that pauses on background (battery rule).

  `SentoriModule.swift` adds 5 ModuleDefinition exports:
  `pushGetStatus`, `pushRequestPermission`, `pushRegister`,
  `pushUnregister`, `pushDrainState`.

  **JS flow**

  `register()`:

  1. `pushRequestPermission()` — OS prompt the first time.
  2. `pushRegister()` — calls
     `UIApplication.registerForRemoteNotifications`.
  3. Polls `pushDrainState()` at 200 ms ticks for up to 8 s waiting
     for the token; rejects with a tagged error on timeout / native
     failure / denied permission.
  4. POSTs `/v1/push/tokens` with
     `provider: 'apns'`, `env: __DEV__ ? 'sandbox' : 'production'`,
     `nativeToken: <hex>`, `linkHash?`, `metadata`.
  5. Caches the `ipt_*` handle to
     `@react-native-async-storage/async-storage` if installed,
     otherwise a module-scoped variable.
  6. Starts a 1 Hz drain loop that fires `onMessage` / `onTap` from
     buffered events. Pauses on `AppState.change → 'background'`,
     resumes on `'active'`.

  **Default off** — host calls `register()` when ready. Sentori SDK
  init never triggers the OS prompt on its own. Same opt-in posture as
  `trackAutoBreadcrumb` and v2.8's `registerWeb`.

  **Recipe**

  `docs-site/src/content/docs/recipes/push-from-react-native-ios.md` —
  end-to-end walkthrough: APNs credential upload, Xcode capability +
  Info.plist + entitlements setup, register flow, send via
  `@goliapkg/sentori-next/push`, troubleshooting matrix for the APNS\_\*
  status codes the server's dispatcher surfaces.

  **Compatibility**

  Wire shape is unchanged from v2.7 / v2.8. The new device tokens land
  in the same `device_tokens` table with `provider: 'apns'`. Customers
  sending via raw REST keep working with no changes.

  iOS only in this release — Android FCM lands in v2.10, same JS API.

  **Tests**

  6 new bun tests in `src/__tests__/push.test.ts` cover:

  - register rejects cleanly on denied permission
  - register rejects cleanly on native-token timeout
  - register POSTs the right shape (provider/env/nativeToken/linkHash)
    and resolves to the server-issued `ipt_*`
  - register surfaces server errors
  - foreground notifications buffered during the token wait flush to
    `onMessage`
  - unregister DELETEs the cached `ipt`, clears local state, and calls
    the native unregister

  Full RN test suite: 185 pass, 0 fail.

### Patch Changes

- Updated dependencies [[`d8e38b2`](https://github.com/goliajp/sentori-selfhosted/commit/d8e38b222a4a5b1d76362514e637bc996c805cc2)]:
  - @goliapkg/sentori-core@1.3.0

## 3.0.0

### Major Changes

- v2.17 — Drop Expo SDK 50 / 51 / 52 / 53 / 54 support; target Expo 55+ (RN 0.81+).

  `@expo/config-plugins` now ships synced to the Expo SDK version
  (`~55.0.x` for Expo 55, `~56.0.x` for Expo 56) — the prior
  `^9 || ^10` dep range was broken from Expo 54 onward and effectively
  made `@goliapkg/sentori-expo` uninstallable on any current Expo SDK.

  This release aligns peer + dependency ranges with what Expo actually
  ships today:

  **`@goliapkg/sentori-expo` (major — 5.0.0 → 6.0.0)**

  - `peerDependencies.expo`: `">=50"` → `">=55.0.0 <57.0.0"`
  - `peerDependencies.expo-application`: `">=5"` → `">=55.0.0 <57.0.0"`
  - `peerDependencies.react-native`: `">=0.74"` → `">=0.81.0"`
  - `peerDependencies.@goliapkg/sentori-react-native`: `">=2.2.0"` → `">=3.0.0"`
  - `dependencies.@expo/config-plugins`: `"^9 || ^10"` → `">=55.0.0 <57.0.0"`

  **`@goliapkg/sentori-react-native` (major — 2.x → 3.0.0)**

  Cascade — Expo SDK 55+ ships `expo-modules-core` versioned with the
  SDK (no longer the standalone `2.x` line), so the peer range has to
  move with it.

  - `peerDependencies.expo-modules-core`: `">=2.0"` → `">=55.0.0 <57.0.0"`
  - `peerDependencies.react`: `">=18"` → `">=19"`
  - `peerDependencies.react-native`: `">=0.74"` → `">=0.81.0"`

  **Support window**

  Currently-supported Expo SDKs: **55** (Aug 2025, RN 0.81) and
  **56** (Nov 2025, RN 0.82). Older Expo apps stay on
  `@goliapkg/sentori-expo@5.x` / `@goliapkg/sentori-react-native@2.x`,
  which keep the existing Push notification setup the v2.7–v2.12
  series shipped.

  **No runtime code change.** Every config-plugin API
  (`withInfoPlist` / `withEntitlementsPlist` / `withAndroidManifest` /
  `withProjectBuildGradle` / `withAppBuildGradle` / `withDangerousMod` /
  `withPlugins` / `AndroidConfig.Permissions.addPermission`) used in
  `app.plugin.js` has been stable since `@expo/config-plugins@4`; the
  v2.11 Push plugin runs unchanged on the new range.

### Minor Changes

- [`4e12ddf`](https://github.com/goliajp/sentori-selfhosted/commit/4e12ddfb8b87ace79521e9f2a2363e2d0bd79b20) Thanks [@doracawl](https://github.com/doracawl)! - v2.10 — Android FCM push notification opt-in for React Native.

  Fourth phase of the v2.7→v2.12 Push rollout. v2.9 shipped the iOS
  branch; v2.10 lights the Android FCM branch behind the same JS
  surface — `sentori.push.register({...})` now resolves to an `ipt_*`
  handle on Android too.

  **Native — Kotlin**

  - New `android/src/main/java/com/sentori/SentoriPushNotifications.kt`
    — static singleton owning the 32-slot FIFO buffers (token /
    notifications / taps), runtime POST_NOTIFICATIONS permission flow
    for Android 13+, and the FCM token retrieval via
    `Class.forName("FirebaseMessaging").getInstance().getToken()` so
    the SDK works whether or not Firebase is on the host's classpath.
  - New `SentoriFirebaseMessagingService.kt` extends
    `FirebaseMessagingService` and routes `onNewToken` /
    `onMessageReceived` into `SentoriPushNotifications`. Manifest-
    registered in the SDK's `AndroidManifest.xml`; manifest merger
    combines it with the host app's manifest.
  - `SentoriModule.kt` gains 5 ModuleDefinition exports identical to
    iOS: `pushGetStatus`, `pushRequestPermission`, `pushRegister`,
    `pushUnregister`, `pushDrainState`.
  - `android/build.gradle` adds `compileOnly` on
    `com.google.firebase:firebase-messaging:24.0.3` so non-push hosts
    pay nothing. The host app declares the runtime Firebase dep + the
    `com.google.gms.google-services` plugin themselves.
  - `AndroidManifest.xml` declares
    `<uses-permission android:name="android.permission.POST_NOTIFICATIONS"/>`
    and the FirebaseMessagingService.

  **JS — `src/push.ts` cross-platform**

  - `detectPlatform()` reads `Platform.OS` from `react-native` (with
    test-only `__setPlatformForTests` hook).
  - `registerWithServer` now sends `provider: 'fcm'` and omits `env`
    on Android. iOS still sends `provider: 'apns'` + `env: 'sandbox' |
'production'` keyed by `__DEV__`.
  - All other public surface (`register`, `unregister`, `getCachedIpt`,
    `getStatus`, `requestPermission`) is unchanged — host app code is
    platform-agnostic.

  **Recipe**

  `docs-site/src/content/docs/recipes/push-from-react-native-android.md`
  walks the `google-services.json` placement, gradle deps, plugin
  application, register flow, send via `@goliapkg/sentori-next/push`,
  and a troubleshooting matrix for the FCM\_\* status codes the server
  dispatcher surfaces.

  **Tests**

  One new bun test in `src/__tests__/push.test.ts` covers the Android
  branch — asserts `provider: 'fcm'` is sent and `env` is omitted.
  Full RN test suite 186 pass / 0 fail.

  **Compatibility**

  Wire shape unchanged from v2.7 / v2.8 / v2.9. Android device tokens
  land in the same `device_tokens` table with `provider: 'fcm'`. Raw
  REST customers keep working with no changes.

  v2.11 — Expo config plugin + dashboard Push module — lands next.

- [`cd4aa8e`](https://github.com/goliajp/sentori-selfhosted/commit/cd4aa8e58491d846b3ea575a02aac761791c72bc) Thanks [@doracawl](https://github.com/doracawl)! - v2.18 — `expo-notifications` drop-in shim at `@goliapkg/sentori-react-native/expo-compat`.

  90% drop-in. Customers migrating from `expo-notifications` change
  ONE line:

  ```diff
  - import * as Notifications from 'expo-notifications'
  + import * as Notifications from '@goliapkg/sentori-react-native/expo-compat'
  ```

  …and the rest of their client-side code keeps compiling. Type
  shapes (`Notification` / `NotificationResponse` /
  `NotificationContent` / `NotificationRequest`) match
  `expo-notifications` byte-for-byte; constants
  (`AndroidImportance` / `IosAuthorizationStatus` /
  `DEFAULT_ACTION_IDENTIFIER` / `SchedulableTriggerInputTypes`) are
  re-exported with identical values; the listener registry +
  1 Hz drain loop mirror the upstream's event semantics.

  **Covered today** (just-works after the import swap):

  - `getPermissionsAsync` / `requestPermissionsAsync` (incl. iOS
    sub-options — `allowProvisional` falls back to regular auth, with
    a debug-log note)
  - `getDevicePushTokenAsync` (raw APNs / FCM token)
  - `getExpoPushTokenAsync` (returns native token wrapped in the same
    envelope shape; **server-side change required** — POST to Sentori
    ingest instead of exp.host)
  - `addNotificationReceivedListener` (foreground)
  - `addNotificationResponseReceivedListener` (tap)
  - `addPushTokenListener` (rotation)
  - `setNotificationHandler` (handler runs; presentation-override
    flags are a follow-up)
  - `unregisterForNotificationsAsync`

  **Throws today** (each error message points at the recipe section
  that documents the workaround):

  `scheduleNotificationAsync` + 7 trigger types · `setBadgeCountAsync`
  / `getBadgeCountAsync` · `setNotificationChannelAsync` + channel
  groups · `setNotificationCategoryAsync` + interactive actions ·
  `useLastNotificationResponse` / `getLastNotificationResponseAsync` ·
  `subscribeToTopicAsync` / `unsubscribeFromTopicAsync` ·
  `registerTaskAsync` · `dismissNotificationAsync` /
  `dismissAllNotificationsAsync` / `getPresentedNotificationsAsync`

  The unsupported list is the follow-up minor backlog — every entry
  needs a native module surface we don't have yet, but nothing here
  is a permanent gap.

  **Server-side migration** documented in the recipe at
  `docs-site/src/content/docs/recipes/migrate-from-expo-notifications.md`
  — covers the `exp.host` → Sentori ingest swap, the device-token
  shape change (`ExponentPushToken[...]` → `ipt_<uuid>`), and the
  credential upload flow.

  **Coexistence** — `expo-notifications` AND
  `@goliapkg/sentori-react-native/expo-compat` can run side-by-side in
  the same app; they register separately with the OS push service.
  Useful for a 1-week cut-over where you compare delivery rates.

  **Compatibility** — additive subpath export. The existing
  `sentori.push.*` namespace is unchanged. Wire shape with
  `/v1/push/*` is the same as v2.7–v2.12 Push series.

- [`fd81428`](https://github.com/goliajp/sentori-selfhosted/commit/fd81428f380da7bbadbae24eccc9270b1b59144a) Thanks [@doracawl](https://github.com/doracawl)! - v2.9 — iOS push notification opt-in for React Native.

  Third phase of the v2.7→v2.12 Push rollout. v2.7 shipped the server
  foundation (APNs + FCM providers + dispatch cron + secrets-sealed
  credentials + `/v1/push/*` routes); v2.8 shipped Web Push end-to-end;
  v2.9 lights the iOS APNs branch for `@goliapkg/sentori-react-native`.

  **New surface — `sentori.push.*`**

  ```ts
  const { ipt } = await sentori.push.register({
    linkHash: '...',
    onMessage: (m) => { ... },
    onTap: (data) => { ... },
  })
  await sentori.push.unregister()
  sentori.push.getCachedIpt() // ipt_... | null
  await sentori.push.getStatus() // 'granted' | 'denied' | 'notDetermined' | ...
  await sentori.push.requestPermission()
  ```

  **Native iOS module**

  A new `SentoriPushNotifications.swift` owns:

  - UNUserNotificationCenter delegate (foreground + tap)
  - AppDelegate method swizzle for
    `application:didRegisterForRemoteNotificationsWithDeviceToken:` +
    the failure variant. Idempotent; opt-out via Info.plist
    `Sentori.disableAppDelegateSwizzle = YES`.
  - In-memory buffers (32-slot FIFO each) for the token, foreground
    notifications, and tap responses. JS drains them via a 1 Hz loop
    that pauses on background (battery rule).

  `SentoriModule.swift` adds 5 ModuleDefinition exports:
  `pushGetStatus`, `pushRequestPermission`, `pushRegister`,
  `pushUnregister`, `pushDrainState`.

  **JS flow**

  `register()`:

  1. `pushRequestPermission()` — OS prompt the first time.
  2. `pushRegister()` — calls
     `UIApplication.registerForRemoteNotifications`.
  3. Polls `pushDrainState()` at 200 ms ticks for up to 8 s waiting
     for the token; rejects with a tagged error on timeout / native
     failure / denied permission.
  4. POSTs `/v1/push/tokens` with
     `provider: 'apns'`, `env: __DEV__ ? 'sandbox' : 'production'`,
     `nativeToken: <hex>`, `linkHash?`, `metadata`.
  5. Caches the `ipt_*` handle to
     `@react-native-async-storage/async-storage` if installed,
     otherwise a module-scoped variable.
  6. Starts a 1 Hz drain loop that fires `onMessage` / `onTap` from
     buffered events. Pauses on `AppState.change → 'background'`,
     resumes on `'active'`.

  **Default off** — host calls `register()` when ready. Sentori SDK
  init never triggers the OS prompt on its own. Same opt-in posture as
  `trackAutoBreadcrumb` and v2.8's `registerWeb`.

  **Recipe**

  `docs-site/src/content/docs/recipes/push-from-react-native-ios.md` —
  end-to-end walkthrough: APNs credential upload, Xcode capability +
  Info.plist + entitlements setup, register flow, send via
  `@goliapkg/sentori-next/push`, troubleshooting matrix for the APNS\_\*
  status codes the server's dispatcher surfaces.

  **Compatibility**

  Wire shape is unchanged from v2.7 / v2.8. The new device tokens land
  in the same `device_tokens` table with `provider: 'apns'`. Customers
  sending via raw REST keep working with no changes.

  iOS only in this release — Android FCM lands in v2.10, same JS API.

  **Tests**

  6 new bun tests in `src/__tests__/push.test.ts` cover:

  - register rejects cleanly on denied permission
  - register rejects cleanly on native-token timeout
  - register POSTs the right shape (provider/env/nativeToken/linkHash)
    and resolves to the server-issued `ipt_*`
  - register surfaces server errors
  - foreground notifications buffered during the token wait flush to
    `onMessage`
  - unregister DELETEs the cached `ipt`, clears local state, and calls
    the native unregister

  Full RN test suite: 185 pass, 0 fail.

### Patch Changes

- Updated dependencies [[`d8e38b2`](https://github.com/goliajp/sentori-selfhosted/commit/d8e38b222a4a5b1d76362514e637bc996c805cc2)]:
  - @goliapkg/sentori-core@1.3.0

## 2.2.0

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

### Patch Changes

- [`1bdda31`](https://github.com/goliajp/sentori-selfhosted/commit/1bdda31e9afc8ed2f2bc119a9e59de8917f6df56) Thanks [@doracawl](https://github.com/doracawl)! - v2.3 W6.3 — Sentry-compat translation coverage + test surface.

  The compat module (`@goliapkg/sentori-react-native/compat`) was
  already substantively implemented (DSN parser, init, captureException,
  captureMessage, setUser with `ip_address` drop + `segment → tag`
  remap, setTag, addBreadcrumb category-to-type mapping, withScope,
  configureScope, startTransaction, close, flush, Severity enum,
  warn-once dedup). This patch adds:

  - Test exports for the three pure-function pieces:
    `__parseDsnForTests`, `__mapCategoryToTypeForTests`,
    `__mapLevelForTests`. Direct callers should keep using
    `Sentry.init` / `Sentry.addBreadcrumb` / `Sentry.captureMessage`;
    the hooks exist so the unit test can verify the translation
    table without mocking the entire native init chain.
  - 18 new unit tests (`__tests__/compat-sentry.test.ts`):
    parseDsn (5 cases: happy path, custom port, wrong token
    prefix, missing token, malformed URL),
    mapCategoryToType (6 cases covering user / net / nav / log
    buckets + unknown → custom + undefined),
    mapLevel (3 cases: Critical → fatal, Log → info, 5-level
    passthrough, undefined),
    Severity export (3 cases).

  No behaviour change. The compat surface itself was already shipped
  and unchanged; this is purely test coverage filling a Phase 5 gap.

- Updated dependencies [[`c26c88c`](https://github.com/goliajp/sentori-selfhosted/commit/c26c88c690bb9260881651e0d787c5d5b4b87bc3), [`f1559cb`](https://github.com/goliajp/sentori-selfhosted/commit/f1559cbad697cc23e286f8f5d68f172b182d7d58)]:
  - @goliapkg/sentori-core@1.2.0

## 2.1.0

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

### Patch Changes

- Updated dependencies []:
  - @goliapkg/sentori-core@1.1.0

## 2.0.0

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

### Patch Changes

- Updated dependencies []:
  - @goliapkg/sentori-core@1.0.0

## 1.3.0

### Minor Changes

- [`afcc7d8`](https://github.com/goliajp/sentori-selfhosted/commit/afcc7d81bba90b4735a9bbb0249e180b1f145d7e) Thanks [@doracawl](https://github.com/doracawl)! - v2.3 W6.3 — Sentry-compatible API surface at `/compat`

  Drop-in shim for code (or LLM-generated code) written against
  `@sentry/react-native`. Every Sentry call maps to exactly one
  Sentori-native call internally. Translation differences fire a
  one-shot console hint at `info` level (deduplicated per
  {api, dropped_field}).

  ```ts
  import * as Sentry from "@goliapkg/sentori-react-native/compat";

  Sentry.init({ dsn: "https://<token>@<host>/<project>" });
  Sentry.captureException(err);
  Sentry.setUser({ id, email });
  // email → linkBy.email (hashed client-side; raw never sent)
  ```

  Coverage:

  - `Sentry.init({ dsn })` — parses Sentory's URL DSN, refuses non-
    Sentori tokens (must start `st_pk_`), warns on ignored fields
    (`integrations`, `beforeSend`, `attachStacktrace`, etc.)
  - `Sentry.captureException` / `Sentry.captureMessage` — same
    signature; `extra` merges into `tags` with a hint
  - `Sentry.setUser({ id, email, username, ip_address, segment })`
    — `email`/`username` → `linkBy.*` (hashed); `ip_address`
    dropped + hint; `segment` → tag `user.segment`
  - `Sentry.setTag` / `Sentry.setTags` — identical
  - `Sentry.addBreadcrumb({ category })` — category mapped to
    Sentori type via well-known table; original preserved in
    `data.category`
  - `Sentry.startTransaction({ op, name })` — returns Sentori Span
    with partial Sentry API surface (`.finish`, `.setStatus`,
    `.setTag`, `.startChild`)
  - `Sentry.withScope(fn)` / `Sentry.configureScope(fn)` — proxy
    scope that funnels to module-level setTag etc.
  - `Sentry.Severity.{Fatal/Critical/Error/Warning/Log/Info/Debug}` —
    enum maps to Sentori's 5-level scale (Critical → fatal, Log →
    info; one-shot hints fire on collapses)
  - `Sentry.flush` / `Sentry.close` — identical

  Not supported (compat throws clear errors at call site, pointing
  at the Sentori equivalent):

  - Custom transports
  - Hub manipulation (`getCurrentHub`, etc.)
  - `Sentry.Integrations.*` class registration
  - `Sentry.Replay` integration

  Native and compat share state (same scope, same transport, same
  identity layer). Mixing them in one app is supported.

  Documented at `docs-site/src/content/docs/sentry-compat.md` (that site was removed in 2026-08).

## 1.2.0

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

### Patch Changes

- Updated dependencies [[`ff0be91`](https://github.com/goliajp/sentori-selfhosted/commit/ff0be919b7d5cc0a1ba84e00d6203218806c5450), [`f4748cf`](https://github.com/goliajp/sentori-selfhosted/commit/f4748cf3f1030fb1df6fcc1f4bd5d6fd16d0aeca)]:
  - @goliapkg/sentori-core@0.10.0

## 1.1.0

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

- [`ff6d036`](https://github.com/goliajp/sentori-selfhosted/commit/ff6d03698d4bde47e857fd58e859910364032241) Thanks [@doracawl](https://github.com/doracawl)! - v2.0 W3 — top-level `flush()` / `close()` + `recordMetric` parent option.

  Two small additive APIs for v2 ergonomics. RN only — JS SDK doesn't ship a metrics module today and its transport is already fire-and-forget without a buffer to drain.

  **Top-level lifecycle:**

  ```ts
  await sentori.flush(5_000); // drains events / metrics / track within 5 s
  await sentori.close(); // flush + shut down; further captures are no-ops
  ```

  Both wrapped per the NEVER rule — individual buffer-flush failures are self-reported via the circuit-breaker, never propagated to the caller. The promise always resolves; the value is `undefined`.

  Use before short-lived process exit (CLI, fixture cleanup, serverless function) to make sure pending captures land before the process dies.

  **`recordMetric` parent option:**

  ```ts
  const span = sentori.startSpan({ name: "db.query users" });
  sentori.recordMetric("db.query.duration_ms", 42, undefined, { parent: span });
  span.end({ status: "ok" });
  ```

  When `opts.parent` is provided, the metric point carries `tags.span_id` + `tags.trace_id` so the dashboard can join metric series to a specific span. No schema change — the link rides on existing tag wire shape.

### Patch Changes

- Updated dependencies [[`09c823f`](https://github.com/goliajp/sentori-selfhosted/commit/09c823f4bcc9216f7c14943480dff390bef7d9de), [`cdddae4`](https://github.com/goliajp/sentori-selfhosted/commit/cdddae448347fe6fdb7ceeb87c9818b13a9844d0)]:
  - @goliapkg/sentori-core@0.9.0

## 1.0.0

### Stable cut — v1.x final

Stable 1.0.0 release. No behavioural change vs. `1.0.0-rc.10`; the `rc` tag is dropped now that the v1.x polish sprint is complete.

This release ships alongside the v1.x final polish (changesets framework adoption, monorepo dep-range overhaul to caret semantics, doc canonical-name sync to `captureException` / `sentori.init`, dashboard placeholder cleanup, perf-budget runbook). Trail in `CHANGELOG.md` (root) under "v1.0.0-rc.10" and below, and in the commits between [`5df0351`](https://github.com/goliajp/sentori-selfhosted/commit/5df0351) (v1.0.0-rc.1 cut) and the polish sprint commits.

Inter-package dep on `@goliapkg/sentori-core` now uses a caret range (`^0.8.3`) instead of an exact pin, so consumers can pick up core's patch updates without forcing a synchronised re-bump of every sibling.

If you were on `^1.0.0-rc`, upgrading to `1.0.0` is a no-op — the same package code with the `rc` label removed. If you were pinned to an explicit `1.0.0-rc.10`, change the spec to `^1.0.0` (or stricter as your tolerance for upstream patches dictates).

## 1.0.0-rc.10

See [the root CHANGELOG](../../CHANGELOG.md#v100-rc10--default-capture-rate-4-hz--2-hz-per-perf-rule) — full rc.4 → rc.10 entries are documented there during the v1.x polish doc-closeout. The root CHANGELOG remains the authoritative narrative for the rc series; this per-package CHANGELOG starts tracking from 1.0.0 stable forward.
