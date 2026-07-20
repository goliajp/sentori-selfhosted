import { describe, expect, test } from 'bun:test'

import {
  addBreadcrumb,
  captureException,
  initSentori,
  sentoriHandleError,
  traceNavigation,
} from '../index.js'

describe('@goliapkg/sentori-svelte exports', () => {
  test('exports the SDK init + capture surface', () => {
    expect(typeof initSentori).toBe('function')
    expect(typeof captureException).toBe('function')
    expect(typeof addBreadcrumb).toBe('function')
  })

  test('sentoriHandleError returns a SvelteKit-shaped handler', () => {
    const handler = sentoriHandleError()
    const result = handler({ error: new Error('boom'), message: 'custom' })
    expect(result).toEqual({ message: 'custom' })

    const result2 = handler({ error: 'string-thrown' })
    expect(result2.message).toBe('string-thrown')
  })

  test('traceNavigation accepts null + nav-object shape without throwing', () => {
    expect(() => {
      traceNavigation(null)
      traceNavigation({ from: { url: { pathname: '/a' } }, to: { url: { pathname: '/b' } } })
      traceNavigation(null)
    }).not.toThrow()
  })
})
