// Event transport — the only place the SDK talks to the network.
//
// Quiet by default, complete when it matters (design.md §4): events
// batch on a 5 s timer or a 10-deep queue, whichever first; assert
// pass-counts piggyback on whatever batch goes out next (never their
// own request); failures back off and finally persist to an offline
// queue drained on next launch. Nothing here ever throws into the
// host app.

import type { AssertStat, BatchEnvelope, WireEvent } from '@goliapkg/sentori-core';
import { logger } from '@goliapkg/sentori-core';

import { getConfig } from './config';
import { isAnyNativeModuleLinked } from './native-loader';

const FLUSH_INTERVAL_MS = 5_000;
const BATCH_SIZE = 10;
const MAX_RETRY = 3;
const STORAGE_KEY = '@sentori/pending';
const MAX_PERSISTED = 1000;

// Pinned to package.json by a test — bump both together.
const SDK_VERSION = '5.6.0';

let _queue: WireEvent[] = [];
let _assertStats = new Map<string, AssertStat>();
let _flushTimer: ReturnType<typeof setTimeout> | null = null;
let _started = false;

export const enqueue = (event: WireEvent): void => {
  _queue.push(event);
  if (_queue.length >= BATCH_SIZE) {
    void flush();
  } else if (!_flushTimer) {
    _flushTimer = setTimeout(() => {
      _flushTimer = null;
      void flush();
    }, FLUSH_INTERVAL_MS);
  }
};

/**
 * Count an assert outcome. Pass-counts NEVER become events — they
 * aggregate here and ride the next batch envelope (the liveness
 * ledger without a heartbeat flood).
 */
export const countAssert = (name: string, ok: boolean, release: string): void => {
  const key = `${name}${release}`;
  const cur = _assertStats.get(key) ?? { name, release, passDelta: 0, failDelta: 0 };
  if (ok) cur.passDelta += 1;
  else cur.failDelta = (cur.failDelta ?? 0) + 1;
  _assertStats.set(key, cur);
  // Stats with no event traffic still ship eventually, on a lazy
  // timer six times the batch interval.
  if (!_flushTimer && _queue.length === 0) {
    _flushTimer = setTimeout(() => {
      _flushTimer = null;
      void flush();
    }, FLUSH_INTERVAL_MS * 6);
  }
};

export const startTransport = (): void => {
  _started = true;
};

export const flush = async (): Promise<void> => {
  if (!_started) return;
  const config = getConfig();
  if (!config) return;

  const events = _queue.splice(0, _queue.length);
  const stats = [..._assertStats.values()];
  _assertStats = new Map();
  if (_flushTimer) {
    clearTimeout(_flushTimer);
    _flushTimer = null;
  }
  if (events.length === 0 && stats.length === 0) return;

  const envelope: BatchEnvelope = { events };
  if (stats.length > 0) envelope.assertStats = stats;
  if (config.backendHealthUrl) envelope.backendHealthUrl = config.backendHealthUrl;

  try {
    await sendWithRetry(envelope, config.ingestUrl, config.token);
    // Only now do the events exist server-side — release their
    // queued attachments. Uploading before this point 404s: the
    // attachment races the 5s event batch and always wins.
    for (const ev of events) {
      if (ev.id) void sendQueuedAttachments(ev.id);
    }
  } catch {
    // Events survive offline; assert deltas are cheap enough to lose.
    // Their attachments are memory-only and lost with the process —
    // documented; a replay is context, not the crash report itself.
    await persist(events);
  }
};

// ── attachments (deferred until their event is delivered) ─────────

type QueuedAttachment = {
  kind: import('@goliapkg/sentori-core').AttachmentKind;
  blob: { base64?: string; text?: string; mediaType: string };
  source: 'android' | 'ios' | 'js';
};

const _pendingAttachments = new Map<string, QueuedAttachment[]>();

/** Attach a blob to an event that is still in the batch queue. It
 *  uploads right after the batch containing the event lands, so the
 *  server always already knows the event. */
export const queueAttachment = (
  eventId: string,
  kind: QueuedAttachment['kind'],
  blob: QueuedAttachment['blob'],
  opts: { source?: QueuedAttachment['source'] } = {},
): void => {
  const list = _pendingAttachments.get(eventId) ?? [];
  list.push({ kind, blob, source: opts.source ?? 'js' });
  _pendingAttachments.set(eventId, list);
};

const sendQueuedAttachments = async (eventId: string): Promise<void> => {
  const list = _pendingAttachments.get(eventId);
  if (!list) return;
  _pendingAttachments.delete(eventId);
  for (const a of list) {
    await uploadAttachment(eventId, a.kind, a.blob, { source: a.source });
  }
};

const sendWithRetry = async (
  envelope: BatchEnvelope,
  ingestUrl: string,
  token: string,
): Promise<void> => {
  let attempt = 0;
  let delayMs = 1000;
  while (true) {
    try {
      await sendOnce(envelope, ingestUrl, token);
      return;
    } catch (e) {
      attempt++;
      if (attempt >= MAX_RETRY) throw e;
      await sleep(delayMs);
      delayMs *= 2;
    }
  }
};

