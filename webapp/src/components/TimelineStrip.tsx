// The case timeline — the narrative spine of an issue, pinned to
// the bottom of the case file like a video editor's track area.
//
// The axis is fitted to the data, not to a fixed minute: a 3-second
// automation run gets a 5-second axis, a long session stretches to
// whatever the signals cover. The event moment keeps fixed breathing
// room on the right, ticks land on round numbers only, and the span
// before the earliest captured moment is dimmed — "nothing captured
// here" must read differently from "nothing happened here".
//
// Three lanes: replay frames as ticks, user behaviour as dots, the
// OTHER events this user's app reported in the window as flags.
// Everything seeks the replay on click.

import { useEffect, useMemo, useRef, useState } from 'react';

import { useT } from '../i18n';
import type { IssueSummary } from '../lib/api';
import { kindColor } from './kind';

export type StripSignal = { t: number; kind: string; data?: Record<string, unknown> };
export type StripContextEvent = {
  id: string;
  issueId: string;
  kind: IssueSummary['kind'];
  name: string;
  /** Seconds relative to the event, negative. */
  t: number;
};

/** Timeline hues borrow the five kind hues so the palette keeps one
 *  concept model: nav reads calm, freeze reads like the error it
 *  usually precedes. */
export function signalColor(kind: string): string {
  switch (kind) {
    case 'nav':
      return 'var(--s-kind-trace)';
    case 'tap':
      return 'var(--s-kind-warn)';
    case 'http':
      return 'var(--s-kind-assert)';
    case 'trace':
      return 'var(--s-kind-probe)';
    case 'freeze':
      return 'var(--s-kind-error)';
    default:
      return 'var(--sn-fg-muted)';
  }
}

export function summarizeSignal(s: StripSignal): string {
  const d = s.data ?? {};
  const parts = Object.entries(d)
    .filter(([, v]) => v !== undefined && v !== null)
    .slice(0, 4)
    .map(([k, v]) => `${k}=${String(v)}`);
  return parts.join(' ');
}

/** The event moment sits at 95%, not the edge — the most important
 *  point on the axis gets breathing room. */
const RIGHT_PAD_PCT = 5;
/** Room kept clear to the left of the event line for its own label,
 *  which is a phrase in three languages rather than a number. */
const EVENT_LABEL_PX = 96;

/** Round axis spans (s). Beyond the table, multiples of 60. */
const NICE_SPANS = [5, 10, 15, 30, 60, 120, 300];

function niceSpan(coverS: number): number {
  for (const s of NICE_SPANS) if (coverS <= s) return s;
  return Math.ceil(coverS / 60) * 60;
}

/** Roughly the width of a `-60s` label plus breathing room. */
const LABEL_PX = 64;

/**
 * A tick step that yields round labels which fit.
 *
 * `maxLabels` used to be the constant 8, chosen for a wide viewport.
 * The strip lives in the issue detail pane, which is about 330px at a
 * 900px window — seven labels then overlapped into `-60s50s -40s`, and
 * the event marker collided with the last tick. The step now has to
 * clear the measured width as well as the span.
 */
/** Is there room to print this tick's label without the event
 *  line's own label landing on it?
 *
 *  The label at 0s is words rather than a number — `error 触发`,
 *  `error fired`, `error 発生` — and it hangs left from the event
 *  line, straight into the tick before it. At a 60s span with a 5s
 *  step that tick is 8% of the track away, and `-5s` shipped with the
 *  word `error` struck through it.
 *
 *  Reserved in pixels, because that is the unit text occupies: a
 *  percentage that clears the word on a 1400px pane does not on a
 *  500px one. Before the first measurement `trackW` is 0 — assume the
 *  narrow case and drop the label, since a missing tick reads as
 *  spacing and two words on top of each other read as broken.
 *
 *  Exported for `devtools/check-timeline-labels.mjs`. */
export function tickLabelFits(sec: number, spanS: number, trackW: number): boolean {
  if (sec === 0) return true;
  if (trackW <= 0 || spanS <= 0) return false;
  const scale = (100 - RIGHT_PAD_PCT) / 100;
  return ((-sec / spanS) * scale) * trackW >= EVENT_LABEL_PX;
}

