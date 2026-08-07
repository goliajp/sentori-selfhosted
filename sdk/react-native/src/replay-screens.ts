// B-type visual replay — a rolling ring of low-bitrate screenshots.
//
// OFF by default (privacy + the client-zero-cost rule): enable
// with `init({ replayScreens: true })`. One frame every 2.5 s at
// 360 px / q≈0.35 runs 10-20 KB, so the 60 s window a triage
// actually wants is ~24 frames and a few hundred KB — and those
// bytes only ever leave the device when an error/warn fires,
// stapled to that event as a `screens` attachment.
//
// Older native builds have no `captureReplayFrame`; the ring then
// simply stays empty (one warn) — wireframe replay still works.

import { logger } from '@goliapkg/sentori-core';

import { maskedNativeIds } from './mask';
import { captureNativeReplayFrame } from './native';

type CaptureFn = typeof captureNativeReplayFrame;
let _capture: CaptureFn = captureNativeReplayFrame;

/** Frame cadence. 2.5 s keeps the main-thread cost of a capture
 *  (~1-3 ms) far under the 1 % occupancy budget. */
const TICK_INTERVAL_MS = 2_500;
/** Long edge of a replay frame, px. Enough to read a screen's
 *  layout and large text; deliberately not enough to read a
 *  document over someone's shoulder. */
const FRAME_LONG_EDGE_PX = 360;
/** JPEG/WebP quality for replay frames. */
const FRAME_QUALITY = 0.35;

type ScreenFrame = {
  t: number;
  base64: string;
  mediaType: string;
  /** Window logical size (pt/dp), same space as tap pageX/pageY —
   *  present from native ≥ 5.4; older binaries omit it and the
   *  dashboard simply draws no tap markers on pixels. */
  w?: number;
  h?: number;
};

let _ring: ScreenFrame[] = [];
let _capacity = 0;
let _timer: ReturnType<typeof setInterval> | null = null;
let _capturing = false;

/** Start the ring. `windowSeconds` is how far back the replay
 *  reaches when an event fires. */
export const startScreenReplay = (windowSeconds: number): void => {
  if (_timer !== null || windowSeconds <= 0) return;
  _capacity = Math.max(1, Math.ceil((windowSeconds * 1000) / TICK_INTERVAL_MS));
  _timer = setInterval(() => {
    void tick();
  }, TICK_INTERVAL_MS);
  logger.debug('replay-screens', `ring started: ${_capacity} slots / ${windowSeconds}s`);
};

const tick = async (): Promise<void> => {
  // Never overlap captures: a slow frame skips a beat instead of
  // queueing main-thread work.
  if (_capturing) return;
  _capturing = true;
  try {
    const frame = await _capture(maskedNativeIds(), FRAME_LONG_EDGE_PX, FRAME_QUALITY);
    if (frame) {
      _ring.push({ t: Date.now(), ...frame });
      if (_ring.length > _capacity) _ring.splice(0, _ring.length - _capacity);
    }
  } finally {
    _capturing = false;
  }
};

/** Drain the ring into the wire form: NDJSON, one frame per line,
 *  `t` rewritten to seconds-before-now (negative, so the player
 *  reads "-42.5s → 0s"). Returns null when empty. The ring is NOT
 *  cleared — a second error two seconds later should still see
 *  the minute before it. */
export const drainScreenReplay = (): null | string => {
  if (_ring.length === 0) return null;
  const now = Date.now();
  return _ring
    .map((f) =>
      JSON.stringify({
        t: Number(((f.t - now) / 1000).toFixed(1)),
        mediaType: f.mediaType,
        ...(typeof f.w === 'number' && typeof f.h === 'number'
          ? { w: f.w, h: f.h }
          : {}),
        base64: f.base64,
      }),
    )
    .join('\n');
};

export const __resetForTests = (): void => {
  if (_timer !== null) clearInterval(_timer);
  _timer = null;
  _ring = [];
  _capacity = 0;
  _capturing = false;
  _capture = captureNativeReplayFrame;
};

export const __setCaptureForTests = (fn: CaptureFn): void => {
  _capture = fn;
};
