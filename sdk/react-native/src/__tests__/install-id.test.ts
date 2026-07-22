// v1.1 chunk S1 — install-id module persistence + race-safety.

import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
} from 'bun:test';

import {
  __resetInstallIdForTests,
  getInstallId,
  peekInstallId,
} from '../install-id';

describe('install-id', () => {
  beforeEach(() => {
    __resetInstallIdForTests();
  });

  afterEach(() => {
    __resetInstallIdForTests();
  });

  it('peekInstallId is null before first resolve', () => {
    expect(peekInstallId()).toBe(null);
  });

  it('getInstallId generates a stable id and caches it', async () => {
    const first = await getInstallId();
    expect(typeof first).toBe('string');
    expect(first.length).toBeGreaterThan(10);
    // sync peek after resolve returns the same value
    expect(peekInstallId()).toBe(first);
    // second call returns the cached value
    expect(await getInstallId()).toBe(first);
  });

  it('concurrent callers share the same resolve promise', async () => {
    const [a, b, c] = await Promise.all([
      getInstallId(),
      getInstallId(),
      getInstallId(),
    ]);
    expect(a).toBe(b);
    expect(b).toBe(c);
  });

  it('produces UUIDv7 shape (variant + version nibble)', async () => {
    const id = await getInstallId();
    // RFC 4122 dash positions
    expect(id[8]).toBe('-');
    expect(id[13]).toBe('-');
    expect(id[14]).toBe('7'); // version
    expect(id[18]).toBe('-');
    // variant nibble is 8|9|a|b (high 2 bits = 10)
    expect(['8', '9', 'a', 'b']).toContain(id[19]);
  });
});
