# Sentori for Kotlin

Error, warning and push capture for Android apps, with no React Native.

```kotlin
dependencies {
    implementation("jp.golia.sentori:sentori:2.1.0")
}
```

`minSdk 24`, JVM target 17. Apache-2.0 OR MIT. The artifact is on
Maven Central, so `mavenCentral()` must be in your repositories.

The `Application` subclass below only runs if the manifest names it,
and a blank project's manifest does not:

```xml
<!-- AndroidManifest.xml -->
<application android:name=".App" …>
```

Without that line `onCreate` never executes, `start` never runs, and
every verb is a no-op that still returns an id — so the integration
looks finished and reports nothing at all.

The package is `com.sentori`, which is **not** the groupId:

```kotlin
import com.sentori.Sentori
import com.sentori.SentoriConfig
```

`jp.golia.sentori` identifies the artifact on Maven Central and
appears nowhere in the code. Guessing an import from it gives
`Unresolved reference 'jp'` on a dependency that resolved perfectly —
a combination that sends people looking at `mavenCentral()` and
transitive dependencies when the problem is one line. This section
exists because that happened.

## Start

You need two values first, and neither comes from this page: a
**token** (`st_…`, ingest scope) and the **ingest URL** of an instance
you run. There is no hosted signup — see
[getting started](./getting-started.md) for where both come from, and
[self-hosting](./self-hosting.md) for standing an instance up.

```kotlin
import com.sentori.Sentori
import com.sentori.SentoriConfig

class App : Application() {
    override fun onCreate() {
        super.onCreate()
        Sentori.start(
            SentoriConfig(
                token = "st_…",                          // Settings ▸ Tokens, ingest scope
                ingestUrl = "https://sentori.example.com",  // YOUR instance
                release = "com.example.app@1.5.0+220",
                environment = "production",
            ),
            context = this,
        )
        // Optional, and separate: without it a device receives
        // broadcasts and cannot be reached from an issue.
        Sentori.user(id = "the id your app already has", email = null, traits = mapOf("plan" to "pro"))
    }
}
```

Nothing here reaches the network — the first request happens when
there is something to send. The `Context` is used for one thing: the
directory events spill into when the network is gone. Pass the
application context, not an Activity.

Verbs called before `start` are no-ops that still return an id, so a
mis-wired token gives you a silent SDK rather than an exception on a
path you did not know you had.

`release` is what a symbolicated stack is matched against. Use the
same string your ProGuard mapping upload uses.

## The five verbs

```kotlin
Sentori.error(e)                       // what went wrong?
Sentori.warn("checkout.slow")          // where did the user struggle?
Sentori.trace("cart.opened")           // what happened here?
Sentori.assert("total.positive", ok)   // should this hold?
Sentori.probe("SEN-482")               // is that bug back?
```

Every one is synchronous, returns the event id it minted, and never
throws. They do O(1) work on the calling thread — an append under a
lock — and everything expensive happens on a background thread. If the
network is gone, events spill to disk and drain on the next launch.

Three have a behaviour worth knowing:

- **`assert` never stops the program.** That is the difference from
  the language's own `assert` and the reason this one is safe to leave
  in a release build. A *passing* assert never becomes an event
  either — it increments a counter that rides the next batch, so a
  liveness check costs no request. Only failures are events.
- **`trace(name, quiet = true)`** lands in the signal ring and stays
  out of the event stream, which is how a high-frequency breadcrumb
  stays affordable.
- **`probe`** is a tripwire. Reaching the call is the signal; it
  changes no control flow and returns no verdict.

Errors carry a real stack — up to 50 frames of class, method, file and
line:

```kotlin
try { checkout() } catch (e: Exception) {
    Sentori.error(e, mapOf("cartId" to cart.id))
}
```

## Context

```kotlin
Sentori.context(mapOf("tenant" to "acme", "plan" to "pro"))
Sentori.pushSignal("nav", mapOf("to" to "/checkout"))
```

The signal ring is the last sixty seconds of what the user was doing,
shipped inside an error so the crash has a lead-up. Any kind is
accepted. The dashboard reads `http` as `{ method, url, status, ms }`
and `trace` as a quiet breadcrumb.

This SDK deliberately does **not** install an OkHttp interceptor.
Watching your traffic is your decision, not ours to make silently —
push an `http` signal from your own interceptor if you want it. For
the same reason the SDK uses `HttpURLConnection` rather than OkHttp:
its own requests must not travel through the interceptors you
installed for yours, where a crash report could be logged, retried or
blocked by rules written for something else.

## Identity

`Sentori.user(id, email, traits)` sends a SHA-256 of the id (or of the email
when there is no id). The raw values never leave the device, and the
hash is byte-identical to the one the iOS and React Native SDKs
compute — the three are pinned to shared vectors, because a device
that hashed differently would stop matching its own events and nothing
would report it.

It is what makes a device reachable from an issue. Without it a
registered device receives broadcasts only, and Push ▸ Devices shows
that as "N devices, 0 addressable".

