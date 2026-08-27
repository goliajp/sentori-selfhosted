// The triage split — the app's home. A master-detail pair on one
// fixed viewport: the queue on the left (regressed, then new, then
// open — ordered as the eye should read), the case file on the
// right. Selecting never leaves the queue; j/k walk it, Enter/click
// select, x marks, r/i resolve/ignore. Filters live in the URL so a
// reload or a shared link lands on the same view.

import { useEffect, useMemo, useRef, useState } from 'react';
import { Link, useNavigate, useParams, useSearchParams } from 'react-router-dom';

import { useShell } from '../App';
import { ImpactCell, KindBadge, kindColor } from '../components/kind';
import { ErrorBanner, Kbd, clsx, formatRelative } from '../components/ui';
import { useT } from '../i18n';
import { api, type IssueSummary } from '../lib/api';
import { issueHeadline } from '../lib/issue-title';
import { useAsyncData } from '../lib/useAsyncData';

import { IssueDetailPane } from './IssueDetail';

const KINDS = ['error', 'warn', 'trace', 'assert', 'probe'] as const;

export default function TriageView() {
  const t = useT();
  const { activeProject } = useShell();
  const { issueId } = useParams<{ issueId: string }>();
  const navigate = useNavigate();
  const [search, setSearch] = useSearchParams();

  const status = (search.get('status') ?? 'open') as 'ignored' | 'open' | 'resolved';
  const kind = search.get('kind');
  const env = search.get('env');
  // Context dimension: a host-defined key/value pair. Sentori knows
  // nothing about what `qa` or `tenant` mean — it only slices.
  const ctxKey = search.get('ck');
  const ctxVal = search.get('cv');
  const rel = search.get('rel');
  const projectId = activeProject?.id ?? null;

  // Deployment environments actually seen — the env filter's options.
  const { data: envData } = useAsyncData(
    () =>
      projectId
        ? api.projectEnvironments(projectId)
        : Promise.resolve({ environments: [] }),
    [projectId],
  );
  const environments = envData?.environments ?? [];
  // `lastEventAt === null` is the only way to tell a project that has
  // never been connected from one whose queue is simply clear. Without
  // it both read as "Inbox zero", which congratulates an operator who
  // has not finished setting up and drops the guidance at the exact
  // moment it is needed.
  const { data: healthData } = useAsyncData(
    () => (projectId ? api.projectHealth(projectId) : Promise.resolve(null)),
    [projectId],
  );
  const neverReceived = Boolean(projectId) && healthData?.lastEventAt == null;
  const { data: ckData } = useAsyncData(
    () =>
      projectId
        ? api.projectContextKeys(projectId)
        : Promise.resolve({ keys: [] }),
    [projectId],
  );
  const contextKeys = ckData?.keys ?? [];
  const { data: cvData } = useAsyncData(
    () =>
      projectId && ctxKey
        ? api.projectContextValues(projectId, ctxKey)
        : Promise.resolve({ values: [] }),
    [projectId, ctxKey],
  );
  const contextValues = cvData?.values ?? [];
  // Release options for the version filter (release stays out of
  // aggregation; this slices the queue).
  const { data: relData } = useAsyncData(
    () =>
      projectId
        ? api.listReleases(projectId)
        : Promise.resolve({ releases: [] }),
    [projectId],
  );
  const releaseNames = (relData?.releases ?? []).map((r) => r.name);

  const setFilter = (key: string, value: string | null) => {
    const next = new URLSearchParams(search);
    if (value === null) next.delete(key);
    else next.set(key, value);
    setSearch(next, { replace: true });
    setChecked(new Set());
  };
  /** Key + value move together: picking a new key clears the value,
   *  clearing the key clears both. */
  const setCtx = (key: null | string, value: null | string) => {
    const next = new URLSearchParams(search);
    if (key === null) next.delete('ck');
    else next.set('ck', key);
    if (value === null) next.delete('cv');
    else next.set('cv', value);
    setSearch(next, { replace: true });
    setChecked(new Set());
  };

  const { data, error, loading, reload } = useAsyncData(
    () =>
      api.listIssues({
        status,
        kind: kind ?? undefined,
        projectId: projectId ?? undefined,
        environment: env ?? undefined,
        contextKey: ctxKey ?? undefined,
        contextValue: ctxVal ?? undefined,
        release: rel ?? undefined,
        limit: 200,
      }),
    [status, kind, env, ctxKey, ctxVal, rel, projectId],
  );

  // Captured per render, not inside the memo (react-hooks/purity).
  const [now] = useState(() => Date.now());
  const groups = useMemo(() => {
    const issues = data?.issues ?? [];
    const dayAgo = now - 24 * 3600 * 1000;
    const regressed = issues.filter((i) => i.regressedAt && i.status === 'open');
    const rest = issues.filter((i) => !(i.regressedAt && i.status === 'open'));
    const fresh = rest.filter((i) => new Date(i.firstSeen).getTime() > dayAgo);
    const open = rest.filter((i) => new Date(i.firstSeen).getTime() <= dayAgo);
    return { regressed, fresh, open };
  }, [data, now]);

  const flat = useMemo(
    () => [...groups.regressed, ...groups.fresh, ...groups.open],
    [groups],
  );

  // ── selection + keyboard triage ──
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const flatRef = useRef(flat);
  useEffect(() => {
    flatRef.current = flat;
  }, [flat]);
  const selectedRef = useRef(issueId);
  useEffect(() => {
    selectedRef.current = issueId;
  }, [issueId]);

  const open = (id: string) => {
    navigate({ pathname: `/issues/${id}`, search: search.toString() });
  };
  const openRef = useRef(open);
  useEffect(() => {
    openRef.current = open;
  });

  const checkedRef = useRef(checked);
  useEffect(() => {
    checkedRef.current = checked;
  }, [checked]);

  const toggleChecked = (id: string) =>
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  async function bulkAct(action: 'resolve' | 'ignore', ids: string[]) {
    if (ids.length === 0 || busy) return;
    setBusy(true);
    try {
      await Promise.all(
        ids.map((id) =>
          action === 'resolve' ? api.resolveIssue(id) : api.ignoreIssue(id),
        ),
      );
      setChecked(new Set());
      reload();
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const el = e.target as HTMLElement;
      if (
        e.metaKey ||
        e.ctrlKey ||
        e.altKey ||
        /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName)
      )
        return;
      const list = flatRef.current;
      if (list.length === 0) return;
      const at = list.findIndex((i) => i.id === selectedRef.current);
      if (e.key === 'j') {
        openRef.current(list[Math.min(at + 1, list.length - 1)].id);
      } else if (e.key === 'k') {
        openRef.current(list[Math.max(at - 1, 0)].id);
      } else if (e.key === 'x') {
        if (at >= 0) toggleChecked(list[at].id);
      } else if (e.key === 'r' || e.key === 'i') {
        const ids =
          checkedRef.current.size > 0
            ? [...checkedRef.current]
            : at >= 0
              ? [list[at].id]
              : [];
        void bulkAct(e.key === 'r' ? 'resolve' : 'ignore', ids);
      } else {
        return;
      }
      e.preventDefault();
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
    // refs carry the changing state; the listener lives once.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const todayNew = groups.fresh.length;
  const todayRegressed = groups.regressed.length;

  return (
    <div className="flex h-full min-w-0">
      {/* ── the queue ── */}
      <section className="flex w-[360px] shrink-0 flex-col border-r border-border bg-bg">
        <header className="shrink-0 border-b border-border px-3 py-2">
          <div className="flex items-center gap-1">
            {(['open', 'resolved', 'ignored'] as const).map((s) => (
              <button
                key={s}
                type="button"
                onClick={() => setFilter('status', s === 'open' ? null : s)}
                className={clsx(
                  'shrink-0 whitespace-nowrap rounded px-2 py-1 text-xs transition-colors',
                  status === s
                    ? 'bg-raised font-medium text-fg'
                    : 'text-fg-subtle hover:text-fg-muted',
                )}
              >
                {t(`inbox.status.${s}`)}
              </button>
            ))}
            {/* The count yields, the tabs do not: in Japanese this
                line squeezed 未解決 into two lines rather than give up
                a character of the summary beside it. */}
            <span
              className="ml-auto min-w-0 truncate text-xs tabular-nums text-fg-subtle"
              title={t('inbox.pulse', {
                fresh: String(todayNew),
                regressed: String(todayRegressed),
              })}
            >
              {t('inbox.pulse', {
                fresh: String(todayNew),
                regressed: String(todayRegressed),
              })}
            </span>
          </div>
          <div className="mt-1.5 flex flex-wrap items-center gap-1">
            {/* deployment-environment slice — only when the project
                reports more than one */}
            {environments.length > 1 && (
              <select
                value={env ?? ''}
                onChange={(e) => setFilter('env', e.target.value || null)}
                aria-label={t('inbox.envFilter')}
                className="mr-1 h-[22px] rounded border border-border bg-surface px-1 text-xs text-fg-muted"
              >
                <option value="">{t('inbox.envAll')}</option>
                {environments.map((e) => (
                  <option key={e} value={e}>
                    {e}
                  </option>
                ))}
              </select>
            )}
            {/* host-defined context dimension: pick a key, then a
                value; sentori attaches no meaning to either */}
            {contextKeys.length > 0 && (
              <select
                value={ctxKey ?? ''}
                onChange={(e) => setCtx(e.target.value || null, null)}
                aria-label={t('inbox.ctxKeyFilter')}
                className="h-[22px] rounded border border-border bg-surface px-1 text-xs text-fg-muted"
              >
                <option value="">{t('inbox.ctxKeyNone')}</option>
                {contextKeys.map((k) => (
                  <option key={k} value={k}>
                    {k}
                  </option>
                ))}
              </select>
            )}
            {ctxKey && (
              <select
                value={ctxVal ?? ''}
                onChange={(e) => setCtx(ctxKey, e.target.value || null)}
                aria-label={t('inbox.ctxValueFilter')}
                className="mr-1 h-[22px] rounded border border-border bg-surface px-1 text-xs text-fg-muted"
              >
                <option value="">{t('inbox.ctxValueAll')}</option>
                {contextValues.map((v) => (
                  <option key={v} value={v}>
                    {v}
                  </option>
                ))}
              </select>
            )}
            {releaseNames.length > 0 && (
              <select
                value={rel ?? ''}
                onChange={(e) => setFilter('rel', e.target.value || null)}
                aria-label={t('inbox.releaseFilter')}
                className="mr-1 h-[22px] max-w-40 rounded border border-border bg-surface px-1 text-xs text-fg-muted"
              >
                <option value="">{t('inbox.releaseAll')}</option>
                {releaseNames.map((r) => (
                  <option key={r} value={r}>
                    {r}
                  </option>
                ))}
              </select>
            )}
          </div>
          {/* The five kinds are one control, not five more items in
              the select flow: on their own line they read as a set
              and stay a set in every language, instead of the last
              two spilling under the first three. */}
          <div className="mt-1.5 flex flex-wrap items-center gap-1">
            {KINDS.map((k) => (
              <button
                key={k}
                type="button"
                onClick={() => setFilter('kind', kind === k ? null : k)}
                className={clsx(
                  'flex items-center gap-1.5 rounded border px-1.5 py-0.5 font-mono text-xs transition-colors',
                  kind === k
                    ? 'border-border-strong bg-raised text-fg'
                    : 'border-border text-fg-subtle hover:text-fg-muted',
                )}
              >
                {/* the kind's own hue — the palette teaching its
                    one concept model even in a filter chip */}
                <span
                  aria-hidden
                  className="h-1.5 w-1.5 rounded-full"
                  style={{ backgroundColor: kindColor(k) }}
                />
                {k}
              </button>
            ))}
          </div>
        </header>

        {checked.size > 0 && (
          <div className="flex shrink-0 items-center gap-2 border-b border-border bg-raised px-3 py-1.5 text-xs">
            <span className="font-medium">
              {t('inbox.selectedCount', { n: String(checked.size) })}
            </span>
            <button
              type="button"
              disabled={busy}
              onClick={() => bulkAct('resolve', [...checked])}
              className="rounded bg-accent px-2 py-0.5 font-medium text-accent-fg disabled:opacity-40"
            >
              {t('issue.resolve')}
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => bulkAct('ignore', [...checked])}
              className="rounded border border-border px-2 py-0.5 disabled:opacity-40"
            >
              {t('issue.ignore')}
            </button>
            <button
              type="button"
              onClick={() => setChecked(new Set())}
              className="ml-auto text-fg-subtle hover:text-fg"
            >
              {t('inbox.clearSelection')}
            </button>
          </div>
        )}

        <div className="min-h-0 flex-1 overflow-y-auto">
          {error && (
            <div className="p-3">
              <ErrorBanner>
                {t('inbox.loadFailed')}{' '}
                <button type="button" className="underline" onClick={reload}>
                  {t('common.retry')}
                </button>
              </ErrorBanner>
            </div>
          )}
          {loading && !data && (
            <div className="py-16 text-center text-sm text-fg-subtle">…</div>
          )}
          {data && flat.length === 0 && (
            <div className="px-4 py-16 text-center">
              <p className="text-sm text-fg-muted">
                {!activeProject
                  ? t('inbox.emptyNoProjectTitle')
                  : neverReceived
                    ? t('inbox.awaitingFirstTitle')
                    : t('inbox.emptyTitle')}
              </p>
              <p className="mt-1.5 text-xs text-fg-subtle">
                {!activeProject
                  ? t('inbox.emptyNoProject')
                  : neverReceived
                    ? t('inbox.awaitingFirstHint')
                    : t('inbox.emptyHint')}
              </p>
              {neverReceived && (
                <Link
                  to="/settings?tab=tokens"
                  className="mt-3 inline-block text-xs text-accent underline"
                >
                  {t('inbox.awaitingFirstAction')}
                </Link>
              )}
            </div>
          )}
          {groups.regressed.length > 0 && (
            <QueueGroup label={t('inbox.groupRegressed')} tone="danger">
              {groups.regressed.map((i) => (
                <QueueRow
                  key={i.id}
                  issue={i}
                  active={i.id === issueId}
                  checked={checked.has(i.id)}
                  onToggle={() => toggleChecked(i.id)}
                  onOpen={() => open(i.id)}
                />
              ))}
            </QueueGroup>
          )}
          {groups.fresh.length > 0 && (
            <QueueGroup label={t('inbox.groupNew')}>
              {groups.fresh.map((i) => (
                <QueueRow
                  key={i.id}
                  issue={i}
                  active={i.id === issueId}
                  checked={checked.has(i.id)}
                  onToggle={() => toggleChecked(i.id)}
                  onOpen={() => open(i.id)}
                />
              ))}
            </QueueGroup>
          )}
          {groups.open.length > 0 && (
            <QueueGroup label={t(`inbox.group.${status}`)}>
              {groups.open.map((i) => (
                <QueueRow
                  key={i.id}
                  issue={i}
                  active={i.id === issueId}
                  checked={checked.has(i.id)}
                  onToggle={() => toggleChecked(i.id)}
                  onOpen={() => open(i.id)}
                />
              ))}
            </QueueGroup>
          )}
        </div>
      </section>

      {/* ── the case file ── */}
      <section className="min-w-0 flex-1 overflow-hidden bg-canvas">
        {issueId ? (
          <IssueDetailPane issueId={issueId} onChanged={reload} />
        ) : (
          <div className="flex h-full flex-col items-center justify-center gap-3">
            <p className="text-sm text-fg-muted">{t('triage.pickTitle')}</p>
            <p className="flex items-center gap-3 text-xs text-fg-subtle">
              <span className="flex items-center gap-1">
                <Kbd>j</Kbd>
                <Kbd>k</Kbd> {t('triage.hintMove')}
              </span>
              <span className="flex items-center gap-1">
                <Kbd>⏎</Kbd> {t('triage.hintOpen')}
              </span>
              <span className="flex items-center gap-1">
                <Kbd>⌘K</Kbd> {t('triage.hintSearch')}
              </span>
            </p>
          </div>
        )}
      </section>
    </div>
  );
}

