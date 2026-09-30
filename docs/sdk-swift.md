# Sentori for Swift

Error, warning and push capture for iOS apps, with no React Native.

```swift
// Package.swift
dependencies: [
    .package(url: "https://github.com/goliajp/sentori-swift", from: "2.0.0")
],
targets: [
    .target(name: "YourApp", dependencies: [
        .product(name: "Sentori", package: "sentori-swift")
    ])
]
```

In Xcode, the product to tick is **Sentori**. The module you import
has the same name; a product and a module are separate things and are
not always spelled alike.

`from:` is a floor, not a pin: it takes the newest 2.x release. There
is no CocoaPods listing — the podspec is in the repository and the pod
is not on trunk, so SwiftPM is the only way in today.

iOS 14+. Apache-2.0 OR MIT.

## Start

You need two values first, and neither comes from this page: a
**token** (`st_…`, ingest scope) and the **ingest URL** of an instance
you run. There is no hosted signup — see
[getting started](./getting-started.md) for where both come from, and
[self-hosting](./self-hosting.md) for standing an instance up.

`start` has to run before anything the app does. In a SwiftUI app that
is the `App`'s initialiser; with an app delegate it is
`didFinishLaunchingWithOptions`. A top-level call in a source file is
not a place Swift will run it:

```swift
import Sentori
import SwiftUI

@main
struct YourApp: App {
    init() {
        Sentori.start(/* the config below */)
    }
    var body: some Scene { WindowGroup { ContentView() } }
}
```

```swift
import Sentori

Sentori.start(
    SentoriConfig(
        token: "st_…",                       // Settings ▸ Tokens, ingest scope
        ingestUrl: "https://sentori.example.com",   // YOUR instance
        release: "com.example.app@1.5.0+220",
        environment: "production"
    )
)
// Optional, and separate: without it a device receives broadcasts
// and cannot be reached from an issue.
Sentori.user(id: "the id your app already has", email: nil, traits: ["plan": "pro"])
```

Nothing here reaches the network — the first request happens when
there is something to send. Call it once, early; verbs called before
it are no-ops that still return an id, so a mis-wired token gives you
a silent SDK rather than an exception on a path you did not know you
had.

`release` is what a symbolicated stack is matched against. Use the
same string your dSYM upload uses.

## The five verbs

```swift
Sentori.error(err)                    // what went wrong?
Sentori.warn("checkout.slow")         // where did the user struggle?
Sentori.trace("cart.opened")          // what happened here?
Sentori.assert("total.positive", ok)  // should this hold?
Sentori.probe("SEN-482")              // is that bug back?
```

Every one is synchronous, returns the event id it minted, and never
throws. They do O(1) work on the calling thread — an append under a
lock — and everything expensive happens on a background queue. If the
network is gone, events spill to disk and drain on the next launch.

Three of them have a behaviour worth knowing:

- **`assert` never stops the program.** That is the difference from
  the language's own `assert` and the reason this one is safe to leave
  in a release build. A *passing* assert never becomes an event
  either — it increments a counter that rides the next batch, so a
  liveness check costs no request. Only failures are events.
- **`trace(_:quiet:)`** always lands in the signal ring; `quiet: true`
  keeps it out of the event stream, which is how a high-frequency
  breadcrumb stays affordable.
- **`probe`** is a tripwire. Reaching the call is the signal; it
  changes no control flow and returns no verdict.

Any of them takes `data:`:

```swift
Sentori.warn("checkout.slow", data: ["ms": 3200, "cartId": cart.id])
```

## Context

```swift
Sentori.context(["tenant": "acme", "plan": "pro"])   // rides every event
Sentori.pushSignal(kind: "nav", data: ["to": "/checkout"])
```

The signal ring is the last sixty seconds of what the user was doing,
shipped inside an error so the crash has a lead-up. Any `kind` is
accepted. The dashboard reads `http` as
`{ method, url, status, ms }` and `trace` as a quiet breadcrumb.

This SDK deliberately does **not** swizzle `URLSession`. Watching your
traffic is your decision, not ours to make silently — push an `http`
signal from your own interceptor if you want it.

