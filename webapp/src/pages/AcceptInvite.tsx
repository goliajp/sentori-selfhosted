import { useEffect, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';

import { useT } from '../i18n';
import { api } from '../lib/api';

/// Landing page for an invite link (`/invite?token=…`). The logged-in
/// visitor joins the token's workspace; a not-logged-in visitor is
/// pointed at login/register first (the token is preserved in the URL
/// so reopening the link after auth completes the join).
type State =
  | { kind: 'working' }
  | { kind: 'joined'; workspace_id: string; role: string }
  | { kind: 'need_auth' }
  | { kind: 'error'; message: string };

export default function AcceptInvite() {
  const t = useT();
  const [params] = useSearchParams();
  const token = params.get('token') ?? '';
  // Derive the no-token error state at init rather than via a
  // synchronous setState in the effect (which cascades renders).
  const [state, setState] = useState<State>(() =>
    token ? { kind: 'working' } : { kind: 'error', message: t('auth.missingInviteToken') },
  );

  useEffect(() => {
    if (!token) return;
    let cancelled = false;
    // Confirm there's a session first: accepting needs one, and a
    // bare accept call would bounce through the global 401 redirect
    // and lose the token.
    api
      .authMe()
      .then(() => api.acceptInvite(token))
      .then(r => {
        if (!cancelled) {
          setState({
            kind: 'joined',
            workspace_id: r.workspace_id,
            role: r.role,
          });
        }
      })
      .catch(e => {
        if (cancelled) return;
        const msg = String(e);
        if (msg.includes('401') || msg.toLowerCase().includes('unauthorized')) {
          setState({ kind: 'need_auth' });
        } else {
          setState({ kind: 'error', message: msg });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [token]);

  return (
    <div className="flex min-h-screen items-center justify-center bg-bg px-4">
      <div className="w-full max-w-sm rounded-lg border border-border bg-surface p-6 text-center">
        <h1 className="text-lg font-semibold text-fg">{t('auth.workspaceInvite')}</h1>

        {state.kind === 'working' && (
          <p className="mt-4 text-sm text-fg-muted">{t('auth.acceptingInvite')}</p>
        )}

        {state.kind === 'joined' && (
          <>
            <p className="mt-4 text-sm text-fg-muted">
              You've joined the workspace as{' '}
              <span className="font-medium text-accent">{state.role}</span>.
            </p>
            <Link
              to="/main"
              onClick={() => {
                // Land in the freshly-joined workspace's dashboard.
              }}
              className="mt-5 inline-block rounded bg-accent px-4 py-2 text-sm font-medium text-white hover:opacity-90"
            >{t('auth.goToDashboard')}</Link>
          </>
        )}

        {state.kind === 'need_auth' && (
          <>
            <p className="mt-4 text-sm text-fg-muted">
              Log in or create an account first, then reopen this invite
              link to join.
            </p>
            <div className="mt-5 flex justify-center gap-2">
              <Link
                to="/login"
                className="rounded bg-raised px-4 py-2 text-sm font-medium text-fg hover:bg-white"
              >{t('auth.logIn')}</Link>
              <Link
                to="/register"
                className="rounded border border-border-strong px-4 py-2 text-sm font-medium text-fg hover:border-border-strong"
              >{t('auth.signUp')}</Link>
            </div>
          </>
        )}

        {state.kind === 'error' && (
          <p className="mt-4 text-sm text-danger">{state.message}</p>
        )}
      </div>
    </div>
  );
}
