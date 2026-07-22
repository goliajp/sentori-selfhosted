// v0.7.3 — mask discovery via consumer-supplied query callback.
//
// The SDK does not own a registry of masked regions. The host app
// keeps its own (e.g. a `Set<string>` updated as `<Maskable>`
// components mount/unmount) and hands the SDK a thunk that returns
// the current list of native-IDs to redact. The SDK calls the thunk
// once per screenshot capture — cheap, called rarely, only on error.
//
// Why this shape: a logging SDK should never live on the render
// path. Earlier iterations exported `<MaskRegion>` (React component)
// and `setMaskedNode` (imperative ref helper), which forced every
// PII-bearing UI file to import from the SDK and put SDK bugs in
// the user's render tree. This module is JS-only — no React, no JSX,
// no native module touch — so swapping or removing the SDK doesn't
// affect rendering.

type MaskQuery = () => string[];

let _query: MaskQuery | null = null;

/**
 * Register a callback the SDK calls right before each screenshot
 * capture. Return the native-IDs (the `nativeID` prop on the RN
 * `<View>`) that should be blacked-out in the captured image.
 *
 * Idempotent: a second call replaces the first. Pass `null` (or
 * call `clearMaskQuery`) to detach.
 */
export function registerMaskQuery(query: MaskQuery): void {
  _query = query;
}

/** Unregister. Mostly for tests / teardown. */
export function clearMaskQuery(): void {
  _query = null;
}

/** Internal — read by `handlers/screenshot.ts` at capture time. */
export function getRegisteredMaskQuery(): MaskQuery | null {
  return _query;
}
