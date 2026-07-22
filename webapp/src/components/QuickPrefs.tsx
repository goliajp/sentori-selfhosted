// Theme and language, reachable from anywhere.
//
// These were only in Settings, which is the wrong place for them:
// both are read-at-a-glance preferences someone flips while looking
// at something else — a screenshot for a colleague, a Japanese
// stack trace — not settings they navigate to and configure. They
// live in the sidebar footer, one row, always in reach.
//
// Two icon-width controls rather than two labelled selects: the
// sidebar is 224px wide and already carries the account row.

import { useSetThemeMode, useTheme } from '@goliapkg/gds/systems';

import { LOCALES, LOCALE_LABELS, useI18n } from '../i18n';
import type { ThemeMode } from '../lib/theme';

/** Cycles system → light → dark. The glyph shows what is in force,
 *  the tooltip names what a click will do — a control that only
 *  showed its current state would leave you clicking to find out. */
const THEME_ORDER: ThemeMode[] = ['system', 'light', 'dark'];
const THEME_GLYPH: Record<ThemeMode, string> = {
  system: '◐',
  light: '☀',
  dark: '☾',
};

export function QuickPrefs() {
  const { locale, setLocale, t } = useI18n();
  // GDS owns the theme atom, its persistence, and the repaint; this
  // control only names the next value in the cycle.
  const theme = useTheme().mode;
  const setThemeMode = useSetThemeMode();

  const themeLabel: Record<ThemeMode, string> = {
    system: t('prefs.themeSystem'),
    light: t('prefs.themeLight'),
    dark: t('prefs.themeDark'),
  };
  const next = THEME_ORDER[(THEME_ORDER.indexOf(theme) + 1) % THEME_ORDER.length];

  return (
    <div className="flex items-center gap-1">
      <button
        type="button"
        onClick={() => setThemeMode(next)}
        title={`${t('prefs.theme')}: ${themeLabel[theme]} → ${themeLabel[next]}`}
        aria-label={`${t('prefs.theme')}: ${themeLabel[theme]}`}
        className="inline-flex h-7 w-7 items-center justify-center rounded text-fg-subtle transition hover:bg-raised hover:text-fg focus-visible:outline focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-accent"
      >
        <span aria-hidden>{THEME_GLYPH[theme]}</span>
      </button>

      <label className="sr-only" htmlFor="quick-locale">
        {t('prefs.language')}
      </label>
      <select
        id="quick-locale"
        value={locale}
        onChange={e => setLocale(e.target.value as (typeof LOCALES)[number])}
        title={t('prefs.language')}
        className="h-7 rounded border border-transparent bg-transparent px-1 text-xs text-fg-subtle transition hover:border-border hover:text-fg focus-visible:outline focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-accent"
      >
        {LOCALES.map(l => (
          <option key={l} value={l}>
            {LOCALE_LABELS[l]}
          </option>
        ))}
      </select>
    </div>
  );
}
