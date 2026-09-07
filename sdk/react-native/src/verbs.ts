// The five event verbs (design.md §4). Everything here is
// synchronous, never throws, and returns the client-minted event id
// — the zero-cost iron rule made code.
//
//   sentori.error(err)    出了什么事?
//   sentori.warn(name)    用户哪里不舒服?
//   sentori.trace(name)   这里发生了什么?
//   sentori.assert(n, ok) 这里应该成立吗?
//   sentori.probe(ref)    那个 bug 回来了吗?

import {
  coerceError,
  parseStack,
  pushSignal,
  safeFn,
  snapshotSignals,
  uuidV7,
} from '@goliapkg/sentori-core';
import type {
  EventData,
  EventKind,
  SentoriError,
  Surface,
  TraceOptions,
  WireEvent,
  WirePayload,
} from '@goliapkg/sentori-core';

import { getConfig } from './config';
import { collectDevice } from './device';
import { onEventEmitted } from './emit-hooks';
import { symbolicateErrorViaMetro } from './handlers/dev-symbolicate';
import { currentContext, currentUserKey } from './scope';
import { countAssert, enqueue } from './transport';

declare const __DEV__: boolean | undefined;

/** Serialize any Error instances found in the data argument — the
 *  error-in-data convention (design.md §4): a caught-but-noteworthy
 *  exception needs no special API. One level deep is enough; nested
 *  containers of errors are an anti-pattern we don't reward. */
const serializeData = (data?: EventData): Record<string, unknown> | undefined => {
  if (!data) return undefined;
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(data)) {
    out[k] = v instanceof Error ? toSentoriError(v) : v;
  }
  return out;
};

const toSentoriError = (e: Error): SentoriError => ({
  type: e.name || 'Error',
  message: e.message,
  stack: parseStack(e.stack),
  cause:
    e.cause instanceof Error
      ? toSentoriError(e.cause)
      : null,
});

const platformOf = (): 'android' | 'ios' | 'javascript' => {
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const RN = require('react-native') as { Platform?: { OS?: string } };
    const os = RN.Platform?.OS;
    if (os === 'ios') return 'ios';
    if (os === 'android') return 'android';
  } catch {
    // headless / test environment
  }
  return 'javascript';
};

type EmitOptions = {
  name?: string;
  surface?: Surface;
  error?: SentoriError;
  data?: EventData;
  /** Attach the signal-ring snapshot (error/warn: yes; the light
   *  kinds ship without it). */
  withSignals?: boolean;
};

/** Assemble + enqueue one event. The single funnel every verb uses. */
const emit = (kind: EventKind, opts: EmitOptions): string => {
  const id = uuidV7();
  const config = getConfig();
  if (!config || !config.enabled) return id; // no-op before init — iron rule

  const payload: WirePayload = {};
  if (opts.error) payload.error = opts.error;
  const data = serializeData(opts.data);
  if (data) payload.data = data;
  const ctx = currentContext();
  if (ctx) payload.context = ctx;
  if (opts.withSignals) {
    const signals = snapshotSignals();
    if (signals.length > 0) payload.signals = signals;
  }
  const device = collectDevice();
  if (device) payload.device = device;

  let event: WireEvent = {
    id,
    kind,
    occurredAt: new Date().toISOString(),
    platform: platformOf(),
    release: config.release,
    environment: config.environment,
    name: opts.name,
    surface: opts.surface,
    userKey: currentUserKey(),
    payload,
  };

  if (config.beforeSend) {
    try {
      const out = config.beforeSend(event);
      if (out === null) return id; // deliberate drop
      if (out && typeof out === 'object') event = out;
    } catch {
      // Hook broke: the un-mutated event ships. The host's bug must
      // not cost them the crash report.
    }
  }

  // In dev there is no uploaded source map, so without local
  // symbolication errors land as `entry.bundle:721724`. Hold the
  // event out of the batch until Metro's /symbolicate answers
  // (bounded at 2 s inside, never throws) — mutating it after
  // enqueue would race the flush timer. Release builds and
  // error-free events enqueue synchronously as before.
  if (isDevRuntime() && hasErrorShape(payload)) {
    void devSymbolicateThenEnqueue(event);
  } else {
    enqueue(event);
  }
  onEventEmitted(event);
  return id;
};

const isDevRuntime = (): boolean =>
  typeof __DEV__ !== 'undefined' && !!__DEV__;

const hasErrorShape = (payload: WirePayload): boolean => {
  if (payload.error) return true;
  return Object.values(payload.data ?? {}).some(
    (v) => !!v && typeof v === 'object' && Array.isArray((v as SentoriError).stack),
  );
};

const devSymbolicateThenEnqueue = async (event: WireEvent): Promise<void> => {
  try {
    const { error: err, data } = event.payload;
    if (err) await symbolicateErrorViaMetro(err);
    for (const v of Object.values(data ?? {})) {
      // The error-in-data convention: serialized errors riding warn/
      // trace data get the same treatment as the headline error.
      if (!!v && typeof v === 'object' && Array.isArray((v as SentoriError).stack)) {
        await symbolicateErrorViaMetro(v as SentoriError);
      }
    }
  } catch {
    // Symbolication is garnish; delivery is the contract.
  }
  enqueue(event);
};

// ── the verbs ──────────────────────────────────────────────────────

export const error = safeFn('error', (err: unknown, data?: EventData): string => {
  const coerced = coerceError(err);
  return emit('error', {
    error: toSentoriError(coerced),
    data,
    withSignals: true,
  });
});

export const warn = safeFn(
  'warn',
  (name: string, data?: EventData): string => {
    // A hand-written warn's surface can ride in data.surface; the
    // detected scenarios pass it explicitly via warnDetected.
    const surface =
      data && typeof data.surface === 'object'
        ? (data.surface as Surface)
        : undefined;
    return emit('warn', { name, surface, data, withSignals: true });
  },
);

/** SDK-internal: a detected warn scenario with an explicit surface. */
export const warnDetected = (
  scenario: string,
  surface: Surface,
  data?: EventData,
): string =>
  emit('warn', { name: scenario, surface, data, withSignals: true });

export const trace = safeFn(
  'trace',
  (name: string, data?: EventData, opts?: TraceOptions): string => {
    // Every trace is context: it lands in the ring regardless.
    pushSignal('trace', { name, ...(data ?? {}) });
    if (opts?.quiet) return uuidV7();
    return emit('trace', { name, data });
  },
);

export const assert = safeFn(
  'assert',
  (name: string, ok: boolean, data?: EventData): string => {
    const config = getConfig();
    if (config) countAssert(name, ok, config.release);
    if (ok) return uuidV7(); // passes aggregate; they are never events
    return emit('assert', { name, data, withSignals: true });
  },
);

export const probe = safeFn('probe', (ref: string, data?: EventData): string => {
  // A tripwire: reaching this call IS the signal. Never throws,
  // never changes control flow (design.md §4).
  return emit('probe', { name: ref, data });
});

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
};
