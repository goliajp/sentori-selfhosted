import type { ReactNode } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { PublicLayout as SharedPublicLayout } from '../../shared/site/PublicLayout';
import { SiteContext } from '../../shared/site/runtime';
import { siteImage } from '../../shared/site/assets';
import { siteMessages } from '../../shared/site/messages';
import { LOCALES, LOCALE_LABELS, useLocale, useSetLocale, type Locale } from '../i18n';
import { Select, ThemeSwitch } from './ui';
function siteHref(page: string, params?: Record<string, string>) {
  if (page === 'docs') {
    const { topic = '', anchor, ...query } = params ?? {};
    const search = new URLSearchParams(query).toString();
    return `/docs${topic ? `/${topic}` : ''}${search ? `?${search}` : ''}${anchor ? `#${encodeURIComponent(anchor)}` : ''}`;
  }
  return `/${page}${params ? `?${new URLSearchParams(params)}` : ''}`;
}
export function PublicLayout({ children }: { children: ReactNode }) {
  const locale = useLocale(), setLocale = useSetLocale();
  const navigate = useNavigate(), { pathname } = useLocation();
  const t = (key: string, params?: Record<string,string|number>) => {
    const row = siteMessages[key as keyof typeof siteMessages];
    if (!row) throw new Error(`Missing site message: ${key}`);
    return row[{zh: 0, ja: 1, en: 2}[locale]].replace(/\{(\w+)\}/g, (match, name: string) => params?.[name] == null ? match : String(params[name]));
  };
  return <SiteContext value={{ t, locale, number: (n, digits) => new Intl.NumberFormat(locale, {maximumFractionDigits:digits}).format(n), href: siteHref, navigate: (page, params) => navigate(siteHref(page, params)), image: siteImage, path: pathname.split('/')[1] || 'welcome' }}>
    <a href="#public-content" className="sr-only focus:not-sr-only">{t('site.skip')}</a>
    <SharedPublicLayout standalone footer={<div className="flex flex-wrap items-center gap-4"><Select aria-label={t('site.language')} value={locale} onChange={e => setLocale(e.target.value as Locale)}>{LOCALES.map(l => <option key={l} value={l}>{LOCALE_LABELS[l]}</option>)}</Select><ThemeSwitch /></div>}>{children}</SharedPublicLayout>
  </SiteContext>;
}
