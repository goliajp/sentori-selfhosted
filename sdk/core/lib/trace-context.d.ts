import type { SpanContextLike } from './spans.js';
/** Currently active span context, or null. Falls back across the
 *  fallback impl's save-and-restore boundary. */
export declare function activeSpan(): SpanContextLike | null;
/**
 * Run `fn` with `span` as the active span. Use this to wrap any unit
 * of work whose child spans should attribute up to this one:
 *
 *     const span = startSpan('handler.GET')
 *     try {
 *       return await withSpan(span, async () => {
 *         // any startSpan() in here picks up `span` as parent
 *         return await loadUser()
 *       })
 *     } finally {
 *       span.finish({ status: 'ok' })
 *     }
 *
 * Node: routed through AsyncLocalStorage, so awaits inside `fn`
 * preserve the active span.
 *
 * Browser/RN: save-and-restore. Correct for linear awaits;
 * concurrent promises forked inside `fn` won't see the active span
 * after the first await suspends.
 *
 * v2.3 — exported as `withActiveSpan` (clear semantic name). The
 * old export name `withSpan` is re-exported through `spans.ts` as
 * an overloaded function that dispatches by first-arg type
 * (string → high-level wrap helper; SpanContextLike → this
 * function). New code should call `withSpan(name, fn)`.
 */
export declare function withActiveSpan<T>(span: SpanContextLike, fn: () => T): T;
/**
 * Set (or clear, with `null`) the active span outside of a `withSpan`
 * scope. For long-lived contexts where a `fn` wrapper doesn't fit —
 * specifically screen navigation: `useTraceNavigation` opens a
 * `react.navigation` span when a screen is entered and leaves it
 * active for that screen's lifetime, so the screen's `http.client`
 * spans become children (one trace per screen instead of one per
 * request).
 *
 * Browser/RN only in practice — no-op on the Node/AsyncLocalStorage
 * impl (ALS can't "set and leave" cleanly). Don't reach for this in
 * async server code; `withSpan` is the scoped tool there.
 */
export declare function setActiveSpan(span: SpanContextLike | null): void;
/** Reset the implementation choice — test-only. Production code never
 *  calls this; switching propagation strategy at runtime would mean
 *  losing the current active context. */
export declare function __resetTraceContextForTests(): void;
/** Test-only: force the module-variable fallback impl regardless of
 *  environment. `bun test` runs as Node (so the ALS impl is picked),
 *  but navigation — the one feature that relies on `setActiveSpan` —
 *  only runs on browser/RN, where the fallback is in effect. Tests of
 *  that path call this so they exercise the impl that actually ships
 *  there. */
export declare function __useFallbackTraceContextForTests(): void;
//# sourceMappingURL=trace-context.d.ts.map