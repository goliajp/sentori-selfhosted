// v2.1 W2 — runtime metrics ring buffer + emit API.
//
// Storage primitive only. core never owns transport — the
// per-platform SDK (RN, javascript, etc.) is responsible for
// flushing this buffer to /v1/runtime-metrics:batch on its own
// 30 s cadence, coalesced with the existing event flush so the
// host app pays one round-trip instead of two.
//
// Auto-instrument modules (FPS / heap / cold-start / route-nav /
// network — landed in W2 part 2) call `emitMetric(name, value,
// tags?)` directly. Hosts that want to push custom auto-style
// points (rare; the recommended path is the v0.8.3
// `recordMetric` channel) can do the same.
//
// NEVER rule: emit + drain are both wrapped in `safeFn` /
// `safeAsync` at the per-SDK boundary — internal failures here
// never throw to the host. We additionally cap the ring at 10k
// so a runaway auto-instrument can't blow host memory; overflow
// pushes are silently dropped with a `reportInternal` breadcrumb.
import { reportInternal } from './self-report.js';
const RING_CAP = 10_000;
const MAX_NAME_LEN = 200;
const MAX_TAG_KEYS = 16;
const MAX_TAG_KEY_LEN = 40;
const MAX_TAG_VALUE_LEN = 200;
/** Buffer abstraction so per-instance SDK setups can have their
 *  own ring (e.g. multi-org test fixtures). Most callsites use
 *  the module-scoped global below. */
export class RuntimeMetricBuffer {
    cap;
    buf = [];
    dropped = 0;
    constructor(cap = RING_CAP) {
        this.cap = cap;
    }
    push(point) {
        if (this.buf.length >= this.cap) {
            this.dropped += 1;
            return;
        }
        this.buf.push(point);
    }
    /** Drain the buffer atomically. Caller owns the returned array;
     *  if the network post fails the caller can rebuffer via
     *  `pushBatch`. */
    drain() {
        const out = this.buf;
        this.buf = [];
        return out;
    }
    /** Rebuffer dropped points after a failed flush. Bounded by cap
     *  — older overflow stays dropped. */
    pushBatch(points) {
        for (const p of points) {
            this.push(p);
        }
    }
    size() {
        return this.buf.length;
    }
    /** Total points dropped due to ring overflow since process start.
     *  Surfaced once per drain via `reportInternal` so the operator
     *  sees sustained overflow as an SDK self-report instead of
     *  silent loss. */
    takeDroppedCount() {
        const n = this.dropped;
        this.dropped = 0;
        return n;
    }
    clear() {
        this.buf = [];
        this.dropped = 0;
    }
}
const _global = new RuntimeMetricBuffer();
function validatePoint(name, value, tags) {
    if (typeof name !== 'string' || name.length === 0 || name.length > MAX_NAME_LEN) {
        return false;
    }
    if (typeof value !== 'number' || !Number.isFinite(value)) {
        return false;
    }
    if (tags) {
        const keys = Object.keys(tags);
        if (keys.length > MAX_TAG_KEYS)
            return false;
        for (const k of keys) {
            if (k.length === 0 || k.length > MAX_TAG_KEY_LEN)
                return false;
            const v = tags[k];
            if (typeof v !== 'string' || v.length > MAX_TAG_VALUE_LEN)
                return false;
        }
    }
    return true;
}
/** Emit one runtime metric point into the module-scoped buffer.
 *  Auto-instrument hooks (FPS / heap / route-nav / …) call this
 *  every tick; hosts almost never call this directly — use
 *  `sentori.recordMetric` for custom business metrics, which goes
 *  through the v0.8.3 /v1/metrics:batch channel with looser
 *  validation.
 *
 *  Silent on malformed input — internal validation failures don't
 *  throw, just drop the point. Per the NEVER rule. */
export function emitMetric(name, value, tags) {
    if (!validatePoint(name, value, tags))
        return;
    _global.push({
        name,
        value,
        tags,
        ts: new Date().toISOString(),
    });
}
/** Snapshot helper for the per-SDK flusher: drain everything,
 *  surface any overflow count, return the points to POST.
 *
 *  Pattern in the per-SDK transport:
 *
 *      const batch = drainRuntimeMetricsForFlush()
 *      if (batch.length === 0) return
 *      try {
 *        await post('/v1/runtime-metrics:batch', { metrics: batch })
 *      } catch (e) {
 *        // rebuffer; next flush will retry
 *        rebufferRuntimeMetrics(batch)
 *        reportInternal('runtime-metrics.flush', e)
 *      }
 */
export function drainRuntimeMetricsForFlush() {
    const overflow = _global.takeDroppedCount();
    if (overflow > 0) {
        reportInternal('runtime-metrics.ring_overflow', { dropped: overflow });
    }
    return _global.drain();
}
/** Rebuffer points returned by a failed flush. Bounded by ring
 *  cap — under sustained outage the oldest points are dropped to
 *  preserve memory. */
export function rebufferRuntimeMetrics(points) {
    _global.pushBatch(points);
}
/** Test-only escape hatch for vitest / bun:test teardown. */
export function __resetRuntimeMetricsForTests() {
    _global.clear();
}
/** Test-only peek at current ring depth without draining. */
export function __peekRuntimeMetricsSize() {
    return _global.size();
}
//# sourceMappingURL=runtime-metrics.js.map