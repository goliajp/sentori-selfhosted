// Who the person is and what the app was doing, attached to every
// event. Two verbs own this state; everything else reads it.

import { hashIdentities, safeFn } from '@goliapkg/sentori-core'
import type { User } from '@goliapkg/sentori-core'

let _userKey: string | undefined
let _context: Record<string, unknown> = {}
let _hashGeneration = 0

export const setUser = safeFn('user', function setUser(u: null | User): void {
  _userKey = undefined
  const generation = ++_hashGeneration
  if (u === null) return
  // Only a salted hash travels — breadth needs distinctness, not
  // identity. Hashing is async (WebCrypto); the verb stays
  // synchronous and the key lands a tick later, so events in that gap
  // carry no userKey, which under-counts breadth briefly rather than
  // sending a raw identity.
  void hashIdentities({ email: u.email, id: u.id })
    .then((hashes) => {
      if (generation === _hashGeneration) _userKey = hashes.id ?? hashes.email
    })
    .catch(() => {
      // NEVER rule: no key beats a raw identity on the wire.
    })
})

export const patchContext = safeFn('context', function patchContext(
  patch: Record<string, unknown>,
): void {
  _context = { ..._context, ...patch }
})

export const currentUserKey = (): string | undefined => _userKey

export const currentContext = (): Record<string, unknown> | undefined =>
  Object.keys(_context).length > 0 ? { ..._context } : undefined

export const __resetForTests = (): void => {
  _userKey = undefined
  _context = {}
  _hashGeneration = 0
}
