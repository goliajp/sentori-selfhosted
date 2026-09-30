// @goliapkg/sentori-core — the platform-agnostic heart of the v1 SDK.
//
// What lives here: the wire types (single source of truth), the
// signal ring, error coercion, the iron-rule primitives (safe
// wrappers + the self-report circuit breaker), stack parsing, id
// minting, identity hashing. Transport and the 8-verb binding live
// in the per-platform SDKs.

export type {
  App,
  AssertStat,
  AttachmentKind,
  AttachmentSource,
  BatchEnvelope,
  BatchResponse,
  Device,
  EventData,
  EventKind,
  Frame,
  IngestOutcome,
  InitConfig,
  Platform,
  SentoriApi,
  SentoriError,
  Signal,
  Surface,
  TraceOptions,
  User,
  WireEvent,
  WirePayload,
} from './types.js'

export { coerceError } from './coerce-error.js'

export {
  clearSignals,
  configureRing,
  pushSignal,
  snapshotSignals,
} from './signal-ring.js'

export { parseStack, type ParseStackOptions } from './stack.js'

export { normalizeUrl } from './url.js'

export {
  type SessionContext,
  type SessionPing,
  type SessionStatus,
  SessionTracker,
} from './session.js'

export { shouldSample, shouldSampleTrace } from './sampling.js'

export { uuidV7 } from './uuid.js'

export {
  computeReplayDelta,
  indexReplayNodes,
  replayNodeId,
  ReplayRing,
  type ReplayDelta,
  type ReplayEntry,
  type ReplayFrame,
  type ReplayKeyframe,
  type ReplayNode,
  type ReplayRingOptions,
} from './replay-ring.js'

export {
  __resetPlatformDegradeForTests,
  degradePlatform,
  PLATFORM_FALLBACK,
  platformOrFallback,
  refusalIsAboutPlatform,
  type Outcome,
} from './platform-degrade.js'

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

export {
  createTransport,
  PENDING_STORAGE_KEY,
  type Transport,
  type TransportConfig,
  type TransportHost,
  type TransportStorage,
} from './transport.js'

export {
  applyBeforeSend,
  buildWireEvent,
  serializeData,
  toSentoriError,
  type WireEventInputs,
} from './wire-event.js'

export { sha256Hex, utf8Bytes } from './sha256.js'