`traits` are what a push campaign selects on — plan, cohort, org. They
travel raw, unlike the id and email, so put nothing identifying there.
A call describes the person completely: one made without traits means
they have none, and signing out stops a device being selectable as
whoever just left.

A registered device sends this again by itself when it changes, so
calling `user` after `push.register` is fine. It was not: nothing
updated the row in between, and a device that registered before
sign-in carried no user for the life of the install.

## Push

The full signature, because it is a different shape from the Swift
one — two parameters, and a callback rather than a return value:

```kotlin
SentoriPush.register(
    context: Context,
    activity: Activity? = null,   // null skips the prompt and uses what is granted
    timeoutMs: Long = 8_000,      // how long to wait for a token, not for the user
    onMessage: ((Map<String, Any?>) -> Unit)? = null,
    onTap: ((Map<String, Any?>) -> Unit)? = null,
    completion: (Result) -> Unit,
)
```

Swift's is `await Sentori.push.register(onMessage:onTap:) -> Result`.
Android needs the `Context` for storage and the `Activity` to show the
Android 13+ prompt on, and reports through a callback because the
work spans a dialog.

`timeoutMs` is a network budget. Waiting for someone to read a
permission dialog is a different question and has its own budget,
`SentoriPush.permissionTimeoutMs` (two minutes by default) — a person
does not answer a prompt in the eight seconds it is reasonable to
wait for FCM.

In use:

```kotlin
SentoriPush.register(
    context = this,
    activity = this,                      // for the Android 13+ prompt
    onMessage = { payload -> … },         // arrived while in the foreground
    onTap = { data -> … },                // the user opened it
) { result ->
    if (result is SentoriPush.Result.Failure) {
        // result.reason is PERMISSION_DENIED, NO_TRANSPORT,
        //   TOKEN_TIMEOUT, SERVER_REJECTED or NOT_INITIALISED
    }
}
```

Call `Sentori.user` first if the device should be addressable. The
callback runs on a background thread.

`register` never throws, and is safe to call on every launch: Android
returns its cached permission decision without re-prompting and the
server upserts the token. Each failure asks for something different:

| `reason` | what happened | what to do |
|---|---|---|
| `PERMISSION_DENIED` | the user said no | nothing now. Offer it again from a settings screen — do **not** retry on a timer |
| `NO_TRANSPORT` | no Firebase in this build, or no `google-services.json` | check the build; nothing to do at runtime |
| `TOKEN_TIMEOUT` | FCM never returned a token | retrying later is reasonable |
| `SERVER_REJECTED` | Sentori answered non-2xx | look at Settings ▸ Push |
| `NOT_INITIALISED` | `Sentori.start` has not run | a wiring bug |

### A revocation is not a tombstone

Registering again with the same provider token brings the same row
back, and the handle does not change. That is on purpose: the two
things that revoke a device are the provider reporting its token dead
and the device revoking itself, and a device that registers again is
answering both — it is here, with a token the provider has just
issued it. Nothing an operator decided is being undone; there is no
third way to revoke.

What changes is the trail. The reason a device was quarantined is
dropped when it comes back, and `revived_at` is stamped instead — a
live device should not be described by the failure that killed a
token it no longer has.

The one case where the handle *does* change is `unregister`, which
clears the local handle too, so the next `register` arrives with a
fresh provider token and starts a new row.

### Taps, and what still needs you

A notification Sentori posted carries a pending intent of its own, so
`onTap` fires on its own — cold start or warm, nothing to wire up.
That is the path the Sentori server produces: it sends `data`
messages, and the SDK draws the tray entry itself.

Two cases are not ours to close:

- **A `notification` message from another sender.** The system draws
  it and never calls our service. A cold start still works — `register`
  reads the intent the Activity was launched with — but a tap while
  the app is already running goes to your `onNewIntent`, and only you
  can see it. One line closes it:

  ```kotlin
  override fun onNewIntent(intent: Intent) {
      super.onNewIntent(intent)
      SentoriNotificationTap.consume(intent.extras)
  }
  ```

  Safe with any intent: an ordinary launch is not a tap and is
  ignored, and the same tap is never reported twice. This paragraph
  used to end at "only you can see it", while the object it names was
  `internal` and could not be called at all — leaving
  `handleNotificationTap`, which records whatever it is handed,
  including launches that were never taps.
- **A data message with no `title` and no `body`.** Nothing is drawn,
  on purpose: an app that uses silent data messages should not get a
  notification per message. `onMessage` still fires.

`SentoriPush.unregister(context)` revokes it: the local handle is
cleared, the provider token is deleted, and the server marks the
device revoked so nothing more is sent to it.

`SentoriPush.cachedDeviceHandle(context)` returns the handle without a round trip.

### The address survives a rotated token

`register` returns an **spToken** — the address a backend sends to.
It belongs to the installation, not to the vendor's token, so APNs or
FCM issuing a new one (a reinstall, a restore from backup, cleared
app data) updates the same device rather than creating another.
Whatever holds that spToken keeps working.

The SDK reports a rotation as it happens rather than at the next
launch. Before that it stored the new token in a field and sent it to
nobody, so a rotated device received nothing until the app was next
started — which for a resident app is not a bounded wait.

