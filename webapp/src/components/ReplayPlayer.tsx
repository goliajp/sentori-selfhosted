// The visual replay player — the minute before the event, as the
// user saw it.
//
// Mobile frames are portrait, so the player is a portrait dock: a
// phone-shaped viewport that fills its column instead of a letter-
// boxed landscape strip. The `screens` attachment is NDJSON: one
// low-bitrate frame per line, `t` in seconds relative to the event
// (negative). Playback runs at 4× real time (frames arrive ~2.5 s
// apart; a minute of context should take fifteen seconds to review,
// not sixty). Scrubbing pauses; ←/→ step frames while focused.

import { Pause, Play } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';

import { useT } from '../i18n';
import { api } from '../lib/api';

type ReplayFrame = {
  t: number;
  mediaType: string;
  base64: string;
  /** Window logical size (pt/dp) — present from SDK native ≥ 5.4;
   *  the tap markers need it to map coordinates onto the JPEG. */
  w?: number;
  h?: number;
};

/** NDJSON to frames, one line at a time. The upload is append-only,
 *  so a half-written last line is the expected damage — and the
 *  moment it is most likely is the one this player exists for: the
 *  process died mid-flush. Mapping `JSON.parse` over the lines threw
 *  the whole recording away for one bad byte, and the page said the
 *  replay failed to load. The wireframe decoder has always been
 *  per-line tolerant; this one now matches it. */
function decodeFrames(text: string): ReplayFrame[] {
  const out: ReplayFrame[] = [];
  for (const line of text.split('\n')) {
    if (!line.trim()) continue;
    try {
      const f = JSON.parse(line) as ReplayFrame;
      if (typeof f.base64 === 'string') out.push(f);
    } catch {
      /* a partial last line is normal */
    }
  }
  return out;
}

const PLAYBACK_SPEED = 4;

