// DLQ + recent push triage. Status filter + per-row retry.

import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';

import { api } from '../lib/api';
import {
  Badge,
  Button,
  Card,
  CardHeader,
  ErrorBanner,
  PageHeader,
  Section,
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
  const { id: projectId } = useParams<{ id: string }>();
  const [rows, setRows] = useState<Row[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState<'' | 'queued' | 'sent' | 'failed'>(
    '',
  );

  async function refresh() {
    if (!projectId) return;
    try {
      const r = await api.listPushSends(projectId, {
        status: filter || undefined,
        limit: 100,
      });
      setRows(r.sends);
    } catch (e) {
      setError(String(e));
    }
  }

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projectId, filter]);

  async function retry(id: string) {
    if (!projectId) return;
    try {
      await api.retryPushSend(projectId, id);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function retryAllFailed() {
    if (!projectId) return;
    if (!confirm(`Re-queue all ${counts.failed} failed sends?`)) return;
    try {
      await api.retryAllFailedPushSends(projectId);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  if (!projectId) return <ErrorBanner>Project id missing</ErrorBanner>;

  const counts = {
    queued: rows.filter(r => r.status === 'queued').length,
    sent: rows.filter(r => r.status === 'sent').length,
    failed: rows.filter(r => r.status === 'failed').length,
  };

  return (
    <div className="space-y-4">
      <PageHeader
        title="Push sends"
        subtitle="Last 100 push attempts + retry-now for the DLQ."
        actions={
          counts.failed > 0 && (
            <Button size="sm" onClick={retryAllFailed}>
              Retry all {counts.failed} failed
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
                ? 'bg-emerald-600 text-white'
                : 'bg-zinc-800 text-zinc-300 hover:bg-zinc-700'
            }`}
          >
            {s === '' ? 'All' : s}
            {s && (
              <span className="ml-1 font-mono text-zinc-400">
                ({counts[s as keyof typeof counts]})
              </span>
            )}
          </button>
        ))}
      </div>

      <Card>
        <CardHeader title={`Sends (${rows.length})`} />
        <Section>
          {rows.length === 0 ? (
            <div className="py-8 text-center text-sm text-zinc-500">
              No push sends yet.
            </div>
          ) : (
            <ul className="divide-y divide-zinc-800">
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
                        <span className="font-mono text-[10px] text-zinc-500">
                          retry {r.retry_count}
                        </span>
                      )}
                    </div>
                    <div className="mt-1 text-[10px] text-zinc-500">
                      {r.error ||
                        r.provider_outcome ||
                        formatRelative(r.created_at)}
                    </div>
                    {r.next_attempt_at && r.status === 'queued' && (
                      <div className="text-[10px] text-zinc-500">
                        next attempt {formatRelative(r.next_attempt_at)}
                      </div>
                    )}
                  </div>
                  {r.status === 'failed' && (
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() => retry(r.id)}
                    >
                      Retry now
                    </Button>
                  )}
                </li>
              ))}
            </ul>
          )}
        </Section>
      </Card>
    </div>
  );
}
