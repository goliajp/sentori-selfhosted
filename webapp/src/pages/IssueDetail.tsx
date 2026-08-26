// The case file — the right half of the triage split.
//
// Shape (insight feedback round 4): a pinned header that reads as
// one sentence (what broke, how wide, how long, still happening,
// where), then the evidence — replay, the failing code, the user's
// own actions (signals), and the occurrence list. The occurrence
// list is not an appendix: it is the set of concrete events this
// issue aggregates, and picking one switches the whole case view
// (replay / stack / signals) to that event. No comment stream, no
// duplicate environment box, no AI-paste block — density over
// furniture.

import { useCallback, useState } from 'react';
import { Link } from 'react-router-dom';

import { useShell } from '../App';
import { KindBadge, RegressedBadge } from '../components/kind';
import { UserChip } from '../components/identity';
import {
  Button,
  Input,
  Panel,
  PanelEmpty,
  clsx,
  formatRelative,
  formatRelease,
} from '../components/ui';
import { useT } from '../i18n';
import {
  api,
  type EventDetail,
  type IssueReleaseRow,
  type OccurrenceRow,
} from '../lib/api';
import { isBareTypeTitle } from '../lib/issue-title';
import { useAsyncData } from '../lib/useAsyncData';

import { ReplayPlayer } from '../components/ReplayPlayer';
import { StackTrace, type StackFrame as Frame } from '../components/StackTrace';
import {
  TimelineStrip,
  signalColor,
  summarizeSignal,
  type StripContextEvent,
  type StripSignal,
} from '../components/TimelineStrip';
import { WireframePlayer } from '../components/WireframePlayer';

type Signal = { t: number; kind: string; data?: Record<string, unknown> };

