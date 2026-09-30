import { uuidV7 } from './uuid.js';
const RANK = { crashed: 3, errored: 2, exited: 1, ok: 0 };
export class SessionTracker {
    send;
    now;
    active = null;
    constructor(send, now = () => Date.now()) {
        this.send = send;
        this.now = now;
    }
    start(ctx) {
        this.active = {
            ctx,
            id: uuidV7(),
            startedAtMs: this.now(),
            status: 'ok',
        };
    }
    /** Captured a non-fatal error during this session. */
    markErrored() {
        if (!this.active)
            return;
        if (RANK[this.active.status] < RANK.errored)
            this.active.status = 'errored';
    }
    /** Process is going down for the count. */
    markCrashed() {
        if (!this.active)
            return;
        if (RANK[this.active.status] < RANK.crashed)
            this.active.status = 'crashed';
    }
    /** Ship the ping. `finalStatus` overrides the accumulated state if given (e.g. `'exited'` for explicit shutdown). */
    end(finalStatus) {
        if (!this.active)
            return;
        const status = finalStatus
            ? RANK[finalStatus] >= RANK[this.active.status]
                ? finalStatus
                : this.active.status
            : this.active.status;
        const startedAt = new Date(this.active.startedAtMs).toISOString();
        const durationMs = Math.max(0, this.now() - this.active.startedAtMs);
        const ping = {
            durationMs,
            environment: this.active.ctx.environment,
            id: this.active.id,
            platform: this.active.ctx.platform,
            release: this.active.ctx.release,
            startedAt,
            status,
            userId: this.active.ctx.userId,
        };
        this.active = null;
        try {
            this.send(ping);
        }
        catch {
            // Transport failures are best-effort; we've already cleared the
            // session so we don't double-send if the host calls end() again.
        }
    }
    /** Convenience: is there a session in flight? */
    isActive() {
        return this.active !== null;
    }
    /** For tests / introspection only. */
    peek() {
        return this.active ? { ...this.active } : null;
    }
}
//# sourceMappingURL=session.js.map