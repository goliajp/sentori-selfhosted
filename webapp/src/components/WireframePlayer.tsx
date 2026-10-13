import { Button } from '../../shared/design/controls';
import { ReplayControls } from '../../shared/replay/Controls';
// The wireframe replay — the minute before the event, redrawn.
//
// What the SDK captures here is not video: per tick, the rectangles
// that made up the screen, with text and fill where the native
// layer could read them. It costs a fraction of a screen recording
// and cannot leak a password field the way a bitmap can — which is
// why it is always on, while pixel capture (`replayScreens`) is
// opt-in. This player renders it when an event carries no pixels.
//
// On the wire it is NDJSON: a keyframe listing every node, then
// deltas listing only what changed. Reconstruction walks from the
// last keyframe and applies deltas forward — the same shape a video
// codec uses, for the same reason. The viewport is square, like the
// visual player's: portrait and landscape recordings both letterbox
// inside the canvas.

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { useT } from '../i18n';
import { api } from '../lib/api';

import { decodeWireframes, type ReplayNode as Node, type Wireframe as Frame } from '../lib/replay-wireframe';
const fp = (n: Pick<Node, 'x' | 'y' | 'w' | 'h'>) =>
  `${n.x | 0},${n.y | 0},${n.w | 0},${n.h | 0}`;

type Entry = { id: string; node: Node };

/** Ids for a keyframe's nodes: geometry + occurrence, the same
 *  derivation the SDK uses — so SDK-minted delta ids land on the
 *  entries a keyframe created. */
function indexKeyNodes(nodes: Node[]): Entry[] {
  const seen = new Map<string, number>();
  return nodes.map((node) => {
    const base = fp(node);
    const occ = seen.get(base) ?? 0;
    seen.set(base, occ + 1);
    return { id: `${base}#${occ}`, node };
  });
}

/** Pre-5.1.4 recordings carry no ids — fall back to the first entry
 *  with matching geometry (the old, ambiguous behaviour, kept only
 *  for old data). */
function findLegacy(order: Entry[], n: Pick<Node, 'x' | 'y' | 'w' | 'h'>): number {
  const base = fp(n);
  return order.findIndex((e) => e.id.startsWith(`${base}#`));
}

const CANVAS_PX = 640;

