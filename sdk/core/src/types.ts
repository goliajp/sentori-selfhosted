// The v1 wire protocol — single source of truth for what the SDK
// sends and the server stores (design.md §2/§4; server counterpart:
// self-hosted/server/src/handlers/sdk/events.rs).
//
// Five kinds, five verbs, no severity dimension. Breadcrumbs, spans,
// message levels and the capture* vocabulary are gone with the
// Sentry compatibility they came from.

export type Platform = 'android' | 'ios' | 'javascript'

/** The five kinds. The union IS the concept model. */
export type EventKind = 'assert' | 'error' | 'probe' | 'trace' | 'warn'

/** One stack frame, symbolication-ready. */
export type Frame = {
  file?: string
  function?: string
  line?: number
  column?: number
  inApp?: boolean
  absolutePath?: string
  /** True once real source positions are resolved — set by the
   *  server at ingest (release builds) or by the SDK's Metro
   *  symbolication (dev builds). The dashboard's MINIFIED badge
   *  keys off it. */
  symbolicated?: boolean
  /** Source window around the failing line, when the symbolication
   *  source had one (Metro's codeFrame in dev; the sourcemap's
   *  sourcesContent at ingest). */
  preContext?: string[]
  contextLine?: string
  postContext?: string[]
}

/** A thrown error, normalized (coerce-error.ts produces these). */
export type SentoriError = {
  type: string
  message: string
  stack?: Frame[]
  cause?: SentoriError | null
}

/**
 * One entry of the signal ring — the last-60-seconds context that
 * ships inside `payload.signals` when an error/warn goes out.
 * Replaces the Sentry breadcrumb + trail pair.
 */
export type Signal = {
  /** Seconds relative to the event (negative = before). */
  t: number
  /**
   * Auto-produced: nav | tap | lifecycle | freeze | push.
   * Host-produced via `pushSignal` (any kind accepted); dashboard
   * conventions: `http` with { method, url, status, ms } for a
   * request's outcome, `trace` for quiet business breadcrumbs.
   */
  kind: string
  data?: Record<string, unknown>
}

export type Device = {
  os: string
  osVersion?: string
  model?: string
  locale?: string
  screen?: { width: number; height: number; scale?: number }
  memoryMb?: number
  batteryLevel?: number
  network?: string
}

export type App = {
  version: string
  build?: string
  framework?: { name: string; version: string }
}

/** Where a warn happened — fingerprint input on the server. */
export type Surface = {
  screen?: string
  element?: string
  [k: string]: unknown
}

/**
 * One event on the wire. Top-level fields are what the server
 * routes/fingerprints on; everything else rides in `payload`
 * untouched (zero-migration SDK additions).
 */
export type WireEvent = {
  /** Client-minted UUIDv7; the server accepts or mints. */
  id?: string
  kind: EventKind
  /** RFC 3339. */
  occurredAt: string
  platform: Platform
  release?: string
  environment?: string
  /** warn/trace/assert name; probe ref. */
  name?: string
  surface?: Surface
  /** Salted identity hash — computed client-side (identity.ts). */
  userKey?: string
  payload: WirePayload
}

export type WirePayload = {
  error?: SentoriError
  device?: Device
  app?: App
  signals?: Signal[]
  /** The verb's data argument, error instances already serialized. */
  data?: Record<string, unknown>
  /** Ambient context (flags / tags) as patched via sentori.context(). */
  context?: Record<string, unknown>
  [k: string]: unknown
}

/** Client-side aggregate of assert passes (design.md §2). */
export type AssertStat = {
  name: string
  release?: string
  passDelta: number
  failDelta?: number
}

/** POST /v1/events:batch envelope. */
export type BatchEnvelope = {
  events: WireEvent[]
  assertStats?: AssertStat[]
  /** The integrator's backend health-check URL (from init) — the
   *  server remembers it per project and probes it for the
   *  availability card. */
  backendHealthUrl?: string
}

/** Per-event server outcome. */
export type IngestOutcome = {
  eventId?: string
  issueId?: string
  isNewIssue?: boolean
  regressed?: boolean
  error?: string
}

export type BatchResponse = {
  accepted: number
  outcomes: IngestOutcome[]
}

export type AttachmentKind =
  | 'logTail'
  | 'replay'
  | 'screens'
  | 'screenshot'
  | 'stateSnapshot'
  | 'viewTree'

export type AttachmentSource = 'android' | 'ios' | 'js'

// ── The 8-verb API surface (design.md §4) ──────────────────────────
//
// All synchronous, no Promises, never throw (the zero-cost iron
// rule); every event verb returns the client-minted event id.

export type User = {
  id?: string
  name?: string
  email?: string
  /** Attributes a push campaign can select on: plan, cohort, org.
   *
   *  These travel raw, unlike `id` and `email`, which only ever leave
   *  as a hash. That pairing is the point — the identity stays
   *  unreadable and the attributes stay selectable — so put nothing
   *  here that identifies the person. A plan name is a trait; an email
   *  address is not.
   *
   *  They reach the device row on the next push registration, and
   *  registering happens again by itself when this changes. */
  traits?: Record<string, string | number | boolean | null>
}

export type EventData = Record<string, unknown>

export type TraceOptions = {
  /** Ring-only: keep it as context, do not report an event. */
  quiet?: boolean
}

export interface SentoriApi {
  init(config: InitConfig): void
  user(u: User | null): void
  context(patch: Record<string, unknown>): void

  error(err: unknown, data?: EventData): string
  warn(name: string, data?: EventData): string
  trace(name: string, data?: EventData, opts?: TraceOptions): string
  assert(name: string, ok: boolean, data?: EventData): string
  probe(ref: string, data?: EventData): string
}

export type InitConfig = {
  /** Ingest token (`st_…`), scope `ingest`. */
  token: string
  /** The instance to report to, e.g. `https://sentori.golia.jp`. */
  ingestUrl: string
  release?: string
  environment?: string
  /** Warn-scenario auto-detection switches; conservative defaults. */
  detect?: {
    rageTap?: boolean
    longFreeze?: boolean
    slowColdStart?: boolean
    slowApi?: boolean
  }
  /** B-type replay rolling buffer, seconds. 0 disables. */
  replaySeconds?: number
  /** Visual replay: a rolling ring of low-bitrate screenshots
   *  covering `replaySeconds`, shipped only when an error/warn
   *  fires. OFF by default — screenshots can carry user content;
   *  pair with `registerMaskQuery` before enabling. */
  replayScreens?: boolean
  /** A GET-able health endpoint of YOUR backend. Sentori's server
   *  probes it once a minute and shows availability next to the
   *  project — the SDK only carries the URL, the app never pings. */
  backendHealthUrl?: string
  /** Console gate: default `warn` — silent unless genuinely broken. */
  logLevel?: 'debug' | 'error' | 'info' | 'silent' | 'warn'
  /** Last-resort event filter; exceptions fall back to the event. */
  beforeSend?: (event: WireEvent) => WireEvent | null
}
