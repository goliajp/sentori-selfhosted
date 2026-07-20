/**
 * Phase 46 — Session-trail ring buffer.
 *
 * A trail is a ring buffer of the last N "steps" leading up to a
 * crash. Each step is a lightweight snapshot:
 *
 *   { ts, label, breadcrumbRef?, viewTreeRef?, screenshotRef? }
 *
 * Refs are server-issued attachment UUIDs (already in
 * `AttachmentMeta.ref` shape), so the trail itself stays tiny — a
 * 30-step trail without screenshots is ~1 KB of JSON. Screenshots
 * and view trees go through the existing multipart attachment
 * upload path; this buffer only carries pointers.
 *
 * Trail is **client-side only** until `captureException` fires, at
 * which point the buffer is serialised and uploaded as an attachment
 * of kind `sessionTrail`. The buffer auto-evicts oldest entries
 * past `maxSteps`.
 *
 * Privacy: trail steps are opt-in (`init({ capture: { sessionTrail:
 * true } })`); the buffer never auto-attaches screenshots unless the
 * caller passes a `screenshotRef`. MaskRegion (Phase 42 sub-D) and
 * the existing screenshot privacy controls are reused for any
 * screenshots a step does carry.
 */

export type TrailStep = {
  /** Unix ms timestamp. */
  ts: number
  /** Short human-readable label for the step (route name, action). */
  label: string
  /** Optional pointer into the breadcrumb buffer (already on event).
   *  `data` is for small structured payloads (e.g. screen dwell ms,
   *  feature-flag values at the step time). Server caps it at 4 KB. */
  breadcrumb?: { type: string; message: string; data?: Record<string, unknown> }
  /** Optional viewTree attachment ref (uploaded separately). */
  viewTreeRef?: string
  /** Optional screenshot attachment ref (uploaded separately). */
  screenshotRef?: string
}

const DEFAULT_MAX_STEPS = 30

/**
 * Bounded FIFO of trail steps. Push is O(1); drain returns a copy
 * so the caller can serialise without worrying about concurrent
 * mutation (the SDK is single-threaded but we don't want hidden
 * aliasing bugs).
 */
export class TrailBuffer {
  private buffer: TrailStep[] = []
  private readonly max: number

  constructor(maxSteps: number = DEFAULT_MAX_STEPS) {
    this.max = Math.max(1, Math.floor(maxSteps))
  }

  push(step: TrailStep): void {
    this.buffer.push(step)
    if (this.buffer.length > this.max) {
      this.buffer.splice(0, this.buffer.length - this.max)
    }
  }

  /** Snapshot the current buffer without mutating it. */
  snapshot(): TrailStep[] {
    return this.buffer.slice()
  }

  clear(): void {
    this.buffer.length = 0
  }

  size(): number {
    return this.buffer.length
  }
}

/**
 * The serialised payload shape we upload as a `sessionTrail`
 * attachment. Keeping it flat + camelCase to match the rest of the
 * Sentori wire protocol.
 */
export type SessionTrailPayload = {
  /** ISO 8601 timestamp when the trail was sealed. */
  sealedAt: string
  /** Steps oldest → newest. */
  steps: TrailStep[]
}

export function sealTrail(buffer: TrailBuffer): SessionTrailPayload {
  return {
    sealedAt: new Date().toISOString(),
    steps: buffer.snapshot(),
  }
}
