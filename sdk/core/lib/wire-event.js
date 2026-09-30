/**
 * Assembling one event for the wire.
 *
 * Pure: it is handed everything it needs and reaches for nothing. What
 * a runtime provides (which platform, who the user is, what the device
 * is, what the signal ring holds) differs; the shape the server parses
 * does not, and three SDKs assembling it three times is three chances
 * to drift from `WireEvent`.
 */
import { parseStack } from './stack.js';
/**
 * Serialize any Error instances found in the data argument — the
 * error-in-data convention: a caught-but-noteworthy exception needs no
 * special API. One level deep is enough; nested containers of errors
 * are an anti-pattern we don't reward.
 */
export function serializeData(data) {
    if (!data)
        return undefined;
    const out = {};
    for (const [k, v] of Object.entries(data)) {
        out[k] = v instanceof Error ? toSentoriError(v) : v;
    }
    return out;
}
export function toSentoriError(e) {
    return {
        type: e.name || 'Error',
        message: e.message,
        stack: parseStack(e.stack),
        cause: e.cause instanceof Error ? toSentoriError(e.cause) : null,
    };
}
export function buildWireEvent(i) {
    const payload = {};
    if (i.error)
        payload.error = i.error;
    const data = serializeData(i.data);
    if (data)
        payload.data = data;
    if (i.context)
        payload.context = i.context;
    if (i.signals && i.signals.length > 0)
        payload.signals = i.signals;
    if (i.device)
        payload.device = i.device;
    return {
        id: i.id,
        kind: i.kind,
        occurredAt: i.occurredAt ?? new Date().toISOString(),
        platform: i.platform,
        release: i.release,
        environment: i.environment,
        name: i.name,
        surface: i.surface,
        userKey: i.userKey,
        payload,
    };
}
/**
 * Run the host's `beforeSend`, and survive it.
 *
 * `null` means the host deliberately dropped the event. A throw means
 * their hook has a bug, and the un-mutated event ships anyway — their
 * bug must not cost them the crash report. A return value that is not
 * an object is ignored for the same reason.
 */
export function applyBeforeSend(event, hook) {
    if (!hook)
        return event;
    try {
        const out = hook(event);
        if (out === null)
            return null;
        if (out && typeof out === 'object')
            return out;
        return event;
    }
    catch {
        return event;
    }
}
//# sourceMappingURL=wire-event.js.map