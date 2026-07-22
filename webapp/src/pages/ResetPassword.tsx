// Password-reset landing — the link in the reset email points
// here with ?token=…; the user picks a new password.

import { useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';

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
      await api.authResetPassword(token, password);
      setDone(true);
    } catch (e) {
      setErr(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="flex h-screen items-center justify-center bg-bg">
      <form
        onSubmit={submit}
        className="w-96 rounded-lg border border-border bg-surface p-6"
      >
        <h1 className="mb-1 text-xl font-semibold">{t('auth.resetPassword')}</h1>
        <p className="mb-6 text-sm text-fg-subtle">Sentori</p>
        {done ? (
          <div className="space-y-3">
            <p className="text-sm text-accent">
              Password updated — sign in with your new password.
            </p>
            <Link
              to="/login"
              className="block rounded bg-accent px-3 py-2 text-center text-sm font-medium text-white hover:opacity-90"
            >
              {t('auth.signIn')}
            </Link>
          </div>
        ) : (
          <>
            {!token && (
              <p className="mb-3 text-xs text-danger">
                Missing token — open the link from your email.
              </p>
            )}
            <label className="mb-3 block text-sm">
              <span className="mb-1 block text-fg-muted">
                New password (≥12 chars)
              </span>
              <input
                type="password"
                autoFocus
                value={password}
                onChange={e => setPassword(e.target.value)}
                className="w-full rounded border border-border-strong bg-bg px-3 py-2 text-sm focus:border-accent focus:outline-none"
              />
            </label>
            <label className="mb-4 block text-sm">
              <span className="mb-1 block text-fg-muted">{t('auth.confirm')}</span>
              <input
                type="password"
                value={confirm}
                onChange={e => setConfirm(e.target.value)}
                className="w-full rounded border border-border-strong bg-bg px-3 py-2 text-sm focus:border-accent focus:outline-none"
              />
            </label>
            {err && (
              <p className="mb-3 break-all text-xs text-danger">{err}</p>
            )}
            <button
              type="submit"
              disabled={loading || !token}
              className="w-full rounded bg-accent px-3 py-2 text-sm font-medium text-white hover:opacity-90 disabled:opacity-50"
            >
              {loading ? t('auth.saving') : t('auth.setNewPassword')}
            </button>
            <p className="mt-4 text-center text-xs text-fg-subtle">
              <Link to="/login" className="hover:text-fg-muted">{t('auth.backToSignIn')}</Link>
            </p>
          </>
        )}
      </form>
    </div>
  );
}
