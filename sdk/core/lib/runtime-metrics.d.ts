/** One metric point. `ts` is ISO 8601 UTC; SDK transport adds it
 *  if absent. */
export type RuntimeMetricPoint = {
    name: string;
    value: number;
    tags?: Record<string, string>;
    ts: string;
};
/** Buffer abstraction so per-instance SDK setups can have their
 *  own ring (e.g. multi-org test fixtures). Most callsites use
 *  the module-scoped global below. */
export declare class RuntimeMetricBuffer {
    private readonly cap;
    private buf;
    private dropped;
    constructor(cap?: number);
    push(point: RuntimeMetricPoint): void;
    /** Drain the buffer atomically. Caller owns the returned array;
     *  if the network post fails the caller can rebuffer via
     *  `pushBatch`. */
    drain(): RuntimeMetricPoint[];
    /** Rebuffer dropped points after a failed flush. Bounded by cap
     *  — older overflow stays dropped. */
    pushBatch(points: RuntimeMetricPoint[]): void;
    size(): number;
    /** Total points dropped due to ring overflow since process start.
     *  Surfaced once per drain via `reportInternal` so the operator
     *  sees sustained overflow as an SDK self-report instead of
     *  silent loss. */
    takeDroppedCount(): number;
    clear(): void;
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
export declare function emitMetric(name: string, value: number, tags?: Record<string, string>): void;
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
export declare function drainRuntimeMetricsForFlush(): RuntimeMetricPoint[];
/** Rebuffer points returned by a failed flush. Bounded by ring
 *  cap — under sustained outage the oldest points are dropped to
 *  preserve memory. */
export declare function rebufferRuntimeMetrics(points: RuntimeMetricPoint[]): void;
/** Test-only escape hatch for vitest / bun:test teardown. */
export declare function __resetRuntimeMetricsForTests(): void;
/** Test-only peek at current ring depth without draining. */
export declare function __peekRuntimeMetricsSize(): number;
//# sourceMappingURL=runtime-metrics.d.ts.map