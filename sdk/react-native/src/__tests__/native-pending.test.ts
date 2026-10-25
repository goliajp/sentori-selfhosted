import { afterEach, expect, test } from 'bun:test';

import { __resetForTests as resetConfig, setConfig } from '../config';
import { __setNativeForTests } from '../native';
import { shipNativePending } from '../native-pending';
import { __resetForTests as resetTransport, startTransport } from '../transport';

const originalFetch = globalThis.fetch;

afterEach(() => {
  globalThis.fetch = originalFetch;
  __setNativeForTests(undefined);
  resetTransport();
  resetConfig();
});

test('pending crashes retain crash-time release, environment and context on the wire', async () => {
  resetTransport();
  resetConfig();
  setConfig({
    token: 'st_test', ingestUrl: 'http://localhost:18080',
    release: 'new-release', environment: 'staging', enabled: true,
    detect: { rageTap: true, longFreeze: true, slowColdStart: true, slowApi: false },
    replaySeconds: 30,
  });
  startTransport();
  const context = { app: 'insight-native', route: 'checkout', nested: ['a', { enabled: true }, null] };
  __setNativeForTests({ drainPending: async () => [JSON.stringify({
    id: '019ff080-2aeb-7e30-aba1-4431b296d120',
    timestamp: '2026-10-21T00:00:00.000Z', platform: 'android',
    release: 'insight-native@1.2+3', environment: 'production', context,
    error: { type: 'java.lang.IllegalStateException', message: 'crashed', stack: [] },
  })] });
  let posted: { events: Array<Record<string, unknown>> } | undefined;
  globalThis.fetch = (async (_url: unknown, init?: RequestInit) => {
    posted = JSON.parse(String(init?.body));
    return new Response(JSON.stringify({ accepted: 1, outcomes: [] }), { status: 200 });
  }) as typeof fetch;

  expect(await shipNativePending()).toBe(true);
  expect(posted?.events).toHaveLength(1);
  expect(posted?.events[0]).toMatchObject({
    occurredAt: '2026-10-21T00:00:00.000Z', platform: 'android',
    release: 'insight-native@1.2+3', environment: 'production',
    payload: { context, nativeCrash: true },
  });
});
