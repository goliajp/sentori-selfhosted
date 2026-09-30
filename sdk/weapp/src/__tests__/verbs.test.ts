// The five kinds, and the platform's own limits.

import { afterEach, beforeEach, describe, expect, it } from 'bun:test'

import { __resetForTests as resetConfig, setConfig } from '../config.js'
import { __resetDeviceForTests } from '../device.js'
import { __resetForTests as resetScope } from '../scope.js'
import {
  __peekAssertStats,
  __peekQueue,
  __resetForTests as resetTransport,
  byteLength,
  startTransport,
} from '../transport.js'
import { verbs } from '../verbs.js'
import { setWxHost } from '../wx.js'

const config = {
  token: 'st_test',
  ingestUrl: 'https://sentori.example.cn',
  release: 'weapp@1.0.0',
  environment: 'test',
  enabled: true,
  detect: { uncaught: true, platform: true, breadcrumbs: true },
}

describe('verbs', () => {
  beforeEach(() => {
    resetTransport()
    resetScope()
    resetConfig()
    __resetDeviceForTests()
    setWxHost(null)
    setConfig(config)
    startTransport()
  })
  afterEach(() => {
    resetTransport()
    resetConfig()
    setWxHost(null)
  })

  it('sends platform `weapp`, which the server has a column value for', () => {
    verbs.error(new Error('boom'))
    expect(__peekQueue()[0]?.platform).toBe('weapp')
  })

  it('aggregates a passing assert instead of sending it', () => {
    verbs.assert('total.positive', true)
    expect(__peekQueue()).toHaveLength(0)
    expect(__peekAssertStats()[0]?.passDelta).toBe(1)
  })

  it('is a no-op before init, and still returns an id', () => {
    resetConfig()
    const id = verbs.error(new Error('before init'))
    expect(typeof id).toBe('string')
    expect(__peekQueue()).toHaveLength(0)
  })

  it('never throws, whatever it is handed', () => {
    const cyclic: Record<string, unknown> = {}
    cyclic.self = cyclic
    expect(() => verbs.error(cyclic)).not.toThrow()
    expect(() => verbs.warn('x', { get boom(): string { throw new Error('nope') } })).not.toThrow()
    expect(() => verbs.probe(null as unknown as string)).not.toThrow()
  })

  it('works with no `wx` at all', () => {
    // The devtools preview and a unit runner both have none. An SDK
    // that needed the global to exist would throw at import.
    setWxHost(null)
    expect(() => verbs.trace('tick')).not.toThrow()
  })
})

describe('byteLength', () => {
  // The storage limit is on bytes. Counting characters would let a
  // queue of Chinese error messages through at three times the size,
  // and `wx.setStorageSync` throws rather than truncating.
  it('counts UTF-8 bytes, not characters', () => {
    for (const s of ['', 'abc', '订单支付失败', 'Ünïcödé', '🙂', 'a🙂b订单']) {
      expect(byteLength(s)).toBe(new TextEncoder().encode(s).length)
    }
  })
})
