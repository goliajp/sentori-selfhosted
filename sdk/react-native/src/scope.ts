// Ambient scope: the current user and the context patch, attached
// to every outgoing event. Two verbs own this state (user() and
// context()); everything else reads it.

import { hashIdentities, safeFn } from '@goliapkg/sentori-core';
import type { User } from '@goliapkg/sentori-core';

let _user: User | null = null;
let _userKey: string | undefined;
let _context: Record<string, unknown> = {};
let _hashGeneration = 0;
let _traits: Record<string, unknown> | undefined;

// What to tell when the person changes.
//
// The device row carries the identity, and until now nothing updated
// it after registration: an app that registers for push at launch and
// signs in ten seconds later — which is every app with a login screen
// — held a row with no user on it for the life of the install. A send
// aimed at that user reached nobody and reported success.
//
// A callback rather than a direct call because push.ts already imports
// this file; the arrow has to point one way.
let _onIdentityChange: (() => void) | undefined;

/** An identity change that happened while nobody was listening.
 *  Push installs its listener only once a registration has landed, so
 *  a host that signs someone in while that request is in flight
 *  announced to an empty room — and nothing announces it again. */
let _missedAnnounce = false;

/** Register interest in identity changes. Only push does. */
export const onIdentityChange = (fn: (() => void) | undefined): void => {
  _onIdentityChange = fn;
  if (fn != null && _missedAnnounce) {
    _missedAnnounce = false;
    announce();
  }
};

const announce = (): void => {
  if (_onIdentityChange == null) {
    // Replayed by `onIdentityChange` when someone starts listening.
    // Deliberately not a queue: identity is a current value, not a
    // stream, so one flag replays the latest and no more.
    _missedAnnounce = true;
    return;
  }
  try {
    _onIdentityChange();
  } catch {
    // NEVER rule: whatever the listener does, user() returns.
  }
};

export const setUser = safeFn('user', function setUser(u: User | null): void {
  _user = u;
  _userKey = undefined;
  // A call to `user()` describes the person completely, so one made
  // without traits means they have none — not "leave the last ones".
  // Absent and empty are different on the wire: absent keeps what the
  // row has, and a signed-out device that kept them would still be
  // selectable as whoever just left.
  _traits = u?.traits == null ? {} : { ...u.traits };
  const generation = ++_hashGeneration;
  if (u === null) {
    // Signing out is an identity change too, and the one where a
    // stale row matters most: the device would otherwise keep
    // answering to the person who just left.
    announce();
    return;
  }
  // The wire carries only a salted hash — raw identity never leaves
  // the device (breadth × depth needs distinctness, not identity).
  // Hashing is async (WebCrypto); the verb stays synchronous and the
  // key lands a tick later. Events sent in that gap simply carry no
  // userKey, which only under-counts breadth briefly.
  void hashIdentities({ email: u.email, id: u.id })
    .then((hashes) => {
      if (generation === _hashGeneration) {
        _userKey = hashes.id ?? hashes.email;
        // Announced after the hash lands, not before: the whole point
        // is to send the new key, and the key is what arrives late.
        announce();
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

/** The person's attributes, for the device row. Undefined when the
 *  host has not set any — which is different from `{}`, the value that
 *  clears them. */
export const currentUserTraits = (): Record<string, unknown> | undefined =>
  _traits === undefined ? undefined : { ..._traits };

export const currentContext = (): Record<string, unknown> | undefined =>
  Object.keys(_context).length > 0 ? { ..._context } : undefined;

export const __resetForTests = (): void => {
  _user = null;
  _userKey = undefined;
  _traits = undefined;
  _context = {};
  _onIdentityChange = undefined;
  _missedAnnounce = false;
  _hashGeneration++;
};
