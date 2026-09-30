// The denominator. Without these, the crash-free rate is a number
// computed from a stream nothing sends.
import { afterEach, describe, expect, test } from 'bun:test';

import {
  __resetSessionsForTests,
  __setStorageForTests,
  endSession,
  recoverSession,
  startSession,
} from '../sessions';
import { __peekSessions, __resetForTests as resetTransport } from '../transport';

const ctx = {
  environment: 'production',
  platform: 'ios' as const,
  release: 'app@1.0.0+1',
  userId: null,
};

const memoryStore = () => {
  const map = new Map<string, string>();
  return {
    dump: map,
    getItem: (k: string) => Promise.resolve(map.get(k) ?? null),
    removeItem: (k: string) => {
      map.delete(k);
      return Promise.resolve();
    },
    setItem: (k: string, v: string) => {
      map.set(k, v);
      return Promise.resolve();
    },
  };
};

afterEach(() => {
  __resetSessionsForTests();
  resetTransport();
});

describe('sessions', () => {
  test('a clean run reports exited and leaves nothing behind', async () => {
    const store = memoryStore();
    __setStorageForTests(store);
    startSession(ctx);
    await Promise.resolve();
    expect(store.dump.size).toBe(1);

    endSession();
    const queued = __peekSessions();
    expect(queued).toHaveLength(1);
    expect(queued[0]!.status).toBe('exited');
    expect(queued[0]!.platform).toBe('ios');
    await Promise.resolve();
    expect(store.dump.size).toBe(0);
  });

  test('a launch that never ended is a crash only when a crash file backs it', async () => {
    // The OS reclaiming memory and the user swiping the app away look
    // identical from here. Counting those as crashes would understate
    // the rate for something that is not the app's fault.
    const store = memoryStore();
    __setStorageForTests(store);
    startSession(ctx);
    await Promise.resolve();

    __resetSessionsForTests();
    __setStorageForTests(store);
    await recoverSession(false);
    expect(__peekSessions()[0]!.status).toBe('exited');
  });

  test('and is a crash when one does', async () => {
    const store = memoryStore();
    __setStorageForTests(store);
    startSession(ctx);
    await Promise.resolve();

    __resetSessionsForTests();
    __setStorageForTests(store);
    await recoverSession(true);
    const queued = __peekSessions();
    expect(queued).toHaveLength(1);
    expect(queued[0]!.status).toBe('crashed');
    expect(queued[0]!.release).toBe('app@1.0.0+1');
  });

  test('a recovered session is reported once, not on every launch', async () => {
    const store = memoryStore();
    __setStorageForTests(store);
    startSession(ctx);
    await Promise.resolve();

    __resetSessionsForTests();
    __setStorageForTests(store);
    await recoverSession(true);
    await recoverSession(true);
    expect(__peekSessions()).toHaveLength(1);
  });

  test('the id is a uuid, because a malformed one would fail the whole envelope', async () => {
    // `WireSession.id` is a `Uuid` on the server and the sessions ride
    // the same body as the events. A bad id there used to take a batch
    // of crash reports down with it.
    const store = memoryStore();
    __setStorageForTests(store);
    startSession(ctx);
    await Promise.resolve();
    __resetSessionsForTests();
    __setStorageForTests(store);
    await recoverSession(true);
    expect(__peekSessions()[0]!.id).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/,
    );
  });

  test('no storage means no crash recovery, not a crash', async () => {
    // A host without AsyncStorage still counts the sessions it ends
    // cleanly; it just cannot recognise one that never ended.
    __setStorageForTests(null);
    await recoverSession(true);
    expect(__peekSessions()).toHaveLength(0);
  });
});
