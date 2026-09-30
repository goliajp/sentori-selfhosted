// The five kinds, and the iron rule around them.

import { afterEach, beforeEach, describe, expect, it } from 'bun:test'

import { __resetForTests as resetConfig, setConfig } from '../config.js'
import { __resetForTests as resetScope } from '../scope.js'
import {
  __peekAssertStats,
  __peekQueue,
  __resetForTests as resetTransport,
  startTransport,
} from '../transport.js'
import { verbs } from '../verbs.js'

const config = {
  token: 'st_test',
  ingestUrl: 'http://127.0.0.1:18080',
  release: 'site@1.0.0',
  environment: 'test',
  enabled: true,
  detect: { uncaught: true, webVitals: true, breadcrumbs: true },
  replaySeconds: 30,
  replayScreens: false,
}

describe('verbs', () => {
  beforeEach(() => {
    resetTransport()
    resetScope()
    resetConfig()
    setConfig(config)
    startTransport()
  })
  afterEach(() => {
    resetTransport()
    resetConfig()
  })

  it('sends platform `web`, which the server has a column value for', () => {
    verbs.error(new Error('boom'))
    expect(__peekQueue()[0]?.platform).toBe('web')
  })

  it('carries the signal ring on an error and not on a trace', () => {
    verbs.trace('cart.opened')
    verbs.error(new Error('boom'))
    const [trace, error] = __peekQueue()
    expect('signals' in (trace?.payload ?? {})).toBe(false)
    // The trace above put an entry in the ring, so the error has one.
    expect((error?.payload.signals ?? []).length).toBeGreaterThan(0)
  })

  it('aggregates a passing assert instead of sending it', () => {
    verbs.assert('total.positive', true)
    expect(__peekQueue()).toHaveLength(0)
    expect(__peekAssertStats()[0]?.passDelta).toBe(1)
  })

  it('sends a failing assert as an event', () => {
    verbs.assert('total.positive', false)
    expect(__peekQueue()[0]?.kind).toBe('assert')
  })

  it('returns an id and enqueues nothing before init', () => {
    resetConfig()
    const id = verbs.error(new Error('before init'))
    expect(typeof id).toBe('string')
    expect(id.length).toBeGreaterThan(0)
    expect(__peekQueue()).toHaveLength(0)
  })

  it('never throws, whatever it is handed', () => {
    const cyclic: Record<string, unknown> = {}
    cyclic.self = cyclic
    expect(() => verbs.error(cyclic)).not.toThrow()
    expect(() => verbs.warn('x', { get boom(): string { throw new Error('nope') } })).not.toThrow()
    expect(() => verbs.trace(undefined as unknown as string)).not.toThrow()
    expect(() => verbs.probe(null as unknown as string)).not.toThrow()
  })

  it('lets beforeSend drop an event', () => {
    setConfig({ ...config, beforeSend: () => null })
    verbs.error(new Error('dropped'))
    expect(__peekQueue()).toHaveLength(0)
  })
})
