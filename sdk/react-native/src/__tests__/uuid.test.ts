import { describe, it, expect } from 'bun:test';

import { uuidV7 } from '../uuid';

describe('uuidV7', () => {
  it('produces a 36-char hyphenated UUID with version 7', () => {
    const u = uuidV7();
    expect(u).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
    );
  });

  it('encodes the current ms timestamp in the leading 48 bits', () => {
    const before = Date.now();
    const u = uuidV7();
    const after = Date.now();
    const tsHex = u.replace(/-/g, '').slice(0, 12);
    const ts = parseInt(tsHex, 16);
    expect(ts).toBeGreaterThanOrEqual(before);
    expect(ts).toBeLessThanOrEqual(after);
  });

  it('produces unique values across rapid calls', () => {
    const seen = new Set<string>();
    for (let i = 0; i < 1000; i++) seen.add(uuidV7());
    expect(seen.size).toBe(1000);
  });

  it('always sets version 7 nibble', () => {
    for (let i = 0; i < 100; i++) {
      expect(uuidV7().charAt(14)).toBe('7');
    }
  });

  it('always sets variant to 10xx', () => {
    for (let i = 0; i < 100; i++) {
      const ch = uuidV7().charAt(19).toLowerCase();
      expect('89ab'.includes(ch)).toBe(true);
    }
  });
});
