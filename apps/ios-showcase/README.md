# SentoriShowcase — iOS 26 native demo

The flagship iOS showcase for Sentori. Pure SwiftUI 6, iOS 26 deployment
target, Liquid Glass everywhere it earns its keep. Imports the Sentori
SDK's Swift core directly — no React Native, no Expo, no JS bridge.

This is the open-source project's front door. Optimise for delight.

## What it demonstrates

- Crash + caught-error capture (`SentoriCrashHandler`)
- Wireframe replay ring sampler at 2 Hz (`SentoriReplayCapture`)
- Key-window screenshot (`SentoriScreenshotCapture`)
- Mobile vitals: cold-start ms, slow / frozen frame counters (`SentoriMobileVitals`)
- Hang watchdog (`SentoriHangWatchdog`)
- Native exception bridge (`SentoriNativeExceptionBridge`)

Each action card on the home surface triggers one of these. The replay
ring fills live so you can watch frames stream in.

## Build & run

```sh
brew install xcodegen        # one-time
xcodegen generate            # regenerates SentoriShowcase.xcodeproj
xcrun simctl boot sim-sentori
xcodebuild \
  -project SentoriShowcase.xcodeproj \
  -scheme SentoriShowcase \
  -configuration Debug \
  -destination 'platform=iOS Simulator,name=sim-sentori' \
  -derivedDataPath build/ \
  CODE_SIGNING_ALLOWED=NO \
  build
xcrun simctl install sim-sentori \
  build/Build/Products/Debug-iphonesimulator/SentoriShowcase.app
xcrun simctl launch sim-sentori jp.golia.sentori.showcase
```

The `SentoriShowcase.xcodeproj` is generated, not committed — `project.yml`
is the source of truth. Re-run `xcodegen` after editing it.

## Layout

```
SentoriShowcase/
  SentoriShowcaseApp.swift   @main entry
  ContentView.swift          top-level page (hero → KPI → grid → ring → log)
  Services/
    SentoriService.swift     @Observable bridge to the SDK
  Theme/
    Theme.swift              palette + type tokens (mirrors the dashboard)
  Views/
    HeroSection.swift        brand + tagline + live status pill
    KPIRow.swift             3 numeric cells
    ActionGrid.swift         demo cards (Liquid Glass + symbol effects)
    ReplayRingPanel.swift    live ring counter + last-probe readout
    EventLog.swift           recent SDK events stream
  Info.plist
  Assets.xcassets
```

## Why no Podfile

The Sentori SDK's iOS source files compile directly into this app's
target — see the `sources:` block in `project.yml`. `SentoriModule.swift`
is intentionally excluded (it's the Expo Modules JS bridge wrapper,
irrelevant in a pure-native app).

If we ever extract a standalone `Sentori-iOS` pod, the showcase will
switch to a Podfile reference and lose its `sources:` boilerplate.

## Design notes

- Palette mirrors `web/src/index.css` so the showcase reads as the
  same product as the dashboard.
- Tora-orange (#FF7848 in dark mode) is the only chromatic accent.
  Semantic colours (danger / warning / success / info) reserved for
  state.
- All cards use `.ultraThinMaterial` (Liquid Glass) over the warm
  dark backdrop.
- Symbols animate on tap via `symbolEffect(.bounce.up, value:)`.
- Type defaults to system rounded weight semibold for display;
  SF Mono for technical readouts.
