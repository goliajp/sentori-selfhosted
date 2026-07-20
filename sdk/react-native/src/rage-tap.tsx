// v0.9.0 #12 — rage-tap / multi-click detection.
//
// Wrap your app root (typically next to ErrorBoundary) with
// `<sentori.RageTapCapture>{children}</sentori.RageTapCapture>`.
// We listen to bubble-phase `onTouchEnd` and emit a `ui.multiClick`
// breadcrumb when the same native target receives ≥ 3 taps within
// 800 ms. Pure observation — no event capture, no gesture
// interference; existing Touchables / Pressables / GestureHandler
// continue to fire normally.

import React, { useCallback, useRef } from 'react';
import { View, type GestureResponderEvent, type ViewProps } from 'react-native';

import { addBreadcrumb } from './breadcrumbs';
import {
  RAGE_THRESHOLD,
  RAGE_WINDOW_MS,
  recordTap,
} from './rage-tap-detector';

export function RageTapCapture({
  children,
  ...rest
}: ViewProps & { children?: React.ReactNode }): React.JSX.Element {
  const recent = useRef<Map<number, number[]>>(new Map());

  const onTouchEnd = useCallback((e: GestureResponderEvent) => {
    const target = e.nativeEvent?.target;
    if (typeof target !== 'number') return;
    if (recordTap(recent.current, target, Date.now())) {
      addBreadcrumb({
        type: 'user',
        data: {
          kind: 'ui.multiClick',
          target: String(target),
          taps: RAGE_THRESHOLD,
          windowMs: RAGE_WINDOW_MS,
        },
      });
    }
  }, []);

  return (
    <View {...rest} onTouchEnd={onTouchEnd}>
      {children}
    </View>
  );
}
