// Theme selection, delegated to GDS.
//
// The colour values live in @goliapkg/gds — the same system golia.jp
// and the legacy dashboard run on, so the three surfaces read as one
// product rather than three houses with similar paint. GDS keeps the
// theme in an atom, persists it, and `useThemeEffect()` paints the
// resolved `--gds-*` custom properties onto <html> reactively.
//
// This module adds only what Sentori needs on top: the first-run
// posture, and one synchronous paint before React mounts.
//
// Dark stays the default. GDS is dark-native (light is a derived
// adaptation), golia.jp and the marketing site default dark, and half
// an hour of reading stack traces in light mode is measurably more
// tiring.

import {
  DEFAULT_THEME,
  loadPersistedTheme,
  resolveThemeCssVars,
  type ThemeMode,
} from '@goliapkg/gds/systems';

export type { ThemeMode };

/** Compact density: this is a triage tool, not a marketing page. */
const SENTORI_DEFAULT = {
  ...DEFAULT_THEME,
  mode: 'dark' as ThemeMode,
  density: 'compact' as const,
};

export function systemMode(): 'dark' | 'light' {
  if (typeof window === 'undefined' || !window.matchMedia) return 'dark';
  return window.matchMedia('(prefers-color-scheme: light)').matches
    ? 'light'
    : 'dark';
}

/**
 * Paint the persisted theme before React mounts.
 *
 * Resolving inside an effect means a light-mode user watches the app
 * flash dark on every single load, so the entry module calls this
 * synchronously. `useThemeEffect()` takes over for changes afterwards.
 */
export function initTheme(): 'dark' | 'light' {
  const saved = loadPersistedTheme() ?? SENTORI_DEFAULT;
  const resolved = saved.mode === 'system' ? systemMode() : saved.mode;
  if (typeof document !== 'undefined') {
    const vars = resolveThemeCssVars(saved, resolved);
    const root = document.documentElement;
    for (const [k, v] of Object.entries(vars)) root.style.setProperty(k, v);
    root.dataset.theme = resolved;
  }
  return resolved;
}
