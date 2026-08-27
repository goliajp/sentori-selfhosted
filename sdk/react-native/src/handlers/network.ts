// Network instrumentation — v1 role: feed the signal ring (`http`
// signals) and detect the slow_api warn scenario (design.md §3,
// category C: 「转圈不完」server-side flavour). The span/breadcrumb
// machinery this file used to drive is gone with the APM vocabulary.
//
// Mini-spec (slow_api): a completed request slower than 3 s emits
// one `warn`, scenario `slow_api`, surface = current screen, with
// method + scrubbed URL + duration. Per-endpoint cooldown of 60 s so
// one slow backend doesn't flood ingest with identical warns.
//
// Everything here is wrapped so a bug in our patch can never break
// the host's networking (failure isolation): on any internal error
// the original fetch/XHR behaviour wins.

import { normalizeUrl, pushSignal } from '@goliapkg/sentori-core';

import { getConfig } from '../config';
import { currentScreen } from '../navigation';
import { warnDetected } from '../verbs';

let _installed = false;
let _graphqlEnabled = true;

const AUTH_PARAMS = ['token', 'key', 'password', 'secret', 'access_token'];
const GQL_BODY_MAX_BYTES = 8 * 1024;
const SLOW_API_MS = 3_000;
const SLOW_API_COOLDOWN_MS = 60_000;

const _slowApiLastWarn = new Map<string, number>();

// Requests to our own ingest endpoint are never observed — otherwise
// every batch upload would signal itself, and so on.
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
  _slowApiLastWarn.clear();
};

/** One completed (or failed) request: ring signal + slow_api check. */
const observe = (
  method: string,
  url: string,
  status: number | 'error',
  durationMs: number,
  gqlOp?: string,
): void => {
  try {
    const endpoint = gqlOp ? `graphql/${gqlOp}` : normalizeUrl(url);
    pushSignal('http', { method, url: endpoint, status, ms: durationMs });

    if (durationMs <= SLOW_API_MS) return;
    if (getConfig()?.detect.slowApi !== true) return;
    const now = Date.now();
    const last = _slowApiLastWarn.get(endpoint) ?? 0;
    if (now - last < SLOW_API_COOLDOWN_MS) return;
    _slowApiLastWarn.set(endpoint, now);
    warnDetected(
      'slow_api',
      { screen: currentScreen(), element: endpoint },
      { method, endpoint, durationMs, status },
    );
  } catch {
    // Observation must never leak into the host's request path.
  }
};

// ── fetch ──────────────────────────────────────────────────────────

function patchFetch(): void {
  if (typeof globalThis.fetch !== 'function') return;
  const original = globalThis.fetch;

  globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = extractUrl(input);
    if (isIngestUrl(url)) return original(input, init);
    const start = Date.now();
    const scrubbed = scrubUrl(url);
    const method = (
      (init?.method ??
        (typeof input !== 'string' && 'method' in (input as Request)
          ? (input as Request).method
          : 'GET')) as string
    ).toUpperCase();
    const gqlOp = _graphqlEnabled
      ? extractGraphqlOpFromInit(init, input, url)
      : undefined;

    try {
      const resp = await original(input, init);
      observe(method, scrubbed, resp.status, Date.now() - start, gqlOp);
      return resp;
    } catch (e) {
      if (!isAbortError(e)) {
        observe(method, scrubbed, 'error', Date.now() - start, gqlOp);
      }
      throw e;
    }
  }) as typeof fetch;
}

// ── XHR ────────────────────────────────────────────────────────────

type TracedXhr = XMLHttpRequest & {
  __sentoriMethod?: string;
  __sentoriUrl?: string;
  __sentoriStart?: number;
  __sentoriGqlOp?: string;
};

function patchXhr(): void {
  const XHR = (globalThis as { XMLHttpRequest?: typeof XMLHttpRequest })
    .XMLHttpRequest;
  if (typeof XHR !== 'function') return;
  const proto = XHR.prototype as XMLHttpRequest & { __sentoriPatched?: boolean };
  if (proto.__sentoriPatched) return;
  proto.__sentoriPatched = true;

  const originalOpen = proto.open;
  const originalSend = proto.send;

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

  proto.send = function (
    this: TracedXhr,
    body?: Document | XMLHttpRequestBodyInit | null,
  ): void {
    if (isIngestUrl(this.__sentoriUrl ?? '')) return originalSend.call(this, body);
    const method = this.__sentoriMethod ?? 'GET';
    const url = scrubUrl(this.__sentoriUrl ?? '');
    this.__sentoriGqlOp = _graphqlEnabled
      ? extractGraphqlOpFromXhr(body, this.__sentoriUrl ?? '')
      : undefined;
    this.__sentoriStart = Date.now();

    const finish = (statusOverride?: 'error') => {
      const start = this.__sentoriStart;
      if (start === undefined) return;
      this.__sentoriStart = undefined;
      const status =
        statusOverride ?? (this.status === 0 ? ('error' as const) : this.status);
      observe(method, url, status, Date.now() - start, this.__sentoriGqlOp);
    };

    this.addEventListener('load', () => finish());
    this.addEventListener('error', () => finish('error'));
    this.addEventListener('timeout', () => finish('error'));
    this.addEventListener('abort', () => {
      // Deliberate cancellation is not a network event worth noting.
      this.__sentoriStart = undefined;
    });

    return originalSend.call(this, body);
  };
}

// ── helpers ────────────────────────────────────────────────────────

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

// ── GraphQL operation extraction — cheap, sync, never throws ──────

function lookGraphqlish(url: string, contentType?: string): boolean {
  if (contentType?.includes('graphql')) return true;
  return url.includes('/graphql');
}

/** Pull `operationName` out of a JSON body or a raw query body.
 *  Returns `undefined` on any failure mode; body capped at 8 KB so a
 *  hostile / oversize body never lands in `JSON.parse`. */
export function parseGqlOpName(body: string): string | undefined {
  if (typeof body !== 'string' || body.length === 0) return undefined;
  if (body.length > GQL_BODY_MAX_BYTES) return undefined;
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
        const q = (candidate as { query?: unknown }).query;
        if (typeof q === 'string') return parseQueryStringOpName(q);
      }
    } catch {
      return undefined;
    }
    return undefined;
  }
  return parseQueryStringOpName(body);
}

function parseQueryStringOpName(query: string): string | undefined {
  const m =
    /^\s*(?:#[^\n]*\n\s*)*(query|mutation|subscription)\s+([A-Za-z_][A-Za-z0-9_]*)/.exec(
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
