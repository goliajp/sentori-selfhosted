// Forgot password — calls /auth/forgot-password; the reset link
// arrives by email only.

import { useState } from 'react';
import { Link } from 'react-router-dom';

import { AuthShell, Field, Input } from '../components/ui';
import { useT } from '../i18n';
import { api } from '../lib/api';

export default function ForgotPassword() {
  const t = useT();
  const [email, setEmail] = useState('');
  const [err, setErr] = useState<string | null>(null);
  const [silent, setSilent] = useState(false);
  const [loading, setLoading] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setErr(null);
    setSilent(false);
    setLoading(true);
    try {
      await api.forgotPassword(email);
      setSilent(true);
    } catch (e) {
      setErr(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <AuthShell>
      <form onSubmit={submit} className="space-y-3">
        <div>
          <h1 className="text-lg font-semibold">{t('auth.forgot')}</h1>
          <p className="mt-1 text-sm text-fg-subtle">{t('auth.forgotHint')}</p>
        </div>
        <Field label={t('auth.email')}>
          <Input
            type="email"
            autoFocus
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            autoComplete="username"
            required
          />
        </Field>
        {err && <p className="break-all text-xs text-danger">{err}</p>}
        {silent && <p className="text-xs text-fg-muted">{t('auth.resetSent')}</p>}
        <button
          type="submit"
          disabled={loading}
          className="h-8 w-full rounded-md bg-accent text-sm font-medium text-accent-fg transition hover:opacity-90 disabled:opacity-40"
        >
          {loading ? t('auth.sending') : t('auth.sendReset')}
        </button>
        <p className="pt-1 text-center text-xs text-fg-subtle">
          <Link to="/login" className="hover:text-fg-muted">
            {t('auth.backToSignIn')}
          </Link>
        </p>
      </form>
    </AuthShell>
  );
}
