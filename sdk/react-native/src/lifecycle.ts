// v2.0 W3 — top-level lifecycle: flush / close.
//
// Sentry / OTel parity: a single `flush(timeoutMs?)` that drains
// every in-flight buffer (events / metrics / track), gated by a
// timeout so a slow network doesn't hang the host's shutdown path.
//
// Use before short-lived process exit (CLI / serverless function /
// fixture cleanup) to ensure pending captures land before the
// process dies.

import { flush as flushTransport } from './transport';
import { flushMetrics } from './metrics';
import { flushTrack } from './track';
import { reportInternal } from '@goliapkg/sentori-core';

let _closed = false;

/**
 * Force-flush every pending Sentori buffer (events, metrics,
 * track). Returns when the flush settles or the timeout fires —
 * whichever happens first.
 *
 * Never rejects: per the NEVER rule, individual flush failures
 * are silently absorbed (and self-reported via the internal
 * circuit-breaker), and the resolved promise's value is undefined.
 *
 *     await sentori.flush(5_000)   // wait up to 5 s, then move on
 *     process.exit(0)
 */
export async function flush(timeoutMs: number = 5_000): Promise<void> {
  if (_closed) return;
  try {
    const drainAll = Promise.all([
      flushTransport().catch((err) => {
        reportInternal('flush.transport', err);
      }),
      flushMetrics().catch((err) => {
        reportInternal('flush.metrics', err);
      }),
      flushTrack().catch((err) => {
        reportInternal('flush.track', err);
      }),
    ]).then(() => undefined);
    const timer = new Promise<void>((resolve) => setTimeout(resolve, timeoutMs));
    await Promise.race([drainAll, timer]);
  } catch (err) {
    // Belt-and-braces: even though each branch already has its own
    // catch, the wrapping race shouldn't be able to surface a
    // failure to the host either. NEVER rule.
    reportInternal('flush', err);
  }
}

/**
 * Flush + shut down. After `close()`, further capture* calls remain
 * silent no-ops via `_closed` gate. Idempotent — re-calling is safe.
 *
 *     await sentori.close()
 *     // SDK is asleep; no more events go out.
 */
export async function close(timeoutMs?: number): Promise<void> {
  await flush(timeoutMs);
  _closed = true;
}

/** Test hook — reset the shutdown latch so unit tests don't have to
 *  re-init the whole SDK between cases. */
export function __resetLifecycleForTests(): void {
  _closed = false;
}

/** Internal — capture* implementations check this to short-circuit
 *  after `close()`. */
export function isClosed(): boolean {
  return _closed;
}
