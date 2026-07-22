// Per-project replay session list. Replay viewer (scrubber +
// frame playback) is future work — v0.2 just lists what's been
// captured so operators can confirm replays are landing.

import { useEffect, useState } from 'react';
import { Link, useParams } from 'react-router-dom';

import { useT } from '../i18n';
import { api, ReplayRow } from '../lib/api';
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

export default function Replays() {
  const t = useT();
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

  if (!projectId) return <ErrorBanner>{t('common.missingProjectId')}</ErrorBanner>;

  return (
    <div className="space-y-4">
      <PageHeader
        title={t('replays.title')}
        subtitle={t('replays.subtitle')}
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}

      <Card>
        <CardHeader title={`${t('replays.title')} (${rows.length})`} />
        <CardBody>
          {loading ? (
            <div className="py-8 text-center text-sm text-fg-subtle">
              Loading…
            </div>
          ) : rows.length === 0 ? (
            <EmptyState
              title={t('replays.empty')}
              hint={t('replays.emptyHint')}
            />
          ) : (
            <ul className="divide-y divide-border">
              {rows.map(r => (
                <li
                  key={r.id}
                  className="flex items-center justify-between gap-3 px-2 py-3"
                >
                  <Link
                    to={`/projects/${projectId}/replays/${r.id}`}
                    className="min-w-0 flex-1 block hover:bg-surface/40 -m-2 p-2 rounded"
                  >
                    <div className="flex items-center gap-2">
                      <Badge>{(r.duration_ms / 1000).toFixed(1)}s</Badge>
                      <Badge tone="neutral">
                        {t('replays.frames').replace(
                          '{n}',
                          formatNumber(r.frame_count),
                        )}
                      </Badge>
                      <span className="font-mono text-xs text-accent">
                        {t('replays.event')} {r.event_id.slice(0, 8)}…
                      </span>
                    </div>
                    <div className="font-mono text-xs text-fg-subtle mt-1">
                      {t('replays.blob')} {r.blob_hash.slice(0, 16)}… ·{' '}
                      {formatRelative(r.created_at)}
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
