export declare function installFetchInstrumentation(): boolean;
export declare function uninstallFetchInstrumentation(): void;
/**
 * Encode `traceparent` per W3C TraceContext:
 *   00-<32 hex chars: trace-id>-<16 hex chars: parent-id>-01
 *
 * Our internal trace-id is a v7 UUID (32 hex chars total when the
 * dashes are stripped), which fits. Our span-id is also a v7 UUID;
 * the W3C parent-id field is 64 bits / 16 hex, so we truncate to the
 * first 16 hex chars (the high-order bytes — uuid-v7 keeps the
 * timestamp there, which is the most distinguishing prefix).
 *
 * Exported for tests.
 */
export declare function toTraceparent(traceId: string, spanId: string): string;
//# sourceMappingURL=fetch.d.ts.map