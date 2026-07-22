import { addBreadcrumb, drainSpans, logger } from '@goliapkg/sentori-core';

import { getConfig } from './config';
import { isLiveMode } from './control-channel';
import { isAnyNativeModuleLinked } from './native-loader';
import type { Event } from './types';

const FLUSH_INTERVAL_MS = 5_000;
const BATCH_SIZE = 10;
const MAX_RETRY = 3;
const STORAGE_KEY = '@sentori/pending';
const MAX_PERSISTED = 1000;

// Spans are higher-volume and lower-value than error events: we batch
// them on their own timer, cap each request at the server's per-batch
// limit, and drop on failure rather than persisting offline.
const SPAN_FLUSH_INTERVAL_MS = 10_000;
const SPAN_BATCH_MAX = 200;

let _queue: Event[] = [];
let _flushTimer: ReturnType<typeof setTimeout> | null = null;
let _spanTimer: ReturnType<typeof setInterval> | null = null;
let _started = false;

const SDK_VERSION = '0.0.0';

export const enqueue = (event: Event): void => {
  _queue.push(event);
  // v1.1 +S7 升级 — when the dashboard has armed live-debug for the
  // current user, flush immediately instead of waiting for the 5 s
  // batch interval. Dashboard sees each event with sub-second latency.
  if (isLiveMode()) {
    void flush();
    return;
  }
  if (_queue.length >= BATCH_SIZE) {
    void flush();
  } else if (!_flushTimer) {
    _flushTimer = setTimeout(() => {
      _flushTimer = null;
      void flush();
    }, FLUSH_INTERVAL_MS);
  }
};

export const startTransport = (): void => {
  _started = true;
  if (!_spanTimer) {
    _spanTimer = setInterval(() => {
      void flushSpans();
    }, SPAN_FLUSH_INTERVAL_MS);
  }
};

export const flushSpans = async (): Promise<void> => {
  if (!_started) return;
  const spans = drainSpans();
  if (spans.length === 0) return;
  const config = getConfig();
  if (!config) return;
  for (let i = 0; i < spans.length; i += SPAN_BATCH_MAX) {
    const chunk = spans.slice(i, i + SPAN_BATCH_MAX);
    try {
      await sendSpansOnce(chunk, config.ingestUrl, config.token);
    } catch {
      // drop the remaining chunks — span uploads aren't worth retrying
      break;
    }
  }
};

const sendSpansOnce = async (
  spans: unknown[],
  ingestUrl: string,
  token: string,
): Promise<void> => {
  const resp = await fetch(`${ingestUrl}/v1/spans:batch`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
      'Sentori-Sdk': `react-native/${SDK_VERSION}`,
    },
    body: JSON.stringify({ spans }),
  });
  // 5xx: server overloaded. 4xx (bad token / quota / oversized): also
  // pointless to keep sending. Either way, stop the rest of the batch.
  if (resp.status >= 400) throw new Error(`spans-${resp.status}`);
};

export const flush = async (): Promise<void> => {
  if (!_started) return;
  if (_queue.length === 0) return;

  const config = getConfig();
  if (!config) return;

  const batch = _queue.splice(0, _queue.length);
  if (_flushTimer) {
    clearTimeout(_flushTimer);
    _flushTimer = null;
  }

  try {
    await sendWithRetry(batch, config.ingestUrl, config.token);
  } catch {
    await persist(batch);
  }
};

