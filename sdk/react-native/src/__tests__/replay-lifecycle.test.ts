import { afterEach, beforeAll, beforeEach, expect, spyOn, test } from 'bun:test';
import { clearSignals, snapshotSignals } from '@goliapkg/sentori-core';
import { __bindLifecycleForTests, __setLifecycleForTests as state, __uninstallLifecycleForTests } from '../handlers/lifecycle';
import { __resetLongTaskMonitorForTests, startLongTaskMonitor } from '../long-task-monitor';
import { __feedTickForTests, __resetReplayForTests, __tickWithNativeForTests, drainReplay, startReplay, stopReplay } from '../replay';
import { __resetForTests as resetScreens, __setCaptureForTests, __tickScreenReplayForTests as capture, drainScreenReplay, startScreenReplay } from '../replay-screens';
import { __resetForTests as resetConfig, setConfig } from '../config';
import { __setNativeForTests } from '../native';
import { __peekQueue, __resetForTests as resetTransport } from '../transport';

let wall = 1_800_000_000_000;
let mono = 1000;
let timerTick: () => void;
let restore: (() => void)[] = [];
// Other test files may already have called init. Seed that state explicitly so
// this fixture cannot silently depend on discovery order or a fresh process.
beforeAll(() => startLongTaskMonitor());
beforeEach(() => {
  __resetLongTaskMonitorForTests();
  __resetReplayForTests();
  resetScreens();
  resetTransport();
  resetConfig();
  __setNativeForTests(undefined);
  __uninstallLifecycleForTests(); clearSignals();
  wall = 1_800_000_000_000; mono = 1000;
  const wallSpy = spyOn(Date, 'now').mockImplementation(() => wall);
  const monoSpy = spyOn(performance, 'now').mockImplementation(() => mono);
  const timerSpy = spyOn(globalThis, 'setInterval').mockImplementation(((fn: () => void) => {
    timerTick = fn;
    return { unref() {} };
  }) as unknown as typeof setInterval);
  restore = [() => timerSpy.mockRestore(), () => wallSpy.mockRestore(), () => monoSpy.mockRestore()];
  setConfig({ token: 'st_test', ingestUrl: 'http://localhost:18080', release: 'test@1', environment: 'test', enabled: true, detect: { rageTap: false, longFreeze: true, slowColdStart: false, slowApi: false }, replaySeconds: 30 });
});
afterEach(() => {
  __setNativeForTests(undefined);
  __resetLongTaskMonitorForTests(); __resetReplayForTests(); resetScreens();
  __uninstallLifecycleForTests(); resetTransport(); resetConfig(); clearSignals();
  for (const fn of restore) fn();
});
function advance(ms: number) { mono += ms; wall += ms; }
function freezes() { return snapshotSignals().filter((signal) => signal.kind === 'freeze'); }
function frame(ts = wall, text = 'a') { return JSON.stringify({ ts, width: 100, height: 100, nodes: [{ x: 0, y: 0, w: 10, h: 10, text }] }); }
const pixels = { base64: 'pixels', mediaType: 'image/jpeg' };

