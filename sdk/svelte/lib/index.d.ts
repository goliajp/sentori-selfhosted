/**
 * Phase 45 sub-C — Svelte / SvelteKit adapter for Sentori.
 *
 * SvelteKit usage (preferred):
 *
 *     // hooks.client.ts
 *     import { initSentori } from '@goliapkg/sentori-svelte'
 *     initSentori({ token: 'st_pk_...', release: 'myapp@1.0.0' })
 *
 *     export const handleError = sentoriHandleError()
 *
 * What you get:
 *   - `initSentori(opts)` — thin wrapper over the JS SDK init
 *   - `sentoriHandleError()` — returns a SvelteKit `HandleClientError`
 *     hook that captures into Sentori then returns the original
 *     error metadata so SvelteKit's own error page still renders
 *   - `traceNavigation(navigating)` — feed `$navigating` from
 *     `$app/stores` and we open / finish `svelte.navigation` spans
 *     per route transition (one trace per screen, same shape as
 *     the Vue + RN adapters)
 *
 * Vanilla Svelte (no SvelteKit) — call `initSentori` from your app
 * root + use the `<svelte:boundary>` element (Svelte 5+) or
 * `onError` prop on your top-level component.
 */
import { type InitOptions } from '@goliapkg/sentori-javascript';
export type SentoriSvelteOptions = InitOptions;
export declare function initSentori(options: SentoriSvelteOptions): void;
/**
 * SvelteKit `handleError` hook factory. Returns a function with the
 * `HandleClientError` shape (parameters typed loosely so we don't
 * need to import SvelteKit's `@sveltejs/kit` types and force a
 * peer dep).
 */
export declare function sentoriHandleError(): (input: {
    error: unknown;
    event?: unknown;
    status?: number;
    message?: string;
}) => {
    message: string;
};
type NavigatingLike = null | {
    from?: {
        url: {
            pathname: string;
        };
    };
    to?: {
        url: {
            pathname: string;
        };
    };
};
export declare function traceNavigation(navigating: NavigatingLike): void;
export { addBreadcrumb, captureException, captureException as captureError, captureMessage, captureStep, getUser, setUser, } from '@goliapkg/sentori-javascript';
export type { CaptureMessageOptions, MessageLevel, } from '@goliapkg/sentori-javascript';
export { RuntimeMetricBuffer, drainRuntimeMetricsForFlush, emitMetric, flushRuntimeMetrics, rebufferRuntimeMetrics, startRuntimeMetricsTimer, stopRuntimeMetricsTimer, type RuntimeMetricPoint, } from '@goliapkg/sentori-javascript';
export { registerWeb, unregisterWeb, readCachedIpt, type RegisterWebOptions, type RegisterWebResult, } from '@goliapkg/sentori-javascript';
export type { PushMessage, PushOptions, PushPriority, PushReceipt, PushTicket, PushTicketStatus, } from '@goliapkg/sentori-core';
//# sourceMappingURL=index.d.ts.map