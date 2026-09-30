/**
 * Phase 26 sub-A/B: session tracker.
 *
 * One in-flight session at a time. The platform SDK calls:
 *   - `start(...)` when the app foregrounds / page loads
 *   - `markErrored()` from captureException
 *   - `markCrashed()` from a native crash hook
 *   - `end()` when the app backgrounds / page unloads
 *
 * The tracker holds the in-progress state; the *transport* it sends to
 * is supplied by the platform (so JS SDK uses fetch/sendBeacon, RN SDK
 * uses fetch over the Hermes/JSC bridge, etc.). Status promotion is
 * monotonic — once `crashed` is set it can't be downgraded by a later
 * `markErrored()`.
 *
 * Re-entrancy: `start()` while a session is active drops the previous
 * one without sending — that lifecycle is owned by the platform's
 * foreground/background plumbing and dual-active never makes sense.
 */
import type { Platform } from './types.js';
export type SessionStatus = 'crashed' | 'errored' | 'exited' | 'ok';
export type SessionPing = {
    durationMs: number;
    environment: string;
    id: string;
    /** Which runtime the session ran in, so a crash-free rate can be
     *  read per platform rather than only per release. */
    platform: Platform;
    release: string;
    startedAt: string;
    status: SessionStatus;
    /** @deprecated Never populated by any shipped SDK, and a raw id has
     *  no business on the wire. Kept so a server that reads it does not
     *  break; `userKey` is the field. */
    userId: null | string;
    /** The same salted hash events carry as `userKey`, so the crash-free
     *  user count and an issue's breadth are over one population. While
     *  this was missing the two numbers were not comparable, and the
     *  crash-free-users rate had no input at all: `userId` was hard-wired
     *  to null at start and nothing ever filled it in. */
    userKey?: null | string;
};
export type SessionContext = {
    environment: string;
    platform: Platform;
    release: string;
    userId: null | string;
};
type Active = {
    ctx: SessionContext;
    id: string;
    startedAtMs: number;
    status: SessionStatus;
};
export declare class SessionTracker {
    private readonly send;
    private readonly now;
    private active;
    constructor(send: (ping: SessionPing) => void, now?: () => number);
    start(ctx: SessionContext): void;
    /** Captured a non-fatal error during this session. */
    markErrored(): void;
    /** Process is going down for the count. */
    markCrashed(): void;
    /** Ship the ping. `finalStatus` overrides the accumulated state if given (e.g. `'exited'` for explicit shutdown). */
    end(finalStatus?: SessionStatus): void;
    /** Convenience: is there a session in flight? */
    isActive(): boolean;
    /** For tests / introspection only. */
    peek(): Active | null;
}
export {};
//# sourceMappingURL=session.d.ts.map