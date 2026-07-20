// v0.9.6 #4 — long-task monitor unit tests.

import { afterEach, describe, expect, test } from 'bun:test';

import { clearSpans, drainSpans } from '@goliapkg/sentori-core';

import {
  __resetLongTaskMonitorForTests,
  startLongTaskMonitor,
  stopLongTaskMonitor,
} from '../long-task-monitor';

afterEach(() => {
  __resetLongTaskMonitorForTests();
  clearSpans();
});

describe('long-task-monitor', () => {
  test('disabled is a no-op', () => {
    startLongTaskMonitor({ enabled: false });
    expect(drainSpans().length).toBe(0);
  });

  test('double-start is idempotent', () => {
    startLongTaskMonitor({ enabled: true });
    // Second call must not register a parallel interval — would
    // double-count every tick.
    startLongTaskMonitor({ enabled: true });
    stopLongTaskMonitor();
  });

  test('synthetic long task crosses threshold and emits a span', async () => {
    // 80ms threshold so we don't need a 200ms+ block in the test.
    startLongTaskMonitor({ enabled: true, thresholdMs: 80 });
    // Busy-block the JS thread for ~150ms; the next tick will see
    // elapsed >> tickInterval and emit a longtask span.
    const t0 = Date.now();
    while (Date.now() - t0 < 150) {
      // burn — no setTimeout, the thread must actually be busy.
    }
    // Let the next 50ms tick fire.
    await new Promise((r) => setTimeout(r, 80));
    const spans = drainSpans();
    const longtask = spans.find((s) => s.op === 'sentori.longtask');
    expect(longtask).toBeDefined();
    expect(longtask?.tags?.['profile.kind']).toBe('longtask');
    stopLongTaskMonitor();
  });

  test('sub-threshold ticks do not emit', async () => {
    startLongTaskMonitor({ enabled: true, thresholdMs: 500 });
    // Don't block — let normal idle ticks happen.
    await new Promise((r) => setTimeout(r, 200));
    const spans = drainSpans();
    const longtasks = spans.filter((s) => s.op === 'sentori.longtask');
    expect(longtasks.length).toBe(0);
    stopLongTaskMonitor();
  });
});
