// Screen masking — the privacy half of visual replay.
//
// The host registers a query returning the `nativeID`s of views
// that must never appear in a screenshot (camera feeds, user
// identity, payment fields). On iOS the native matcher accepts the
// same value via either `nativeID` or `testID` (RN maps testID to
// accessibilityIdentifier); on Android `nativeID` rides the view
// tag. One prop, both platforms: use `nativeID`. Native paints black rectangles over
// those subtrees in the same render pass, so the pixels never
// leave the device.
//
// The query runs on every frame tick; keep it cheap (return a
// cached array). A throwing query is swallowed and masks NOTHING
// that tick — so a broken query fails visible-in-review rather
// than silently, and can never take the capture path down.

import { reportInternal } from '@goliapkg/sentori-core';

type MaskQuery = () => string[];

let _query: MaskQuery | null = null;

/** Register (or with `null`, clear) the mask query. */
export const registerMaskQuery = (query: MaskQuery | null): void => {
  _query = query;
};

/** The current mask list; empty when unregistered or throwing. */
export const maskedNativeIds = (): string[] => {
  if (!_query) return [];
  try {
    const ids = _query();
    return Array.isArray(ids) ? ids.filter((x) => typeof x === 'string') : [];
  } catch (e) {
    reportInternal('mask-query', e);
    return [];
  }
};

export const __resetForTests = (): void => {
  _query = null;
};
