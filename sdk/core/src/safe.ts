/**
 * Safe-function wrappers — the NEVER rule in code.
 *
 * Every public `sentori.*` API is wrapped with `safeFn` (sync) or
 * `safeAsync` (async). On any thrown error inside the wrapped
 * body, the wrapper:
 *
 *   1. swallows the error completely — the host app never sees a
 *      throw / rejection attributable to Sentori
 *   2. optionally enqueues a self-report event via `reportInternal`
 *      (which is itself circuit-breaker'd to prevent recursion)
 *   3. returns `undefined` (sync) or a resolved `Promise<undefined>`
 *      (async)
 *
 * See `docs/design/manual-instrumentation-v2.md` — principle −1
 * ("NEVER harm the host app"). This module is the *load-bearing*
 * primitive for that rule.
 */

import { reportInternal } from './self-report.js'

/**
 * Wrap a sync function so it can never throw.
 *
 *     export const captureMessage = safeFn('captureMessage', (msg, opts) => {
 *       // body may throw — caller sees `undefined` on failure
 *     })
 */
export function safeFn<TArgs extends readonly unknown[], R>(
  name: string,
  fn: (...args: TArgs) => R,
): (...args: TArgs) => R | undefined {
  return (...args: TArgs): R | undefined => {
    try {
      return fn(...args)
    } catch (err) {
      reportInternal(name, err)
      return undefined
    }
  }
}

/**
 * Wrap an async function so it can never reject. The returned
 * promise always resolves; on internal failure it resolves to
 * `undefined`.
 */
export function safeAsync<TArgs extends readonly unknown[], R>(
  name: string,
  fn: (...args: TArgs) => Promise<R>,
): (...args: TArgs) => Promise<R | undefined> {
  return async (...args: TArgs): Promise<R | undefined> => {
    try {
      return await fn(...args)
    } catch (err) {
      reportInternal(name, err)
      return undefined
    }
  }
}
