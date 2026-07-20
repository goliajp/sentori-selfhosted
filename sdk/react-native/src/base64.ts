/**
 * UTF-8-safe base64 encoder used by every JSON attachment path
 * (sessionTrail, stateSnapshot, replay).
 *
 * Why this needs its own helper:
 *   • Hermes' `globalThis.btoa` (and the WHATWG spec) is **Latin-1
 *     only** — it throws `InvalidCharacterError` on any code point
 *     > 0xFF. A wireframe NDJSON that includes a TextView with
 *     Japanese / Chinese / em-dash text triggers it; the JS-side
 *     `try / catch` then swallows the throw and the replay
 *     attachment silently never lands.
 *   • Insight 2026-05-18 rc.3 verify hit exactly this on Android —
 *     the walker fix in rc.3 surfaced deep TextView text, which
 *     then collided with the unsafe `btoa(ndjson)` path that had
 *     worked accidentally on rc.2's shallow (text-free) snapshots.
 *
 * The pattern `btoa(unescape(encodeURIComponent(s)))` rewrites the
 * UTF-8 byte sequence into a Latin-1-equivalent string that btoa
 * can chew. `unescape` is deprecated for HTML but its byte-level
 * behaviour is stable across every JS engine we ship to.
 *
 * Node / bun test fallback uses `Buffer` directly.
 */
export function base64Utf8(s: string): string {
  if (typeof globalThis.btoa === 'function') {
    return globalThis.btoa(unescape(encodeURIComponent(s)));
  }
  return Buffer.from(s, 'utf8').toString('base64');
}
