import { afterEach, describe, expect, test } from 'bun:test'

import { clearSpans, drainSpans } from '../spans.js'
import { startMoment } from '../moments.js'

afterEach(() => {
  clearSpans()
})

describe('startMoment', () => {
  test('emits a span on .end() with status=ok and moment.name tag', () => {
    const m = startMoment('checkout', { properties: { cartValue: 42 } })
    m.end()
    const [s] = drainSpans()
    expect(s).toBeDefined()
    expect(s!.op).toBe('sentori.moment')
    expect(s!.name).toBe('checkout')
    expect(s!.status).toBe('ok')
    expect(s!.tags['moment.name']).toBe('checkout')
    expect(s!.tags['moment.prop.cartValue']).toBe('42')
  })

  test('.fail() marks error status + carries reason', () => {
    const m = startMoment('checkout')
    m.fail('declined')
    const [s] = drainSpans()
    expect(s!.status).toBe('error')
    expect(s!.tags['moment.fail.reason']).toBe('declined')
  })

  test('.abandon() marks cancelled + abandoned tag', () => {
    const m = startMoment('checkout')
    m.abandon()
    const [s] = drainSpans()
    expect(s!.status).toBe('cancelled')
    expect(s!.tags['moment.abandoned']).toBe('true')
  })

  test('checkpoints ride along as span data', () => {
    const m = startMoment('checkout')
    m.checkpoint('cart-loaded')
    m.checkpoint('payment-submitted')
    m.end()
    const [s] = drainSpans()
    const cp = s!.data?.['moment.checkpoints'] as { label: string }[]
    expect(cp).toBeDefined()
    expect(cp.map((c) => c.label)).toEqual(['cart-loaded', 'payment-submitted'])
  })

  test('double-finalize is a no-op', () => {
    const m = startMoment('x')
    m.end()
    m.abandon() // ignored
    m.fail() // ignored
    const spans = drainSpans()
    expect(spans.length).toBe(1)
    expect(spans[0]!.status).toBe('ok')
  })
})
