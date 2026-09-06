// The server has answered `usable: false` since 2.15.0, and its own
// comment in `artifacts_upload.rs` says "the response says so". It
// did. Nothing read it — both upload paths threw the body away and
// returned `{ id }` — so a Hermes bundle uploaded as a source map
// stored, printed nothing, and sat under a green light on the release
// page until somebody expanded the row months later.
//
// These pin the two halves that were missing: it warns, and it does
// not throw.

import { describe, expect, mock, test } from 'bun:test'

import { warnIfUnusable } from '../upload.js'

/** Capture what the CLI prints, without printing it. */
function captured(fn: () => void): { warn: string[] } {
  const warn: string[] = []
  const orig = console.warn
  console.warn = mock((...a: unknown[]) => void warn.push(a.join(' ')))
  try {
    fn()
  } finally {
    console.warn = orig
  }
  return { warn }
}

describe('what the server said about the file it stored', () => {
  test('an unreadable artifact is named, with what to upload instead', () => {
    const { warn } = captured(() => {
      const said = warnIfUnusable('index.android.bundle', {
        id: 'a',
        usable: false,
        hint: 'React Native: upload the generated .map, not the bundle.',
      })
      expect(said).toBe(true)
    })
    expect(warn).toHaveLength(1)
    // The file, so somebody with eight artifacts knows which one.
    expect(warn[0]).toContain('index.android.bundle')
    // And the server's own instruction, rather than our paraphrase.
    expect(warn[0]).toContain('upload the generated .map')
  })

  test('it warns rather than throwing — an upload may not fail a build', () => {
    // The zero-cost rule: sentori cannot break a customer's release.
    // The file did store; only its usefulness is in question.
    expect(() =>
      captured(() => warnIfUnusable('main.jsbundle', { id: 'a', usable: false })),
    ).not.toThrow()
  })

  test('a readable artifact says nothing', () => {
    const { warn } = captured(() => {
      expect(warnIfUnusable('app.map', { id: 'a', usable: true })).toBe(false)
    })
    expect(warn).toHaveLength(0)
  })

  test('"we did not look" is not "we looked and it is broken"', () => {
    // `srcbundle` and anything uploaded before the check existed come
    // back `null`. Warning on those would train people to ignore the
    // warning that matters.
    for (const usable of [null, undefined]) {
      const { warn } = captured(() => {
        expect(warnIfUnusable('srcbundle.json', { id: 'a', usable })).toBe(false)
      })
      expect(warn).toHaveLength(0)
    }
  })

  test('a hint-less refusal still names the file', () => {
    const { warn } = captured(() => warnIfUnusable('Insight.app-arm64', { id: 'a', usable: false }))
    expect(warn[0]).toContain('Insight.app-arm64')
    expect(warn[0]).toContain('cannot read')
  })
})
