import { type RegisterWebOptions } from '@goliapkg/sentori-javascript';
export type UseSentoriPushOptions = {
    /** VAPID public key (base64url) for the project. Pass the same
     *  value uploaded to the project's `push_credentials` row. */
    vapidPublicKey: string;
    /** Service Worker URL. Defaults to `/sentori-sw.js`. */
    serviceWorkerUrl?: string;
};
export type UseSentoriPushReturn = {
    /** Cached `ipt_*` handle, or `null` when the user hasn't opted in. */
    ipt: null | string;
    /** Browser notification permission: `'default' | 'granted' | 'denied'`,
     *  or `null` when the Notification API isn't available. */
    permission: NotificationPermission | null;
    /** Last error from a `register` / `unregister` call, or null. */
    error: Error | null;
    /** Run the opt-in flow. Returns the `ipt` on success. Optional
     *  per-call callbacks (`onMessage`, `onTap`, `linkHash`) override
     *  whatever was passed to the hook. */
    register: (perCall?: Pick<RegisterWebOptions, 'linkHash' | 'onMessage' | 'onTap' | 'metadata'>) => Promise<{
        ipt: string;
    } | null>;
    /** Revoke the handle + unsubscribe locally. */
    unregister: () => Promise<void>;
};
export declare function useSentoriPush(opts: UseSentoriPushOptions): UseSentoriPushReturn;
//# sourceMappingURL=usePush.d.ts.map