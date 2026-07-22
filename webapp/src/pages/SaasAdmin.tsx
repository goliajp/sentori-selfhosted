// Cross-workspace view — only meaningful in SaaS deployment
// where one sentori-server instance fronts many workspaces.
// In self-hosted mode this just shows the single workspace row.

import { useEffect, useState } from 'react';

import { useT } from '../i18n';
import { api, SaasStats, WorkspaceRow } from '../lib/api';
import {
  Badge,
  Button,
  Card,
  CardBody,
  CardHeader,
  DataTable,
  EmptyState,
  ErrorBanner,
  PageHeader,
  formatNumber,
  formatRelative,
} from '../components/ui';

export default function SaasAdmin() {
  const t = useT();
  const [rows, setRows] = useState<WorkspaceRow[]>([]);
  const [stats, setStats] = useState<SaasStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [name, setName] = useState('');
  const [busy, setBusy] = useState<string | null>(null);

  useEffect(() => {
    void refresh().finally(() => setLoading(false));
  }, []);

  async function refresh() {
    try {
      const [w, s] = await Promise.all([api.listWorkspaces(), api.saasStats()]);
      setRows(w.workspaces);
      setStats(s);
    } catch (e) {
      setError(String(e));
    }
  }

  async function create() {
    if (!name.trim()) return;
    try {
      await api.createWorkspace(name.trim());
      setName('');
      setShowCreate(false);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  // `busy` keys off the workspace id so only the acting row's
  // buttons disable, not the whole table.
  async function act(w: WorkspaceRow, fn: (id: string) => Promise<void>) {
    setBusy(w.id);
    try {
      await fn(w.id);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  }

  async function destroy(w: WorkspaceRow) {
    if (
      !confirm(
        `Delete workspace "${w.name}"? All projects / events / issues CASCADE-deleted.`,
      )
    )
      return;
    await act(w, id => api.deleteWorkspace(id));
  }

  return (
    <div className="space-y-4">
      <PageHeader
        title={t('saas.title')}
        subtitle={t('saas.subtitle')}
        actions={
          <Button onClick={() => setShowCreate(true)}>{'+ ' + t('saas.newWorkspace')}</Button>
        }
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}
      {showCreate && (
        <Card>
          <CardHeader title={t('saas.create')} />
          <CardBody>
            <input
              className="h-8 w-full rounded border border-border px-2.5 text-sm"
              placeholder={t('saas.namePlaceholder')}
              value={name}
              onChange={e => setName(e.target.value)}
            />
            <div className="mt-2 flex gap-2">
              <Button onClick={create}>{t('action.create')}</Button>
              <Button variant="secondary" onClick={() => setShowCreate(false)}>{t('action.cancel')}</Button>
            </div>
          </CardBody>
        </Card>
      )}

      {stats && (
        <div className="grid grid-cols-6 gap-3">
          <StatCard label={t('saas.workspaces')} value={stats.workspaces} />
          <StatCard
            label={t('saas.active')}
            value={stats.active_workspaces}
            tone="ok"
          />
          <StatCard label={t('overview.projects')} value={stats.projects} />
          <StatCard label={t('saas.users')} value={stats.users} />
          <StatCard
            label={t('saas.events24h')}
            value={stats.events_24h ?? 0}
          />
          <StatCard
            label={t('saas.tokens')}
            value={stats.tokens_active ?? 0}
          />
        </div>
      )}

      <Card>
        <CardHeader title={`${t('saas.workspaces')} (${rows.length})`} />
        <CardBody>
          {loading ? (
            <div className="py-8 text-center text-sm text-fg-subtle">Loading…</div>
          ) : rows.length === 0 ? (
            <EmptyState
              title={t('saas.empty')}
              hint={t('saas.emptyHint')}
            />
          ) : (
            <DataTable
              columns={[
                { key: 'name', label: t('saas.name') },
                { key: 'plan', label: t('saas.plan') },
                { key: 'status', label: t('crash.status') },
                { key: 'projects', label: t('saas.projects') },
                { key: 'members', label: t('saas.members') },
                { key: 'created', label: t('saas.created') },
                { key: 'actions', label: '' },
              ]}
              rows={rows.map(w => ({
                key: w.id,
                name: (
                  <div>
                    <div className="font-medium">{w.name}</div>
                    <div className="font-mono text-xs text-fg-muted">
                      {w.id}
                    </div>
                  </div>
                ),
                plan: <Badge>{w.plan}</Badge>,
                status:
                  w.status === 'active' ? (
                    <Badge tone="ok">{w.status}</Badge>
                  ) : (
                    <Badge tone="neutral">{w.status}</Badge>
                  ),
                projects: formatNumber(w.project_count),
                members: formatNumber(w.member_count),
                created: formatRelative(w.created_at),
                actions: (
                  <div className="flex items-center gap-1">
                    <select
                      className="rounded border border-border-strong bg-surface px-1.5 py-1 text-xs text-fg"
                      value={w.plan}
                      disabled={busy === w.id}
                      onChange={e =>
                        act(w, id =>
                          api.saasSetPlan(
                            id,
                            e.target.value as 'free' | 'pro' | 'enterprise',
                          ),
                        )
                      }
                    >
                      <option value="free">free</option>
                      <option value="pro">pro</option>
                      <option value="enterprise">enterprise</option>
                    </select>
                    {w.status === 'active' ? (
                      <Button
                        size="sm"
                        variant="secondary"
                        disabled={busy === w.id}
                        onClick={() => act(w, id => api.suspendWorkspace(id))}
                      >{t('saas.suspend')}</Button>
                    ) : (
                      <Button
                        size="sm"
                        variant="secondary"
                        disabled={busy === w.id}
                        onClick={() => act(w, id => api.resumeWorkspace(id))}
                      >{t('saas.resume')}</Button>
                    )}
                    <Button
                      size="sm"
                      variant="danger"
                      disabled={busy === w.id}
                      onClick={() => destroy(w)}
                    >{t('action.delete')}</Button>
                  </div>
                ),
              }))}
            />
          )}
        </CardBody>
      </Card>
    </div>
  );
}

function StatCard({
  label,
  value,
  tone,
}: {
  label: string;
  value: number;
  tone?: 'ok';
}) {
  return (
    <Card>
      <div className="px-5 py-4">
        <p className="text-xs uppercase tracking-wide text-fg-subtle">
          {label}
        </p>
        <p
          className={`mt-1 text-2xl font-semibold ${tone === 'ok' ? 'text-ok' : 'text-fg'}`}
        >
          {formatNumber(value)}
        </p>
      </div>
    </Card>
  );
}
