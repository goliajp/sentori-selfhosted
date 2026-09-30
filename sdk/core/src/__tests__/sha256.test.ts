// The fallback has to agree with the real thing.
//
// A hash that is merely self-consistent is worse than no hash: the
// same person would be two people, one on the web and one in a mini
// program, and the only symptom is a breadth number that is quietly
// too high. So the test is not a table of digests I wrote down — it
// is the platform's own `crypto.subtle` answering the same inputs.

import { describe, expect, it } from 'bun:test'

import { sha256Hex, utf8Bytes } from '../sha256.js'

const viaWebCrypto = async (s: string): Promise<string> => {
  const buf = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(s))
  return [...new Uint8Array(buf)].map((b) => b.toString(16).padStart(2, '0')).join('')
}

// Inputs chosen for the places a hand-written implementation breaks:
// the empty string, exactly one block, one byte either side of the
// padding boundary, multi-byte UTF-8, and an astral character that
// needs a surrogate pair.
const CASES = [
  '',
  'a',
  'abc',
  'dora@example.com',
  'usr_1234567890',
  // 55, 56 and 64 bytes: the length field needs its own block at 56.
  'x'.repeat(55),
  'x'.repeat(56),
  'x'.repeat(57),
  'x'.repeat(64),
  'x'.repeat(1000),
  '高木さんのメールアドレス',
  'Ünïcödé with combining é',
  '🙂 astral plane 𝔘𝔫𝔦𝔠𝔬𝔡𝔢',
  '\u{10FFFF}',
]

describe('sha256Hex agrees with WebCrypto', () => {
  for (const input of CASES) {
    const label = input.length > 24 ? `${input.slice(0, 20)}… (${input.length})` : JSON.stringify(input)
    it(`matches for ${label}`, async () => {
      expect(sha256Hex(input)).toBe(await viaWebCrypto(input))
    })
  }

  it('matches for a lone surrogate, which TextEncoder replaces', async () => {
    // A string a host can produce by slicing a name in half. Both
    // encoders must turn it into U+FFFD or the digests diverge on
    // input neither of them rejects.
    const lone = 'a\ud800b'
    expect(sha256Hex(lone)).toBe(await viaWebCrypto(lone))
  })

  it('matches on a thousand random strings', async () => {
    // The cases above are the ones I thought of. This is the ones I
    // did not.
    for (let i = 0; i < 1000; i += 1) {
      const len = Math.floor(Math.random() * 200)
      let s = ''
      for (let j = 0; j < len; j += 1) s += String.fromCodePoint(Math.floor(Math.random() * 0x2ffff))
      // eslint-disable-next-line no-await-in-loop
      expect(sha256Hex(s)).toBe(await viaWebCrypto(s))
    }
  })
})

describe('utf8Bytes agrees with TextEncoder', () => {
  it('encodes the same bytes', () => {
    for (const input of CASES) {
      expect([...utf8Bytes(input)]).toEqual([...new TextEncoder().encode(input)])
    }
  })
})
