// The five kinds, in a mini program.
//
// The same five verbs with the same signatures as every other Sentori
// SDK. A team shipping an app and a mini program should not learn two
// vocabularies for one product, and the server's `Kind` enum is the
// same enum either way.

import {
  applyBeforeSend,
  buildWireEvent,
  coerceError,
  pushSignal,
  safeFn,
  snapshotSignals,
  toSentoriError,
  uuidV7,
} from '@goliapkg/sentori-core'
import type {
  EventData,
  EventKind,
  SentoriError,
  Surface,
  TraceOptions,
  WireEvent,
} from '@goliapkg/sentori-core'

import { getConfig } from './config.js'
import { collectDevice } from './device.js'
import { onEventEmitted } from './emit-hooks.js'
import { currentContext, currentUserKey } from './scope.js'
import { countAssert, enqueue } from './transport.js'

type EmitOptions = {
  name?: string
  surface?: Surface
  error?: SentoriError
  data?: EventData
  /** error / warn / assert carry the ring; the light kinds do not. */
  withSignals?: boolean
}

/** Assemble + enqueue one event. The single funnel every verb uses. */
const emit = (kind: EventKind, opts: EmitOptions): string => {
  const id = uuidV7()
  const config = getConfig()
  if (!config || !config.enabled) return id // no-op before init — iron rule

  let event: WireEvent = buildWireEvent({
    id,
    kind,
    platform: 'weapp',
    release: config.release,
    environment: config.environment,
    name: opts.name,
    surface: opts.surface,
    userKey: currentUserKey(),
    error: opts.error,
    data: opts.data,
    context: currentContext(),
    signals: opts.withSignals ? snapshotSignals() : undefined,
    device: collectDevice(),
  })

  const kept = applyBeforeSend(event, config.beforeSend)
  if (kept === null) return id // the host dropped it deliberately
  event = kept

  enqueue(event)
  onEventEmitted(event)
  return id
}

export const error = safeFn('error', (err: unknown, data?: EventData): string =>
  emit('error', { error: toSentoriError(coerceError(err)), data, withSignals: true })
)

export const warn = safeFn('warn', (name: string, data?: EventData): string => {
  // A hand-written warn's surface can ride in data.surface; the
  // detected scenarios pass it explicitly via warnDetected.
  const surface = data && typeof data.surface === 'object' ? (data.surface as Surface) : undefined
  return emit('warn', { name, surface, data, withSignals: true })
})

/** SDK-internal: a detected warn scenario with an explicit surface. */
export const warnDetected = (scenario: string, surface: Surface, data?: EventData): string =>
  emit('warn', { name: scenario, surface, data, withSignals: true })

/** The SDK's own fault, filed as a `warn` so our bug does not sit at
 *  the top of the host's inbox wearing their name. */
export const internalFault = (report: {
  api: string
  message: string
  errorName?: string
  stack?: string
}): void => {
  emit('warn', {
    name: 'sentori.internal',
    data: {
      api: report.api,
      message: report.message,
      ...(report.errorName ? { errorName: report.errorName } : {}),
      ...(report.stack ? { stack: report.stack } : {}),
    },
  })
}

export const trace = safeFn(
  'trace',
  (name: string, data?: EventData, opts?: TraceOptions): string => {
    // Every trace is context: it lands in the ring regardless.
    pushSignal('trace', { name, ...(data ?? {}) })
    if (opts?.quiet) return uuidV7()
    return emit('trace', { name, data })
  }
)

export const assert = safeFn('assert', (name: string, ok: boolean, data?: EventData): string => {
  const config = getConfig()
  if (config) countAssert(name, ok, config.release)
  if (ok) return uuidV7() // passes aggregate; they are never events
  return emit('assert', { name, data, withSignals: true })
})

export const probe = safeFn('probe', (ref: string, data?: EventData): string =>
  // A tripwire: reaching this call IS the signal. Never throws, never
  // changes control flow.
  emit('probe', { name: ref, data })
)

// safeFn returns `R | undefined`; the public surface promises a
// string. An internal failure mints an id that simply never ships —
// honest enough, and the host never sees a throw.
export const verbs = {
  error: (err: unknown, data?: EventData): string => error(err, data) ?? uuidV7(),
  warn: (name: string, data?: EventData): string => warn(name, data) ?? uuidV7(),
  trace: (name: string, data?: EventData, opts?: TraceOptions): string =>
    trace(name, data, opts) ?? uuidV7(),
  assert: (name: string, ok: boolean, data?: EventData): string =>
    assert(name, ok, data) ?? uuidV7(),
  probe: (ref: string, data?: EventData): string => probe(ref, data) ?? uuidV7(),
}
