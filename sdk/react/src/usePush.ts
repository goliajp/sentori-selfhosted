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

import {
  readCachedIpt,
  registerWeb,
  unregisterWeb,
  type RegisterWebOptions,
} from '@goliapkg/sentori-javascript'
import { useCallback, useEffect, useState } from 'react'

export type UseSentoriPushOptions = {
  /** VAPID public key (base64url) for the project. Pass the same
   *  value uploaded to the project's `push_credentials` row. */
  vapidPublicKey: string
  /** Service Worker URL. Defaults to `/sentori-sw.js`. */
  serviceWorkerUrl?: string
}

export type UseSentoriPushReturn = {
  /** Cached `ipt_*` handle, or `null` when the user hasn't opted in. */
  ipt: null | string
  /** Browser notification permission: `'default' | 'granted' | 'denied'`,
   *  or `null` when the Notification API isn't available. */
  permission: NotificationPermission | null
  /** Last error from a `register` / `unregister` call, or null. */
  error: Error | null
  /** Run the opt-in flow. Returns the `ipt` on success. Optional
   *  per-call callbacks (`onMessage`, `onTap`, `linkHash`) override
   *  whatever was passed to the hook. */
  register: (
    perCall?: Pick<RegisterWebOptions, 'linkHash' | 'onMessage' | 'onTap' | 'metadata'>,
  ) => Promise<{ ipt: string } | null>
  /** Revoke the handle + unsubscribe locally. */
  unregister: () => Promise<void>
}

export function useSentoriPush(opts: UseSentoriPushOptions): UseSentoriPushReturn {
  const [ipt, setIpt] = useState<null | string>(() => readCachedIpt())
  const [permission, setPermission] = useState<NotificationPermission | null>(() =>
    typeof Notification === 'undefined' ? null : Notification.permission,
  )
  const [error, setError] = useState<Error | null>(null)

  useEffect(() => {
    // Resync ipt + permission on mount in case another tab toggled them.
    setIpt(readCachedIpt())
    if (typeof Notification !== 'undefined') setPermission(Notification.permission)
  }, [])

  const register = useCallback<UseSentoriPushReturn['register']>(
    async (perCall) => {
      setError(null)
      try {
        const result = await registerWeb({
          vapidPublicKey: opts.vapidPublicKey,
          serviceWorkerUrl: opts.serviceWorkerUrl,
          ...perCall,
        })
        setIpt(result.ipt)
        if (typeof Notification !== 'undefined') setPermission(Notification.permission)
        return result
      } catch (e) {
        const err = e instanceof Error ? e : new Error(String(e))
        setError(err)
        if (typeof Notification !== 'undefined') setPermission(Notification.permission)
        return null
      }
    },
    [opts.vapidPublicKey, opts.serviceWorkerUrl],
  )

  const unregister = useCallback<UseSentoriPushReturn['unregister']>(async () => {
    setError(null)
    try {
      await unregisterWeb()
      setIpt(null)
    } catch (e) {
      const err = e instanceof Error ? e : new Error(String(e))
      setError(err)
    }
  }, [])

  return { ipt, permission, error, register, unregister }
}
