// Register page — calls /auth/register, surfaces the verify
// token plaintext (which would be emailed in prod).

import { useState } from 'react';
import { Link } from 'react-router-dom';

import { useT } from '../i18n';
import { api } from '../lib/api';

export default function Register() {
  const t = useT();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [err, setErr] = useState<string | null>(null);
  const [done, setDone] = useState(false);
  const [loading, setLoading] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setErr(null);
    if (password.length < 12) {
      setErr('password must be ≥12 chars');
      return;
    }
    setLoading(true);
    try {
      await api.authRegister({ email, password });
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
        <h1 className="mb-1 text-xl font-semibold">{t('auth.createAccount')}</h1>
        <p className="mb-6 text-sm text-fg-subtle">Sentori v0.2</p>
        {done ? (
          <div className="space-y-3">
            <p className="text-sm text-fg-muted">
              Account created — check your inbox.
            </p>
            <p className="text-xs text-fg-subtle">
              We emailed you a verification link. Open it to activate
              your account, then sign in.
            </p>
            <Link
              to="/login"
              className="block rounded bg-accent px-3 py-2 text-center text-sm font-medium text-white hover:opacity-90"
            >{t('auth.continueToSignIn')}</Link>
          </div>
        ) : (
          <>
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
            <label className="mb-4 block text-sm">
              <span className="mb-1 block text-fg-muted">
                {t('auth.passwordMin')}
              </span>
              <input
                type="password"
                value={password}
                onChange={e => setPassword(e.target.value)}
                className="w-full rounded border border-border-strong bg-bg px-3 py-2 text-sm focus:border-accent focus:outline-none"
              />
            </label>
            {err && (
              <p className="mb-3 text-xs text-danger break-all">{err}</p>
            )}
            <button
              type="submit"
              disabled={loading}
              className="w-full rounded bg-accent px-3 py-2 text-sm font-medium text-white hover:opacity-90 disabled:opacity-50"
            >
              {loading ? t('auth.creating') : t('auth.createAccount')}
            </button>
            <div className="mt-4 text-center text-xs text-fg-subtle">
              <Link to="/login" className="hover:text-fg-muted">
                {t('auth.haveAccount')} {t('auth.signIn')}
              </Link>
            </div>
          </>
        )}
      </form>
    </div>
  );
}
