import { clearSpans, drainSpans } from '@goliapkg/sentori-core';
import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  test,
} from 'bun:test';

import { installNetworkHandler } from '../handlers/network';

// network.ts installs ONCE per process. To test reliably we:
//   1. set up a single static recorder on globalThis.fetch
//   2. install the wrapper exactly once (in beforeAll)
//   3. between tests, only mutate the recorder's queue — NEVER
//      re-assign globalThis.fetch, because the wrapper captured the
//      first reference at install time.

const recorderCalls: Array<{ init?: RequestInit; url: string }> = [];
let recorderQueue: Array<{ status: number } | Error> = [];

const recorder = (async (input: Request | string | URL, init?: RequestInit) => {
  const url =
    typeof input === 'string'
      ? input
      : input instanceof URL
        ? input.toString()
        : input.url;
  recorderCalls.push({ init, url });
  const r = recorderQueue.shift() ?? { status: 200 };
  if (r instanceof Error) throw r;
  return new Response('', { status: r.status });
}) as unknown as typeof fetch;

// Fake XMLHttpRequest so installNetworkHandler()'s patchXhr() has a
// prototype to patch. RN's native XHR isn't present in bun:test.
type FakeListener = () => void;
class FakeXHR {
  status = 0;
  private listeners: Record<string, FakeListener[]> = {};
  private requestHeaders: Record<string, string> = {};
  private opened = false;

  open(_method: string, _url: string | URL): void {
    this.opened = true;
  }
  setRequestHeader(name: string, value: string): void {
    if (!this.opened) throw new Error('setRequestHeader before open');
    this.requestHeaders[name.toLowerCase()] = value;
  }
  send(_body?: unknown): void {}
  addEventListener(event: string, fn: FakeListener): void {
    (this.listeners[event] ??= []).push(fn);
  }
  getHeader(name: string): string | undefined {
    return this.requestHeaders[name.toLowerCase()];
  }
  fire(event: string): void {
    for (const fn of this.listeners[event] ?? []) fn();
  }
}

beforeAll(() => {
  (globalThis as { fetch: typeof fetch }).fetch = recorder;
  (globalThis as { XMLHttpRequest: unknown }).XMLHttpRequest = FakeXHR as unknown;
  installNetworkHandler(); // patches globalThis.fetch + XMLHttpRequest.prototype
});

beforeEach(() => {
  clearSpans();
  recorderCalls.length = 0;
  recorderQueue = [{ status: 200 }];
});

afterEach(() => {
  clearSpans();
});

describe('RN network handler tracing', () => {
  test('wrapped fetch emits an http.client span', async () => {
    const resp = await fetch('https://api.example.com/v1/users/me', {
      method: 'GET',
    });
    expect(resp.status).toBe(200);

    const spans = drainSpans();
    expect(spans).toHaveLength(1);
    expect(spans[0]?.op).toBe('http.client');
    expect(spans[0]?.name).toBe('GET https://api.example.com/v1/users/me');
    expect(spans[0]?.tags).toMatchObject({
      'http.method': 'GET',
      'http.status': '200',
      'http.url': 'https://api.example.com/v1/users/me',
    });
    expect(spans[0]?.status).toBe('ok');
  });

  test('injects W3C traceparent header', async () => {
    await fetch('https://api.example.com/x');
    expect(recorderCalls).toHaveLength(1);
    const headers = new Headers(recorderCalls[0]?.init?.headers);
    const tp = headers.get('traceparent');
    expect(tp).not.toBeNull();
    expect(tp).toMatch(/^00-[0-9a-f]{32}-[0-9a-f]{16}-01$/);
  });

  test('span name normalizes id-like segments; tag keeps full scrubbed url', async () => {
    await fetch('https://api.example.com/users/123/orders/456?token=abc');
    const sp = drainSpans()[0]!;
    expect(sp.name).toBe('GET https://api.example.com/users/{id}/orders/{id}');
    // RN's scrubUrl redacts auth params in the tag, but keeps path + host.
    expect(sp.tags['http.url']).toContain('https://api.example.com/users/123/orders/456');
    expect(sp.tags['http.url']).not.toContain('token=abc');
  });

  test('5xx → span.status = "error"', async () => {
    recorderQueue = [{ status: 503 }];
    await fetch('https://api.example.com/x');
    expect(drainSpans()[0]?.status).toBe('error');
  });

  test('throws → status = "error" with error.message tag', async () => {
    recorderQueue = [new TypeError('NetworkError: offline')];
    await expect(fetch('https://api.example.com/x')).rejects.toThrow('NetworkError');
    const sp = drainSpans()[0]!;
    expect(sp.status).toBe('error');
    expect(sp.tags['error.message']).toContain('offline');
  });

  test('AbortError → status = "cancelled"', async () => {
    recorderQueue = [Object.assign(new Error('aborted'), { name: 'AbortError' })];
    await expect(fetch('https://api.example.com/x')).rejects.toThrow('aborted');
    expect(drainSpans()[0]?.status).toBe('cancelled');
  });

  test('preserves caller-supplied headers alongside traceparent', async () => {
    await fetch('https://api.example.com/x', {
      headers: { Authorization: 'Bearer xyz', 'X-Custom': '1' },
    });
    const h = new Headers(recorderCalls[0]?.init?.headers);
    expect(h.get('authorization')).toBe('Bearer xyz');
    expect(h.get('x-custom')).toBe('1');
    expect(h.get('traceparent')).toBeTruthy();
  });
});

describe('RN XHR tracing (axios goes through XHR on RN)', () => {
  test('patched XHR emits an http.client span on loadend', () => {
    const x = new FakeXHR();
    x.open('POST', 'https://api.example.com/v1/orders');
    x.send('{}');
    x.status = 201;
    x.fire('loadend');

    const sp = drainSpans()[0]!;
    expect(sp.op).toBe('http.client');
    expect(sp.name).toBe('POST https://api.example.com/v1/orders');
    expect(sp.tags).toMatchObject({
      'http.method': 'POST',
      'http.status': '201',
      'http.url': 'https://api.example.com/v1/orders',
    });
    expect(sp.status).toBe('ok');
  });

  test('injects W3C traceparent request header', () => {
    const x = new FakeXHR();
    x.open('GET', 'https://api.example.com/x');
    x.send();
    expect(x.getHeader('traceparent')).toMatch(/^00-[0-9a-f]{32}-[0-9a-f]{16}-01$/);
    x.status = 200;
    x.fire('loadend');
  });

  test('5xx → span.status = "error"', () => {
    const x = new FakeXHR();
    x.open('GET', 'https://api.example.com/x');
    x.send();
    x.status = 502;
    x.fire('loadend');
    expect(drainSpans()[0]?.status).toBe('error');
  });

  test('status 0 → span.status = "error"', () => {
    const x = new FakeXHR();
    x.open('GET', 'https://api.example.com/x');
    x.send();
    x.status = 0;
    x.fire('loadend');
    expect(drainSpans()[0]?.status).toBe('error');
  });

  test('abort → span.status = "cancelled"', () => {
    const x = new FakeXHR();
    x.open('GET', 'https://api.example.com/x');
    x.send();
    x.fire('abort');
    expect(drainSpans()[0]?.status).toBe('cancelled');
  });
});
