import type { SessionPing } from './session.js';
import type { AssertStat, AttachmentKind, WireEvent } from './types.js';
/** Where the offline queue lives. One key, one SDK. */
export declare const PENDING_STORAGE_KEY = "@sentori/pending";
/** What the transport needs to know about the instance it reports to.
 *  Read through a function, not captured: `init` can run after some
 *  module has already taken a reference. */
export type TransportConfig = {
    ingestUrl: string;
    token: string;
    backendHealthUrl?: string;
};
/**
 * Persistence for the offline queue.
 *
 * Optional on purpose. React Native's is a peer dependency the host
 * may not have installed, and a mini-program's is synchronous with a
 * hard size limit. A transport with no storage still works — it just
 * counts what it could not keep, which is the part that used to be
 * missing.
 */
export type TransportStorage = {
    getItem(key: string): Promise<null | string>;
    setItem(key: string, value: string): Promise<void>;
    removeItem(key: string): Promise<void>;
};
export type TransportHost = {
    config: () => TransportConfig | null;
    /** `null` when this runtime has none, or the host never installed
     *  it. Resolved lazily because on React Native it requires a native
     *  module that may not be linked. */
    storage: () => Promise<null | TransportStorage>;
    /** The `Sentori-Sdk` header, e.g. `react-native/7.0.1`. */
    sdkLabel: string;
    /** Injected so a runtime without a global `fetch` (mini-programs)
     *  can pass its own, and so a test can drive one without touching
     *  the global. */
    fetch: typeof fetch;
    setTimeout?: (fn: () => void, ms: number) => unknown;
    clearTimeout?: (handle: unknown) => void;
};
type QueuedAttachment = {
    kind: AttachmentKind;
    blob: {
        base64?: string;
        text?: string;
        mediaType: string;
    };
    source: 'android' | 'ios' | 'js';
};
export type Transport = ReturnType<typeof createTransport>;
export declare function createTransport(host: TransportHost): {
    enqueue: (event: WireEvent) => void;
    countAssert: (name: string, ok: boolean, release: string) => void;
    queueSession: (ping: SessionPing) => void;
    start: () => void;
    flush: () => Promise<void>;
    queueAttachment: (eventId: string, kind: AttachmentKind, blob: QueuedAttachment['blob'], opts?: {
        source?: QueuedAttachment['source'];
    }) => void;
    uploadAttachment: (eventId: string, kind: AttachmentKind, blob: {
        base64?: string;
        text?: string;
        mediaType: string;
    }, opts?: {
        source?: 'android' | 'ios' | 'js';
    }) => Promise<null | {
        ref: string;
    }>;
    drainOfflineQueue: () => Promise<void>;
    reset(): void;
    peekQueue: () => readonly WireEvent[];
    peekSessions: () => readonly SessionPing[];
    peekDropped: () => number;
    peekAssertStats: () => readonly AssertStat[];
};
export {};
//# sourceMappingURL=transport.d.ts.map