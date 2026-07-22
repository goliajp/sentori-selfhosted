// Per-project trace list. Each row is a trace (root span op +
// span count + duration + status). Click → drilldown to the
// span timeline (TraceDetail).

import { useEffect, useState } from 'react';
import { Link, useParams } from 'react-router-dom';

import { useT } from '../i18n';
import { api, TraceRow } from '../lib/api';
import {
  Badge,
  Card,
  CardBody,
  CardHeader,
  EmptyState,
  ErrorBanner,
  PageHeader,
  formatNumber,
  formatRelative,
} from '../components/ui';

export default function Traces() {
  const t = useT();
  const { id: projectId } = useParams<{ id: string }>();
  const [rows, setRows] = useState<TraceRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!projectId) return;
    api
      .listTraces(projectId, 100)
      .then(r => setRows(r.traces))
      .catch(e => setError(String(e)))
      .finally(() => setLoading(false));
  }, [projectId]);

  if (!projectId) return <ErrorBanner>{t('common.missingProjectId')}</ErrorBanner>;

  return (
    <div className="space-y-4">
      <PageHeader
        title={t('traces.title')}
        subtitle={t('traces.subtitle')}
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}
      <Card>
        <CardHeader title={`${t('traces.title')} (${rows.length})`} />
        <CardBody>
          {loading ? (
            <div className="py-8 text-center text-sm text-fg-subtle">
              Loading…
            </div>
          ) : rows.length === 0 ? (
            <EmptyState
              title={t('traces.empty')}
              hint={t('traces.emptyHint')}
            />
          ) : (
            <ul className="divide-y divide-border">
              {rows.map(t => (
                <li key={t.trace_id} className="px-2 py-3">
                  <Link
                    to={`/projects/${projectId}/traces/${t.trace_id}`}
                    className="block hover:bg-surface/40 -m-2 p-2 rounded"
                  >
                    <div className="flex items-center justify-between gap-3">
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <span className="font-mono text-xs text-accent">
                            {t.root_op ?? '—'}
                          </span>
                          <span className="font-mono text-sm text-fg">
                            {t.root_name ?? t.trace_id.slice(0, 8) + '…'}
                          </span>
                          <Badge
                            tone={t.status === 'ok' ? 'ok' : 'neutral'}
                          >
                            {t.status}
                          </Badge>
                        </div>
                        <div className="font-mono text-xs text-fg-subtle mt-1">
                          {t.trace_id}
                        </div>
                      </div>
                      <div className="text-right">
                        <div className="font-mono text-sm text-fg tabular-nums">
                          {formatNumber(t.duration_ms)}ms
                        </div>
                        <div className="font-mono text-xs text-fg-subtle">
                          {t.span_count} span{t.span_count === 1 ? '' : 's'} ·{' '}
                          {formatRelative(t.last_seen)}
                        </div>
                      </div>
                    </div>
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </CardBody>
      </Card>
    </div>
  );
}
