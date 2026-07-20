/**
 * Wire-format types now live in `@goliapkg/sentori-core`. This file is
 * kept as a thin re-export so existing relative imports inside the
 * package continue to work. JS-specific extras (the `InitOptions`
 * shape with `enableGlobalHooks`) are declared here.
 */

export type {
  App,
  Breadcrumb,
  BreadcrumbType,
  CaptureExtras,
  Device,
  DeviceOS,
  Event,
  EventKind,
  Frame,
  Platform,
  ReadyInfo,
  SentoriError,
  Tags,
  User,
} from '@goliapkg/sentori-core'

import type {
  BeforeSendHook,
  CommonInitOptions,
  LogLevel,
  ReadyInfo,
} from '@goliapkg/sentori-core'

export type InitOptions = CommonInitOptions & {
  /** Override automatic global hooks. Default: true on browser + node. */
  enableGlobalHooks?: boolean
  /** Phase 44 sub-B — client-side sampling rates `[0, 1]`. Absent /
   *  null → 1.0 (keep everything). `traces` is deterministic over
   *  traceId so all spans of a trace share the same decision.
   *  v2.3 back-compat alias for `sample`; both shapes accepted. */
  sampling?: {
    errors?: null | number
    traces?: null | number
    /** v2.0 — sampling rate for `kind: 'message'` events emitted via
     *  `captureMessage`. Default 1.0 (keep all). */
    messages?: null | number
  }
  /** v2.3 — canonical sampling field (renamed from `sampling`). Same
   *  shape; if both are passed, `sample` wins. The older `sampling`
   *  field stays accepted indefinitely as a back-compat alias. */
  sample?: {
    errors?: null | number
    traces?: null | number
    messages?: null | number
  }
  /** Phase 46 — opt in to recording a session-trail buffer that
   *  uploads alongside the next `captureException`. */
  capture?: {
    sessionTrail?: boolean
    /** v2.1 W2 — start the 30 s runtime-metrics flusher. Off by
     *  default in JS because the auto-instrument modules (FPS /
     *  heap / network bytes) are RN-only in 2.1.0; web hosts that
     *  want to push metrics today can flip this on and call
     *  `emitMetric()` directly. The transport pipe is identical
     *  to RN's so the dashboard treats both sources uniformly.
     *  Defaults to `false`. */
    runtimeMetrics?: boolean
  }
  /** v2.3 — Sentori SDK's own console output gate. Default `'warn'`:
   *  SDK is silent unless something is genuinely broken. Set
   *  `'silent'` for absolute silence; `'info'` / `'debug'` when
   *  debugging Sentori itself. Shared `LogLevel` type with the RN
   *  SDK. */
  logLevel?: LogLevel
  /** v2.3 — fires once after init completes. Use this to know the
   *  SDK is live instead of scanning the console. JS SDK does not
   *  carry `coldStartMs` / `native` — those are RN-only fields and
   *  remain undefined here. */
  onReady?: (info: ReadyInfo) => void
  /** v2.3 — host-side mutate-or-drop hook (sync). See
   *  `BeforeSendHook` for the contract. */
  beforeSend?: BeforeSendHook
}
