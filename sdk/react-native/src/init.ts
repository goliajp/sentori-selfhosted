// sentori.init(config) — the single configuration entry point
// (design.md §4). Synchronous, never throws; a bad config degrades
// every verb to a no-op with one console.warn, never a crash
// (failure-isolation iron rule).

import { safeFn, setLogLevel } from '@goliapkg/sentori-core';
import type { InitConfig } from '@goliapkg/sentori-core';

import { setConfig } from './config';
import { registerEmitHook } from './emit-hooks';
import { installGlobalHandler } from './handlers/global';
import { installLifecycleHandler } from './handlers/lifecycle';
import { installNetworkHandler } from './handlers/network';
import { installPromiseHandler } from './handlers/promise';
import { startLongTaskMonitor } from './long-task-monitor';
import { checkColdStart } from './mobile-vitals';
import { armLaunch } from './launch';
import { markNativeJsBridgeReady, setNativeConfig } from './native';
import { shipNativePending } from './native-pending';
import { drainReplay, startReplay } from './replay';
import {
  drainScreenReplay,
  screenReplayArmed,
  screenReplayCaptured,
  startScreenReplay,
} from './replay-screens';
import { drainOfflineQueue, queueAttachment, startTransport } from './transport';

let _initialized = false;

export const init = safeFn('init', (config: InitConfig): void => {
  if (_initialized) return;

  if (!config || typeof config.token !== 'string' || config.token.length === 0) {
    // eslint-disable-next-line no-console
    console.warn('[sentori] init skipped: token missing — SDK is a no-op');
    return;
  }
  if (typeof config.ingestUrl !== 'string' || config.ingestUrl.length === 0) {
    // eslint-disable-next-line no-console
    console.warn('[sentori] init skipped: ingestUrl missing — SDK is a no-op');
    return;
  }

  _initialized = true;

  setConfig({
    token: config.token,
    ingestUrl: config.ingestUrl.replace(/\/+$/, ''),
    release: config.release ?? '',
    environment: config.environment ?? 'production',
    enabled: true,
    detect: {
      rageTap: config.detect?.rageTap ?? true,
      longFreeze: config.detect?.longFreeze ?? true,
      slowColdStart: config.detect?.slowColdStart ?? true,
      slowApi: config.detect?.slowApi ?? false,
    },
    replaySeconds: config.replaySeconds ?? 30,
    replayScreens: config.replayScreens ?? false,
    backendHealthUrl: config.backendHealthUrl,
    beforeSend: config.beforeSend,
  });
  setLogLevel(config.logLevel ?? 'warn');

  // JS-side error capture + the signal-ring feeders.
  installGlobalHandler();
  installPromiseHandler();
  installNetworkHandler();
  installLifecycleHandler();

  // Warn-scenario detectors (design.md §3 minimum set). rage_tap
  // rides the RageTapCapture component; the rest start here.
  if (config.detect?.longFreeze !== false) startLongTaskMonitor();

  // B-type replay: a rolling in-memory wireframe ring; an error/warn
  // going out drains it into a replay attachment on that event.
  const replaySeconds = config.replaySeconds ?? 30;
  if (replaySeconds > 0) {
    startReplay({ mode: 'wireframe' });
    // Visual ring is opt-in: screenshots can carry user content.
    if (config.replayScreens === true) startScreenReplay(replaySeconds);
    registerEmitHook((event) => {
      if (event.kind !== 'error' && event.kind !== 'warn') return;
      if (!event.id) return;
      const lines = drainReplay();
      if (lines) {
        queueAttachment(event.id, 'replay', {
          text: lines,
          mediaType: 'application/x-sentori-replay',
        });
      }
      const frames = drainScreenReplay();
      if (frames) {
        queueAttachment(event.id, 'screens', {
          text: frames,
          mediaType: 'application/x-sentori-screens',
        });
      } else if (screenReplayArmed()) {
        // Asked for, produced nothing. Without this the dashboard
        // cannot tell that from "never enabled", and it was telling
        // readers to switch on a setting that was already on. A
        // handful of bytes on an event already going out — no extra
        // request, and nothing at all when the feature is off.
        event.payload = {
          ...(event.payload ?? {}),
          replay: {
            screens: 'empty',
            captured: screenReplayCaptured(),
          },
        };
      }
    });
  }

  // Native side: hand over release/environment for the crash-file
  // writer, mark the bridge live, then drain crashes from previous
  // launches. All fire-and-forget — init stays synchronous and fast
  // (< 50 ms budget; the work below happens off the critical path).
  setNativeConfig({
    environment: config.environment ?? 'production',
    release: config.release ?? '',
    token: config.token,
  });
  markNativeJsBridgeReady();

  startTransport();
  checkColdStart(config.detect?.slowColdStart !== false);
  armLaunch();
  void shipNativePending().catch(() => undefined);
  void drainOfflineQueue();
});


export const __resetForTests = (): void => {
  _initialized = false;
};