test('11-hour background suspension never becomes a JS freeze on resume', () => {
  startLongTaskMonitor(); state('background'); advance(40_500_694); state('active'); timerTick();
  expect(freezes()).toEqual([]); expect(__peekQueue()).toHaveLength(0);
  advance(3050); timerTick();
  expect(freezes()[0]?.data).toEqual({ blockedMs: 3000 });
  expect(__peekQueue()[0]?.name).toBe('long_freeze');
});
test('starting inactive and ticking in background or immediately after resume is ignored', () => {
  state('inactive'); startLongTaskMonitor(); advance(5000); timerTick();
  state('active'); advance(5000); timerTick(); advance(50); timerTick();
  expect(freezes()).toEqual([]);
});
test('clock corrections do not manufacture or hide foreground freezes', () => {
  startLongTaskMonitor(); wall += 40_500_694; mono += 50; timerTick();
  wall -= 80_000_000; mono += 50; timerTick(); expect(freezes()).toEqual([]);
  advance(3050); timerTick(); expect(freezes()[0]?.data).toEqual({ blockedMs: 3000 });
});
test('foreground freeze warns retain the six-per-minute limit', () => {
  startLongTaskMonitor();
  for (let i = 0; i < 8; i++) { advance(3050); timerTick(); }
  expect(__peekQueue()).toHaveLength(6);
  advance(60_000); timerTick(); expect(__peekQueue()).toHaveLength(7);
});
test('wireframes expire even if capture never runs again', () => {
  __feedTickForTests(frame()); advance(40_500_694); expect(drainReplay()).toBe('');
});
test('wireframes discard orphan deltas after their base expires', () => {
  __feedTickForTests(frame(wall - 61_000)); __feedTickForTests(frame(wall - 59_000, 'b'));
  __feedTickForTests(frame(wall - 56_000, 'c'));
  const entries = drainReplay().split('\n').map((line) => JSON.parse(line));
  expect(entries).toHaveLength(1); expect(entries[0].kind).toBe('key'); expect(entries[0].nodes[0].text).toBe('c');
});
test('wireframe capture skips background and starts a new keyframe after resume', () => {
  let calls = 0;
  const native = { captureWireframe: () => { calls++; return frame(); } };
  __tickWithNativeForTests(native); state('background'); __tickWithNativeForTests(native);
  expect(calls).toBe(1); state('active'); advance(500); __tickWithNativeForTests(native);
  expect(drainReplay().split('\n').map((line) => JSON.parse(line).kind)).toEqual(['key']);
});
test('stop and wall clock rewind cannot leak old wireframes', () => {
  __feedTickForTests(frame()); stopReplay(); expect(drainReplay()).toBe('');
  __feedTickForTests(frame()); wall -= 5000; __feedTickForTests(frame(wall, 'new'));
  expect(JSON.parse(drainReplay()).nodes[0].text).toBe('new');
});
test('pixels expire by age, not just number of captures', async () => {
  __setCaptureForTests(async () => pixels); startScreenReplay(30); await capture();
  advance(30_001); expect(drainScreenReplay()).toBeNull();
});
test('pixels pending across a background/resume transition are discarded', async () => {
  let finish!: (value: typeof pixels) => void;
  __setCaptureForTests(() => new Promise((resolve) => { finish = resolve; }));
  startScreenReplay(30); const pending = capture(); state('background'); state('active');
  finish(pixels); await pending; expect(drainScreenReplay()).toBeNull();
});
test('a capture from a reset session cannot populate a restarted ring', async () => {
  let finish!: (value: typeof pixels) => void;
  __setCaptureForTests(() => new Promise((resolve) => { finish = resolve; }));
  startScreenReplay(30); const pending = capture(); resetScreens(); startScreenReplay(30);
  finish(pixels); await pending; expect(drainScreenReplay()).toBeNull();
});
test('rejected capture is isolated and the next capture still succeeds', async () => {
  __setCaptureForTests(async () => { throw new Error('native failure'); });
  startScreenReplay(30); await capture(); __setCaptureForTests(async () => pixels); await capture();
  expect(drainScreenReplay()).not.toBeNull();
});
test('slow capture uses its start time and cannot publish expired pixels', async () => {
  __setCaptureForTests(async () => { advance(40_000); return pixels; });
  startScreenReplay(30); await capture(); expect(drainScreenReplay()).toBeNull();
});

test('initial background and native state ahead of the queued callback are respected', () => {
  const appState = { currentState: 'background', addEventListener: () => ({ remove() {} }) };
  __bindLifecycleForTests(appState);
  startLongTaskMonitor(); advance(5000); timerTick();
  appState.currentState = 'active'; advance(5000); timerTick();
  advance(50); timerTick();
  appState.currentState = 'background'; advance(5000); timerTick();
  expect(freezes()).toEqual([]);
});
test('unknown initial native state waits for foreground; a broken listener is isolated', () => {
  const appState = { currentState: null as string | null, addEventListener: () => { throw new Error('bridge missing'); } };
  expect(() => __bindLifecycleForTests(appState)).not.toThrow();
  startLongTaskMonitor(); advance(5000); timerTick(); expect(freezes()).toEqual([]);
  appState.currentState = 'active'; advance(5000); timerTick();
  advance(3050); timerTick(); expect(freezes()[0]?.data).toEqual({ blockedMs: 3000 });
});
test('wireframe retention honors the configured window', () => {
  __setNativeForTests({ captureWireframe: () => null });
  startReplay({ mode: 'wireframe', windowSeconds: 5 });
  __feedTickForTests(frame()); advance(5001);
  expect(drainReplay()).toBe('');
});
