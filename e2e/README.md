# Sentori e2e

End-to-end smoke test. Boots `sentori-server` and exercises the
`@sentori/react-native` SDK transport from a bun process to verify the
SDK↔server protocol matches.

## Run

```bash
bash e2e/run.sh
```

Exits 0 on success; non-zero on any check failure.

## What it does

1. `bun install` in `e2e/` (links `@sentori/react-native`)
2. `cargo build --bin sentori-server`
3. Starts the server on `127.0.0.1:8080` in the background
4. Uses the SDK in bun to send one event via `sentori.captureError(...)`
5. Polls `GET /v1/events/_recent` and asserts the event arrived

## What it does NOT do

- Does not boot iOS simulator or Android emulator
- Does not load the example app

Simulator end-to-end is manual for v0.1 — see
`apps/rn-example/README.md` for the iOS / Android bring-up steps.
GUI automation (`xcrun simctl`-driven button taps, `adb shell`, GitHub
Actions macOS runner) is deferred to v0.2.

## Files

| file | role |
|---|---|
| `run.sh` | the smoke test |
| `send-event.ts` | bun script that drives the SDK |
| `package.json` | links the SDK via `file:` |
