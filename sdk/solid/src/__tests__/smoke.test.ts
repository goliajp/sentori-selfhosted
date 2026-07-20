import { describe, expect, test } from 'bun:test'

import {
  addBreadcrumb,
  captureException,
  initSentori,
  sentoriOnCatch,
  traceSolidRouter,
} from '../index.js'

describe('@goliapkg/sentori-solid exports', () => {
  test('exports the SDK init + capture surface', () => {
    expect(typeof initSentori).toBe('function')
    expect(typeof captureException).toBe('function')
    expect(typeof addBreadcrumb).toBe('function')
  })

  test('sentoriOnCatch normalises non-Error throws', () => {
    expect(() => {
      sentoriOnCatch(new Error('explicit'))
      sentoriOnCatch('strung')
      sentoriOnCatch({ unexpected: 'shape' })
    }).not.toThrow()
  })

  test('traceSolidRouter is idempotent across same pathname', () => {
    expect(() => {
      traceSolidRouter('/a')
      traceSolidRouter('/a')
      traceSolidRouter('/b')
    }).not.toThrow()
  })
})
