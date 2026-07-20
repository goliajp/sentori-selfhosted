/**
 * v2.3 W6.3 — Sentry-compatible API surface.
 *
 * Drop-in for code (or LLM-generated code) written against
 * `@sentry/react-native`. Every Sentry call maps to exactly one
 * Sentori-native call internally. Translation differences (e.g.
 * `Sentry.setUser({ ip_address })` — Sentori never stores IP) fire
 * a one-shot console hint at `info` level, deduplicated per
 * (api, dropped_field).
 *
 * Why this exists: LLMs have seen a LOT of Sentry code; letting
 * them write the same syntax against Sentori is one less thing
 * Sentori asks the host to think about. Combined with the v2.3
 * "free bonus" stance — host adds Sentori without unlearning
 * anything.
 *
 * Usage:
 *
 *   import * as Sentry from '@goliapkg/sentori-react-native/compat'
 *
 *   Sentry.init({ dsn: 'https://<token>@<host>/<projectId>', ... })
 *   Sentry.captureException(err)
 *   Sentry.setUser({ id, email })   // email → linkBy.email (hashed)
 *
 * The compat layer holds NO state of its own — it's a thin shim
 * over the same Sentori internals. Mixing `Sentry.*` and
 * `sentori.*` calls in the same app works fine.
 *
 * See `docs/design/sdk-v2.3-redesign.md` §4 for the full
 * translation table + design rationale.
 */

import { logger } from '@goliapkg/sentori-core'

import { addBreadcrumb as nativeAddBreadcrumb } from '../breadcrumbs'
import {
  captureException as nativeCaptureException,
  captureMessage as nativeCaptureMessage,
  setTag as nativeSetTag,
  setTags as nativeSetTags,
  setUser as nativeSetUser,
} from '../capture'
import { type InitOptions, init as nativeInit } from '../init'

// ── one-shot warn dedup ───────────────────────────────────────────────────

const _warnedOnce = new Set<string>()
function warnOnce(key: string, msg: string): void {
  if (_warnedOnce.has(key)) return
  _warnedOnce.add(key)
  logger.info('compat', msg)
}

// ── DSN parsing ───────────────────────────────────────────────────────────

type SentryInitOpts = {
  dsn: string
  environment?: string
  release?: string
  tracesSampleRate?: number
  sampleRate?: number
  attachStacktrace?: boolean
  autoSessionTracking?: boolean
  /** Sentry's `debug: true` ≈ Sentori's `logLevel: 'debug'`. */
  debug?: boolean
  /** Catch-all for fields Sentori either ignores or doesn't map yet. */
  [other: string]: unknown
}

/** Exported for tests only — direct callers should use `Sentry.init`,
 *  which threads the parsed DSN through the rest of the init machinery. */
export const __parseDsnForTests = parseDsn

function parseDsn(dsn: string): { token: string; ingestUrl: string } {
  // Sentry DSN shape: `https://<key>@<host>[:port][/<projectId>]`
  // Sentori cares about `<key>` (must be `st_pk_…`) and `<host>`.
  let url: URL
  try {
    url = new URL(dsn)
  } catch {
    throw new Error(`Sentory compat: dsn is not a valid URL: ${dsn}`)
  }
  const key = url.username
  if (!key) {
    throw new Error(`Sentory compat: dsn missing token in user-info component`)
  }
  if (!key.startsWith('st_pk_')) {
    throw new Error(
      `Sentory compat: dsn token must start with 'st_pk_' (got prefix '${key.slice(0, 8)}…'). ` +
        `Sentori does not parse Sentry-issued tokens — generate a Sentori project token via the dashboard.`,
    )
  }
  // strip user-info to reconstruct the ingest origin
  const ingestUrl = `${url.protocol}//${url.host}`
  return { token: key, ingestUrl }
}

// ── Sentry.init ───────────────────────────────────────────────────────────

