/**
 * v2.3 — client-side identity hashing for cross-project user lookup.
 *
 * Host call:
 *   sentori.setUser({ id: 'usr_123', linkBy: { email: 'a@b.com' } })
 *
 * The SDK:
 *   1. Normalises each `linkBy` value per well-known type (email
 *      lowercase+trim, phone digit-only+'+' prefix, etc).
 *   2. Hashes the normalised value via `crypto.subtle.digest('SHA-256',
 *      …)`. Output = 64-char lowercase hex.
 *   3. Discards the raw value. Only the `linkHashes` map is stored
 *      on scope and travels over the wire.
 *
 * Server then layers a per-scope salt on top (`sha256(scope.salt ||
 * key_type || ':' || client_hash)`) before writing the
 * `identity_fingerprints` denorm row. Raw values never reach the
 * server.
 *
 * Privacy contract: this module is the **single source of truth**
 * for what gets hashed and how. Any future identity-related code
 * must route through `hashIdentities` so the contract holds.
 */

import { sha256Hex } from './sha256.js'

/** Map of (key_type → raw_value) accepted at the public SDK
 *  `setUser` API. Common types have well-known normalisation; the
 *  index signature lets host apps add custom keys. */
export type LinkBy = {
  email?: string
  phone?: string
  googleSub?: string
  appleSub?: string
  metaSub?: string
  username?: string
} & Record<string, string | undefined>

/** Normalise + hash a single (type, value) pair. Returns null on
 *  empty / undefined input. */
async function hashOne(keyType: string, raw: string | undefined): Promise<null | string> {
  if (raw == null || raw === '') return null
  const normalised = normalise(keyType, raw)
  if (normalised === '') return null

  // Most platforms (browsers, RN 0.71+ via Hermes, Node 18+) expose
  // `globalThis.crypto.subtle`, and it is native code, so it is used
  // wherever it exists.
  //
  // WeChat mini programs do not have it. This used to throw there,
  // `setUser` swallowed the throw per the NEVER rule, and every event
  // from a mini program shipped with no `userKey` — breadth reading
  // zero, push reaching nobody, and nothing anywhere saying why. The
  // fallback is plain JavaScript and produces the same 64 characters;
  // `sha256.test.ts` holds it to `crypto.subtle`'s own answers,
  // including on a thousand random strings, because a hash that is
  // only self-consistent makes one person into two.
  const subtle = globalThis.crypto?.subtle
  if (!subtle) return sha256Hex(normalised)
  const enc = new TextEncoder()
  const buf = await subtle.digest('SHA-256', enc.encode(normalised))
  return bufferToHex(buf)
}

function bufferToHex(buf: ArrayBuffer): string {
  const bytes = new Uint8Array(buf)
  let out = ''
  for (let i = 0; i < bytes.length; i += 1) {
    const h = bytes[i]!.toString(16)
    out += h.length === 1 ? '0' + h : h
  }
  return out
}

/** Well-known type normalisations. Custom keys pass through with a
 *  generic trim. */
function normalise(keyType: string, raw: string): string {
  switch (keyType) {
    case 'email':
      return raw.trim().toLowerCase()
    case 'phone':
      // Strip everything that isn't `+` or `0-9`. Conservative: doesn't
      // enforce E.164 — that's the host's job. We just kill formatting
      // noise so `'+81 (90) 1234-5678'` and `'+81 9012345678'` hash same.
      return raw.replace(/[^+\d]/g, '')
    case 'username':
      return raw.trim().toLowerCase()
    case 'googleSub':
    case 'appleSub':
    case 'metaSub':
      // OAuth sub claims are opaque ids — no normalisation.
      return raw
    default:
      return raw.trim()
  }
}

/**
 * Hash every entry in a LinkBy bag concurrently. Returns the
 * `linkHashes` record ready to attach to the User wire payload.
 *
 * Failures propagate to the caller so `setUser` can decide what to do
 * (most paths swallow via safeFn per the NEVER rule, ending up with no
 * linkHashes — better than sending raw). A missing `crypto.subtle` is
 * no longer one of them: there is a fallback now.
 */
export async function hashIdentities(linkBy: LinkBy): Promise<Record<string, string>> {
  const entries = Object.entries(linkBy)
  const out: Record<string, string> = {}
  await Promise.all(
    entries.map(async ([key, val]) => {
      const h = await hashOne(key, val)
      if (h !== null) out[key] = h
    }),
  )
  return out
}
