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
