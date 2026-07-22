import { type AttachmentKind, type AttachmentMeta, type SessionPing } from '@goliapkg/sentori-core';
import type { Event } from './types.js';
/**
 * Minimal HTTP transport. POST /v1/events with a Bearer token.
 * - Browser: prefers `navigator.sendBeacon` on page-unload paths;
 *   otherwise plain fetch with `keepalive: true` so events survive
 *   a tab close mid-flight.
 * - Node: plain fetch (Node 18+ has it global).
 *
 * On 4xx/5xx the SDK currently drops the event silently — retry +
 * persistent queue is a follow-up if anyone actually wants it.
 */
export type TransportConfig = {
    ingestUrl: string;
    token: string;
};
export declare function send(cfg: TransportConfig, event: Event): Promise<void>;
/**
 * Phase 26 sub-B: session ping. Same beacon → fetch fallback as `send`,
 * because sessions almost always close on the same path that closes
 * the tab — beacon survives that, fetch with `keepalive: true` is the
 * fallback when beacon is unavailable.
 */
export declare function sendSession(cfg: TransportConfig, ping: SessionPing): Promise<void>;
export declare function startSpanFlush(): void;
export declare function stopSpanFlush(): void;
export declare function flushSpans(): Promise<void>;
/**
 * Phase 46 — upload an attachment blob (used for `sessionTrail` and
 * any future per-event JSON blobs the Web SDK wants to ship).
 *
 * Mirrors the RN SDK's `uploadAttachment` shape but uses the browser's
 * native `Blob` instead of base64-roundtripping. Returns the server-
 * issued ref UUID on success, `null` on any failure (we ship the rest
 * of the event regardless).
 */
export declare function uploadAttachment(cfg: TransportConfig, eventId: string, kind: AttachmentKind, blob: {
    body: string;
    mediaType: string;
}): Promise<AttachmentMeta | null>;
//# sourceMappingURL=transport.d.ts.map