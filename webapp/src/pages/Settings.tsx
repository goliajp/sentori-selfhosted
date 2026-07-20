import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import { api, UsageResponse } from '../lib/api';
import { Card, PageHeader, Section, Badge } from '../components/ui';

export function SettingsPage() {
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
    } catch {}
    localStorage.removeItem('sentori_user_id');
    localStorage.removeItem('sentori_email');
    navigate('/login');
  }

  return (
    <div className="p-8">
      <PageHeader
        title="Settings"
        subtitle="Workspace + plan + integrations + members."
      />

      {email && (
        <Section title="Account">
          <Card>
            <div className="flex items-center justify-between p-4">
              <div>
                <p className="text-xs text-zinc-500">Signed in as</p>
                <p className="font-mono text-sm">{email}</p>
              </div>
              <button
                onClick={logout}
                className="rounded border border-red-500 px-3 py-1.5 text-sm text-red-400 hover:bg-red-500 hover:text-white"
              >
                Sign out
              </button>
            </div>
          </Card>
        </Section>
      )}

      <Section title="Plan">
        <Card>
          <div className="grid grid-cols-3 divide-x divide-zinc-800">
            <Cell label="Tier">
              {usage ? (
                <Badge tone={usage.plan === 'free' ? 'neutral' : 'info'}>
                  {usage.plan}
                </Badge>
              ) : (
                '—'
              )}
            </Cell>
            <Cell label="Status">
              {usage ? (
                <Badge tone={usage.status === 'active' ? 'ok' : 'warn'}>
                  {usage.status}
                </Badge>
              ) : (
                '—'
              )}
            </Cell>
            <Cell label="Period">
              <span className="font-mono text-sm">
                {usage?.period_yyyymm ?? '—'}
              </span>
            </Cell>
          </div>
        </Card>
      </Section>

      <Section title="Members">
        <Card>
          <div className="p-6 text-sm text-zinc-500">
            Member management UI lands in v0.1.x. Backend ready (K1
            workspace_members + K16 tenant-scoping ACL gate). Use the
            <code className="mx-1 rounded bg-zinc-800 px-1 py-0.5 text-xs">
              sentorictl
            </code>
            CLI for now.
          </div>
        </Card>
      </Section>

      <Section title="Integrations">
        <Card>
          <div className="p-6 text-sm text-zinc-500">
            K12 IntegrationAdapter trait shipped with Slack reference impl.
            UI for connect/disconnect lands as K12.1-K12.4 vendor adapters
            roll out (Linear / Jira / GitHub / GitLab).
          </div>
        </Card>
      </Section>

      <Section title="Notifier transports">
        <Card>
          <div className="p-6 text-sm text-zinc-500">
            K11 NotifierService is operator-configured via env at boot
            (SMTP host / port / auth). Webhook + Mock transports always
            available. delivery_log persistence visible via the audit
            log when admin actions trigger fan-out.
          </div>
        </Card>
      </Section>

      <Section title="Active sessions">
        <Card>
          <div className="p-4 flex items-center justify-between">
            <p className="text-sm text-zinc-300">
              Detailed list, IP+UA per session, revoke individual entries.
            </p>
            <button
              onClick={() => navigate('/sessions')}
              className="rounded border border-zinc-700 px-3 py-1.5 text-sm hover:bg-zinc-800"
            >
              Open Sessions →
            </button>
          </div>
        </Card>
        <SessionsCard />
      </Section>

      <Section title="API ingest">
        <Card>
          <div className="p-6 text-sm text-zinc-300">
            <p className="mb-2">
              Send events to:{' '}
              <code className="rounded bg-zinc-800 px-1 py-0.5 text-xs">
                POST /v1/events/&lt;project_id&gt;
              </code>
            </p>
            <p className="text-sm text-zinc-500">
              Per-project token auth lands with K2 token middleware in
              v0.1.x. Until then, restrict access to the ingest port at the
              network layer (firewall / k8s NetworkPolicy / Caddy
              allowlist).
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
    <div className="p-4">
      <p className="mb-1 text-[11px] uppercase tracking-wide text-zinc-500">
        {label}
      </p>
      <div>{children}</div>
    </div>
  );
}

function SessionsCard() {
  const [rows, setRows] = useState<
    {
      id_hash_hex: string;
      created_at: string;
      last_used_at: string | null;
      expires_at: string;
      ip: string | null;
      user_agent: string | null;
    }[]
  >([]);
  const [loading, setLoading] = useState(true);

  async function refresh() {
    setLoading(true);
    try {
      const r = await api.listSessions();
      setRows(r.sessions);
    } catch {
      // noop
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    refresh();
  }, []);

  async function revoke(id: string) {
    if (!confirm('Revoke this session?')) return;
    await api.revokeSession(id);
    await refresh();
  }

  return (
    <Card>
      <div className="p-4 text-sm">
        {loading ? (
          <p className="text-zinc-500 text-xs">Loading…</p>
        ) : rows.length === 0 ? (
          <p className="text-zinc-500 text-xs">No active sessions.</p>
        ) : (
          <ul className="divide-y divide-zinc-800">
            {rows.map(s => (
              <li
                key={s.id_hash_hex}
                className="flex items-center justify-between py-2"
              >
                <div>
                  <p className="font-mono text-[10px] text-zinc-400">
                    {s.id_hash_hex.slice(0, 12)}…
                  </p>
                  <p className="text-[10px] text-zinc-500">
                    {s.ip ?? '?'} · {s.user_agent?.slice(0, 40) ?? '?'}
                  </p>
                  <p className="text-[10px] text-zinc-500">
                    expires {s.expires_at}
                  </p>
                </div>
                <button
                  onClick={() => revoke(s.id_hash_hex)}
                  className="rounded border border-red-700 px-2 py-1 text-[10px] text-red-300 hover:bg-red-700/30"
                >
                  Revoke
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </Card>
  );
}