export function init(opts: SentryInitOpts): void {
  const { token, ingestUrl } = parseDsn(opts.dsn)

  const sentoriOpts: InitOptions = {
    token,
    release: opts.release ?? '',
    ingestUrl,
    ...(opts.environment ? { environment: opts.environment } : {}),
    sample: {
      ...(opts.tracesSampleRate !== undefined ? { traces: opts.tracesSampleRate } : {}),
      ...(opts.sampleRate !== undefined ? { errors: opts.sampleRate } : {}),
    },
    ...(opts.debug ? { logLevel: 'debug' as const } : {}),
  }

  // Empty release is the most common Sentry-init mistake; warn but
  // continue.
  if (!sentoriOpts.release) {
    warnOnce(
      'init:no-release',
      'Sentry.init() with no `release` — Sentori requires release for grouping + drop-down menus to make sense. Set `release: "myapp@1.2.3"` for production cuts.',
    )
    // Sentori's init throws when release is empty; provide a
    // reasonable fallback so the rest of the code path works in dev.
    sentoriOpts.release = `unspecified@${Date.now()}`
  }

  // Pass-through informational hints for ignored fields.
  for (const ignored of [
    'attachStacktrace',
    'autoSessionTracking',
    'integrations',
    'beforeSend',
    'beforeBreadcrumb',
    'maxBreadcrumbs',
  ]) {
    if (ignored in opts) {
      warnOnce(
        `init:ignored:${ignored}`,
        `Sentry.init({ ${ignored} }) ignored. ` +
          `${getIgnoredHint(ignored)}`,
      )
    }
  }

  nativeInit(sentoriOpts)
}

function getIgnoredHint(field: string): string {
  switch (field) {
    case 'attachStacktrace':
      return 'Sentori always sends stack traces — no toggle.'
    case 'autoSessionTracking':
      return "Sentori sessions are on by default; toggle via `init({ capture: { sessions: true|false } })`."
    case 'integrations':
      return 'Sentori uses `init({ capture: {...} })` toggles instead of Integration classes — see the docs.'
    case 'beforeSend':
    case 'beforeBreadcrumb':
      return 'Sentori does not support an arbitrary beforeSend hook today. Server-side PII scrubbing is automatic.'
    case 'maxBreadcrumbs':
      return 'Sentori uses a fixed 100-slot ring buffer.'
    default:
      return ''
  }
}

// ── Severity / level enum ─────────────────────────────────────────────────

/** Sentry's severity values, surfaced as strings (Sentori only
 *  uses 5 levels). `Log` and `Critical` collapse to `'info'` and
 *  `'fatal'` respectively. */
export const Severity = {
  Fatal: 'fatal' as const,
  Critical: 'fatal' as const,
  Error: 'error' as const,
  Warning: 'warning' as const,
  Log: 'info' as const,
  Info: 'info' as const,
  Debug: 'debug' as const,
}

type SentryLevelString =
  | 'critical'
  | 'debug'
  | 'error'
  | 'fatal'
  | 'info'
  | 'log'
  | 'warning'

type SentoriLevel = 'debug' | 'error' | 'fatal' | 'info' | 'warning'

function mapLevel(level: SentryLevelString | undefined): SentoriLevel | undefined {
  if (!level) return undefined
  switch (level) {
    case 'critical':
      warnOnce(
        'severity:critical',
        "Sentry.Severity.Critical → mapped to 'fatal' (Sentori's 5-level syslog-style scale).",
      )
      return 'fatal'
    case 'log':
      warnOnce(
        'severity:log',
        "Sentry.Severity.Log → mapped to 'info' (Sentori's 5-level scale has no separate Log).",
      )
      return 'info'
    default:
      return level as SentoriLevel
  }
}

// ── captureException ──────────────────────────────────────────────────────

type SentryCaptureContext = {
  tags?: Record<string, string>
  extra?: Record<string, unknown>
  level?: SentryLevelString
  fingerprint?: string[]
  user?: SentrySetUserInput
}

