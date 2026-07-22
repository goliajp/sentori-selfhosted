// Per-project integrations admin — Slack / Linear / Jira / GitHub / GitLab.

import { useState } from 'react';
import { useParams } from 'react-router-dom';

import { useT } from '../i18n';
import { api, IntegrationRow } from '../lib/api';
import { useAsyncData } from '../lib/useAsyncData';
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
  formatRelative,
} from '../components/ui';

const KINDS = ['slack', 'linear', 'jira', 'github', 'gitlab'] as const;

export default function Integrations() {
  const t = useT();
  const { id: projectId } = useParams<{ id: string }>();
  const [showAdd, setShowAdd] = useState(false);
  const [kind, setKind] = useState<(typeof KINDS)[number]>('slack');
  const [config, setConfig] = useState('{}');

  const {
    data,
    loading,
    error,
    reload: refresh,
    setError,
  } = useAsyncData(
    async (): Promise<IntegrationRow[]> =>
      projectId ? (await api.listIntegrations(projectId)).integrations : [],
    [projectId],
    String,
  );
  const rows = data ?? [];

  async function upsert() {
    if (!projectId) return;
    let parsed: unknown;
    try {
      parsed = JSON.parse(config);
    } catch {
      setError(t('common.jsonInvalid'));
      return;
    }
    try {
      await api.upsertIntegration(projectId, { kind, config: parsed });
      setConfig('{}');
      setShowAdd(false);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function toggle(it: IntegrationRow) {
    if (!projectId) return;
    try {
      await api.setIntegrationActive(projectId, it.kind, !it.active);
      refresh();
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
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  if (!projectId) {
    return <ErrorBanner>{t('common.missingProjectId')}</ErrorBanner>;
  }

  return (
    <div className="space-y-4">
      <PageHeader
        title={t('integrations.title')}
        subtitle={t('integrations.subtitle')}
        actions={<Button onClick={() => setShowAdd(true)}>{'+ ' + t('integrations.connectShort')}</Button>}
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {showAdd && (
        <Card>
          <CardHeader title={t('integrations.connect')} />
          <CardBody>
            <label className="block text-xs text-fg-subtle mb-1">Provider</label>
            <select
              className="h-8 w-full rounded border border-border px-2.5 text-sm"
              value={kind}
              onChange={e => setKind(e.target.value as (typeof KINDS)[number])}
            >
              {KINDS.map(k => (
                <option key={k} value={k}>
                  {k}
                </option>
              ))}
            </select>
            <label className="mt-2 block text-xs text-fg-subtle mb-1">
              Config (JSON — webhook URL, OAuth tokens, team id, etc.)
            </label>
            <textarea
              className="w-full h-40 rounded border border-border px-3 py-2 text-xs font-mono"
              value={config}
              onChange={e => setConfig(e.target.value)}
            />
            <div className="mt-2 flex gap-2">
              <Button onClick={upsert}>{t('action.save')}</Button>
              <Button variant="secondary" onClick={() => setShowAdd(false)}>{t('action.cancel')}</Button>
            </div>
          </CardBody>
        </Card>
      )}

      <Card>
        <CardHeader title={`${t('common.configured')} (${rows.length})`} />
        <CardBody>
          {loading ? (
            <div className="py-8 text-center text-sm text-fg-subtle">Loading…</div>
          ) : rows.length === 0 ? (
            <EmptyState
              title={t('integrations.empty')}
              hint={t('integrations.emptyHint')}
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
                      {it.active ? t('action.pause') : t('saas.resume')}
                    </Button>
                    <Button size="sm" variant="danger" onClick={() => destroy(it)}>{t('action.disconnect')}</Button>
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
