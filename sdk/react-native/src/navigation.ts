// react-navigation auto-instrumentation — v1 role: feed the signal
// ring (`nav` signals) and remember the current screen so warn
// surfaces can name where they happened. The span/trace/metric
// machinery this file used to drive is gone with the APM vocabulary.
//
// Mount `useTraceNavigation(navigationRef)` next to your
// `<NavigationContainer ref={navigationRef}>`. react-navigation
// stays an OPTIONAL peer dependency: the hook never imports from
// @react-navigation/native — consumers pass the ref they already
// have and we read `getCurrentRoute()`.

import { useEffect, useRef } from 'react';

import { pushSignal } from '@goliapkg/sentori-core';

/** Minimal contract: anything with `addListener('state', cb)` and
 *  `getCurrentRoute()` works. The real @react-navigation/native
 *  NavigationContainer ref matches this shape. */
export type NavigationRefLike = {
  addListener: (event: 'state', listener: () => void) => () => void;
  getCurrentRoute: () => { name: string } | undefined;
};

/** Process-global last-known route — surfaces on detected warns
 *  (`surface.screen`) and rides nav signals. Null before the first
 *  navigation (splash, dev launcher). */
let _lastRoute: null | string = null;

export function currentScreen(): string | undefined {
  return _lastRoute ?? undefined;
}

/** Kept for compat with earlier integrations. */
export function getLastRoute(): null | string {
  return _lastRoute;
}

export function useTraceNavigation(navigationRef: NavigationRefLike): void {
  const lastRouteRef = useRef<null | string>(null);
  const enteredAtRef = useRef<null | number>(null);

  useEffect(() => {
    if (typeof navigationRef.addListener !== 'function') return;
    if (typeof navigationRef.getCurrentRoute !== 'function') return;

    const noteScreen = (from: null | string, to: string) => {
      const dwellMs =
        enteredAtRef.current !== null
          ? Math.max(0, Date.now() - enteredAtRef.current)
          : undefined;
      lastRouteRef.current = to;
      _lastRoute = to;
      enteredAtRef.current = Date.now();
      pushSignal('nav', {
        from: from ?? undefined,
        to,
        dwellMsPrev: dwellMs,
      });
    };

    const initial = navigationRef.getCurrentRoute()?.name ?? null;
    if (initial !== null) noteScreen(null, initial);

    const unsubscribe = navigationRef.addListener('state', () => {
      const next = navigationRef.getCurrentRoute()?.name ?? null;
      const prev = lastRouteRef.current;
      if (next === null || next === prev) return;
      noteScreen(prev, next);
    });

    return () => {
      unsubscribe();
      enteredAtRef.current = null;
    };
  }, [navigationRef]);
}

export const __resetForTests = (): void => {
  _lastRoute = null;
};
