// What the user did before it broke.
//
// Breadcrumbs arrive oldest-first and that ordering is the point —
// this reads as a story ending in the crash, so it renders in
// collected order with the crash pinned at the bottom as the last
// beat.
//
// Time is shown as an offset from the crash ("−4.2s"), not as a wall
// clock. Nobody debugging asks what time it was; they ask how long
// before the end it happened.

import { useT } from '../../i18n';
import type { Breadcrumb, BreadcrumbType } from '../../lib/api';

/** One glyph per source, so a run of network calls is scannable
 *  without reading a word of it. Deliberately not colour — colour is
 *  reserved for severity here. */
const GLYPH: Record<BreadcrumbType, string> = {
  custom: '·',
  log: '›',
  nav: '→',
  net: '⇅',
  push: '▽',
  track: '◆',
  user: '✦',
};

export function BreadcrumbTimeline({
  breadcrumbs,
  crashedAt,
  playheadTs,
}: {
  breadcrumbs: Breadcrumb[];
  crashedAt: string;
  /** When the replay is scrubbing, the crumb nearest the playhead is
   *  marked — the recording and the log read as one timeline rather
   *  than two things that happen to be on the same page. */
  playheadTs?: number;
}) {
  const t = useT();
  const end = new Date(crashedAt).getTime();
  const activeIndex =
    playheadTs === undefined
      ? -1
      : breadcrumbs.reduce(
          (best, b, i) =>
            new Date(b.timestamp).getTime() <= playheadTs ? i : best,
          -1,
        );

  return (
    <ol className="relative space-y-0">
      {breadcrumbs.map((b, i) => (
        <Row
          key={i}
          crumb={b}
          offsetMs={new Date(b.timestamp).getTime() - end}
          active={i === activeIndex}
        />
      ))}
      <li className="flex items-baseline gap-3 border-l-2 border-l-danger py-1.5 pl-3">
        <span className="w-14 shrink-0 text-right font-mono text-xs text-danger">
          0.0s
        </span>
        <span className="w-4 shrink-0 text-center font-mono text-xs text-danger">
          ✕
        </span>
        <span className="text-xs font-medium text-danger">{t('crash.crash')}</span>
      </li>
    </ol>
  );
}

function Row({
  crumb,
  offsetMs,
  active,
}: {
  crumb: Breadcrumb;
  offsetMs: number;
  active?: boolean;
}) {
  return (
    <li
      className={`flex items-baseline gap-3 border-l-2 py-1.5 pl-3 transition ${
        active
          ? 'border-l-accent bg-raised/40'
          : 'border-l-border hover:border-l-border-strong'
      }`}
    >
      <span className="w-14 shrink-0 text-right font-mono text-xs tabular-nums text-fg-subtle">
        {formatOffset(offsetMs)}
      </span>
      <span
        className="w-4 shrink-0 text-center font-mono text-xs text-fg-subtle"
        aria-hidden
      >
        {GLYPH[crumb.type] ?? '·'}
      </span>
      <span className="min-w-0 flex-1 truncate font-mono text-xs text-fg-muted">
        <span className="text-fg-subtle">{crumb.type}</span>{' '}
        {summarise(crumb)}
      </span>
    </li>
  );
}

/** Negative offsets read as "before the crash"; anything at or after
 *  zero is clamped so a slightly-skewed clock doesn't render "+0.3s"
 *  on an event that by definition preceded the throw. */
function formatOffset(ms: number): string {
  if (!Number.isFinite(ms)) return '—';
  const secondsBefore = Math.max(0, -ms) / 1000;
  if (secondsBefore >= 60) {
    const m = Math.floor(secondsBefore / 60);
    return `−${m}m${Math.round(secondsBefore % 60)}s`;
  }
  return `−${secondsBefore.toFixed(1)}s`;
}

/** One line per crumb. The interesting field differs per type, so
 *  pick it rather than dumping the whole `data` object — a timeline
 *  you have to expand to skim is not a timeline. */
function summarise(crumb: Breadcrumb): string {
  const d = crumb.data ?? {};
  const pick = (...keys: string[]): string | undefined => {
    for (const k of keys) {
      const v = d[k];
      if (typeof v === 'string' && v) return v;
      if (typeof v === 'number') return String(v);
    }
    return undefined;
  };

  switch (crumb.type) {
    case 'nav':
      return [pick('from'), pick('to', 'route', 'screen')]
        .filter(Boolean)
        .join(' → ');
    case 'net': {
      const status = pick('status', 'statusCode');
      const url = pick('url', 'path') ?? '';
      const method = pick('method');
      return [method, url, status && `· ${status}`].filter(Boolean).join(' ');
    }
    case 'track':
    case 'push':
      return pick('name', 'title', 'msgId') ?? '';
    case 'user':
      return pick('action', 'target', 'id') ?? '';
    case 'log':
      return pick('message', 'msg', 'text') ?? '';
    default:
      return pick('name', 'message') ?? Object.keys(d).slice(0, 3).join(' ');
  }
}
