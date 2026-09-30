// @goliapkg/sentori-web — the browser SDK.
//
// One package, framework-agnostic on purpose. The last attempt at this
// shipped six packages, one per framework, and they drifted: a fix
// landed in the React one and not the others, and an integrator on the
// fifth could not tell which behaviours they had. A React app and a
// Svelte app make the same five calls.

export { init } from './init.js'
export { patchContext as context, setUser as user } from './scope.js'
export { flush } from './transport.js'
export { registerEmitHook } from './emit-hooks.js'
export { registerMaskQuery } from './replay.js'

import { patchContext, setUser } from './scope.js'
import { init } from './init.js'
import { flush } from './transport.js'
import { verbs } from './verbs.js'

/** The whole public surface: init, identity, the five kinds, flush. */
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

export type { WebInitConfig } from './config.js'
export type {
  EventData,
  Surface,
  TraceOptions,
  User,
  WireEvent,
} from '@goliapkg/sentori-core'
