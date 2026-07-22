import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import { useT } from '../i18n';
import { api, UsageResponse } from '../lib/api';
import { useAsyncData } from '../lib/useAsyncData';
import { Preferences } from '../components/Preferences';
import { Card, PageHeader, Section, Badge } from '../components/ui';

export function SettingsPage() {
  const t = useT();
  const [usage, setUsage] = useState<UsageResponse | null>(null);
  const navigate = useNavigate();
  const email =
    typeof localStorage !== 'undefined'
      ? localStorage.getItem('sentori_email')
      : null;
  useEffect(() => {
    api.usage().then(setUsage).catch(() => {});
  }, []);

  async function logout() {
    // Cookie is HttpOnly; the server clears it via Set-Cookie
    // Max-Age=0 in its response. We just hit the endpoint and
    // tidy up the UI-display localStorage entries.
    try {
      await fetch('/auth/logout', {
        method: 'POST',
        credentials: 'include',
      });
    } catch {
      // Best-effort: local state is cleared below regardless.
    }
    localStorage.removeItem('sentori_user_id');
    localStorage.removeItem('sentori_email');
    navigate('/login');
  }

  return (
    <div>
      <PageHeader
        title={t('settings.title')}
        subtitle={t('settings.subtitle')}
      />

      {email && (
        <Section title={t('settings.account')}>
          <Card>
            <div className="flex items-center justify-between px-5 py-4">
              <div>
                <p className="text-xs text-fg-subtle">{t('settings.signedInAs')}</p>
                <p className="font-mono text-sm">{email}</p>
              </div>
              <button
                onClick={logout}
                className="rounded border border-danger/40 px-3 py-1.5 text-sm text-danger hover:bg-danger/20 hover:text-white"
              >
                {t('action.signOut')}
              </button>
            </div>
          </Card>
        </Section>
      )}

      <Section title={t('settings.preferences')}>
        <Card>
          <Preferences />
        </Card>
      </Section>

      <Section title={t('settings.plan')}>
        <Card>
          <div className="grid grid-cols-3 divide-x divide-border">
            <Cell label={t('settings.tier')}>
              {usage ? (
                <Badge tone={usage.plan === 'free' ? 'neutral' : 'info'}>
                  {usage.plan}
                </Badge>
              ) : (
                '—'
              )}
            </Cell>
            <Cell label={t('crash.status')}>
              {usage ? (
                <Badge tone={usage.status === 'active' ? 'ok' : 'warn'}>
                  {usage.status}
                </Badge>
              ) : (
                '—'
              )}
            </Cell>
            <Cell label={t('settings.period')}>
              <span className="font-mono text-sm">
                {usage?.period_yyyymm ?? '—'}
              </span>
            </Cell>
          </div>
          <div className="flex items-center justify-between border-t border-border px-5 py-4">
            <p className="text-sm text-fg-subtle">
              {t('settings.billingHint')}
            </p>
            <button
              onClick={() => navigate('/settings/billing')}
              className="inline-flex h-8 items-center rounded border border-border-strong px-3 text-sm hover:bg-raised"
            >
              {t('settings.manageBilling')} →
            </button>
          </div>
        </Card>
      </Section>

      <Section title={t('members.title')}>
        <Card>
          {/* This used to say member management was unbuilt and to use
              the CLI. Both the members page and the tokens page have
              existed for releases; a settings screen advertising a
              workaround for a shipped feature sends people the long way
              round. */}
          <div className="flex items-center justify-between px-5 py-4">
            <p className="text-sm text-fg-muted">{t('settings.membersHint')}</p>
            <button
              onClick={() => navigate('/members')}
              className="inline-flex h-8 shrink-0 items-center rounded border border-border-strong px-3 text-sm hover:bg-raised"
            >
              {t('settings.openMembers')} →
            </button>
          </div>
        </Card>
      </Section>

      <Section title={t('settings.integrations')}>
        <Card>
          <div className="p-6 text-sm text-fg-subtle">
            {t('settings.integrationsHint')}
          </div>
        </Card>
      </Section>

      <Section title={t('settings.notifiers')}>
        <Card>
          <div className="p-6 text-sm text-fg-subtle">
            {t('settings.notifiersHint')}
          </div>
        </Card>
      </Section>

      <Section title={t('settings.sessions')}>
        <Card>
          <div className="px-5 py-4 flex items-center justify-between">
            <p className="text-sm text-fg-muted">
              {t('settings.sessionsHint')}
            </p>
            <button
              onClick={() => navigate('/sessions')}
              className="inline-flex h-8 items-center rounded border border-border-strong px-3 text-sm hover:bg-raised"
            >
              {t('settings.openSessions')} →
            </button>
          </div>
        </Card>
        <SessionsCard />
      </Section>

      <Section title={t('settings.apiIngest')}>
        <Card>
          <div className="p-6 text-sm text-fg-muted">
            <p className="mb-2">
              <code className="rounded bg-raised px-1 py-0.5 text-xs">
                POST /v1/events/&lt;project_id&gt;
              </code>
            </p>
            <p className="text-sm text-fg-subtle">
              {t('settings.ingestHint')}
            </p>
          </div>
        </Card>
      </Section>
    </div>
  );
}

function Cell({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="px-5 py-4">
      <p className="mb-1 text-xs uppercase tracking-wide text-fg-subtle">
        {label}
      </p>
      <div>{children}</div>
    </div>
  );
}

function SessionsCard() {
  const t = useT();
  const { data, loading, reload: refresh } = useAsyncData<
    {
      id_hash_hex: string;
      created_at: string;
      last_used_at: string | null;
      expires_at: string;
      ip: string | null;
      user_agent: string | null;
    }[]
  >(async () => (await api.listSessions()).sessions, []);
  const rows = data ?? [];

  async function revoke(id: string) {
    if (!confirm(t('sessions.confirmRevoke'))) return;
    await api.revokeSession(id);
    refresh();
  }

  return (
    <Card>
      <div className="px-5 py-4 text-sm">
        {loading ? (
          <p className="text-fg-subtle text-xs">Loading…</p>
        ) : rows.length === 0 ? (
          <p className="text-fg-subtle text-xs">No active sessions.</p>
        ) : (
          <ul className="divide-y divide-border">
            {rows.map(s => (
              <li
                key={s.id_hash_hex}
                className="flex items-center justify-between py-2"
              >
                <div>
                  <p className="font-mono text-xs text-fg-muted">
                    {s.id_hash_hex.slice(0, 12)}…
                  </p>
                  <p className="text-xs text-fg-subtle">
                    {s.ip ?? '?'} · {s.user_agent?.slice(0, 40) ?? '?'}
                  </p>
                  <p className="text-xs text-fg-subtle">
                    expires {s.expires_at}
                  </p>
                </div>
                <button
                  onClick={() => revoke(s.id_hash_hex)}
                  className="rounded border border-danger/40 px-2 py-1 text-xs text-danger hover:bg-danger/20"
                >{t('action.revoke')}</button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </Card>
  );
}
