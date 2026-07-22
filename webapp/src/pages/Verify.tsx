// Email-verification landing — the link in the verification
// email points here with ?token=…; we consume it immediately.

import { useEffect, useRef, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';

import { useT } from '../i18n';
import { api } from '../lib/api';

export default function Verify() {
  const t = useT();
  const [params] = useSearchParams();
  const token = params.get('token') ?? '';
  // A missing token is knowable during render — no effect needed.
  const [result, setResult] = useState<{
    state: 'working' | 'ok' | 'error';
    err: string | null;
  }>(() =>
    token
      ? { state: 'working', err: null }
      : { state: 'error', err: t('verify.missingToken') },
  );
  const { state, err } = result;
  const fired = useRef(false);

  useEffect(() => {
    if (!token || fired.current) return;
    fired.current = true;
    api
      .authVerify(token)
      .then(() => setResult({ state: 'ok', err: null }))
      .catch((e: unknown) =>
        setResult({ state: 'error', err: String(e) }),
      );
  }, [token]);

  return (
    <div className="flex h-screen items-center justify-center bg-bg">
      <div className="w-96 rounded-lg border border-border bg-surface p-6">
        <h1 className="mb-1 text-xl font-semibold">{t('auth.verifyEmail')}</h1>
        <p className="mb-6 text-sm text-fg-subtle">Sentori</p>
        {state === 'working' && (
          <p className="text-sm text-fg-muted">{t('auth.verifying')}</p>
        )}
        {state === 'ok' && (
          <div className="space-y-3">
            <p className="text-sm text-accent">
              Email verified — your account is active.
            </p>
            <Link
              to="/login"
              className="block rounded bg-accent px-3 py-2 text-center text-sm font-medium text-white hover:opacity-90"
            >
              {t('auth.signIn')}
            </Link>
          </div>
        )}
        {state === 'error' && (
          <div className="space-y-3">
            <p className="break-all text-xs text-danger">{err}</p>
            <p className="text-xs text-fg-subtle">
              The link may have expired. Sign up again or request a
              new verification email.
            </p>
            <Link
              to="/login"
              className="block rounded border border-border-strong px-3 py-2 text-center text-sm text-fg-muted hover:bg-raised"
            >{t('auth.backToSignIn')}</Link>
          </div>
        )}
      </div>
    </div>
  );
}
