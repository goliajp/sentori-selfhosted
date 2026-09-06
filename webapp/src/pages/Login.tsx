// Sign-in — the only public entrance. No self-signup, no OAuth
// (design.md §9): accounts come from the owner or the env bootstrap.

import { useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';

import { AuthShell, Field, Input } from '../components/ui';
import { useT } from '../i18n';
import { api } from '../lib/api';

export function LoginPage() {
  const t = useT();
  const navigate = useNavigate();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
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
      navigate(returnTo && returnTo.startsWith('/') ? returnTo : '/');
    } catch {
      setFailed(true);
    } finally {
      setLoading(false);
    }
  };

  return (
    <AuthShell>
      <form onSubmit={submit} className="space-y-3">
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

        {failed && <p className="text-xs text-kind-error">{t('auth.signInFailed')}</p>}

        <button
          type="submit"
          disabled={loading}
          className="h-8 w-full rounded-md bg-accent text-sm font-medium text-accent-fg transition hover:opacity-90 disabled:opacity-40"
        >
          {loading ? t('auth.signingIn') : t('auth.signIn')}
        </button>

        <p className="pt-1 text-center text-xs text-fg-subtle">
          <Link to="/forgot-password" className="hover:text-fg-muted">
            {t('auth.forgot')}
          </Link>
        </p>
      </form>
    </AuthShell>
  );
}
