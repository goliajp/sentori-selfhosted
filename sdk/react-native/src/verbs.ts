// The five event verbs. Everything here is
// synchronous, never throws, and returns the client-minted event id
// — the zero-cost iron rule made code.
//
//   sentori.error(err)    出了什么事?
//   sentori.warn(name)    用户哪里不舒服?
//   sentori.trace(name)   这里发生了什么?
//   sentori.assert(n, ok) 这里应该成立吗?
//   sentori.probe(ref)    那个 bug 回来了吗?

import {
  applyBeforeSend,
  buildWireEvent,
  coerceError,
  platformOrFallback,
  pushSignal,
  toSentoriError,
  safeFn,
  snapshotSignals,
  uuidV7,
} from '@goliapkg/sentori-core';
import type {
  EventData,
  EventKind,
  Platform,
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

const detectPlatform = (): 'android' | 'ios' | 'javascript' => {
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

// The server gets the last word: one that refuses what we detect has
// told us it predates this SDK, and the transport drops the session
// to a value it does accept.
export const platformOf = (): Platform => platformOrFallback(detectPlatform());

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

  // The shape goes through the kernel; what fills it is this
  // runtime's business. Which kinds carry the signal ring stays a
  // decision of the verb layer, so the snapshot is taken here and
  // handed over rather than inferred there.
  let event: WireEvent = buildWireEvent({
    id,
    kind,
    platform: platformOf(),
    release: config.release,
    environment: config.environment,
    name: opts.name,
    surface: opts.surface,
    userKey: currentUserKey(),
    error: opts.error,
    data: opts.data,
    context: currentContext(),
    signals: opts.withSignals ? snapshotSignals() : undefined,
    device: collectDevice() ?? undefined,
  });

  const kept = applyBeforeSend(event, config.beforeSend);
  if (kept === null) return id; // the host dropped it deliberately
  event = kept;
  const payload = event.payload;

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

/**
 * The SDK's own fault, filed where the host can see it.
 *
 * `reportInternal` has always existed and `setInternalReporter` has
 * always been exported, and nothing ever called the setter — so every
 * internal failure went to the console and stopped there. A crash
 * reporter whose own faults are invisible is asking to be trusted on
 * the word of the thing that broke.
 *
 * `warn`, not `error`: the host app did not fail, we did, and filing
 * it as an error puts our bug at the top of their inbox looking like
 * theirs. The name is fixed so one query finds all of them.
 *
 * No signal ring and no surface. The ring is context for the host's
 * failure, and spending a screen lookup inside an already-failing path
 * is how a fault handler becomes a second fault.
 */
export const internalFault = (report: {
  api: string;
  message: string;
  errorName?: string;
  stack?: string;
}): void => {
  emit('warn', {
    name: 'sentori.internal',
    data: {
      api: report.api,
      message: report.message,
      ...(report.errorName ? { errorName: report.errorName } : {}),
      ...(report.stack ? { stack: report.stack } : {}),
    },
  });
};

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
  // never changes control flow.
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