export function IssueDetailPane({
  issueId,
  onChanged,
}: {
  issueId: string;
  onChanged?: () => void;
}) {
  const t = useT();
  useShell();

  const { data: issue, error, loading, reload } = useAsyncData(
    () => api.getIssue(issueId),
    [issueId],
  );
  const { data: occ } = useAsyncData(() => api.listOccurrences(issueId), [issueId]);

  // The selected occurrence drives the whole case view. Reset when
  // the issue changes (adjust-during-render, no effect needed).
  const [sel, setSel] = useState<{ issue: string; id: null | string }>({
    issue: issueId,
    id: null,
  });
  if (sel.issue !== issueId) setSel({ issue: issueId, id: null });
  const currentId =
    (sel.id && occ?.events.some((e) => e.id === sel.id) ? sel.id : null) ??
    occ?.events[0]?.id;

  const { data: current } = useAsyncData<EventDetail | null>(
    () => (currentId ? api.getEvent(currentId) : Promise.resolve(null)),
    [currentId],
  );
  const { data: ctx } = useAsyncData(
    () => (currentId ? api.eventContext(currentId) : Promise.resolve({ events: [] })),
    [currentId],
  );

  const [replaySeek, setReplaySeek] = useState<{ t: number; n: number } | null>(null);
  // Frame moments arrive from whichever player loaded; tagged with
  // the event they belong to so switching events never shows stale
  // ticks (and needs no reset effect).
  const [framesFor, setFramesFor] = useState<{ id: null | string; times: number[] }>({
    id: null,
    times: [],
  });
  // The replay playhead, for highlighting the signal list.
  const [playhead, setPlayhead] = useState<null | number>(null);
  const [resolveRelease, setResolveRelease] = useState('');
  const [busy, setBusy] = useState(false);

  const act = async (fn: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await fn();
      reload();
      onChanged?.();
    } finally {
      setBusy(false);
    }
  };

  // Replay ladder, same-event first, richest form first:
  //   ① this event's pixels  ② this event's wireframe (always-on
  //   capture)  ③ pixels from the newest occurrence that captured
  //   any  ④ the enable hint.
  const screensLatest =
    current?.attachments?.find((a) => a.kind === 'screens')?.ref ?? null;
  const wireframeLatest =
    current?.attachments?.find((a) => a.kind === 'replay')?.ref ?? null;
  const screensFallback =
    screensLatest || wireframeLatest
      ? null
      : (occ?.events.find((e) => e.screensRef) ?? null);
  const screensRef = screensLatest ?? screensFallback?.screensRef ?? null;
  const replaySeekable = !!(screensLatest || wireframeLatest);
  const seekReplay = (tv: number) =>
    setReplaySeek((prev) => ({ t: tv, n: (prev?.n ?? 0) + 1 }));
  const frameTimes = framesFor.id === currentId ? framesFor.times : [];
  const reportFrames = useCallback(
    (times: number[]) => setFramesFor({ id: currentId ?? null, times }),
    [currentId],
  );
  const reportTime = useCallback((tv: number) => setPlayhead(tv), []);
  const eventAtMs = current ? new Date(current.occurredAt).getTime() : 0;
  const contextEvents: StripContextEvent[] = (ctx?.events ?? []).map((e) => ({
    id: e.id,
    issueId: e.issueId,
    kind: e.kind,
    name: e.name,
    t: (new Date(e.occurredAt).getTime() - eventAtMs) / 1000,
  }));
  const payload = current?.payload as
    | {
        error?: { type?: string; message?: string; stack?: Frame[] };
        signals?: Signal[];
        device?: Record<string, unknown>;
        /** Present when the SDK had pixel replay running and the
         *  ring still came up empty — an older native binary, or a
         *  capture that keeps failing. Its absence means the host
         *  never asked for it. */
        replay?: { screens?: string; captured?: number };
      }
    | undefined;

  if (error) {
    return (
      <div className="flex h-full items-center justify-center">
        <p className="text-sm text-fg-subtle">
          {t('issue.loadFailed')}{' '}
          <button type="button" className="underline" onClick={reload}>
            {t('common.retry')}
          </button>
        </p>
      </div>
    );
  }
  if (loading || !issue) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-fg-subtle">
        …
      </div>
    );
  }

  // The SDK says so when pixel replay was running and captured
  // nothing. Telling a reader to switch on a setting that is already
  // on is worse than saying nothing.
  const screensArmed = payload?.replay?.screens === 'empty';
  const surface = issue.surface as { screen?: string; element?: string };
  const hasStack = !!payload?.error?.stack && payload.error.stack.length > 0;
  const device = payload?.device;
  const signals = ((payload?.signals ?? []) as Signal[])
    .slice()
    .sort((a, b) => a.t - b.t);
  // Tap moments with coordinates (SDK ≥ 5.3) — drawn on the replay.
  const taps = signals
    .filter(
      (s) =>
        s.kind === 'tap' &&
        typeof s.data?.x === 'number' &&
        typeof s.data?.y === 'number',
    )
    .map((s) => ({ t: s.t, x: s.data!.x as number, y: s.data!.y as number }));

  // A bare "Error" is a type, not a headline (A13).
  const demoteTitle =
    isBareTypeTitle(issue.title) &&
    issue.messageSample &&
    issue.messageSample !== issue.title;
  const headline = demoteTitle ? issue.messageSample : issue.title;
  const subline = demoteTitle
    ? issue.title
    : issue.messageSample && !issue.title.includes(issue.messageSample)
      ? issue.messageSample
      : null;

  return (
    <div className="flex h-full min-w-0 flex-col">
      {/* ── pinned case header: one readable sentence ── */}
      <header className="shrink-0 border-b border-border bg-bg px-5 pb-3 pt-3.5">
        <div className="flex items-center gap-2">
          <KindBadge kind={issue.kind} />
          {issue.regressedAt && issue.status === 'open' && <RegressedBadge />}
          {demoteTitle && (
            <span className="rounded bg-raised px-1.5 py-0.5 font-mono text-xs text-fg-muted">
              {subline}
            </span>
          )}
          {surface.screen && (
            <span className="rounded bg-raised px-1.5 py-0.5 font-mono text-xs text-fg-muted">
              {surface.screen}
              {surface.element ? ` · ${surface.element}` : ''}
            </span>
          )}
          <span className="ml-auto flex items-center gap-2">
            {issue.status === 'open' ? (
              <>
                {/* labelled, so the release box reads as "anchor the
                    fix to this release", not a mystery filter (A12) */}
                <label className="flex items-center gap-1.5">
                  <span className="shrink-0 text-xs text-fg-muted">
                    {t('issue.resolveInRelease')}
                  </span>
                  <Input
                    value={resolveRelease}
                    onChange={(e) => setResolveRelease(e.target.value)}
                    placeholder={
                      issue.lastRelease ? formatRelease(issue.lastRelease) : 'app@x.y.z'
                    }
                    className="h-7 w-52 font-mono text-xs"
                  />
                </label>
                <Button
                  size="sm"
                  disabled={busy}
                  onClick={() =>
                    void act(() => api.resolveIssue(issue.id, resolveRelease || undefined))
                  }
                >
                  {t('issue.resolve')}
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={busy}
                  onClick={() => void act(() => api.ignoreIssue(issue.id))}
                >
                  {t('issue.ignore')}
                </Button>
              </>
            ) : (
              <Button
                size="sm"
                disabled={busy}
                onClick={() => void act(() => api.reopenIssue(issue.id))}
              >
                {t('issue.reopen')}
              </Button>
            )}
          </span>
        </div>
        <h1 className="mt-1.5 truncate text-[20px] font-semibold tracking-tight">
          {headline}
        </h1>
        {!demoteTitle && subline && (
          <p className="mt-0.5 truncate text-sm text-fg-muted">{subline}</p>
        )}
        <div className="mt-2 flex flex-wrap items-baseline gap-x-4 gap-y-1 text-xs tabular-nums text-fg-muted">
          <span className="text-fg">
            {issue.usersCount > 0
              ? t('issue.impactUsers', {
                  users: String(issue.usersCount),
                  events: String(issue.eventCount),
                  max: String(issue.maxPerUser),
                })
              : t('issue.impactAnon', { events: String(issue.eventCount) })}
          </span>
          {/* The trip from "this is fixed" to "tell the people it
              happened to" was a person copying a uuid, if they knew
              the audience could name an issue at all. It is a link
              now — to the place that counts the audience first, not to
              a send: nothing here should be one click from a
              notification. Hidden when nobody identified hit it,
              because then it targets nobody. */}
          {issue.usersCount > 0 && (
            <Link
              to={`/push?tab=audience&issue=${issue.id}`}
              className="text-accent hover:underline"
            >
              {t('issue.notifyAffected')}
            </Link>
          )}
          <span>
            {t('issue.firstSeen')} {formatRelative(issue.firstSeen)}
          </span>
          <span>
            {t('issue.lastSeen')} {formatRelative(issue.lastSeen)}
          </span>
          {issue.lastRelease && (
            <span title={issue.lastRelease}>{formatRelease(issue.lastRelease)}</span>
          )}
          {current && (
            <span>
              {[
                device?.model,
                device?.os &&
                  `${String(device.os)} ${String(device.osVersion ?? '')}`.trim(),
              ]
                .filter(Boolean)
                .join(' · ')}
            </span>
          )}
          <span>{issue.platform ?? current?.platform}</span>
          <span>{issue.environment ?? current?.environment}</span>
        </div>
      </header>

      {/* ── the evidence: what the user saw (primary), the code +
          the user's own actions + the occurrence set (secondary),
          the minute itself pinned underneath ──

          The split waits for 2xl, not xl. Behind a 190px rail and a
          360px queue, 1280 left the stack column 222px wide: every
          code line clipped, every signal truncated to "from=…". One
          700px column beats two useless ones. */}
      <div className="min-h-0 flex-1 overflow-hidden">
        <div className="grid h-full grid-cols-1 gap-4 overflow-y-auto p-4 2xl:grid-cols-[minmax(360px,460px)_minmax(0,1fr)] 2xl:overflow-hidden">
          <div className="min-w-0 space-y-4 2xl:overflow-y-auto">
            <Panel
              title={t('replay.title')}
              action={
                screensFallback ? (
                  <span className="truncate text-xs normal-case tracking-normal text-fg-subtle">
                    {t('issue.replayFrom', {
                      when: formatRelative(screensFallback.occurredAt),
                    })}
                  </span>
                ) : !screensRef && wireframeLatest ? (
                  <span
                    className="text-xs normal-case tracking-normal text-fg-subtle"
                    title={
                      screensArmed
                        ? t('issue.replayScreensEmptyTip')
                        : t('issue.replayWireframeTip')
                    }
                  >
                    wireframe
                  </span>
                ) : undefined
              }
            >
              {screensRef ? (
                <ReplayPlayer
                  attachmentRef={screensRef}
                  seek={replaySeek}
                  onFrames={screensLatest ? reportFrames : undefined}
                  onTime={screensLatest ? reportTime : undefined}
                  taps={screensLatest ? taps : undefined}
                />
              ) : wireframeLatest ? (
                <>
                  <WireframePlayer
                    attachmentRef={wireframeLatest}
                    seek={replaySeek}
                    onFrames={reportFrames}
                    onTime={reportTime}
                    taps={taps}
                  />
                  {/* Visible, not a tooltip. This build asked for
                      pixels and got none, which is fixable — and a
                      reader who does not hover would otherwise be
                      told, elsewhere on this page, to turn on the
                      setting they already turned on. */}
                  {screensArmed && (
                    <p className="border-t border-border/60 px-3.5 py-2 text-xs text-fg-subtle">
                      {t('issue.replayScreensEmpty')}
                    </p>
                  )}
                </>
              ) : (
                <PanelEmpty>
                  {screensArmed
                    ? t('issue.replayScreensEmpty')
                    : t('issue.replayNone')}
                </PanelEmpty>
              )}
            </Panel>

            {issue.status === 'resolved' && (
              <Panel title={<span className="text-ok">{t('issue.guardTitle')}</span>}>
                <div className="space-y-1 p-3.5 text-sm text-fg-muted">
                  <p>
                    {issue.resolvedInRelease
                      ? t('issue.guardAnchored', { release: issue.resolvedInRelease })
                      : t('issue.guardUnanchored')}
                  </p>
                  <p>{t('issue.guardProbeHint')}</p>
                </div>
              </Panel>
            )}
          </div>

          <div className="min-w-0 space-y-4 2xl:overflow-y-auto">
            {hasStack && (
              <Panel title={t('issue.code')}>
                <StackTrace frames={payload!.error!.stack!} />
              </Panel>
            )}

            <Panel title={`${t('issue.signals')}${signals.length ? ` (${signals.length})` : ''}`}>
              {signals.length === 0 ? (
                <PanelEmpty>{t('issue.signalsNone')}</PanelEmpty>
              ) : (
                <SignalList
                  signals={signals}
                  playhead={playhead}
                  onSeek={replaySeekable ? seekReplay : undefined}
                />
              )}
            </Panel>

            {occ && occ.events.length > 0 && (
              <Panel title={`${t('issue.occurrences')} (${occ.events.length})`}>
                <OccurrenceList
                  rows={occ.events}
                  currentId={currentId ?? null}
                  onPick={(id) => setSel({ issue: issueId, id })}
                />
              </Panel>
            )}

            {issue.releases && issue.releases.length > 0 && (
              <Panel title={`${t('issue.releasesTitle')} (${issue.releases.length})`}>
                <ReleaseSpread
                  rows={issue.releases}
                  resolvedIn={issue.resolvedInRelease}
                  regressedIn={issue.regressedInRelease}
                />
              </Panel>
            )}
          </div>
        </div>
      </div>

      {/* ── the minute itself: the case's narrative spine ── */}
      <TimelineStrip
        signals={signals as StripSignal[]}
        frameTimes={frameTimes}
        context={contextEvents}
        issueKind={issue.kind}
        onSeek={replaySeekable ? seekReplay : undefined}
      />
    </div>
  );
}

