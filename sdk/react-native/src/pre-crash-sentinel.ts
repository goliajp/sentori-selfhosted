// v0.9.1 +S4 — pre-crash sentinel.
//
// Predictive (vs reactive) telemetry. Subscribes to JS-thread frame
// timing via requestAnimationFrame and, when a rolling 60-frame
// window has ≥ 50% of frames slower than 32 ms (i.e. < 30 fps for
// half the window), emits an `event.kind = nearCrash` to the server.
// Backend stores it in the same events stream so the dashboard shows
// "X user sessions had near-crash signals 4 minutes before the
// actual NSException".
//
// Memory pressure / OOM / storage low signals will need native
// system observers and ship in v1.0. v0.9.1 covers the most common
// runaway-render-loop case purely from JS.

import { startSpan } from '@goliapkg/sentori-core';

import { getBundleInfo } from './bundle-info';
import { collectDeviceForSentinel, getAppForSentinel } from './sentinel-context';
import { getConfig, isInitialized } from './config';
import { enqueue } from './transport';
import { uuidV7 } from './uuid';
import type { Event } from './types';

const FRAME_BUDGET_MS = 32; // < 30 fps
const WINDOW_FRAMES = 60; // ~1 s at 60 fps
const TRIP_RATIO = 0.5;
const COOLDOWN_MS = 60_000; // don't spam: one nearCrash event per minute

let _running = false;
let _lastFrameAt = 0;
let _slowFrames = 0;
let _totalFrames = 0;
let _lastEmitAt = 0;
let _channels: Set<string> = new Set();

export type PreCrashChannel =
  | 'frame-budget-overrun'
  | 'memory-pressure'  // native, v1.0
  | 'oom-warning'      // native, v1.0
  | 'storage-low';     // native, v1.0

export type PreCrashSentinelOptions = {
  enabled: boolean;
  channels?: PreCrashChannel[];
  /** Lower → more sensitive. Default 32 ms (< 30 fps). */
  frameBudgetMs?: number;
  /** Fraction of frames in the window that must miss budget. Default 0.5. */
  tripRatio?: number;
};

export function startPreCrashSentinel(opts: PreCrashSentinelOptions): void {
  if (!opts.enabled || _running) return;
  _running = true;
  _channels = new Set(opts.channels ?? ['frame-budget-overrun']);

  if (_channels.has('frame-budget-overrun')) {
    startFrameBudgetWatch(opts.frameBudgetMs ?? FRAME_BUDGET_MS, opts.tripRatio ?? TRIP_RATIO);
  }
  // Native channels (memory-pressure, oom-warning, storage-low) hook
  // through a TODO native module in v1.0.
}

export function stopPreCrashSentinel(): void {
  _running = false;
  _slowFrames = 0;
  _totalFrames = 0;
  _channels.clear();
}

function startFrameBudgetWatch(budgetMs: number, tripRatio: number): void {
  if (typeof requestAnimationFrame !== 'function') return;
  _lastFrameAt = Date.now();
  function tick() {
    if (!_running) return;
    const now = Date.now();
    const delta = now - _lastFrameAt;
    _lastFrameAt = now;
    if (delta >= budgetMs) _slowFrames++;
    _totalFrames++;
    if (_totalFrames >= WINDOW_FRAMES) {
      const ratio = _slowFrames / _totalFrames;
      if (ratio >= tripRatio && now - _lastEmitAt > COOLDOWN_MS) {
        _lastEmitAt = now;
        emitNearCrash({
          slowFrames: _slowFrames,
          totalFrames: _totalFrames,
          ratio,
          windowMs: WINDOW_FRAMES * (budgetMs / 2), // approximate
          channel: 'frame-budget-overrun',
        });
      }
      _slowFrames = 0;
      _totalFrames = 0;
    }
    requestAnimationFrame(tick);
  }
  requestAnimationFrame(tick);
}

function emitNearCrash(data: {
  slowFrames: number;
  totalFrames: number;
  ratio: number;
  windowMs: number;
  channel: PreCrashChannel;
}): void {
  if (!isInitialized()) return;
  const config = getConfig();
  if (!config) return;
  const span = startSpan('sentori.nearCrash', {
    name: data.channel,
    tags: {
      'nearCrash.channel': data.channel,
      'nearCrash.ratio': data.ratio.toFixed(3),
      'nearCrash.slow_frames': String(data.slowFrames),
      'nearCrash.total_frames': String(data.totalFrames),
    },
  });
  span.finish({ status: 'ok' });
  const event: Event = {
    id: uuidV7(),
    timestamp: new Date().toISOString(),
    kind: 'nearCrash',
    platform: 'javascript',
    release: config.release,
    environment: config.environment,
    device: collectDeviceForSentinel(),
    app: getAppForSentinel(config.release),
    ...(getBundleInfo() ? { bundle: getBundleInfo() as { id: string } } : {}),
    error: {
      type: 'NearCrash',
      message: `frame budget overrun: ${(data.ratio * 100).toFixed(0)}% of last ${data.totalFrames} frames slow`,
      stack: [],
    },
    tags: {
      'nearCrash.channel': data.channel,
    },
  };
  enqueue(event);
}
