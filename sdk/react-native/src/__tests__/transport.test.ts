// Transport on the v1 wire: batch envelope, assert piggyback,
// offline persistence hooks.

import { afterEach, beforeEach, describe, expect, it, mock } from 'bun:test';

import { __resetForTests as resetConfig, setConfig } from '../config';
import {
  __peekAssertStats,
  __peekQueue,
  __resetForTests as resetTransport,
  countAssert,
  enqueue,
  flush,
  startTransport,
} from '../transport';

const baseConfig = {
  token: 'st_test',
  ingestUrl: 'http://localhost:18080',
  release: 'app@1.0.0',
  environment: 'test',
  enabled: true,
  detect: { rageTap: true, longFreeze: true, slowColdStart: true, slowApi: false },
  replaySeconds: 30,
};

const wire = (kind: 'error' | 'trace') => ({
  id: '019e0000-0000-7000-8000-000000000001',
  kind,
  occurredAt: new Date().toISOString(),
  platform: 'javascript' as const,
  payload: {},
});

const originalFetch = globalThis.fetch;

describe('transport (v1 wire)', () => {
  beforeEach(() => {
    resetTransport();
    resetConfig();
    setConfig(baseConfig);
    startTransport();
  });
  afterEach(() => {
    globalThis.fetch = originalFetch;
    resetTransport();
    resetConfig();
  });

  it('posts the batch envelope to /v1/events:batch', async () => {
    let seenUrl = '';
    let seenBody: unknown;
    globalThis.fetch = mock(async (url: unknown, init?: RequestInit) => {
      seenUrl = String(url);
      seenBody = JSON.parse(String(init?.body));
      return new Response(JSON.stringify({ accepted: 1, outcomes: [] }), { status: 200 });
    }) as typeof fetch;

    enqueue(wire('error'));
    await flush();
    expect(seenUrl).toBe('http://localhost:18080/v1/events:batch');
    expect((seenBody as { events: unknown[] }).events.length).toBe(1);
  });

  it('piggybacks assert stats on the envelope and clears them', async () => {
    let seenBody: { assertStats?: unknown[] } = {};
    globalThis.fetch = mock(async (_url: unknown, init?: RequestInit) => {
      seenBody = JSON.parse(String(init?.body));
      return new Response(JSON.stringify({ accepted: 0, outcomes: [] }), { status: 200 });
    }) as typeof fetch;

    countAssert('total-positive', true, 'app@1.0.0');
    countAssert('total-positive', true, 'app@1.0.0');
    countAssert('total-positive', false, 'app@1.0.0');
    expect(__peekAssertStats().length).toBe(1);
    await flush();
    const stats = seenBody.assertStats as Array<{
      name: string;
      passDelta: number;
      failDelta: number;
    }>;
    expect(stats.length).toBe(1);
    expect(stats[0]?.passDelta).toBe(2);
    expect(stats[0]?.failDelta).toBe(1);
    expect(__peekAssertStats().length).toBe(0);
  });

  it('never flushes before startTransport', async () => {
    resetTransport();
    let called = 0;
    globalThis.fetch = mock(async () => {
      called += 1;
      return new Response('{}', { status: 200 });
    }) as typeof fetch;
    enqueue(wire('trace'));
    await flush();
    expect(called).toBe(0);
    expect(__peekQueue().length).toBe(1);
  });
});

it('SDK_VERSION constant matches package.json (staleness gate)', async () => {
  const { __sdkVersion } = await import('../transport');
  const pkg = (await import('../../package.json')) as { default?: { version?: string }; version?: string };
  expect(__sdkVersion()).toBe(pkg.default?.version ?? pkg.version);
});

it('the batch envelope carries backendHealthUrl when configured', async () => {
  const { __resetForTests, enqueue, flush, startTransport } = await import('../transport');
  const { __resetForTests: resetCfg, setConfig } = await import('../config');
  __resetForTests();
  resetCfg();
  setConfig({
    token: 'st_t',
    ingestUrl: 'http://localhost:1',
    release: 'a@1',
    environment: 'test',
    enabled: true,
    detect: { rageTap: false, longFreeze: false, slowColdStart: false, slowApi: false },
    replaySeconds: 0,
    replayScreens: false,
    backendHealthUrl: 'https://api.example.com/healthz',
  });
  startTransport();
  const seen: unknown[] = [];
  const realFetch = globalThis.fetch;
  globalThis.fetch = ((url: unknown, opts: { body?: string }) => {
    seen.push(JSON.parse(opts.body ?? '{}'));
    return Promise.resolve(new Response('{}', { status: 200 }));
  }) as typeof fetch;
  try {
    enqueue({ id: 'e1', kind: 'trace', occurredAt: 'now', platform: 'ios', release: 'a@1', environment: 'test', payload: {} } as never);
    await flush();
  } finally {
    globalThis.fetch = realFetch;
  }
  expect((seen[0] as { backendHealthUrl?: string }).backendHealthUrl).toBe(
    'https://api.example.com/healthz',
  );
});
