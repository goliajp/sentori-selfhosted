/**
 * Phase 44 sub-A — sampling primitives.
 *
 * Two surfaces:
 *
 *   shouldSample(rate)          — uniform random per call
 *   shouldSampleTrace(traceId, rate)  — deterministic over traceId
 *
 * Determinism for the trace path matters: every span in a trace
 * needs the same yes/no answer or you end up with broken half-
 * traces on the server. We hash the first 8 hex chars of the
 * traceId to a `u32`, normalise to `[0, 1)`, and compare.
 *
 * Rate semantics:
 *   - `null`, `undefined`, or NaN → 1.0 (keep everything)
 *   - clamped to `[0, 1]`
 *   - 0.0 always rejects, 1.0 always keeps
 */
/** Keep with probability `rate`. Uniform random per call. */
export declare function shouldSample(rate: null | number | undefined): boolean;
/**
 * Deterministic over `traceId` — same traceId always returns the
 * same decision so all spans in a trace stay together. Hash the
 * first 32 bits of the trace id and compare normalised against
 * `rate`. Falls back to uniform random if the trace id doesn't
 * yield 8+ hex chars.
 */
export declare function shouldSampleTrace(traceId: null | string | undefined, rate: null | number | undefined): boolean;
//# sourceMappingURL=sampling.d.ts.map