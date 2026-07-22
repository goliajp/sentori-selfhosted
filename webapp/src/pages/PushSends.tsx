// DLQ + recent push triage. Status filter + per-row retry.

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
  ErrorBanner,
  PageHeader,
  formatRelative,
} from '../components/ui';

interface Row {
  id: string;
  provider: string;
  status: string;
  provider_outcome: string | null;
  error: string | null;
  retry_count: number;
  created_at: string;
  sent_at: string | null;
  next_attempt_at: string | null;
}

export default function PushSends() {
  const t = useT();
  const { id: projectId } = useParams<{ id: string }>();
  const [filter, setFilter] = useState<'' | 'queued' | 'sent' | 'failed'>(
    '',
  );

  const {
    data,
    error,
    reload: refresh,
    setError,
  } = useAsyncData(
    async (): Promise<Row[]> =>
      projectId
        ? (
            await api.listPushSends(projectId, {
              status: filter || undefined,
              limit: 100,
            })
          ).sends
        : [],
    [projectId, filter],
    String,
  );
  const rows = data ?? [];

  async function retry(id: string) {
    if (!projectId) return;
    try {
      await api.retryPushSend(projectId, id);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function retryAllFailed() {
    if (!projectId) return;
    if (!confirm(`Re-queue all ${counts.failed} failed sends?`)) return;
    try {
      await api.retryAllFailedPushSends(projectId);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  if (!projectId) return <ErrorBanner>{t('common.missingProjectId')}</ErrorBanner>;

  const counts = {
    queued: rows.filter(r => r.status === 'queued').length,
    sent: rows.filter(r => r.status === 'sent').length,
    failed: rows.filter(r => r.status === 'failed').length,
  };

  return (
    <div className="space-y-4">
      <PageHeader
        title={t('push.sends')}
        subtitle={t('push.sendsSubtitle')}
        actions={
          counts.failed > 0 && (
            <Button size="sm" onClick={retryAllFailed}>
              {t('push.retryAllFailed').replace('{n}', String(counts.failed))}
            </Button>
          )
        }
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}

      <div className="flex gap-2 text-xs">
        {(['', 'queued', 'sent', 'failed'] as const).map(s => (
          <button
            key={s || 'all'}
            onClick={() => setFilter(s)}
            className={`rounded px-3 py-1 ${
              filter === s
                ? 'bg-accent text-white'
                : 'bg-raised text-fg-muted hover:bg-raised'
            }`}
          >
            {s === '' ? 'All' : s}
            {s && (
              <span className="ml-1 font-mono text-fg-muted">
                ({counts[s as keyof typeof counts]})
              </span>
            )}
          </button>
        ))}
      </div>

      <Card>
        <CardHeader title={`${t('push.sendsShort')} (${rows.length})`} />
        <CardBody>
          {rows.length === 0 ? (
            <div className="py-8 text-center text-sm text-fg-subtle">
              No push sends yet.
            </div>
          ) : (
            <ul className="divide-y divide-border">
              {rows.map(r => (
                <li
                  key={r.id}
                  className="flex items-center justify-between gap-3 px-2 py-3"
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <Badge>{r.provider}</Badge>
                      {r.status === 'sent' ? (
                        <Badge tone="ok">{r.status}</Badge>
                      ) : r.status === 'failed' ? (
                        <Badge tone="neutral">{r.status}</Badge>
                      ) : (
                        <Badge>{r.status}</Badge>
                      )}
                      {r.retry_count > 0 && (
                        <span className="font-mono text-xs text-fg-subtle">
                          retry {r.retry_count}
                        </span>
                      )}
                    </div>
                    <div className="mt-1 text-xs text-fg-subtle">
                      {r.error ||
                        r.provider_outcome ||
                        formatRelative(r.created_at)}
                    </div>
                    {r.next_attempt_at && r.status === 'queued' && (
                      <div className="text-xs text-fg-subtle">
                        next attempt {formatRelative(r.next_attempt_at)}
                      </div>
                    )}
                  </div>
                  {r.status === 'failed' && (
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() => retry(r.id)}
                    >{t('action.retryNow')}</Button>
                  )}
                </li>
              ))}
            </ul>
          )}
        </CardBody>
      </Card>
    </div>
  );
}
