// Register page — calls /auth/register, surfaces the verify
// token plaintext (which would be emailed in prod).

import { useState } from 'react';
import { Link } from 'react-router-dom';

import { api } from '../lib/api';

export default function Register() {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [err, setErr] = useState<string | null>(null);
  const [verifyToken, setVerifyToken] = useState<string | null>(null);
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
      const r = await api.authRegister({ email, password });
      setVerifyToken(r.verify_token);
    } catch (e) {
      setErr(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="flex h-screen items-center justify-center bg-zinc-950">
      <form
        onSubmit={submit}
        className="w-96 rounded-lg border border-zinc-800 bg-zinc-900 p-6"
      >
        <h1 className="mb-1 text-xl font-semibold">Create account</h1>
        <p className="mb-6 text-sm text-zinc-500">Sentori v0.2</p>
        {verifyToken ? (
          <div className="space-y-3">
            <p className="text-sm text-zinc-300">
              Account created. Verify with this token:
            </p>
            <pre className="overflow-x-auto whitespace-pre-wrap break-all rounded bg-zinc-950 p-3 text-xs font-mono text-emerald-400">
              {verifyToken}
            </pre>
            <p className="text-xs text-zinc-500">
              In production this would be in your inbox. Paste into the
              verify endpoint to activate.
            </p>
            <Link
              to="/login"
              className="block rounded bg-emerald-600 px-3 py-2 text-center text-sm font-medium text-white hover:bg-emerald-500"
            >
              Continue to sign in
            </Link>
          </div>
        ) : (
          <>
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
              <span className="mb-1 block text-zinc-400">
                Password (≥12 chars)
              </span>
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
              {loading ? 'Creating…' : 'Create account'}
            </button>
            <div className="mt-4 text-center text-xs text-zinc-500">
              <Link to="/login" className="hover:text-zinc-300">
                Already have an account? Sign in
              </Link>
            </div>
          </>
        )}
      </form>
    </div>
  );
}
