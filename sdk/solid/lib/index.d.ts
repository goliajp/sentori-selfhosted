/**
 * Phase 45 sub-D — SolidJS adapter for Sentori.
 *
 *     import { initSentori, captureException } from '@goliapkg/sentori-solid'
 *
 *     initSentori({ token: 'st_pk_...', release: 'myapp@1.0.0' })
 *
 * What's in this package vs `@goliapkg/sentori-javascript`:
 *
 *   - `initSentori` / `captureException` re-exported as-is (no
 *     Solid-specific transform needed; Solid uses ES2020 modules
 *     and the JS SDK is framework-agnostic)
 *   - `wrapWithBoundary(props)` — adapter that hands an error
 *     callback to Solid's built-in `<ErrorBoundary>` so anything
 *     thrown in render lands in Sentori. Apps import Solid's
 *     `<ErrorBoundary>` directly; this is just the `onCatch`
 *     callback they pass.
 *   - `traceSolidRouter(routerLocation)` — pass `useLocation()`
 *     from `@solidjs/router` and we open a `solid.navigation` span
 *     per route. SDK consumers wire it inside a `createEffect`.
 *
 * SolidJS is a small enough framework that we deliberately don't
 * ship a giant API. Most users will just need `initSentori` +
 * `captureException`.
 */
import { type InitOptions } from '@goliapkg/sentori-javascript';
export type SentoriSolidOptions = InitOptions;
export declare function initSentori(options: SentoriSolidOptions): void;
/**
 * Callback to wire into Solid's built-in `<ErrorBoundary onCatch={...}>`.
 *
 *     import { ErrorBoundary } from 'solid-js'
 *     import { sentoriOnCatch } from '@goliapkg/sentori-solid'
 *
 *     <ErrorBoundary fallback={...} onCatch={sentoriOnCatch}>
 *       <App />
 *     </ErrorBoundary>
 *
 * (Solid's `<ErrorBoundary>` exposes the onCatch hook via
 * `solidjs.ErrorBoundary`; the callback fires on `Error` thrown in
 * render / lifecycle and is the SolidJS-idiomatic place to forward
 * into a monitoring service.)
 */
export declare function sentoriOnCatch(err: unknown): void;
export declare function traceSolidRouter(pathname: string): void;
export { addBreadcrumb, captureException, captureException as captureError, captureMessage, captureStep, getUser, setUser, } from '@goliapkg/sentori-javascript';
export type { CaptureMessageOptions, MessageLevel, } from '@goliapkg/sentori-javascript';
export { RuntimeMetricBuffer, drainRuntimeMetricsForFlush, emitMetric, flushRuntimeMetrics, rebufferRuntimeMetrics, startRuntimeMetricsTimer, stopRuntimeMetricsTimer, type RuntimeMetricPoint, } from '@goliapkg/sentori-javascript';
export { registerWeb, unregisterWeb, readCachedIpt, type RegisterWebOptions, type RegisterWebResult, } from '@goliapkg/sentori-javascript';
export type { PushMessage, PushOptions, PushPriority, PushReceipt, PushTicket, PushTicketStatus, } from '@goliapkg/sentori-core';
//# sourceMappingURL=index.d.ts.map