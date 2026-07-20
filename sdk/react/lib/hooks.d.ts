import type { CaptureExtras, SentoriContextValue } from './types.js';
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
export declare function useSentori(): SentoriContextValue;
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
export declare function useCaptureError<TArgs extends unknown[], TRet>(fn: (...args: TArgs) => Promise<TRet> | TRet, extras?: CaptureExtras): (...args: TArgs) => Promise<TRet>;
//# sourceMappingURL=hooks.d.ts.map