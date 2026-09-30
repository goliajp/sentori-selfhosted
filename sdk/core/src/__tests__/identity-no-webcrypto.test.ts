// The same identity, on a runtime with no WebCrypto.
//
// `gen-identity-vectors --check` runs in Node, which has
// `crypto.subtle`, so it only ever exercises the native path. The
// fixtures it guards are the ones Swift and Kotlin assert against —
// and a mini program has to land on the same values or the same
// person becomes two.

import { afterEach, describe, expect, it } from 'bun:test'

import vectors from '../../../native/fixtures/identity-vectors.json'
import { hashIdentities } from '../identity.js'

type Vector = { keyType: string; raw: string; sha256Hex: string }

const realCrypto = globalThis.crypto

afterEach(() => {
  Object.defineProperty(globalThis, 'crypto', { value: realCrypto, configurable: true })
})

/** What a WeChat mini program looks like from here. */
const withoutWebCrypto = (): void => {
  Object.defineProperty(globalThis, 'crypto', { value: undefined, configurable: true })
}

describe('identity without crypto.subtle', () => {
  it('produces the vectors the native SDKs assert against', async () => {
    const list = (vectors as { vectors: Vector[] }).vectors
    expect(list.length).toBeGreaterThan(0)
    withoutWebCrypto()
    for (const v of list) {
      // eslint-disable-next-line no-await-in-loop
      const out = await hashIdentities({ [v.keyType]: v.raw })
      expect(out[v.keyType]).toBe(v.sha256Hex)
    }
  })

  it('does not throw, which is what it used to do', async () => {
    withoutWebCrypto()
    const out = await hashIdentities({ id: 'usr_1', email: 'A@B.com ' })
    // Normalisation still applies: the fallback is the digest, not a
    // different pipeline.
    expect(Object.keys(out).sort()).toEqual(['email', 'id'])
    expect(out.email).toHaveLength(64)
  })
})
