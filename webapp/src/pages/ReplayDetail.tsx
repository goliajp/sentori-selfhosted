// A recording on its own page: the same canvas player the crash view
// embeds, plus the decoded frame under the playhead for anyone who
// needs to see what the SDK actually sent.
//
// This used to be a slider over raw JSON with a footnote saying a real
// player was coming in v0.3. The player shipped with the crash
// evidence view; this page just hadn't been told.

import { useEffect, useMemo, useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';

import { useT } from '../i18n';
import { api } from '../lib/api';
import { ReplayPlayer } from '../components/crash/ReplayPlayer';
import { useKeyHandlers } from '../lib/useShortcuts';
import {
  Badge,
  Button,
  Card,
  CardBody,
  CardHeader,
  ErrorBanner,
  PageHeader,
  formatNumber,
} from '../components/ui';

interface Frame {
  raw: string;
  ts?: number;
  kind?: string;
  parsed?: Record<string, unknown>;
}

export default function ReplayDetail() {
  const t = useT();
  const { id: projectId, replayId } = useParams<{
    id: string;
    replayId: string;
  }>();
  const [text, setText] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [idx, setIdx] = useState(0);
  const navigate = useNavigate();
  useKeyHandlers({
    Escape: () => projectId && navigate(`/projects/${projectId}/replays`),
    ArrowLeft: () => setIdx(i => Math.max(0, i - 1)),
    ArrowRight: () => setIdx(i => i + 1),
  });

  useEffect(() => {
    if (!projectId || !replayId) return;
    api
      .replayNdjson(projectId, replayId)
      .then(t => setText(t))
      .catch(e => setError(String(e)))
      .finally(() => setLoading(false));
  }, [projectId, replayId]);

  const frames: Frame[] = useMemo(() => {
    if (!text) return [];
    return text
      .split('\n')
      .filter(line => line.trim().length > 0)
      .map(raw => {
        try {
          const obj = JSON.parse(raw) as Record<string, unknown>;
          return {
            raw,
            ts:
              typeof obj.ts === 'number'
                ? obj.ts
                : typeof obj.timestamp === 'number'
                  ? obj.timestamp
                  : undefined,
            kind:
              typeof obj.kind === 'string'
                ? obj.kind
                : typeof obj.type === 'string'
                  ? obj.type
                  : 'unknown',
            parsed: obj,
          };
        } catch {
          return { raw };
        }
      });
  }, [text]);

  if (!projectId || !replayId) {
    return <ErrorBanner>Missing project / replay id</ErrorBanner>;
  }
  if (loading) {
    return (
      <div className="py-16 text-center text-sm text-fg-subtle">Loading…</div>
    );
  }
  if (error) return <ErrorBanner>{error}</ErrorBanner>;
  if (!frames.length) {
    return (
      <div className="space-y-4">
        <PageHeader title={t('replays.detail')} subtitle={replayId} />
        <ErrorBanner>Empty NDJSON blob</ErrorBanner>
      </div>
    );
  }

  const cur = frames[Math.min(idx, frames.length - 1)];
  const firstTs = frames.find(f => f.ts !== undefined)?.ts;
  const offsetMs =
    cur.ts !== undefined && firstTs !== undefined ? cur.ts - firstTs : null;

  return (
    <div className="space-y-4">
      <PageHeader
        title={t('replays.detail')}
        subtitle={t('replays.frames').replace('{n}', formatNumber(frames.length))}
        actions={
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              variant="secondary"
              onClick={() => {
                navigator.clipboard?.writeText(window.location.href);
              }}
            >
              {t('crash.copyLink')}
            </Button>
            <Link
              to={`/projects/${projectId}/replays`}
              className="inline-flex h-8 items-center rounded border border-border px-3 text-sm text-fg-subtle hover:bg-raised"
            >
              {t('replays.allReplays')}
            </Link>
          </div>
        }
      />

      {/* The player owns playback; the scrubber below it stays because
          a frame-by-frame view is what you want when the question is
          "what did the SDK send", not "what did the user see". */}
      <Card>
        <CardHeader title={t('replays.detail')} />
        <CardBody>
          <ReplayPlayer ndjson={text ?? undefined} />
        </CardBody>
      </Card>

      <Card>
        <CardHeader title={t('replays.scrubber')} />
        <CardBody>
          <div className="space-y-3">
            <input
              type="range"
              min={0}
              max={frames.length - 1}
              value={idx}
              onChange={e => setIdx(parseInt(e.target.value, 10))}
              className="w-full"
            />
            <div className="flex items-center justify-between text-xs text-fg-subtle">
              <span>
                frame {idx + 1} / {frames.length}
              </span>
              <span>
                {offsetMs !== null ? `+${offsetMs}ms from start` : 'no timestamp'}
              </span>
            </div>
            <div className="flex items-center gap-2">
              <Badge>{cur.kind ?? 'unknown'}</Badge>
              {cur.ts !== undefined && (
                <span className="font-mono text-xs text-fg-subtle">
                  ts {cur.ts}
                </span>
              )}
            </div>
          </div>
        </CardBody>
      </Card>

      <Card>
        <CardHeader title={t('replays.framePayload')} />
        <CardBody>
          <pre className="overflow-x-auto whitespace-pre-wrap break-all bg-bg p-3 text-xs font-mono text-fg-muted">
            {cur.parsed
              ? JSON.stringify(cur.parsed, null, 2)
              : cur.raw}
          </pre>
        </CardBody>
      </Card>

    </div>
  );
}
