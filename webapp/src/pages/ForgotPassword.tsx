// Forgot password — calls /auth/forgot-password; the reset link
// arrives by email only.

import { useState } from 'react';
import { Link } from 'react-router-dom';

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
      await api.authForgotPassword(email);
      setSilent(true);
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
        <h1 className="mb-1 text-xl font-semibold">{t('auth.forgot')}</h1>
        <p className="mb-6 text-sm text-fg-subtle">
          We'll email you a password reset link.
        </p>
        <label className="mb-3 block text-sm">
          <span className="mb-1 block text-fg-muted">{t('auth.email')}</span>
          <input
            type="email"
            autoFocus
            value={email}
            onChange={e => setEmail(e.target.value)}
            className="w-full rounded border border-border-strong bg-bg px-3 py-2 text-sm focus:border-accent focus:outline-none"
          />
        </label>
        {err && (
          <p className="mb-3 text-xs text-danger break-all">{err}</p>
        )}
        {silent && (
          <p className="mb-3 text-xs text-fg-muted">
            If that email is registered, instructions have been sent.
          </p>
        )}
        <button
          type="submit"
          disabled={loading}
          className="w-full rounded bg-accent px-3 py-2 text-sm font-medium text-white hover:opacity-90 disabled:opacity-50"
        >
          {loading ? t('auth.sending') : t('auth.sendReset')}
        </button>
        <div className="mt-4 text-center text-xs text-fg-subtle">
          <Link to="/login" className="hover:text-fg-muted">{t('auth.backToSignIn')}</Link>
        </div>
      </form>
    </div>
  );
}
