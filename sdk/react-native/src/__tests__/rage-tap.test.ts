import { describe, expect, test } from 'bun:test';

import { recordTap } from '../rage-tap-detector';

describe('recordTap', () => {
  test('two fast taps do not trip rage', () => {
    const m = new Map<number, number[]>();
    expect(recordTap(m, 7, 0)).toBe(false);
    expect(recordTap(m, 7, 200)).toBe(false);
    expect(m.get(7)?.length).toBe(2);
  });

  test('three fast taps on the same target trip rage', () => {
    const m = new Map<number, number[]>();
    recordTap(m, 7, 0);
    recordTap(m, 7, 200);
    expect(recordTap(m, 7, 400)).toBe(true);
    // bucket cleared so the very next tap doesn't immediately re-trip
    expect(m.get(7)).toBeUndefined();
  });

  test('taps spread over > 800 ms do not trip', () => {
    const m = new Map<number, number[]>();
    recordTap(m, 7, 0);
    recordTap(m, 7, 500);
    expect(recordTap(m, 7, 1500)).toBe(false);
    // only the last (since it landed > 800ms after the previous two)
    expect(m.get(7)?.length).toBe(1);
  });

  test('different targets do not pool', () => {
    const m = new Map<number, number[]>();
    recordTap(m, 1, 0);
    recordTap(m, 2, 0);
    recordTap(m, 3, 0);
    expect(m.size).toBe(3);
  });
});
