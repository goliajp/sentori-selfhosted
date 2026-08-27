// The signal ring — "what the user was doing" for the last N
// seconds, shipped inside payload.signals when an error/warn event
// goes out (design.md §4: the breadcrumb concept's successor).
//
// Bounded, overwrite-oldest, zero allocation on the hot path beyond
// the entry object itself. Auto signals (nav / tap / http /
// lifecycle) and quiet traces both land here.
const DEFAULT_CAPACITY = 100;
// Matches the replay window: with signals at 30s and replay at 60s
// the left half of the case timeline had frames but never events
// (insight round-4 A3).
const DEFAULT_WINDOW_MS = 60_000;
let entries = [];
let capacity = DEFAULT_CAPACITY;
let windowMs = DEFAULT_WINDOW_MS;
export const configureRing = (opts) => {
    if (opts.capacity && opts.capacity > 0)
        capacity = opts.capacity;
    if (opts.windowMs && opts.windowMs > 0)
        windowMs = opts.windowMs;
};
/** Push one signal. O(1); oldest entry drops past capacity. */
export const pushSignal = (kind, data) => {
    entries.push({ at: Date.now(), kind, data });
    if (entries.length > capacity)
        entries.splice(0, entries.length - capacity);
};
/**
 * Snapshot the ring relative to `now`, windowed and oldest-first —
 * the shape `payload.signals` carries.
 */
export const snapshotSignals = (now = Date.now()) => {
    const cutoff = now - windowMs;
    return entries
        .filter((e) => e.at >= cutoff)
        .map((e) => ({
        t: Math.round((e.at - now) / 100) / 10, // one decimal, seconds
        kind: e.kind,
        data: e.data,
    }));
};
/** Test / logout hygiene. */
export const clearSignals = () => {
    entries = [];
};
//# sourceMappingURL=signal-ring.js.map