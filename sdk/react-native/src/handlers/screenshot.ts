// v0.7.3 — capture a screenshot of the current view tree on
// `captureException`. Off-main-thread, best-effort, opt-in.
//
// JS owns the registry of which regions to redact: the host app
// passes a thunk via `sentori.registerMaskQuery(() => string[])` that
// returns the `nativeID`s currently mounted as masked. We call it
// once per capture and forward the list to the native module, which
// renders the bitmap and paints black rectangles over the matching
// subviews in a single pass.
//
// History: pre-v0.7.3 went through `react-native-view-shot` (peer
// dep) and used a JS-side overlay-opacity trick (`<MaskRegion>` /
// `setMaskedNode`) to hide PII before snapshotting. That design put
// the SDK on the render path; a single SDK bug could break the host
// app's UI. v0.7.3 cuts that coupling — the SDK no longer ships
// React components, and the screenshot path runs entirely through
// the native module already used for native-crash captures.
//
// Performance:
//   - Yield one paint via `requestAnimationFrame` before the native
//     call so post-error UI state has committed.
//   - 480 px on the longest edge, JPEG q=70 (iOS) / WEBP_LOSSY q=70
//     (Android 11+). Typical payload 30-80 KB; multipart hard cap
//     is 500 KB.
//   - On any failure we silently return null. The error event still
//     goes to the server; the user just doesn't see a thumbnail.

import { getRegisteredMaskQuery } from '../mask';
import { captureNativeScreenshotWithMask } from '../native';

/** What `captureScreenshot()` hands back when it succeeds. */
export type ScreenshotBlob = {
  base64: string;
  mediaType: string;
};

/**
 * Take one screenshot, yielding the JS thread first. Returns null on
 * any error (no native module bound, native side refused, capture
 * timed out, etc.). Caller is responsible for opt-in checks
 * (`config.screenshotsEnabled`).
 */
export async function captureScreenshot(): Promise<ScreenshotBlob | null> {
  // Yield one paint frame so the post-error UI has committed before
  // we ask the OS to snapshot it.
  await new Promise<void>((resolve) => {
    requestAnimationFrame(() => resolve());
  });

  // Read the consumer-supplied mask query once per capture. If
  // the host never called `registerMaskQuery`, no mask is applied
  // and the full screenshot ships — sane default: SDK does nothing
  // unless told to.
  const query = getRegisteredMaskQuery();
  let maskedIds: string[] = [];
  if (query) {
    try {
      maskedIds = query();
    } catch {
      // A throwing query is the host's bug, not ours; skip mask
      // rather than skip the screenshot.
      maskedIds = [];
    }
  }

  const result = await captureNativeScreenshotWithMask(maskedIds);
  if (!result) return null;
  return { base64: result.base64, mediaType: result.mediaType };
}
