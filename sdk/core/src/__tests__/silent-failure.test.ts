import { afterEach, beforeEach, describe, expect, spyOn, test } from 'bun:test'
import { getLogLevel, logger, setLogLevel, setLogTransport } from '../logger.js'
import { safeAsync, safeFn } from '../safe.js'
import { __resetCircuitForTests, reportInternal, setInternalReporter } from '../self-report.js'
import { applyBeforeSend } from '../wire-event.js'
import type { WireEvent } from '../types.js'

describe('SDK faults are silent at the host boundary', () => {
  let output: ReturnType<typeof spyOn>[]
  beforeEach(() => {
    setLogLevel(undefined)
    setLogTransport(null)
    setInternalReporter(null)
    __resetCircuitForTests()
    output = ['log', 'warn', 'error', 'info', 'debug'].map(method =>
      spyOn(console, method as 'log').mockImplementation(() => {}))
  })
  afterEach(() => {
    for (const spy of output) spy.mockRestore()
    setLogLevel(undefined)
    setLogTransport(null)
    setInternalReporter(null)
    __resetCircuitForTests()
  })

  test('default silence is checked with real triggered failures and reporter evidence', async () => {
    const reports: string[] = []
    setInternalReporter(event => { reports.push(event.api) })
    const sync = safeFn('sync-fault', () => { throw new Error('injected') })
    const asyncFault = safeAsync('async-fault', async () => { throw new Error('injected') })
    expect(sync()).toBeUndefined()
    await expect(asyncFault()).resolves.toBeUndefined()
    logger.warn('transport', 'server unavailable')
    expect(getLogLevel()).toBe('silent')
    expect(reports).toEqual(['sync-fault', 'async-fault'])
    for (const spy of output) expect(spy).not.toHaveBeenCalled()
  })

  test('an accidentally async reporter cannot create an unhandled rejection', async () => {
    let called = 0
    setInternalReporter(async () => { called++; throw new Error('reporter reject') })
    reportInternal('injected', new Error('fault'))
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(called).toBe(1)
    for (const spy of output) expect(spy).not.toHaveBeenCalled()
  })

  test('an accidentally async beforeSend is ignored and its rejection is consumed', async () => {
    let called = 0
    const event = { id: 'original' } as WireEvent
    const asyncHook = async () => { called++; throw new Error('hook rejected') }
    expect(applyBeforeSend(event, asyncHook as unknown as (e: WireEvent) => WireEvent)).toBe(event)
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(called).toBe(1)
    for (const spy of output) expect(spy).not.toHaveBeenCalled()
  })

  test('explicit diagnostics stay bounded without touching object getters', () => {
    setLogLevel('debug')
    const lines: unknown[][] = []
    setLogTransport((_level, _tag, args) => { lines.push(args) })
    const hostile = { get message() { throw new Error('must not evaluate') } }
    for (let n = 0; n < 1000; n++) logger.error('test', 'x'.repeat(4096), hostile)
    expect(lines).toHaveLength(20)
    expect(lines.every(line => (line[0] as string).length <= 2048)).toBe(true)
    expect(lines[0]?.[1]).toBe('[object omitted]')
    for (const spy of output) expect(spy).not.toHaveBeenCalled()
  })

  test('a failing diagnostics transport gets no surprise console fallback', async () => {
    setLogLevel('debug')
    let calls = 0
    setLogTransport(async () => { calls++; throw new Error('diagnostics rejected') })
    logger.error('test', 'first')
    await new Promise(resolve => setTimeout(resolve, 0))
    logger.error('test', 'second')
    expect(calls).toBe(1)
    for (const spy of output) expect(spy).not.toHaveBeenCalled()
  })

  test('even an opted-in but broken host console cannot throw out of the logger', () => {
    setLogLevel('warn')
    output[1]!.mockImplementation(() => { throw new Error('console unavailable') })
    expect(() => logger.warn('test', 'fault')).not.toThrow()
  })
})
