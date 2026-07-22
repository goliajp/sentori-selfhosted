// v0.8.3 — custom metrics buffer.
//
// `recordMetric(name, value, tags?)` pushes a point into a fixed-size
// ring. A timer flushes the ring every 30 s (or when the buffer is
// full); captureException also forces a flush so the metrics line up
// with the error event in the dashboard. Best-effort: a flush failure
// drops the batch on the floor — metrics aren't critical telemetry.
//
// Why not one fetch per point: noisy loops (`recordMetric('frame', 1)`
// in a render hook) would burn the JS thread + saturate the device's
// outgoing connection pool. Batching makes the SDK safe to use as a
// cheap counter primitive.

import type { SpanContextLike } from '@goliapkg/sentori-core';

import { getConfig, isInitialized } from './config';
import { sendMetricsBatch } from './transport';

type Point = {
  name: string;
  tags?: Record<string, string>;
  ts: string;
  value: number;
};

const MAX_BUFFER = 500;
const FLUSH_INTERVAL_MS = 30_000;

let _buf: Point[] = [];
let _timer: ReturnType<typeof setInterval> | null = null;

/**
 * Record a numeric metric point. Cheap to call from a render hook —
 * pushes into a 500-slot ring drained every 30 s (or on overflow).
 *
 * v2.0 — accepts an optional `parent: SpanContextLike` so a metric
 * can be correlated to a span. The dashboard span-detail view joins
 * metric points by `tags.span_id` / `tags.trace_id`.
 *
 *     const span = sentori.startSpan({ name: 'db.query users' })
 *     sentori.recordMetric('db.query.duration_ms', 42, undefined, { parent: span })
 *     span.end({ status: 'ok' })
 */
export function recordMetric(
  name: string,
  value: number,
  tags?: Record<string, string>,
  opts?: { parent?: SpanContextLike },
): void {
  if (!isInitialized()) return;
  if (typeof name !== 'string' || name.length === 0 || name.length > 200) return;
  if (typeof value !== 'number' || !Number.isFinite(value)) return;
  if (tags && Object.keys(tags).length > 20) return;
  // Merge span-context tags so the dashboard can join metric points
  // to the span that produced them without a separate schema column.
  const finalTags: Record<string, string> | undefined = opts?.parent
    ? { ...(tags ?? {}), span_id: opts.parent.spanId, trace_id: opts.parent.traceId }
    : tags;
  _buf.push({ name, tags: finalTags, ts: new Date().toISOString(), value });
  if (_buf.length >= MAX_BUFFER) {
    void flushMetrics();
  }
}

export async function flushMetrics(): Promise<void> {
  if (_buf.length === 0) return;
  const config = getConfig();
  if (!config) return;
  const batch = _buf;
  _buf = [];
  await sendMetricsBatch(config.ingestUrl, config.token, batch);
}

/**
 * Start the 30 s flush timer. Called once from `init()`. Idempotent.
 * `clearMetricsTimer` is exposed for tests / teardown.
 */
export function startMetricsTimer(): void {
  if (_timer !== null) return;
  _timer = setInterval(() => {
    void flushMetrics();
  }, FLUSH_INTERVAL_MS);
  // Don't keep the process alive solely for this timer (Node would).
  // In RN setInterval is a NoopRef so this is harmless there.
  (_timer as unknown as { unref?: () => void }).unref?.();
}

export function __resetMetricsForTests(): void {
  if (_timer !== null) {
    clearInterval(_timer);
    _timer = null;
  }
  _buf = [];
}
