// v2.1 W2 part 4 — network bytes auto-instrument.
//
// Two counters (sent / received bytes) incremented by the existing
// fetch patch on every request/response, drained every 30 s as
// runtime.network.bytes_sent + runtime.network.bytes_received.
//
// Best-effort: not every Response exposes `content-length`
// (chunked encoding, server stripped it, gzipped without a
// pre-decode header). For those we report 0 — undercounts vs.
// over-attributing arbitrary numbers. Request body size is
// estimated only when init.body is a string or has `.byteLength`;
// FormData / Blob / ReadableStream are not measured.
//
// XHR is NOT instrumented here — the existing patchXhr() in
// handlers/network.ts does the trace span work, and XHR usage in
// 2026-era RN apps is rare enough that the engineering cost of
// a second patch isn't justified by the missing data. If a host
// app shows up with significant XHR traffic we add it in a patch
// release.

import { emitMetric } from '@goliapkg/sentori-core';

const TICK_MS = 30_000;

let _bytesSent = 0;
let _bytesReceived = 0;
let _timer: null | ReturnType<typeof setInterval> = null;

/** Called from handlers/network.ts on every outbound request.
 *  Cheap — two adds, no allocation. */
export function recordNetworkBytes(sent: number, received: number): void {
  if (sent > 0) _bytesSent += sent;
  if (received > 0) _bytesReceived += received;
}

function estimateBodyBytes(body: BodyInit | null | undefined): number {
  if (body == null) return 0;
  if (typeof body === 'string') return body.length;
  // ArrayBuffer / Uint8Array / ArrayBufferView all carry byteLength.
  const maybe = body as { byteLength?: number };
  if (typeof maybe.byteLength === 'number') return maybe.byteLength;
  // FormData / Blob / ReadableStream — not measured.
  return 0;
}

/** Estimate request bytes from a fetch init. Used by the fetch
 *  patch in handlers/network.ts. */
export function estimateRequestBytes(init?: RequestInit): number {
  return estimateBodyBytes(init?.body);
}

/** Read response bytes from the Content-Length header. Returns
 *  0 if missing or unparseable — undercount-safe. */
export function estimateResponseBytes(headers: Headers | undefined | null): number {
  if (!headers) return 0;
  const v = headers.get?.('content-length');
  if (!v) return 0;
  const n = parseInt(v, 10);
  return Number.isFinite(n) && n > 0 ? n : 0;
}

function emit(): void {
  if (_bytesSent > 0) {
    emitMetric('runtime.network.bytes_sent', _bytesSent);
    _bytesSent = 0;
  }
  if (_bytesReceived > 0) {
    emitMetric('runtime.network.bytes_received', _bytesReceived);
    _bytesReceived = 0;
  }
}

/** Idempotent start — second call is a no-op. */
export function startNetworkBytesInstrument(): void {
  if (_timer !== null) return;
  _timer = setInterval(emit, TICK_MS);
  (_timer as unknown as { unref?: () => void }).unref?.();
}

/** Stop the periodic emit. Idempotent. */
export function stopNetworkBytesInstrument(): void {
  if (_timer !== null) {
    clearInterval(_timer);
    _timer = null;
  }
}

/** Test-only: force one emit tick + reset counters. */
export function __forceNetworkEmitForTests(): void {
  emit();
}

/** Test-only: peek raw counter state without resetting. */
export function __peekNetworkCountersForTests(): { sent: number; received: number } {
  return { sent: _bytesSent, received: _bytesReceived };
}

/** Test-only: reset for clean test runs. */
export function __resetNetworkBytesForTests(): void {
  stopNetworkBytesInstrument();
  _bytesSent = 0;
  _bytesReceived = 0;
}
