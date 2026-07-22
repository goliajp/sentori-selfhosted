export type RegisterWebOptions = {
    serviceWorkerUrl?: string;
    vapidPublicKey: string;
    linkHash?: string;
    metadata?: Record<string, unknown>;
    onMessage?: (payload: {
        title?: string;
        body?: string;
        data?: unknown;
    }) => void;
    onTap?: (data: unknown) => void;
    onError?: (err: Error) => void;
};
export type RegisterWebResult = {
    ipt: string;
};
/**
 * Register the current browser tab for Web Push and return the
 * resulting `ipt_*` handle. Opt-in: caller invokes when ready.
 *
 * Steps:
 *  1. Permission prompt via `Notification.requestPermission()`.
 *  2. Register the Service Worker (idempotent — reuses an existing
 *     registration with the same scope if present).
 *  3. Subscribe via `pushManager.subscribe()` with the project's
 *     VAPID public key.
 *  4. POST the subscription JSON to `/v1/push/tokens`.
 *  5. Stash the `ipt_*` handle in localStorage + return it.
 *
 * On any rejection the promise rejects with a tagged Error.
 */
export declare function registerWeb(opts: RegisterWebOptions): Promise<RegisterWebResult>;
/**
 * Revoke the cached `ipt_*` handle (DELETE /v1/push/tokens/{ipt})
 * + unsubscribe locally. Idempotent — repeat calls are no-ops.
 *
 * Does not unregister the Service Worker; another host app feature
 * might rely on it. Customers who own the SW exclusively can
 * `navigator.serviceWorker.getRegistration().then(r => r?.unregister())`
 * after this call.
 */
export declare function unregisterWeb(): Promise<void>;
export declare function readCachedIpt(): string | null;
//# sourceMappingURL=push.d.ts.map