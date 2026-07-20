import {
  __resetTraceContextForTests,
  __useFallbackTraceContextForTests,
  activeSpan,
  clearSpans,
  drainSpans,
  setActiveSpan,
  startSpan,
} from '@goliapkg/sentori-core';
import { afterAll, afterEach, beforeEach, describe, expect, test } from 'bun:test';

import { type NavigationRefLike, useTraceNavigation } from '../navigation';

// We test the hook without a React renderer (sdk/react-native has no
// react-test-renderer / @testing-library dev-dep). `harness` mirrors
// the hook's effect body 1:1 — when production changes, this changes
// too. The point is to verify the observable contract: spans pushed
// in the right order + the open screen span left active.

class FakeNav implements NavigationRefLike {
  private listeners: Array<() => void> = [];
  private route: { name: string } | undefined;

  setInitialRoute(name: string): void {
    this.route = { name };
  }
  addListener(_event: 'state', listener: () => void): () => void {
    this.listeners.push(listener);
    return () => {
      this.listeners = this.listeners.filter((l) => l !== listener);
    };
  }
  getCurrentRoute(): { name: string } | undefined {
    return this.route;
  }
  go(name: string): void {
    this.route = { name };
    this.listeners.forEach((l) => l());
  }
}

function harness(navigationRef: NavigationRefLike): () => void {
  let lastRoute: null | string = null;
  let openSpan: ReturnType<typeof startSpan> | null = null;

  const openScreenSpan = (from: null | string, to: string) => {
    const span = startSpan('react.navigation', {
      name: from ? `${from} → ${to}` : to,
      parent: null,
      tags: { 'nav.from': from ?? '', 'nav.to': to },
    });
    openSpan = span;
    setActiveSpan(span);
    lastRoute = to;
  };

  const initial = navigationRef.getCurrentRoute()?.name ?? null;
  if (initial !== null) openScreenSpan(null, initial);
  else lastRoute = null;

  const unsub = navigationRef.addListener('state', () => {
    const next = navigationRef.getCurrentRoute()?.name ?? null;
    if (next === null || next === lastRoute) return;
    openSpan?.finish({ status: 'ok' });
    openScreenSpan(lastRoute, next);
  });

  return () => {
    unsub();
    openSpan?.finish({ status: 'ok' });
    openSpan = null;
    setActiveSpan(null);
  };
}

beforeEach(() => {
  // Navigation relies on setActiveSpan, which is a no-op on the
  // ALS impl that bun (Node) picks. In production this runs on RN,
  // where the fallback impl is in effect — exercise that one.
  __useFallbackTraceContextForTests();
  clearSpans();
});
afterEach(() => {
  setActiveSpan(null);
  clearSpans();
});
afterAll(() => {
  __resetTraceContextForTests();
});

describe('useTraceNavigation', () => {
  test('exports a hook', () => {
    expect(typeof useTraceNavigation).toBe('function');
  });

  test('initial mount opens a span for the first screen and leaves it active', () => {
    const nav = new FakeNav();
    nav.setInitialRoute('Home');
    const cleanup = harness(nav);

    // span not finished yet (still on Home) → buffer empty
    expect(drainSpans()).toHaveLength(0);
    expect(activeSpan()).not.toBeNull();

    cleanup();
    const spans = drainSpans();
    expect(spans).toHaveLength(1);
    expect(spans[0]?.op).toBe('react.navigation');
    expect(spans[0]?.name).toBe('Home');
    expect(spans[0]?.tags).toEqual({ 'nav.from': '', 'nav.to': 'Home' });
  });

  test('mount with no current route → no span, no active', () => {
    const nav = new FakeNav(); // no initial route
    const cleanup = harness(nav);
    expect(activeSpan()).toBeNull();
    cleanup();
    expect(drainSpans()).toHaveLength(0);
  });

  test('http.client span during a screen becomes a child of the nav span', () => {
    const nav = new FakeNav();
    nav.setInitialRoute('Home');
    const cleanup = harness(nav);
    const navSpan = activeSpan()!;

    // simulate the network handler opening a request span (no explicit
    // parent → inherits the active nav span)
    const req = startSpan('http.client', { name: 'GET /x' });
    expect(req.parentSpanId).toBe(navSpan.spanId);
    expect(req.traceId).toBe(navSpan.traceId);
    req.finish({ status: 'ok' });

    cleanup();
    const spans = drainSpans();
    const navTrace = spans.find((s) => s.op === 'react.navigation')!.traceId;
    expect(spans.every((s) => s.traceId === navTrace)).toBe(true);
  });

  test('each screen is its own trace root (transitions do not nest)', () => {
    const nav = new FakeNav();
    nav.setInitialRoute('A');
    const cleanup = harness(nav);
    nav.go('B');
    nav.go('C');
    cleanup();

    const spans = drainSpans().filter((s) => s.op === 'react.navigation');
    expect(spans).toHaveLength(3);
    expect(spans.map((s) => s.name)).toEqual(['A', 'A → B', 'B → C']);
    expect(spans.every((s) => s.parentSpanId === null)).toBe(true);
    expect(new Set(spans.map((s) => s.traceId)).size).toBe(3);
  });

  test('same-route state event does not open a new span', () => {
    const nav = new FakeNav();
    nav.setInitialRoute('Home');
    const cleanup = harness(nav);
    nav.go('Home');
    cleanup();
    expect(drainSpans().filter((s) => s.op === 'react.navigation')).toHaveLength(1);
  });

  test('cleanup finishes the open span and clears the active span', () => {
    const nav = new FakeNav();
    nav.setInitialRoute('Home');
    const cleanup = harness(nav);
    nav.go('Settings');
    cleanup();

    expect(activeSpan()).toBeNull();
    const spans = drainSpans().filter((s) => s.op === 'react.navigation');
    expect(spans.map((s) => s.name)).toEqual(['Home', 'Home → Settings']);
  });
});
