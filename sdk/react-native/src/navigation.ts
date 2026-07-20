// Phase 35 sub-C: react-navigation auto-instrumentation.
// Phase 39 sub-B: the open `react.navigation` span is also made the
// active span for that screen's lifetime, so the screen's
// `http.client` spans (and any other startSpan() calls) attach to it
// as children — one trace per screen instead of one per request.
//
// Mount `useTraceNavigation(navigationRef)` next to your
// `<NavigationContainer ref={navigationRef}>`. Span names are
// `<from> → <to>` (or just the route name for the first screen) so
// the trace list reads as a navigation log.
//
// react-navigation is an OPTIONAL peer dependency — apps that don't
// use it never have to install it. The hook doesn't import from
// @react-navigation/native; consumers pass in the ref they already
// have, and we read its state via the public `getCurrentRoute()`.
//
// Caveat (active-span on RN is a module variable): requests fired
// from a `setTimeout` / background poll / detached promise after the
// screen settled may not see the nav span as active. If you want such
// a request parented to the current screen, pass it explicitly:
// `startSpan(op, { parent: activeSpan() })`.

import { useEffect, useRef } from 'react';

import { emitMetric, setActiveSpan, startSpan, type SpanHandle } from '@goliapkg/sentori-core';

import {
  getNativeFrameCounters,
  resetNativeFrameCounters,
} from './native';
import { track } from './track';
import { captureStep } from './trail';

/** Minimal contract: anything with `addListener('state', cb)` and
 *  `getCurrentRoute()` works. The real @react-navigation/native
 *  NavigationContainer ref matches this shape. */
export type NavigationRefLike = {
  addListener: (event: 'state', listener: () => void) => () => void;
  getCurrentRoute: () => { name: string } | undefined;
};

/** Process-global last-known route — populated by the navigation
 *  span path below, read by the analytics heartbeat so its per-tick
 *  payload includes the screen the user is on right now. Initial null
 *  when no nav has happened yet (app just launched, dev-launcher,
 *  splash). */
let _lastRoute: null | string = null;

export function getLastRoute(): null | string {
  return _lastRoute;
}

/**
 * Subscribe to react-navigation state changes and emit a
 * `react.navigation` span per screen (including the initial one),
 * each a fresh trace root. The span is kept active while the screen
 * is current; child spans created in that window attribute up to it.
 *
 *     import { NavigationContainer, useNavigationContainerRef } from '@react-navigation/native'
 *     import { useTraceNavigation } from '@goliapkg/sentori-react-native'
 *
 *     function App() {
 *       const navigationRef = useNavigationContainerRef()
 *       useTraceNavigation(navigationRef)
 *       return <NavigationContainer ref={navigationRef}>{...}</NavigationContainer>
 *     }
 *
 * Each span carries `{ nav.from, nav.to }` tags.
 */
export function useTraceNavigation(navigationRef: NavigationRefLike): void {
  // Latest route name we've observed.
  const lastRouteRef = useRef<null | string>(null);
  // Wall-clock ms when the last screen was entered. Drives dwell time
  // attached to the leaving span + the next captureStep breadcrumb.
  const lastRouteEnteredAtRef = useRef<null | number>(null);
  // Span for the screen the user is currently on. Finished when the
  // next screen is entered (or on unmount).
  const openSpanRef = useRef<null | SpanHandle>(null);

  useEffect(() => {
    if (typeof navigationRef.addListener !== 'function') return;
    if (typeof navigationRef.getCurrentRoute !== 'function') return;

    // Each screen gets its own trace root — detach from whatever the
    // previous screen's span was (we keep it active, so without
    // `parent: null` the new one would nest under it).
    const openScreenSpan = (
      from: null | string,
      to: string,
      prevDwellMs: null | number,
    ) => {
      const span = startSpan('react.navigation', {
        name: from ? `${from} → ${to}` : to,
        parent: null,
        tags: { 'nav.from': from ?? '', 'nav.to': to },
      });
      openSpanRef.current = span;
      setActiveSpan(span);
      lastRouteRef.current = to;
      _lastRoute = to;
      lastRouteEnteredAtRef.current = Date.now();
      // v0.8.0-b — dwell on the previous screen surfaces in the
      // session trail. The leaving span's `durationMs` already
      // carries the same number, but the trail is the most-glanced
      // surface so we duplicate it as breadcrumb data. No bytes wasted:
      // a breadcrumb without sessionTrail enabled is a no-op.
      captureStep(`screen:${to}`, {
        breadcrumb: {
          data: prevDwellMs !== null ? { dwellMsPrev: prevDwellMs } : undefined,
          message: from ? `${from} → ${to}` : to,
          type: 'navigation',
        },
      });
      // v1.1 chunk B — auto-pageview. Pushed into the track ring
      // alongside the navigation span so the analytics path sees
      // every screen entry without app code needing to wire its own
      // pageview calls. `from`/`dwellMsPrev` ride in props so the
      // Behavior view (chunk D) can reconstruct the user journey
      // even when only the track stream is queried.
      track(
        '$pageview',
        prevDwellMs !== null
          ? { from: from ?? null, dwellMsPrev: prevDwellMs }
          : { from: from ?? null },
        to,
      );
    };

    const finishOpenSpanWithDwell = (): null | number => {
      const span = openSpanRef.current;
      const enteredAt = lastRouteEnteredAtRef.current;
      if (!span) return null;
      const dwellMs = enteredAt !== null ? Math.max(0, Date.now() - enteredAt) : null;
      // v0.9.4 #1 — drain native frame counters at screen-leave.
      // Empty/null when native module not linked; tags omitted.
      const fc = getNativeFrameCounters();
      const finishTags: Record<string, string> = {};
      if (dwellMs !== null) finishTags['nav.dwell_ms'] = String(dwellMs);
      if (fc) {
        finishTags['vital.slow_frames'] = String(fc.slow);
        finishTags['vital.frozen_frames'] = String(fc.frozen);
      }
      span.finish({
        status: 'ok',
        tags: Object.keys(finishTags).length > 0 ? finishTags : undefined,
      });
      // Reset counters for the next screen.
      resetNativeFrameCounters();
      return dwellMs;
    };

    // Open a span for the initial screen so its requests are grouped
    // too (auth / config / first data load are usually the busiest
    // screen of a session).
    const initial = navigationRef.getCurrentRoute()?.name ?? null;
    if (initial !== null) openScreenSpan(null, initial, null);
    else lastRouteRef.current = null;

    const unsubscribe = navigationRef.addListener('state', () => {
      const next = navigationRef.getCurrentRoute()?.name ?? null;
      const prev = lastRouteRef.current;
      if (next === null || next === prev) return;
      const dwellMs = finishOpenSpanWithDwell();
      // v2.1 W2 — emit dwell as a runtime metric so the BI panel
      // can rank screens by how long users sat on them. Tagged
      // with `from` + `to` so a release-over-release diff can spot
      // a 2x dwell regression on Onboarding etc. emit is cheap +
      // bounded by the core ring cap; if `capture.runtimeMetrics:
      // false` the flush is off but the ring still records up to
      // the cap before dropping oldest.
      if (typeof dwellMs === 'number' && dwellMs >= 0) {
        emitMetric('runtime.route_nav_ms', dwellMs, {
          from: prev ?? '',
          to: next ?? '',
        });
      }
      openScreenSpan(prev, next, dwellMs);
    });

    return () => {
      unsubscribe();
      finishOpenSpanWithDwell();
      openSpanRef.current = null;
      lastRouteEnteredAtRef.current = null;
      setActiveSpan(null);
    };
  }, [navigationRef]);
}
