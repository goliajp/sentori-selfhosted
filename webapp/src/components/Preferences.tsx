// Language and theme pickers.
//
// Both are segmented controls rather than dropdowns: three options
// each, all worth showing at once, and a segmented control states the
// current value without being opened.

import { useSetThemeMode, useTheme } from '@goliapkg/gds/systems';

import { LOCALES, LOCALE_LABELS, useI18n } from '../i18n';
import type { ThemeMode } from '../lib/theme';

export function Preferences() {
  const { locale, setLocale, t } = useI18n();
  const theme = useTheme().mode;
  const setThemeMode = useSetThemeMode();

  const themeOptions: { value: ThemeMode; label: string }[] = [
    { value: 'system', label: t('prefs.themeSystem') },
    { value: 'light', label: t('prefs.themeLight') },
    { value: 'dark', label: t('prefs.themeDark') },
  ];

  return (
    <div className="grid gap-6 px-5 py-4 sm:grid-cols-2">
      <Field label={t('prefs.theme')}>
        <Segmented
          options={themeOptions}
          value={theme}
          onChange={setThemeMode}
        />
      </Field>
      <Field label={t('prefs.language')}>
        <Segmented
          options={LOCALES.map(l => ({ value: l, label: LOCALE_LABELS[l] }))}
          value={locale}
          onChange={setLocale}
        />
      </Field>
    </div>
  );
}

function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <p className="mb-2 text-xs uppercase tracking-wide text-fg-subtle">
        {label}
      </p>
      {children}
    </div>
  );
}

function Segmented<T extends string>({
  options,
  value,
  onChange,
}: {
  options: { value: T; label: string }[];
  value: T;
  onChange: (v: T) => void;
}) {
  return (
    <div
      role="radiogroup"
      className="inline-flex rounded border border-border bg-surface p-0.5"
    >
      {options.map(o => {
        const selected = o.value === value;
        return (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={selected}
            onClick={() => onChange(o.value)}
            className={`rounded px-3 py-1 text-sm transition focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent ${
              selected
                ? 'bg-raised text-fg'
                : 'text-fg-muted hover:text-fg'
            }`}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}
