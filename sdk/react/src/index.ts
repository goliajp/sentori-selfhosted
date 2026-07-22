export { SentoriProvider } from './SentoriProvider.js'
export { SentoriErrorBoundary } from './SentoriErrorBoundary.js'
export { SentoriSuspense } from './SentoriSuspense.js'
export { TraceRender } from './SentoriTrace.js'
export { useSentori, useCaptureError } from './hooks.js'

export type {
  Breadcrumb,
  BreadcrumbType,
  CaptureExtras,
  SentoriContextValue,
  SentoriReactConfig,
  Tags,
  User,
} from './types.js'

// v2.12 — Push notifications hook + types passthrough.
export {
  useSentoriPush,
  type UseSentoriPushOptions,
  type UseSentoriPushReturn,
} from './usePush.js'
export {
  registerWeb,
  unregisterWeb,
  readCachedIpt,
  type RegisterWebOptions,
  type RegisterWebResult,
} from '@goliapkg/sentori-javascript'
export type {
  PushMessage,
  PushOptions,
  PushPriority,
  PushReceipt,
  PushTicket,
  PushTicketStatus,
} from '@goliapkg/sentori-core'
