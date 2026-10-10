import { AuthenticationMode } from '../components/AuthenticationMode';
import { AuthShell } from '../components/AuthShell';
import { Button } from '../../shared/design/controls';
// Password-reset landing — the link in the reset email points
// here with ?token=…; the user picks a new password.

import { useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';

import { Field, Input } from '../components/ui';
import { useT } from '../i18n';
import { api } from '../lib/api';

export default function ResetPassword() {
  const t = useT();
  const [params] = useSearchParams();
  const token = params.get('token') ?? '';
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [err, setErr] = useState<string | null>(null);
  const [done, setDone] = useState(false);
  const [loading, setLoading] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setErr(null);
    if (password.length < 12) {
      setErr(t('auth.passwordTooShort'));
      return;
    }
    if (password !== confirm) {
      setErr(t('auth.passwordsDiffer'));
      return;
    }
    setLoading(true);
    try {
      await api.resetPassword(token, password);
      setDone(true);
    } catch {
      setErr(t('auth.resetFailed'));
    } finally {
      setLoading(false);
    }
  }

  return (
    <AuthShell><AuthenticationMode>
      {done ? (
        <div className="auth-form">
          <h1 className="text-lg font-semibold">{t('auth.resetPassword')}</h1>
          <p className="text-sm text-fg-muted">{t('auth.resetDone')}</p>
          <Link
            to="/login"
            className="button button-primary button-large"
          >
            {t('auth.signIn')}
          </Link>
        </div>
      ) : (
        <form onSubmit={submit} className="auth-form">
          <h1 className="text-lg font-semibold">{t('auth.resetPassword')}</h1>
          {!token && <p className="text-xs text-danger">{t('auth.missingToken')}</p>}
          <Field label={t('auth.newPassword12')}>
            <Input
              type="password"
              autoFocus
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              minLength={12}
              autoComplete="new-password"
              required
            />
          </Field>
          <Field label={t('auth.confirm')}>
            <Input
              type="password"
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              minLength={12}
              autoComplete="new-password"
              required
            />
          </Field>
          {err && <p role="alert" className="break-all text-xs text-danger">{err}</p>}
          <Button size="large" variant="primary"
            type="submit"
            disabled={loading || !token}
            busy={loading}
          >
            {loading ? t('auth.saving') : t('auth.setNewPassword')}
          </Button>
          <p className="pt-1 text-center text-xs text-fg-subtle">
            <Link to="/login" className="hover:text-fg-muted">
              {t('auth.backToSignIn')}
            </Link>
          </p>
        </form>
      )}
    </AuthenticationMode></AuthShell>
  );
}
