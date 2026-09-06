// Staged launch measurement (insight QIP-8 #2).
//
// One scalar closing at JS init() cannot represent user-perceived
// launch: apps that defer init() off the critical path get a number
// that is neither time-to-first-frame nor time-to-usable, and
// everything after init (bootstrap, splash-hide, first data) is
// invisible. This is the staged shape instead:
//
//   native anchors (process start → JS ready, from mobile vitals)
//   + sentori.launch.mark(name)   — app-defined waypoints
//   + sentori.launch.complete()   — the app's own "usable now"
//
// complete() emits ONE `app.launch` trace event with segment
// durations — it rides the existing event channel (batched, quiet
// network posture untouched) and aggregates like any trace. Apps
// that never call the marks keep today's behaviour exactly: no
// marks, no event.
//
// Same iron-rule surface as every verb: synchronous, O(1), never
// throws (safeFn), init-failure degrades to no-op.

import { safeFn } from '@goliapkg/sentori-core';

import { getColdStartMs, isColdStartPrewarmed } from './mobile-vitals';
import { trace } from './verbs';

let _armedAt: null | number = null;
let _marks: { name: string; at: number }[] = [];
let _completed = false;

/** Called from init(): anchors the JS side of the staged timeline. */
export function armLaunch(): void {
  if (_armedAt === null) _armedAt = Date.now();
}

const mark = safeFn('launch.mark', (name: string): void => {
  if (_completed || typeof name !== 'string' || name.length === 0) return;
  if (_marks.length >= 32) return; // bounded, like every buffer
  _marks.push({ at: Date.now(), name });
});

const complete = safeFn('launch.complete', (): void => {
  if (_completed || _armedAt === null) return;
  _completed = true;
  const completeAt = Date.now();
  const coldMs = getColdStartMs();
  const prewarmed = isColdStartPrewarmed();

  // Segment durations, in narrative order: the native span first
  // (process start → JS ready), then each waypoint since init, then
  // the tail up to complete().
  const segments: { ms: number; name: string }[] = [];
  if (coldMs !== null) segments.push({ ms: Math.round(coldMs), name: 'native' });
  let prev = _armedAt;
  for (const m of _marks.sort((a, b) => a.at - b.at)) {
    segments.push({ ms: Math.max(0, m.at - prev), name: m.name });
    prev = m.at;
  }
  segments.push({ ms: Math.max(0, completeAt - prev), name: 'complete' });

  const jsMs = completeAt - _armedAt;
  const totalMs = Math.round((coldMs ?? 0) + jsMs);
  trace('app.launch', {
    totalMs,
    ...(coldMs !== null ? { coldStartMs: Math.round(coldMs) } : { jsOnly: true }),
    ...(prewarmed ? { prewarmed: true } : {}),
    segments,
  });
});

/** The staged-launch surface: `sentori.launch.mark/complete`. */
export const launch = { complete, mark };

/** Test-only. */
export function __resetLaunchForTests(): void {
  _armedAt = null;
  _marks = [];
  _completed = false;
}
