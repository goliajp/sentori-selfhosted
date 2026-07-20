// v2.1 W2 part 2 — RN runtime metrics flusher + cold-start
// instrument.
//
// The buffer + emit primitives live in @goliapkg/sentori-core
// (runtime-metrics.ts). This module owns:
//   • the periodic flush timer (30 s, coalesced w/ event flush)
//   • the rebuffer-on-failure recovery path
//   • the cold-start auto-instrument (one-shot at init)
//
// Other auto-instrument modules (FPS / heap / route-nav / network
// bytes) ship in the W2 part 3+ chunks; each is its own file with
// its own per-tick perf budget test gated as stop-ship in CI.
//
// NEVER rule: every public surface here is wrapped in safeFn
// boundaries by the SDK init layer; the flush timer itself
// catches all rejections + reports via the circuit breaker.

import {
  drainRuntimeMetricsForFlush,
  emitMetric,
  rebufferRuntimeMetrics,
  reportInternal,
} from '@goliapkg/sentori-core';

import { getConfig, isInitialized } from './config';
import { sendRuntimeMetricsBatch } from './transport';

const FLUSH_INTERVAL_MS = 30_000;

let _timer: null | ReturnType<typeof setInterval> = null;
let _coldStartT0: null | number = null;
let _coldStartEmitted = false;

/**
 * Drain core's runtime-metrics ring and POST to
 * /v1/runtime-metrics:batch. Rebuffers on failure so the next
 * tick retries; sustained outages spill into the ring's drop
 * counter which reports to the SDK self-report channel.
 *
 * Returns when the round-trip settles (success or failure). Per
 * the NEVER rule, never throws — failure is logged + rebuffered,
 * the resolved promise's value is undefined.
 */
export async function flushRuntimeMetrics(): Promise<void> {
  if (!isInitialized()) return;
  const config = getConfig();
  if (!config) return;
  const batch = drainRuntimeMetricsForFlush();
  if (batch.length === 0) return;
  const ok = await sendRuntimeMetricsBatch(config.ingestUrl, config.token, batch);
  if (!ok) {
    rebufferRuntimeMetrics(batch);
    reportInternal('runtime-metrics.flush', new Error('runtime-metrics POST failed'));
  }
}

/**
 * Start the 30 s flush timer. Called once from `init()`. Idempotent
 * — repeated calls are a no-op so users that call
 * `Sentori.init({ metrics: true })` more than once (HMR, fast
 * refresh) don't get multiple timers.
 */
export function startRuntimeMetricsTimer(): void {
  if (_timer !== null) return;
  _timer = setInterval(() => {
    void flushRuntimeMetrics();
  }, FLUSH_INTERVAL_MS);
  // Don't keep Node alive solely for this timer. RN's setInterval
  // is a NoopRef so this is harmless there; Node + CLI tests
  // benefit so the process can exit cleanly.
  (_timer as unknown as { unref?: () => void }).unref?.();
}

/**
 * Capture the wall-clock at `init()` so cold-start instrumentation
 * has a t0. Called from init() before any other auto-instrument
 * fires. Returns the captured t0 in millis (callers that want to
 * stash + use it later can hold the value).
 */
export function markColdStartT0(): number {
  if (_coldStartT0 === null) {
    _coldStartT0 = Date.now();
  }
  return _coldStartT0;
}

/**
 * Emit one `runtime.cold_start_ms` metric point. Idempotent per
 * session — the second + later calls are a no-op so route-nav
 * instrument (W2 part 3) can call this safely without worrying
 * about double-counting.
 *
 * Hosts that want a more interactive-pixel-perfect cold-start
 * boundary (TTI vs. raw init→render) can call
 * `markTimeToFullDisplay()` and we emit that separately as
 * `runtime.time_to_full_display_ms` (existing v1 instrument).
 */
export function emitColdStart(): void {
  if (_coldStartEmitted) return;
  if (_coldStartT0 === null) return;
  const ms = Date.now() - _coldStartT0;
  if (ms <= 0 || ms > 60_000) {
    // Implausible — likely the host called emit before init or
    // we got into a freeze-then-resume situation. Drop silently.
    return;
  }
  emitMetric('runtime.cold_start_ms', ms);
  _coldStartEmitted = true;
}

/**
 * Test-only escape hatch: stop the timer + reset cold-start state
 * so vitest / bun:test teardown doesn't leak intervals across runs.
 */
export function __resetRuntimeMetricsRnForTests(): void {
  if (_timer !== null) {
    clearInterval(_timer);
    _timer = null;
  }
  _coldStartT0 = null;
  _coldStartEmitted = false;
}
