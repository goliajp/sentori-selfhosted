import { afterEach, describe, expect, test } from 'bun:test';

import {
  __resetFeatureFlagsForTests,
  clearAllFeatureFlags,
  clearFeatureFlag,
  getFeatureFlagSnapshot,
  getFeatureFlags,
  setFeatureFlag,
} from '../feature-flags';

afterEach(() => {
  __resetFeatureFlagsForTests();
});

describe('feature flags', () => {
  test('set / get round-trip', () => {
    setFeatureFlag('checkout-v2', 'variant-a');
    setFeatureFlag('shipping', 'fast');
    expect(getFeatureFlags()).toEqual({ 'checkout-v2': 'variant-a', shipping: 'fast' });
  });

  test('clear removes only the named flag', () => {
    setFeatureFlag('a', '1');
    setFeatureFlag('b', '2');
    clearFeatureFlag('a');
    expect(getFeatureFlags()).toEqual({ b: '2' });
  });

  test('clearAll empties the map', () => {
    setFeatureFlag('a', '1');
    clearAllFeatureFlags();
    expect(getFeatureFlags()).toEqual({});
  });

  test('snapshot returns null when empty (so capture can elide the field)', () => {
    expect(getFeatureFlagSnapshot()).toBeNull();
    setFeatureFlag('x', 'y');
    expect(getFeatureFlagSnapshot()).toEqual({ x: 'y' });
  });

  test('rejects oversize names / values silently', () => {
    setFeatureFlag('x'.repeat(201), 'v');
    setFeatureFlag('name', 'v'.repeat(201));
    expect(getFeatureFlags()).toEqual({});
  });

  test('respects the 50-flag cap but updates existing flags freely', () => {
    for (let i = 0; i < 50; i++) setFeatureFlag(`flag-${i}`, 'a');
    setFeatureFlag('flag-overflow', 'a'); // rejected silently
    expect(Object.keys(getFeatureFlags()).length).toBe(50);
    setFeatureFlag('flag-0', 'b'); // update — allowed
    expect(getFeatureFlags()['flag-0']).toBe('b');
  });
});
