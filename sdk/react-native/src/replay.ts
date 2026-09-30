// rc.9 — wireframe Session Replay v2 encoding.
//
// Replaces rc.8's "one full snapshot per tick" with keyframe + delta:
//   - Native walker still emits a full snapshot string every tick.
//   - JS parses the snapshot, builds a fingerprint→node map, and either
//     emits a keyframe (cold start / every KEYFRAME_INTERVAL_MS / when
//     the delta would be bigger than a fresh key) OR a delta against
//     the previous emit's reconstructed state.
//   - Static-UI ticks produce zero-delta heartbeats which we drop.
//
// At 4 Hz capture / 4 s keyframes the wire bytes drop ~50 % vs rc.8 at
// 1 Hz for the same 60 s pre-error window, and the dashboard player
// cross-fades between captures at 24 fps so playback reads as motion
// rather than a 1 Hz slideshow.
//
// Wire schema: docs/replay-encoding-v2.md.

import {
  computeReplayDelta,
  indexReplayNodes,
  logger,
  ReplayRing,
} from '@goliapkg/sentori-core';
import type { ReplayFrame, ReplayNode } from '@goliapkg/sentori-core';

import { maskedNativeIds } from './mask';
import { describeWireframeNative } from './native';

declare const __DEV__: boolean | undefined;

/** Default capture interval (2 Hz). Override via `replay.hz`. rc.10
 *  rolled this back from rc.9's 4 Hz default: iOS sim measured 1 ms
 *  per tick on a thin dev panel but extrapolation to a 200-node
 *  Insight-class UI on Android pushes JS-thread occupancy past 1 %,
 *  which violates the "几乎不能造成性能抖动" rule. Apps that want
 *  smoother playback motion can opt into `replay.hz: 4` explicitly. */
const TICK_INTERVAL_MS = 500;

/** How often to emit a fresh keyframe — caps reconstruction chain
 *  length and lets the player re-sync after a dropped line. */
const KEYFRAME_INTERVAL_MS = 4_000;

/** Floor on tick period. < 100 ms the native view-tree walk dominates
 *  the JS thread on mid-tier Android. */
const MIN_TICK_PERIOD_MS = 100;

type Node = ReplayNode;

type NativeFrame = ReplayFrame;

let _ring = new ReplayRing();
let _timer: ReturnType<typeof setInterval> | null = null;
let _running = false;

let _nativeMod: ReplayNativeModule | null = null;

export type ReplayOptions = {
  mode?: 'off' | 'wireframe';
  /** Ticks per second. Default 2. Opt into 4 (or 8) for
   *  motion-heavy apps where playback smoothness matters more than
   *  the marginal CPU saving. */
  hz?: number;
  /** Keyframe cadence in ms. Default 4000. */
  keyframeMs?: number;
};

let _keyframeIntervalMs = KEYFRAME_INTERVAL_MS;

export function startReplay(opts: ReplayOptions): void {
  if (_running) return;
  if (opts.mode !== 'wireframe') return;
  const info = describeWireframeNative();
  if (!info.bound) {
    logger.warn(
      'replay',
      'native module not bound (expo-modules-core); replay attachments will stay empty',
    );
    return;
  }
  logger.debug(
    'replay',
    'starting; bound=', info.bound, 'hasCaptureWireframe=', info.hasCaptureWireframe,
  );
  _running = true;
  _nativeMod = loadNativeReplay();
  _keyframeIntervalMs = opts.keyframeMs ?? KEYFRAME_INTERVAL_MS;
  // A fresh ring, so a restart with a different cadence does not
  // carry the previous one's state or its keyframe clock.
  _ring = new ReplayRing({ keyframeMs: _keyframeIntervalMs });
  const hz = opts.hz ?? 2;
  const period = Math.max(MIN_TICK_PERIOD_MS, Math.round(1000 / hz));
  _timer = setInterval(() => {
    captureTick();
  }, period);
  logger.debug(
    'replay',
    'scheduled; tick period=', period, 'ms keyframe=', _keyframeIntervalMs, 'ms',
  );
}

export function stopReplay(): void {
  _running = false;
  if (_timer !== null) {
    clearInterval(_timer);
    _timer = null;
  }
  _nativeMod = null;
  _emptyTickCount = 0;
  _emptyTickLogStride = 1;
  _firstTickLogged = false;
  _okTickCount = 0;
  _thinTickCount = 0;
  _thinTickLogStride = 1;
}

let _emptyTickCount = 0;
let _emptyTickLogStride = 1;
let _thinTickCount = 0;
let _thinTickLogStride = 1;
let _okTickCount = 0;
let _firstTickLogged = false;

const THIN_RESULT_NODES = 6;

