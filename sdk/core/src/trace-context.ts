// Phase 35 sub-A: "active span" propagation.
//
// `startSpan()` inherits trace/parent from whichever span is "active"
// on the current call stack — implementations of fetch wrappers /
// middleware push a span onto this stack on entry and pop on exit.
// Without active-context the caller must pass `parent` explicitly,
// which is fine but awkward across async boundaries.
//
// Two implementations behind the same surface:
//
//   - Node (server SDK): we wire AsyncLocalStorage lazily — present
//     since Node 16, mainstream in everything we target. AsyncLocalStorage
//     follows continuations through Promise / setImmediate / setTimeout
//     correctly, which is what we need to span an async function with
//     `withSpan(span, fn)`.
//
//   - Browser / RN: no AsyncLocalStorage. We fall back to a plain
//     module-scoped variable. `withSpan` save-and-restore is honest
//     for synchronous + linear-await call chains, but loses the active
//     value if you fork into multiple concurrent promises. Callers in
//     that situation should pass `parent` to `startSpan` directly.
//
// The pragmatic stance: do not depend on context propagation for
// correctness; treat it as a convenience. SDK callsites that care
// about correctness (the fetch wrapper, server middleware) pass
// `parent` explicitly.

import type { SpanContextLike } from './spans.js'

type Store = { span: SpanContextLike | null }

interface ContextImpl {
  get(): SpanContextLike | null
  run<T>(span: SpanContextLike, fn: () => T): T
  /** Set the active span without a scope — for long-lived contexts
   *  (screen navigation) that aren't a single `fn` call. No-op on the
   *  AsyncLocalStorage impl: ALS has no clean "set and leave", and the
   *  only caller (navigation) runs on browser/RN, where the fallback
   *  impl is in effect. */
  set(span: SpanContextLike | null): void
}

function loadNodeImpl(): ContextImpl | null {
  // Probe AsyncLocalStorage without statically importing — keeps the
  // module bundleable for browser/RN without conditionals in the
  // build. `globalThis.process` is the most reliable Node sniff;
  // `require` may be polyfilled in RN.
  const proc = (globalThis as { process?: { versions?: { node?: string } } }).process
  if (!proc?.versions?.node) return null
  try {
    // TS 6 typecheck without @types/node — `require` isn't ambient
    // here. Cast through globalThis (Node injects require even in
    // CommonJS-emitted-as-ESM bundles via interop).
    const req = (globalThis as { require?: (id: string) => unknown }).require
    if (typeof req !== 'function') return null
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const mod = req('node:async_hooks') as {
      AsyncLocalStorage: new <T>() => {
        getStore(): T | undefined
        run<R>(s: T, fn: () => R): R
      }
    }
    const als = new mod.AsyncLocalStorage<Store>()
    return {
      get: () => als.getStore()?.span ?? null,
      run: (span, fn) => als.run({ span }, fn),
      set: () => {
        // No-op — see ContextImpl.set doc. Navigation is browser/RN.
      },
    }
  } catch {
    return null
  }
}

function fallbackImpl(): ContextImpl {
  let current: SpanContextLike | null = null
  return {
    get: () => current,
    run: (span, fn) => {
      const prev = current
      current = span
      try {
        return fn()
      } finally {
        current = prev
      }
    },
    set: (span) => {
      current = span
    },
  }
}

let _impl: ContextImpl | null = null

function impl(): ContextImpl {
  if (_impl) return _impl
  _impl = loadNodeImpl() ?? fallbackImpl()
  return _impl
}

/** Currently active span context, or null. Falls back across the
 *  fallback impl's save-and-restore boundary. */
export function activeSpan(): SpanContextLike | null {
  return impl().get()
}

/**
 * Run `fn` with `span` as the active span. Use this to wrap any unit
 * of work whose child spans should attribute up to this one:
 *
 *     const span = startSpan('handler.GET')
 *     try {
 *       return await withSpan(span, async () => {
 *         // any startSpan() in here picks up `span` as parent
 *         return await loadUser()
 *       })
 *     } finally {
 *       span.finish({ status: 'ok' })
 *     }
 *
 * Node: routed through AsyncLocalStorage, so awaits inside `fn`
 * preserve the active span.
 *
 * Browser/RN: save-and-restore. Correct for linear awaits;
 * concurrent promises forked inside `fn` won't see the active span
 * after the first await suspends.
 *
 * v2.3 — exported as `withActiveSpan` (clear semantic name). The
 * old export name `withSpan` is re-exported through `spans.ts` as
 * an overloaded function that dispatches by first-arg type
 * (string → high-level wrap helper; SpanContextLike → this
 * function). New code should call `withSpan(name, fn)`.
 */
export function withActiveSpan<T>(span: SpanContextLike, fn: () => T): T {
  return impl().run(span, fn)
}

/**
 * Set (or clear, with `null`) the active span outside of a `withSpan`
 * scope. For long-lived contexts where a `fn` wrapper doesn't fit —
 * specifically screen navigation: `useTraceNavigation` opens a
 * `react.navigation` span when a screen is entered and leaves it
 * active for that screen's lifetime, so the screen's `http.client`
 * spans become children (one trace per screen instead of one per
 * request).
 *
 * Browser/RN only in practice — no-op on the Node/AsyncLocalStorage
 * impl (ALS can't "set and leave" cleanly). Don't reach for this in
 * async server code; `withSpan` is the scoped tool there.
 */
export function setActiveSpan(span: SpanContextLike | null): void {
  impl().set(span)
}

/** Reset the implementation choice — test-only. Production code never
 *  calls this; switching propagation strategy at runtime would mean
 *  losing the current active context. */
export function __resetTraceContextForTests(): void {
  _impl = null
}

/** Test-only: force the module-variable fallback impl regardless of
 *  environment. `bun test` runs as Node (so the ALS impl is picked),
 *  but navigation — the one feature that relies on `setActiveSpan` —
 *  only runs on browser/RN, where the fallback is in effect. Tests of
 *  that path call this so they exercise the impl that actually ships
 *  there. */
export function __useFallbackTraceContextForTests(): void {
  _impl = fallbackImpl()
}