function QueueGroup({
  label,
  tone,
  children,
}: {
  label: string;
  tone?: 'danger';
  children: React.ReactNode;
}) {
  return (
    <div>
      <div
        className={clsx(
          'sticky top-0 z-10 border-b border-border bg-bg px-3 py-1 text-xs font-semibold',
          tone === 'danger' ? 'text-kind-error' : 'text-fg-subtle',
        )}
      >
        {label}
      </div>
      <div className="divide-y divide-border/60">{children}</div>
    </div>
  );
}

function QueueRow({
  issue,
  active,
  checked,
  onToggle,
  onOpen,
}: {
  issue: IssueSummary;
  active: boolean;
  checked: boolean;
  onToggle: () => void;
  onOpen: () => void;
}) {
  const surface = issue.surface as { screen?: string; element?: string };
  const where = [surface.screen, surface.element].filter(Boolean).join(' · ');
  // Same demotion the crash view does: a row headed "Error" tells you
  // nothing, and five of them tell you nothing five times. The class
  // name keeps its place as a chip.
  const { headline, type } = issueHeadline(issue);
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onOpen}
      onKeyDown={(e) => {
        if (e.key === 'Enter') onOpen();
      }}
      data-active={active || undefined}
      className={clsx(
        'group cursor-pointer px-3 py-2 transition-colors',
        active ? 'bg-raised' : 'hover:bg-surface/60',
      )}
    >
      <div className="flex items-center gap-2">
        <input
          type="checkbox"
          checked={checked}
          onClick={(e) => e.stopPropagation()}
          onChange={onToggle}
          className={clsx(
            'h-3.5 w-3.5 shrink-0 accent-accent',
            checked ? '' : 'opacity-0 transition-opacity group-hover:opacity-60',
          )}
        />
        <span className="min-w-0 flex-1 truncate text-sm font-medium text-fg">
          {headline}
        </span>
        <span className="shrink-0 text-xs tabular-nums text-fg-subtle">
          {formatRelative(issue.lastSeen)}
        </span>
      </div>
      <div className="mt-0.5 flex items-center gap-2 pl-[22px]">
        <KindBadge kind={issue.kind} />
        {issue.regressedAt && issue.status === 'open' && (
          <span className="font-mono text-xs font-semibold uppercase text-kind-error">
            regressed
          </span>
        )}
        {/* the class name, once the message has taken the headline */}
        {type && (
          <span className="shrink-0 font-mono text-xs text-fg-muted">{type}</span>
        )}
        {/* the second line's context slot: the surface when we have
            one, else the message — unless the message is already the
            headline, in which case printing it twice says nothing
            twice */}
        {(where || (!type && issue.messageSample)) && (
          <span className="min-w-0 truncate text-xs text-fg-subtle">
            {where || issue.messageSample}
          </span>
        )}
        <span className="ml-auto flex shrink-0 items-baseline gap-2">
          {issue.platform && (
            <span className="text-xs text-fg-subtle">{issue.platform}</span>
          )}
          <ImpactCell
            users={issue.usersCount}
            maxPerUser={issue.maxPerUser}
            events={issue.eventCount}
          />
        </span>
      </div>
    </div>
  );
}
