// Per-project trace list. Each row is a trace (root span op +
// span count + duration + status). Click → drilldown to the
// span timeline (TraceDetail).

import { useEffect, useState } from 'react';
import { Link, useParams } from 'react-router-dom';

import { api, TraceRow } from '../lib/api';
import {
  Badge,
  Card,
  CardHeader,
  EmptyState,
  ErrorBanner,
  PageHeader,
  Section,
  formatNumber,
  formatRelative,
} from '../components/ui';

export default function Traces() {
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

  if (!projectId) return <ErrorBanner>Project id missing</ErrorBanner>;

  return (
    <div className="space-y-4">
      <PageHeader
        title="Traces"
        subtitle="Distributed tracing — last 100 traces by recency."
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}
      <Card>
        <CardHeader title={`Traces (${rows.length})`} />
        <Section>
          {loading ? (
            <div className="py-8 text-center text-sm text-zinc-500">
              Loading…
            </div>
          ) : rows.length === 0 ? (
            <EmptyState
              title="No traces yet"
              hint="SDKs call POST /v1/spans to send tracing data."
            />
          ) : (
            <ul className="divide-y divide-zinc-800">
              {rows.map(t => (
                <li key={t.trace_id} className="px-2 py-3">
                  <Link
                    to={`/projects/${projectId}/traces/${t.trace_id}`}
                    className="block hover:bg-zinc-900/40 -m-2 p-2 rounded"
                  >
                    <div className="flex items-center justify-between gap-3">
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <span className="font-mono text-xs text-emerald-400">
                            {t.root_op ?? '—'}
                          </span>
                          <span className="font-mono text-sm text-zinc-100">
                            {t.root_name ?? t.trace_id.slice(0, 8) + '…'}
                          </span>
                          <Badge
                            tone={t.status === 'ok' ? 'ok' : 'neutral'}
                          >
                            {t.status}
                          </Badge>
                        </div>
                        <div className="font-mono text-[10px] text-zinc-500 mt-1">
                          {t.trace_id}
                        </div>
                      </div>
                      <div className="text-right">
                        <div className="font-mono text-sm text-zinc-200 tabular-nums">
                          {formatNumber(t.duration_ms)}ms
                        </div>
                        <div className="font-mono text-[10px] text-zinc-500">
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
        </Section>
      </Card>
    </div>
  );
}