## Identity

`Sentori.user(id:email:traits:)` sends a SHA-256 of the id (or of the email
when there is no id). The raw values never leave the device.

It is what makes a device reachable from an issue: with it, "notify
the people who hit this" is a join. Without it a registered device
receives broadcasts only, and Settings ▸ Push shows that as
"N devices, 0 addressable" — the one symptom with no other
explanation.

## Push

```swift
let result = await Sentori.push.register(
    onMessage: { payload in … },   // arrived while in the foreground
    onTap:     { data in … }       // the user opened it
)

if case .failure(let reason, let message) = result {
    // reason is .permissionDenied, .noTransport, .tokenTimeout,
    //           .serverRejected or .notInitialised
}
```

Call `Sentori.user` first if the device should be addressable.

`register` never throws, and is safe to call on every launch: iOS
returns its cached permission decision without re-prompting and the
server upserts the token. Each failure asks for something different:

| `reason` | what happened | what to do |
|---|---|---|
| `permissionDenied` | the user said no | nothing now. Offer it again from a settings screen — do **not** retry on a timer |
| `noTransport` | no push entitlement in this build | check the build; nothing to do at runtime |
| `tokenTimeout` | the OS never returned a token | usually provisioning. Retrying later is reasonable |
| `serverRejected` | Sentori answered non-2xx | look at Settings ▸ Push |
| `notInitialised` | `Sentori.start` has not run | a wiring bug |

`Sentori.push.unregister()` revokes it: the local handle is cleared,
the device is unregistered with APNs, and the server marks it revoked
so nothing more is sent to it. `cachedDeviceHandle()` returns the
handle without a round trip.

`register` returns an **spToken** — the address a backend sends to.
It belongs to the installation, not to the vendor's token, so APNs
issuing a new one (a reinstall, a restore from backup) updates the
same device rather than creating another. Whatever holds that spToken
keeps working.

The SDK reports a rotation as it happens rather than at the next
launch. Before that it stored the new token in a field and sent it to
nobody, so a rotated device received nothing until the app was next
started — which for a resident app is not a bounded wait.

`unregister` is the one thing that does change the address: it clears
the installation's local state, so the next `register` starts a new
one. A revoked device coming back should be a new registration, not a
resumed one.

Your app still needs the `aps-environment` entitlement and the
`remote-notification` background mode; the SDK does not add
capabilities to your target.

## Making a crash readable

A native stack arrives as addresses. The server turns them into file
and line names using the dSYM your build produced, matched by the
`release` string and the binary's UUID — so a crash is readable only
if the dSYM for that exact build was uploaded.

These commands need four values, and this page used to print them as
bare `$NAME` without saying where any of them came from:

```bash
ARCHIVE=build/YourApp.xcarchive            # xcodebuild -archivePath
VERSION=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' \
  "$ARCHIVE/Products/Applications/YourApp.app/Info.plist")
BUILD=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' \
  "$ARCHIVE/Products/Applications/YourApp.app/Info.plist")

export SENTORI_API_URL=https://sentori.example.com   # YOUR instance
export SENTORI_TOKEN=st_…                            # api scope
```

`--api-url` is not optional in a self-hosted world. Without it the CLI
defaults to `https://sentori.golia.jp`, which is GOLIA's own instance
— so the upload leaves your build machine, goes somewhere that is not
yours, and exits 0.

`$SENTORI_TOKEN` is the name the CLI reads, and `$SENTORI_ADMIN_TOKEN`
also works. No other spelling does: the CLI answers `--token is
required` for any of them, with the value sitting in the
environment.

Right after archiving, in CI:

```bash
npx @goliapkg/sentori-cli@latest upload dsym \
  --api-url "$SENTORI_API_URL" \
  --release "com.example.app@$VERSION+$BUILD" \
  --token "$SENTORI_TOKEN" \
  "$ARCHIVE/dSYMs/YourApp.app.dSYM"
```

The `--release` here and the `release` you pass to `Sentori.start`
must be the same string. They are matched literally; a build number
in one and not the other is a release the server has never heard of.

