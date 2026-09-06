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
import { uuidV7 } from './uuid.js'

export type SessionStatus = 'crashed' | 'errored' | 'exited' | 'ok'

export type SessionPing = {
  durationMs: number
  environment: string
  id: string
  release: string
  startedAt: string
  status: SessionStatus
  userId: null | string
}

export type SessionContext = {
  environment: string
  release: string
  userId: null | string
}

type Active = {
  ctx: SessionContext
  id: string
  startedAtMs: number
  status: SessionStatus
}

const RANK: Record<SessionStatus, number> = { crashed: 3, errored: 2, exited: 1, ok: 0 }

export class SessionTracker {
  private active: Active | null = null

  constructor(
    private readonly send: (ping: SessionPing) => void,
    private readonly now: () => number = () => Date.now()
  ) {}

  start(ctx: SessionContext): void {
    this.active = {
      ctx,
      id: uuidV7(),
      startedAtMs: this.now(),
      status: 'ok',
    }
  }

  /** Captured a non-fatal error during this session. */
  markErrored(): void {
    if (!this.active) return
    if (RANK[this.active.status] < RANK.errored) this.active.status = 'errored'
  }

  /** Process is going down for the count. */
  markCrashed(): void {
    if (!this.active) return
    if (RANK[this.active.status] < RANK.crashed) this.active.status = 'crashed'
  }

  /** Ship the ping. `finalStatus` overrides the accumulated state if given (e.g. `'exited'` for explicit shutdown). */
  end(finalStatus?: SessionStatus): void {
    if (!this.active) return
    const status = finalStatus
      ? RANK[finalStatus] >= RANK[this.active.status]
        ? finalStatus
        : this.active.status
      : this.active.status
    const startedAt = new Date(this.active.startedAtMs).toISOString()
    const durationMs = Math.max(0, this.now() - this.active.startedAtMs)
    const ping: SessionPing = {
      durationMs,
      environment: this.active.ctx.environment,
      id: this.active.id,
      release: this.active.ctx.release,
      startedAt,
      status,
      userId: this.active.ctx.userId,
    }
    this.active = null
    try {
      this.send(ping)
    } catch {
      // Transport failures are best-effort; we've already cleared the
      // session so we don't double-send if the host calls end() again.
    }
  }

  /** Convenience: is there a session in flight? */
  isActive(): boolean {
    return this.active !== null
  }

  /** For tests / introspection only. */
  peek(): Active | null {
    return this.active ? { ...this.active } : null
  }
}