/** The user's own actions before the event, as a readable list —
 *  every row seeks the replay; the row nearest the playhead is
 *  highlighted while the replay runs (A3). */
function SignalList({
  signals,
  playhead,
  onSeek,
}: {
  signals: Signal[];
  playhead: null | number;
  onSeek?: (t: number) => void;
}) {
  return (
    // Capped only below the two-column split, where the whole page
    // scrolls and an eighty-signal journey would push the stack out
    // of reach. At xl the right column scrolls on its own, so a
    // second scrollbar inside it just hides the second half of the
    // journey behind an affordance nobody sees.
    <div className="max-h-72 overflow-y-auto 2xl:max-h-none 2xl:overflow-visible">
      {signals.map((s, i) => {
        const near = playhead !== null && Math.abs(s.t - playhead) < 2.5;
        // http rows read as a request line, not as k=v soup; a
        // failed request (no response / 5xx) carries the error hue.
        const d = s.data ?? {};
        const httpFailed =
          s.kind === 'http' &&
          (d.status === 0 || (typeof d.status === 'number' && d.status >= 500));
        const summary =
          s.kind === 'http'
            ? `${String(d.method ?? '?')} ${String(d.url ?? '?')} → ${
                d.status === 0 ? '×' : String(d.status ?? '?')
              }${typeof d.ms === 'number' ? ` · ${d.ms}ms` : ''}`
            : // `target` is RN's internal node tag — meaningless to a
              // reader (A2); it stays in the raw payload only.
              summarizeSignal({
                ...s,
                data: Object.fromEntries(
                  Object.entries(d).filter(([k]) => k !== 'target'),
                ),
              });
        return (
          <button
            key={i}
            type="button"
            disabled={!onSeek}
            onClick={() => onSeek?.(s.t)}
            className={clsx(
              'flex w-full items-baseline gap-2.5 border-b border-border/60 px-3.5 py-1.5 text-left text-xs last:border-b-0',
              near ? 'bg-raised' : onSeek ? 'hover:bg-raised/50' : '',
            )}
          >
            <span className="w-12 shrink-0 text-right tabular-nums text-fg-subtle">
              {s.t.toFixed(1)}s
            </span>
            <span
              aria-hidden
              className="h-1.5 w-1.5 shrink-0 self-center rounded-full"
              style={{ backgroundColor: signalColor(s.kind) }}
            />
            <span className="w-14 shrink-0 text-fg">{s.kind}</span>
            <span
              className={`min-w-0 flex-1 truncate font-mono ${
                httpFailed ? 'text-kind-error' : 'text-fg-muted'
              }`}
            >
              {summary}
            </span>
          </button>
        );
      })}
    </div>
  );
}

