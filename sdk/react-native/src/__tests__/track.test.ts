// v1.1 chunk B — analytics `track` buffer + flush.

import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  mock,
} from 'bun:test';

import { __resetForTests as resetConfig, setConfig } from '../config';
import { setUser } from '../capture';
import {
  __peekTrackBuffer,
  __resetTrackForTests,
  flushTrack,
  track,
} from '../track';

const originalFetch = globalThis.fetch;

describe('track buffer', () => {
  beforeEach(() => {
    resetConfig();
    __resetTrackForTests();
    setConfig({
      token: 'st_pk_test',
      release: 'app@1.0.0+1',
      environment: 'test',
      ingestUrl: 'http://localhost:8080',
      enabled: true,
    });
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
    setUser(null);
  });

  it('buffers calls and tags them with release + environment', () => {
    track('checkout.started', { cart: 42 });
    track('$pageview', undefined, 'Cart');
    const buf = __peekTrackBuffer();
    expect(buf.length).toBe(2);
    expect(buf[0].name).toBe('checkout.started');
    expect(buf[0].release).toBe('app@1.0.0+1');
    expect(buf[0].environment).toBe('test');
    expect(buf[0].props).toEqual({ cart: 42 });
    expect(buf[1].name).toBe('$pageview');
    expect(buf[1].route).toBe('Cart');
  });

  it('attaches the current user id when set', () => {
    setUser({ id: 'u_abc' });
    track('signed_in');
    const buf = __peekTrackBuffer();
    expect(buf[0].userId).toBe('u_abc');
  });

  it('drops oversized names + over-cap prop bags silently', () => {
    track('x'.repeat(201));
    const tooManyProps: Record<string, number> = {};
    for (let i = 0; i < 41; i += 1) tooManyProps[`k${i}`] = i;
    track('big-bag', tooManyProps);
    expect(__peekTrackBuffer().length).toBe(0);
  });

  it('flushTrack drains the buffer and POSTs the batch envelope', async () => {
    const calls: { body: string; url: string }[] = [];
    globalThis.fetch = mock(async (url: unknown, init: unknown) => {
      calls.push({
        body: String((init as { body?: unknown })?.body ?? ''),
        url: String(url),
      });
      return new Response('{}', { status: 202 });
    }) as unknown as typeof fetch;

    track('a');
    track('b');
    await flushTrack();

    expect(calls.length).toBe(1);
    expect(calls[0].url).toBe('http://localhost:8080/v1/track:batch');
    const parsed = JSON.parse(calls[0].body) as {
      events: Array<{ name: string }>;
    };
    expect(parsed.events.map((e) => e.name)).toEqual(['a', 'b']);
    expect(__peekTrackBuffer().length).toBe(0);
  });
});
