// Dashboard sign-in. Calls /auth/login → stashes session token
// in localStorage (will become HttpOnly cookie once middleware
// lands in Phase E step 7+) then routes to Overview.

import { useEffect, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';

import { api } from '../lib/api';

export function LoginPage() {
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
      let returnTo = '/';
      try {
        const stashed = sessionStorage.getItem('sentori_return_to');
        if (stashed) {
          returnTo = stashed;
          sessionStorage.removeItem('sentori_return_to');
        }
      } catch {}
      navigate(returnTo);
    } catch (e) {
      setErr(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="flex h-screen items-center justify-center bg-zinc-950">
      <form
        onSubmit={handleSubmit}
        className="w-80 rounded-lg border border-zinc-800 bg-zinc-900 p-6"
      >
        <h1 className="mb-1 text-xl font-semibold">Sign in to Sentori</h1>
        <ServerVersion />
        <label className="mb-3 block text-sm">
          <span className="mb-1 block text-zinc-400">Email</span>
          <input
            type="email"
            autoFocus
            value={email}
            onChange={e => setEmail(e.target.value)}
            className="w-full rounded border border-zinc-700 bg-zinc-950 px-3 py-2 text-sm focus:border-brand-500 focus:outline-none"
          />
        </label>
        <label className="mb-4 block text-sm">
          <span className="mb-1 block text-zinc-400">Password</span>
          <input
            type="password"
            value={password}
            onChange={e => setPassword(e.target.value)}
            className="w-full rounded border border-zinc-700 bg-zinc-950 px-3 py-2 text-sm focus:border-brand-500 focus:outline-none"
          />
        </label>
        {err && (
          <p className="mb-3 text-xs text-red-400 break-all">{err}</p>
        )}
        <button
          type="submit"
          disabled={loading}
          className="w-full rounded bg-emerald-600 px-3 py-2 text-sm font-medium text-white hover:bg-emerald-500 disabled:opacity-50"
        >
          {loading ? 'Signing in…' : 'Sign in'}
        </button>
        <div className="mt-4 flex justify-between text-xs text-zinc-500">
          <Link to="/register" className="hover:text-zinc-300">
            Create account
          </Link>
          <Link to="/forgot-password" className="hover:text-zinc-300">
            Forgot password?
          </Link>
        </div>
      </form>
    </div>
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
  return <p className="mb-6 font-mono text-xs text-zinc-500">{v}</p>;
}