/** Which releases this case has appeared in, at what volume — the
 *  version dimension read as a distribution instead of splitting
 *  the issue. The resolve anchor and the regression release carry
 *  their own markers. */
function ReleaseSpread({
  rows,
  resolvedIn,
  regressedIn,
}: {
  rows: IssueReleaseRow[];
  resolvedIn: null | string;
  regressedIn: null | string;
}) {
  const t = useT();
  const max = Math.max(...rows.map((r) => r.events), 1);
  return (
    <div className="divide-y divide-border/60">
      {rows.map((r) => (
        <div key={r.release} className="flex items-center gap-3 px-3.5 py-1.5 text-xs">
          <span
            className="w-56 shrink-0 truncate font-mono text-fg"
            title={r.release}
          >
            {formatRelease(r.release)}
          </span>
          {/* volume bar: share of this issue's events in that release */}
          <span className="h-2 min-w-0 flex-1 overflow-hidden rounded-sm bg-raised">
            <span
              className="block h-full rounded-sm"
              style={{
                width: `${Math.max(2, (r.events / max) * 100)}%`,
                backgroundColor: 'var(--sn-accent)',
                opacity: 0.55,
              }}
            />
          </span>
          <span className="w-14 shrink-0 text-right tabular-nums text-fg-muted">
            {r.events}ev
          </span>
          <span className="w-16 shrink-0 text-right tabular-nums text-fg-subtle">
            {formatRelative(r.lastAt)}
          </span>
          <span className="w-14 shrink-0 text-right">
            {r.release === resolvedIn && (
              <span className="text-ok" title={t('issue.releaseResolvedHere')}>
                ✓ fix
              </span>
            )}
            {r.release === regressedIn && (
              <span className="text-kind-error" title={t('issue.releaseRegressedHere')}>
                ↩
              </span>
            )}
          </span>
        </div>
      ))}
    </div>
  );
}

