// The five things a mini program tells you went wrong.
//
// `wx.onError` hands a **string**, not an Error — the message and the
// stack already flattened into one blob. So the stack is parsed back
// out rather than invented: the shape is the same
// `at fn (file:line:col)` the JS engines produce, and `parseStack`
// already reads it.
//
// The other three have no equivalent anywhere else in this product:
// a route that does not exist, the system asking for memory back, and
// a subpackage that failed to load. Each is a real user-facing failure
// in this ecosystem and invisible in every other one.

import { coerceError } from '@goliapkg/sentori-core'

import { error as reportError, warn as reportWarn } from '../verbs.js'
import { getWx } from '../wx.js'

let installed = false

export const installUncaughtHandlers = (): void => {
  const wx = getWx()
  if (installed || !wx) return
  installed = true

  wx.onError?.((msg: string) => {
    // The first line is `Name: message`; the rest is the stack. A
    // blob with no newline is a bare message and stays one.
    const text = typeof msg === 'string' ? msg : String(msg)
    const cut = text.indexOf('\n')
    const head = cut === -1 ? text : text.slice(0, cut)
    const colon = head.indexOf(':')

    // Rebuilt as a real Error rather than hand-assembled into the wire
    // shape. The `error` verb coerces whatever it is given, so a
    // pre-shaped object goes through `coerceError` and comes out as
    // `Error: Non-Error thrown: [object Object]` — which is what the
    // first version of this did, and every mini-program crash arrived
    // under one title.
    const err = new Error(colon > 0 ? head.slice(colon + 1).trim() : head)
    if (colon > 0) err.name = head.slice(0, colon).trim()
    // `parseStack` reads the same `at fn (file:line:col)` shape the
    // base library flattens into this blob.
    if (cut !== -1) err.stack = text
    reportError(err, { handled: false })
  })

  wx.onUnhandledRejection?.((res: { reason: unknown }) => {
    reportError(coerceError(res?.reason), { handled: false, source: 'rejection' })
  })
}

export const installPlatformHandlers = (): void => {
  const wx = getWx()
  if (!wx) return

  // A route the app navigated to that is not in the build. It is a
  // warn, not an error: nothing threw, but somebody reached a dead end.
  wx.onPageNotFound?.((res: { path: string }) => {
    reportWarn('weapp.pageNotFound', { path: res?.path, surface: { screen: res?.path } })
  })

  // The system asking for memory back. The level is the platform's:
  // 0 is a hint, 10 and 15 are the ones that precede a kill.
  wx.onMemoryWarning?.((res: { level?: number }) => {
    reportWarn('weapp.memoryWarning', { level: res?.level ?? null })
  })

  // A subpackage that failed to download. The screen the person asked
  // for simply never appears, and without this nothing anywhere
  // records that it did not.
  wx.onLazyLoadError?.((res: { type?: string; errMsg?: string }) => {
    reportWarn('weapp.lazyLoadError', { type: res?.type, detail: res?.errMsg })
  })
}

export const __resetForTests = (): void => {
  installed = false
}
