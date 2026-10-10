import { Code } from '../../shared/documents/Code';
import { Dialog } from '../../shared/design/Dialog';
import { useSharedT } from '../i18n/shared';
import { useCallback, useRef, useState, type ReactNode } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { Heading, Surface } from '../../shared/design/Surface';
import { Metric } from '../../shared/design/Metric';
import { Tabs } from '../../shared/design/Tabs';
import { Button, Input, Field, Badge } from '../../shared/design/controls';

import { KindBadge, RegressedBadge } from '../components/kind';
import { UserChip } from '../components/identity';
import { PanelEmpty, Select, ErrorBanner, TimeAgo, clsx, formatRelative, formatRelease, formatReleaseIn } from '../components/ui';
import { useT } from '../i18n';
import { platformLabel } from '../lib/platform-label';
import {
  api,
  type IssueReleaseRow,
  type OccurrenceRow,
} from '../lib/api';
import { isBareTypeTitle } from '../lib/issue-title';
import { formatApiError } from '../lib/useAsyncData';
import { useScopedAsyncData } from '../lib/useScopedAsyncData';

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
  const sharedT = useSharedT();
  const [rawOpen,setRawOpen] = useState(false);
  const [timelineOpen, setTimelineOpen] = useState(false);
  const [params, setParams] = useSearchParams();
  const tab = ['evidence','occurrences','activity'].includes(params.get('tab') ?? '') ? params.get('tab')! : 'evidence';
  const rawOffset = Number(params.get('occurrenceOffset') ?? 0);
  const occurrenceOffset = Number.isSafeInteger(rawOffset) && rawOffset >= 0 ? rawOffset : 0;
  function updateParams(changes: Record<string,string>) {
    const next = new URLSearchParams(params);
    for (const [key,value] of Object.entries(changes)) { if(value)next.set(key,value);else next.delete(key); }
    setParams(next,{replace:true});
  }
  const [note,setNote] = useState('');
  const [failure,setFailure] = useState<string|null>(null);
  const actionBusy = useRef(false);

  const { data: issue, error, loading, reload } = useScopedAsyncData(issueId, () => api.getIssue(issueId));
  const occurrences = useScopedAsyncData(JSON.stringify([issueId,occurrenceOffset]), () => api.listOccurrences(issueId,occurrenceOffset,50));
  const occ = occurrences.data;
  const currentId = params.get('occurrence') || occ?.events[0]?.id;
  const event = useScopedAsyncData(currentId ?? '', () => currentId ? api.getEvent(currentId) : Promise.resolve(null));
  // A pasted occurrence ID is not evidence for this issue, even if the viewer
  // can access its project. Never combine its payload with this issue's header.
  const current = event.data?.issueId === issueId ? event.data : null;
  const context = useScopedAsyncData(current?.id ?? '', () => current ? api.eventContext(current.id) : Promise.resolve({events:[]}));
  const ctx = context.data;

  const [replaySeek, setReplaySeek] = useState<{ id: string; t: number; n: number } | null>(null);
  // Frame moments arrive from whichever player loaded; tagged with
  // the event they belong to so switching events never shows stale
  // ticks (and needs no reset effect).
  const [framesFor, setFramesFor] = useState<{ id: null | string; times: number[] }>({
    id: null,
    times: [],
  });
  // The replay playhead, for highlighting the signal list.
  const [playhead, setPlayhead] = useState<{id:string; t:number}|null>(null);
  const [resolveRelease, setResolveRelease] = useState('');
  const [busy, setBusy] = useState(false);

  const act = async (fn: () => Promise<unknown>) => {
    if (actionBusy.current) return false;
    actionBusy.current=true; setBusy(true); setFailure(null);
    try {
      await fn();
      reload();
      onChanged?.();
      return true;
    } catch (error) { setFailure(formatApiError(error)); return false; }
    finally { setBusy(false); actionBusy.current=false; }
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
    !current || screensLatest || wireframeLatest
      ? null
      : (occ?.events.find((e) => e.screensRef) ?? null);
  const screensRef = screensLatest ?? screensFallback?.screensRef ?? null;
  const replaySeekable = !!(screensLatest || wireframeLatest);
  const seekReplay = (tv: number) =>
    setReplaySeek((prev) => ({ id: currentId ?? '', t: tv, n: (prev?.n ?? 0) + 1 }));
  const frameTimes = framesFor.id === currentId ? framesFor.times : [];
  const reportFrames = useCallback(
    (times: number[]) => setFramesFor({ id: currentId ?? null, times }),
    [currentId],
  );
  const reportTime = useCallback((tv: number) => setPlayhead({id:currentId ?? '',t:tv}), [currentId]);
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
        <div className="text-sm text-fg-subtle">
          <ErrorBanner reason={error}>{t('issue.loadFailed')}</ErrorBanner>
          <button type="button" className="underline" onClick={reload}>
            {t('common.retry')}
          </button>
        </div>
      </div>
    );
  }
  if ((!issue && loading) || !issue) {
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
  const stack = Array.isArray(payload?.error?.stack) ? payload.error.stack.filter(frame=>frame&&typeof frame==='object') : [];
  const hasStack = stack.length > 0;
  const device = payload?.device;
  const signals = (Array.isArray(payload?.signals) ? payload.signals : [])
    .filter(signal=>signal&&typeof signal.t==='number'&&Number.isFinite(signal.t)&&typeof signal.kind==='string')
    .map(signal=>({...signal,data:signal.data&&typeof signal.data==='object'?signal.data:undefined}))
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
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      <Link className="text-link text-sm" to={`/tasks?project=${issue.projectId}&issue=${issue.id}`}>{t('team.issueTask')}</Link>
      <Heading title={headline} description={issue.id} action={<div className="flex flex-wrap gap-2"><KindBadge kind={issue.kind}/><Badge tone={issue.status==='resolved'?'success':'neutral'}>{t(`inbox.group.${issue.status}`)}</Badge>{issue.regressedAt&&issue.status==='open'&&<RegressedBadge/>}</div>}/>
      {subline&&<p className="muted mb-5 break-words">{subline}</p>}
      <div className="control-bar">
        <Field label={t('detail.assignee')}><Select disabled={busy||loading} value={issue.assigneeUserId ?? ''} onChange={e=>void act(()=>api.assignIssue(issue.id,e.target.value||null))}><option value="">{t('detail.unassigned')}</option>{issue.assigneeUserId&&!issue.assignees.some(user=>user.id===issue.assigneeUserId)&&<option value={issue.assigneeUserId}>{t('detail.previousAssignee',{id:issue.assigneeUserId})}</option>}{issue.assignees.map(user=><option key={user.id} value={user.id}>{user.displayName||user.email}</option>)}</Select></Field>
        <Field label={t('issue.resolveInRelease')}><Input disabled={busy} value={resolveRelease} onChange={e=>setResolveRelease(e.target.value)} placeholder={issue.lastRelease ? formatRelease(issue.lastRelease) : undefined}/></Field>
        {issue.status==='open'?<><Button variant="primary" disabled={busy||loading} onClick={()=>void act(()=>api.resolveIssue(issue.id,resolveRelease||undefined))}>{t('issue.resolve')}</Button><Button disabled={busy||loading} onClick={()=>void act(()=>api.ignoreIssue(issue.id))}>{t('issue.ignore')}</Button></>:<Button disabled={busy||loading} onClick={()=>void act(()=>api.reopenIssue(issue.id))}>{t('issue.reopen')}</Button>}
        {issue.usersCount>0&&<Link className="text-link" to={`/push?project=${issue.projectId}&tab=audience&issue=${issue.id}`}>{t('issue.notifyAffected')}</Link>}
      </div>
      {failure&&<ErrorBanner reason={failure}>{t('queue.actionFailed')}</ErrorBanner>}
      {issue.status==='resolved'&&<Surface className="mb-6"><p>{issue.resolvedInRelease?t('issue.guardAnchored',{release:issue.resolvedInRelease}):t('issue.guardUnanchored')}</p><p className="muted mt-2">{t('issue.guardProbeHint')}</p></Surface>}
      <div className="metrics-grid mb-6">
        <Metric label={t('queue.events')} value={issue.eventCount.toLocaleString()}/>
        <Metric label={t('queue.users')} value={issue.usersCount?issue.usersCount.toLocaleString():'—'} note={issue.usersCount?undefined:t('queue.usersUnknown')}/>
        <Metric label={t('detail.firstSeen')} value={<TimeAgo iso={issue.firstSeen}/>}/>
        <Metric label={t('detail.lastReceived')} value={issue.lastReceivedAt?<TimeAgo iso={issue.lastReceivedAt}/>:'—'}/>
      </div>
      <Tabs value={tab} onChange={value=>updateParams({tab:value})} items={[["evidence",t('detail.evidence')],["occurrences",t('issue.occurrences')],["activity",t('detail.activity')]]}>
      {tab==='evidence'&&<>
        <div className="control-bar"><Button disabled={!current} onClick={()=>setRawOpen(true)}>{t('detail.rawEvent')}</Button><Field label={t('detail.event')}><Select value={currentId??''} onChange={e=>updateParams({occurrence:e.target.value})} disabled={occurrences.loading||!occ?.events.length}><option value="" disabled>{t('detail.selectEvent')}</option>{currentId&&!occ?.events.some(row=>row.id===currentId)&&<option value={currentId}>{currentId}</option>}{occ?.events.map(row=><option key={row.id} value={row.id}>{row.id} · {new Date(row.receivedAt).toLocaleString()}</option>)}</Select></Field></div>
        {occurrences.error&&<ErrorBanner reason={occurrences.error}>{t('detail.eventsFailed')}</ErrorBanner>}
        {event.error&&<div><ErrorBanner reason={event.error}>{t('detail.eventFailed')}</ErrorBanner><Button onClick={event.reload}>{t('common.retry')}</Button></div>}
        {event.data&&!current&&<ErrorBanner reason={t('detail.foreignEvent')}>{t('detail.eventFailed')}</ErrorBanner>}
        {current&&<div className="flex flex-wrap gap-x-6 gap-y-2 text-xs muted mb-5" data-event-id={current.id}>
          <span>{t('detail.lastReceived')}: <TimeAgo iso={current.receivedAt}/></span>
          <span>{t('issue.deviceClock')}: <TimeAgo iso={current.occurredAt}/></span>
          <span>{formatRelease(current.release)} · {current.environment} · {platformLabel(current.platform,t)}</span>
          <span>{[device?.model,device?.os,device?.osVersion,surface.screen,surface.element].filter(Boolean).map(String).join(' · ')}</span>
        </div>}
        {event.loading&&currentId&&<p role="status">{t('projects.loading')}</p>}
        {context.error&&<ErrorBanner reason={context.error}>{t('detail.contextFailed')}</ErrorBanner>}
        {current&&<>
      {/* The same evidence order at every width: code first, then replay.
          One scroll container keeps evidence reachable on a phone. */}
      <div className="min-h-0 flex-1 overflow-y-auto">
        <div className="grid grid-cols-1 items-start gap-4 p-3 sm:p-4 xl:grid-cols-[minmax(0,1fr)_minmax(260px,0.8fr)]">
          <section data-evidence-code className="min-w-0">
            <Panel title={t('issue.code')}>
              {hasStack ? (
                <StackTrace key={current.id} frames={stack} />
              ) : (
                <PanelEmpty>{t('issue.codeNone')}</PanelEmpty>
              )}
            </Panel>
          </section>
          <section className="min-w-0 space-y-4">
            <Panel
              title={t('replay.title')}
              action={
                screensFallback ? (
                  <span className="truncate text-xs normal-case tracking-normal text-fg-subtle">
                    {t('issue.replayFrom', {
                      // bare-relative: interpolated into a sentence, no element to hold a title
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
                  key={currentId+':'+screensRef}
                  attachmentRef={screensRef}
                  seek={replaySeek?.id===currentId?replaySeek:null}
                  onFrames={screensLatest ? reportFrames : undefined}
                  onTime={screensLatest ? reportTime : undefined}
                  taps={screensLatest ? taps : undefined}
                />
              ) : wireframeLatest ? (
                <>
                  <WireframePlayer
                    key={currentId+':'+wireframeLatest}
                    attachmentRef={wireframeLatest}
                    eventAt={eventAtMs}
                    seek={replaySeek?.id===currentId?replaySeek:null}
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

          </section>
          <div className="min-w-0 space-y-4 xl:col-span-2">
            <Panel title={`${t('issue.signals')}${signals.length ? ` (${signals.length})` : ''}`}>
              {signals.length === 0 ? (
                <PanelEmpty>{t('issue.signalsNone')}</PanelEmpty>
              ) : (
                <SignalList
                  signals={signals}
                  playhead={playhead&&playhead.id===currentId?playhead.t:null}
                  onSeek={replaySeekable ? seekReplay : undefined}
                />
              )}
            </Panel>

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
      <button type="button" aria-expanded={timelineOpen} aria-controls="issue-timeline" onClick={() => setTimelineOpen(!timelineOpen)} className="min-h-10 shrink-0 border-t border-border px-4 text-left text-xs text-fg-muted sm:hidden">
        {t(timelineOpen ? 'triage.hideTimeline' : 'triage.showTimeline')}
      </button>
      <div id="issue-timeline" className={clsx('shrink-0', timelineOpen ? 'block' : 'hidden sm:block')}>
      <TimelineStrip
        signals={signals as StripSignal[]}
        frameTimes={frameTimes}
        context={contextEvents}
        issueKind={issue.kind}
        onSeek={replaySeekable ? seekReplay : undefined}
      />
      </div>
      </>}
      {!currentId&&!occurrences.loading&&!occurrences.error&&<Surface><p>{t('detail.noEvents')}</p></Surface>}
      </>}
      {tab==='occurrences'&&<Surface body={false}>
        {occurrences.error&&<ErrorBanner reason={occurrences.error}>{t('detail.eventsFailed')}</ErrorBanner>}
        {occurrences.loading?<p className="p-6" role="status">{t('projects.loading')}</p>:occ?.events.length?<OccurrenceList rows={occ.events} currentId={currentId??null} onPick={id=>updateParams({occurrence:id,tab:'evidence'})}/>:<p className="p-6">{t('detail.noEvents')}</p>}
        <div className="table-pagination"><span>{occ?t('queue.pageCount',{first:String(occ.events.length?occurrenceOffset+1:0),last:String(occ.events.length?occurrenceOffset+occ.events.length:0),total:String(occ.total)}):'—'}</span><div className="flex gap-2"><Button small disabled={occurrences.loading||occurrenceOffset===0} onClick={()=>updateParams({occurrenceOffset:String(Math.max(0,occurrenceOffset-50))})}>{t('queue.previous')}</Button><Button small disabled={occurrences.loading||occ?.nextOffset==null} onClick={()=>updateParams({occurrenceOffset:String(occ!.nextOffset)})}>{t('queue.next')}</Button></div></div>
      </Surface>}
      {tab==='activity'&&<div className="section-stack"><Surface title={t('detail.addNote')}><form className="form-stack" onSubmit={async e=>{e.preventDefault();if(await act(()=>api.addNote(issue.id,note)))setNote('');}}><Field label={t('detail.note')}><textarea className="input" required maxLength={10000} value={note} disabled={busy} onChange={e=>setNote(e.target.value)}/></Field><Button type="submit" variant="primary" disabled={busy||!note.trim()}>{t('detail.saveNote')}</Button></form></Surface><Surface title={t('detail.activity')}><p className="subtle text-xs mb-4">{t('detail.activityLimit')}</p>{issue.activity.length?<ol className="space-y-5">{issue.activity.map(item=><li key={item.id}><div className="flex flex-wrap items-center gap-3 text-sm"><Badge>{t(`detail.activity.${item.kind}`)}</Badge><span>{item.actorEmail??t('detail.system')}</span><TimeAgo iso={item.at}/></div><p className="mt-2 whitespace-pre-wrap break-words text-sm">{item.kind==='note'?String(item.body.text??''):item.kind==='assign'?(issue.assignees.find(user=>user.id===item.body.assignee)?.displayName||issue.assignees.find(user=>user.id===item.body.assignee)?.email||String(item.body.assignee??t('detail.unassigned'))):JSON.stringify(item.body)}</p></li>)}</ol>:<p>{t('detail.noActivity')}</p>}</Surface></div>}
      </Tabs>
      <Dialog open={rawOpen&&!!current} title={t('detail.rawEvent')} closeLabel={t('common.close')} onClose={()=>setRawOpen(false)}>{current&&<Code key={current.id} t={sharedT} code={JSON.stringify(current,null,2)} language="json" fileName={current.id+'.json'}/>}</Dialog>
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
        <div key={r.release} className="flex flex-wrap items-center gap-3 px-3.5 py-1.5 text-xs">
          <span
            className="w-28 sm:w-56 shrink-0 truncate font-mono text-fg"
            title={r.release}
          >
            {formatReleaseIn(
              r.release,
              rows.map((x) => x.release),
            )}
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
            <TimeAgo iso={r.lastAt} />
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
  const t = useT();
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
      {/* A div rather than a button, because the row ends in a UserChip
          and that is a real button — the clipboard is behind it. A
          button inside a button is invalid HTML, and React says so on
          every render of this list. Same shape the issue list uses for
          the same reason. */}
      {rows.map((r) => (
        <div
          key={r.id}
          role="button"
          tabIndex={0}
          onClick={() => onPick(r.id)}
          onKeyDown={(e) => {
            if ((e.key === 'Enter' || e.key === ' ') && e.target === e.currentTarget) { e.preventDefault(); onPick(r.id); }
          }}
          className={clsx(
            'flex w-full flex-wrap cursor-pointer items-center gap-4 px-3.5 py-2 text-left text-sm transition-colors',
            r.id === currentId ? 'bg-raised' : 'hover:bg-raised/50',
          )}
        >
          <span
            className={clsx(
              'w-16 shrink-0 tabular-nums',
              r.id === currentId ? 'font-semibold text-fg' : 'text-fg-muted',
            )}
          >
            <TimeAgo iso={r.receivedAt} />
          </span>
          <span className={dim(varies.platform)}>{platformLabel(r.platform, t)}</span>
          <span
            className={clsx('min-w-0 truncate font-mono text-xs', dim(varies.release))}
            title={r.release}
          >
            {formatReleaseIn(
              r.release,
              rows.map((x) => x.release),
            )}
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
        </div>
      ))}
    </div>
  );
}

function Panel({title,action,children}:{title:ReactNode;action?:ReactNode;children:ReactNode}) { return <section className="surface"><header className="surface-head"><h2>{title}</h2>{action}</header>{children}</section>; }
