// Phase 39 sub-A: URL normalization for span names.
//
// `http.client GET https://api.example.com/devices/69ef2dc5c11ea38…`
// is useless on a trace list — every request to a different id is its
// own row. We collapse high-cardinality path segments (numeric ids,
// UUIDs, long hex tokens) to `{id}` so the Traces list aggregates by
// route. The *full* (auth-scrubbed) URL still goes in the `http.url`
// tag — this only shapes the human-facing `name`.
//
// Conservative on purpose: only segments we're highly confident are
// ids get replaced, so `/products/winter-jacket-2024` stays intact.
// Hosts are left alone (per-tenant subdomains are a separate problem).

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
const LONG_HEX_RE = /^[0-9a-f]{16,}$/i // mongo ObjectId (24), sha-ish, etc.
const ALL_DIGITS_RE = /^\d+$/
// 20+ char mixed alphanumeric with at least one digit — opaque tokens
// (base32/base36 ids, signed blobs). 20 is high enough to skip slugs.
const LONG_OPAQUE_RE = /^(?=.*\d)[A-Za-z0-9_-]{20,}$/

function isIdLike(segment: string): boolean {
  return (
    ALL_DIGITS_RE.test(segment) ||
    UUID_RE.test(segment) ||
    LONG_HEX_RE.test(segment) ||
    LONG_OPAQUE_RE.test(segment)
  )
}

function normalizePathname(pathname: string): string {
  // Preserve a leading slash and an empty result ('' or '/').
  const parts = pathname.split('/')
  return parts.map((seg) => (seg && isIdLike(seg) ? '{id}' : seg)).join('/')
}

/**
 * Normalize a request URL for use as a span `name`. Keeps
 * scheme + host + path (with id-like segments → `{id}`); drops the
 * query string and fragment. Falls back to treating the input as a
 * bare path if it isn't a parseable absolute URL (relative requests).
 */
export function normalizeUrl(url: string): string {
  try {
    const u = new URL(url)
    return `${u.protocol}//${u.host}${normalizePathname(u.pathname)}`
  } catch {
    // Relative URL or garbage — strip query/fragment, normalize path.
    const pathOnly = url.split(/[?#]/, 1)[0] ?? url
    return normalizePathname(pathOnly)
  }
}
