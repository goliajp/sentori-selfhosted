import type { ReactNode } from 'react';
import { Button } from '../../shared/design/controls';
import { ErrorBanner } from './ui';
import { useT } from '../i18n';
import { api } from '../lib/api';
import { useAsyncData } from '../lib/useAsyncData';

/** Unknown/unavailable configuration never offers an incorrect password flow. */
export function AuthenticationMode({ children, account = false }: { children: ReactNode; account?: boolean }) {
  const t = useT();
  const mode = useAsyncData(() => api.authConfig(), []);
  if (!mode.data) return <div className="auth-form">
    {mode.error ? <><ErrorBanner reason={mode.error}>{t('auth.modeFailed')}</ErrorBanner><Button onClick={mode.reload}>{t('common.retry')}</Button></> : <p role="status">{t('shell.loading')}</p>}
  </div>;
  if (mode.data.mode === 'password') return children;
  return <div className="auth-form">
    <h1 className="text-lg font-semibold">{t('auth.cloudTitle')}</h1>
    <p className="text-sm text-fg-muted">{t(account ? 'auth.cloudManaged' : 'auth.cloudHint')}</p>
    <a className="button button-primary button-large" href={account ? `${mode.data.controlOrigin}/cloud/account` : '/auth/cloud/start'}>{t(account ? 'auth.cloudAccount' : 'auth.cloudSignIn')}</a>
  </div>;
}
