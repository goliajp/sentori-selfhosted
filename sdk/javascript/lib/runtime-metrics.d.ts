/** Atomic drain + POST + rebuffer-on-failure. Per the NEVER rule:
 *  never throws, never rejects. */
export declare function flushRuntimeMetrics(): Promise<void>;
/** Idempotent start of the 30 s flush timer. Called from init()
 *  when `runtimeMetrics: true` is set. */
export declare function startRuntimeMetricsTimer(): void;
/** Stop the periodic flush. Idempotent. Used by tests + by hosts
 *  that want to opt out mid-session. */
export declare function stopRuntimeMetricsTimer(): void;
//# sourceMappingURL=runtime-metrics.d.ts.map