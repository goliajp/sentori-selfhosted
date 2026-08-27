/**
 * v2.3 — client-side identity hashing for cross-project user lookup.
 *
 * Host call:
 *   sentori.setUser({ id: 'usr_123', linkBy: { email: 'a@b.com' } })
 *
 * The SDK:
 *   1. Normalises each `linkBy` value per well-known type (email
 *      lowercase+trim, phone digit-only+'+' prefix, etc).
 *   2. Hashes the normalised value via `crypto.subtle.digest('SHA-256',
 *      …)`. Output = 64-char lowercase hex.
 *   3. Discards the raw value. Only the `linkHashes` map is stored
 *      on scope and travels over the wire.
 *
 * Server then layers a per-scope salt on top (`sha256(scope.salt ||
 * key_type || ':' || client_hash)`) before writing the
 * `identity_fingerprints` denorm row. Raw values never reach the
 * server.
 *
 * Privacy contract: this module is the **single source of truth**
 * for what gets hashed and how. Any future identity-related code
 * must route through `hashIdentities` so the contract holds.
 */
/** Map of (key_type → raw_value) accepted at the public SDK
 *  `setUser` API. Common types have well-known normalisation; the
 *  index signature lets host apps add custom keys. */
export type LinkBy = {
    email?: string;
    phone?: string;
    googleSub?: string;
    appleSub?: string;
    metaSub?: string;
    username?: string;
} & Record<string, string | undefined>;
/**
 * Hash every entry in a LinkBy bag concurrently. Returns the
 * `linkHashes` record ready to attach to the User wire payload.
 *
 * Failures (e.g. crypto.subtle unavailable) propagate to the caller
 * so `setUser` can decide what to do (most paths swallow via safeFn
 * per the NEVER rule, ending up with no linkHashes — better than
 * sending raw).
 */
export declare function hashIdentities(linkBy: LinkBy): Promise<Record<string, string>>;
//# sourceMappingURL=identity.d.ts.map