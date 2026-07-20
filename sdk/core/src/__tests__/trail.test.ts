import { describe, expect, test } from 'bun:test'

import { TrailBuffer, sealTrail } from '../trail.js'

describe('TrailBuffer', () => {
  test('respects maxSteps and evicts oldest first', () => {
    const buf = new TrailBuffer(3)
    for (let i = 0; i < 5; i++) buf.push({ ts: i, label: `step-${i}` })
    expect(buf.size()).toBe(3)
    const snap = buf.snapshot()
    expect(snap.map((s) => s.label)).toEqual(['step-2', 'step-3', 'step-4'])
  })

  test('default maxSteps is 30', () => {
    const buf = new TrailBuffer()
    for (let i = 0; i < 35; i++) buf.push({ ts: i, label: String(i) })
    expect(buf.size()).toBe(30)
    expect(buf.snapshot()[0]!.label).toBe('5')
  })

  test('snapshot returns a copy', () => {
    const buf = new TrailBuffer(5)
    buf.push({ ts: 1, label: 'a' })
    const snap = buf.snapshot()
    snap.push({ ts: 2, label: 'mutation' })
    expect(buf.size()).toBe(1)
  })

  test('clear empties the buffer', () => {
    const buf = new TrailBuffer(5)
    buf.push({ ts: 1, label: 'a' })
    buf.push({ ts: 2, label: 'b' })
    buf.clear()
    expect(buf.size()).toBe(0)
  })

  test('sealTrail produces an ISO 8601 sealedAt + steps copy', () => {
    const buf = new TrailBuffer(5)
    buf.push({ ts: 1, label: 'a', screenshotRef: 'attach-uuid' })
    const payload = sealTrail(buf)
    expect(payload.sealedAt).toMatch(/^\d{4}-\d{2}-\d{2}T/)
    expect(payload.steps).toEqual([{ ts: 1, label: 'a', screenshotRef: 'attach-uuid' }])
  })

  test('maxSteps below 1 is clamped to 1', () => {
    const buf = new TrailBuffer(0)
    buf.push({ ts: 1, label: 'a' })
    buf.push({ ts: 2, label: 'b' })
    expect(buf.size()).toBe(1)
    expect(buf.snapshot()[0]!.label).toBe('b')
  })
})
