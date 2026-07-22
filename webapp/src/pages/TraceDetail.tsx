// Single trace — meta + flat span list. Span tree visualizer
// (waterfall chart) is a future enhancement; v0.2 starts with
// indented list keyed by parent_span_id.

import { useEffect, useState } from 'react';
import { Link, useParams } from 'react-router-dom';

import { useNavigate } from 'react-router-dom';

import { useT } from '../i18n';
import { api, SpanRow, TraceRow } from '../lib/api';
import { useKeyHandlers } from '../lib/useShortcuts';
import {
  Badge,
  Card,
  CardBody,
  CardHeader,
  ErrorBanner,
  PageHeader,
  formatNumber,
  formatRelative,
} from '../components/ui';

export default function TraceDetail() {
  const t = useT();
  const { id: projectId, traceId } = useParams<{
    id: string;
    traceId: string;
  }>();
  const [trace, setTrace] = useState<TraceRow | null>(null);
  const [spans, setSpans] = useState<SpanRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const navigate = useNavigate();
  useKeyHandlers({
    Escape: () => projectId && navigate(`/projects/${projectId}/traces`),
  });

  useEffect(() => {
    if (!projectId || !traceId) return;
    api
      .getTrace(projectId, traceId)
      .then(r => {
        setTrace(r.trace);
        setSpans(r.spans);
      })
      .catch(e => setError(String(e)))
      .finally(() => setLoading(false));
  }, [projectId, traceId]);

  if (!projectId || !traceId) {
    return <ErrorBanner>Missing project / trace id</ErrorBanner>;
  }
  if (loading) {
    return (
      <div className="py-16 text-center text-sm text-fg-subtle">Loading…</div>
    );
  }
  if (error) return <ErrorBanner>{error}</ErrorBanner>;
  if (!trace) return <ErrorBanner>Trace not found</ErrorBanner>;

  // Build depth map from parent_span_id chains so the flat list
  // can be visually nested without recursion.
  const depthFor: Map<string, number> = new Map();
  for (const s of spans) {
    if (!s.parent_span_id) {
      depthFor.set(s.id, 0);
    } else {
      const pd = depthFor.get(s.parent_span_id) ?? 0;
      depthFor.set(s.id, pd + 1);
    }
  }

  // Compute total wall-clock span for the waterfall bar.
  const startMin = spans.length
    ? Math.min(
        ...spans.map(s => new Date(s.started_at).getTime()),
      )
    : 0;
  const endMax = spans.length
    ? Math.max(
        ...spans.map(
          s => new Date(s.started_at).getTime() + s.duration_ms,
        ),
      )
    : 0;
  const totalMs = Math.max(1, endMax - startMin);

  return (
    <div className="space-y-4">
      <PageHeader
        title={trace.root_name ?? trace.trace_id.slice(0, 16) + '…'}
        subtitle={`${t('traces.trace')} ${trace.trace_id}`}
        actions={
          <Link
            to={`/projects/${projectId}/traces`}
            className="inline-flex h-8 items-center rounded border border-border px-3 text-sm text-fg-subtle hover:bg-raised"
          >
            ← All traces
          </Link>
        }
      />

      <Card>
        <CardHeader title={t('crash.meta')} />
        <CardBody>
          <div className="grid grid-cols-4 gap-4">
            <Cell label={t('crash.status')}>
              <Badge tone={trace.status === 'ok' ? 'ok' : 'neutral'}>
                {trace.status}
              </Badge>
            </Cell>
            <Cell label={t('traces.rootOp')}>
              <span className="font-mono text-xs">{trace.root_op ?? '—'}</span>
            </Cell>
            <Cell label={t('overview.spans')}>{formatNumber(trace.span_count)}</Cell>
            <Cell label={t('traces.duration')}>
              {formatNumber(trace.duration_ms)} ms
            </Cell>
            <Cell label={t('crash.firstSeen')}>{formatRelative(trace.first_seen)}</Cell>
            <Cell label={t('crash.lastSeen')}>{formatRelative(trace.last_seen)}</Cell>
          </div>
        </CardBody>
      </Card>

      <Card>
        <CardHeader title={`${t('traces.spans')} (${spans.length})`} />
        <CardBody>
          {spans.length === 0 ? (
            <div className="py-8 text-center text-sm text-fg-subtle">
              No spans recorded.
            </div>
          ) : (
            <ul className="space-y-1">
              {spans.map(s => (
                <SpanRowItem
                  key={s.id}
                  span={s}
                  depth={depthFor.get(s.id) ?? 0}
                  startMin={startMin}
                  totalMs={totalMs}
                />
              ))}
            </ul>
          )}
        </CardBody>
      </Card>
    </div>
  );
}

function Cell({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <p className="text-xs uppercase tracking-wide text-fg-subtle">
        {label}
      </p>
      <div className="mt-1 text-sm">{children}</div>
    </div>
  );
}

function SpanRowItem({
  span: s,
  depth,
  startMin,
  totalMs,
}: {
  span: SpanRow;
  depth: number;
  startMin: number;
  totalMs: number;
}) {
  const [open, setOpen] = useState(false);
  const startedMs = new Date(s.started_at).getTime();
  const offsetPct = ((startedMs - startMin) / totalMs) * 100;
  const widthPct = Math.max(0.3, (s.duration_ms / totalMs) * 100);
  const hasTags = s.tags && typeof s.tags === 'object'
    ? Object.keys(s.tags as object).length > 0
    : false;

  return (
    <li
      className="rounded border border-border p-2 text-xs"
      style={{ marginLeft: `${depth * 16}px` }}
    >
      <button
        onClick={() => hasTags && setOpen(!open)}
        className={`flex w-full items-center justify-between gap-2 text-left ${hasTags ? '' : 'cursor-default'}`}
      >
        <div className="flex items-center gap-2 min-w-0">
          {hasTags && (
            <span className="font-mono text-xs text-fg-subtle w-3">
              {open ? '▼' : '▶'}
            </span>
          )}
          <Badge>{s.op}</Badge>
          <span className="font-mono text-xs text-fg truncate">
            {s.name}
          </span>
          <Badge tone={s.status === 'ok' ? 'ok' : 'neutral'}>
            {s.status}
          </Badge>
        </div>
        <span className="font-mono tabular-nums text-fg-muted">
          {s.duration_ms}ms
        </span>
      </button>
      <div className="mt-1 h-1.5 w-full rounded bg-raised relative overflow-hidden">
        <div
          className="absolute top-0 h-full rounded bg-accent/70"
          style={{
            left: `${offsetPct.toFixed(2)}%`,
            width: `${widthPct.toFixed(2)}%`,
          }}
        />
      </div>
      {open && hasTags && (
        <pre className="mt-2 overflow-x-auto whitespace-pre-wrap break-all bg-bg p-2 text-xs font-mono text-fg-muted">
          {JSON.stringify(s.tags, null, 2)}
        </pre>
      )}
    </li>
  );
}

