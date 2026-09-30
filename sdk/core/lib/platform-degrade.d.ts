import type { Platform } from './types';
/** Accepted by every server this product has ever shipped. */
export declare const PLATFORM_FALLBACK: Platform;
/** One entry of a batch response's `outcomes` array. */
export type Outcome = {
    error?: string;
    detail?: string;
};
/**
 * Whether these outcomes say the platform was the problem.
 *
 * Matched on the detail text rather than a code because the servers
 * this exists for are already released: they answer
 * `invalid_payload` with `detail: "platform must be ..."` and will
 * never gain a more specific code. Requiring `invalid_payload` as
 * well keeps an unrelated message that happens to contain the word
 * from degrading a session for nothing.
 */
export declare function refusalIsAboutPlatform(outcomes: readonly Outcome[] | undefined): boolean;
/**
 * Drop this session to {@link PLATFORM_FALLBACK}.
 *
 * Session-scoped and one-way: the next launch tries the real value
 * again, because the operator may have upgraded in between, and
 * nothing that happens inside one run tells us they did.
 *
 * Warns once. A line per batch would be the SDK shouting about a
 * condition it has already handled, which is how a host team decides
 * the thing in their console is the problem.
 */
export declare function degradePlatform(warn: (message: string) => void): void;
/** The platform to put on the wire: the real one, unless degraded. */
export declare function platformOrFallback(actual: Platform): Platform;
export declare function __resetPlatformDegradeForTests(): void;
//# sourceMappingURL=platform-degrade.d.ts.map