const sendWithRetry = async (
  events: Event[],
  ingestUrl: string,
  token: string,
): Promise<void> => {
  let attempt = 0;
  let delayMs = 1000;
  while (true) {
    try {
      await sendOnce(events, ingestUrl, token);
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
  events: Event[],
  ingestUrl: string,
  token: string,
): Promise<void> => {
  const url =
    events.length === 1 ? `${ingestUrl}/v1/events` : `${ingestUrl}/v1/events:batch`;
  const body = events.length === 1 ? events[0] : { events };

  const resp = await fetch(url, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
      'Sentori-Sdk': `react-native/${SDK_VERSION}`,
    },
    body: JSON.stringify(body),
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
  // 4xx other than 429 = client error, drop silently
};

const sleep = (ms: number): Promise<void> =>
  new Promise((r) => setTimeout(r, ms));

type AsyncStorageLike = {
  getItem(key: string): Promise<string | null>;
  setItem(key: string, value: string): Promise<void>;
  removeItem(key: string): Promise<void>;
};

const getAsyncStorage = async (): Promise<AsyncStorageLike | null> => {
  // v0.8.5 — host may have the JS package without pod install /
  // prebuild → getItem crashes from a microtask outside our reach.
  if (!isAnyNativeModuleLinked(['RNCAsyncStorage', 'AsyncStorageModule'])) {
    return null;
  }
  try {
    // Resolve via the host's runtime `require` rather than `import()`.
    // `import()` is type-checked at build time (TS6 strict-mode); the
    // peer dep isn't installed in monorepo-root CI, which made `bun
    // run build:sdks` fail to find the type declarations. The peer is
    // optional at runtime anyway — the isAnyNativeModuleLinked guard
    // above already returned `null` if the package isn't installed.
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const mod = require('@react-native-async-storage/async-storage') as {
      default?: AsyncStorageLike;
    } & AsyncStorageLike;
    return mod.default ?? mod;
  } catch {
    return null;
  }
};

const persist = async (events: Event[]): Promise<void> => {
  const AsyncStorage = await getAsyncStorage();
  if (!AsyncStorage) return;
  try {
    const existing = await AsyncStorage.getItem(STORAGE_KEY);
    const prev: Event[] = existing ? JSON.parse(existing) : [];
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
    const events: Event[] = JSON.parse(raw);
    for (const e of events) _queue.push(e);
    await flush();
  } catch {
    // best-effort
  }
};

export const __resetForTests = (): void => {
  _queue = [];
  if (_flushTimer) clearTimeout(_flushTimer);
  _flushTimer = null;
  if (_spanTimer) clearInterval(_spanTimer);
  _spanTimer = null;
  _started = false;
};

export const __peekQueue = (): readonly Event[] => _queue;

/**
 * Phase 26 sub-B: session ping transport. Best-effort; we don't queue
 * pings the way we queue events because they fire on background and
 * AsyncStorage writes during background can be killed by the OS. If
 * the network's down, the ping is lost — the session counters tolerate
 * this.
 */
export const sendSessionPing = async (
  ingestUrl: string,
  token: string,
  ping: unknown
): Promise<void> => {
  try {
    await fetch(`${ingestUrl}/v1/sessions`, {
      body: JSON.stringify(ping),
      headers: {
        Authorization: `Bearer ${token}`,
        'Content-Type': 'application/json',
        'Sentori-Sdk': `react-native/${SDK_VERSION}`,
      },
      method: 'POST',
    });
  } catch {
    // best-effort
  }
};

/**
 * v1.1 chunk B — flush a batched set of `sentori.track()` analytics
 * events. The host SDK batches calls into a fixed-size ring drained
 * every 30 s. Best-effort; a failure drops the batch.
 */
export const sendTrackBatch = async (
  ingestUrl: string,
  token: string,
  events: Array<{
    environment?: string;
    name: string;
    props?: Record<string, unknown>;
    release?: string;
    route?: string;
    sessionId?: string;
    ts: string;
    userId?: string;
  }>,
): Promise<void> => {
  if (events.length === 0) return;
  try {
    await fetch(`${ingestUrl}/v1/track:batch`, {
      body: JSON.stringify({ events }),
      headers: {
        Authorization: `Bearer ${token}`,
        'Content-Type': 'application/json',
        'Sentori-Sdk': `react-native/${SDK_VERSION}`,
      },
      method: 'POST',
    });
  } catch {
    // best-effort
  }
};

/**
 * v0.8.3 — flush a batched set of custom metrics. The host SDK
 * batches recordMetric() calls into a fixed-size ring (drained on
 * a timer + at next captureException) so a busy loop doesn't spin
 * up one fetch per point. Best-effort, no retry.
 */
export const sendMetricsBatch = async (
  ingestUrl: string,
  token: string,
  metrics: Array<{
    name: string;
    tags?: Record<string, string>;
    ts?: string;
    value: number;
  }>,
): Promise<void> => {
  if (metrics.length === 0) return;
  try {
    await fetch(`${ingestUrl}/v1/metrics:batch`, {
      body: JSON.stringify({ metrics }),
      headers: {
        Authorization: `Bearer ${token}`,
        'Content-Type': 'application/json',
        'Sentori-Sdk': `react-native/${SDK_VERSION}`,
      },
      method: 'POST',
    });
  } catch {
    // best-effort
  }
};

/**
 * v2.1 W2 — POST a batched set of auto-instrument runtime metric
 * points. Sibling of sendMetricsBatch; different endpoint
 * (`/v1/runtime-metrics:batch`) because the storage shape +
 * validation rules + rate-limit budget differ — see
 * docs/design/v2-metrics.md.
 *
 * Returns true on 2xx so the caller can leave the batch drained;
 * returns false on anything else (network error / non-2xx) so the
 * caller rebuffer-and-retries on the next flush via
 * `rebufferRuntimeMetrics(batch)`.
 */
export const sendRuntimeMetricsBatch = async (
  ingestUrl: string,
  token: string,
  metrics: Array<{
    name: string;
    tags?: Record<string, string>;
    ts: string;
    value: number;
  }>,
): Promise<boolean> => {
  if (metrics.length === 0) return true;
  try {
    const resp = await fetch(`${ingestUrl}/v1/runtime-metrics:batch`, {
      body: JSON.stringify({ metrics }),
      headers: {
        Authorization: `Bearer ${token}`,
        'Content-Type': 'application/json',
        'Sentori-Sdk': `react-native/${SDK_VERSION}`,
      },
      method: 'POST',
    });
    return resp.ok;
  } catch {
    return false;
  }
};

/**
 * v0.8.2 — submit a user-supplied bug report. Fire-and-forget; resolves
 * with the server-assigned id on success or `null` on any failure.
 * The host app typically calls this from a "Report a problem" form;
 * pass `eventId` if you're reporting a specific crash the user just
 * saw so the report links to that event's issue automatically.
 */
export const sendUserReport = async (
  ingestUrl: string,
  token: string,
  report: {
    body: string;
    email?: string;
    eventId?: string;
    name?: string;
    title: string;
  },
): Promise<null | { id: string; issueId: null | string }> => {
  try {
    const resp = await fetch(`${ingestUrl}/v1/user-reports`, {
      body: JSON.stringify(report),
      headers: {
        Authorization: `Bearer ${token}`,
        'Content-Type': 'application/json',
        'Sentori-Sdk': `react-native/${SDK_VERSION}`,
      },
      method: 'POST',
    });
    if (!resp.ok) return null;
    const j = (await resp.json()) as { id: string; issueId: null | string };
    return j;
  } catch {
    return null;
  }
};

// ──────────────────────────────────────────────────────────────────
// Phase 42 sub-D.05 — attachment upload pipeline
// ──────────────────────────────────────────────────────────────────

/**
 * Upload a base64-encoded binary blob as an attachment for a known
 * event. The event must NOT have been POSTed yet — the server-side
 * ingest validation in events.rs only honours `event.attachments[].ref`
 * when the matching `event_attachments` row already exists for the
 * same (event_id, project_id). Caller's contract:
 *
 *   1. Generate `event.id` (uuidV7).
 *   2. Build the blob (e.g. via `captureScreenshot`).
 *   3. `await uploadAttachment(...)` → get `{ ref, sizeBytes, mediaType }`.
 *   4. Push `{ ref, kind, ... }` into `event.attachments` then enqueue.
 *
 * Returns `null` on any non-fatal failure (network down, store
 * disabled, 4xx, timeout). The error event still ships without the
 * attachment so we never lose the actual crash.
 */
export const uploadAttachment = async (
  eventId: string,
  kind: import('./types').AttachmentMeta['kind'],
  blob: { base64: string; mediaType: string },
  opts: { source?: 'android' | 'ios' | 'js' } = {},
): Promise<import('./types').AttachmentMeta | null> => {
  const config = getConfig();
  if (!config) return null;
  const url = `${config.ingestUrl}/v1/events/${encodeURIComponent(eventId)}/attachments/${encodeURIComponent(kind)}`;

  // RN-style multipart: `{ uri, type, name }` is what the native
  // FormData implementation expects for a file part — the bridge
  // serializes a data: URI without us having to allocate a Blob.
  const form = new FormData();
  form.append(
    'file',
    {
      name: filenameFor(kind, blob.mediaType),
      type: blob.mediaType,
      uri: `data:${blob.mediaType};base64,${blob.base64}`,
    } as unknown as Blob,
  );
  form.append('source', opts.source ?? 'js');

  try {
    const resp = await fetch(url, {
      body: form,
      headers: {
        Authorization: `Bearer ${config.token}`,
        'Sentori-Sdk': `react-native/${SDK_VERSION}`,
      },
      method: 'POST',
    });
    // Phase 48 sub-A: accept any 2xx instead of strict 201. Reverse
    // proxies in front of ingest occasionally rewrite 201 → 202 (the
    // exact symptom Insight observed), and a 200 is also a valid
    // "stored" response. We still require a JSON body shaped like
    // UploadResponse; non-JSON bodies fall through to null.
    if (resp.status < 200 || resp.status >= 300) {
      noteAttachmentFailure(eventId, kind, `http_${resp.status}`);
      // rc.6 — surface the status in dev so Insight-style triage
      // doesn't have to guess between 413/422/500. Pre-rc.6 only
      // the breadcrumb carried the reason; logcat only saw the
      // generic `upload returned null` line.
      logger.debug('transport', 'attachment upload non-2xx',
          'eventId=', eventId,
          'kind=', kind,
          'status=', resp.status,
        );
      return null;
    }
    const j = (await resp.json().catch(() => null)) as null | {
      refId: string;
      sizeBytes: number;
      mediaType: string;
      kind: string;
    };
    if (!j || !j.refId) {
      noteAttachmentFailure(eventId, kind, 'bad_response_body');
      logger.debug('transport', 'attachment upload bad-response-body',
          'eventId=', eventId,
          'kind=', kind,
          'status=', resp.status,
        );
      return null;
    }
    return {
      kind,
      mediaType: j.mediaType,
      ref: j.refId,
      sizeBytes: j.sizeBytes,
      source: opts.source ?? 'js',
    };
  } catch (e) {
    const reason = e instanceof Error ? `fetch_${e.name}` : 'fetch_unknown';
    noteAttachmentFailure(eventId, kind, reason);
    logger.debug('transport', 'attachment upload fetch threw',
        'eventId=', eventId,
        'kind=', kind,
        'reason=', reason,
      );
    return null;
  }
};

/**
 * v0.9.7 — Insight F2 fix-forward. When uploadAttachment silently
 * returns null, the host event ships without the screenshot and the
 * dashboard renders "No attachments captured" — looking identical
 * to "host didn't enable screenshots" and leaving us guessing at
 * which layer broke.
 *
 * Drop a breadcrumb so the next event on the same SDK instance
 * carries a tagged trail entry. Dashboard's breadcrumb panel will
 * show `sentori.attach.failed { eventId, kind, reason }` and we can
 * tell upload-broke from never-tried at a glance.
 */
function noteAttachmentFailure(
  eventId: string,
  kind: string,
  reason: string,
): void {
  addBreadcrumb('custom', {
    category: 'sentori.attach.failed',
    eventId,
    kind,
    reason,
  });
}

function filenameFor(kind: string, mediaType: string): string {
  const ext = mediaType.split('/')[1] ?? 'bin';
  return `${kind}.${ext}`;
}
