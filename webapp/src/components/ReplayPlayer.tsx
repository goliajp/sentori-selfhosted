import { Button } from '../../shared/design/controls';
import { fitReplayViewport } from '../../shared/replay/viewport';
import { ReplayControls } from '../../shared/replay/Controls';
// The visual replay player — the minute before the event, as the
// user saw it.
//
// Mobile frames are portrait, so the player is a portrait dock: a
// phone-shaped viewport that fills its column instead of a letter-
// boxed landscape strip. The `screens` attachment is NDJSON: one
// low-bitrate frame per line, `t` in seconds relative to the event
// (negative). Playback follows the recorded inter-frame gaps with a selectable
// speed, defaulting to real time. Scrubbing pauses; ←/→ step frames while focused.

import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';

import { useT } from '../i18n';
import { api } from '../lib/api';

import { decodeScreenFrames, type ReplayFrame } from '../lib/replay-screens';

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
  const [attempt,setAttempt] = useState(0);
  const [idx, setIdx] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState('1');
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    let alive = true;
    api
      .fetchAttachmentText(attachmentRef)
      .then((text) => {
        if (!alive) return;
        const parsed = decodeScreenFrames(text);
        setFrames(parsed);
        setIdx(Math.max(0, parsed.length - 1));
      })
      .catch(() => {
        if (alive) setFailed(true);
      });
    return () => {
      alive = false;
    };
  }, [attachmentRef,attempt]);

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
      ((frames[idx + 1]!.t - frames[idx]!.t) * 1000) / Number(speed),
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
  }, [playing, idx, frames, speed]);

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
  const stage = useRef<HTMLDivElement>(null);
  const [stageWidth,setStageWidth] = useState(0);
  const [natural,setNatural] = useState<{src:string;width:number;height:number}|null>(null);
  useLayoutEffect(()=>{
    if(!stage.current)return;
    const observer=new ResizeObserver(([entry])=>setStageWidth(entry.contentRect.width));
    observer.observe(stage.current);return()=>observer.disconnect();
  },[frames]);
  const src = useMemo(
    () => (current ? `data:${current.mediaType};base64,${current.base64}` : null),
    [current],
  );

  const viewport = current?.w && current?.h ? {width:current.w,height:current.h} : natural?.src===src ? natural : null;
  const fit = viewport ? fitReplayViewport(stageWidth,viewport) : null;

  if (failed) {
    return <div className="p-5" role="alert"><p>{t('replay.loadFailed')}</p><Button className="mt-3" onClick={()=>{setFailed(false);setFrames(null);setAttempt(n=>n+1);}}>{t('common.retry')}</Button></div>;
  }
  if (!frames) {
    return <p className="text-sm text-fg-subtle">{t('shell.loading')}</p>;
  }
  if (frames.length === 0) {
    return <p className="text-sm text-fg-subtle">{t('replay.empty')}</p>;
  }

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
          setIdx((i) => Math.min(i + 1, frames.length - 1));
        } else if (e.key === 'ArrowLeft') {
          e.preventDefault();
          setPlaying(false);
          setIdx((i) => Math.max(i - 1, 0));
        } else if (e.key === ' ') {
          e.preventDefault();
          if (frames.length > 1) {
            if (!playing && idx >= frames.length - 1) setIdx(0);
            setPlaying(!playing);
          }
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
      <div className="replay-stage" ref={stage}>
        {src && (
          // relative wrapper shrink-wraps the img, so percentage
          // positions inside it ARE positions on the frame
          <span className="replay-frame" style={fit ? {width:fit.width,height:fit.height} : {width:'100%',height:'100%'}}>
            <img
              src={src}
              alt={t('replay.frameAlt', { t: current!.t.toFixed(1) })}
              className="h-full w-full object-contain"
              onError={()=>setFailed(true)}
              onLoad={event=>setNatural({src:src!,width:event.currentTarget.naturalWidth,height:event.currentTarget.naturalHeight})}
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
      <ReplayControls index={idx} last={frames.length - 1} playing={playing} speed={speed} onSpeed={setSpeed}
        time={`${(current!.t).toFixed(1)}s · ${idx+1}/${frames.length}`}
        onIndex={value=>{setPlaying(false);setIdx(value);}}
        onToggle={()=>{if(!playing&&idx>=frames.length - 1)setIdx(0);setPlaying(!playing);}}
        labels={{play:t('replay.play'),pause:t('replay.pause'),back:t('replay.stepBack'),forward:t('replay.stepForward'),seek:t('replay.scrubber'),speed:t('replay.speed')}}/>
    </div>
  );
}
