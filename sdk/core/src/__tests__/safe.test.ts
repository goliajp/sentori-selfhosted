import { afterAll, afterEach, beforeAll, describe, expect, mock, test } from 'bun:test'

import { setLogLevel } from '../logger.js'
import { safeAsync, safeFn } from '../safe.js'
import {
  __resetCircuitForTests,
  isCircuitOpen,
  reportInternal,
  setInternalReporter,
} from '../self-report.js'

// v2.3 — `reportInternal` now routes through the logger at
// `error` level (default `'warn'` surfaces it). Tests intentionally
// fire `reportInternal` to verify behaviour; the resulting
// console.error noise makes bun test exit non-zero. Silence the
// logger for the duration of this suite — we're exercising the
// reporter, not the output.
beforeAll(() => {
  setLogLevel('silent')
})
afterAll(() => {
  setLogLevel('warn')
})

afterEach(() => {
  setInternalReporter(null)
  __resetCircuitForTests()
})

describe('safeFn — NEVER rule', () => {
  test('returns the wrapped fn result on the happy path', () => {
    const add = safeFn('add', (a: number, b: number) => a + b)
    expect(add(2, 3)).toBe(5)
  })

  test('swallows thrown errors and returns undefined', () => {
    const boom = safeFn('boom', (_: number) => {
      throw new Error('internal failure')
    })
    expect(() => boom(1)).not.toThrow()
    expect(boom(1)).toBeUndefined()
  })

  test('reports the failure via the internal reporter (with details)', () => {
    const reports: { api: string; message: string }[] = []
    setInternalReporter((r) => {
      reports.push({ api: r.api, message: r.message })
    })

    const boom = safeFn('captureMessage', () => {
      throw new Error('oh no')
    })
    boom()
    expect(reports).toHaveLength(1)
    expect(reports[0].api).toBe('captureMessage')
    expect(reports[0].message).toBe('oh no')
  })

  test('non-Error throws still report (string / object payload)', () => {
    const reports: string[] = []
    setInternalReporter((r) => {
      reports.push(r.message)
    })

    safeFn('s', () => {
      throw 'a string'
    })()
    safeFn('o', () => {
      throw { code: 'X' }
    })()

    expect(reports).toHaveLength(2)
    expect(reports[0]).toBe('a string')
    expect(reports[1]).toContain('[object Object]')
  })

  test('a thrown reporter itself does not propagate', () => {
    setInternalReporter(() => {
      throw new Error('reporter exploded')
    })
    const boom = safeFn('boom', () => {
      throw new Error('original')
    })
    expect(() => boom()).not.toThrow()
  })
})

describe('safeAsync — NEVER rule', () => {
  test('returns the wrapped result on the happy path', async () => {
    const fn = safeAsync('ok', async () => 42)
    await expect(fn()).resolves.toBe(42)
  })

  test('swallows rejected promises and resolves to undefined', async () => {
    const fn = safeAsync('reject', async () => {
      throw new Error('async failure')
    })
    await expect(fn()).resolves.toBeUndefined()
  })

  test('swallows synchronously thrown errors inside an async wrapper', async () => {
    const fn = safeAsync('sync-throw', async () => {
      throw new Error('sync inside async')
    })
    await expect(fn()).resolves.toBeUndefined()
  })

  test('reports the failure via the internal reporter', async () => {
    const reports: string[] = []
    setInternalReporter((r) => {
      reports.push(r.api)
    })
    const fn = safeAsync('flush', async () => {
      throw new Error('boom')
    })
    await fn()
    expect(reports).toEqual(['flush'])
  })
})

describe('reportInternal — circuit breaker', () => {
  test('first call opens nothing; reports flow through', () => {
    const reports: string[] = []
    setInternalReporter((r) => reports.push(r.api))
    reportInternal('api1', new Error('1'))
    expect(reports).toEqual(['api1'])
    expect(isCircuitOpen()).toBe(false)
  })

  test('stops reporting after 10 failures in a rolling minute', () => {
    const reports: string[] = []
    setInternalReporter((r) => reports.push(r.api))

    for (let i = 0; i < 15; i++) {
      reportInternal(`api${i}`, new Error(`${i}`))
    }
    expect(reports).toHaveLength(10)
    expect(isCircuitOpen()).toBe(true)
  })

  test('respects the absence of a configured reporter (silent)', () => {
    // No setInternalReporter — the call should not throw.
    expect(() => reportInternal('no-reporter', new Error('x'))).not.toThrow()
  })

  test('survives a reporter that throws', () => {
    setInternalReporter(() => {
      throw new Error('reporter exploded')
    })
    expect(() => reportInternal('api', new Error('y'))).not.toThrow()
  })
})
