// Per-project metrics list. Click a metric → expand inline
// timeseries sparkline (24h minute rollup by default).

import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';

import { api, MetricPoint, MetricSummary } from '../lib/api';
import { Sparkline } from '../components/Sparkline';
import {
  Card,
  CardHeader,
  EmptyState,
  ErrorBanner,
  PageHeader,
  Section,
  formatNumber,
  formatRelative,
} from '../components/ui';

export default function Metrics() {
  const { id: projectId } = useParams<{ id: string }>();
  const [rows, setRows] = useState<MetricSummary[]>([]);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [series, setSeries] = useState<Record<string, MetricPoint[]>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!projectId) return;
    api
      .listMetrics(projectId)
      .then(r => setRows(r.metrics))
      .catch(e => setError(String(e)))
      .finally(() => setLoading(false));
  }, [projectId]);

  async function expand(name: string) {
    if (!projectId) return;
    if (expanded === name) {
      setExpanded(null);
      return;
    }
    setExpanded(name);
    if (!series[name]) {
      try {
        const r = await api.metricsTimeseries(projectId, name, 24);
        setSeries(s => ({ ...s, [name]: r.points }));
      } catch (e) {
        setError(String(e));
      }
    }
  }

  if (!projectId) return <ErrorBanner>Project id missing</ErrorBanner>;

  return (
    <div className="space-y-4">
      <PageHeader
        title="Metrics"
        subtitle="Custom counters / gauges / distributions emitted via SDK metrics:batch. Last 24h shown by default."
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}

      <Card>
        <CardHeader title={`Active metrics (${rows.length})`} />
        <Section>
          {loading ? (
            <div className="py-8 text-center text-sm text-zinc-500">
              Loading…
            </div>
          ) : rows.length === 0 ? (
            <EmptyState
              title="No metrics yet"
              hint="SDKs call POST /v1/metrics:batch to send rolled-up samples."
            />
          ) : (
            <ul className="divide-y divide-zinc-800">
              {rows.map(m => {
                const isExpanded = expanded === m.name;
                const points = series[m.name];
                const sparkValues = points?.map(p => p.sum / Math.max(p.count, 1)) ?? [];
                return (
                  <li key={m.name}>
                    <button
                      onClick={() => expand(m.name)}
                      className="flex w-full items-center justify-between gap-3 px-2 py-3 text-left hover:bg-zinc-900/40 rounded"
                    >
                      <div className="flex items-center gap-3 min-w-0">
                        <span className="font-mono text-xs text-zinc-500 w-4">
                          {isExpanded ? '▼' : '▶'}
                        </span>
                        <span className="font-mono text-sm text-zinc-100">
                          {m.name}
                        </span>
                      </div>
                      <div className="flex items-center gap-4">
                        <div className="text-right">
                          <p className="font-mono text-xs text-zinc-400">
                            24h count
                          </p>
                          <p className="font-mono text-sm text-zinc-200 tabular-nums">
                            {formatNumber(m.total_count)}
                          </p>
                        </div>
                        <div className="text-right">
                          <p className="font-mono text-xs text-zinc-400">avg</p>
                          <p className="font-mono text-sm text-zinc-200 tabular-nums">
                            {m.avg_value.toFixed(2)}
                          </p>
                        </div>
                        <span className="text-xs text-zinc-500 w-24 text-right">
                          {m.last_bucket
                            ? formatRelative(m.last_bucket)
                            : '—'}
                        </span>
                      </div>
                    </button>
                    {isExpanded && (
                      <div className="bg-zinc-950 px-12 py-3">
                        {points ? (
                          points.length === 0 ? (
                            <p className="text-xs text-zinc-500">
                              No samples in the last 24h.
                            </p>
                          ) : (
                            <>
                              <Sparkline
                                values={sparkValues}
                                width={600}
                                height={64}
                              />
                              <div className="mt-2 flex gap-6 text-xs text-zinc-500">
                                <span>
                                  buckets: {formatNumber(points.length)}
                                </span>
                                <span>
                                  min:{' '}
                                  {Math.min(...sparkValues).toFixed(2)}
                                </span>
                                <span>
                                  max:{' '}
                                  {Math.max(...sparkValues).toFixed(2)}
                                </span>
                              </div>
                            </>
                          )
                        ) : (
                          <p className="text-xs text-zinc-500">Loading…</p>
                        )}
                      </div>
                    )}
                  </li>
                );
              })}
            </ul>
          )}
        </Section>
      </Card>
    </div>
  );
}
