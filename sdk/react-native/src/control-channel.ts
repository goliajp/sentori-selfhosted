// v1.1 +S7 升级 — SDK control channel for sub-second live debug.
//
// Every 30 s the SDK polls `/v1/control/poll?userId=<current>` and
// reads back `{ liveMode: boolean, ttlMs: number }`. While live mode
// is on, the transport flushes events immediately (BATCH_SIZE = 1,
// FLUSH_INTERVAL = 0) so dashboard's live-debug viewer sees each
// event with sub-second latency instead of waiting for the 5 s
// batch tick.
//
// When live mode goes off the SDK reverts to its normal batching —
// no permanent overhead, no state to clean up.
//
// Implementation note: the control channel is purely advisory.
// transport.ts reads `isLiveMode()` before each enqueue and decides
// whether to wait for batch or flush right now.

import { getConfig } from './config';
import { getCurrentUserId } from './capture';

const POLL_INTERVAL_MS = 30_000;

/** Cap on the live-mode window even if the server reports a longer
 *  TTL. Defense-in-depth: an unbounded `ttlMs` (server bug, clock
 *  skew, malicious server) would otherwise pin the SDK into flush-
 *  every-event mode indefinitely, which is expensive on slow
 *  networks. 15 min matches the dashboard's default arm length so
 *  normal operation stays uncapped. */
const MAX_LIVE_MODE_TTL_MS = 15 * 60_000;

let _liveMode = false;
let _liveModeUntil = 0;
let _timer: ReturnType<typeof setInterval> | null = null;

export function isLiveMode(): boolean {
  if (!_liveMode) return false;
  // Self-expire if the server-side TTL has passed without a refresh.
  if (Date.now() > _liveModeUntil) {
    _liveMode = false;
    return false;
  }
  return true;
}

export function startControlChannel(): void {
  if (_timer !== null) return;
  // Fire one poll right away so init → setUser → quick captureException
  // can already pick up an existing live-debug session.
  void pollOnce();
  _timer = setInterval(() => {
    void pollOnce();
  }, POLL_INTERVAL_MS);
  (_timer as unknown as { unref?: () => void }).unref?.();
}

export function stopControlChannel(): void {
  if (_timer !== null) {
    clearInterval(_timer);
    _timer = null;
  }
  _liveMode = false;
  _liveModeUntil = 0;
}

async function pollOnce(): Promise<void> {
  const config = getConfig();
  if (!config) return;
  const userId = getCurrentUserId();
  if (!userId) {
    // Without setUser we have no key. Live mode is per-user-id.
    _liveMode = false;
    _liveModeUntil = 0;
    return;
  }
  try {
    const url = `${config.ingestUrl}/v1/control/poll?userId=${encodeURIComponent(userId)}`;
    const resp = await fetch(url, {
      headers: { Authorization: `Bearer ${config.token}` },
      method: 'GET',
    });
    if (!resp.ok) return;
    const body = (await resp.json()) as { liveMode?: boolean; ttlMs?: number };
    if (body.liveMode === true) {
      _liveMode = true;
      _liveModeUntil = Date.now() + Math.max(0, Math.min(body.ttlMs ?? 0, MAX_LIVE_MODE_TTL_MS));
    } else {
      _liveMode = false;
      _liveModeUntil = 0;
    }
  } catch {
    // Network failure → leave the previous state. Self-expiring TTL
    // handles "didn't reach server" gracefully.
  }
}

/** Test-only. */
export function __resetControlChannelForTests(): void {
  stopControlChannel();
}
