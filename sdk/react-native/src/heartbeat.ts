// Analytics v1 — concurrent-user heartbeat.
//
// Once per minute, while the app is in the foreground, POST a tiny
// `{ sessionId, userId?, release, route?, os?, ts }` body to
// `/v1/heartbeat`. The server keeps a per-project Valkey ZSET keyed
// by member (user.id when set, sessionId otherwise) and the
// dashboard's `Live` page reads the set to render concurrent-user
// count + per-dim breakdowns.
//
// Iron-rule budget (CLAUDE.md):
// - 1 POST / min foreground only — never fires when backgrounded
// - ~200 B body
// - fire-and-forget; never blocks the JS thread, no retries
// - bounce suppression: if AppState flaps active → inactive → active
//   inside 30 s, we still fire at most one heartbeat per 30 s
//
// The heartbeat is independent of the session ping in
// `session-tracker.ts`. Session pings fire only at session close
// (transport-batched); the heartbeat exists *during* the session to
// signal presence.

import { logger } from '@goliapkg/sentori-core';

import { getConfig } from './config';
import { getUser } from './capture';
import { getLastRoute } from './navigation';
import { uuidV7 } from './uuid';

declare const __DEV__: boolean | undefined;

const DEFAULT_INTERVAL_MS = 60_000;
const MIN_GAP_MS = 30_000;

type AppStateLike = {
  addEventListener: (
    event: 'change',
    handler: (state: string) => void
  ) => { remove: () => void };
  currentState?: string;
};

let _running = false;
let _timer: ReturnType<typeof setInterval> | null = null;
let _appStateSub: null | { remove: () => void } = null;
let _sessionId: null | string = null;
let _lastBeatTs = 0;
let _intervalMs = DEFAULT_INTERVAL_MS;

export type HeartbeatOptions = {
  /** Override the default 60 s interval. Floor 10 s — anything below
   *  trips the perf rule and the server's rate-limit anyway. */
  intervalMs?: number;
};

export function startHeartbeat(opts: HeartbeatOptions = {}): void {
  if (_running) return;
  _running = true;
  _intervalMs = Math.max(10_000, opts.intervalMs ?? DEFAULT_INTERVAL_MS);
  _sessionId = uuidV7();

  // AppState gate — only beat while app is in the foreground.
  let AppState: AppStateLike | undefined;
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    AppState = (require('react-native') as { AppState?: AppStateLike }).AppState;
  } catch {
    // Not in RN runtime (tests). The interval still runs; the gate
    // is just permissive. Suppression below still applies.
    AppState = undefined;
  }

  const isForeground = (): boolean => {
    if (!AppState) return true;
    return (AppState.currentState ?? 'active') === 'active';
  };

  const beat = () => {
    if (!_running) return;
    if (!isForeground()) return;
    const now = Date.now();
    if (now - _lastBeatTs < MIN_GAP_MS) return;
    _lastBeatTs = now;
    void send();
  };

  // First beat as soon as we start (so the dashboard sees the user
  // immediately, not 60 s after launch). Subsequent fires on the
  // interval. AppState transitions can poke an immediate beat too —
  // an active resume is a meaningful presence event.
  beat();
  _timer = setInterval(beat, _intervalMs);

  if (AppState && typeof AppState.addEventListener === 'function') {
    _appStateSub = AppState.addEventListener('change', (state) => {
      if (state === 'active') beat();
    });
  }
}

export function stopHeartbeat(): void {
  _running = false;
  if (_timer !== null) {
    clearInterval(_timer);
    _timer = null;
  }
  if (_appStateSub) {
    _appStateSub.remove();
    _appStateSub = null;
  }
  _sessionId = null;
  _lastBeatTs = 0;
}

async function send(): Promise<void> {
  const config = getConfig();
  if (!config) return;
  const user = getUser();
  const body: Record<string, unknown> = {
    sessionId: _sessionId ?? '',
    release: config.release,
    ts: Date.now(),
  };
  if (user?.id) body.userId = user.id;
  const route = getLastRoute();
  if (route) body.route = route;
  const os = readOsString();
  if (os) body.os = os;

  try {
    await fetch(`${config.ingestUrl}/v1/heartbeat`, {
      body: JSON.stringify(body),
      headers: {
        Authorization: `Bearer ${config.token}`,
        'Content-Type': 'application/json',
      },
      method: 'POST',
    });
  } catch (e) {
    logger.debug('heartbeat', 'failed (best-effort, normal on offline)', e);
  }
}

function readOsString(): null | string {
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const RN = require('react-native') as {
      Platform: { OS: string; Version: string | number };
    };
    return `${RN.Platform.OS} ${RN.Platform.Version}`;
  } catch {
    return null;
  }
}

export function __resetHeartbeatForTests(): void {
  stopHeartbeat();
}
