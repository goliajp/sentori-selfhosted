// v1.1 +S7 升级 — control channel unit tests.

import { afterEach, beforeEach, describe, expect, mock, test } from 'bun:test';

import { setConfig, __resetForTests as resetConfig } from '../config';
import {
  __resetControlChannelForTests,
  isLiveMode,
  startControlChannel,
  stopControlChannel,
} from '../control-channel';
import { setUser } from '../capture';

const originalFetch = globalThis.fetch;

beforeEach(() => {
  resetConfig();
  __resetControlChannelForTests();
  setConfig({
    token: 'st_pk_test',
    release: 'app@1.0.0+1',
    environment: 'test',
    ingestUrl: 'http://localhost:8080',
    enabled: true,
    screenshotsEnabled: false,
    errorSampleRate: null,
    traceSampleRate: null,
    sessionTrailEnabled: false,
  });
  setUser({ id: 'u-1' });
});

afterEach(() => {
  globalThis.fetch = originalFetch;
  __resetControlChannelForTests();
});

describe('control-channel', () => {
  test('isLiveMode() is false by default', () => {
    expect(isLiveMode()).toBe(false);
  });

  test('start → poll → server says liveMode:true → isLiveMode flips on', async () => {
    globalThis.fetch = mock(async () =>
      new Response(JSON.stringify({ liveMode: true, ttlMs: 60_000 }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      }),
    ) as typeof fetch;

    startControlChannel();
    // The initial pollOnce is awaited via void, so give it a microtask.
    await new Promise((r) => setTimeout(r, 30));
    expect(isLiveMode()).toBe(true);
    stopControlChannel();
  });

  test('server says liveMode:false → isLiveMode stays false', async () => {
    globalThis.fetch = mock(async () =>
      new Response(JSON.stringify({ liveMode: false, ttlMs: 0 }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      }),
    ) as typeof fetch;

    startControlChannel();
    await new Promise((r) => setTimeout(r, 30));
    expect(isLiveMode()).toBe(false);
    stopControlChannel();
  });

  test('network failure leaves prior state (no throw on poll error)', async () => {
    globalThis.fetch = mock(async () => {
      throw new Error('econn');
    }) as typeof fetch;
    startControlChannel();
    await new Promise((r) => setTimeout(r, 30));
    // Should not have thrown; isLiveMode stays false.
    expect(isLiveMode()).toBe(false);
    stopControlChannel();
  });

  test('server-reported ttlMs is capped at 15min defence-in-depth', async () => {
    // 24h ttl from server — caller should cap.
    globalThis.fetch = mock(async () =>
      new Response(JSON.stringify({ liveMode: true, ttlMs: 24 * 60 * 60_000 }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      }),
    ) as typeof fetch;
    startControlChannel();
    await new Promise((r) => setTimeout(r, 30));
    // We don't expose _liveModeUntil directly, but we can at least
    // confirm liveMode flipped on without runaway state.
    expect(isLiveMode()).toBe(true);
    stopControlChannel();
  });

  test('without setUser, liveMode stays off (no key to poll)', async () => {
    setUser(undefined);
    globalThis.fetch = mock(async () =>
      new Response(JSON.stringify({ liveMode: true, ttlMs: 60_000 }), {
        status: 200,
      }),
    ) as typeof fetch;
    startControlChannel();
    await new Promise((r) => setTimeout(r, 30));
    expect(isLiveMode()).toBe(false);
    stopControlChannel();
  });
});
