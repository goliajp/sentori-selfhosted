import { afterEach, describe, expect, test } from 'bun:test';

import {
  __resetStateSnapshotsForTests,
  bindState,
  getStateSnapshots,
  recordState,
  shallowDiff,
  unbindState,
} from '../state-snapshots';

afterEach(() => {
  __resetStateSnapshotsForTests();
});

describe('shallowDiff', () => {
  test('emits only changed top-level keys', () => {
    const a = { foo: 1, bar: 2 };
    const b = { bar: 2, foo: 3 };
    expect(shallowDiff(a, b)).toEqual({ foo: 3 });
  });

  test('emits new keys from next', () => {
    expect(shallowDiff({ a: 1 }, { a: 1, b: 2 })).toEqual({ b: 2 });
  });

  test('treats deletion as undefined', () => {
    expect(shallowDiff({ a: 1, b: 2 }, { a: 1 })).toEqual({ b: undefined });
  });

  test('no change returns empty object', () => {
    expect(shallowDiff({ a: 1 }, { a: 1 })).toEqual({});
  });

  test('non-object prev → full clone of next', () => {
    expect(shallowDiff(undefined, { a: 1 })).toEqual({ a: 1 });
  });
});

describe('recordState', () => {
  test('appends snapshot with default source=manual', () => {
    recordState({ cart: 3 });
    const snaps = getStateSnapshots();
    expect(snaps.length).toBe(1);
    expect(snaps[0]!.source).toBe('manual');
    expect(snaps[0]!.diff).toEqual({ cart: 3 });
  });

  test('caps at 50 snapshots (ring buffer)', () => {
    for (let i = 0; i < 60; i++) recordState({ n: i });
    expect(getStateSnapshots().length).toBe(50);
    // oldest 10 dropped; first kept entry has n=10
    expect(getStateSnapshots()[0]!.diff).toEqual({ n: 10 });
  });
});

describe('bindState', () => {
  test('subscribes to a redux-like store and records diffs', () => {
    let state: { count: number } = { count: 0 };
    const listeners: (() => void)[] = [];
    const store = {
      getState: () => state,
      subscribe: (cb: () => void) => {
        listeners.push(cb);
        return () => {
          const i = listeners.indexOf(cb);
          if (i >= 0) listeners.splice(i, 1);
        };
      },
    };
    bindState({ redux: store });
    state = { count: 1 };
    listeners.forEach((l) => l());
    state = { count: 2 };
    listeners.forEach((l) => l());

    const snaps = getStateSnapshots();
    expect(snaps.length).toBe(2);
    expect(snaps[0]!.diff).toEqual({ count: 1 });
    expect(snaps[1]!.diff).toEqual({ count: 2 });
    expect(snaps[0]!.source).toBe('redux');
  });

  test('unbindState stops recording', () => {
    let state: { v: number } = { v: 0 };
    const listeners: (() => void)[] = [];
    const store = {
      getState: () => state,
      subscribe: (cb: () => void) => {
        listeners.push(cb);
        return () => listeners.splice(listeners.indexOf(cb), 1);
      },
    };
    bindState({ redux: store });
    state = { v: 1 };
    listeners.forEach((l) => l());
    unbindState();
    state = { v: 2 };
    listeners.forEach((l) => l());

    expect(getStateSnapshots().length).toBe(1);
  });

  test('custom stores produce labeled snapshots', () => {
    let auth = { user: null };
    const listeners: (() => void)[] = [];
    const store = {
      getState: () => auth,
      subscribe: (cb: () => void) => {
        listeners.push(cb);
        return () => listeners.splice(listeners.indexOf(cb), 1);
      },
    };
    bindState({ custom: { auth: store } });
    auth = { user: 'alice' as never };
    listeners.forEach((l) => l());
    expect(getStateSnapshots()[0]!.source).toBe('auth');
  });
});
