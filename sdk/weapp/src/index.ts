// @goliapkg/sentori-weapp — the WeChat mini-program SDK.
//
// The same five verbs as every other Sentori SDK, over the same wire.
// What differs is the platform: no `fetch`, no `crypto.subtle`, no
// `PerformanceObserver`, synchronous storage with a hard cap, and
// three failure modes (a missing page, a memory warning, a subpackage
// that would not load) that exist nowhere else.

export { init } from './init.js'
export { patchContext as context, setUser as user } from './scope.js'
export { flush } from './transport.js'
export { registerEmitHook } from './emit-hooks.js'
export { setWxHost, type WxRequestOptions } from './wx.js'

import { init } from './init.js'
import { patchContext, setUser } from './scope.js'
import { flush } from './transport.js'
import { verbs } from './verbs.js'

export const sentori = {
  init,
  user: setUser,
  context: patchContext,
  error: verbs.error,
  warn: verbs.warn,
  trace: verbs.trace,
  assert: verbs.assert,
  probe: verbs.probe,
  flush,
}

export type { WeappInitConfig } from './config.js'
export type { WxLike } from './wx.js'
export type { EventData, Surface, TraceOptions, User, WireEvent } from '@goliapkg/sentori-core'
