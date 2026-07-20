// Cross-workspace view — only meaningful in SaaS deployment
// where one sentori-server instance fronts many workspaces.
// In self-hosted mode this just shows the single workspace row.

import { useEffect, useState } from 'react';

import { api, SaasStats, WorkspaceRow } from '../lib/api';
import {
  Badge,
  Card,
  CardHeader,
  DataTable,
  EmptyState,
  ErrorBanner,
  PageHeader,
  Section,
  formatNumber,
  formatRelative,
} from '../components/ui';

export default function SaasAdmin() {
  const [rows, setRows] = useState<WorkspaceRow[]>([]);
  const [stats, setStats] = useState<SaasStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([api.listWorkspaces(), api.saasStats()])
      .then(([w, s]) => {
        setRows(w.workspaces);
        setStats(s);
      })
      .catch(e => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  return (
    <div className="space-y-4">
      <PageHeader
        title="SaaS admin"
        subtitle="Cross-workspace operator view. In self-hosted mode shows your single workspace."
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {stats && (
        <div className="grid grid-cols-6 gap-3">
          <StatCard label="Workspaces" value={stats.workspaces} />
          <StatCard
            label="Active"
            value={stats.active_workspaces}
            tone="ok"
          />
          <StatCard label="Projects" value={stats.projects} />
          <StatCard label="Users" value={stats.users} />
          <StatCard
            label="Events 24h"
            value={stats.events_24h ?? 0}
          />
          <StatCard
            label="Tokens"
            value={stats.tokens_active ?? 0}
          />
        </div>
      )}

      <Card>
        <CardHeader title={`Workspaces (${rows.length})`} />
        <Section>
          {loading ? (
            <div className="py-8 text-center text-sm text-zinc-500">Loading…</div>
          ) : rows.length === 0 ? (
            <EmptyState
              title="No workspaces"
              hint="No workspaces have been provisioned yet."
            />
          ) : (
            <DataTable
              columns={[
                { key: 'name', label: 'Name' },
                { key: 'plan', label: 'Plan' },
                { key: 'status', label: 'Status' },
                { key: 'projects', label: 'Projects' },
                { key: 'members', label: 'Members' },
                { key: 'created', label: 'Created' },
              ]}
              rows={rows.map(w => ({
                key: w.id,
                name: (
                  <div>
                    <div className="font-medium">{w.name}</div>
                    <div className="font-mono text-[10px] text-zinc-400">
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
              }))}
            />
          )}
        </Section>
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
      <div className="p-4">
        <p className="text-[11px] uppercase tracking-wide text-zinc-500">
          {label}
        </p>
        <p
          className={`mt-1 text-2xl font-semibold ${tone === 'ok' ? 'text-emerald-600' : 'text-zinc-800'}`}
        >
          {formatNumber(value)}
        </p>
      </div>
    </Card>
  );
}
