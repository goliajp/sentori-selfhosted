/**
 * Wire-format types for the Sentori `/v1/events` endpoint.
 *
 * Single source of truth shared by every `@goliapkg/sentori-*` SDK and
 * mirrored by the server's `event::Event` Rust type. Anything added /
 * removed / renamed here must move in lockstep with `docs/protocol.md`
 * and the server.
 */

export type Platform = 'android' | 'ios' | 'javascript'
export type DeviceOS = 'android' | 'ios' | 'other' | 'web'
/**
 * `error` is the default — anything thrown / uncaught.
 * `anr` is the Android ANR watchdog (≥ 5 s main-thread freeze) and,
 * once Phase 22 sub-E lands, iOS hang detection.
 */
export type EventKind = 'anr' | 'error' | 'message' | 'nearCrash'

/**
 * Severity level for `kind: 'message'` events. 5 levels aligned with
 * RFC 5424 / syslog. We deliberately do NOT include Sentry's `'log'`
 * level — it overlaps `'info'` and creates confusion about when to
 * use which.
 */
export type MessageLevel = 'debug' | 'error' | 'fatal' | 'info' | 'warning'

/**
 * Options for `captureMessage(message, opts?)`. Strictly typed —
 * the function does NOT accept `(message, level)` as Sentry does
 * (that overload makes TS inference + LLM autocompletion ambiguous).
 *
 *     sentori.captureMessage('user denied location permission')
 *     sentori.captureMessage('Payment fell back to provider B', {
 *       level: 'warning',
 *       tags: { feature: 'maps' },
 *     })
 */
export type CaptureMessageOptions = {
  /** Default `'info'`. */
  level?: MessageLevel
  /** Per-call tags merged on top of any global scope tags. */
  tags?: Tags
  /** Per-call user override. Otherwise picks up `setUser(...)` global. */
  user?: null | User
  /** Free-form payload attached to the event. */
  data?: Record<string, unknown>
  /** Optional explicit breadcrumb list. If omitted, the current
   *  ring-buffer snapshot is sealed and attached. */
  breadcrumbs?: Breadcrumb[]
}
/** v2.0 W3 — `track` joins the breadcrumb type axis. Emitted
 *  automatically by `sentori.track()` when `init.capture.trackAutoBreadcrumb`
 *  is `true`, so a later `captureException` carries the customer
 *  journey leading up to the failure.
 *  v2.26 — `push` joins the type axis. Emitted automatically by
 *  the SDK push receive path when the incoming payload carries
 *  `_sentori.msgId`. Data shape: `{ msgId, title?, body?, opened:
 *  bool, provider }`. Part of Observability link-through (rule #4). */
export type BreadcrumbType = 'custom' | 'log' | 'nav' | 'net' | 'push' | 'track' | 'user'

export type Event = {
  app: App
  /** Phase 42 sub-C.05 / sub-D.02: references to blobs previously
   *  uploaded via `POST /v1/events/<id>/attachments/<kind>`. Server
   *  validates each `ref` matches a row it issued for this event_id;
   *  unknown refs are silently dropped (the rest of the event still
   *  lands). Empty / absent on every event today; sub-D / E / F / G
   *  populate this as native + JS layers ship attachment capture. */
  attachments?: AttachmentMeta[]
  breadcrumbs?: Breadcrumb[]
  device: Device
  environment: string
  /** Present for `kind ∈ {'error','anr','nearCrash'}`. Absent for
   *  `kind: 'message'` — those events carry `level` + `message`
   *  instead and don't have a normalised error / stack. */
  error?: SentoriError
  /** Required when `kind === 'message'`; otherwise undefined. */
  level?: MessageLevel
  /** Required when `kind === 'message'`; otherwise undefined. */
  message?: string
  fingerprint?: string[]
  /** v0.9.0 #10 — OTA bundle the JS was running off. */
  bundle?: Bundle
  /** v0.8.0-d — server-set from a GeoIP lookup on the client's IP.
   *  Clients never set this; the server overwrites any incoming
   *  value before persist. `undefined` when the operator hasn't
   *  configured a db or the IP isn't resolvable (private range). */
  geo?: Geo
  id: string
  kind: EventKind
  platform: Platform
  release: string
  spanId?: null | string
  tags?: Tags
  /** v0.9.0 #13 — feature-flag state at capture time. Distinct
   *  dimension from `tags`: dashboard facets/filters on these as
   *  "experiment was X, variant was Y". Same string→string shape,
   *  separate field on the wire so the dashboard can treat them
   *  differently. */
  flags?: Record<string, string>
  timestamp: string
  traceId?: null | string
  user?: null | User
}

export type Geo = {
  /** ISO 3166-1 alpha-2, uppercase. */
  country: string
  /** ISO 3166-2 subdivision (no country prefix). City-grade db only. */
  region?: string
  /** Localised English city name. City-grade db only. */
  city?: string
}

/**
 * Phase 42 sub-D.02 — wire-format reference to an already-uploaded
 * blob. The SDK uploads the binary first (multipart POST), the
 * server returns a `ref` (UUID it generated), and the SDK echoes
 * the ref back inside the next `event.attachments[]`.
 */
