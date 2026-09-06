# Replay encoding v2 — keyframe + delta

Ship target: `@goliapkg/sentori-react-native@1.0.0-rc.9` + dashboard (deploys together — no over-the-wire backwards compat, since this is pre-1.0).

## Why v2

Pre-rc.9 every replay tick captured a full wireframe snapshot. For a typical 200-node dense UI at 1 Hz × 60 s, that produced ~300 KB NDJSON. Most adjacent frames had near-identical content, so most of the bytes were redundant.

v2 replaces "one full snapshot per tick" with "one full snapshot every ~4 seconds (a *keyframe*) plus small deltas in between". Same temporal coverage, 50–70 % less wire bytes, and the headroom lets us **lift the native tick rate from 1 Hz to 4 Hz** so the playback timeline reads as motion rather than a stuttery slideshow. The dashboard player tweens between captured states at 24 fps via cross-fade, so the perceived smoothness exceeds the capture rate without spending more bytes.

## Tick rate

| | rc.8 | rc.9 | rc.10 |
|---|---|---|---|
| Native tick interval | 1000 ms | 250 ms (4 Hz) | **500 ms (2 Hz)** |
| Source samples per 60 s window | 60 | 240 | 120 |
| Playback target | step-frame | 24 fps cross-fade | 24 fps cross-fade |

rc.10 rolled the default back to 2 Hz after iOS sim verify measured ~1 ms / tick on a thin dev panel — fine on iOS, but extrapolation to dense (200-node) Android UIs pushes JS-thread occupancy past 1 %, violating the project's "几乎不能造成性能抖动" rule. Apps that want smoother motion can opt into `replay.hz: 4` explicitly; the cross-fade renderer adapts automatically (more captures means tighter bracketing, less interpolation distance).

The `replay.hz` SDK option still accepts 1 / 2 / 4 / 8. Floor is 100 ms (10 Hz) under MIN_TICK_PERIOD_MS — don't crank past that.

## Ring buffer

Eviction switches from `RING_SIZE = 60` fixed-count to **time-based**: keep entries with `ts > now - 60_000`. Hard memory cap `MAX_RING_ITEMS = 1000` so a wedged ts clock can't blow the heap.

## Wire format — NDJSON, one line per emit

Two line shapes. Both carry `ts` (capture millis epoch) and a discriminator field.

### Keyframe line

```jsonc
{
  "ts": 1779000000000,
  "kind": "key",
  "width": 1080,
  "height": 2340,
  "nodes": [
    {"x":0,"y":0,"w":1080,"h":2340,"kind":"rect","color":"#0E0E10FF"},
    {"x":60,"y":192,"w":960,"h":112,"kind":"text","text":"Sentori","color":"#FFFFFFB3"},
    ...
  ]
}
```

Identical shape to the rc.8 single-frame payload **plus** a top-level `kind: "key"` discriminator. Player anchors all reconstruction off the most recent keyframe ≤ target time.

### Delta line

```jsonc
{
  "ts": 1779000000250,
  "kind": "delta",
  "added":   [/* full node objects */],
  "changed": [/* full node objects — same fingerprint as a prev node, but other fields differ */],
  "removed": [
    {"x":105,"y":618,"w":870,"h":57}
  ]
}
```

- `added`: nodes whose spatial fingerprint `${x},${y},${w},${h}` did not exist in the *previous emit's reconstructed state*.
- `changed`: nodes whose fingerprint matched a prev node but whose `kind` / `text` / `color` differs.
- `removed`: prev-state nodes whose fingerprint is absent in the new tick. Only `x,y,w,h` are required (fingerprint is enough to delete).

All three arrays may be empty. A delta with all-empty arrays is a *no-op heartbeat* and is **dropped** before write (saves 60 bytes per static-UI tick).

## Keyframe cadence

A keyframe is emitted when:

1. The replay subsystem first starts (cold or after `stopReplay()`).
2. `ts - lastKeyframeTs >= 4000 ms` (4 s default; tunable via `replay.keyframeMs` option).
3. The delta against the last reconstructed state has more entries than re-keying would cost — i.e. when `added.length + changed.length + removed.length >= currentNodeCount * 0.4`. Catches "big screen transition" cases where the delta would be larger than a fresh keyframe.

Rule 3 caps worst-case delta size. Rule 2 caps reconstruction chain length (≤ 16 deltas at 4 Hz, sub-ms to replay forward).

## Reconstruction

To render state at any timestamp `T`:

1. Find `keyframeIdx` = largest `i` such that `lines[i].kind === "key"` and `lines[i].ts <= T`.
2. Start from `lines[keyframeIdx]`'s nodes.
3. For each `lines[j]` with `j > keyframeIdx` and `lines[j].ts <= T` (in order), apply delta:
   - For each entry in `removed`: delete the node with that fingerprint from current state.
   - For each in `added`: add the node.
   - For each in `changed`: replace the node with same fingerprint with the new fields.
4. Return current state.

Player memoises last reconstructed `(T, state)` — scrubbing forward at 24 fps means usually the next render reuses the prior state plus one or two deltas, not a full rewind to keyframe.

