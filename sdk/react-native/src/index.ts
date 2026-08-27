// @goliapkg/sentori-react-native — the 8-verb surface (design.md §4).
//
//   sentori.init(config)   sentori.user(u)      sentori.context(patch)
//   sentori.error(err)     sentori.warn(name)   sentori.trace(name)
//   sentori.assert(n, ok)  sentori.probe(ref)
//
// Everything is synchronous, returns immediately, and can never
// throw into the host app. Learn eight words and you know the whole
// product. The only additions beyond the verbs: ErrorBoundary (React
// idiom for the error verb) and the push namespace (carried).

import { init } from './init';
import { patchContext, setUser } from './scope';
import { verbs } from './verbs';

import type {
  EventData,
  InitConfig,
  TraceOptions,
  User,
} from '@goliapkg/sentori-core';

export const sentori = {
  /** Configure and start the SDK. One call, at app start. */
  init: (config: InitConfig): void => {
    init(config);
  },
  /** Set (or clear, with null) the current user. Drives the
   *  breadth × depth importance stats via a salted hash — raw
   *  identity never leaves the device. */
  user: (u: User | null): void => {
    setUser(u);
  },
  /** Merge ambient context (feature flags, AB variants, tags)
   *  attached to every subsequent event. */
  context: (patch: Record<string, unknown>): void => {
    patchContext(patch);
  },

  /** 出了什么事 — report a caught-but-fatal-worthy error. Unhandled
   *  errors are captured automatically; this is the manual lane. */
  error: (err: unknown, data?: EventData): string => verbs.error(err, data),
  /** 用户哪里不舒服 — a hand-written sub-health report. */
  warn: (name: string, data?: EventData): string => verbs.warn(name, data),
  /** 这里发生了什么 — a neutral observation point. Also lands in the
   *  signal ring; `{ quiet: true }` keeps it ring-only. */
  trace: (name: string, data?: EventData, opts?: TraceOptions): string =>
    verbs.trace(name, data, opts),
  /** 这里应该成立吗 — a production assertion. Failure reports and
   *  NEVER halts the program (unlike a language-level assert);
   *  passes aggregate into a liveness ledger. */
  assert: (name: string, ok: boolean, data?: EventData): string =>
    verbs.assert(name, ok, data),
  /** 那个 bug 回来了吗 — a regression tripwire. Reaching this call is
   *  the signal; it never changes control flow. Plant it in the
   *  branch that used to break. */
  probe: (ref: string, data?: EventData): string => verbs.probe(ref, data),
};

export default sentori;

export { init } from './init';
export { ErrorBoundary } from './error-boundary';
export { RageTapCapture } from './rage-tap';
// Privacy: register a query returning nativeIDs that must be
// blacked out in every captured frame (visual replay, screenshots).
export { registerMaskQuery } from './mask';
export { launch } from './launch';
// Dev-panel helper: crash the native layer on purpose to exercise
// the pending-crash upload pipeline. Not part of the 8-verb surface.
export { triggerNativeCrash } from './native';
export { type NavigationRefLike, useTraceNavigation } from './navigation';

// Wire + config types for typed hosts.
export type {
  AssertStat,
  Device,
  EventData,
  EventKind,
  Frame,
  InitConfig,
  SentoriApi,
  SentoriError,
  Signal,
  Surface,
  TraceOptions,
  User,
  WireEvent,
  WirePayload,
} from '@goliapkg/sentori-core';

// Logger gate, for hosts debugging Sentori itself.
export {
  getLogLevel,
  type LogLevel,
  setLogLevel,
  type LogTransport,
  setLogTransport,
} from '@goliapkg/sentori-core';

// The signal channel, for hosts feeding their own breadcrumbs into
// the ring (insight round-4 §4). Same O(1) fire-and-forget the
// auto signals use; entries ride payload.signals when an error/warn
// fires and appear on the case timeline + user-events panel.
//
// Field conventions the dashboard understands:
//   pushSignal('http', { method, url, status, ms })  — a request's
//     outcome (host's own interceptor is the right producer; the
//     SDK deliberately does not monkey-patch fetch/XHR)
//   any other kind renders with its data as `k=v` pairs.
export { pushSignal } from '@goliapkg/sentori-core';

// Push namespace — carried as-is; outside the v1 acceptance surface.
export * as push from './push';