To see the failing line rather than only the function name, upload the
sources too:

```bash
npx @goliapkg/sentori-cli@latest upload srcbundle \
  --api-url "$SENTORI_API_URL" \
  --release "com.example.app@$VERSION+$BUILD" \
  --token "$SENTORI_TOKEN" Sources
```

An upload that fails exits 0 and prints the command to run by hand.
It is not your build's job to fail because our server was
unreachable, and a dSYM uploaded later is applied to crashes that
already arrived. If you would rather know at build time, add
`--strict`.

The step worth failing on is the one that asks the server what
actually landed:

```bash
npx @goliapkg/sentori-cli@latest artifacts check \
  --api-url "$SENTORI_API_URL" \
  --release "com.example.app@$VERSION+$BUILD" \
  --token "$SENTORI_TOKEN" --expect dsym
```

That catches the case a local "we ran the upload" note cannot: the
upload step that quietly stopped being called.

### What is captured

| Crash | Caught by |
|---|---|
| `NSException` | the uncaught-exception handler |
| force-unwrapped nil, index out of range, overflow | the signal handler (`SIGTRAP`) |
| `fatalError`, failed precondition, C `assert` | the signal handler (`SIGABRT`) |
| bad pointer, stack overflow | the signal handler (`SIGSEGV`) |

The signal handlers chain: whatever your app installed before calling
`Sentori.start` is kept and called, and the signal is re-raised with
the default disposition so the system still writes its own report. If
you already use another crash reporter, both of you get the crash.

A crash is written to disk as it happens and sent on the next launch —
the process is dying, and a network request is not something it can
finish.

## Check it works

Symbolication and push can wait. First make one crash appear.

```swift
// A temporary button, or anything you can reach twice.
Button("crash") { fatalError("sentori smoke test") }
```

Then, and this is the step people skip:

1. **Stop the debugger.** Xcode catches the signal first, so a crash
   run under the debugger never reaches the handler. Run the app, stop
   it in Xcode, launch it again from the device's home screen.
2. Tap the button. The app dies — that is the point.
3. **Launch the app a third time.** A crash is written to disk as the
   process dies and sent on the next launch; a dying process cannot
   finish a network request.
4. Open your instance, go to Issues, and the crash is the top row.

On a simulator, `localhost` works: the simulator shares the host's
network, so `http://localhost:8080` reaches a server running on your
Mac. An Android emulator is the one that needs `10.0.2.2` — see the
Kotlin page.

Nothing arrived? The verbs are no-ops before `start` runs, and they
still return an id, so a `start` that never executed looks exactly
like a quiet app. `SentoriConfig.isInitialised` answers that question
directly — check it after `start` and before you look anywhere else.

## What it costs you

The contract this SDK is written against is that adopting it is free:

- verbs never throw and never block the caller
- the in-memory queue is bounded at 500 events, the spill file at 1000
- a failure inside Sentori — a bad token, a dead server, a full disk —
  never becomes your failure
- values that cannot be encoded are replaced, not dropped, and never
  raise

If you ever measure Sentori costing your app something a user could
feel, that is a bug worth reporting as a P0.

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

## Also in the box

An uncaught `NSException` is written to disk as the app dies, along
with a screenshot of the last frame and the view tree behind it. The
next `Sentori.start` sends the crash, and once the server has taken
it, uploads the two blobs against it — in that order, because an
attachment keyed on an event the server has not seen is a 404.

**The view tree is a UIKit view tree.** It records a `UILabel`, a
`UITextView`, a `UIImageView` and anything with a background colour.
SwiftUI draws into layers instead of creating a view per view, so a
screen built entirely in SwiftUI yields almost nothing — measured on
a simulator: six nodes for a screen with UIKit views on it, one for
the same screen in pure SwiftUI. The screenshot is unaffected and is
the useful artefact there. If your app is SwiftUI, treat the
wireframe as empty until this says otherwise.

Nothing here needs configuring. The hang watchdog, thread sampler and
mobile vitals are compiled in and driven by the React Native SDK
today; they are not yet part of this public surface.
