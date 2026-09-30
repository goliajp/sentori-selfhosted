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
export function uuidV7() {
    const ts = Date.now();
    const buf = new Uint8Array(16);
    buf[0] = Math.floor(ts / 0x10000000000) & 0xff;
    buf[1] = Math.floor(ts / 0x100000000) & 0xff;
    buf[2] = Math.floor(ts / 0x1000000) & 0xff;
    buf[3] = Math.floor(ts / 0x10000) & 0xff;
    buf[4] = Math.floor(ts / 0x100) & 0xff;
    buf[5] = ts & 0xff;
    fillRandom(buf.subarray(6));
    buf[6] = (buf[6] & 0x0f) | 0x70;
    buf[8] = (buf[8] & 0x3f) | 0x80;
    const hex = Array.from(buf, (b) => b.toString(16).padStart(2, '0')).join('');
    return (hex.slice(0, 8) +
        '-' +
        hex.slice(8, 12) +
        '-' +
        hex.slice(12, 16) +
        '-' +
        hex.slice(16, 20) +
        '-' +
        hex.slice(20, 32));
}
function fillRandom(buf) {
    const c = globalThis.crypto;
    if (c?.getRandomValues) {
        c.getRandomValues(buf);
        return;
    }
    for (let i = 0; i < buf.length; i++) {
        buf[i] = Math.floor(Math.random() * 256);
    }
}
//# sourceMappingURL=uuid.js.map