export function captureException(
  err: unknown,
  hint?: { captureContext?: SentryCaptureContext } | SentryCaptureContext,
): void {
  // Sentry v8+ takes the context inline (Hint); earlier versions
  // wrapped it in `{ captureContext: {...} }`. Accept both.
  const ctx: SentryCaptureContext | undefined = (() => {
    if (!hint) return undefined
    if ('captureContext' in (hint as { captureContext?: unknown })) {
      return (hint as { captureContext?: SentryCaptureContext }).captureContext
    }
    return hint as SentryCaptureContext
  })()

  if (ctx?.extra) {
    warnOnce(
      'captureException:extra',
      'Sentry.captureException(err, { extra }) → `extra` mapped to `tags` (Sentori does not have a separate extra namespace).',
    )
  }
  if (ctx?.user) {
    // Apply the per-call user via setUser (Sentori takes the
    // current scope user automatically).
    setUser(ctx.user)
  }

  const mergedTags = {
    ...(ctx?.tags ?? {}),
    ...(ctx?.extra
      ? Object.fromEntries(
          Object.entries(ctx.extra).map(([k, v]) => [k, String(v)]),
        )
      : {}),
  }

  nativeCaptureException(err as Error, {
    ...(Object.keys(mergedTags).length > 0 ? { tags: mergedTags } : {}),
    ...(ctx?.fingerprint ? { fingerprint: ctx.fingerprint } : {}),
  })
}

// ── captureMessage ────────────────────────────────────────────────────────

export function captureMessage(
  msg: string,
  levelOrCtx?: SentryCaptureContext | SentryLevelString,
): void {
  let level: SentoriLevel | undefined
  let tags: Record<string, string> | undefined
  if (typeof levelOrCtx === 'string') {
    level = mapLevel(levelOrCtx)
  } else if (levelOrCtx) {
    level = mapLevel(levelOrCtx.level)
    tags = levelOrCtx.tags
  }
  nativeCaptureMessage(msg, {
    ...(level ? { level } : {}),
    ...(tags ? { tags } : {}),
  })
}

// ── setUser ───────────────────────────────────────────────────────────────

type SentrySetUserInput = {
  id?: string
  email?: string
  username?: string
  ip_address?: string
  segment?: string
  [other: string]: unknown
} | null

export function setUser(user: SentrySetUserInput): void {
  if (user == null) {
    nativeSetUser(null)
    return
  }
  const { id, email, username, ip_address, segment, ...rest } = user

  if (ip_address !== undefined) {
    warnOnce(
      'setUser:ip_address',
      'Sentry.setUser({ ip_address }) → dropped. Sentori never stores IP (privacy by design).',
    )
  }
  if (segment !== undefined) {
    warnOnce(
      'setUser:segment',
      'Sentry.setUser({ segment }) → mapped to tag `user.segment`. Set via setTag for clarity.',
    )
    if (typeof segment === 'string') nativeSetTag('user.segment', segment)
  }

  const linkBy: Record<string, string> = {}
  if (email) linkBy.email = email
  if (username) linkBy.username = username

  // Surface any other fields the host bolted on (Sentry historically
  // accepted arbitrary keys) — they pass through as tags.
  for (const [k, v] of Object.entries(rest)) {
    if (v !== undefined && v !== null) nativeSetTag(`user.${k}`, String(v))
  }

  nativeSetUser({
    ...(id ? { id } : {}),
    ...(Object.keys(linkBy).length > 0 ? { linkBy } : {}),
  })
}

// ── setTag / setTags ──────────────────────────────────────────────────────

// Re-export Sentori-native semantics; identical signatures.
export const setTag = nativeSetTag
export const setTags = nativeSetTags

// ── addBreadcrumb ─────────────────────────────────────────────────────────

type SentryBreadcrumb = {
  category?: string
  message?: string
  level?: SentryLevelString
  type?: 'default' | 'error' | 'http' | 'info' | 'navigation' | 'query' | 'user' | string
  data?: Record<string, unknown>
  timestamp?: number
}

type SentoriBreadcrumbType = 'custom' | 'log' | 'nav' | 'net' | 'user'

/** Exported for tests only. */
export const __mapCategoryToTypeForTests = mapCategoryToType
export const __mapLevelForTests = mapLevel

function mapCategoryToType(category: string | undefined): SentoriBreadcrumbType | undefined {
  if (!category) return undefined
  if (['auth', 'click', 'gesture', 'input', 'touch', 'ui'].includes(category)) return 'user'
  if (['fetch', 'http', 'xhr'].includes(category)) return 'net'
  if (['nav', 'navigation', 'route'].includes(category)) return 'nav'
  if (['console', 'log', 'sentry'].includes(category)) return 'log'
  return 'custom'
}

function mapSentryType(t: string | undefined): SentoriBreadcrumbType | undefined {
  if (!t) return undefined
  switch (t) {
    case 'http':
      return 'net'
    case 'navigation':
      return 'nav'
    case 'user':
    case 'log':
    case 'custom':
      return t
    default:
      return 'custom'
  }
}

