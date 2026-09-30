import type { Signal } from './types';
export declare const configureRing: (opts: {
    capacity?: number;
    windowMs?: number;
}) => void;
/** Push one signal. O(1); oldest entry drops past capacity. */
export declare const pushSignal: (kind: string, data?: Record<string, unknown>) => void;
/**
 * Snapshot the ring relative to `now`, windowed and oldest-first —
 * the shape `payload.signals` carries.
 */
export declare const snapshotSignals: (now?: number) => Signal[];
/** Test / logout hygiene. */
export declare const clearSignals: () => void;
//# sourceMappingURL=signal-ring.d.ts.map