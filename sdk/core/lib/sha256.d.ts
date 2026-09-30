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
/** UTF-8 bytes, without `TextEncoder` — a mini program has that, but
 *  a shim runtime may not, and the encoding has to be the same one
 *  WebCrypto was handed or the digests differ for any non-ASCII
 *  input. Surrogate pairs are combined; a lone surrogate becomes
 *  U+FFFD, which is what `TextEncoder` does. */
export declare function utf8Bytes(str: string): Uint8Array;
/** The digest as 64 lowercase hex characters. */
export declare function sha256Hex(input: string): string;
//# sourceMappingURL=sha256.d.ts.map