function captureTick(): void {
  if (!_running) return;
  if (!_firstTickLogged) {
    logger.debug('replay', 'tick: first invocation');
    _firstTickLogged = true;
  }
  try {
    const maskIds = readMaskIds();
    const snapshotJson = _nativeMod?.captureWireframe?.(maskIds);
    if (typeof snapshotJson !== 'string' || snapshotJson.length === 0) {
      handleEmptyTick(snapshotJson);
      return;
    }

    let snapshot: NativeFrame;
    try {
      snapshot = JSON.parse(snapshotJson) as NativeFrame;
    } catch (e) {
      logger.warn('replay', 'tick: native JSON parse failed', e);
      return;
    }

    _emptyTickCount = 0;
    _emptyTickLogStride = 1;

    encodeAndPush(snapshot);

    if (typeof __DEV__ !== 'undefined' && __DEV__) {
      diagnosticForTick(snapshot, snapshotJson.length);
    }
  } catch (e) {
    logger.warn('replay', 'tick threw', e);
  }
}

function encodeAndPush(snapshot: NativeFrame): void {
  _ring.push(snapshot);
}

export { computeReplayDelta as computeDelta, indexReplayNodes as indexNodes };

function handleEmptyTick(snapshot: unknown): void {
  _emptyTickCount += 1;
  if (_emptyTickCount === 1 || _emptyTickCount === _emptyTickLogStride) {
    logger.debug(
      'replay',
      'tick empty — native returned',
      snapshot === null
        ? 'null'
        : typeof snapshot === 'string'
          ? `empty (length=${snapshot.length})`
          : typeof snapshot,
      `(empty so far: ${_emptyTickCount})`,
    );
    _emptyTickLogStride = Math.max(_emptyTickLogStride * 10, 10);
  }
}

function diagnosticForTick(snapshot: NativeFrame, snapshotBytes: number): void {
  _okTickCount += 1;
  const nodeCount = snapshot.nodes.length;
  const isThin = nodeCount < THIN_RESULT_NODES;
  if (isThin) {
    _thinTickCount += 1;
    if (_thinTickCount === 1 || _thinTickCount === _thinTickLogStride) {
      logger.debug(
        'replay',
        `tick thin: nodes=${nodeCount} sizeBytes=${snapshotBytes} (thin so far: ${_thinTickCount})`,
      );
      _thinTickLogStride = Math.max(_thinTickLogStride * 10, 10);
    }
  } else {
    _thinTickCount = 0;
    _thinTickLogStride = 1;
  }
  if (_okTickCount === 1) {
    logger.debug('replay', `first ok tick — nodes=${nodeCount} sizeBytes=${snapshotBytes}`);
  }
}

function readMaskIds(): string[] {
  // This returned `[]` unconditionally, under a comment saying
  // masking did not exist yet. It did: `replay-screens.ts` has read
  // the same registry all along. So a host that registered a mask
  // query and turned on wireframe replay got masked screenshots and
  // unmasked wireframes — and a wireframe carries text, which is
  // where a payment field's contents would be.
  return maskedNativeIds();
}

type ReplayNativeModule = {
  captureWireframe?: (maskedIds: string[]) => null | string;
};

function loadNativeReplay(): ReplayNativeModule | null {
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const core = require('expo-modules-core') as {
      requireNativeModule: <T>(name: string) => T;
    };
    return core.requireNativeModule<ReplayNativeModule>('Sentori');
  } catch {
    return null;
  }
}

export function isReplayRunning(): boolean {
  return _running;
}

/** Drain the ring as NDJSON (keyframe or delta per line). Empty
 *  string when the ring is empty. Resets state so the next session's
 *  replay starts with a fresh keyframe. */
export function drainReplay(): string {
  const entries = _ring.drain();
  if (entries.length === 0) return '';
  return entries.map((e) => JSON.stringify(e)).join('\n');
}

/// Drive one tick against an injected capture, so the ids the native
/// module actually receives can be asserted. Without this the mask
/// path had no test at all — which is how it stayed empty under a
/// comment saying masking did not exist.
export function __tickWithNativeForTests(mod: ReplayNativeModule): void {
  _nativeMod = mod;
  _running = true;
  captureTick();
}

export function __resetReplayForTests(): void {
  stopReplay();
  _ring = new ReplayRing({ keyframeMs: _keyframeIntervalMs });
}

/** rc.9 — test seam. Lets unit tests drive the encoder without a
 *  native module; pretends we received `frameJson` on the tick. */
export function __feedTickForTests(frameJson: string): void {
  if (!_running) {
    // Simulate "running" without an actual setInterval — caller drives.
    _running = true;
  }
  const snapshot = JSON.parse(frameJson) as NativeFrame;
  encodeAndPush(snapshot);
}