## Player — playback model

| | rc.8 | rc.9 |
|---|---|---|
| Seek axis | frame index (1..N) | **seconds within 60 s window** |
| Playback render | step (one state) | **rAF @ 24 fps, cross-fade between bracketing captures** |
| Frame-list panel | full list of captures | timeline scrubber + segment markers (keyframe = solid tick, delta = hairline tick) |

Cross-fade renders **two** `<g>` SVG layers stacked. Render time `T` finds bracketing capture times `t_before <= T <= t_after`; the `before` layer renders at opacity `1 - α`, the `after` layer at opacity `α`, where `α = (T - t_before) / (t_after - t_before)`. With 4 Hz capture and 24 fps render, each rendered frame interpolates over a 6-render-frame window — eyes read this as motion rather than stutter.

**Position interpolation** (linear-tween x/y for "moving" nodes) is **not in scope for rc.9** — without stable node IDs from the walker, we can't reliably correlate two fingerprints that differ only by position. Cross-fade is the honest visualization for this data shape.

## Size budget (dense-UI baseline)

- rc.8: 60 s × 1 Hz × ~5 KB/snapshot = **~300 KB**
- rc.9 @ 4 Hz, 4 s keyframe: 15 keyframes × 5 KB + 225 deltas × ~0.5 KB = **~190 KB** at **4× temporal resolution**
- rc.9 @ 8 Hz, 4 s keyframe: 15 keyframes × 5 KB + 465 deltas × ~0.5 KB = ~310 KB at 8× resolution (only marginally worse than rc.8 baseline)

Server cap stays 16 MB per attachment (rc.6). Comfortable headroom for both rate settings.

## What the SDK code change looks like

`sdk/react-native/src/replay.ts`:

```ts
type Frame = { ts: number; width: number; height: number; nodes: Node[] };

// Mutable state in the encoder:
let _lastFrameState: Map<string, Node> | null = null;  // fingerprint → node
let _lastKeyframeTs: number = 0;
let _ring: string[] = [];  // NDJSON-encoded lines

function captureTick() {
  const snapshot = parseNativeSnapshot();  // current behaviour
  const currentState = new Map(snapshot.nodes.map((n) => [fingerprint(n), n]));

  const shouldKeyframe =
    _lastFrameState === null ||
    snapshot.ts - _lastKeyframeTs >= KEYFRAME_MS ||
    deltaSizeWouldExceedKeyframe(_lastFrameState, currentState);

  if (shouldKeyframe) {
    _ring.push(JSON.stringify({ ts: snapshot.ts, kind: "key", width: snapshot.width, height: snapshot.height, nodes: snapshot.nodes }));
    _lastKeyframeTs = snapshot.ts;
  } else {
    const delta = computeDelta(_lastFrameState, currentState);
    if (delta.added.length + delta.changed.length + delta.removed.length === 0) {
      // heartbeat no-op; drop
    } else {
      _ring.push(JSON.stringify({ ts: snapshot.ts, kind: "delta", ...delta }));
    }
  }
  _lastFrameState = currentState;
  evictByTime(now() - 60_000);
}
```

`drainReplay` unchanged: just `_ring.join("\n")`.

## What the dashboard code change looks like

- New `web/src/lib/replay-reconstruct.ts`: pure functions `parseLines(text) → Line[]`, `reconstructAt(lines, ts) → Snapshot`, with the last-state memo.
- `ReplayPlayer.tsx`: rAF loop at 24 fps; on each tick computes `currentTime` from `playStart + Date.now() - perfStart`, calls reconstruct twice (before/after), renders cross-faded.
- `replay-tab.tsx`: same.
- Timeline scrubber: HTML `<input type="range" min={0} max={duration} step={0.01}>` plus a parallel SVG track showing keyframe ticks (visual cue for cadence).

## Open question for later

Real motion interpolation needs stable node IDs from the walker. Two paths if/when we do this:

1. Synthesise a hash ID from `(parent-fingerprint, child-index, kind, text)` — stable across positional drift.
2. Have the walker emit `View.hashCode()` (Android) / `ObjectIdentifier(view)` (iOS). Both are GC-lifetime-stable.

Either lets the player diff "node X moved from (a,b) to (c,d) at t=5.4s" and emit linear-tween position frames. Not in rc.9 scope; revisit if cross-fade feels visually insufficient.

## Versioning

rc.9 is the only release that carries the v2 wire format. There is no `v: 2` envelope field — the keyframe/delta `kind` discriminator already serves as the schema marker (rc.8 NDJSON has *only* fields `ts/width/height/nodes` per line, no top-level `kind`). The dashboard parser distinguishes:

- Line has `kind: "key"` → v2 keyframe
- Line has `kind: "delta"` → v2 delta
- Line has neither (only `nodes` etc) → v1 (rc.8) full snapshot — for archival event playback during the rollover window.

So old events captured under rc.8 still play (rendered as keyframe-only timelines with frame-rate = source rate). New events captured under rc.9 play with cross-fade. No data migration needed.
