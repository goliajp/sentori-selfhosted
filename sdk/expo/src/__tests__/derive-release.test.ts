import { describe, expect, test } from 'bun:test'

import { deriveRelease } from '../release.js'

describe('deriveRelease', () => {
  test('builds slug@version+build from expo-application fields', () => {
    expect(
      deriveRelease({
        applicationId: 'com.example.myapp',
        nativeApplicationVersion: '1.2.3',
        nativeBuildVersion: '42',
      }),
    ).toBe('com.example.myapp@1.2.3+42')
  })

  test('substitutes defaults for missing fields', () => {
    expect(
      deriveRelease({
        applicationId: 'com.example.myapp',
      }),
    ).toBe('com.example.myapp@0.0.0+0')
  })

  test('returns undefined when module is missing', () => {
    expect(deriveRelease(undefined)).toBeUndefined()
  })

  test('handles null fields gracefully (Expo bare workflow)', () => {
    expect(
      deriveRelease({
        applicationId: null,
        nativeApplicationVersion: null,
        nativeBuildVersion: null,
      }),
    ).toBe('app@0.0.0+0')
  })
})
