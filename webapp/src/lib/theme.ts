// Theme selection, self-owned.
//
// The whole mechanism: a persisted mode ('system' | 'light' |
// 'dark'), one resolved `data-theme` attribute on <html> that the
// stylesheet keys every token off, and a media-query listener for
// people who picked "system". No design-system runtime, no atom —
// the stylesheet is the single source of colour truth.
//
// Dark stays the default posture: this is a triage tool read at
// length, and golia.jp itself defaults dark.

import { useSyncExternalStore } from 'react';

export type ThemeMode = 'dark' | 'light' | 'system';

const STORAGE_KEY = 'sentori-theme';

let _mode: ThemeMode = 'dark';
const _listeners = new Set<() => void>();

function systemMode(): 'dark' | 'light' {
  if (typeof window === 'undefined' || !window.matchMedia) return 'dark';
  return window.matchMedia('(prefers-color-scheme: light)').matches
    ? 'light'
    : 'dark';
}

function apply(): void {
  const resolved = _mode === 'system' ? systemMode() : _mode;
  document.documentElement.dataset.theme = resolved;
}

/**
 * Paint the persisted theme before React mounts, so no page flashes
 * the wrong scheme. Returns the resolved mode.
 */
export function initTheme(): 'dark' | 'light' {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved === 'light' || saved === 'dark' || saved === 'system') _mode = saved;
  } catch {
    // storage unavailable — the dark default stands
  }
  apply();
  if (typeof window !== 'undefined' && window.matchMedia) {
    window
      .matchMedia('(prefers-color-scheme: light)')
      .addEventListener('change', () => {
        if (_mode === 'system') {
          apply();
          _listeners.forEach((l) => l());
        }
      });
  }
  return _mode === 'system' ? systemMode() : _mode;
}

export function setThemeMode(mode: ThemeMode): void {
  _mode = mode;
  try {
    localStorage.setItem(STORAGE_KEY, mode);
  } catch {
    // storage unavailable — the choice still applies this session
  }
  apply();
  _listeners.forEach((l) => l());
}

/** The current mode, reactive. */
export function useThemeMode(): ThemeMode {
  return useSyncExternalStore(
    (cb) => {
      _listeners.add(cb);
      return () => _listeners.delete(cb);
    },
    () => _mode,
  );
}
