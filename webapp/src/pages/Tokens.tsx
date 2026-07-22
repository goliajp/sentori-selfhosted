// Token management page — mint / list / revoke SDK ingest tokens.
//
// This is the new-customer onboarding step that produces the
// `st_pk_<26 base32>` string they paste into SDK init().

import { useState } from 'react';
import { useParams } from 'react-router-dom';

import { useT } from '../i18n';
import { api } from '../lib/api';
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

export default function Tokens() {
  const t = useT();
  const { id: projectId } = useParams<{ id: string }>();
  const [showCreate, setShowCreate] = useState(false);
  const [label, setLabel] = useState('');
  const [newToken, setNewToken] = useState<string | null>(null);

  const {
    data,
    loading,
    error,
    reload: refresh,
    setError,
  } = useAsyncData(
    async () => (projectId ? (await api.listTokens(projectId)).tokens : []),
    [projectId],
    String,
  );
  const rows = data ?? [];

  async function mint() {
    if (!projectId) return;
    try {
      const r = await api.mintToken(projectId, {
        label: label.trim() || undefined,
        kind: 'public',
      });
      setNewToken(r.token);
      setLabel('');
      setShowCreate(false);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function revoke(id: string) {
    if (!confirm(t('tokens.confirmRevoke')))
      return;
    try {
      await api.revokeToken(id);
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
        title={t('tokens.title')}
        subtitle={t('tokens.subtitle')}
        actions={
          <Button onClick={() => setShowCreate(true)}>{'+ ' + t('tokens.mintShort')}</Button>
        }
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}
      {newToken && (
        <Card>
          <CardHeader title={t('tokens.new')} />
          <CardBody>
            <pre className="overflow-x-auto whitespace-pre-wrap break-all bg-raised p-3 text-xs font-mono">
              {newToken}
            </pre>
            <div className="text-xs text-fg-subtle mt-2">
              Copy this now — it won't be shown again. Plaintext lives only in
              your dashboard session.
            </div>
            <div className="mt-2 flex gap-2">
              <Button
                onClick={() => {
                  navigator.clipboard?.writeText(newToken);
                }}
              >{t('action.copy')}</Button>
              <Button
                variant="secondary"
                onClick={() => setNewToken(null)}
              >{t('action.done')}</Button>
            </div>
          </CardBody>
        </Card>
      )}
      {showCreate && (
        <Card>
          <CardHeader title={t('tokens.mint')} />
          <CardBody>
            <input
              className="h-8 w-full rounded border border-border px-2.5 text-sm"
              placeholder={t('tokens.labelHint')}
              value={label}
              onChange={e => setLabel(e.target.value)}
            />
            <div className="mt-2 flex gap-2">
              <Button onClick={mint}>{t('action.create')}</Button>
              <Button
                variant="secondary"
                onClick={() => setShowCreate(false)}
              >{t('action.cancel')}</Button>
            </div>
          </CardBody>
        </Card>
      )}
      <Quickstart projectId={projectId} token={newToken} />
      <Card>
        <CardHeader title={`${t('tokens.title')} (${rows.length})`} />
        <CardBody>
          {loading ? (
            <div className="py-8 text-center text-sm text-fg-subtle">
              Loading…
            </div>
          ) : rows.length === 0 ? (
            <EmptyState
              title={t('tokens.empty')}
              hint={t('tokens.emptyHint')}
            />
          ) : (
            <DataTable
              columns={[
                { key: 'label', label: 'Label' },
                { key: 'kind', label: 'Kind' },
                { key: 'last4', label: 'Token …' },
                { key: 'created', label: 'Created' },
                { key: 'status', label: 'Status' },
                { key: 'actions', label: '' },
              ]}
              rows={rows.map(tok => ({
                key: tok.id,
                label: tok.label || '(unlabelled)',
                kind: <Badge>{tok.kind}</Badge>,
                last4: tok.last4 ? `…${tok.last4}` : '—',
                created: formatRelative(tok.created_at),
                status: tok.revoked_at ? (
                  <Badge tone="neutral">revoked</Badge>
                ) : (
                  <Badge tone="ok">active</Badge>
                ),
                actions: !tok.revoked_at && (
                  <Button
                    variant="danger"
                    size="sm"
                    onClick={() => revoke(tok.id)}
                  >{t('action.revoke')}</Button>
                ),
              }))}
            />
          )}
        </CardBody>
      </Card>
    </div>
  );
}

// The SaaS ingest host is the SDK's built-in default `ingestUrl`;
// it never appears in the dashboard otherwise, so surface it here.
const DEFAULT_INGEST_URL = 'https://ingest.sentori.golia.jp';

function Quickstart({
  projectId,
  token,
}: {
  projectId: string;
  token: string | null;
}) {
  const t = useT();
  const tk = token ?? 'st_pk_<your project token>';
  const snippet = `import { sentori } from '@goliapkg/sentori-react-native';

sentori.init({
  token: '${tk}',
  release: 'myapp@1.0.0+1',
  ingestUrl: '${DEFAULT_INGEST_URL}', // optional — this is the default
});`;
  return (
    <Card>
      <CardHeader
        title={t('tokens.quickstart')}
        subtitle={t('tokens.quickstartHint')}
      />
      <CardBody>
        <div className="mb-3 grid gap-3 sm:grid-cols-2">
          <Field label={t('tokens.ingestUrl')} value={DEFAULT_INGEST_URL} />
          <Field label={t('tokens.projectId')} value={projectId} mono />
        </div>
        <div className="relative">
          <pre className="overflow-x-auto rounded bg-bg p-3 text-xs font-mono text-fg">
            {snippet}
          </pre>
          <div className="absolute right-2 top-2">
            <Button
              size="sm"
              variant="secondary"
              onClick={() => navigator.clipboard?.writeText(snippet)}
            >{t('action.copy')}</Button>
          </div>
        </div>
        <p className="mt-2 text-xs text-fg-subtle">
          {token
            ? t('tokens.justMinted')
            : t('tokens.otherFrameworks')}
        </p>
      </CardBody>
    </Card>
  );
}

function Field({
  label,
  value,
  mono,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div className="rounded border border-border bg-surface px-3 py-2">
      <p className="text-xs uppercase tracking-wide text-fg-subtle">
        {label}
      </p>
      <div className="flex items-center justify-between gap-2">
        <span
          className={`truncate text-xs text-fg ${mono ? 'font-mono' : ''}`}
        >
          {value}
        </span>
        <button
          onClick={() => navigator.clipboard?.writeText(value)}
          className="shrink-0 text-xs text-fg-subtle hover:text-fg-muted"
        >
          copy
        </button>
      </div>
    </div>
  );
}
