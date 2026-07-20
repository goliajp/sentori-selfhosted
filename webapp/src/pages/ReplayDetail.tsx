// Replay scrubber — basic version. Loads NDJSON, decodes frame
// types, lets the user step through them with a slider. Full
// canvas/DOM replay player is K7-replay work (v0.3).

import { useEffect, useMemo, useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';

import { api } from '../lib/api';
import { useKeyHandlers } from '../lib/useShortcuts';
import {
  Badge,
  Button,
  Card,
  CardHeader,
  ErrorBanner,
  PageHeader,
  Section,
  formatNumber,
} from '../components/ui';

interface Frame {
  raw: string;
  ts?: number;
  kind?: string;
  parsed?: Record<string, unknown>;
}

export default function ReplayDetail() {
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
      <div className="py-16 text-center text-sm text-zinc-500">Loading…</div>
    );
  }
  if (error) return <ErrorBanner>{error}</ErrorBanner>;
  if (!frames.length) {
    return (
      <div className="space-y-4">
        <PageHeader title="Replay" subtitle={replayId} />
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
        title="Replay"
        subtitle={`${formatNumber(frames.length)} frames · ${replayId.slice(0, 16)}…`}
        actions={
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              variant="secondary"
              onClick={() => {
                navigator.clipboard?.writeText(window.location.href);
              }}
            >
              Copy link
            </Button>
            <Link
              to={`/projects/${projectId}/replays`}
              className="rounded border border-zinc-300 px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-50"
            >
              ← All replays
            </Link>
          </div>
        }
      />

      <Card>
        <CardHeader title="Scrubber" />
        <Section>
          <div className="space-y-3">
            <input
              type="range"
              min={0}
              max={frames.length - 1}
              value={idx}
              onChange={e => setIdx(parseInt(e.target.value, 10))}
              className="w-full"
            />
            <div className="flex items-center justify-between text-xs text-zinc-500">
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
                <span className="font-mono text-[10px] text-zinc-500">
                  ts {cur.ts}
                </span>
              )}
            </div>
          </div>
        </Section>
      </Card>

      <Card>
        <CardHeader title="Frame payload" />
        <Section>
          <pre className="overflow-x-auto whitespace-pre-wrap break-all bg-zinc-950 p-3 text-[11px] font-mono text-zinc-300">
            {cur.parsed
              ? JSON.stringify(cur.parsed, null, 2)
              : cur.raw}
          </pre>
        </Section>
      </Card>

      <p className="text-center text-[11px] text-zinc-500">
        Canvas / DOM replay player coming in v0.3. Today: scrub
        through raw frames + payload inspection.
      </p>
    </div>
  );
}
