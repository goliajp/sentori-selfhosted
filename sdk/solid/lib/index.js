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
import { coerceError, setActiveSpan, startSpan } from '@goliapkg/sentori-core';
import { captureException as captureExceptionJs, captureStep, initSentori as initSentoriJs, } from '@goliapkg/sentori-javascript';
export function initSentori(options) {
    initSentoriJs(options);
}
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
export function sentoriOnCatch(err) {
    // `coerceError` JSON-stringifies plain-object throws (`throw {code:
    // 'auth/expired'}`) so the dashboard shows the real payload instead
    // of `[object Object]`. See @goliapkg/sentori-core/coerce-error.
    const e = coerceError(err);
    captureExceptionJs(e);
}
/**
 * Trace navigation by calling this from a `createEffect` whenever
 * `useLocation().pathname` changes:
 *
 *     import { useLocation } from '@solidjs/router'
 *     import { createEffect } from 'solid-js'
 *     import { traceSolidRouter } from '@goliapkg/sentori-solid'
 *
 *     const loc = useLocation()
 *     createEffect(() => traceSolidRouter(loc.pathname))
 */
let _active = null;
let _lastPath = null;
export function traceSolidRouter(pathname) {
    if (pathname === _lastPath)
        return;
    if (_active) {
        _active.finish({ status: 'ok' });
        _active = null;
    }
    const from = _lastPath ?? '/';
    const span = startSpan('solid.navigation', {
        name: `${from} → ${pathname}`,
        parent: null,
        tags: { 'nav.from': from, 'nav.to': pathname },
    });
    _active = span;
    setActiveSpan(span);
    captureStep(`route:${pathname}`, {
        breadcrumb: { type: 'navigation', message: `${from} → ${pathname}` },
    });
    _lastPath = pathname;
}
export { addBreadcrumb, captureException, captureException as captureError, captureMessage, captureStep, getUser, setUser, } from '@goliapkg/sentori-javascript';
// v2.1 W2 — runtime metrics surface. Off by default; opt in
// via `initSentori({ capture: { runtimeMetrics: true } })`.
export { RuntimeMetricBuffer, drainRuntimeMetricsForFlush, emitMetric, flushRuntimeMetrics, rebufferRuntimeMetrics, startRuntimeMetricsTimer, stopRuntimeMetricsTimer, } from '@goliapkg/sentori-javascript';
// v2.12 — Push notifications passthrough. Solid hosts wrap
// `registerWeb` in a `createResource` / `createSignal` chain
// idiomatically; we ship the primitives + types.
export { registerWeb, unregisterWeb, readCachedIpt, } from '@goliapkg/sentori-javascript';
//# sourceMappingURL=index.js.map