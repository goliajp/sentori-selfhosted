// v2.12 — `useSentoriPush()` React hook.
//
// Thin React wrapper over `registerWeb` from
// `@goliapkg/sentori-javascript`. The hook owns three pieces of
// reactive state — the `ipt` handle, the OS permission state, and
// the last error — plus stable `register` + `unregister` callbacks.
//
// Hosts call:
//
//   const { ipt, register, permission, error } = useSentoriPush({
//     vapidPublicKey: '...',
//   })
//
//   <button onClick={() => register({ onMessage: ... })}>
//     {ipt ? 'Disabled' : 'Enable notifications'}
//   </button>
//
// The hook never auto-calls `register()` — opt-in stays a host-app
// decision per the v2.7→v2.12 design.
import { readCachedIpt, registerWeb, unregisterWeb, } from '@goliapkg/sentori-javascript';
import { useCallback, useEffect, useState } from 'react';
export function useSentoriPush(opts) {
    const [ipt, setIpt] = useState(() => readCachedIpt());
    const [permission, setPermission] = useState(() => typeof Notification === 'undefined' ? null : Notification.permission);
    const [error, setError] = useState(null);
    useEffect(() => {
        // Resync ipt + permission on mount in case another tab toggled them.
        setIpt(readCachedIpt());
        if (typeof Notification !== 'undefined')
            setPermission(Notification.permission);
    }, []);
    const register = useCallback(async (perCall) => {
        setError(null);
        try {
            const result = await registerWeb({
                vapidPublicKey: opts.vapidPublicKey,
                serviceWorkerUrl: opts.serviceWorkerUrl,
                ...perCall,
            });
            setIpt(result.ipt);
            if (typeof Notification !== 'undefined')
                setPermission(Notification.permission);
            return result;
        }
        catch (e) {
            const err = e instanceof Error ? e : new Error(String(e));
            setError(err);
            if (typeof Notification !== 'undefined')
                setPermission(Notification.permission);
            return null;
        }
    }, [opts.vapidPublicKey, opts.serviceWorkerUrl]);
    const unregister = useCallback(async () => {
        setError(null);
        try {
            await unregisterWeb();
            setIpt(null);
        }
        catch (e) {
            const err = e instanceof Error ? e : new Error(String(e));
            setError(err);
        }
    }, []);
    return { ipt, permission, error, register, unregister };
}
//# sourceMappingURL=usePush.js.map