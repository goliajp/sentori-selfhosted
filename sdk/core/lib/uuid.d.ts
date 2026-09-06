/**
 * RFC 9562 UUID v7 generator.
 *
 * Layout (48-bit Unix-ms timestamp + 74 random bits, version 7, variant 10):
 *   bytes 0–5  — Unix epoch milliseconds, big-endian
 *   byte  6    — high nibble: version 7 (0x70); low nibble: random
 *   byte  7    — random
 *   byte  8    — high 2 bits: variant 10; low 6 bits: random
 *   bytes 9–15 — random
 *
 * Falls back to `Math.random` when `crypto.getRandomValues` is unavailable
 * (extremely old Hermes / RN; uniqueness is preserved, entropy is not).
 */
export declare function uuidV7(): string;
//# sourceMappingURL=uuid.d.ts.map