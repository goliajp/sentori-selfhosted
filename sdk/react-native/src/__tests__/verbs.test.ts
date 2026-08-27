// The five verbs: synchronous, never throw, id out, correct wire.

import { afterEach, beforeEach, describe, expect, it } from 'bun:test';

import { clearSignals } from '@goliapkg/sentori-core';

import { __resetForTests as resetConfig, setConfig } from '../config';
import { __resetForTests as resetScope } from '../scope';
import {
  __peekAssertStats,
  __peekQueue,
  __resetForTests as resetTransport,
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

describe('the five verbs', () => {
  beforeEach(() => {
    resetTransport();
    resetConfig();
    resetScope();
    clearSignals();
    setConfig(baseConfig);
    startTransport();
  });
  afterEach(() => {
    resetTransport();
    resetConfig();
    resetScope();
    clearSignals();
  });

  it('error coerces anything and enqueues with a stack', () => {
    const id = verbs.error(new Error('boom'));
    expect(typeof id).toBe('string');
    const q = __peekQueue();
    expect(q.length).toBe(1);
    expect(q[0]?.kind).toBe('error');
    expect(q[0]?.payload.error?.type).toBe('Error');
    expect(q[0]?.payload.error?.message).toBe('boom');
  });

  it('error serializes Error instances found in data (error-in-data)', () => {
    verbs.warn('pay.retry', { attempt: 2, error: new TypeError('nope') });
    const q = __peekQueue();
    const data = q[0]?.payload.data as { error?: { type?: string } };
    expect(data.error?.type).toBe('TypeError');
  });

  it('warn carries name; trace quiet stays out of the queue', () => {
    verbs.warn('rage_tap');
    expect(__peekQueue().length).toBe(1);
    verbs.trace('checkout.start', undefined, { quiet: true });
    expect(__peekQueue().length).toBe(1); // unchanged
    verbs.trace('checkout.start');
    expect(__peekQueue().length).toBe(2);
  });

  it('assert pass aggregates without an event; fail is an event', () => {
    verbs.assert('total-positive', true);
    expect(__peekQueue().length).toBe(0);
    expect(__peekAssertStats()[0]?.passDelta).toBe(1);
    verbs.assert('total-positive', false);
    expect(__peekQueue().length).toBe(1);
    expect(__peekQueue()[0]?.kind).toBe('assert');
  });

  it('probe never throws and reports the ref as name', () => {
    const id = verbs.probe('SENT-123');
    expect(typeof id).toBe('string');
    expect(__peekQueue()[0]?.kind).toBe('probe');
    expect(__peekQueue()[0]?.name).toBe('SENT-123');
  });

  it('before init everything is a silent no-op that still returns an id', () => {
    resetConfig();
    const id = verbs.error(new Error('x'));
    expect(typeof id).toBe('string');
    expect(__peekQueue().length).toBe(0);
  });
});

describe('dev symbolication wiring', () => {
  beforeEach(() => {
    resetTransport();
    resetConfig();
    resetScope();
    clearSignals();
    setConfig(baseConfig);
    startTransport();
  });
  afterEach(() => {
    resetTransport();
    resetConfig();
    resetScope();
    clearSignals();
    delete (globalThis as { __DEV__?: boolean }).__DEV__;
  });

  it('in __DEV__ an error is enqueued asynchronously, never lost', async () => {
    (globalThis as { __DEV__?: boolean }).__DEV__ = true;
    const id = verbs.error(new Error('dev boom'));
    expect(typeof id).toBe('string');
    // Held out of the batch while Metro symbolication settles…
    expect(__peekQueue().length).toBe(0);
    // …and lands afterwards (no Metro in tests → original stack).
    await new Promise((r) => setTimeout(r, 20));
    expect(__peekQueue().length).toBe(1);
    expect(__peekQueue()[0]?.payload.error?.message).toBe('dev boom');
  });

  it('outside __DEV__ the error path stays fully synchronous', () => {
    verbs.error(new Error('prod boom'));
    expect(__peekQueue().length).toBe(1);
  });

  it('a warn carrying error-in-data also takes the deferred path in dev', async () => {
    (globalThis as { __DEV__?: boolean }).__DEV__ = true;
    verbs.warn('pay.retry', { error: new TypeError('gateway 503') });
    expect(__peekQueue().length).toBe(0);
    await new Promise((r) => setTimeout(r, 20));
    expect(__peekQueue().length).toBe(1);
  });
});
