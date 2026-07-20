/**
 * Wire process.on('uncaughtException') + 'unhandledRejection'.
 * Idempotent. Returns false if not running on Node (no `process.on`).
 *
 * Node policy notes:
 *   - We do NOT call process.exit on uncaughtException; Sentori doesn't
 *     own the host's crash strategy. The host's existing handler
 *     (default: log + exit 1) runs after ours.
 *   - Bun + Deno expose process.on for compatibility; the same code
 *     path covers them.
 */
export declare function installNodeHooks(): boolean;
export declare function _resetNodeHooksForTesting(): void;
//# sourceMappingURL=node.d.ts.map