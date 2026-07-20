import type { Span, SpanStatus } from './types.js';
/** Hint passed to `startSpan`. `parent` overrides whatever
 *  `activeSpan()` would resolve to; `traceId` overrides both. */
export type StartSpanOptions = {
    data?: Record<string, unknown>;
    name?: string;
    parent?: null | SpanContextLike;
    tags?: Record<string, string>;
    /** Wall-clock for testing; defaults to `Date.now()`. */
    startNowMs?: number;
    /** Force the trace id (used when continuing a distributed trace
     *  from a `traceparent` header). When both `parent` and `traceId`
     *  are provided, `traceId` wins. */
    traceId?: string;
    traceparent?: string;
};
/** Anything that has the two id fields we care about — covers
 *  `SpanHandle`, decoded `traceparent`, and naked literal objects. */
export type SpanContextLike = {
    spanId: string;
    traceId: string;
};
/** Returned from `startSpan`. Mutable; sealed by `end()` (canonical
 *  in v2) or `finish()` (v1.x name, preserved). */
export declare class SpanHandle {
    readonly spanId: string;
    readonly traceId: string;
    readonly parentSpanId: null | string;
    readonly op: string;
    readonly startedAt: string;
    readonly traceparent: string | undefined;
    private name;
    private readonly tags;
    private data;
    private readonly startNowMs;
    private finished;
    /** v2.0 — stashed by setStatus(), applied at end()/finish() time. */
    private pendingStatus;
    constructor(op: string, opts?: StartSpanOptions);
    setName(name: string): this;
    setTag(key: string, value: string): this;
    setData(key: string, value: unknown): this;
    /**
     * v2.0 — set a single attribute (key/value) on the span. Stringifies
     * non-string values for wire-shape simplicity. Sentry / OTel parity.
     * Backed by the same `tags` map setTag() writes to.
     */
    setAttribute(key: string, value: unknown): this;
    /**
     * v2.0 — bulk attribute setter. Each value goes through the same
     * String() coercion as setAttribute.
     */
    setAttributes(record: Record<string, unknown>): this;
    /**
     * v2.0 — set the span status without ending the span. The status
     * (and optional message stashed under `tags['status.message']`) is
     * applied at the next end()/finish() call. Sentry / OTel parity.
     */
    setStatus(code: SpanStatus, message?: string): this;
    /**
     * v2.0 — attach an exception to this span without ending it. Stored
     * under `data.exception` for the dashboard span-detail view to
     * render alongside other span context. Sentry / OTel parity.
     */
    recordException(err: Error): this;
    /**
     * v2.0 — true while the span has not been ended/finished yet. Cheap
     * to call from a render hook. OTel parity.
     */
    isRecording(): boolean;
    /**
     * v2.0 — canonical name for sealing the span. Equivalent to
     * `finish()`; both stay first-class for backwards compatibility.
     */
    end(opts?: {
        endNowMs?: number;
        status?: SpanStatus;
        tags?: Record<string, string>;
    }, buffer?: SpanBuffer): Span | null;
    isFinished(): boolean;
    /**
     * Seal the span and push it onto `buffer`. Second + later calls are
     * a no-op (returning the already-sealed result is harder than it
     * sounds because we don't keep the Span around — easier to just
     * forbid double finish).
     */
    finish(opts?: {
        endNowMs?: number;
        status?: SpanStatus;
        tags?: Record<string, string>;
    }, buffer?: SpanBuffer): Span | null;
}
export declare class SpanBuffer {
    private readonly cap;
    private readonly items;
    constructor(cap?: number);
    push(span: Span): void;
    snapshot(): Span[];
    drain(): Span[];
    clear(): void;
    get size(): number;
}
/**
 * Open a span. When no `parent` or `traceId` is provided, this
 * inherits from the current active span (see `trace-context.ts`); if
 * there is none either, a fresh trace is rooted with `parentSpanId =
 * null`.
 */
export declare function startSpan(op: string, opts?: StartSpanOptions): SpanHandle;
/**
 * v2.0 — open a NEW top-level trace. Equivalent to
 * `startSpan(name, { parent: null, ...opts })` with the root span
 * auto-tagged `source: 'manual'` so the dashboard Traces module
 * can distinguish manual-rooted traces from auto-instrumented.
 *
 * Use when the entry point of a workflow isn't covered by
 * auto-instrumentation (CLI command, worker tick, deliberately
 * detached background task).
 *
 *     const trace = sentori.startTrace('checkout-flow')
 *     // ... work ...
 *     trace.end({ status: 'ok' })
 *
 * The first positional argument doubles as `name` AND `op` for
 * Sentori convention — manual traces are usually named after the
 * workflow ("checkout-flow", "nightly-cron-tick") so reusing the
 * string for both keeps the API tight.
 */
export declare function startTrace(name: string, opts?: Omit<StartSpanOptions, 'parent' | 'name'>): SpanHandle;
/**
 * v2.0 — scoped span. Opens a span, runs the callback, ends the
 * span when the callback resolves (or throws). Returns the
 * callback's return value (awaited if async). Sentry / OTel
 * ergonomics.
 *
 *     const result = await sentori.withScopedSpan('db.query users', async (s) => {
 *       return await db.query(…)
 *     })
 *
 * - sync fn: span ends after fn returns; status `'ok'` on success,
 *   `'error'` on throw (the exception is `recordException`-d).
 * - async fn: span ends on promise resolution; same status mapping.
 *
 * Distinct from the lower-level `withSpan(span, fn)` in
 * `trace-context.ts` (which pushes a pre-existing span onto the
 * active-context stack). This one creates + auto-finishes.
 */
export declare function withScopedSpan<T>(op: string, fn: (span: SpanHandle) => T, opts?: StartSpanOptions): T;
/**
 * v2.3 — unified `withSpan` entry point per design §2.3. Dispatches
 * by first-argument type:
 *
 *   `withSpan(name: string, fn)`  → high-level wrap helper.
 *                                   Opens a span, runs fn, ends
 *                                   the span. Same as
 *                                   `withScopedSpan(name, fn)`.
 *
 *   `withSpan(span: SpanContextLike, fn)` → low-level active-span
 *                                   manager. Pushes the span onto
 *                                   the active-context stack for the
 *                                   duration of fn so child spans
 *                                   inherit it as parent. Same as
 *                                   `withActiveSpan(span, fn)`.
 *
 * `withScopedSpan` + `withActiveSpan` remain exported for callers
 * who want the explicit name; both compile to the same runtime.
 */
export declare function withSpan<T>(span: SpanContextLike, fn: () => T): T;
export declare function withSpan<T>(name: string, fn: (span: SpanHandle) => T, opts?: StartSpanOptions): T;
/** Snapshot the global buffer (does not drain). */
export declare function getSpans(): Span[];
/** Take everything out of the global buffer (used by transport flush). */
export declare function drainSpans(): Span[];
export declare function clearSpans(): void;
//# sourceMappingURL=spans.d.ts.map