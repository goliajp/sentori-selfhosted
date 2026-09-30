// Assembling one event, away from any runtime.
//
// This is the shape the server parses. React Native built it inline
// for two years, which was fine while there was one SDK; web and
// mini-programs make it three, and three assemblies of the same
// envelope are three chances to drift.

import { describe, expect, it } from 'bun:test'

import { applyBeforeSend, buildWireEvent, serializeData } from '../wire-event.js'
import type { WireEvent } from '../types.js'

const base = {
  id: 'evt-1',
  kind: 'error' as const,
  platform: 'web' as const,
  release: 'app@1.0.0+1',
  environment: 'test',
  occurredAt: '2026-09-30T00:00:00.000Z',
}

describe('buildWireEvent', () => {
  it('leaves absent parts out rather than sending empty ones', () => {
    const e = buildWireEvent(base)
    expect(e.payload).toEqual({})
    // An empty object and an absent key read the same to a person and
    // differently to a fingerprint.
    expect('signals' in e.payload).toBe(false)
    expect('device' in e.payload).toBe(false)
  })

  it('drops an empty signal ring instead of sending []', () => {
    expect(buildWireEvent({ ...base, signals: [] }).payload.signals).toBeUndefined()
  })

  it('serializes an Error sitting in data', () => {
    const e = buildWireEvent({ ...base, data: { cause: new TypeError('nope'), id: 7 } })
    const cause = e.payload.data?.cause as { type: string; message: string }
    expect(cause.type).toBe('TypeError')
    expect(cause.message).toBe('nope')
    expect(e.payload.data?.id).toBe(7)
  })

  it('follows an Error chain one level down', () => {
    const inner = new Error('inner')
    const outer = new Error('outer', { cause: inner })
    const out = serializeData({ e: outer })?.e as { cause: { message: string } }
    expect(out.cause.message).toBe('inner')
  })
})

describe('applyBeforeSend', () => {
  it('passes the event through when there is no hook', () => {
    const e = buildWireEvent(base)
    expect(applyBeforeSend(e, undefined)).toBe(e)
  })

  it('treats null as a deliberate drop', () => {
    expect(applyBeforeSend(buildWireEvent(base), () => null)).toBeNull()
  })

  it('takes the replacement the hook returns', () => {
    const replaced = applyBeforeSend(buildWireEvent(base), (e) => ({ ...e, release: 'edited' }))
    expect(replaced?.release).toBe('edited')
  })

  it('ships the original when the hook throws', () => {
    // The host's bug must not cost them the crash report.
    const e = buildWireEvent(base)
    const out = applyBeforeSend(e, () => {
      throw new Error('host hook is broken')
    })
    expect(out).toBe(e)
  })

  it('ignores a hook that returns something that is not an event', () => {
    const e = buildWireEvent(base)
    expect(applyBeforeSend(e, (() => 'nonsense') as unknown as (x: WireEvent) => WireEvent)).toBe(e)
  })
})
