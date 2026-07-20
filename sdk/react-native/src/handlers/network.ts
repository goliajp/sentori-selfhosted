import { normalizeUrl, startSpan } from '@goliapkg/sentori-core';

import { addBreadcrumb } from '../breadcrumbs';
import { getConfig } from '../config';
// v2.1 W2 — bytes counters drive the runtime.network.{sent,received}
// metrics. Cheap two-add per request, no allocation.
import {
  estimateRequestBytes,
  estimateResponseBytes,
  recordNetworkBytes,
} from '../runtime-metrics-network';

let _installed = false;
let _graphqlEnabled = true;

const AUTH_PARAMS = ['token', 'key', 'password', 'secret', 'access_token'];

// v0.9.0 #11 — cap on body size we'll parse for `operationName`.
// 8 KB is generous for any sensible GraphQL request and keeps the
// hot-path JSON.parse bounded.
const GQL_BODY_MAX_BYTES = 8 * 1024;

// Requests to our own ingest endpoint shouldn't be traced — otherwise
// every span upload spawns another http.client span, and so on.
const isIngestUrl = (url: string): boolean => {
  const base = getConfig()?.ingestUrl;
  return !!base && url.startsWith(base);
};

export const installNetworkHandler = (opts?: { graphql?: boolean }): void => {
  if (_installed) return;
  _installed = true;
  _graphqlEnabled = opts?.graphql !== false;
  patchFetch();
  patchXhr();
};

/** Test-only — reset module state between runs. */
export const __resetNetworkHandlerForTests = (): void => {
  _installed = false;
  _graphqlEnabled = true;
};

// ── fetch ──────────────────────────────────────────────────────────

function patchFetch(): void {
  if (typeof globalThis.fetch !== 'function') return;
  const original = globalThis.fetch;

  globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
    const start = Date.now();
    const url = extractUrl(input);
    if (isIngestUrl(url)) return original(input, init);
    const scrubbed = scrubUrl(url);
    const method = (init?.method ??
      (typeof input !== 'string' && 'method' in (input as Request)
        ? (input as Request).method
        : 'GET')) as string;

    // v0.9.0 #11 — GraphQL operation auto-naming. Inspect the request
    // body cheaply (string only, < 8 KB) when the request looks like
    // GraphQL (URL contains /graphql or content-type hints it). On
    // success we override the span name to `graphql/<operationName>`
    // and ride along `gql.operation` on the breadcrumb so the dashboard
    // can group + filter by operation instead of by the useless
    // `POST /graphql` line.
    const gqlOp = _graphqlEnabled
      ? extractGraphqlOpFromInit(init, input, url)
      : undefined;

    // Phase 35 sub-C: also open an http.client span so the request
    // shows up in the trace waterfall. Breadcrumbs stay — they're
    // attached to error events at capture time and serve a different
    // surface (the "last 100 things" timeline on the issue page).
    const span = startSpan('http.client', {
      name: gqlOp
        ? `graphql/${gqlOp}`
        : `${method.toUpperCase()} ${normalizeUrl(scrubbed)}`,
      tags: gqlOp
        ? {
            'http.method': method.toUpperCase(),
            'http.url': scrubbed,
            'gql.operation': gqlOp,
          }
        : { 'http.method': method.toUpperCase(), 'http.url': scrubbed },
    });

    // Inject traceparent header on outbound requests.
    const reqInit: RequestInit = { ...(init ?? {}) };
    const headers = mergeHeaders(input, init);
    headers.set('traceparent', toTraceparent(span.traceId, span.spanId));
    reqInit.headers = headers;

    try {
      const resp = await original(input, reqInit);
      span.setTag('http.status', String(resp.status));
      span.finish({ status: resp.status >= 400 ? 'error' : 'ok' });
      // v2.1 W2 — bytes accounting. Sent estimated from
      // init.body; received read from response Content-Length
      // header (0 when missing / chunked — undercount-safe).
      recordNetworkBytes(estimateRequestBytes(init), estimateResponseBytes(resp.headers));
      addBreadcrumb({
        type: 'net',
        data: gqlOp
          ? {
              method,
              url: scrubbed,
              status: resp.status,
              durationMs: Date.now() - start,
              'gql.operation': gqlOp,
            }
          : {
              method,
              url: scrubbed,
              status: resp.status,
              durationMs: Date.now() - start,
            },
      });
      return resp;
    } catch (e) {
      const isAbort = isAbortError(e);
      if (e instanceof Error) span.setTag('error.message', e.message);
      span.finish({ status: isAbort ? 'cancelled' : 'error' });
      addBreadcrumb({
        type: 'net',
        data: {
          method,
          url: scrubbed,
          status: 0,
          durationMs: Date.now() - start,
          error: String(e),
          ...(gqlOp ? { 'gql.operation': gqlOp } : {}),
        },
      });
      throw e;
    }
  }) as typeof fetch;
}

