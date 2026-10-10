import { AuthenticationMode } from '../components/AuthenticationMode';
import { AuthShell } from '../components/AuthShell';
import { Button } from '../../shared/design/controls';
// Authentication mode is supplied by this instance.

import { useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';

import { Field, Input } from '../components/ui';
import { useT } from '../i18n';
import { api } from '../lib/api';

export function LoginPage() {
  const t = useT();
  const navigate = useNavigate();
  const [email, setEmail] = useState(__LOCAL_DEV_LOGIN__?.email ?? '');
  const [password, setPassword] = useState(__LOCAL_DEV_LOGIN__?.password ?? '');
  const [loading, setLoading] = useState(false);
  const [failed, setFailed] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setFailed(false);
    try {
      await api.login(email, password);
      const returnTo = sessionStorage.getItem('sentori_return_to');
      sessionStorage.removeItem('sentori_return_to');
      navigate(returnTo && returnTo.startsWith('/') && !returnTo.startsWith('//') && !returnTo.includes('\\') ? returnTo : '/');
    } catch {
      setFailed(true);
    } finally {
      setLoading(false);
    }
  };

  return (
    <AuthShell><AuthenticationMode>
      <form onSubmit={submit} className="auth-form">
        <h1 className="text-lg font-semibold">{t('auth.signInTitle')}</h1>
        <Field label={t('auth.email')}>
          <Input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            autoComplete="username"
            required
          />
        </Field>
        <Field label={t('auth.password')}>
          <Input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoComplete="current-password"
            required
          />
        </Field>

        {failed && <p role="alert" className="text-xs text-kind-error">{t('auth.signInFailed')}</p>}

        <Button size="large" variant="primary"
          type="submit"
          disabled={loading}
          busy={loading}
        >
          {loading ? t('auth.signingIn') : t('auth.signIn')}
        </Button>

        <p className="pt-1 text-center text-xs text-fg-subtle">
          <Link to="/forgot-password" className="hover:text-fg-muted">
            {t('auth.forgot')}
          </Link>
        </p>
        <p className="border-t border-border pt-4 text-center text-sm text-fg-muted">
          <Link to="/welcome" className="hover:text-fg">{t('site.discover')}</Link>
        </p>
      </form>
    </AuthenticationMode></AuthShell>
  );
}
