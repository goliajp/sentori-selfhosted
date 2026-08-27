export type SessionStatus = 'crashed' | 'errored' | 'exited' | 'ok';
export type SessionPing = {
    durationMs: number;
    environment: string;
    id: string;
    release: string;
    startedAt: string;
    status: SessionStatus;
    userId: null | string;
};
export type SessionContext = {
    environment: string;
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