// Single trace — meta + flat span list. Span tree visualizer
// (waterfall chart) is a future enhancement; v0.2 starts with
// indented list keyed by parent_span_id.

import { useEffect, useState } from 'react';
import { Link, useParams } from 'react-router-dom';

import { useNavigate } from 'react-router-dom';

import { api, SpanRow, TraceRow } from '../lib/api';
import { useKeyHandlers } from '../lib/useShortcuts';
import {
  Badge,
  Card,
  CardHeader,
  ErrorBanner,
  PageHeader,
  Section,
  formatNumber,
  formatRelative,
} from '../components/ui';

export default function TraceDetail() {
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
      <div className="py-16 text-center text-sm text-zinc-500">Loading…</div>
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
        subtitle={`Trace ${trace.trace_id}`}
        actions={
          <Link
            to={`/projects/${projectId}/traces`}
            className="rounded border border-zinc-300 px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-50"
          >
            ← All traces
          </Link>
        }
      />

      <Card>
        <CardHeader title="Meta" />
        <Section>
          <div className="grid grid-cols-4 gap-4">
            <Cell label="Status">
              <Badge tone={trace.status === 'ok' ? 'ok' : 'neutral'}>
                {trace.status}
              </Badge>
            </Cell>
            <Cell label="Root op">
              <span className="font-mono text-xs">{trace.root_op ?? '—'}</span>
            </Cell>
            <Cell label="Spans">{formatNumber(trace.span_count)}</Cell>
            <Cell label="Duration">
              {formatNumber(trace.duration_ms)} ms
            </Cell>
            <Cell label="First seen">{formatRelative(trace.first_seen)}</Cell>
            <Cell label="Last seen">{formatRelative(trace.last_seen)}</Cell>
          </div>
        </Section>
      </Card>

      <Card>
        <CardHeader title={`Spans (${spans.length})`} />
        <Section>
          {spans.length === 0 ? (
            <div className="py-8 text-center text-sm text-zinc-500">
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
        </Section>
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
      <p className="text-[10px] uppercase tracking-wide text-zinc-500">
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
      className="rounded border border-zinc-200 p-2 text-xs"
      style={{ marginLeft: `${depth * 16}px` }}
    >
      <button
        onClick={() => hasTags && setOpen(!open)}
        className={`flex w-full items-center justify-between gap-2 text-left ${hasTags ? '' : 'cursor-default'}`}
      >
        <div className="flex items-center gap-2 min-w-0">
          {hasTags && (
            <span className="font-mono text-[10px] text-zinc-500 w-3">
              {open ? '▼' : '▶'}
            </span>
          )}
          <Badge>{s.op}</Badge>
          <span className="font-mono text-[11px] text-zinc-200 truncate">
            {s.name}
          </span>
          <Badge tone={s.status === 'ok' ? 'ok' : 'neutral'}>
            {s.status}
          </Badge>
        </div>
        <span className="font-mono tabular-nums text-zinc-300">
          {s.duration_ms}ms
        </span>
      </button>
      <div className="mt-1 h-1.5 w-full rounded bg-zinc-100 relative overflow-hidden">
        <div
          className="absolute top-0 h-full rounded bg-emerald-500/70"
          style={{
            left: `${offsetPct.toFixed(2)}%`,
            width: `${widthPct.toFixed(2)}%`,
          }}
        />
      </div>
      {open && hasTags && (
        <pre className="mt-2 overflow-x-auto whitespace-pre-wrap break-all bg-zinc-950 p-2 text-[10px] font-mono text-zinc-300">
          {JSON.stringify(s.tags, null, 2)}
        </pre>
      )}
    </li>
  );
}

