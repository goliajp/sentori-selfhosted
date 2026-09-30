// Drive the core transport against a real Sentori server.
//
//   node scripts/lib/core-live-driver.mjs <baseUrl> <token> <idsOutFile>
//
// The ids go to a file, not to stdout. Captured in a `$(...)` the
// whole run was silent when it failed — a red gate that cannot say
// what it saw trains whoever hits it to re-run instead of read.
//
// The transport moved out of the React Native package so that web and
// mini-program SDKs share one queueing / backoff / offline
// implementation. Every test of it until now used a fake `fetch`, and
// a fake agrees with whatever mistake the envelope is making — which
// is how a wire format diverges quietly. The one thing that cannot
// agree with a mistake is the server.
//
// The offline path especially: it has never once been exercised
// against a real endpoint. It is the path that runs when the network
// is down, which is exactly when nobody is watching.

import { writeFileSync } from 'node:fs';

import { createTransport } from '../../sdk/core/lib/index.js';

const [base, token, idsOut] = process.argv.slice(2);
if (!base || !token || !idsOut) {
  console.error('usage: core-live-driver.mjs <baseUrl> <token> <idsOutFile>');
  process.exit(1);
}

/** The storage a host provides. In-memory here: what matters is that
 *  the transport puts a failed batch somewhere and takes it back out,
 *  not which box it used. */
const box = new Map();
const storage = {
  getItem: async (k) => box.get(k) ?? null,
  setItem: async (k, v) => void box.set(k, v),
  removeItem: async (k) => void box.delete(k),
};

let ingestUrl = base;
const transport = createTransport({
  config: () => ({ ingestUrl, token }),
  storage: async () => storage,
  sdkLabel: 'core-live/0.0.0',
  fetch: (...a) => fetch(...a),
});

const event = (kind, name, extra = {}) => ({
  id: crypto.randomUUID(),
  kind,
  occurredAt: new Date().toISOString(),
  platform: 'web',
  release: 'core-live@1.0.0+1',
  environment: 'test',
  name,
  userKey: 'core-live-person',
  payload: {},
  ...extra,
});

const fail = (m) => {
  console.error(`✗ ${m}`);
  process.exit(1);
};

transport.start();

// ── 1. the ordinary path ──────────────────────────────────────────
const direct = event('error', undefined, {
  payload: {
    error: { type: 'CoreLiveError', message: 'from the extracted kernel', stack: [] },
  },
});
transport.enqueue(direct);
transport.countAssert('core.live.assert', true, 'core-live@1.0.0+1');
transport.queueSession({
  id: crypto.randomUUID(),
  status: 'exited',
  release: 'core-live@1.0.0+1',
  environment: 'test',
  platform: 'web',
  startedAt: new Date().toISOString(),
  durationMs: 4200,
  userId: null,
  userKey: 'core-live-person',
});
await transport.flush();
if (transport.peekDropped() !== 0) {
  fail(`the server refused something on the ordinary path: ${transport.peekDropped()} dropped`);
}
console.log('  sent an event, an assert stat and a session');

// ── 2. the offline path ───────────────────────────────────────────
// Point at a port nothing answers on. The batch must end up in
// storage rather than vanish, and must not be counted as dropped —
// "kept for later" and "lost" are different facts and the console
// prints one of them.
ingestUrl = 'http://127.0.0.1:1';
const offline = event('warn', 'core.live.offline');
transport.enqueue(offline);
await transport.flush();

const stored = box.get('@sentori/pending');
if (!stored) fail('a batch that could not be sent left nothing in storage');
const parsed = JSON.parse(stored);
if (!parsed.some((e) => e.id === offline.id)) {
  fail('storage holds a batch, but not the event that failed to send');
}
if (transport.peekDropped() !== 0) {
  fail('an event that was kept for later was also counted as dropped — it would be reported twice');
}
console.log(`  a failed batch persisted ${parsed.length} event(s) instead of losing them`);

// ── 3. and the next launch delivers it ────────────────────────────
ingestUrl = base;
await transport.drainOfflineQueue();
if (box.get('@sentori/pending')) fail('the offline queue was not cleared after a successful drain');
console.log('  the next launch drained it');

writeFileSync(idsOut, JSON.stringify({ directId: direct.id, offlineId: offline.id }));
