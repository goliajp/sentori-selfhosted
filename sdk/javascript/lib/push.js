// v2.8 — Web Push registration on top of the Sentori server's
// `/v1/push/*` API.
//
// `registerWeb` walks the browser permission → Service Worker →
// PushSubscription → server-register flow and returns the
// `ipt_<uuid>` handle the server uses as a stable device id. The
// caller (host app) decides WHEN to invoke this — Sentori never
// triggers a permission prompt on its own.
//
// The native token persisted server-side is `JSON.stringify(subscription)`
// — the whole `{ endpoint, keys: { p256dh, auth } }` blob. The Web
// Push provider on the server decodes this to drive RFC 8291
// payload encryption.
import { logger } from '@goliapkg/sentori-core';
import { getConfig } from './config.js';
const STORAGE_KEY = 'sentori.push.ipt';
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
export async function registerWeb(opts) {
    try {
        assertBrowserApi();
        const cfg = getConfig();
        if (!cfg)
            throw new Error('sentori not initialised — call initSentori() first');
        const swUrl = opts.serviceWorkerUrl ?? '/sentori-sw.js';
        const permission = await Notification.requestPermission();
        if (permission !== 'granted') {
            throw new Error(`Notification permission '${permission}'; cannot subscribe`);
        }
        const reg = await navigator.serviceWorker.register(swUrl);
        await navigator.serviceWorker.ready;
        // pushManager.subscribe is idempotent for the same VAPID key
        // pair — returns the existing subscription if already subscribed.
        const subscription = await reg.pushManager.subscribe({
            userVisibleOnly: true,
            applicationServerKey: urlBase64ToBuffer(opts.vapidPublicKey),
        });
        const ipt = await registerWithServer(cfg, subscription, opts);
        cacheIpt(ipt);
        // Wire SW → page message channel for foreground delivery.
        bindServiceWorkerListener(opts.onMessage, opts.onTap);
        return { ipt };
    }
    catch (e) {
        const err = e instanceof Error ? e : new Error(String(e));
        logger.warn('sentori.push.registerWeb failed:', err.message);
        opts.onError?.(err);
        throw err;
    }
}
/**
 * Revoke the cached `ipt_*` handle (DELETE /v1/push/tokens/{ipt})
 * + unsubscribe locally. Idempotent — repeat calls are no-ops.
 *
 * Does not unregister the Service Worker; another host app feature
 * might rely on it. Customers who own the SW exclusively can
 * `navigator.serviceWorker.getRegistration().then(r => r?.unregister())`
 * after this call.
 */
export async function unregisterWeb() {
    const cfg = getConfig();
    const ipt = readCachedIpt();
    if (cfg && ipt) {
        try {
            await fetch(joinUrl(cfg.ingestUrl, `/v1/push/tokens/${ipt}`), {
                method: 'DELETE',
                headers: { authorization: `Bearer ${cfg.token}` },
                keepalive: true,
            });
        }
        catch (e) {
            logger.warn('sentori.push.unregisterWeb: server delete failed', e);
        }
    }
    if (typeof navigator !== 'undefined' && navigator.serviceWorker) {
        try {
            const reg = await navigator.serviceWorker.getRegistration();
            const sub = await reg?.pushManager.getSubscription();
            await sub?.unsubscribe();
        }
        catch (e) {
            logger.warn('sentori.push.unregisterWeb: local unsubscribe failed', e);
        }
    }
    clearCachedIpt();
}
/// Read the last registered `ipt_*` handle from localStorage.
/// Useful for hosts that want to short-circuit re-registration on
/// page load.
export function readCachedIpt() {
    try {
        return typeof localStorage !== 'undefined' ? localStorage.getItem(STORAGE_KEY) : null;
    }
    catch {
        return null;
    }
}
function cacheIpt(ipt) {
    try {
        if (typeof localStorage !== 'undefined')
            localStorage.setItem(STORAGE_KEY, ipt);
    }
    catch {
        /* private mode / quota — non-fatal */
    }
}
function clearCachedIpt() {
    try {
        if (typeof localStorage !== 'undefined')
            localStorage.removeItem(STORAGE_KEY);
    }
    catch {
        /* non-fatal */
    }
}
function assertBrowserApi() {
    if (typeof window === 'undefined') {
        throw new Error('sentori.push.registerWeb requires a browser environment');
    }
    if (!('Notification' in window)) {
        throw new Error('Notification API not available in this browser');
    }
    if (!('serviceWorker' in navigator)) {
        throw new Error('Service Worker API not available in this browser');
    }
    if (!('PushManager' in window)) {
        throw new Error('PushManager not available in this browser');
    }
}
async function registerWithServer(cfg, subscription, opts) {
    const native = JSON.stringify(subscription.toJSON());
    const body = {
        provider: 'webpush',
        nativeToken: native,
        linkHash: opts.linkHash,
        metadata: { ...defaultMetadata(), ...(opts.metadata ?? {}) },
    };
    const res = await fetch(joinUrl(cfg.ingestUrl, '/v1/push/tokens'), {
        method: 'POST',
        headers: {
            authorization: `Bearer ${cfg.token}`,
            'content-type': 'application/json',
        },
        body: JSON.stringify(body),
    });
    if (!res.ok) {
        throw new Error(`/v1/push/tokens HTTP ${res.status}`);
    }
    const json = (await res.json());
    if (typeof json.id !== 'string' || !json.id.startsWith('ipt_')) {
        throw new Error('server did not return an ipt_* handle');
    }
    return json.id;
}
function defaultMetadata() {
    if (typeof navigator === 'undefined')
        return {};
    return {
        userAgent: navigator.userAgent,
        language: navigator.language,
        platform: navigator.platform,
    };
}
function joinUrl(base, path) {
    return `${base.replace(/\/+$/, '')}${path}`;
}
/**
 * Convert a base64url-encoded VAPID public key into the ArrayBuffer
 * shape `pushManager.subscribe({ applicationServerKey })` accepts.
 * We hand back a `Uint8Array` whose underlying buffer is a fresh
 * `ArrayBuffer` (not `SharedArrayBuffer`) so the strict DOM lib
 * typings line up.
 */
function urlBase64ToBuffer(b64url) {
    const padding = '='.repeat((4 - (b64url.length % 4)) % 4);
    const base64 = (b64url + padding).replace(/-/g, '+').replace(/_/g, '/');
    const raw = typeof atob !== 'undefined'
        ? atob(base64)
        : Buffer.from(base64, 'base64').toString('binary');
    const buf = new ArrayBuffer(raw.length);
    const view = new Uint8Array(buf);
    for (let i = 0; i < raw.length; i++)
        view[i] = raw.charCodeAt(i);
    return buf;
}
function bindServiceWorkerListener(onMessage, onTap) {
    if (!onMessage && !onTap)
        return;
    if (typeof navigator === 'undefined' || !navigator.serviceWorker)
        return;
    navigator.serviceWorker.addEventListener('message', (event) => {
        const data = event.data;
        if (!data || !data.type)
            return;
        if (data.type === 'sentori.push.message' && onMessage)
            onMessage(data.payload);
        if (data.type === 'sentori.push.tap' && onTap)
            onTap(data.payload?.data);
    });
}
//# sourceMappingURL=push.js.map