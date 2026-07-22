// Phase 46 — singleton TrailBuffer for the RN SDK.
//
// Kept in its own module so callers (including navigation.ts, which
// is intentionally lightweight) can record steps without pulling in
// capture.ts → handlers/screenshot.ts → react-native. The buffer is
// drained inside capture.ts when an event captures and
// `sessionTrailEnabled` is on.

import { TrailBuffer, type TrailStep } from '@goliapkg/sentori-core';

const _trail = new TrailBuffer(30);

/**
 * Phase 46 — record a step into the session-trail buffer. The buffer
 * is a fixed-size FIFO (default 30 steps); pushing past capacity
 * drops the oldest entry. Steps are only uploaded if
 * `init({ capture: { sessionTrail: true } })` is on AND a
 * `captureException` follows.
 */
export const captureStep = (label: string, opts?: Partial<TrailStep>): void => {
  _trail.push({
    ts: Date.now(),
    label,
    ...(opts ?? {}),
  });
};

export const getTrailBuffer = (): TrailBuffer => _trail;

export const __resetTrailForTests = (): void => {
  _trail.clear();
};
