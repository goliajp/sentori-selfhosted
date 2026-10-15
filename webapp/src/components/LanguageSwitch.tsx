import { Select } from './ui';
import { LOCALES, LOCALE_LABELS, useI18n, type Locale } from '../i18n';

export function LanguageSwitch() {
  const { t, locale, setLocale } = useI18n();
  return <Select aria-label={t('settings.language')} value={locale}
    onChange={event => setLocale(event.target.value as Locale)}>
    {LOCALES.map(value => <option key={value} value={value}>{LOCALE_LABELS[value]}</option>)}
  </Select>;
}