export function addBreadcrumb(crumb: SentryBreadcrumb): void {
  if (!crumb.message) {
    crumb.message = crumb.category ?? crumb.type ?? '(no message)'
  }
  if (crumb.category && !crumb.type) {
    warnOnce(
      'breadcrumb:category',
      'Sentry.addBreadcrumb({ category }) → mapped to `type` via well-known table; the category string itself is preserved under `data.category`.',
    )
  }
  // RN SDK shape: { type, data }. No top-level `message` —
  // Sentry's message goes into data.message.
  nativeAddBreadcrumb({
    type: mapSentryType(crumb.type) ?? mapCategoryToType(crumb.category) ?? 'custom',
    data: {
      message: crumb.message,
      ...(crumb.data ?? {}),
      ...(crumb.category ? { category: crumb.category } : {}),
      ...(crumb.level ? { level: mapLevel(crumb.level) } : {}),
    },
  })
}

// ── flush / close ─────────────────────────────────────────────────────────

// Re-export Sentori native flush + close — same signatures.
export { close, flush } from '../lifecycle'

// ── startTransaction / startSpan / withScope ──────────────────────────────

// Trace mapping is non-trivial (Sentry's transaction object exposes
// startChild, etc.). v2.3 ships a minimum-viable Sentry trace
// surface that supports startTransaction returning a Sentori Span
// object with a partial Sentry-style API (.startChild, .finish).
// Anything beyond that throws a clear error directing to the native
// `sentori.startSpan` / `sentori.withScopedSpan`.

import { startSpan } from '@goliapkg/sentori-core'

type SentrySpanOpts = {
  op?: string
  name?: string
  description?: string
  tags?: Record<string, string>
}

export function startTransaction(opts: SentrySpanOpts): {
  finish: (status?: 'cancelled' | 'error' | 'ok') => void
  setStatus: (status: 'cancelled' | 'error' | 'ok') => void
  setTag: (k: string, v: string) => void
  startChild: (childOpts: SentrySpanOpts) => unknown
} {
  warnOnce(
    'startTransaction',
    'Sentry.startTransaction() → mapped to sentori.startSpan() with op as name. Native equivalent: sentori.startTrace(name) or sentori.startSpan({ name }).',
  )
  const name = opts.name ?? opts.op ?? 'transaction'
  const span = startSpan(name, {
    parent: null,
    tags: opts.tags,
  })
  return {
    finish: (status) => span.finish({ status: status === 'ok' ? 'ok' : 'error' }),
    setStatus: (status) => { span.setTag('status', status) },
    setTag: (k, v) => { span.setTag(k, v) },
    startChild: (childOpts) => {
      return startSpan(childOpts.name ?? childOpts.op ?? 'child', {
        tags: childOpts.tags,
      })
    },
  }
}

// ── withScope / configureScope (no-op scoping; same state as native) ─────

type ScopeProxy = {
  setTag: (k: string, v: string) => void
  setTags: (rec: Record<string, string>) => void
  setUser: (u: SentrySetUserInput) => void
  setExtra: (k: string, v: unknown) => void
  setLevel: (l: SentryLevelString) => void
}

function scopeProxy(): ScopeProxy {
  return {
    setTag: nativeSetTag,
    setTags: nativeSetTags,
    setUser,
    setExtra: (k, v) => nativeSetTag(`extra.${k}`, String(v)),
    setLevel: () => {
      warnOnce(
        'scope:setLevel',
        'Sentry.withScope(s => s.setLevel(…)) → not supported. Sentori levels travel on capture call, not on scope.',
      )
    },
  }
}

export function withScope<T>(fn: (scope: ScopeProxy) => T): T {
  // Sentori has no Hub; tags set inside `fn` persist (best-effort
  // approximation of Sentry semantics). For most callers this is
  // fine; tighter isolation needs an actual Hub which we don't ship.
  warnOnce(
    'withScope',
    'Sentry.withScope() → tag mutations are NOT auto-reverted on scope exit. Use sentori.setTag/clearTags explicitly for tight isolation.',
  )
  return fn(scopeProxy())
}

export function configureScope(fn: (scope: ScopeProxy) => void): void {
  fn(scopeProxy())
}
