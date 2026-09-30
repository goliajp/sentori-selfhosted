// SDK hot-path perf budget (the zero-cost iron rule, dimension 1).
// Run by sdk-perf.yml on every core/RN change. Budgets are per-op
// microseconds on CI hardware; a regression here is a regression in
// every host app's main thread.

import { describe, expect, test } from 'bun:test'

import { coerceError } from '../coerce-error.js'
import { shouldSample, shouldSampleTrace } from '../sampling.js'
import { clearSignals, pushSignal, snapshotSignals } from '../signal-ring.js'
import { uuidV7 } from '../uuid.js'

function timed(label: string, loops: number, fn: () => void): number {
  // Warm-up — eject any first-call JIT cost from the measurement.
  for (let i = 0; i < Math.min(loops, 1000); i++) fn()
  const start = performance.now()
  for (let i = 0; i < loops; i++) fn()
  const total = performance.now() - start
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
    const id = uuidV7()
    const perOp = timed('shouldSampleTrace', 100_000, () => {
      shouldSampleTrace(id, 0.5)
    })
    expect(perOp).toBeLessThan(5)
  })

  test('pushSignal < 1 µs/op — every tap/nav/http pays this', () => {
    clearSignals()
    const perOp = timed('pushSignal', 100_000, () => {
      pushSignal('tap', { target: 'PayButton' })
    })
    expect(perOp).toBeLessThan(1)
  })

  test('snapshotSignals < 50 µs/op — paid once per error/warn', () => {
    clearSignals()
    for (let i = 0; i < 100; i++) pushSignal('nav', { i })
    const perOp = timed('snapshotSignals', 5_000, () => {
      snapshotSignals()
    })
    expect(perOp).toBeLessThan(50)
  })

  test('coerceError(Error) < 10 µs/op — the error verb hot path', () => {
    const err = new Error('boom')
    const perOp = timed('coerceError', 20_000, () => {
      coerceError(err)
    })
    expect(perOp).toBeLessThan(10)
  })
})
