// v0.8.5 — shared guard for "JS package is in node_modules but native
// module isn't linked".
//
// The pattern that bit us in 0.8.0–0.8.3 (NetInfo, AsyncStorage,
// expo-sensors, ...): host had the JS package via npm hoisting (or
// via our old `optionalDependencies` mistake) but never ran
// `pod install` / `prebuild` / `react-native link`. We `require(...)`
// successfully, call a method, then the lib's internal native bridge
// access throws — and most of those errors arrive on an emitter or
// microtask where our try/catch can't reach.
//
// Fix: before requireing such a package, check that the registered
// NativeModule actually exists. `null` → skip the whole feature; cost
// is one nullable map read.

export function isNativeModuleLinked(name: string): boolean {
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const RN = require('react-native') as {
      NativeModules?: Record<string, unknown>;
    };
    return RN.NativeModules?.[name] != null;
  } catch {
    return false;
  }
}

/** Some native modules are registered under multiple alternative names
 *  across RN versions / Expo packaging. Returns `true` iff *any* name
 *  is linked. */
export function isAnyNativeModuleLinked(names: string[]): boolean {
  return names.some(isNativeModuleLinked);
}
