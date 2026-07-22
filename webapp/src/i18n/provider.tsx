// The locale provider. Kept apart from the hooks and constants in
// `./index.ts` so this file exports a component and nothing else —
// react-refresh can only hot-swap a module whose exports are all
// components.

import { useCallback, useMemo, useState, type ReactNode } from 'react';

import { CATALOGUES, LocaleContext, STORAGE_KEY, detectLocale, type Locale } from './index';
import type { MessageKey } from './en';

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(() => {
    const initial = detectLocale();
    if (typeof document !== 'undefined') {
      document.documentElement.lang = initial;
    }
    return initial;
  });

  const setLocale = useCallback((next: Locale) => {
    setLocaleState(next);
    // Screen readers and the browser's own translation prompt both
    // read this; leaving it stale is worse than not setting it.
    if (typeof document !== 'undefined') {
      document.documentElement.lang = next;
    }
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // Choice just won't survive the reload.
    }
  }, []);

  const value = useMemo(
    () => ({
      locale,
      setLocale,
      t: (key: MessageKey) => CATALOGUES[locale][key],
    }),
    [locale, setLocale],
  );

  return (
    <LocaleContext.Provider value={value}>{children}</LocaleContext.Provider>
  );
}
