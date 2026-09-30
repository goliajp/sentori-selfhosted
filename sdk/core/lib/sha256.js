/**
 * SHA-256 in plain JavaScript, for runtimes that have no WebCrypto.
 *
 * WeChat mini programs have no `crypto.subtle`. Without this,
 * `hashIdentities` throws there, `setUser` swallows it, and every
 * event from a mini program carries no `userKey` — so breadth reads
 * zero, push reaches nobody, and nothing anywhere says why.
 *
 * It exists only as a fallback. Where WebCrypto is present it is used,
 * because it is native code and this is not. What matters is that both
 * paths produce the same 64 characters for the same input: a person
 * who signs in on the web and in a mini program must be one person,
 * and `gen-identity-vectors --check` holds the two to the same
 * fixtures.
 *
 * FIPS 180-4. The constants are the first 32 bits of the fractional
 * parts of the square roots (h) and cube roots (k) of the first
 * primes; they are transcribed, not derived, so they are worth
 * checking against the standard rather than reading.
 */
const K = new Uint32Array([
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
]);
/** UTF-8 bytes, without `TextEncoder` — a mini program has that, but
 *  a shim runtime may not, and the encoding has to be the same one
 *  WebCrypto was handed or the digests differ for any non-ASCII
 *  input. Surrogate pairs are combined; a lone surrogate becomes
 *  U+FFFD, which is what `TextEncoder` does. */
export function utf8Bytes(str) {
    const out = [];
    for (let i = 0; i < str.length; i += 1) {
        let cp = str.charCodeAt(i);
        if (cp >= 0xd800 && cp <= 0xdbff) {
            const next = str.charCodeAt(i + 1);
            if (next >= 0xdc00 && next <= 0xdfff) {
                cp = (cp - 0xd800) * 0x400 + (next - 0xdc00) + 0x10000;
                i += 1;
            }
            else {
                cp = 0xfffd;
            }
        }
        else if (cp >= 0xdc00 && cp <= 0xdfff) {
            cp = 0xfffd;
        }
        if (cp < 0x80)
            out.push(cp);
        else if (cp < 0x800)
            out.push(0xc0 | (cp >> 6), 0x80 | (cp & 0x3f));
        else if (cp < 0x10000)
            out.push(0xe0 | (cp >> 12), 0x80 | ((cp >> 6) & 0x3f), 0x80 | (cp & 0x3f));
        else {
            out.push(0xf0 | (cp >> 18), 0x80 | ((cp >> 12) & 0x3f), 0x80 | ((cp >> 6) & 0x3f), 0x80 | (cp & 0x3f));
        }
    }
    return new Uint8Array(out);
}
const rotr = (x, n) => (x >>> n) | (x << (32 - n));
/** The digest as 64 lowercase hex characters. */
export function sha256Hex(input) {
    const msg = utf8Bytes(input);
    const bitLen = msg.length * 8;
    // Pad to a multiple of 64 bytes: 0x80, zeros, then the length as a
    // big-endian 64-bit count of bits.
    const withPadding = new Uint8Array(((msg.length + 9 + 63) >> 6) << 6);
    withPadding.set(msg);
    withPadding[msg.length] = 0x80;
    // JavaScript numbers hold 53 bits exactly, so the high word is
    // written from the float rather than by shifting — `<< 32` is a
    // no-op on a 32-bit shift and would silently drop it.
    const view = new DataView(withPadding.buffer);
    view.setUint32(withPadding.length - 8, Math.floor(bitLen / 0x100000000));
    view.setUint32(withPadding.length - 4, bitLen >>> 0);
    const h = new Uint32Array([
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ]);
    const w = new Uint32Array(64);
    for (let offset = 0; offset < withPadding.length; offset += 64) {
        for (let i = 0; i < 16; i += 1)
            w[i] = view.getUint32(offset + i * 4);
        for (let i = 16; i < 64; i += 1) {
            const a = w[i - 15];
            const b = w[i - 2];
            const s0 = rotr(a, 7) ^ rotr(a, 18) ^ (a >>> 3);
            const s1 = rotr(b, 17) ^ rotr(b, 19) ^ (b >>> 10);
            w[i] = (w[i - 16] + s0 + w[i - 7] + s1) >>> 0;
        }
        let [a, b, c, d, e, f, g, hh] = [h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]];
        for (let i = 0; i < 64; i += 1) {
            const S1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25);
            const ch = (e & f) ^ (~e & g);
            const temp1 = (hh + S1 + ch + K[i] + w[i]) >>> 0;
            const S0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22);
            const maj = (a & b) ^ (a & c) ^ (b & c);
            const temp2 = (S0 + maj) >>> 0;
            hh = g;
            g = f;
            f = e;
            e = (d + temp1) >>> 0;
            d = c;
            c = b;
            b = a;
            a = (temp1 + temp2) >>> 0;
        }
        h[0] = (h[0] + a) >>> 0;
        h[1] = (h[1] + b) >>> 0;
        h[2] = (h[2] + c) >>> 0;
        h[3] = (h[3] + d) >>> 0;
        h[4] = (h[4] + e) >>> 0;
        h[5] = (h[5] + f) >>> 0;
        h[6] = (h[6] + g) >>> 0;
        h[7] = (h[7] + hh) >>> 0;
    }
    let out = '';
    for (let i = 0; i < 8; i += 1)
        out += h[i].toString(16).padStart(8, '0');
    return out;
}
//# sourceMappingURL=sha256.js.map