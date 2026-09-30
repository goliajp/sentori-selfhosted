// What an SDK does when the server refuses the platform it sends.
//
// A batch answers 200 and puts each refusal in an outcome, so an SDK
// newer than its server loses every event and nothing says so. Servers
// from v4 on store an unrecognised platform as `unknown` instead, but
// the instances already in the field do not, and a self-hosted product
// cannot assume the server was upgraded first — that ordering is the
// operator's, not ours.
//
// So the SDK reads the refusals it got back, and if they are about the
// platform it drops to a value every shipped server accepts and keeps
// reporting. The events in that batch are already lost; what this
// saves is every batch after it.

import type { Platform } from './types'

/** Accepted by every server this product has ever shipped. */
export const PLATFORM_FALLBACK: Platform = 'javascript'

/** One entry of a batch response's `outcomes` array. */
export type Outcome = { error?: string; detail?: string }

/**
 * Whether these outcomes say the platform was the problem.
 *
 * Matched on the detail text rather than a code because the servers
 * this exists for are already released: they answer
 * `invalid_payload` with `detail: "platform must be ..."` and will
 * never gain a more specific code. Requiring `invalid_payload` as
 * well keeps an unrelated message that happens to contain the word
 * from degrading a session for nothing.
 */
export function refusalIsAboutPlatform(outcomes: readonly Outcome[] | undefined): boolean {
  return (outcomes ?? []).some(
    (o) => o?.error === 'invalid_payload' && (o.detail ?? '').includes('platform'),
  )
}

let _degraded = false

/**
 * Drop this session to {@link PLATFORM_FALLBACK}.
 *
 * Session-scoped and one-way: the next launch tries the real value
 * again, because the operator may have upgraded in between, and
 * nothing that happens inside one run tells us they did.
 *
 * Warns once. A line per batch would be the SDK shouting about a
 * condition it has already handled, which is how a host team decides
 * the thing in their console is the problem.
 */
export function degradePlatform(warn: (message: string) => void): void {
  if (_degraded) return
  _degraded = true
  warn(
    `this server refuses the platform this SDK reports; sending '${PLATFORM_FALLBACK}' ` +
      'for the rest of this session. Events already sent in the refused batch are lost. ' +
      'Upgrading the server keeps the real platform.',
  )
}

/** The platform to put on the wire: the real one, unless degraded. */
export function platformOrFallback(actual: Platform): Platform {
  return _degraded ? PLATFORM_FALLBACK : actual
}

export function __resetPlatformDegradeForTests(): void {
  _degraded = false
}
