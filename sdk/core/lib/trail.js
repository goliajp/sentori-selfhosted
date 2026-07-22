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
const DEFAULT_MAX_STEPS = 30;
/**
 * Bounded FIFO of trail steps. Push is O(1); drain returns a copy
 * so the caller can serialise without worrying about concurrent
 * mutation (the SDK is single-threaded but we don't want hidden
 * aliasing bugs).
 */
export class TrailBuffer {
    buffer = [];
    max;
    constructor(maxSteps = DEFAULT_MAX_STEPS) {
        this.max = Math.max(1, Math.floor(maxSteps));
    }
    push(step) {
        this.buffer.push(step);
        if (this.buffer.length > this.max) {
            this.buffer.splice(0, this.buffer.length - this.max);
        }
    }
    /** Snapshot the current buffer without mutating it. */
    snapshot() {
        return this.buffer.slice();
    }
    clear() {
        this.buffer.length = 0;
    }
    size() {
        return this.buffer.length;
    }
}
export function sealTrail(buffer) {
    return {
        sealedAt: new Date().toISOString(),
        steps: buffer.snapshot(),
    };
}
//# sourceMappingURL=trail.js.map