function tickStep(span: number, maxLabels: number): number {
  for (const step of [1, 2, 5, 10, 15, 30, 60, 120]) {
    if (span / step <= maxLabels) return step;
  }
  return Math.ceil(span / Math.max(2, maxLabels - 2) / 60) * 60;
}

export function TimelineStrip({
  signals,
  frameTimes,
  context,
  issueKind,
  onSeek,
}: {
  signals: StripSignal[];
  /** Replay frame moments, seconds relative to the event (negative). */
  frameTimes: number[];
  context: StripContextEvent[];
  issueKind: IssueSummary['kind'];
  onSeek?: (t: number) => void;
}) {
  const t = useT();
  const trackRef = useRef<HTMLDivElement>(null);
  const [readout, setReadout] = useState<string | null>(null);

  // Fit the axis to the data: earliest captured moment across all
  // three sources decides the span (min 5s, round numbers only).
  // Measured, not assumed: the pane this sits in is resizable and the
  // window is not the constraint.
  const [trackW, setTrackW] = useState(0);
  useEffect(() => {
    const el = trackRef.current;
    if (!el || typeof ResizeObserver === 'undefined') return;
    const ro = new ResizeObserver(([entry]) => {
      setTrackW(entry.contentRect.width);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const { spanS, coverStart, ticks } = useMemo(() => {
    const all = [
      ...signals.map((s) => s.t),
      ...frameTimes,
      ...context.map((c) => c.t),
    ].filter((v) => v <= 0);
    const earliest = all.length ? Math.min(...all) : -60;
    const span = niceSpan(Math.max(5, -earliest));
    // Before the first measurement, assume a narrow pane: too few
    // labels reads as sparse, too many reads as broken.
    const maxLabels = trackW > 0 ? Math.max(2, Math.floor(trackW / LABEL_PX)) : 4;
    const step = tickStep(span, maxLabels);
    const marks: number[] = [];
    for (let s = -span; s < 0; s += step) marks.push(s);
    marks.push(0);
    return { spanS: span, coverStart: all.length ? earliest : null, ticks: marks };
  }, [signals, frameTimes, context, trackW]);

  const pct = (tv: number): number =>
    Math.min(100, Math.max(0, ((tv + spanS) / spanS) * (100 - RIGHT_PAD_PCT)));

  const clearsEventLabel = (sec: number): boolean =>
    tickLabelFits(sec, spanS, trackW);

  const inWindow = useMemo(
    () => ({
      signals: signals.filter((s) => s.t >= -spanS && s.t <= 0),
      frames: frameTimes.filter((f) => f >= -spanS && f <= 0),
      context: context.filter((c) => c.t >= -spanS && c.t <= 0),
    }),
    [signals, frameTimes, context, spanS],
  );

  const seekFromTrack = (e: React.MouseEvent) => {
    if (!onSeek || !trackRef.current) return;
    const box = trackRef.current.getBoundingClientRect();
    const frac = Math.min(1, Math.max(0, (e.clientX - box.left) / box.width));
    // invert pct(): the horizontal fraction back to event-relative s
    const tv = (frac * 100 / (100 - RIGHT_PAD_PCT)) * spanS - spanS;
    onSeek(Math.min(0, tv));
  };

  return (
    <section className="flex h-36 shrink-0 flex-col border-t border-border bg-bg">
      <header className="flex h-7 shrink-0 items-center gap-3 border-b border-border px-4">
        <h3 className="shrink-0 text-sm font-semibold text-fg-muted">
          {t('issue.timeline')}
        </h3>
        <span className="font-mono text-xs text-fg-subtle">{spanS}s</span>
        <span className="min-w-0 flex-1 truncate text-right font-mono text-xs text-fg-muted">
          {readout ?? ''}
        </span>
      </header>

      <div className="relative min-h-0 flex-1 px-4 py-2">
        {/* lane labels */}
        <div className="pointer-events-none absolute inset-y-2 left-4 z-10 flex w-14 flex-col justify-between py-0.5 font-mono text-xs text-fg-subtle">
          <span>{t('strip.frames')}</span>
          <span>{t('strip.signals')}</span>
          <span>{t('strip.events')}</span>
        </div>

        <div
          ref={trackRef}
          role="presentation"
          onClick={seekFromTrack}
          className={`relative ml-[68px] mr-1.5 h-full ${onSeek ? 'cursor-pointer' : ''}`}
        >
          {/* the span before the earliest captured moment: nothing was
              captured there — dim it so it can't read as "quiet" */}
          {coverStart !== null && coverStart > -spanS && (
            <div
              aria-hidden
              className="absolute inset-y-0 left-0 bg-raised/40"
              style={{ width: `${pct(coverStart)}%` }}
            />
          )}

          {/* gridlines on round seconds + the event line */}
          {ticks.map((sec, tickIdx) => (
            <div
              key={sec}
              className="absolute inset-y-0"
              style={{ left: `${pct(sec)}%` }}
            >
              <div
                className={
                  sec === 0
                    ? 'absolute inset-y-0 w-px'
                    : 'absolute inset-y-0 w-px bg-border/60'
                }
                style={sec === 0 ? { backgroundColor: kindColor(issueKind) } : undefined}
              />
              {/* the 0s label hangs left of its line so the track's
                  right edge never clips it; the earliest one hangs
                  right for the same reason at the other end — a
                  centred label there reaches back into the lane
                  gutter, which in Japanese is full of 「イベント」 */}
              <span
                className={`absolute bottom-0 whitespace-nowrap font-mono text-xs ${
                  sec === 0
                    ? '-translate-x-full pr-1.5 font-semibold'
                    : tickIdx === 0
                      ? 'translate-x-0 pl-0.5 text-fg-subtle'
                      : '-translate-x-1/2 text-fg-subtle'
                }`}
                style={sec === 0 ? { color: kindColor(issueKind) } : undefined}
              >
                {sec === 0
                  ? t('issue.eventMoment', { kind: issueKind })
                  : clearsEventLabel(sec)
                    ? `${sec}s`
                    : ''}
              </span>
            </div>
          ))}

          {/* lane 1 — replay frames as ticks */}
          <div className="absolute inset-x-0 top-[6%] h-[16%]">
            {inWindow.frames.map((f, i) => (
              <button
                key={i}
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  onSeek?.(f);
                }}
                onMouseEnter={() => setReadout(`${f.toFixed(1)}s`)}
                onMouseLeave={() => setReadout(null)}
                aria-label={`${f.toFixed(1)}s`}
                className="absolute top-0 h-full w-[3px] -translate-x-1/2 rounded-sm bg-border-strong hover:bg-fg-muted"
                style={{ left: `${pct(f)}%` }}
              />
            ))}
          </div>

          {/* lane 2 — behaviour signals as dots */}
          <div className="absolute inset-x-0 top-[38%] h-[16%]">
            {inWindow.signals.map((s, i) => (
              <button
                key={i}
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  onSeek?.(s.t);
                }}
                onMouseEnter={() =>
                  setReadout(
                    `${s.t.toFixed(1)}s · ${s.kind} ${summarizeSignal(s)}`.trim(),
                  )
                }
                onMouseLeave={() => setReadout(null)}
                aria-label={`${s.t.toFixed(1)}s ${s.kind}`}
                className="absolute top-1/2 h-2.5 w-2.5 -translate-x-1/2 -translate-y-1/2 rounded-full ring-2 ring-bg hover:h-3.5 hover:w-3.5"
                style={{ left: `${pct(s.t)}%`, backgroundColor: signalColor(s.kind) }}
              />
            ))}
          </div>

          {/* lane 3 — the other events of the same user's minute */}
          <div className="absolute inset-x-0 top-[68%] h-[16%]">
            {inWindow.context.map((c) => (
              <button
                key={c.id}
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  onSeek?.(c.t);
                }}
                onMouseEnter={() =>
                  setReadout(`${c.t.toFixed(1)}s · ${c.kind} ${c.name}`)
                }
                onMouseLeave={() => setReadout(null)}
                aria-label={`${c.kind} ${c.name}`}
                className="absolute top-1/2 h-2.5 w-2.5 -translate-x-1/2 -translate-y-1/2 rotate-45 ring-2 ring-bg hover:h-3.5 hover:w-3.5"
                style={{ left: `${pct(c.t)}%`, backgroundColor: kindColor(c.kind) }}
              />
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}