`unregister` is the one thing that does change the address: it clears
the installation's local state, so the next `register` starts a new
one. That is deliberate — a revoked device coming back should be a
new registration, not a resumed one.

## Making a crash readable

A native stack arrives with obfuscated names. The server de-obfuscates
them using the R8 / ProGuard mapping your build produced, matched by
the `release` string — so a crash is readable only if the mapping for
that exact build was uploaded.

That mapping only exists if R8 ran. A blank project ships with
`minifyEnabled false`, so `mapping.txt` is not there and the upload
below fails on a missing file:

```kotlin
// app/build.gradle.kts
android {
    buildTypes {
        release {
            isMinifyEnabled = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"))
        }
    }
}
```

The commands need three values:

```bash
VERSION=$(./gradlew -q printVersionName)   # or read it from your own build logic
BUILD=$(./gradlew -q printVersionCode)

export SENTORI_API_URL=https://sentori.example.com   # YOUR instance
export SENTORI_TOKEN=st_…                            # api scope
```

`--api-url` is not optional in a self-hosted world. Without it the CLI
defaults to `https://sentori.golia.jp`, which is GOLIA's own instance
— so the mapping leaves your build machine, goes somewhere that is not
yours, and exits 0.

`$SENTORI_TOKEN` is the name the CLI reads, and `$SENTORI_ADMIN_TOKEN`
also works. No other spelling does.

Right after the release build, in CI:

```bash
npx @goliapkg/sentori-cli@latest upload mapping \
  --api-url "$SENTORI_API_URL" \
  --release "com.example.app@$VERSION+$BUILD" \
  --token "$SENTORI_TOKEN" \
  app/build/outputs/mapping/release/mapping.txt
```

The `--release` here and the `release` you pass to `Sentori.start`
must be the same string. They are matched literally; a build number
in one and not the other is a release the server has never heard of.

To see the failing line rather than only the method name, upload the
sources too:

```bash
npx @goliapkg/sentori-cli@latest upload srcbundle \
  --api-url "$SENTORI_API_URL" \
  --release "com.example.app@$VERSION+$BUILD" \
  --token "$SENTORI_TOKEN" app/src/main/java
```

An upload that fails exits 0 and prints the command to run by hand.
It is not your build's job to fail because our server was
unreachable, and a mapping uploaded later is applied to crashes that
already arrived. If you would rather know at build time, add
`--strict`.

The step worth failing on is the one that asks the server what
actually landed:

```bash
npx @goliapkg/sentori-cli@latest artifacts check \
  --api-url "$SENTORI_API_URL" \
  --release "com.example.app@$VERSION+$BUILD" \
  --token "$SENTORI_TOKEN" --expect proguard
```

That catches the case a local "we ran the upload" note cannot: the
upload step that quietly stopped being called.

### ANR

On Android 11 and later an ANR is read from the system's own record
(`ApplicationExitInfo`) at the next launch — the same source Play
Console reports on, with the system's own trace of where the main
thread was stuck. Nothing to configure, and no number of ours that
can disagree with the one on your release dashboard.

Below 11 there is no such record, so a watchdog detects a blocked main
thread instead. It is an inference, and a debugger pause looks like
one; it runs only on those devices.

## Check it works

Symbolication and push can wait. First make one crash appear.

```kotlin
// A temporary button, or anything you can reach twice.
findViewById<Button>(R.id.crash).setOnClickListener {
    throw IllegalStateException("sentori smoke test")
}
```

Then, and this is the step people skip:

1. **Detach the debugger.** Run the app, stop it in Android Studio,
   then launch it again from the launcher.
2. Tap the button. The app dies — that is the point.
3. **Launch the app a third time.** A crash is written to disk as the
   process dies and sent on the next launch; a dying process cannot
   finish a network request.
4. Open your instance, go to Issues, and the crash is the top row.

On an emulator, `ingestUrl` cannot be `localhost` — that is the
emulator itself. The host is `http://10.0.2.2:8080`.

Nothing arrived? Check the manifest line first (`android:name=".App"`,
above): without it `start` never runs, and the verbs stay no-ops that
return an id, so a missing integration and a quiet app look the same.
`SentoriConfig.isInitialised` tells the two apart.

## What it costs you

The contract this SDK is written against is that adopting it is free:

- verbs never throw and never block the caller
- the in-memory queue is bounded at 500 events, the spill file at 1000
- a failure inside Sentori — a bad token, a dead server, a full disk —
  never becomes your failure
- `firebase-messaging` is `compileOnly`, so push costs nothing to an
  app that does not use it

If you ever measure Sentori costing your app something a user could
feel, that is a bug worth reporting as a P0.

## Also in the box

An uncaught exception is written to disk as the app dies, along with a
screenshot of the last frame and the view tree behind it. The next
`Sentori.start` sends the crash, and once the server has taken it,
uploads the two blobs against it — in that order, because an
attachment keyed on an event the server has not seen is a 404.

Nothing here needs configuring. The ANR watchdog and mobile vitals are
compiled in and driven by the React Native SDK today; they are not yet
part of this public surface.