export function ReplayPlayer({
  attachmentRef,
  seek,
  onFrames,
  onTime,
  taps,
}: {
  attachmentRef: string;
  /** Timeline → replay: jump to the frame nearest this moment
   *  (seconds relative to the event, negative). `n` re-arms the
   *  same instant twice in a row. */
  seek?: { t: number; n: number } | null;
  /** Replay → timeline: the loaded frame moments (relative s). */
  onFrames?: (times: number[]) => void;
  /** Replay → page: the playhead moment (relative s), on every
   *  frame step — the signal list highlights along. */
  onTime?: (t: number) => void;
  /** Tap moments with logical-pt coordinates (SDK ≥ 5.3): drawn as
   *  rings over the frame near their moment — needs frames that
   *  carry the window size (native ≥ 5.4), else nothing is drawn. */
  taps?: { t: number; x: number; y: number }[];
}) {
  const t = useT();
  const [frames, setFrames] = useState<null | ReplayFrame[]>(null);
  const [failed, setFailed] = useState(false);
  const [idx, setIdx] = useState(0);
  const [playing, setPlaying] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    let alive = true;
    api
      .fetchAttachmentText(attachmentRef)
      .then((text) => {
        if (!alive) return;
        const parsed = decodeFrames(text);
        setFrames(parsed);
        setIdx(Math.max(0, parsed.length - 1));
      })
      .catch(() => {
        if (alive) setFailed(true);
      });
    return () => {
      alive = false;
    };
  }, [attachmentRef]);

  // Report loaded frame moments upward. A separate effect, NOT the
  // fetch closure: the player can mount via the occurrence-fallback
  // race with `onFrames` still undefined, and the fetch effect never
  // re-runs for a prop identity change.
  useEffect(() => {
    if (frames) onFrames?.(frames.map((f) => f.t));
  }, [frames, onFrames]);

  // Playhead position, up to the page (drives the signal-list
  // highlight). Fires at frame pacing, ~2–4 Hz.
  useEffect(() => {
    if (frames && frames.length > 0 && onTime) {
      onTime(frames[Math.min(idx, frames.length - 1)]!.t);
    }
  }, [frames, idx, onTime]);

  // Advance on a per-frame timer scaled by the real inter-frame gap.
  // The end-of-strip stop happens inside the timer callback (never
  // synchronously in the effect body — react-hooks/set-state-in-effect).
  useEffect(() => {
    if (!playing || !frames || frames.length === 0 || idx >= frames.length - 1) return;
    const gapMs = Math.max(
      100,
      ((frames[idx + 1]!.t - frames[idx]!.t) * 1000) / PLAYBACK_SPEED,
    );
    timerRef.current = setTimeout(() => {
      setIdx((i) => {
        const next = Math.min(i + 1, frames.length - 1);
        if (next >= frames.length - 1) setPlaying(false);
        return next;
      });
    }, gapMs);
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [playing, idx, frames]);

  // Seek applies during render (the adjust-state-from-props pattern)
  // — an effect would be a cascading second render for no gain.
  const [appliedSeek, setAppliedSeek] = useState(0);
  if (seek && frames && frames.length > 0 && seek.n !== appliedSeek) {
    setAppliedSeek(seek.n);
    setPlaying(false);
    let best = 0;
    let bestGap = Infinity;
    frames.forEach((f, i) => {
      const gap = Math.abs(f.t - seek.t);
      if (gap < bestGap) {
        bestGap = gap;
        best = i;
      }
    });
    setIdx(best);
  }

  const current = frames?.[idx];
  const src = useMemo(
    () => (current ? `data:${current.mediaType};base64,${current.base64}` : null),
    [current],
  );

  if (failed) {
    return <p className="text-sm text-fg-subtle">{t('replay.loadFailed')}</p>;
  }
  if (!frames) {
    return <p className="text-sm text-fg-subtle">{t('shell.loading')}</p>;
  }
  if (frames.length === 0) {
    return <p className="text-sm text-fg-subtle">{t('replay.empty')}</p>;
  }

  return (
    <div
      className="overflow-hidden"
      tabIndex={0}
      role="group"
      aria-label={t('replay.title')}
      onKeyDown={(e) => {
        if (e.key === 'ArrowRight') {
          e.preventDefault();
          setPlaying(false);
          setIdx((i) => Math.min(i + 1, frames.length - 1));
        } else if (e.key === 'ArrowLeft') {
          e.preventDefault();
          setPlaying(false);
          setIdx((i) => Math.max(i - 1, 0));
        } else if (e.key === ' ') {
          e.preventDefault();
          setPlaying((p) => !p);
        }
      }}
    >
      {/* Square viewport: frames are usually portrait, but a rotated
          phone or a tablet sends landscape — a square is the one
          shape that letterboxes both gracefully instead of betting
          on an orientation. */}
      {/* Always a bounded square: below the xl split the panel goes
          full-width and an uncapped aspect-square balloons into a
          viewport-sized block. */}
      <div className="mx-auto flex aspect-square w-full max-w-[440px] items-center justify-center bg-bg p-3">
        {src && (
          // relative wrapper shrink-wraps the img, so percentage
          // positions inside it ARE positions on the frame
          <span className="relative inline-flex max-h-full max-w-full">
            <img
              src={src}
              alt={t('replay.frameAlt', { t: current!.t.toFixed(1) })}
              className="max-h-full max-w-full rounded-sm border border-border object-contain"
            />
            {typeof current!.w === 'number' &&
              typeof current!.h === 'number' &&
              (taps ?? [])
                .filter((tap) => Math.abs(tap.t - current!.t) <= 1.5)
                .map((tap, i) => (
                  <span
                    key={i}
                    aria-hidden
                    className="absolute h-5 w-5 -translate-x-1/2 -translate-y-1/2 rounded-full border-2"
                    style={{
                      left: `${(tap.x / current!.w!) * 100}%`,
                      top: `${(tap.y / current!.h!) * 100}%`,
                      borderColor: 'var(--s-kind-warn)',
                      backgroundColor:
                        'color-mix(in srgb, var(--s-kind-warn) 30%, transparent)',
                    }}
                  />
                ))}
          </span>
        )}
      </div>
      <div className="flex items-center gap-2.5 border-t border-border px-3 py-2">
        <button
          type="button"
          onClick={() => {
            if (!playing && idx >= frames.length - 1) setIdx(0);
            setPlaying((p) => !p);
          }}
          aria-label={playing ? t('replay.pause') : t('replay.play')}
          className="flex h-7 w-7 shrink-0 items-center justify-center rounded border border-border-strong text-fg hover:bg-raised"
        >
          {playing ? (
            <Pause aria-hidden className="h-3.5 w-3.5" />
          ) : (
            <Play aria-hidden className="h-3.5 w-3.5" />
          )}
        </button>
        <input
          type="range"
          min={0}
          max={frames.length - 1}
          value={idx}
          onChange={(e) => {
            setPlaying(false);
            setIdx(Number(e.target.value));
          }}
          aria-label={t('replay.scrubber')}
          className="min-w-0 flex-1 accent-accent"
        />
        <span className="shrink-0 text-right font-mono text-xs tabular-nums text-fg-muted">
          {current!.t.toFixed(1)}s · {idx + 1}/{frames.length}
        </span>
      </div>
    </div>
  );
}
