import { beforeEach, describe, expect, it } from 'vitest'

import {
  __resetPlatformDegradeForTests,
  degradePlatform,
  PLATFORM_FALLBACK,
  platformOrFallback,
  refusalIsAboutPlatform,
} from '../platform-degrade'

describe('refusalIsAboutPlatform', () => {
  it('reads the message a released server actually sends', () => {
    expect(
      refusalIsAboutPlatform([
        { eventId: 'x' } as never,
        { error: 'invalid_payload', detail: 'platform must be javascript|ios|android' },
      ]),
    ).toBe(true)
  })

  it('leaves an unrelated refusal alone', () => {
    // Degrading here would throw away the runtime's real identity for
    // a batch that was refused for something else entirely.
    expect(
      refusalIsAboutPlatform([{ error: 'invalid_payload', detail: 'kind must be one of five' }]),
    ).toBe(false)
    expect(refusalIsAboutPlatform([{ error: 'ingest_failed' }])).toBe(false)
  })

  it('is false for a batch with nothing refused, and for no body at all', () => {
    expect(refusalIsAboutPlatform([])).toBe(false)
    expect(refusalIsAboutPlatform(undefined)).toBe(false)
  })

  it('falls back to a value every shipped server accepts', () => {
    // The three the first CHECK constraint was written with; a value
    // added later would be refused by exactly the servers this path
    // exists for.
    expect(['javascript', 'ios', 'android']).toContain(PLATFORM_FALLBACK)
  })
})

describe('degradePlatform', () => {
  beforeEach(() => {
    __resetPlatformDegradeForTests()
  })

  it('swaps the wire value and warns exactly once', () => {
    const warnings: string[] = []
    const warn = (m: string) => warnings.push(m)
    expect(platformOrFallback('weapp')).toBe('weapp')

    degradePlatform(warn)
    expect(platformOrFallback('weapp')).toBe(PLATFORM_FALLBACK)
    expect(platformOrFallback('web')).toBe(PLATFORM_FALLBACK)

    degradePlatform(warn)
    degradePlatform(warn)
    expect(warnings).toHaveLength(1)
    expect(warnings[0]).toContain(PLATFORM_FALLBACK)
  })

  it('does not touch a platform the server already accepts', () => {
    expect(platformOrFallback('ios')).toBe('ios')
  })
})
