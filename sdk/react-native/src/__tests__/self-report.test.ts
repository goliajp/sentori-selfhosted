// The SDK's own faults reach the server.
//
// `reportInternal` was written, `setInternalReporter` was exported, and
// nothing ever called the setter — so a failure inside a public verb
// logged to the host's console and stopped there. Every piece was
// present and the chain was open in the middle, which no unit test of
// either end could show.
//
// So the test drives the public API, makes it fail the way a host
// would by accident, and looks in the queue.

import { afterEach, beforeEach, describe, expect, it } from 'bun:test';

import { __resetCircuitForTests } from '@goliapkg/sentori-core';

import { __resetForTests as resetConfig } from '../config';
import { init, __resetForTests as resetInit } from '../init';
import { __resetForTests as resetScope } from '../scope';
import { registerEmitHook, __resetForTests as resetHooks } from '../emit-hooks';
import { flush, __peekQueue, __resetForTests as resetTransport } from '../transport';
import { verbs } from '../verbs';

const config = {
  token: 'st_test',
  ingestUrl: 'http://localhost:18080',
  release: 'app@1.0.0',
  environment: 'test',
};

/** Data whose own shape throws when anything walks it — the accidental
 *  version of this is a getter that reads a field of something already
 *  torn down. */
const boobyTrapped = () => ({
  get detail(): string {
    throw new Error('reading this blows up');
  },
});

describe('an SDK-internal failure becomes an event', () => {
  // The budget test enqueues enough faults to reach the transport's
  // batch size, so a flush really happens. Left to the real `fetch` it
  // fails, counts ten dropped events, and lands *after* this file has
  // finished — which showed up as `transport.test.ts` seeing 12 drops
  // where it expected 2. A test that poisons a later file is a worse
  // problem than the one it was written to catch.
  let realFetch: typeof fetch;

  beforeEach(() => {
    realFetch = globalThis.fetch;
    globalThis.fetch = (async () =>
      new Response(JSON.stringify({ accepted: 0, outcomes: [] }), {
        status: 202,
        headers: { 'content-type': 'application/json' },
      })) as typeof fetch;
    resetTransport();
    resetHooks();
    resetScope();
    resetConfig();
    resetInit();
    __resetCircuitForTests();
    init(config);
  });
  afterEach(async () => {
    // Let any scheduled flush settle against the stub before the real
    // `fetch` goes back.
    await flush();
    globalThis.fetch = realFetch;
    resetTransport();
    resetInit();
  });

  it('files the fault as a warn the host can find', () => {
    expect(() => verbs.error(new Error('host error'), boobyTrapped())).not.toThrow();

    const faults = __peekQueue().filter((e) => e.name === 'sentori.internal');
    expect(faults.length).toBeGreaterThan(0);
    const fault = faults[0]!;
    // `warn`, not `error`: the host's app did not break, ours did, and
    // filing it as an error puts our bug at the top of their inbox
    // wearing their name.
    expect(fault.kind).toBe('warn');
    expect(fault.payload.data?.api).toBe('error');
    expect(typeof fault.payload.data?.message).toBe('string');
  });

  it('does not report a verb that worked', () => {
    verbs.error(new Error('ordinary'), { cartId: 'c_1' });
    expect(__peekQueue().filter((e) => e.name === 'sentori.internal')).toHaveLength(0);
  });

  it('stops after the budget rather than reporting its own reporting', () => {
    // Counted at the emit hook, not in the queue. The transport
    // flushes at its batch size, so a queue-length assertion here
    // would have read zero and looked like "no faults were reported"
    // when the truth is they had already been sent — a probe that
    // fails the same way as the defect it is watching for.
    const seen: string[] = [];
    registerEmitHook((e) => {
      if (e.name === 'sentori.internal') seen.push(e.id);
    });
    // A transport that is itself broken would otherwise make each
    // report produce the next one. The cap is what makes that finite.
    for (let i = 0; i < 40; i += 1) verbs.error(new Error('e'), boobyTrapped());
    expect(seen.length).toBeGreaterThan(0);
    expect(seen.length).toBeLessThanOrEqual(10);
  });
});
