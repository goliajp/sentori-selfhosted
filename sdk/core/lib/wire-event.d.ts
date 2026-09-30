/**
 * Assembling one event for the wire.
 *
 * Pure: it is handed everything it needs and reaches for nothing. What
 * a runtime provides (which platform, who the user is, what the device
 * is, what the signal ring holds) differs; the shape the server parses
 * does not, and three SDKs assembling it three times is three chances
 * to drift from `WireEvent`.
 */
import type { EventData, EventKind, SentoriError, Surface, WireEvent, WirePayload } from './types.js';
/**
 * Serialize any Error instances found in the data argument — the
 * error-in-data convention: a caught-but-noteworthy exception needs no
 * special API. One level deep is enough; nested containers of errors
 * are an anti-pattern we don't reward.
 */
export declare function serializeData(data?: EventData): Record<string, unknown> | undefined;
export declare function toSentoriError(e: Error): SentoriError;
export type WireEventInputs = {
    id: string;
    kind: EventKind;
    platform: WireEvent['platform'];
    release: string;
    environment: string;
    name?: string;
    surface?: Surface;
    userKey?: string;
    error?: SentoriError;
    data?: EventData;
    context?: Record<string, unknown>;
    /** Already snapshotted by the caller: whether an event carries the
     *  ring is a per-kind decision (error and warn do, the light kinds
     *  do not), and that decision belongs to the verb, not here. */
    signals?: WirePayload['signals'];
    device?: WirePayload['device'];
    /** Injected only so a test can pin it. */
    occurredAt?: string;
};
export declare function buildWireEvent(i: WireEventInputs): WireEvent;
/**
 * Run the host's `beforeSend`, and survive it.
 *
 * `null` means the host deliberately dropped the event. A throw means
 * their hook has a bug, and the un-mutated event ships anyway — their
 * bug must not cost them the crash report. A return value that is not
 * an object is ignored for the same reason.
 */
export declare function applyBeforeSend(event: WireEvent, hook?: (e: WireEvent) => null | WireEvent): null | WireEvent;
//# sourceMappingURL=wire-event.d.ts.map