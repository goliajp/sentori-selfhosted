// Per-project replay session list. Replay viewer (scrubber +
// frame playback) is future work — v0.2 just lists what's been
// captured so operators can confirm replays are landing.

import { useEffect, useState } from 'react';
import { Link, useParams } from 'react-router-dom';

import { api, ReplayRow } from '../lib/api';
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

export default function Replays() {
  const { id: projectId } = useParams<{ id: string }>();
  const [rows, setRows] = useState<ReplayRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!projectId) return;
    api
      .listReplays(projectId, 100)
      .then(r => setRows(r.replays))
      .catch(e => setError(String(e)))
      .finally(() => setLoading(false));
  }, [projectId]);

  if (!projectId) return <ErrorBanner>Project id missing</ErrorBanner>;

  return (
    <div className="space-y-4">
      <PageHeader
        title="Replays"
        subtitle="Session replays captured around error events. Last 100 by recency."
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}

      <Card>
        <CardHeader title={`Replays (${rows.length})`} />
        <Section>
          {loading ? (
            <div className="py-8 text-center text-sm text-zinc-500">
              Loading…
            </div>
          ) : rows.length === 0 ? (
            <EmptyState
              title="No replays yet"
              hint="SDKs capture replays automatically around captureException calls. Verify your SDK init has replay enabled."
            />
          ) : (
            <ul className="divide-y divide-zinc-800">
              {rows.map(r => (
                <li
                  key={r.id}
                  className="flex items-center justify-between gap-3 px-2 py-3"
                >
                  <Link
                    to={`/projects/${projectId}/replays/${r.id}`}
                    className="min-w-0 flex-1 block hover:bg-zinc-900/40 -m-2 p-2 rounded"
                  >
                    <div className="flex items-center gap-2">
                      <Badge>{(r.duration_ms / 1000).toFixed(1)}s</Badge>
                      <Badge tone="neutral">
                        {formatNumber(r.frame_count)} frames
                      </Badge>
                      <span className="font-mono text-[11px] text-emerald-400">
                        event {r.event_id.slice(0, 8)}…
                      </span>
                    </div>
                    <div className="font-mono text-[10px] text-zinc-500 mt-1">
                      blob {r.blob_hash.slice(0, 16)}… ·{' '}
                      {formatRelative(r.created_at)}
                    </div>
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </Section>
      </Card>

      <p className="text-center text-[11px] text-zinc-500">
        Replay viewer (scrubber + frame playback) coming in v0.3.
      </p>
    </div>
  );
}
