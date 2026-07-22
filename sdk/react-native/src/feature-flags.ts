// v0.9.0 #13 — feature-flag dimensionality.
//
// `sentori.setFeatureFlag(name, value)` is the dual of `setTag()` for
// experiment / flag state: distinct dashboard dimension, dense small
// strings, designed to be filtered/faceted on. Bugsnag's analog has
// proven surprisingly load-bearing. Implementation is a tiny in-memory
// map; every capture rides along the current snapshot.
//
// Constraints (silent — never throw):
//   • name and value are strings, length 1..200
//   • cap at 50 distinct flags to bound payload
//   • already-set name can update (no cap check)

const MAX_FLAGS = 50;
const MAX_LEN = 200;

let _flags = new Map<string, string>();

export const setFeatureFlag = (name: string, value: string): void => {
  if (typeof name !== 'string' || name.length === 0 || name.length > MAX_LEN) return;
  if (typeof value !== 'string' || value.length > MAX_LEN) return;
  if (_flags.size >= MAX_FLAGS && !_flags.has(name)) return;
  _flags.set(name, value);
};

export const clearFeatureFlag = (name: string): void => {
  _flags.delete(name);
};

export const clearAllFeatureFlags = (): void => {
  _flags.clear();
};

export const getFeatureFlags = (): Record<string, string> => {
  return Object.fromEntries(_flags);
};

/** Internal — capture.ts pulls a snapshot per event. Empty object
 *  collapses out of the payload via `Object.keys.length === 0` check. */
export const getFeatureFlagSnapshot = (): null | Record<string, string> => {
  if (_flags.size === 0) return null;
  return Object.fromEntries(_flags);
};

export const __resetFeatureFlagsForTests = (): void => {
  _flags.clear();
};
