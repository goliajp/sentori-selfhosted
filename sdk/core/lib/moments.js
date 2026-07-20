// v0.9.0 #6 — Moments / abandonment.
//
// `startMoment('checkout')` opens a span (op = `sentori.moment`) with
// the moment name + caller-supplied properties as tags. The handle
// exposes `checkpoint` (record intermediate timestamps), `end()` (ok),
// `fail(reason)` (error), `abandon()` (cancelled + `abandoned=true`
// tag for the dashboard funnel view).
//
// Under the hood this is a thin span wrapper. Schema 简: moments are
// not a separate table; they're spans with op=sentori.moment + a
// `moment.name` tag the dashboard indexes on.
import { SpanHandle, startSpan } from './spans.js';
export class MomentHandle {
    span;
    status = 'open';
    checkpoints = [];
    startedAtMs;
    constructor(name, props) {
        const tags = { 'moment.name': name };
        for (const [k, v] of Object.entries(props)) {
            tags[`moment.prop.${k}`] = String(v);
        }
        this.startedAtMs = Date.now();
        this.span = startSpan('sentori.moment', {
            name,
            parent: null,
            startNowMs: this.startedAtMs,
            tags,
        });
    }
    get name() {
        return this.span.name;
    }
    /** Record a named checkpoint within the moment. Cheap, in-memory;
     *  serialised onto the span data field at finish time. */
    checkpoint(label) {
        if (this.status !== 'open')
            return;
        if (typeof label !== 'string' || label.length === 0 || label.length > 100)
            return;
        this.checkpoints.push({ atMs: Date.now() - this.startedAtMs, label });
    }
    /** Successful completion. */
    end() {
        if (this.status !== 'open')
            return;
        this.status = 'ok';
        this.finishWith('ok');
    }
    /** Failed completion — moment ran but didn't reach success. */
    fail(reason) {
        if (this.status !== 'open')
            return;
        this.status = 'failed';
        if (reason)
            this.span.setTag('moment.fail.reason', reason.slice(0, 200));
        this.finishWith('error');
    }
    /** User abandoned (foregrounded → backgrounded for > 30s, or app
     *  closed without `.end()`). Dashboard counts this in abandonment
     *  rate. */
    abandon() {
        if (this.status !== 'open')
            return;
        this.status = 'abandoned';
        this.span.setTag('moment.abandoned', 'true');
        this.finishWith('cancelled');
    }
    /** Internal — finalize the span with the right status + ship
     *  checkpoint timestamps as data. */
    finishWith(status) {
        if (this.checkpoints.length > 0) {
            this.span.setData('moment.checkpoints', this.checkpoints);
        }
        this.span.finish({ status });
    }
    /** Test-only. */
    __getStatus() {
        return this.status;
    }
}
export function startMoment(name, opts) {
    return new MomentHandle(name, opts?.properties ?? {});
}
//# sourceMappingURL=moments.js.map