export function WireframePlayer({
  attachmentRef,
  eventAt,
  seek,
  onFrames,
  onTime,
  taps,
}: {
  attachmentRef: string;
  /** Event device timestamp, in milliseconds. Capture can end before it. */
  eventAt: number;
  /** Timeline → replay: jump to the frame nearest this moment
   *  (seconds relative to the event, negative). Frame timestamps
   *  are absolute ms and are anchored to the actual event timestamp. */
  seek?: { t: number; n: number } | null;
  /** Replay → timeline: the loaded frame moments (relative s). */
  onFrames?: (times: number[]) => void;
  /** Replay → page: the playhead moment (relative s), on every
   *  frame step — the signal list highlights along. */
  onTime?: (t: number) => void;
  /** Tap moments with logical-pt coordinates (SDK ≥ 5.3): drawn as
   *  rings on the wireframe near their moment, so "the user tapped
   *  HERE" is visible instead of an internal node number. */
  taps?: { t: number; x: number; y: number }[];
}) {
  const t = useT();
  const [frames, setFrames] = useState<Frame[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [attempt,setAttempt] = useState(0);
  const [index, setIndex] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState('1');
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    let alive = true;
    api
      .fetchAttachmentText(attachmentRef)
      .then((text) => {
        if (!alive) return;
        const decoded = decodeWireframes(text);
        setFrames(decoded);
        setIndex(Math.max(0, decoded.length - 1));
      })
      .catch(() => {
        if (alive) setFailed(true);
      });
    return () => {
      alive = false;
    };
  }, [attachmentRef,attempt]);

  // Report loaded frame moments upward relative to the actual event. A separate effect, NOT the fetch
  // closure — see ReplayPlayer for the fallback-race rationale.
  useEffect(() => {
    if (frames && frames.length > 0) {
      const last = eventAt;
      onFrames?.(frames.map((f) => (f.ts - last) / 1000));
    }
  }, [frames, onFrames, eventAt]);

  // Playhead position, up to the page (drives the signal-list
  // highlight). Cheap: fires at frame pacing, ~2–4 Hz.
  useEffect(() => {
    if (frames && frames.length > 0 && onTime) {
      const lastTs = eventAt;
      onTime((frames[Math.min(index, frames.length - 1)]!.ts - lastTs) / 1000);
    }
  }, [frames, index, onTime, eventAt]);

  // Seek applies during render (the adjust-state-from-props pattern)
  // — an effect would be a cascading second render for no gain.
  const [appliedSeek, setAppliedSeek] = useState(0);
  if (seek && frames && frames.length > 0 && seek.n !== appliedSeek) {
    setAppliedSeek(seek.n);
    setPlaying(false);
    const eventTs = eventAt;
    const target = eventTs + seek.t * 1000;
    let best = 0;
    let bestGap = Infinity;
    frames.forEach((f, i) => {
      const gap = Math.abs(f.ts - target);
      if (gap < bestGap) {
        bestGap = gap;
        best = i;
      }
    });
    setIndex(best);
  }

  /** Screen size comes from the most recent keyframe at or before the
   *  playhead — a rotation mid-recording changes it.
   *
   *  Reconstruction keeps an ORDERED entry list, never a
   *  geometry-keyed Map: a page root and the fullscreen overlay
   *  covering it share x/y/w/h, and a Map collapsed them into one
   *  node at the wrong z-position — the replay drew the page
   *  through its own opaque overlay (insight 2026-08-01). The
   *  keyframe's array order IS the native paint order; deltas
   *  locate entries by the SDK-minted id (geometry#occurrence). */
  const { nodes, width, height } = useMemo(() => {
    if (!frames?.length) return { nodes: [] as Node[], width: 0, height: 0 };
    let order: Entry[] = [];
    let w = 0;
    let h = 0;
    for (let i = 0; i <= Math.min(index, frames.length - 1); i++) {
      const f = frames[i]!;
      if (f.kind === 'key') {
        order = indexKeyNodes(f.nodes);
        w = f.width;
        h = f.height;
      } else {
        for (const n of f.removed) {
          const at = n.id ? order.findIndex((e) => e.id === n.id) : findLegacy(order, n);
          if (at >= 0) order.splice(at, 1);
        }
        for (const n of f.changed) {
          const at = n.id ? order.findIndex((e) => e.id === n.id) : findLegacy(order, n);
          if (at >= 0) order[at] = { id: order[at]!.id, node: n };
        }
        for (const n of f.added) {
          const id = n.id ?? `${fp(n)}#new${i}`;
          const existing = order.findIndex((e) => e.id === id);
          if (existing >= 0) {
            order[existing] = { id, node: n };
          } else if (typeof n.at === 'number' && n.at >= 0 && n.at <= order.length) {
            order.splice(n.at, 0, { id, node: n });
          } else {
            order.push({ id, node: n });
          }
        }
      }
    }
    return { nodes: order.map((e) => e.node), width: w, height: h };
  }, [frames, index]);

  // Playback steps frame-to-frame at the recording's own pacing,
  // scaled by the chosen playback speed.
  useEffect(() => {
    if (!playing || !frames?.length) return;
    const last = frames.length - 1;
    if (index >= last) return;
    const gap = Math.max(16, (frames[index + 1]!.ts - frames[index]!.ts) / Number(speed));
    const id = setTimeout(() => {
      const next = index + 1;
      setIndex(next);
      if (next >= last) setPlaying(false);
    }, gap);
    return () => clearTimeout(id);
  }, [playing, frames, index, speed]);

  const draw = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas || !width || !height) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const scale = Math.min(canvas.width / width, canvas.height / height);
    const ox = (canvas.width - width * scale) / 2;
    const oy = (canvas.height - height * scale) / 2;

    const css = getComputedStyle(document.documentElement);
    const surface = css.getPropertyValue('--ui-surface').trim();
    const outline = css.getPropertyValue('--ui-border').trim();
    const ink = css.getPropertyValue('--ui-muted').trim();

    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.fillStyle = surface;
    ctx.fillRect(ox, oy, width * scale, height * scale);

    // Everything draws inside the device rectangle. SDKs before
    // 5.1.3 report scroll content at its full unclipped size (a
    // 2000pt ruler arrives as 2000pt), so the clip is what keeps
    // historical recordings inside the phone.
    ctx.save();
    ctx.beginPath();
    ctx.rect(ox, oy, width * scale, height * scale);
    ctx.clip();

    // Solid fills only — per-node strokes turned a busy screen into
    // a grid of borders. Layers separate by luminance, the way a
    // squinted-at screenshot would; an image is a flat neutral
    // block, not a hollow frame.
    for (const n of nodes) {
      const x = ox + n.x * scale;
      const y = oy + n.y * scale;
      const w = n.w * scale;
      const h = n.h * scale;
      if (n.color) {
        ctx.fillStyle = n.color;
        ctx.fillRect(x, y, w, h);
      } else if (n.kind === 'image') {
        ctx.fillStyle = css.getPropertyValue('--ui-replay-placeholder').trim();
        ctx.fillRect(x, y, w, h);
      }
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
    ctx.restore();

    // One outline for the device itself, so the phone still reads as
    // an object against the canvas.
    ctx.strokeStyle = outline;
    ctx.lineWidth = 1;
    ctx.strokeRect(ox + 0.5, oy + 0.5, width * scale - 1, height * scale - 1);

    // Tap markers (SDK ≥ 5.3 sends pageX/pageY in the same logical-pt
    // space as the wireframe): a ring at the touch point, visible for
    // ~1.2s of recording time around the tap and fading out.
    if (taps?.length && frames?.length) {
      const tapInk = css.getPropertyValue('--ui-warning').trim();
      const playheadTs = frames[Math.min(index, frames.length - 1)]!.ts;
      const eventTs = eventAt;
      for (const tap of taps) {
        const tapTs = eventTs + tap.t * 1000;
        const age = playheadTs - tapTs;
        if (age < -400 || age > 1200) continue;
        const fade = age <= 0 ? 1 : 1 - age / 1200;
        const x = ox + tap.x * scale;
        const y = oy + tap.y * scale;
        ctx.save();
        ctx.globalAlpha = 0.35 * fade;
        ctx.beginPath();
        ctx.arc(x, y, 9, 0, Math.PI * 2);
        ctx.fillStyle = tapInk;
        ctx.fill();
        ctx.globalAlpha = 0.9 * fade;
        ctx.beginPath();
        ctx.arc(x, y, 9 + (1 - fade) * 6, 0, Math.PI * 2);
        ctx.lineWidth = 2;
        ctx.strokeStyle = tapInk;
        ctx.stroke();
        ctx.restore();
      }
    }
  }, [nodes, width, height, taps, frames, index, eventAt]);

  useEffect(() => {
    draw();
    const observer = new MutationObserver(draw);
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
    return () => observer.disconnect();
  }, [draw]);

  if (failed) {
    return <div className="p-5" role="alert"><p>{t('replay.loadFailed')}</p><Button className="mt-3" onClick={()=>{setFailed(false);setFrames(null);setAttempt(n=>n+1);}}>{t('common.retry')}</Button></div>;
  }
  if (!frames) {
    return <p className="text-sm text-fg-subtle">{t('shell.loading')}</p>;
  }
  if (frames.length === 0) {
    return <p className="text-sm text-fg-subtle">{t('replay.empty')}</p>;
  }

  const last = frames.length - 1;

  return (
    <div
      className="replay-surface p-5"
      tabIndex={0}
      role="group"
      aria-label={t('replay.title')}
      onKeyDown={(e) => {
        if (e.target !== e.currentTarget) return;
        if (e.key === 'ArrowRight') {
          e.preventDefault();
          setPlaying(false);
          setIndex((i) => Math.min(i + 1, last));
        } else if (e.key === 'ArrowLeft') {
          e.preventDefault();
          setPlaying(false);
          setIndex((i) => Math.max(i - 1, 0));
        } else if (e.key === ' ') {
          e.preventDefault();
          if (last > 0) {
            if (!playing && index >= last) setIndex(0);
            setPlaying(!playing);
          }
        }
      }}
    >
      {/* Always a bounded square: below the xl split the panel goes
          full-width and an uncapped aspect-square balloons into a
          viewport-sized block. */}
      <div className="replay-stage">
        <canvas
          ref={canvasRef}
          width={CANVAS_PX}
          height={CANVAS_PX}
          className="h-full w-full"
        />
      </div>
      <ReplayControls index={index} last={last} playing={playing} speed={speed} onSpeed={setSpeed}
        time={`${((frames[index]!.ts-eventAt)/1000).toFixed(1)}s · ${index+1}/${frames.length}`}
        onIndex={value=>{setPlaying(false);setIndex(value);}}
        onToggle={()=>{if(!playing&&index>=last)setIndex(0);setPlaying(!playing);}}
        labels={{play:t('replay.play'),pause:t('replay.pause'),back:t('replay.stepBack'),forward:t('replay.stepForward'),seek:t('replay.scrubber'),speed:t('replay.speed')}}/>
    </div>
  );
}
