import { drainSpans, logger, } from '@goliapkg/sentori-core';
import { getConfig } from './config.js';
const SDK_HEADER = 'sentori-javascript/0.1.0';
export async function send(cfg, event) {
    await postJson(cfg, '/v1/events', JSON.stringify(event));
}
/**
 * Phase 26 sub-B: session ping. Same beacon → fetch fallback as `send`,
 * because sessions almost always close on the same path that closes
 * the tab — beacon survives that, fetch with `keepalive: true` is the
 * fallback when beacon is unavailable.
 */
export async function sendSession(cfg, ping) {
    await postJson(cfg, '/v1/sessions', JSON.stringify(ping));
}
// ── span flush ─────────────────────────────────────────────────────
//
// http.client / react.render / navigation spans pile up in the core
// SpanBuffer as work happens; this drains them on a timer and POSTs to
// /v1/spans:batch (server caps a batch at 200). Spans are best-effort:
// no retry, no offline queue — a dropped span just doesn't show in the
// waterfall.
const SPAN_FLUSH_INTERVAL_MS = 5_000;
const SPAN_BATCH_MAX = 200;
let _spanTimer = null;
export function startSpanFlush() {
    if (_spanTimer)
        return;
    _spanTimer = setInterval(() => {
        void flushSpans();
    }, SPAN_FLUSH_INTERVAL_MS);
    _spanTimer.unref?.();
}
export function stopSpanFlush() {
    if (_spanTimer)
        clearInterval(_spanTimer);
    _spanTimer = null;
}
export async function flushSpans() {
    const cfg = getConfig();
    if (!cfg)
        return;
    const spans = drainSpans();
    if (spans.length === 0)
        return;
    const base = cfg.ingestUrl.replace(/\/+$/, '');
    for (let i = 0; i < spans.length; i += SPAN_BATCH_MAX) {
        const chunk = spans.slice(i, i + SPAN_BATCH_MAX);
        try {
            const resp = await fetch(`${base}/v1/spans:batch`, {
                body: JSON.stringify({ spans: chunk }),
                headers: {
                    Authorization: `Bearer ${cfg.token}`,
                    'Content-Type': 'application/json',
                    'Sentori-Sdk': SDK_HEADER,
                },
                keepalive: true,
                method: 'POST',
            });
            // 5xx: server's struggling — stop sending the rest of the batch.
            // 4xx: drop too (bad token / quota / oversized) — also stop.
            if (resp.status >= 400)
                break;
        }
        catch {
            break;
        }
    }
}
/**
 * Phase 46 — upload an attachment blob (used for `sessionTrail` and
 * any future per-event JSON blobs the Web SDK wants to ship).
 *
 * Mirrors the RN SDK's `uploadAttachment` shape but uses the browser's
 * native `Blob` instead of base64-roundtripping. Returns the server-
 * issued ref UUID on success, `null` on any failure (we ship the rest
 * of the event regardless).
 */
export async function uploadAttachment(cfg, eventId, kind, blob) {
    const base = cfg.ingestUrl.replace(/\/+$/, '');
    const url = `${base}/v1/events/${encodeURIComponent(eventId)}/attachments/${encodeURIComponent(kind)}`;
    const form = new FormData();
    form.append('file', new Blob([blob.body], { type: blob.mediaType }), `${kind}.json`);
    form.append('source', 'js');
    try {
        const resp = await fetch(url, {
            body: form,
            headers: {
                Authorization: `Bearer ${cfg.token}`,
                'Sentori-Sdk': SDK_HEADER,
            },
            method: 'POST',
        });
        // Phase 48 sub-A — accept any 2xx (reverse proxies sometimes
        // rewrite 201 → 202). Body must still parse as UploadResponse.
        if (resp.status < 200 || resp.status >= 300)
            return null;
        const j = (await resp.json().catch(() => null));
        if (!j || !j.refId)
            return null;
        return { kind, mediaType: j.mediaType, ref: j.refId, sizeBytes: j.sizeBytes, source: 'js' };
    }
    catch {
        return null;
    }
}
async function postJson(cfg, path, body) {
    const url = `${cfg.ingestUrl.replace(/\/+$/, '')}${path}`;
    const headers = {
        Authorization: `Bearer ${cfg.token}`,
        'Content-Type': 'application/json',
        'Sentori-Sdk': SDK_HEADER,
    };
    // Browser: navigator.sendBeacon is fire-and-forget and survives
    // tab close. Bound by user-agent quotas (~64KB), so we feature-detect
    // and only use it for small bodies.
    const beacon = globalThis
        .navigator?.sendBeacon;
    if (typeof beacon === 'function' && body.length < 60_000) {
        try {
            const blob = new Blob([body], { type: 'application/json' });
            // sendBeacon doesn't carry headers — Authorization moves into
            // a query param so the server's existing Bearer auth still works.
            const beaconUrl = `${url}?token=${encodeURIComponent(cfg.token)}`;
            if (beacon.call(globalThis.navigator, beaconUrl, blob))
                return;
        }
        catch {
            // fall through to fetch
        }
    }
    try {
        await fetch(url, {
            body,
            headers,
            keepalive: true,
            method: 'POST',
        });
    }
    catch (e) {
        // No retry — log and forget. Hosts that care can wrap and add
        // their own retry policy at the app layer. Default logLevel
        // 'warn' surfaces this; bump to 'silent' to hide entirely.
        logger.warn('transport', 'failed:', e.message);
    }
}
//# sourceMappingURL=transport.js.map