export type AttachmentKind =
  | 'logTail'
  | 'replay'
  | 'screenshot'
  | 'sessionTrail'
  | 'stateSnapshot'
  | 'viewTree'
export type AttachmentSource = 'android' | 'ios' | 'js'

export type AttachmentMeta = {
  /** Server-issued UUID — the only field ingest actually trusts. */
  ref: string
  kind: AttachmentKind
  /** Echoed back so the dashboard can render the right viewer
   *  without a second round-trip. */
  mediaType?: string
  sizeBytes?: number
  source?: AttachmentSource
}

export type Device = {
  /** v1.1 chunk S1 — stable per-install id persisted to device-side
   *  secure storage (Keychain on iOS when react-native-keychain is
   *  installed, AsyncStorage otherwise). Survives app restarts; on
   *  iOS specifically it survives uninstall + reinstall because the
   *  Keychain is independent of the app sandbox. Opaque UUID; not
   *  tied to user identity unless the host also calls `setUser`. */
  installId?: string
  locale?: string
  model?: string
  /** v0.8.0-c — effective connection class at capture time.
   *  Web: `navigator.connection.effectiveType` (Network Information
   *  API, Chrome / Edge / Safari Tech). RN: `@react-native-community/netinfo`
   *  if installed (NetInfo's `details.cellularGeneration` mapped to the
   *  same enum). `undefined` when not available. */
  networkType?: '2g' | '3g' | '4g' | 'offline' | 'slow-2g' | 'unknown' | 'wifi'
  os: DeviceOS
  osVersion: string
}

export type App = {
  build?: string
  framework?: { name: string; version: string }
  version: string
}

/** v0.9.0 #10 — OTA bundle info. Identifies the JS bundle currently
 *  loaded (vs the store binary, which is `app.version` / `release`).
 *  Populated automatically when the host has `expo-updates` or
 *  `react-native-code-push` installed; otherwise absent. */
export type Bundle = {
  /** Stable identifier — Expo updateId or CodePush label/hash. */
  id: string
  /** When the bundle was published. RFC 3339. */
  deployedAt?: string
  /** Which OTA system reported it. */
  source?: 'codepush' | 'expo'
}

/**
 * PII-minimal user shape sent over the wire.
 *
 *   id          — host's internal pseudonym. Stored raw on server.
 *                  Host's call whether this is PII (if so, prefer
 *                  putting the PII identifier under `linkBy`).
 *   name        — display name. Stored raw. Host's call.
 *   anonymous   — boolean flag for "user not logged in."
 *   linkHashes  — **already-hashed** identity values for cross-
 *                  project lookup, computed client-side. NEVER
 *                  contains raw email / phone / sub. Format
 *                  enforced server-side: each value must be a
 *                  64-character lowercase hex sha256.
 *
 * SDKs accept raw identity values via the `User.linkBy` field on
 * their public `setUser({...})` API; the SDK hashes them
 * client-side via `subtle.digest('SHA-256', ...)` BEFORE the value
 * leaves the device, and only the `linkHashes` map ever travels.
 */
export type User = {
  anonymous?: boolean
  id?: string
  name?: string
  linkHashes?: Record<string, string>
}

export type Tags = Record<string, string>

export type SentoriError = {
  cause?: null | SentoriError
  message: string
  stack: Frame[]
  type: string
}

export type Frame = {
  absolutePath?: string
  column?: number
  file: string
  function?: string
  inApp: boolean
  line: number
  /** RN native symbolication may attach surrounding source lines. */
  postContext?: string[]
  preContext?: string[]
}

export type Breadcrumb = {
  data: Record<string, unknown>
  timestamp: string
  type: BreadcrumbType
}

/** Optional context attached at capture time. */
export type CaptureExtras = {
  fingerprint?: string[]
  tags?: Tags
  user?: User
}

/** Phase 34 sub-A: span wire format. See docs/protocol.md#span-schema. */
export type SpanStatus = 'cancelled' | 'error' | 'ok'

export type Span = {
  data?: Record<string, unknown>
  durationMs: number
  id: string
  name: string
  op: string
  parentSpanId: null | string
  startedAt: string
  status: SpanStatus
  tags: Record<string, string>
  /** Original W3C traceparent header value if this span continues a
   *  trace from another process. Optional. */
  traceparent?: string
  traceId: string
}

/** v2.0 W3 — capture-time policy knobs shared across SDKs. Optional
 *  block; absent → all defaults. Defaults are conservative to avoid
 *  changing existing customer breadcrumb shape; new integrations are
 *  encouraged to set `trackAutoBreadcrumb: true`. */
export type CaptureOptions = {
  /** When `true`, every `track(name, props)` call also pushes a
   *  breadcrumb of `{ type: 'track', data: { name, props } }` so the
   *  customer journey shows up on a subsequent `captureException` /
   *  `captureMessage`. Defaults to `false` to preserve v1 behaviour
   *  on upgrade. */
  trackAutoBreadcrumb?: boolean
}

