// Locale plumbing: pick a language, remember it, translate.
//
// Deliberately not a library. The catalogue is three typed objects
// (see en.ts), so completeness is a compile-time property and the
// runtime job left over is small enough to read in one screen:
// resolve a locale, keep `<html lang>` honest, look a key up.
//
// The provider component lives in `./provider` — this module stays
// component-free so both halves hot-reload cleanly.

import { createContext, useContext } from 'react';

import { en, type MessageKey, type Messages } from './en';
import { ja } from './ja';
import { zh } from './zh';

export const LOCALES = ['en', 'zh', 'ja'] as const;
export type Locale = (typeof LOCALES)[number];

/** Endonyms — a language picker that names languages in the reader's
 *  own language is useless to the reader who cannot read the current
 *  one. */
export const LOCALE_LABELS: Record<Locale, string> = {
  en: 'English',
  ja: '\u65e5\u672c\u8a9e',
  zh: '\u7b80\u4f53\u4e2d\u6587',
};

export const CATALOGUES: Record<Locale, Messages> = { en, ja, zh };
export const STORAGE_KEY = 'sentori_locale';

function isLocale(v: string | null): v is Locale {
  return v !== null && (LOCALES as readonly string[]).includes(v);
}

/** Stored choice, else the closest match to the browser's languages,
 *  else English. */
export function detectLocale(): Locale {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (isLocale(stored)) return stored;
  } catch {
    // Storage disabled — fall through to negotiation.
  }
  const preferred =
    typeof navigator === 'undefined' ? [] : (navigator.languages ?? []);
  for (const tag of preferred) {
    const base = tag.toLowerCase().split('-')[0];
    if (isLocale(base)) return base;
  }
  return 'en';
}

export type I18nCtx = {
  locale: Locale;
  setLocale: (l: Locale) => void;
  t: (key: MessageKey) => string;
};

export const LocaleContext = createContext<I18nCtx | null>(null);

/** Translate. Throws outside the provider rather than silently
 *  rendering keys. */
export function useI18n(): I18nCtx {
  const ctx = useContext(LocaleContext);
  if (!ctx) throw new Error('useI18n must be used inside <I18nProvider>');
  return ctx;
}

/** Shorthand for the common case. */
export function useT(): (key: MessageKey) => string {
  return useI18n().t;
}
