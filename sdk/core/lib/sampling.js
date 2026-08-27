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
export function shouldSample(rate) {
    const r = normalizeRate(rate);
    if (r >= 1)
        return true;
    if (r <= 0)
        return false;
    return Math.random() < r;
}
/**
 * Deterministic over `traceId` — same traceId always returns the
 * same decision so all spans in a trace stay together. Hash the
 * first 32 bits of the trace id and compare normalised against
 * `rate`. Falls back to uniform random if the trace id doesn't
 * yield 8+ hex chars.
 */
export function shouldSampleTrace(traceId, rate) {
    const r = normalizeRate(rate);
    if (r >= 1)
        return true;
    if (r <= 0)
        return false;
    if (!traceId)
        return Math.random() < r;
    // UUIDs ship with dashes; strip + take the first 8 hex chars.
    const hex = traceId.replace(/-/g, '').slice(0, 8);
    if (hex.length < 8)
        return Math.random() < r;
    const u32 = parseInt(hex, 16);
    if (!Number.isFinite(u32))
        return Math.random() < r;
    // Map `[0, 0xffffffff]` → `[0, 1)`. Using division by 2^32 keeps
    // the upper bound strict so `u32 = 0xffffffff` ⇒ ~0.999... < 1.0
    // and stays below `rate = 1` correctly.
    const normalised = u32 / 0x100000000;
    return normalised < r;
}
function normalizeRate(rate) {
    if (rate == null || Number.isNaN(rate))
        return 1;
    if (rate < 0)
        return 0;
    if (rate > 1)
        return 1;
    return rate;
}
//# sourceMappingURL=sampling.js.map