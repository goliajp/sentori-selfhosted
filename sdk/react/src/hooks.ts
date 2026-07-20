import { coerceError } from '@goliapkg/sentori-core'
import { useCallback } from 'react'

import { useSentoriCtx } from './SentoriProvider.js'

import type { CaptureExtras, SentoriContextValue } from './types.js'

/**
 * Imperative access to the SDK from inside a component. Returns the
 * full context — most code only needs `captureError`, `addBreadcrumb`,
 * or `setUser`.
 *
 *     const { captureError, addBreadcrumb } = useSentori()
 *     try {
 *       await api.checkout(order)
 *     } catch (e) {
 *       captureError(e as Error, { tags: { stage: 'checkout' } })
 *     }
 */
export function useSentori(): SentoriContextValue {
  return useSentoriCtx()
}

/**
 * Wraps an async function so any thrown / rejected error is captured
 * and rethrown. The wrapper is `useCallback`-stable across renders if
 * `extras` is stable too — pass it inside `useMemo` if you build it
 * inline.
 *
 *     const checkout = useCaptureError(
 *       async (order: Order) => api.checkout(order),
 *       { tags: { stage: 'checkout' } },
 *     )
 *     await checkout(order)   // captures + throws on failure
 */
export function useCaptureError<TArgs extends unknown[], TRet>(
  fn: (...args: TArgs) => Promise<TRet> | TRet,
  extras?: CaptureExtras,
): (...args: TArgs) => Promise<TRet> {
  const { captureError } = useSentoriCtx()
  return useCallback(
    async (...args: TArgs) => {
      try {
        return await fn(...args)
      } catch (e) {
        // `coerceError` keeps non-Error throws useful — JSON-stringifies
        // plain objects instead of letting `String(e)` collapse them to
        // the literal text `[object Object]`. See coerce-error.ts.
        captureError(coerceError(e), extras)
        throw e
      }
    },
    [captureError, fn, extras],
  )
}