// ── XMLHttpRequest ─────────────────────────────────────────────────
//
// React Native's XHR is a native polyfill, not built on fetch — so
// patching `globalThis.fetch` alone misses every axios / older-style
// request. axios on RN uses its `xhr` adapter by default. We patch
// the prototype's `open` + `send` so the instance carries the span
// from `send()` to `loadend`.

type TracedXhr = XMLHttpRequest & {
  __sentoriMethod?: string;
  __sentoriUrl?: string;
  __sentoriSpan?: ReturnType<typeof startSpan>;
  __sentoriStart?: number;
  __sentoriGqlOp?: string;
};

function patchXhr(): void {
  const XHR = (globalThis as { XMLHttpRequest?: typeof XMLHttpRequest }).XMLHttpRequest;
  if (typeof XHR !== 'function') return;
  const proto = XHR.prototype as XMLHttpRequest & {
    __sentoriPatched?: boolean;
  };
  if (proto.__sentoriPatched) return;
  proto.__sentoriPatched = true;

  const originalOpen = proto.open;
  const originalSend = proto.send;
  const originalSetHeader = proto.setRequestHeader;

  proto.open = function (
    this: TracedXhr,
    method: string,
    url: string | URL,
    ...rest: unknown[]
  ): void {
    this.__sentoriMethod = String(method).toUpperCase();
    this.__sentoriUrl = typeof url === 'string' ? url : String(url);
    // @ts-expect-error variadic forwarding to the native signature
    return originalOpen.call(this, method, url, ...rest);
  };

  proto.send = function (this: TracedXhr, body?: Document | XMLHttpRequestBodyInit | null): void {
    if (isIngestUrl(this.__sentoriUrl ?? '')) return originalSend.call(this, body);
    const method = this.__sentoriMethod ?? 'GET';
    const url = scrubUrl(this.__sentoriUrl ?? '');
    // v0.9.0 #11 — GraphQL operation auto-naming on XHR.
    const gqlOp = _graphqlEnabled
      ? extractGraphqlOpFromXhr(body, this.__sentoriUrl ?? '')
      : undefined;
    this.__sentoriGqlOp = gqlOp;
    const span = startSpan('http.client', {
      name: gqlOp ? `graphql/${gqlOp}` : `${method} ${normalizeUrl(url)}`,
      tags: gqlOp
        ? {
            'http.method': method,
            'http.url': url,
            'gql.operation': gqlOp,
          }
        : { 'http.method': method, 'http.url': url },
    });
    this.__sentoriSpan = span;
    this.__sentoriStart = Date.now();

    // setRequestHeader must be called between open() and send(); we're
    // inside send() before the underlying call, so this is legal.
    try {
      originalSetHeader.call(this, 'traceparent', toTraceparent(span.traceId, span.spanId));
    } catch {
      // Some XHR polyfills are strict about header timing; if it
      // rejects, drop the header rather than fail the request.
    }

    const finish = () => {
      const s = this.__sentoriSpan;
      if (!s) return;
      this.__sentoriSpan = undefined;
      const status = this.status;
      s.setTag('http.status', String(status));
      // status 0 means network error / aborted / CORS block — treat
      // as error. The `abort` event handler below downgrades aborts.
      s.finish({ status: status === 0 || status >= 400 ? 'error' : 'ok' });
      const op = this.__sentoriGqlOp;
      addBreadcrumb({
        type: 'net',
        data: op
          ? {
              method,
              url,
              status,
              durationMs: Date.now() - (this.__sentoriStart ?? Date.now()),
              'gql.operation': op,
            }
          : {
              method,
              url,
              status,
              durationMs: Date.now() - (this.__sentoriStart ?? Date.now()),
            },
      });
    };

    this.addEventListener('loadend', finish);
    this.addEventListener('abort', () => {
      const s = this.__sentoriSpan;
      if (!s) return;
      this.__sentoriSpan = undefined;
      s.finish({ status: 'cancelled' });
      const op = this.__sentoriGqlOp;
      addBreadcrumb({
        type: 'net',
        data: {
          method,
          url,
          status: 0,
          durationMs: Date.now() - (this.__sentoriStart ?? Date.now()),
          error: 'aborted',
          ...(op ? { 'gql.operation': op } : {}),
        },
      });
    });

    return originalSend.call(this, body);
  };
}

function mergeHeaders(input: RequestInfo | URL, init?: RequestInit): Headers {
  const out = new Headers();
  if (typeof input !== 'string' && !(input instanceof URL)) {
    (input as Request).headers.forEach((v, k) => out.set(k, v));
  }
  if (init?.headers) {
    new Headers(init.headers).forEach((v, k) => out.set(k, v));
  }
  return out;
}

