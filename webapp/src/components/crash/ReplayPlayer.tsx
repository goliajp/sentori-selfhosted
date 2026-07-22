// The last sixty seconds before the crash, redrawn.
//
// What the SDK captures is not video — it is a wireframe: per tick,
// the rectangles that made up the screen, with text and fill where
// the native layer could read them. That is a deliberate trade. It
// costs a fraction of a screen recording, it survives a 60-second
// rolling buffer on a mid-range phone, and it cannot leak a password
// field the way a bitmap can.
//
// On the wire it is NDJSON: a keyframe listing every node, then
// deltas listing only what changed, with a fresh keyframe whenever a
// delta would have approached a rewrite. Reconstruction walks from
// the last keyframe at or before the playhead and applies deltas
// forward — the same shape a video codec uses, for the same reason.
//
// Nodes are matched across ticks by rounded geometry, so a node that
// moved arrives as a removal plus an addition, while one that merely
// changed its text keeps its identity.

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { useT } from '../../i18n';
import { api } from '../../lib/api';

type Node = {
  x: number;
  y: number;
  w: number;
  h: number;
  kind?: string;
  text?: string;
  color?: string;
};

type Frame =
  | { ts: number; kind: 'key'; width: number; height: number; nodes: Node[] }
  | {
      ts: number;
      kind: 'delta';
      added: Node[];
      changed: Node[];
      removed: Pick<Node, 'x' | 'y' | 'w' | 'h'>[];
    };

/**
 * NDJSON to frames.
 *
 * One malformed line should not cost the whole recording: the format is
 * append-only, so a truncated tail is the expected failure rather than
 * a corrupt file.
 */
function decodeFrames(text: string): Frame[] {
  const out: Frame[] = [];
  for (const line of text.split('\n')) {
    if (!line.trim()) continue;
    try {
      out.push(JSON.parse(line) as Frame);
    } catch {
      /* a partial last line is normal */
    }
  }
  return out;
}

const fp = (n: Pick<Node, 'x' | 'y' | 'w' | 'h'>) =>
  `${n.x | 0},${n.y | 0},${n.w | 0},${n.h | 0}`;

