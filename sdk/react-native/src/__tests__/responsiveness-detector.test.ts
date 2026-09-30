// dead_button / sluggish_button verdict logic.

import { describe, expect, test } from 'bun:test';

import {
  DEAD_THRESHOLD,
  DEAD_WINDOW_MS,
  RESPONSE_WINDOW_MS,
  SLUGGISH_COOLDOWN_MS,
  SLUGGISH_MS,
  classifyTap,
  recordDeadTap,
  recordSluggish,
} from '../responsiveness-detector';

describe('classifyTap', () => {
  test('a fast reaction is responsive', () => {
    expect(classifyTap(1000, [1300])).toBe('responsive');
  });

  test('a slow reaction is sluggish', () => {
    expect(classifyTap(1000, [1000 + SLUGGISH_MS + 200])).toBe('sluggish');
  });

  test('no reaction inside the window is dead', () => {
    expect(classifyTap(1000, [])).toBe('dead');
    expect(classifyTap(1000, [1000 + RESPONSE_WINDOW_MS + 500])).toBe('dead');
  });

  test('signals from before the tap do not count', () => {
    expect(classifyTap(1000, [900, 500])).toBe('dead');
  });

  test('the earliest reaction decides', () => {
    expect(classifyTap(1000, [1000 + SLUGGISH_MS + 400, 1200])).toBe('responsive');
  });
});

describe('recordDeadTap', () => {
  test('warns only at the threshold, then resets', () => {
    const buckets = new Map<number, number[]>();
    let warned = 0;
    for (let i = 0; i < DEAD_THRESHOLD * 2; i++) {
      if (recordDeadTap(buckets, 7, 1000 + i * 100)) warned += 1;
    }
    expect(warned).toBe(2);
  });

  test('stale dead taps age out of the window', () => {
    const buckets = new Map<number, number[]>();
    recordDeadTap(buckets, 7, 0);
    recordDeadTap(buckets, 7, 100);
    // Third tap arrives after the window: the old two are gone.
    expect(recordDeadTap(buckets, 7, DEAD_WINDOW_MS + 200)).toBe(false);
  });

  test('targets are independent', () => {
    const buckets = new Map<number, number[]>();
    recordDeadTap(buckets, 1, 0);
    recordDeadTap(buckets, 1, 10);
    expect(recordDeadTap(buckets, 2, 20)).toBe(false);
    expect(recordDeadTap(buckets, 1, 30)).toBe(true);
  });
});

describe('recordSluggish', () => {
  test('cooldown gates repeat warns per target', () => {
    const warns = new Map<number, number>();
    expect(recordSluggish(warns, 7, 1000)).toBe(true);
    expect(recordSluggish(warns, 7, 2000)).toBe(false);
    expect(recordSluggish(warns, 7, 1000 + SLUGGISH_COOLDOWN_MS + 1)).toBe(true);
    expect(recordSluggish(warns, 8, 2000)).toBe(true);
  });
});