/** Subset of init options that every SDK accepts. SDKs may extend. */
export type CommonInitOptions = {
  /** v2.0 W3 — capture-time policy knobs. See `CaptureOptions`. */
  capture?: CaptureOptions
  /** "prod" / "dev" / "staging" / whatever you want. */
  environment: string
  /** e.g. https://ingest.sentori.golia.jp */
  ingestUrl: string
  /** e.g. "myapp@1.2.3+456" */
  release: string
  /** Public token, format `st_pk_<26 base32 chars>`. */
  token: string
}

/**
 * v2.3 — payload handed to `init({ onReady })` after init completes.
 * Shared across SDKs so a host that switches from web to RN reads the
 * same shape. `native` is optional because non-mobile SDKs never have
 * a native module to bind. `coldStartMs` is also optional — only the
 * RN SDK measures it via the native bridge timing.
 */
export type ReadyInfo = {
  /** npm version of the SDK package that fired this. */
  sdkVersion: string
  /** Milliseconds between cold-start signal and `init()` completion.
   *  Only populated by the RN SDK; undefined elsewhere. */
  coldStartMs?: number
  /** Native module bind status. Present on RN; absent on web / JS. */
  native?: { bound: boolean; methods: string[] }
}

/**
 * v2.3 — host-supplied filter / mutator hook. Called once per event
 * just before transport enqueue. Return the event (possibly mutated)
 * to send it, or `null` to drop it entirely. Synchronous — async
 * pre-send mutation is intentionally not supported (would let the
 * host stall the SDK's hot path).
 *
 * If the hook throws, SDK swallows the error (NEVER rule), emits one
 * one-shot `logger.warn`, and falls back to the un-mutated event.
 * If it returns a non-event (e.g. `undefined`), same treatment.
 *
 * Use for host-side PII scrubbing the SDK can't do automatically
 * (custom field names, application-specific redaction). Server-side
 * privacy_lab still runs even when no beforeSend is configured.
 */
export type BeforeSendHook = (event: Event) => Event | null

/**
 * Phase 44 sub-A — per-event-class client-side sampling. Each rate
 * is in `[0, 1]`; absent / null → 1.0 (keep everything). The
 * **client** drops sampled-out events before they ever leave the
 * device, so a 10w-user app can dial down trace volume by 10x
 * without ingest-side budget changes.
 *
 * `traces` is sampled deterministically over `traceId` so every
 * span in the same trace shares the same decision — you never get
 * the root-span-without-children / half-trace shape.
 *
 * `errors` is sampled uniformly per event (no notion of "session"
 * here); apps that want a session-keyed decision can pre-compute
 * a derived rate per session and feed it through.
 */
export type SamplingConfig = {
  errors?: null | number
  traces?: null | number
  /** Sampling rate for `kind: 'message'` events emitted via
   *  `captureMessage`. Default 1.0 (keep all). Manual messages are
   *  rarer than auto-captured errors / traces, so dropping them by
   *  default would defeat the point. */
  messages?: null | number
}

// v2.8 — Push notification types. Mirror the Sentori-native wire
// shape of `/v1/push/send`. Re-exported by every framework wrapper
// so server-side helpers (`sentori-next`'s `sentoriPush`) share one
// canonical message contract with the dashboard's send dialog and
// the SDK's documentation.

/** Priority hint for the delivery channel.
 *
 *  - `'high'` maps to APNs `apns-priority: 10`, FCM `priority: high`,
 *    Web Push `Urgency: high`. Suitable for user-perceptible alerts
 *    that must wake the device.
 *  - `'normal'` is the default; mapped to APNs `5` / FCM `normal` /
 *    Web Push `normal`. Suitable for background data sync. */
export type PushPriority = 'normal' | 'high'

/** Per-message delivery options. Each field is best-effort —
 *  providers ignore fields they don't understand. The server-side
 *  normalises sounds + badges per platform; you pass the same
 *  options object regardless of who's receiving. */
export type PushOptions = {
  sound?: null | string
  badge?: number
  priority?: PushPriority
  ttl?: number
  mutableContent?: boolean
  contentAvailable?: boolean
  collapseKey?: string
  channelId?: string
  category?: string
}

/** Wire shape for `POST /v1/push/send`. `to` accepts a single
 *  `ipt_*` handle or an array — the server fans the array out into
 *  one queued row per recipient and returns a ticket per row. */
export type PushMessage = {
  to: string | string[]
  title?: string
  body?: string
  data?: Record<string, unknown>
  options?: PushOptions
  /** Idempotency key scoped per project. Two POSTs with the same
   *  (project, idempotencyKey) collapse to one queued send. */
  idempotencyKey?: string
}

/** Lifecycle state of one send. */
export type PushTicketStatus = 'queued' | 'sent' | 'failed'

/** Server response shape — one element per recipient in the send. */
export type PushTicket = {
  id: string
  status: PushTicketStatus
  providerOutcome?: string
  error?: string
  retryCount: number
  createdAt: string
  sentAt?: string
}

/** Wire shape returned by `GET /v1/push/receipts/{id}`. */
export type PushReceipt = {
  ticket: PushTicket
}
