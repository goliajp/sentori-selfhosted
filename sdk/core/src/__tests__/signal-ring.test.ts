import { beforeEach, describe, expect, test } from 'bun:test'

import { clearSignals, configureRing, pushSignal, snapshotSignals } from '../signal-ring.js'

describe('signal ring', () => {
  beforeEach(() => {
    clearSignals()
    configureRing({ capacity: 100, windowMs: 30_000 })
  })

  test('snapshot is oldest-first with negative relative seconds', () => {
    const now = Date.now()
    pushSignal('nav', { to: '/checkout' })
    pushSignal('tap', { target: 'PayButton' })
    const out = snapshotSignals(now + 1000)
    expect(out.length).toBe(2)
    expect(out[0]?.kind).toBe('nav')
    expect(out[1]?.kind).toBe('tap')
    for (const s of out) expect(s.t).toBeLessThanOrEqual(0)
  })

  test('capacity bounds the ring — oldest drops', () => {
    configureRing({ capacity: 5 })
    for (let i = 0; i < 20; i++) pushSignal('tap', { i })
    const out = snapshotSignals()
    expect(out.length).toBe(5)
    expect(out[0]?.data?.i).toBe(15)
  })

  test('window drops stale entries from the snapshot', () => {
    configureRing({ windowMs: 1000 })
    pushSignal('old')
    const out = snapshotSignals(Date.now() + 5000)
    expect(out.length).toBe(0)
  })
})
