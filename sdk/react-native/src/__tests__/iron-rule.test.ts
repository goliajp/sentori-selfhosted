// The zero-cost iron rule, as executable gates (design.md §6).
//
// Dimension 3 (failure isolation): no Sentori failure may ever
// throw into, block, or alter the host app. These tests inject the
// failures; a single uncaught throw fails the suite.

import { afterEach, beforeEach, describe, expect, it } from 'bun:test';

import { clearSignals } from '@goliapkg/sentori-core';

import { __resetForTests as resetConfig, setConfig } from '../config';
import { init, __resetForTests as resetInit } from '../init';
import { __resetForTests as resetScope, patchContext, setUser } from '../scope';
import {
  __resetForTests as resetTransport,
  flush,
  startTransport,
} from '../transport';
import { verbs } from '../verbs';
import { __setNativeForTests as setNative } from '../native';
import {
  __resetForTests as resetPush,
  __setPlatformForTests as setPlatform,
  getCachedIpt,
  register as pushRegister,
  unregister as pushUnregister,
} from '../push';

const baseConfig = {
  token: 'st_test',
  ingestUrl: 'http://localhost:18080',
  release: 'app@1.0.0',
  environment: 'test',
  enabled: true,
  detect: { rageTap: true, longFreeze: true, slowColdStart: true, slowApi: false },
  replaySeconds: 30,
};

const originalFetch = globalThis.fetch;

const resetAll = () => {
  resetTransport();
  resetConfig();
  resetScope();
  resetInit();
  clearSignals();
};

describe('iron rule: failure isolation', () => {
  beforeEach(() => {
    resetAll();
    setConfig(baseConfig);
    startTransport();
  });
  afterEach(() => {
    globalThis.fetch = originalFetch;
    resetAll();
  });

  it('server 500 never reaches the caller', async () => {
    globalThis.fetch = (async () => new Response('boom', { status: 500 })) as typeof fetch;
    verbs.error(new Error('x'));
    await flush(); // retries internally, persists, never rejects
    expect(true).toBe(true);
  });

  it('network down never reaches the caller', async () => {
    globalThis.fetch = (async () => {
      throw new TypeError('Network request failed');
    }) as typeof fetch;
    verbs.warn('rage_tap');
    await flush();
    expect(true).toBe(true);
  });

  it('a hostile beforeSend cannot break the verb', () => {
    setConfig({
      ...baseConfig,
      beforeSend: () => {
        throw new Error('host bug');
      },
    });
    const id = verbs.error(new Error('x'));
    expect(typeof id).toBe('string');
  });

  it('fuzz: the verbs accept garbage without throwing', () => {
    const circular: Record<string, unknown> = {};
    circular.self = circular;
    const huge = 'x'.repeat(1_000_000);

    expect(typeof verbs.error(null)).toBe('string');
    expect(typeof verbs.error(undefined)).toBe('string');
    expect(typeof verbs.error(42)).toBe('string');
    expect(typeof verbs.error(circular)).toBe('string');
    expect(typeof verbs.warn('', circular)).toBe('string');
    expect(typeof verbs.warn(huge)).toBe('string');
    expect(typeof verbs.trace('t', { huge })).toBe('string');
    expect(typeof verbs.assert('a', false, circular)).toBe('string');
    expect(typeof verbs.probe('p', { circular })).toBe('string');
    setUser({ id: huge });
    setUser(null);
    patchContext(circular);
  });

  it('every verb before init is a silent no-op returning an id', () => {
    resetAll();
    expect(typeof verbs.error(new Error('x'))).toBe('string');
    expect(typeof verbs.warn('w')).toBe('string');
    expect(typeof verbs.trace('t')).toBe('string');
    expect(typeof verbs.assert('a', false)).toBe('string');
    expect(typeof verbs.probe('p')).toBe('string');
  });

  it('init with a broken config degrades to no-op, never throws', () => {
    resetAll();
    init(undefined as never);
    init({} as never);
    init({ token: '', ingestUrl: '' } as never);
    expect(typeof verbs.error(new Error('x'))).toBe('string');
  });
});

