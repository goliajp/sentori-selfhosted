// Build-time iron rule: upload failure exits 0 with a friendly,
// actionable note; --strict opts into a real non-zero.

import { describe, expect, test } from 'bun:test'

import { isStrict, lenientFail, stripStrict } from '../lenient.js'

describe('lenient upload contract', () => {
  test('default: failure exits 0', () => {
    const code = lenientFail(false, {
      failure: 'network down',
      impact: 'stacks stay minified',
      retry: 'sentori-cli upload sourcemap ...',
    })
    expect(code).toBe(0)
  })

  test('--strict: failure exits 1', () => {
    const code = lenientFail(true, {
      failure: 'network down',
      impact: 'stacks stay minified',
      retry: 'retry-cmd',
    })
    expect(code).toBe(1)
  })

  test('strict flag parses and strips', () => {
    expect(isStrict(['--release', 'r', '--strict'])).toBe(true)
    expect(isStrict(['--release', 'r'])).toBe(false)
    expect(stripStrict(['--strict', 'a'])).toEqual(['a'])
  })
})
