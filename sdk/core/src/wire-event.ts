/**
 * Assembling one event for the wire.
 *
 * Pure: it is handed everything it needs and reaches for nothing. What
 * a runtime provides (which platform, who the user is, what the device
 * is, what the signal ring holds) differs; the shape the server parses
 * does not, and three SDKs assembling it three times is three chances
 * to drift from `WireEvent`.
 */

import { parseStack } from './stack.js'
import type { EventData, EventKind, SentoriError, Surface, WireEvent, WirePayload } from './types.js'

/**
 * Serialize any Error instances found in the data argument — the
 * error-in-data convention: a caught-but-noteworthy exception needs no
 * special API. One level deep is enough; nested containers of errors
 * are an anti-pattern we don't reward.
 */
export function serializeData(data?: EventData): Record<string, unknown> | undefined {
  if (!data) return undefined
  const out: Record<string, unknown> = {}
  for (const [k, v] of Object.entries(data)) {
    out[k] = v instanceof Error ? toSentoriError(v) : v
  }
  return out
}

export function toSentoriError(e: Error): SentoriError {
  return {
    type: e.name || 'Error',
    message: e.message,
    stack: parseStack(e.stack),
    cause: e.cause instanceof Error ? toSentoriError(e.cause) : null,
  }
}

export type WireEventInputs = {
  id: string
  kind: EventKind
  platform: WireEvent['platform']
  release: string
  environment: string
  name?: string
  surface?: Surface
  userKey?: string
  error?: SentoriError
  data?: EventData
  context?: Record<string, unknown>
  /** Already snapshotted by the caller: whether an event carries the
   *  ring is a per-kind decision (error and warn do, the light kinds
   *  do not), and that decision belongs to the verb, not here. */
  signals?: WirePayload['signals']
  device?: WirePayload['device']
  /** Injected only so a test can pin it. */
  occurredAt?: string
}

export function buildWireEvent(i: WireEventInputs): WireEvent {
  const payload: WirePayload = {}
  if (i.error) payload.error = i.error
  const data = serializeData(i.data)
  if (data) payload.data = data
  if (i.context) payload.context = i.context
  if (i.signals && i.signals.length > 0) payload.signals = i.signals
  if (i.device) payload.device = i.device

  return {
    id: i.id,
    kind: i.kind,
    occurredAt: i.occurredAt ?? new Date().toISOString(),
    platform: i.platform,
    release: i.release,
    environment: i.environment,
    name: i.name,
    surface: i.surface,
    userKey: i.userKey,
    payload,
  }
}

/**
 * Run the host's `beforeSend`, and survive it.
 *
 * `null` means the host deliberately dropped the event. A throw means
 * their hook has a bug, and the un-mutated event ships anyway — their
 * bug must not cost them the crash report. A return value that is not
 * an object is ignored for the same reason.
 */
export function applyBeforeSend(
  event: WireEvent,
  hook?: (e: WireEvent) => null | WireEvent
): null | WireEvent {
  if (!hook) return event
  try {
    const out = hook(event)
    if (out === null) return null
    if (out && typeof out === 'object') return out
    return event
  } catch {
    return event
  }
}
