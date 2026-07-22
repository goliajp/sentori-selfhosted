// Dashboard sign-in. Calls /auth/login → stashes session token
// in localStorage (will become HttpOnly cookie once middleware
// lands in Phase E step 7+) then routes to Overview.

import { useEffect, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';

import { useT } from '../i18n';
import { api } from '../lib/api';

export function LoginPage() {
  const t = useT();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [err, setErr] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const navigate = useNavigate();

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setErr(null);
    if (!email || !password) {
      setErr('email + password required');
      return;
    }
    setLoading(true);
    try {
      const r = await api.authLogin({ email, password });
      // Session is stored as HttpOnly cookie server-side. We
      // only stash UI-display fields here (user_id + email),
      // never the token itself.
      localStorage.setItem('sentori_user_id', r.user_id);
      localStorage.setItem('sentori_email', r.email);
      // If the user was bounced here by the 401 redirect, bring
      // them back to where they were.
      // Default to the dashboard home path, not `/` — on SaaS `/` is
      // the marketing site and only resolves to this SPA via a
      // client-side navigation, so it breaks on refresh / bookmark.
      let returnTo = '/main';
      try {
        const stashed = sessionStorage.getItem('sentori_return_to');
        if (stashed) {
          returnTo = stashed;
          sessionStorage.removeItem('sentori_return_to');
        }
      } catch {
        // Storage disabled — fall through to the default returnTo.
      }
      navigate(returnTo);
    } catch (e) {
      setErr(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="flex h-screen items-center justify-center bg-bg">
      <form
        onSubmit={handleSubmit}
        className="w-80 rounded-lg border border-border bg-surface p-6"
      >
        <h1 className="mb-1 text-xl font-semibold">{t('auth.signInTitle')}</h1>
        <ServerVersion />
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
          <span className="mb-1 block text-fg-muted">{t('auth.password')}</span>
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
          {loading ? t('auth.signingIn') : t('auth.signIn')}
        </button>
        <OAuthButtons />
        <div className="mt-4 flex justify-between text-xs text-fg-subtle">
          <Link to="/register" className="hover:text-fg-muted">
            {t('auth.createAccount')}
          </Link>
          <Link to="/forgot-password" className="hover:text-fg-muted">
            {t('auth.forgot')}
          </Link>
        </div>
      </form>
    </div>
  );
}

const OAUTH_LABELS: Record<string, string> = {
  github: 'GitHub',
  google: 'Google',
};

// Renders nothing at all until the server confirms a provider is
// configured — a button that 400s on "oauth_not_configured" is worse
// than no button.
function OAuthButtons() {
  const t = useT();
  const [enabled, setEnabled] = useState<string[]>([]);

  useEffect(() => {
    api
      .authOAuthProviders()
      .then(p =>
        setEnabled(
          Object.entries(p)
            .filter(([, on]) => on)
            .map(([name]) => name),
        ),
      )
      .catch(() => setEnabled([]));
  }, []);

  if (enabled.length === 0) return null;

  return (
    <>
      <div className="my-4 flex items-center gap-3">
        <span className="h-px flex-1 bg-raised" />
        <span className="text-xs uppercase tracking-wide text-fg-subtle">
          {t('auth.or')}
        </span>
        <span className="h-px flex-1 bg-raised" />
      </div>
      <div className="flex flex-col gap-2">
        {enabled.map(name => (
          // A plain link, not fetch: /start answers 302 to the
          // provider, so the browser must be the one following it.
          <a
            key={name}
            href={`/auth/oauth/${name}/start`}
            className="w-full rounded border border-border-strong bg-surface px-3 py-2 text-center text-sm font-medium text-fg transition hover:bg-raised"
          >
            {t('auth.continueWith').replace('{provider}', OAUTH_LABELS[name] ?? name)}
          </a>
        ))}
      </div>
    </>
  );
}

function ServerVersion() {
  const [v, setV] = useState<string>('…');
  useEffect(() => {
    fetch('/healthz')
      .then(r => r.json())
      .then(j => setV(`v${j.version ?? '?'}`))
      .catch(() => setV('v?'));
  }, []);
  return <p className="mb-6 font-mono text-xs text-fg-subtle">{v}</p>;
}