export function ReplayPlayer({
  projectId,
  attachmentRef,
  ndjson,
  onSeek,
}: {
  projectId?: string;
  attachmentRef?: string;
  /** The recording, already fetched. The replay page loads it by
   *  replay id rather than by attachment ref, and rebuilding the
   *  canvas a second time over there would be two players to keep in
   *  step with each other. */
  ndjson?: string;
  /** Playhead position, as a wall-clock ms, so the surrounding page
   *  can follow along — the breadcrumb timeline highlights in step. */
  onSeek?: (ts: number) => void;
}) {
  const t = useT();
  const [fetched, setFetched] = useState<Frame[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [index, setIndex] = useState(0);
  const [playing, setPlaying] = useState(false);
  const canvasRef = useRef<HTMLCanvasElement>(null);

  // Text that was handed to us needs no effect — it is already here, so
  // deriving is both simpler and correct. Setting state inside an
  // effect for a value available at render costs a second render pass
  // and is what `react-hooks/set-state-in-effect` is pointing at.
  const provided = useMemo(
    () => (ndjson === undefined ? null : decodeFrames(ndjson)),
    [ndjson],
  );
  const frames = provided ?? fetched;

  useEffect(() => {
    if (ndjson !== undefined) return;
    if (!projectId || !attachmentRef) return;
    let cancelled = false;
    fetch(api.attachmentUrl(projectId, attachmentRef), {
      credentials: 'include',
    })
      .then(r => {
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        return r.text();
      })
      .then(text => !cancelled && setFetched(decodeFrames(text)))
      .catch(e => !cancelled && setError(String(e)));
    return () => {
      cancelled = true;
    };
  }, [projectId, attachmentRef, ndjson]);

  /** Screen size comes from the most recent keyframe at or before the
   *  playhead — a rotation mid-recording changes it. */
  const { nodes, width, height } = useMemo(() => {
    if (!frames?.length) return { nodes: [], width: 0, height: 0 };
    const state = new Map<string, Node>();
    let w = 0;
    let h = 0;
    for (let i = 0; i <= Math.min(index, frames.length - 1); i++) {
      const f = frames[i];
      if (f.kind === 'key') {
        state.clear();
        for (const n of f.nodes) state.set(fp(n), n);
        w = f.width;
        h = f.height;
      } else {
        for (const n of f.removed) state.delete(fp(n));
        for (const n of f.added) state.set(fp(n), n);
        for (const n of f.changed) state.set(fp(n), n);
      }
    }
    return { nodes: [...state.values()], width: w, height: h };
  }, [frames, index]);

  useEffect(() => {
    if (onSeek && frames?.[index]) onSeek(frames[index].ts);
  }, [onSeek, frames, index]);

  // Playback steps frame-to-frame at the recording's own pacing,
  // clamped so a long idle gap doesn't stall the playhead.
  useEffect(() => {
    if (!playing || !frames?.length) return;
    const lastFrame = frames.length - 1;
    // Already parked on the final frame: schedule nothing. Stopping
    // here would mean a setState in the effect body, which cascades a
    // render; the timeout below owns that transition instead.
    if (index >= lastFrame) return;
    const gap = Math.min(
      2000,
      Math.max(60, frames[index + 1].ts - frames[index].ts),
    );
    const id = setTimeout(() => {
      const next = index + 1;
      setIndex(next);
      if (next >= lastFrame) setPlaying(false);
    }, gap);
    return () => clearTimeout(id);
  }, [playing, frames, index]);

  const draw = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas || !width || !height) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const scale = Math.min(canvas.width / width, canvas.height / height);
    const ox = (canvas.width - width * scale) / 2;
    const oy = (canvas.height - height * scale) / 2;

    const css = getComputedStyle(document.documentElement);
    const surface = css.getPropertyValue('--s-surface').trim() || '#18181b';
    const stroke = css.getPropertyValue('--s-border-strong').trim() || '#3f3f46';
    const ink = css.getPropertyValue('--s-fg-muted').trim() || '#a1a1aa';

    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.fillStyle = surface;
    ctx.fillRect(ox, oy, width * scale, height * scale);

    for (const n of nodes) {
      const x = ox + n.x * scale;
      const y = oy + n.y * scale;
      const w = n.w * scale;
      const h = n.h * scale;
      if (n.color) {
        ctx.fillStyle = n.color;
        ctx.fillRect(x, y, w, h);
      }
      ctx.strokeStyle = stroke;
      ctx.lineWidth = 1;
      ctx.strokeRect(x + 0.5, y + 0.5, w - 1, h - 1);
      if (n.text && h > 10) {
        ctx.fillStyle = ink;
        ctx.font = `${Math.max(9, Math.min(13, h * scale * 0.5))}px ui-monospace, monospace`;
        ctx.save();
        ctx.beginPath();
        ctx.rect(x, y, w, h);
        ctx.clip();
        ctx.fillText(n.text, x + 4, y + Math.min(h - 4, 13));
        ctx.restore();
      }
    }
  }, [nodes, width, height]);

  useEffect(draw, [draw]);

  if (error) {
    return (
      <p className="text-sm text-fg-subtle">
        {t('crash.loadFailed')} ({error}).
      </p>
    );
  }
  if (!frames) {
    return <p className="text-sm text-fg-subtle">{t('crash.loadingRecording')}</p>;
  }
  if (!frames.length) {
    return <p className="text-sm text-fg-subtle">{t('crash.emptyRecording')}</p>;
  }

  const last = frames.length - 1;
  const elapsed = ((frames[index].ts - frames[0].ts) / 1000).toFixed(1);
  const total = ((frames[last].ts - frames[0].ts) / 1000).toFixed(1);

  return (
    <div className="space-y-2">
      <div className="flex justify-center rounded border border-border bg-bg p-3">
        <canvas
          ref={canvasRef}
          width={360}
          height={640}
          className="h-auto max-h-[640px] w-full max-w-[360px]"
        />
      </div>
      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={() => {
            if (index >= last) setIndex(0);
            setPlaying(p => !p);
          }}
          className="w-16 shrink-0 rounded border border-border px-2 py-1 text-xs text-fg-muted transition hover:text-fg focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
        >
          {playing ? t('crash.pause') : t('crash.play')}
        </button>
        <input
          type="range"
          min={0}
          max={last}
          value={index}
          aria-label={t('crash.recordingPosition')}
          onChange={e => {
            setPlaying(false);
            setIndex(Number(e.target.value));
          }}
          className="h-1 flex-1 accent-accent"
        />
        <span className="w-24 shrink-0 text-right font-mono text-xs tabular-nums text-fg-subtle">
          {elapsed}s / {total}s
        </span>
      </div>
    </div>
  );
}