/** The concrete events this issue aggregates. Picking one switches
 *  the whole case view; fields identical across every occurrence
 *  render dimmed so the differences carry the row (A6). */
function OccurrenceList({
  rows,
  currentId,
  onPick,
}: {
  rows: OccurrenceRow[];
  currentId: null | string;
  onPick: (id: string) => void;
}) {
  const distinct = (get: (r: OccurrenceRow) => string) =>
    new Set(rows.map(get)).size > 1;
  const varies = {
    platform: distinct((r) => r.platform),
    release: distinct((r) => r.release),
    environment: distinct((r) => r.environment),
    user: distinct((r) => r.userKey ?? ''),
  };
  // Stable per-issue user numbering, so "U2" means something across
  // rows without exposing the raw hash (A6).
  const userIndex = new Map<string, number>();
  for (const r of rows) {
    if (r.userKey && !userIndex.has(r.userKey)) {
      userIndex.set(r.userKey, userIndex.size + 1);
    }
  }
  const dim = (varying: boolean) => (varying ? 'text-fg' : 'text-fg-subtle/60');

  return (
    <div className="divide-y divide-border/60">
      {rows.map((r) => (
        <button
          key={r.id}
          type="button"
          onClick={() => onPick(r.id)}
          className={clsx(
            'flex w-full items-center gap-4 px-3.5 py-2 text-left text-sm transition-colors',
            r.id === currentId ? 'bg-raised' : 'hover:bg-raised/50',
          )}
        >
          <span
            className={clsx(
              'w-16 shrink-0 tabular-nums',
              r.id === currentId ? 'font-semibold text-fg' : 'text-fg-muted',
            )}
          >
            {formatRelative(r.receivedAt)}
          </span>
          <span className={dim(varies.platform)}>{r.platform}</span>
          <span
            className={clsx('min-w-0 truncate font-mono text-xs', dim(varies.release))}
            title={r.release}
          >
            {formatRelease(r.release)}
          </span>
          <span className={dim(varies.environment)}>{r.environment}</span>
          <span className="ml-auto shrink-0">
            {r.userKey && (
              <UserChip
                userKey={r.userKey}
                label={`U${userIndex.get(r.userKey)}`}
                muted={!varies.user}
              />
            )}
          </span>
        </button>
      ))}
    </div>
  );
}
