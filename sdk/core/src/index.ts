export type {
  App,
  AttachmentKind,
  AttachmentMeta,
  AttachmentSource,
  BeforeSendHook,
  Breadcrumb,
  BreadcrumbType,
  Bundle,
  CaptureExtras,
  CommonInitOptions,
  Device,
  DeviceOS,
  CaptureMessageOptions,
  Event,
  EventKind,
  Frame,
  Geo,
  MessageLevel,
  Platform,
  ReadyInfo,
  SamplingConfig,
  SentoriError,
  Span,
  SpanStatus,
  Tags,
  User,
  // v2.8 — Push notification wire types.
  PushMessage,
  PushOptions,
  PushPriority,
  PushReceipt,
  PushTicket,
  PushTicketStatus,
} from './types.js'

export { coerceError } from './coerce-error.js'

export {
  MomentHandle,
  type MomentProperties,
  type MomentStatus,
  startMoment,
} from './moments.js'

export { shouldSample, shouldSampleTrace } from './sampling.js'

export { uuidV7 } from './uuid.js'

export {
  BreadcrumbBuffer,
  addBreadcrumb,
  clearBreadcrumbs,
  getBreadcrumbs,
} from './breadcrumbs.js'

export { parseStack, type ParseStackOptions } from './stack.js'

export { normalizeUrl } from './url.js'

export {
  type SessionContext,
  type SessionPing,
  type SessionStatus,
  SessionTracker,
} from './session.js'

export {
  SpanBuffer,
  SpanHandle,
  type SpanContextLike,
  type StartSpanOptions,
  clearSpans,
  drainSpans,
  getSpans,
  startSpan,
  startTrace,
  withScopedSpan,
  // v2.3 — `withSpan` is now the unified entry point per design §2.3.
  // Overloaded: `withSpan(name, fn)` = high-level wrap helper
  // (equivalent to `withScopedSpan`); `withSpan(span, fn)` = low-level
  // active-span manager (equivalent to `withActiveSpan`).
  withSpan,
} from './spans.js'

export {
  __resetTraceContextForTests,
  __useFallbackTraceContextForTests,
  activeSpan,
  setActiveSpan,
  // v2.3 — renamed from `withSpan` (which now dispatches in spans.ts).
  // `withActiveSpan(span, fn)` is the explicit name for the
  // low-level active-context manager.
  withActiveSpan,
} from './trace-context.js'

export {
  TrailBuffer,
  sealTrail,
  type SessionTrailPayload,
  type TrailStep,
} from './trail.js'

export { safeAsync, safeFn } from './safe.js'

export {
  __resetCircuitForTests,
  isCircuitOpen,
  reportInternal,
  setInternalReporter,
} from './self-report.js'

export {
  getLogLevel,
  type LogLevel,
  logger,
  type LogTransport,
  setLogLevel,
  setLogTransport,
} from './logger.js'

export { hashIdentities, type LinkBy } from './identity.js'

/** v2.1 W2 — runtime metrics ring + emit API. Storage primitive
 *  only — transport (POST /v1/runtime-metrics:batch) lives in
 *  the per-platform SDK. Auto-instrument modules (FPS / heap /
 *  cold-start / route-nav / network) push via `emitMetric`; the
 *  per-SDK flusher drains via `drainRuntimeMetricsForFlush()`
 *  on its 30 s tick, coalesced with the existing event flush. */
export {
  RuntimeMetricBuffer,
  __peekRuntimeMetricsSize,
  __resetRuntimeMetricsForTests,
  drainRuntimeMetricsForFlush,
  emitMetric,
  rebufferRuntimeMetrics,
  type RuntimeMetricPoint,
} from './runtime-metrics.js'
