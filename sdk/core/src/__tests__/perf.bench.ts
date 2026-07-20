/**
 * Phase 47.04 — perf-budget gate for the SDK's hot paths.
 *
 * Not a unit test — we assert that each hot operation stays under a
 * fixed wall-clock budget so the SDK can't regress in a future PR
 * without a visible test failure. Budgets are intentionally generous
 * (10x the typical observed time) to keep the gate stable on a
 * shared CI runner; if any of these *fails* we have a real regression.
 *
 * Run with:
 *     bun test src/__tests__/perf.bench.ts
 */

import { describe, expect, test } from 'bun:test'

import { addBreadcrumb, clearBreadcrumbs, getBreadcrumbs } from '../breadcrumbs.js'
import {
  type LogTransport,
  logger,
  setLogLevel,
  setLogTransport,
} from '../logger.js'
import {
  __resetRuntimeMetricsForTests,
  drainRuntimeMetricsForFlush,
  emitMetric,
} from '../runtime-metrics.js'
import { shouldSample, shouldSampleTrace } from '../sampling.js'
import { TrailBuffer, sealTrail } from '../trail.js'
import { uuidV7 } from '../uuid.js'

function timed(label: string, loops: number, fn: () => void): number {
  // Warm-up — eject any first-call JIT cost from the measurement.
  for (let i = 0; i < Math.min(loops, 1000); i++) fn()
  const start = performance.now()
  for (let i = 0; i < loops; i++) fn()
  const total = performance.now() - start
  // Per-op µs.
  const perOp = (total * 1000) / loops
  // eslint-disable-next-line no-console
  console.log(`bench: ${label} ${perOp.toFixed(2)} µs/op (${loops} loops)`)
  return perOp
}

describe('SDK perf budget', () => {
  test('uuidV7 < 5 µs/op', () => {
    const perOp = timed('uuidV7', 50_000, () => {
      uuidV7()
    })
    expect(perOp).toBeLessThan(5)
  })

  test('shouldSample(rate) < 1 µs/op', () => {
    const perOp = timed('shouldSample', 100_000, () => {
      shouldSample(0.5)
    })
    expect(perOp).toBeLessThan(1)
  })

  test('shouldSampleTrace(traceId, rate) < 5 µs/op', () => {
    const id = '019eaa00000070008000000000000001'
    const perOp = timed('shouldSampleTrace', 100_000, () => {
      shouldSampleTrace(id, 0.5)
    })
    expect(perOp).toBeLessThan(5)
  })

  test('addBreadcrumb + getBreadcrumbs round-trip < 10 µs/op', () => {
    clearBreadcrumbs()
    const perOp = timed('breadcrumb round-trip', 20_000, () => {
      addBreadcrumb('custom', { x: 1 })
      getBreadcrumbs()
    })
    expect(perOp).toBeLessThan(10)
  })

  test('TrailBuffer.push (eviction path) < 1 µs/op', () => {
    const buf = new TrailBuffer(30)
    const perOp = timed('TrailBuffer.push', 50_000, () => {
      buf.push({ label: 'step', ts: Date.now() })
    })
    expect(perOp).toBeLessThan(1)
  })

  test('sealTrail(buffer) < 50 µs', () => {
    const buf = new TrailBuffer(30)
    for (let i = 0; i < 30; i++) buf.push({ label: `step-${i}`, ts: i })
    // sealTrail allocates — measured separately as one-shot wall time.
    const perOp = timed('sealTrail', 5_000, () => {
      sealTrail(buf)
    })
    expect(perOp).toBeLessThan(50)
  })

  // v2.1 W2 — runtime metrics emit/drain hot paths. Auto-instrument
  // (FPS / heap / route-nav / network) hits emitMetric on every tick;
  // the budget must clear with margin so the SDK's main-thread
  // contribution stays inside the .claude/CLAUDE.md ceiling
  // (< 1 % sustained, < 5 ms per tick at app level — emitMetric
  // is one of dozens of per-tick calls so its individual budget
  // is much tighter).
  test('emitMetric (no tags) < 5 µs/op', () => {
    __resetRuntimeMetricsForTests()
    const perOp = timed('emitMetric.no-tags', 50_000, () => {
      emitMetric('runtime.fps.p50', 60)
    })
    // Ring overflow at 10k cap — drain mid-test to keep the
    // memory pattern stable across loop iterations.
    drainRuntimeMetricsForFlush()
    expect(perOp).toBeLessThan(5)
  })

  test('emitMetric (with 3 tags) < 10 µs/op', () => {
    __resetRuntimeMetricsForTests()
    const perOp = timed('emitMetric.3-tags', 30_000, () => {
      emitMetric('runtime.route_nav_ms', 120, {
        from: 'Home',
        to: 'Profile',
        os: 'ios',
      })
    })
    drainRuntimeMetricsForFlush()
    expect(perOp).toBeLessThan(10)
  })

  test('drainRuntimeMetricsForFlush (300 pts) < 1000 µs', () => {
    __resetRuntimeMetricsForTests()
    for (let i = 0; i < 300; i++) emitMetric('runtime.fps.p50', 60)
    const perOp = timed('drainRuntimeMetricsForFlush.300', 1_000, () => {
      drainRuntimeMetricsForFlush()
      // Refill so each iteration measures the same shape.
      for (let i = 0; i < 300; i++) emitMetric('runtime.fps.p50', 60)
    })
    drainRuntimeMetricsForFlush()
    expect(perOp).toBeLessThan(1000)
  })

  // v2.3 W6.0 — logger hot path budgets. The SDK calls logger.* on
  // every fetched response, every replay tick, every breadcrumb add
  // etc.; a gated-out log (level filter dropping it) must be
  // essentially free, and an emitted log must not be the bottleneck.
  // These run against `bun:test`'s console capture — same process,
  // no real terminal IO involved.
  test('logger.debug gated-out at level=warn < 1 µs/op', () => {
    // Default level is 'warn' so 'debug' calls fall the cheap path
    // (single ORDER lookup + comparison + early return). Verify we
    // really did get gated out.
    setLogLevel('warn')
    let emitted = 0
    const cap: LogTransport = () => {
      emitted += 1
    }
    setLogTransport(cap)
    const perOp = timed('logger.debug gated', 100_000, () => {
      logger.debug('replay', 'tick ok')
    })
    setLogTransport(null)
    expect(emitted).toBe(0)
    expect(perOp).toBeLessThan(1)
  })

  test('logger.warn emit (transport path) < 5 µs/op', () => {
    setLogLevel('warn')
    let emitted = 0
    const cap: LogTransport = () => {
      emitted += 1
    }
    setLogTransport(cap)
    const perOp = timed('logger.warn emit', 20_000, () => {
      logger.warn('transport', 'retry')
    })
    setLogTransport(null)
    // `timed()` runs a 1k warm-up before the measured loop, so the
    // total emission count is loops + min(loops, 1000) = 21_000.
    expect(emitted).toBe(21_000)
    expect(perOp).toBeLessThan(5)
  })

  test('setLogLevel toggle < 1 µs/op', () => {
    const perOp = timed('setLogLevel', 100_000, () => {
      setLogLevel('warn')
    })
    expect(perOp).toBeLessThan(1)
  })
})