const sendOnce = async (
  envelope: BatchEnvelope,
  ingestUrl: string,
  token: string,
): Promise<void> => {
  const resp = await fetch(`${ingestUrl}/v1/events:batch`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
      'Sentori-Sdk': `react-native/${SDK_VERSION}`,
    },
    body: JSON.stringify(envelope),
  });

  if (resp.status === 429) {
    let retryAfterMs = 5000;
    try {
      const j = (await resp.json()) as { retryAfterMs?: number };
      if (typeof j.retryAfterMs === 'number') retryAfterMs = j.retryAfterMs;
    } catch {
      // ignore body parse error
    }
    await sleep(retryAfterMs);
    throw new Error('rate-limited');
  }

  if (resp.status >= 500) {
    throw new Error(`server-${resp.status}`);
  }
  // 4xx other than 429 = client error; per-item outcomes are the
  // server's business — drop silently rather than crashloop.
};

const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

type AsyncStorageLike = {
  getItem(key: string): Promise<string | null>;
  setItem(key: string, value: string): Promise<void>;
  removeItem(key: string): Promise<void>;
};

const getAsyncStorage = async (): Promise<AsyncStorageLike | null> => {
  // Host may have the JS package without pod install / prebuild →
  // getItem would crash from a microtask outside our reach.
  if (!isAnyNativeModuleLinked(['RNCAsyncStorage', 'AsyncStorageModule'])) {
    return null;
  }
  try {
    // Resolve via the host's runtime `require` rather than `import()`
    // — the peer dep is optional and absent in monorepo CI.
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const mod = require('@react-native-async-storage/async-storage') as {
      default?: AsyncStorageLike;
    } & AsyncStorageLike;
    return mod.default ?? mod;
  } catch {
    return null;
  }
};

const persist = async (events: WireEvent[]): Promise<void> => {
  if (events.length === 0) return;
  const AsyncStorage = await getAsyncStorage();
  if (!AsyncStorage) return;
  try {
    const existing = await AsyncStorage.getItem(STORAGE_KEY);
    const prev: WireEvent[] = existing ? JSON.parse(existing) : [];
    const merged = [...prev, ...events].slice(-MAX_PERSISTED);
    await AsyncStorage.setItem(STORAGE_KEY, JSON.stringify(merged));
  } catch {
    // best-effort
  }
};

export const drainOfflineQueue = async (): Promise<void> => {
  const AsyncStorage = await getAsyncStorage();
  if (!AsyncStorage) return;
  try {
    const raw = await AsyncStorage.getItem(STORAGE_KEY);
    if (!raw) return;
    await AsyncStorage.removeItem(STORAGE_KEY);
    const events: WireEvent[] = JSON.parse(raw);
    for (const e of events) _queue.push(e);
    await flush();
  } catch {
    // best-effort
  }
};

/**
 * Upload one attachment for an already-enqueued event. Returns null
 * on any non-fatal failure — the event still ships without the
 * attachment so the crash itself is never lost.
 */
export const uploadAttachment = async (
  eventId: string,
  kind: import('@goliapkg/sentori-core').AttachmentKind,
  blob: { base64?: string; text?: string; mediaType: string },
  opts: { source?: 'android' | 'ios' | 'js' } = {},
): Promise<{ ref: string } | null> => {
  const config = getConfig();
  if (!config) return null;
  const url = `${config.ingestUrl}/v1/events/${encodeURIComponent(eventId)}/attachments/${encodeURIComponent(kind)}`;

  // Hand-built multipart. React Native's FormData file part wants a
  // `uri`, and its `data:` URI form throws a bare network error on
  // iOS — this shipped untested and every JS attachment silently
  // died. Text payloads (replay/screens NDJSON) embed directly;
  // base64 payloads embed as base64 with the transfer-encoding
  // header so the server knows to decode.
  const boundary = `----sentori-${eventId}`;
  const isText = typeof blob.text === 'string';
  const content = isText ? (blob.text ?? '') : (blob.base64 ?? '');
  const encodingHeader = isText ? '' : 'Content-Transfer-Encoding: base64\r\n';
  const wireBody =
    `--${boundary}\r\n` +
    `Content-Disposition: form-data; name="file"; filename="${kind}.bin"\r\n` +
    `Content-Type: ${blob.mediaType}\r\n` +
    encodingHeader +
    `\r\n${content}\r\n` +
    `--${boundary}\r\n` +
    `Content-Disposition: form-data; name="source"\r\n` +
    `\r\n${opts.source ?? 'js'}\r\n` +
    `--${boundary}--\r\n`;

  try {
    const resp = await fetch(url, {
      body: wireBody,
      headers: {
        Authorization: `Bearer ${config.token}`,
        'Content-Type': `multipart/form-data; boundary=${boundary}`,
        'Sentori-Sdk': `react-native/${SDK_VERSION}`,
      },
      method: 'POST',
    });
    if (resp.status < 200 || resp.status >= 300) {
      logger.warn(`attachment ${kind} upload http_${resp.status}`);
      return null;
    }
    const body = (await resp.json()) as { refId?: string };
    return body.refId ? { ref: body.refId } : null;
  } catch (e) {
    logger.warn(`attachment ${kind} upload failed: ${String(e)}`);
    return null;
  }
};

export const __resetForTests = (): void => {
  _queue = [];
  _assertStats = new Map();
  if (_flushTimer) clearTimeout(_flushTimer);
  _flushTimer = null;
  _started = false;
};

export const __peekQueue = (): readonly WireEvent[] => _queue;
export const __sdkVersion = (): string => SDK_VERSION;
export const __peekAssertStats = (): readonly AssertStat[] => [..._assertStats.values()];
