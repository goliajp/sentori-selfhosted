import { afterEach, beforeEach, describe, expect, test } from 'bun:test'

import { shouldSample, shouldSampleTrace } from '../sampling.js'

describe('shouldSample (uniform)', () => {
  const origRandom = Math.random
  afterEach(() => {
    Math.random = origRandom
  })

  test('rate >= 1 always keeps', () => {
    Math.random = () => 0.9999
    expect(shouldSample(1)).toBe(true)
    expect(shouldSample(2)).toBe(true) // clamped to 1
  })

  test('rate <= 0 always rejects', () => {
    Math.random = () => 0
    expect(shouldSample(0)).toBe(false)
    expect(shouldSample(-1)).toBe(false) // clamped to 0
  })

  test('null / undefined / NaN → 1.0 (keep)', () => {
    expect(shouldSample(null)).toBe(true)
    expect(shouldSample(undefined)).toBe(true)
    expect(shouldSample(NaN)).toBe(true)
  })

  test('rate = 0.5 + Math.random = 0.4 → keep', () => {
    Math.random = () => 0.4
    expect(shouldSample(0.5)).toBe(true)
  })

  test('rate = 0.5 + Math.random = 0.6 → reject', () => {
    Math.random = () => 0.6
    expect(shouldSample(0.5)).toBe(false)
  })
})

describe('shouldSampleTrace (deterministic over traceId)', () => {
  test('same traceId always yields the same decision', () => {
    const tid = '019eaa00-7000-7000-8000-000000000001'
    const decisions = new Set<boolean>()
    for (let i = 0; i < 100; i++) {
      decisions.add(shouldSampleTrace(tid, 0.5))
    }
    expect(decisions.size).toBe(1)
  })

  test('rate >= 1 always keeps + rate <= 0 always rejects', () => {
    const tid = 'abc-123'
    expect(shouldSampleTrace(tid, 1)).toBe(true)
    expect(shouldSampleTrace(tid, 0)).toBe(false)
  })

  test('low first-32-bits trace id keeps when rate is high', () => {
    // first 8 hex = "00000001" → u32 = 1 → normalised ≈ 0
    // any positive rate above 0 → keep
    expect(shouldSampleTrace('00000001-0000-0000-0000-000000000000', 0.001)).toBe(true)
  })

  test('high first-32-bits trace id rejects when rate is low', () => {
    // first 8 hex = "ffffffff" → u32 = 4_294_967_295 → normalised ≈ 0.999…
    expect(shouldSampleTrace('ffffffff-0000-0000-0000-000000000000', 0.5)).toBe(false)
  })

  test('null traceId falls back to uniform random', () => {
    // Just confirm it doesn't throw + returns a boolean.
    expect(typeof shouldSampleTrace(null, 0.5)).toBe('boolean')
  })

  test('short / malformed traceId falls back to uniform random', () => {
    expect(typeof shouldSampleTrace('abc', 0.5)).toBe('boolean')
  })

  test('UUIDs with dashes hash correctly', () => {
    // first 8 hex of "019eaa00-7000-7000-8000-000000000001" = "019eaa00"
    // → 27176960 → normalised ≈ 0.00633 → all rates ≥ 0.01 keep
    const tid = '019eaa00-7000-7000-8000-000000000001'
    expect(shouldSampleTrace(tid, 0.01)).toBe(true)
    expect(shouldSampleTrace(tid, 0.001)).toBe(false)
  })

  test('rate=0.5 over 10000 random uuids hits ~50% within 5pp', () => {
    let kept = 0
    const N = 10_000
    for (let i = 0; i < N; i++) {
      // Build a UUID-like string with random hex first 8 chars.
      const u32 = Math.floor(Math.random() * 0x100000000)
      const hex = u32.toString(16).padStart(8, '0')
      const tid = `${hex}-0000-0000-0000-000000000000`
      if (shouldSampleTrace(tid, 0.5)) kept++
    }
    const ratio = kept / N
    expect(ratio).toBeGreaterThan(0.45)
    expect(ratio).toBeLessThan(0.55)
  })
})