describe('iron rule: init timing', () => {
  beforeEach(resetAll);
  afterEach(resetAll);

  it('init completes inside the 50 ms budget', () => {
    const start = performance.now();
    init(baseConfig);
    const ms = performance.now() - start;
    // The budget is for the synchronous path; native drains and the
    // offline queue are fire-and-forget behind it.
    expect(ms).toBeLessThan(50);
  });
});

// Dimension 3, for push specifically.
//
// The rule was written down and the gate did not reach here: this file
// covered the five event verbs and mentioned push nowhere, while push
// is the surface with the most ways to go wrong — an OS permission, a
// vendor token, a network round trip, a background loop, and the
// host's own handlers running inside it.
//
// A notification that does not arrive is something the host can live
// with. An exception out of an SDK it merely opted into is not.
describe('iron rule: push never reaches the host', () => {
  beforeEach(() => {
    resetAll();
    resetPush();
    setPlatform('ios');
    setConfig(baseConfig);
  });
  afterEach(() => {
    globalThis.fetch = originalFetch;
    setPlatform(null);
    setNative(undefined);
    resetPush();
    resetAll();
  });

  const granting = {
    pushRequestPermission: () => Promise.resolve('granted'),
    pushGetStatus: () => Promise.resolve('granted'),
    pushRegister: () => undefined,
    pushUnregister: () => undefined,
    pushDrainState: () => Promise.resolve({ notifications: [], taps: [], token: 'abc' }),
  };

  it('a server that answers 500 is an outcome, not an exception', async () => {
    setNative(granting);
    globalThis.fetch = (async () => new Response('boom', { status: 500 })) as typeof fetch;
    const r = await pushRegister();
    expect(r.ok).toBe(false);
  });

  it('a network that is down is an outcome, not an exception', async () => {
    setNative(granting);
    globalThis.fetch = (async () => {
      throw new Error('offline');
    }) as typeof fetch;
    const r = await pushRegister();
    expect(r.ok).toBe(false);
  });

  it('a server that answers nonsense is an outcome, not an exception', async () => {
    setNative(granting);
    globalThis.fetch = (async () => new Response('<html>not json</html>', { status: 200 })) as typeof fetch;
    const r = await pushRegister();
    expect(r.ok).toBe(false);
  });

  it('a native module that throws is an outcome, not an exception', async () => {
    setNative({
      ...granting,
      pushRequestPermission: () => {
        throw new Error('bridge died');
      },
    });
    const r = await pushRegister();
    expect(r.ok).toBe(false);
  });

  it('fuzz: the push verbs accept garbage without throwing', async () => {
    setNative(granting);
    globalThis.fetch = (async () =>
      new Response(JSON.stringify({ spToken: 'dev-1' }), { status: 200 })) as typeof fetch;
    const circular: Record<string, unknown> = { a: 1 };
    circular.self = circular;
    const garbage = [
      undefined,
      null,
      {},
      { metadata: circular },
      { metadata: { huge: 'x'.repeat(200_000) } },
      { onMessage: 'not a function' },
      { tokenTimeoutMs: -1 },
      { tokenTimeoutMs: Number.NaN },
    ];
    for (const g of garbage) {
      const r = await pushRegister(g as never);
      expect(typeof r.ok).toBe('boolean');
    }
  });

  it('every push verb before init is a quiet outcome', async () => {
    resetConfig();
    setNative(granting);
    const r = await pushRegister();
    expect(r).toEqual({ ok: false, message: 'sentori.init() has not run', reason: 'not-initialised' });
    await pushUnregister();
    expect(await getCachedIpt()).toBe(null);
  });

  it('unregister cannot fail, whatever the server does', async () => {
    setNative(granting);
    globalThis.fetch = (async () =>
      new Response(JSON.stringify({ spToken: 'dev-1' }), { status: 200 })) as typeof fetch;
    await pushRegister();
    globalThis.fetch = (async () => {
      throw new Error('offline');
    }) as typeof fetch;
    await pushUnregister();
  });
});
