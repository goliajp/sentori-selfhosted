// Phase 35 sub-A: client-side span buffer + lifecycle.
//
// Mirrors the breadcrumb buffer pattern: bounded ring, module-scoped
// default, opt-in fresh instance for SDKs that need per-process
// isolation. Callers don't push pre-built spans; they call
// `startSpan()` to get a mutable handle, mutate as work happens, then
// `finish()` — that's the moment the span is sealed and pushed onto
// the buffer. The SDK's transport flushes whatever's in the buffer
// at its own cadence.
import { withActiveSpan as withActiveSpanImpl } from './trace-context.js';
import { uuidV7 } from './uuid.js';
const DEFAULT_CAP = 1000;
/** Returned from `startSpan`. Mutable; sealed by `end()` (canonical
 *  in v2) or `finish()` (v1.x name, preserved). */
export class SpanHandle {
    spanId;
    traceId;
    parentSpanId;
    op;
    startedAt;
    traceparent;
    name;
    tags;
    data;
    startNowMs;
    finished = false;
    /** v2.0 — stashed by setStatus(), applied at end()/finish() time. */
    pendingStatus;
    constructor(op, opts = {}) {
        this.op = op;
        this.name = opts.name ?? op;
        this.tags = { ...(opts.tags ?? {}) };
        this.data = opts.data;
        this.traceparent = opts.traceparent;
        const parent = opts.parent;
        this.traceId = opts.traceId ?? parent?.traceId ?? uuidV7();
        this.parentSpanId = parent ? parent.spanId : null;
        this.spanId = uuidV7();
        this.startNowMs = opts.startNowMs ?? Date.now();
        this.startedAt = new Date(this.startNowMs).toISOString();
    }
    setName(name) {
        this.name = name;
        return this;
    }
    setTag(key, value) {
        this.tags[key] = value;
        return this;
    }
    setData(key, value) {
        if (!this.data)
            this.data = {};
        this.data[key] = value;
        return this;
    }
    // --- v2.0 — Sentry / OTel-aligned methods ---
    /**
     * v2.0 — set a single attribute (key/value) on the span. Stringifies
     * non-string values for wire-shape simplicity. Sentry / OTel parity.
     * Backed by the same `tags` map setTag() writes to.
     */
    setAttribute(key, value) {
        this.tags[key] = typeof value === 'string' ? value : String(value);
        return this;
    }
    /**
     * v2.0 — bulk attribute setter. Each value goes through the same
     * String() coercion as setAttribute.
     */
    setAttributes(record) {
        for (const [k, v] of Object.entries(record)) {
            this.tags[k] = typeof v === 'string' ? v : String(v);
        }
        return this;
    }
    /**
     * v2.0 — set the span status without ending the span. The status
     * (and optional message stashed under `tags['status.message']`) is
     * applied at the next end()/finish() call. Sentry / OTel parity.
     */
    setStatus(code, message) {
        this.pendingStatus = code;
        if (message !== undefined) {
            this.tags['status.message'] = message;
        }
        return this;
    }
    /**
     * v2.0 — attach an exception to this span without ending it. Stored
     * under `data.exception` for the dashboard span-detail view to
     * render alongside other span context. Sentry / OTel parity.
     */
    recordException(err) {
        if (!this.data)
            this.data = {};
        this.data.exception = {
            type: err.name,
            message: err.message,
            stack: typeof err.stack === 'string' ? err.stack : undefined,
        };
        return this;
    }
    /**
     * v2.0 — true while the span has not been ended/finished yet. Cheap
     * to call from a render hook. OTel parity.
     */
    isRecording() {
        return !this.finished;
    }
    /**
     * v2.0 — canonical name for sealing the span. Equivalent to
     * `finish()`; both stay first-class for backwards compatibility.
     */
    end(opts = {}, buffer = _global) {
        return this.finish(opts, buffer);
    }
    isFinished() {
        return this.finished;
    }
    /**
     * Seal the span and push it onto `buffer`. Second + later calls are
     * a no-op (returning the already-sealed result is harder than it
     * sounds because we don't keep the Span around — easier to just
     * forbid double finish).
     */
    finish(opts = {}, buffer = _global) {
        if (this.finished)
            return null;
        this.finished = true;
        if (opts.tags)
            Object.assign(this.tags, opts.tags);
        const endMs = opts.endNowMs ?? Date.now();
        const durationMs = Math.max(0, endMs - this.startNowMs);
        // Status precedence: per-call opts > setStatus() > default 'ok'.
        // setStatus()'s pendingStatus is honoured when end()/finish() is
        // called without an opts.status, matching Sentry / OTel ergonomics.
        const status = opts.status ?? this.pendingStatus ?? 'ok';
        const span = {
            data: this.data,
            durationMs,
            id: this.spanId,
            name: this.name,
            op: this.op,
            parentSpanId: this.parentSpanId,
            startedAt: this.startedAt,
            status,
            tags: { ...this.tags },
            traceId: this.traceId,
            ...(this.traceparent ? { traceparent: this.traceparent } : {}),
        };
        buffer.push(span);
        return span;
    }
}
export class SpanBuffer {
    cap;
    items = [];
    constructor(cap = DEFAULT_CAP) {
        this.cap = cap;
    }
    push(span) {
        this.items.push(span);
        while (this.items.length > this.cap) {
            this.items.shift();
        }
    }
    snapshot() {
        return this.items.slice();
    }
    drain() {
        const out = this.items.slice();
        this.items.length = 0;
        return out;
    }
    clear() {
        this.items.length = 0;
    }
    get size() {
        return this.items.length;
    }
}
const _global = new SpanBuffer();
/**
 * Open a span. When no `parent` or `traceId` is provided, this
 * inherits from the current active span (see `trace-context.ts`); if
 * there is none either, a fresh trace is rooted with `parentSpanId =
 * null`.
 */
export function startSpan(op, opts = {}) {
    const resolved = opts.parent === undefined ? activeSpan() : opts.parent;
    return new SpanHandle(op, { ...opts, parent: resolved });
}
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
export function startTrace(name, opts = {}) {
    return new SpanHandle(name, {
        ...opts,
        name,
        parent: null,
        tags: { ...(opts.tags ?? {}), source: 'manual' },
    });
}
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
export function withScopedSpan(op, fn, opts = {}) {
    const span = startSpan(op, opts);
    try {
        const result = fn(span);
        if (result instanceof Promise) {
            return result.then((v) => {
                span.end({ status: 'ok' });
                return v;
            }, (e) => {
                if (e instanceof Error)
                    span.recordException(e);
                span.end({ status: 'error' });
                throw e;
            });
        }
        span.end({ status: 'ok' });
        return result;
    }
    catch (e) {
        if (e instanceof Error)
            span.recordException(e);
        span.end({ status: 'error' });
        throw e;
    }
}
export function withSpan(arg, fn, opts = {}) {
    if (typeof arg === 'string') {
        return withScopedSpan(arg, fn, opts);
    }
    return withActiveSpanImpl(arg, fn);
}
/** Snapshot the global buffer (does not drain). */
export function getSpans() {
    return _global.snapshot();
}
/** Take everything out of the global buffer (used by transport flush). */
export function drainSpans() {
    return _global.drain();
}
export function clearSpans() {
    _global.clear();
}
// Trace context is imported lazily to avoid a circular module load —
// trace-context.ts itself imports SpanHandle.
import { activeSpan } from './trace-context.js';
//# sourceMappingURL=spans.js.map