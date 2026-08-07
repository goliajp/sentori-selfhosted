// Ambient scope: the current user and the context patch, attached
// to every outgoing event. Two verbs own this state (user() and
// context()); everything else reads it.

import { hashIdentities, safeFn } from '@goliapkg/sentori-core';
import type { User } from '@goliapkg/sentori-core';

let _user: User | null = null;
let _userKey: string | undefined;
let _context: Record<string, unknown> = {};
let _hashGeneration = 0;

export const setUser = safeFn('user', function setUser(u: User | null): void {
  _user = u;
  _userKey = undefined;
  const generation = ++_hashGeneration;
  if (u === null) return;
  // The wire carries only a salted hash — raw identity never leaves
  // the device (breadth × depth needs distinctness, not identity).
  // Hashing is async (WebCrypto); the verb stays synchronous and the
  // key lands a tick later. Events sent in that gap simply carry no
  // userKey, which only under-counts breadth briefly.
  void hashIdentities({ email: u.email, id: u.id })
    .then((hashes) => {
      if (generation === _hashGeneration) {
        _userKey = hashes.id ?? hashes.email;
      }
    })
    .catch(() => {
      // NEVER rule: no key beats a raw identity on the wire.
    });
});

export const patchContext = safeFn('context', function patchContext(
  patch: Record<string, unknown>,
): void {
  _context = { ..._context, ...patch };
});

export const currentUserKey = (): string | undefined => _userKey;

export const currentContext = (): Record<string, unknown> | undefined =>
  Object.keys(_context).length > 0 ? { ..._context } : undefined;

export const __resetForTests = (): void => {
  _user = null;
  _userKey = undefined;
  _context = {};
  _hashGeneration++;
};
