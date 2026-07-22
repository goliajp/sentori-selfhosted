// v2.1 W2 part 3 — FPS auto-instrument.
//
// Rolling 5 s window of inter-frame Δt measured via rAF; emit
// runtime.fps.p50 + runtime.fps.p95 every 5 s.
//
// Perf bedrock (.claude/CLAUDE.md):
//   • per-tick cost must be < 0.5 ms on a Pixel-5-equivalent
//     bench → stop-ship gate in W2 part 4 CI
//   • main-thread sustained < 1 %
//   • no allocation inside the rAF callback hot path
//
// Implementation choices:
//   • single-frame Δt = perf.now() - prev — cheap, no allocation
//   • rolling window stored in a fixed-size Float32Array (no
//     splice / shift — circular buffer write index)
//   • percentile computed by copying-and-sorting only at emit
//     time (every 5 s, ~300 floats) — no per-tick sort
//
// Bail-outs:
//   • if requestAnimationFrame isn't available (rare RN host),
//     never start the loop
//   • if perf.now() is missing (older RN engines), fall back to
//     Date.now() — coarser but better than crashing

import { emitMetric } from '@goliapkg/sentori-core';

const TICK_FRAMES_BEFORE_EMIT = 300; // ~5 s at 60 fps
const SAMPLE_CAP = 600; // safety: 10 s at 60 fps caps memory

let _running = false;
let _samples = new Float32Array(SAMPLE_CAP);
let _write = 0;
let _count = 0;
let _prev = 0;

function now(): number {
  // Engines without perf.now() (very old RN runtimes / minimal
  // JSCore builds) fall back to wall-clock. Date.now() has 1 ms
  // resolution so FPS readings cap at 1000; good enough.
  const p = (globalThis as { performance?: { now?: () => number } }).performance;
  return p?.now ? p.now() : Date.now();
}

function rafTick(t: number): void {
  if (!_running) return;
  if (_prev !== 0) {
    const dt = t - _prev;
    if (dt > 0 && dt < 1000) {
      _samples[_write] = dt;
      _write = (_write + 1) % SAMPLE_CAP;
      _count += 1;
      if (_count >= TICK_FRAMES_BEFORE_EMIT) {
        emitWindow();
        _count = 0;
        _write = 0;
      }
    }
  }
  _prev = t;
  requestAnimationFrame(rafTick);
}

function emitWindow(): void {
  // Copy + sort. ~300 floats, well under 1 ms even on a slow phone.
  // Allocation lives outside the per-tick hot path — only at emit.
  const n = _count;
  if (n === 0) return;
  const slice = new Float32Array(n);
  for (let i = 0; i < n; i++) slice[i] = _samples[i]!;
  slice.sort();
  const p50dt = percentile(slice, 0.5);
  const p95dt = percentile(slice, 0.95);
  // FPS = 1000 ms / per-frame Δt.
  const fpsP50 = p50dt > 0 ? Math.round(1000 / p50dt) : 0;
  // p95 of Δt is the *slowest* 95 % — i.e. when frames are bad.
  // p95 FPS conventionally means "5 % worst frames as fps".
  const fpsP5Slow = p95dt > 0 ? Math.round(1000 / p95dt) : 0;
  emitMetric('runtime.fps.p50', fpsP50);
  emitMetric('runtime.fps.p95', fpsP5Slow);
}

function percentile(sorted: Float32Array, q: number): number {
  if (sorted.length === 0) return 0;
  // Discrete percentile — index = ceil(q * n) - 1, clamped.
  const idx = Math.min(sorted.length - 1, Math.max(0, Math.ceil(q * sorted.length) - 1));
  return sorted[idx]!;
}

/** Idempotent start. Safe to call multiple times; only the first
 *  call kicks off the rAF loop. */
export function startFpsInstrument(): void {
  if (_running) return;
  if (typeof requestAnimationFrame !== 'function') return;
  _running = true;
  _prev = 0;
  _count = 0;
  _write = 0;
  requestAnimationFrame(rafTick);
}

/** Stop the loop. Used by tests + by hosts that want to opt out
 *  mid-session. */
export function stopFpsInstrument(): void {
  _running = false;
}

/** Test-only state inspection. */
export function __peekFpsInstrumentState(): {
  count: number;
  running: boolean;
} {
  return { count: _count, running: _running };
}

/** Test-only force-emit (skips the every-300-frames gate). */
export function __forceEmitFpsForTests(): void {
  emitWindow();
  _count = 0;
  _write = 0;
}

/** Test-only sample injection so we can assert without spinning
 *  rAF in the test runner. */
export function __pushSampleForTests(dt: number): void {
  _samples[_write] = dt;
  _write = (_write + 1) % SAMPLE_CAP;
  _count += 1;
}

/** Test-only reset between runs. */
export function __resetFpsInstrumentForTests(): void {
  _running = false;
  _samples = new Float32Array(SAMPLE_CAP);
  _write = 0;
  _count = 0;
  _prev = 0;
}
