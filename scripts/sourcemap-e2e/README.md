# Source-map e2e (Phase 16 sub-E, 回填 Phase 8)

Round-trip test: minified RN bundle → upload via `sentori-cli upload
sourcemap` → trigger a deliberately-thrown error in a fixture → fetch
the issue from the dashboard API → assert the stack frame names + line
numbers map back to the original source positions.

## What's here

- `app.tsx` — fixture component with a `throw` deep enough to produce
  a real-looking stack.
- `metro.fixture.config.js` — Metro bundler config that emits both
  `bundle.js` (minified) and `bundle.js.map` to `dist/`.
- `run.sh` — orchestrator: bundle → upload → trigger → poll → assert.

## Prereqs

- Server running with PG + Valkey, dev token in `SENTORI_DEV_TOKEN`.
- `sentori-cli` built: `cargo build --release --manifest-path cli/Cargo.toml`.
- bun (for Metro / RN bundling).

## Run

```sh
SENTORI_BASE=http://localhost:8080 \
SENTORI_DEV_TOKEN=devtoken \
SENTORI_PROJECT_ID=019508a0-0000-7000-8000-000000000000 \
./run.sh
```

The script exits non-zero if any frame in the symbolicated stack still
points at `bundle.js` (i.e. symbolication didn't kick in) or if the
top frame's function name doesn't match the source.

## What can't be automated yet

- Running the bundle inside an actual JS engine (Hermes / V8) is left
  to the simulator e2e job (`mobile-e2e.yml`). This script uses Node
  to evaluate the bundle so we get a stack with real minified line
  numbers without bringing up an emulator.
