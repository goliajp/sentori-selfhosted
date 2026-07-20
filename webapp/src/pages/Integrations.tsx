// Per-project integrations admin — Slack / Linear / Jira / GitHub / GitLab.

import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';

import { api, IntegrationRow } from '../lib/api';
import {
  Badge,
  Button,
  Card,
  CardHeader,
  DataTable,
  EmptyState,
  ErrorBanner,
  PageHeader,
  Section,
  formatRelative,
} from '../components/ui';

const KINDS = ['slack', 'linear', 'jira', 'github', 'gitlab'] as const;

export default function Integrations() {
  const { id: projectId } = useParams<{ id: string }>();
  const [rows, setRows] = useState<IntegrationRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showAdd, setShowAdd] = useState(false);
  const [kind, setKind] = useState<(typeof KINDS)[number]>('slack');
  const [config, setConfig] = useState('{}');

  async function refresh() {
    if (!projectId) return;
    setLoading(true);
    setError(null);
    try {
      const r = await api.listIntegrations(projectId);
      setRows(r.integrations);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }
  useEffect(() => {
    refresh();
  }, [projectId]);

  async function upsert() {
    if (!projectId) return;
    let parsed: unknown;
    try {
      parsed = JSON.parse(config);
    } catch {
      setError('Config must be valid JSON');
      return;
    }
    try {
      await api.upsertIntegration(projectId, { kind, config: parsed });
      setConfig('{}');
      setShowAdd(false);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function toggle(it: IntegrationRow) {
    if (!projectId) return;
    try {
      await api.setIntegrationActive(projectId, it.kind, !it.active);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function destroy(it: IntegrationRow) {
    if (!projectId) return;
    if (!confirm(`Disconnect ${it.kind}? Pending dispatches will fail.`))
      return;
    try {
      await api.deleteIntegration(projectId, it.kind);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  if (!projectId) {
    return <ErrorBanner>Project id missing</ErrorBanner>;
  }

  return (
    <div className="space-y-4">
      <PageHeader
        title="Integrations"
        subtitle="External services that receive sentori events: Slack notifications, Linear issue creation, GitHub mentions, etc."
        actions={<Button onClick={() => setShowAdd(true)}>+ Connect</Button>}
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {showAdd && (
        <Card>
          <CardHeader title="Connect integration" />
          <Section>
            <label className="block text-xs text-zinc-500 mb-1">Provider</label>
            <select
              className="w-full rounded border border-zinc-300 px-3 py-2 text-sm"
              value={kind}
              onChange={e => setKind(e.target.value as (typeof KINDS)[number])}
            >
              {KINDS.map(k => (
                <option key={k} value={k}>
                  {k}
                </option>
              ))}
            </select>
            <label className="mt-2 block text-xs text-zinc-500 mb-1">
              Config (JSON — webhook URL, OAuth tokens, team id, etc.)
            </label>
            <textarea
              className="w-full h-40 rounded border border-zinc-300 px-3 py-2 text-xs font-mono"
              value={config}
              onChange={e => setConfig(e.target.value)}
            />
            <div className="mt-2 flex gap-2">
              <Button onClick={upsert}>Save</Button>
              <Button variant="secondary" onClick={() => setShowAdd(false)}>
                Cancel
              </Button>
            </div>
          </Section>
        </Card>
      )}

      <Card>
        <CardHeader title={`Configured (${rows.length})`} />
        <Section>
          {loading ? (
            <div className="py-8 text-center text-sm text-zinc-500">Loading…</div>
          ) : rows.length === 0 ? (
            <EmptyState
              title="No integrations"
              hint="Connect Slack or Linear to forward sentori events into your workflows."
            />
          ) : (
            <DataTable
              columns={[
                { key: 'kind', label: 'Provider' },
                { key: 'active', label: 'Active' },
                { key: 'connected', label: 'Connected' },
                { key: 'actions', label: '' },
              ]}
              rows={rows.map(it => ({
                key: it.id,
                kind: <Badge>{it.kind}</Badge>,
                active: it.active ? (
                  <Badge tone="ok">on</Badge>
                ) : (
                  <Badge tone="neutral">off</Badge>
                ),
                connected: formatRelative(it.connected_at),
                actions: (
                  <div className="flex gap-1">
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() => toggle(it)}
                    >
                      {it.active ? 'Pause' : 'Resume'}
                    </Button>
                    <Button size="sm" variant="danger" onClick={() => destroy(it)}>
                      Disconnect
                    </Button>
                  </div>
                ),
              }))}
            />
          )}
        </Section>
      </Card>
    </div>
  );
}