function toTraceparent(traceId: string, spanId: string): string {
  const trace = traceId.replace(/-/g, '').toLowerCase();
  const parent = spanId.replace(/-/g, '').toLowerCase().slice(0, 16);
  return `00-${trace}-${parent}-01`;
}

function isAbortError(err: unknown): boolean {
  if (typeof err !== 'object' || err === null) return false;
  return (err as { name?: unknown }).name === 'AbortError';
}

const extractUrl = (input: RequestInfo | URL): string => {
  if (typeof input === 'string') return input;
  if (input instanceof URL) return input.href;
  return (input as Request).url;
};

const scrubUrl = (url: string): string => {
  try {
    const u = new URL(url);
    let modified = false;
    for (const p of AUTH_PARAMS) {
      if (u.searchParams.has(p)) {
        u.searchParams.set(p, '[redacted]');
        modified = true;
      }
    }
    return modified ? u.toString() : url;
  } catch {
    return url;
  }
};

// ── v0.9.0 #11 — GraphQL operation extraction ─────────────────────
//
// Cheap, sync, never throws. Two callers (fetch + xhr) feed in
// whatever they have on hand; both end up calling `parseGqlOpName`.

function lookGraphqlish(url: string, contentType?: string): boolean {
  if (contentType) {
    if (contentType.includes('graphql')) return true;
    // application/json is too generic to gate on alone, but combined
    // with a `/graphql` path it's a strong hint.
  }
  if (url.includes('/graphql')) return true;
  return false;
}

/** Pull `operationName` out of a JSON body or a raw query body. Returns
 *  `undefined` on any failure mode. Cap at GQL_BODY_MAX_BYTES so a
 *  hostile / oversize body never lands in `JSON.parse`. */
export function parseGqlOpName(body: string): string | undefined {
  if (typeof body !== 'string' || body.length === 0) return undefined;
  if (body.length > GQL_BODY_MAX_BYTES) return undefined;
  // First char `{` or `[` → JSON path. Most clients (Apollo, urql,
  // Relay) send `{"query":"…","operationName":"…","variables":{…}}`
  // or an array of such objects (batched).
  const first = body.charCodeAt(0);
  if (first === 0x7b /* { */ || first === 0x5b /* [ */) {
    try {
      const parsed = JSON.parse(body) as unknown;
      const candidate = Array.isArray(parsed) ? parsed[0] : parsed;
      if (candidate && typeof candidate === 'object') {
        const name = (candidate as { operationName?: unknown }).operationName;
        if (typeof name === 'string' && name.length > 0 && name.length <= 200) {
          return name;
        }
        // No operationName — try to sniff the `query` string for
        // `query Foo {…}` / `mutation Bar {…}` / `subscription Baz {…}`.
        const q = (candidate as { query?: unknown }).query;
        if (typeof q === 'string') return parseQueryStringOpName(q);
      }
    } catch {
      return undefined;
    }
    return undefined;
  }
  // `application/graphql` body is the bare query string — no JSON wrapper.
  return parseQueryStringOpName(body);
}

function parseQueryStringOpName(query: string): string | undefined {
  // Skip leading whitespace + comments. We only need the first non-trivial
  // top-level operation keyword to extract a name; nested operations are
  // a non-issue because GraphQL forbids them.
  const m = /^\s*(?:#[^\n]*\n\s*)*(query|mutation|subscription)\s+([A-Za-z_][A-Za-z0-9_]*)/.exec(
    query,
  );
  return m?.[2];
}

function extractGraphqlOpFromInit(
  init: RequestInit | undefined,
  input: RequestInfo | URL,
  url: string,
): string | undefined {
  const method = (init?.method ??
    (typeof input !== 'string' && 'method' in (input as Request)
      ? (input as Request).method
      : 'GET')) as string;
  if (method.toUpperCase() !== 'POST') return undefined;
  const contentType = headerValue(init, input, 'content-type');
  if (!lookGraphqlish(url, contentType)) return undefined;
  const body = init?.body;
  if (typeof body !== 'string') return undefined;
  return parseGqlOpName(body);
}

function extractGraphqlOpFromXhr(
  body: Document | XMLHttpRequestBodyInit | null | undefined,
  url: string,
): string | undefined {
  if (typeof body !== 'string') return undefined;
  if (!lookGraphqlish(url)) return undefined;
  return parseGqlOpName(body);
}

function headerValue(
  init: RequestInit | undefined,
  input: RequestInfo | URL,
  name: string,
): string | undefined {
  const target = name.toLowerCase();
  if (init?.headers) {
    try {
      const h = new Headers(init.headers);
      const v = h.get(target);
      if (v) return v;
    } catch {
      // ignore — bad header shape, treat as absent
    }
  }
  if (typeof input !== 'string' && !(input instanceof URL)) {
    try {
      const v = (input as Request).headers.get(target);
      if (v) return v;
    } catch {
      // ignore
    }
  }
  return undefined;
}
