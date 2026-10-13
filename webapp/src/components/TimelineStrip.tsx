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

/** Round axis spans (s). Beyond the table, multiples of 60. */
const NICE_SPANS = [5, 10, 15, 30, 60, 120, 300];

function niceSpan(coverS: number): number {
  for (const s of NICE_SPANS) if (coverS <= s) return s;
  return Math.ceil(coverS / 60) * 60;
}

/** Roughly the width of a `-60s` label plus breathing room. */
const LABEL_PX = 64;

/** Numeric labels use the same centred alignment, including 0s.
 *  The trigger description lives in the status bar, outside the ruler.
 *  Suppress a final intermediate tick when rounding crowds the endpoint. */
export function tickLabelFits(sec: number, spanS: number, trackW: number): boolean {
  if (sec === 0 || sec === -spanS) return true;
  if (trackW <= 0 || spanS <= 0) return false;
  return (-sec / spanS) * trackW >= LABEL_PX;
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
    Math.min(100, Math.max(0, ((tv + spanS) / spanS) * 100));
  const visibleTicks = ticks.filter(sec => tickLabelFits(sec, spanS, trackW));

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
    onSeek(frac * spanS - spanS);
  };

  return (
    <section className="issue-timeline">
      <header className="issue-timeline-ruler">
        <h3>
          {t('issue.timeline')}
        </h3>
        <div className="issue-timeline-axis">
          {visibleTicks.map(sec => <span key={sec} data-timeline-tick={sec}
            className="issue-timeline-tick" style={{left: `${pct(sec)}%`, color: sec === 0 ? kindColor(issueKind) : undefined}}>
            {sec}s
          </span>)}
        </div>
      </header>

      <div className="issue-timeline-body">
        {/* lane labels */}
        <div className="issue-timeline-lanes">
          <span>{t('strip.frames')}</span>
          <span>{t('strip.signals')}</span>
          <span>{t('strip.events')}</span>
        </div>

        <div
          ref={trackRef}
          role="presentation"
          onClick={seekFromTrack}
          className={`issue-timeline-axis issue-timeline-track ${onSeek ? 'cursor-pointer' : ''}`}
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
          {visibleTicks.map((sec) => (
            <div
              key={sec}
              data-timeline-grid={sec}
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
            </div>
          ))}

          {/* lane 1 — replay frames as ticks */}
          <div className="absolute inset-x-0 top-0 h-1/3">
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
                onFocus={() => setReadout(`${f.toFixed(1)}s`)}
                onBlur={() => setReadout(null)}
                aria-label={`${f.toFixed(1)}s`}
                className="absolute top-1/4 h-1/2 w-[3px] -translate-x-1/2 rounded-sm bg-border-strong hover:bg-fg-muted"
                style={{ left: `${pct(f)}%` }}
              />
            ))}
          </div>

          {/* lane 2 — behaviour signals as dots */}
          <div className="absolute inset-x-0 top-1/3 h-1/3">
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
                onFocus={() => setReadout(`${s.t.toFixed(1)}s · ${s.kind} ${summarizeSignal(s)}`.trim())}
                onBlur={() => setReadout(null)}
                aria-label={`${s.t.toFixed(1)}s ${s.kind}`}
                className="absolute top-1/2 h-2.5 w-2.5 -translate-x-1/2 -translate-y-1/2 rounded-full ring-2 ring-bg hover:h-3.5 hover:w-3.5"
                style={{ left: `${pct(s.t)}%`, backgroundColor: signalColor(s.kind) }}
              />
            ))}
          </div>

          {/* lane 3 — the other events of the same user's minute */}
          <div className="absolute inset-x-0 top-2/3 h-1/3">
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
                onFocus={() => setReadout(`${c.t.toFixed(1)}s · ${c.kind} ${c.name}`)}
                onBlur={() => setReadout(null)}
                aria-label={`${c.kind} ${c.name}`}
                className="absolute top-1/2 h-2.5 w-2.5 -translate-x-1/2 -translate-y-1/2 rotate-45 ring-2 ring-bg hover:h-3.5 hover:w-3.5"
                style={{ left: `${pct(c.t)}%`, backgroundColor: kindColor(c.kind) }}
              />
            ))}
          </div>
        </div>
      </div>
      <footer className="issue-timeline-status">
        <output title={readout ?? undefined}>{readout ?? t('strip.inspectHint')}</output>
        <span style={{color: kindColor(issueKind)}}>{t('issue.eventMoment', {kind: issueKind})} · 0s</span>
      </footer>
    </section>
  );
}
