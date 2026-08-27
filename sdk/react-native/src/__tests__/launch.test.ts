// Staged launch measurement (QIP-8 #2): marks + complete emit ONE
// app.launch trace with segment durations; never before arm, never
// twice, never a throw.

import { afterEach, beforeEach, describe, expect, it } from 'bun:test';

import { clearSignals } from '@goliapkg/sentori-core';

import { __resetForTests as resetConfig, setConfig } from '../config';
import { __resetLaunchForTests, armLaunch, launch } from '../launch';
import { __resetMobileVitalsForTests } from '../mobile-vitals';
import { __resetForTests as resetScope } from '../scope';
import {
  __peekQueue,
  __resetForTests as resetTransport,
  startTransport,
} from '../transport';

const baseConfig = {
  token: 'st_test',
  ingestUrl: 'http://localhost:18080',
  release: 'app@1.0.0',
  environment: 'test',
  enabled: true,
  detect: { rageTap: true, longFreeze: true, slowColdStart: true, slowApi: false },
  replaySeconds: 30,
};

describe('launch marks', () => {
  beforeEach(() => {
    resetTransport();
    resetConfig();
    resetScope();
    clearSignals();
    __resetLaunchForTests();
    __resetMobileVitalsForTests();
    setConfig(baseConfig);
    startTransport();
  });
  afterEach(() => {
    resetTransport();
    resetConfig();
    resetScope();
    clearSignals();
    __resetLaunchForTests();
    __resetMobileVitalsForTests();
  });

  const launchEvents = () =>
    __peekQueue().filter((e) => e.kind === 'trace' && e.name === 'app.launch');

  it('complete() before arm is a no-op (init never ran)', () => {
    launch.mark('bootstrap');
    launch.complete();
    expect(launchEvents()).toHaveLength(0);
  });

  it('arm → marks → complete emits one app.launch trace with ordered segments', () => {
    armLaunch();
    launch.mark('bootstrap');
    launch.mark('first-tree');
    launch.complete();
    const evs = launchEvents();
    expect(evs).toHaveLength(1);
    const data = (evs[0]!.payload as { data?: Record<string, unknown> })?.data ?? {};
    expect(typeof data.totalMs).toBe('number');
    const segs = data.segments as { ms: number; name: string }[];
    // no native module in tests → jsOnly, segments are the marks + tail
    expect(data.jsOnly).toBe(true);
    expect(segs.map((s) => s.name)).toEqual(['bootstrap', 'first-tree', 'complete']);
    for (const s of segs) expect(s.ms).toBeGreaterThanOrEqual(0);
  });

  it('complete() is once — the second call emits nothing', () => {
    armLaunch();
    launch.complete();
    launch.mark('late'); // ignored after complete
    launch.complete();
    expect(launchEvents()).toHaveLength(1);
  });

  it('never throws on garbage input', () => {
    armLaunch();
    expect(() => {
      launch.mark(undefined as unknown as string);
      launch.mark('');
      launch.complete();
    }).not.toThrow();
  });
});
