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

import { logger, startSpan } from '@goliapkg/sentori-core';

import { getRegisteredMaskQuery } from './mask';
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

/** When the delta against the previous frame would carry ≥ this
 *  fraction of the current node count, prefer a fresh keyframe —
 *  emits roughly the same bytes but doesn't grow the chain. */
const DELTA_TO_KEYFRAME_RATIO = 0.4;

/** Floor under which the keyframe-vs-delta ratio heuristic does
 *  not apply. Trivial UIs (≤ 10 nodes — boot splash, dev panel,
 *  tests) shouldn't drop to keyframes on every change. */
const KEYFRAME_RATIO_MIN_NODES = 10;

/** Replay window kept in the ring buffer. captureException drains. */
const REPLAY_WINDOW_MS = 60_000;

/** Hard ceiling on ring item count — defence against a wedged tick
 *  clock filling memory; under normal capture rates we evict by time
 *  long before this fires. */
const MAX_RING_ITEMS = 1000;

/** Floor on tick period. < 100 ms the native view-tree walk dominates
 *  the JS thread on mid-tier Android. */
const MIN_TICK_PERIOD_MS = 100;

type Node = {
  x: number;
  y: number;
  w: number;
  h: number;
  kind?: string;
  text?: string;
  color?: string;
};

type NativeFrame = { ts: number; width: number; height: number; nodes: Node[] };

type RingItem = { ts: number; line: string };

let _ring: RingItem[] = [];
let _timer: ReturnType<typeof setInterval> | null = null;
let _running = false;

/** Last emit's reconstructed state — fingerprint → node. Null until
 *  the first keyframe lands; reset on drain so the next session
 *  starts with a fresh keyframe. */
let _lastFrameState: Map<string, Node> | null = null;
let _lastKeyframeTs = 0;

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
  let tickSpan: ReturnType<typeof startSpan> | null = null;
  try {
    tickSpan = startSpan('sentori.replay.tick', { name: 'tick' });
  } catch {
    // never fatal
  }
  try {
    const maskIds = readMaskIds();
    const snapshotJson = _nativeMod?.captureWireframe?.(maskIds);
    if (typeof snapshotJson !== 'string' || snapshotJson.length === 0) {
      handleEmptyTick(snapshotJson);
      tickSpan?.finish({ status: 'ok' });
      return;
    }

    let snapshot: NativeFrame;
    try {
      snapshot = JSON.parse(snapshotJson) as NativeFrame;
    } catch (e) {
      logger.warn('replay', 'tick: native JSON parse failed', e);
      tickSpan?.finish({ status: 'error' });
      return;
    }

    _emptyTickCount = 0;
    _emptyTickLogStride = 1;

    encodeAndPush(snapshot);

    if (typeof __DEV__ !== 'undefined' && __DEV__) {
      diagnosticForTick(snapshot, snapshotJson.length);
    }
    tickSpan?.finish({ status: 'ok' });
  } catch (e) {
    if (e instanceof Error) tickSpan?.setTag('error.message', e.message);
    tickSpan?.finish({ status: 'error' });
    logger.warn('replay', 'tick threw', e);
  }
}

function encodeAndPush(snapshot: NativeFrame): void {
  const currentState = new Map<string, Node>();
  for (const n of snapshot.nodes) currentState.set(fingerprint(n), n);

  const ts = snapshot.ts;
  const isCold = _lastFrameState === null;
  const keyframeOverdue = ts - _lastKeyframeTs >= _keyframeIntervalMs;

  let line: string;

  if (isCold || keyframeOverdue) {
    line = encodeKeyframe(snapshot);
    _lastKeyframeTs = ts;
  } else {
    const delta = computeDelta(_lastFrameState as Map<string, Node>, currentState);
    const totalChanged = delta.added.length + delta.changed.length + delta.removed.length;
    if (totalChanged === 0) {
      // No-op heartbeat — drop. Keep _lastFrameState as-is (identical).
      return;
    }
    if (
      currentState.size >= KEYFRAME_RATIO_MIN_NODES &&
      totalChanged >= currentState.size * DELTA_TO_KEYFRAME_RATIO
    ) {
      // Big screen transition on a substantial UI — emit a fresh
      // keyframe so reconstruction doesn't carry a near-rewrite delta.
      line = encodeKeyframe(snapshot);
      _lastKeyframeTs = ts;
    } else {
      line = JSON.stringify({
        ts,
        kind: 'delta',
        added: delta.added,
        changed: delta.changed,
        removed: delta.removed,
      });
    }
  }

  _ring.push({ ts, line });
  evictRing(ts);
  _lastFrameState = currentState;
}

function encodeKeyframe(snapshot: NativeFrame): string {
  return JSON.stringify({
    ts: snapshot.ts,
    kind: 'key',
    width: snapshot.width,
    height: snapshot.height,
    nodes: snapshot.nodes,
  });
}

function evictRing(nowTs: number): void {
  const cutoff = nowTs - REPLAY_WINDOW_MS;
  while (_ring.length > 0 && _ring[0]!.ts < cutoff) _ring.shift();
  while (_ring.length > MAX_RING_ITEMS) _ring.shift();
}

/** Fingerprint integer-rounds before joining so sub-pixel jitter from
 *  RN's Fabric layout (occasionally floats) doesn't break stable
 *  matching across ticks. */
function fingerprint(n: Node): string {
  return `${n.x | 0},${n.y | 0},${n.w | 0},${n.h | 0}`;
}

type Delta = { added: Node[]; changed: Node[]; removed: Pick<Node, 'x' | 'y' | 'w' | 'h'>[] };

export function computeDelta(prev: Map<string, Node>, curr: Map<string, Node>): Delta {
  const added: Node[] = [];
  const changed: Node[] = [];
  const removed: Pick<Node, 'x' | 'y' | 'w' | 'h'>[] = [];
  for (const [fp, node] of curr) {
    const p = prev.get(fp);
    if (!p) {
      added.push(node);
      continue;
    }
    if (
      (p.kind ?? '') !== (node.kind ?? '') ||
      (p.color ?? '') !== (node.color ?? '') ||
      (p.text ?? '') !== (node.text ?? '')
    ) {
      changed.push(node);
    }
  }
  for (const [fp, node] of prev) {
    if (!curr.has(fp)) removed.push({ x: node.x, y: node.y, w: node.w, h: node.h });
  }
  return { added, changed, removed };
}

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
  const q = getRegisteredMaskQuery();
  if (!q) return [];
  try {
    return q();
  } catch {
    return [];
  }
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
  if (_ring.length === 0) return '';
  const out = _ring.map((r) => r.line).join('\n');
  _ring = [];
  _lastFrameState = null;
  _lastKeyframeTs = 0;
  return out;
}

export function __resetReplayForTests(): void {
  stopReplay();
  _ring = [];
  _lastFrameState = null;
  _lastKeyframeTs = 0;
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
