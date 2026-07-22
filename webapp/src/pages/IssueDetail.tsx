// Single issue detail — meta + matching events tail.

import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';

import {
  api,
  EventDetail,
  EventRow,
  IssueDetail as Issue,
  MemberRow,
  UserReport,
} from '../lib/api';
import { EventEvidence } from '../components/crash/EventEvidence';
import { useT } from '../i18n';
import { useKeyHandlers } from '../lib/useShortcuts';
import {
  Badge,
  Button,
  Card,
  CardBody,
  CardHeader,
  ErrorBanner,
  LinkButton,
  PageHeader,
  Select,
  formatNumber,
  formatRelative,
} from '../components/ui';

export default function IssueDetail() {
  const { id: projectId, issueId } = useParams<{
    id: string;
    issueId: string;
  }>();
  const [issue, setIssue] = useState<Issue | null>(null);
  const [events, setEvents] = useState<EventRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const t = useT();
  const [watchers, setWatchers] = useState<string[]>([]);
  // The latest matching event, loaded in full. This is the crash the
  // page is actually about — the issue row is just its aggregate.
  const [latest, setLatest] = useState<EventDetail | null>(null);
  const [selectedEventId, setSelectedEventId] = useState<string | null>(null);
  const myUserId =
    typeof localStorage !== 'undefined'
      ? localStorage.getItem('sentori_user_id')
      : null;
  const watching = myUserId ? watchers.includes(myUserId) : false;

  // Members, for the assignee picker. Failing to load them leaves the
  // picker with just "Nobody" rather than breaking the page — you can
  // still read the crash.
  const [members, setMembers] = useState<MemberRow[]>([]);
  const [reports, setReports] = useState<UserReport[]>([]);
  useEffect(() => {
    if (!projectId || !issueId) return;
    api
      .listUserReports(projectId, { issue_id: issueId })
      .then(r => setReports(r.reports))
      .catch(() => setReports([]));
  }, [projectId, issueId]);

  useEffect(() => {
    api
      .listMembers()
      .then(r => setMembers(r.members))
      .catch(() => {});
  }, []);

  useEffect(() => {
    if (!issueId) return;
    api
      .listWatchers(issueId)
      .then(r => setWatchers(r.watchers.map(w => w.user_id)))
      .catch(() => {});
  }, [issueId]);

  async function toggleWatch() {
    if (!issueId || !myUserId) return;
    try {
      if (watching) {
        await api.unwatchIssue(issueId);
        setWatchers(ws => ws.filter(w => w !== myUserId));
      } else {
        await api.watchIssue(issueId);
        setWatchers(ws => [...ws, myUserId]);
      }
    } catch (e) {
      setError(String(e));
    }
  }

  useKeyHandlers({
    e: () => act('resolved'),
    i: () => act('ignored'),
    r: () => act('active'),
    w: () => toggleWatch(),
  });

  /** Any subset of the triage fields, then re-read so the page shows
   *  what the server stored rather than what we hoped it would. */
  async function setField(patch: {
    priority?: Issue['priority'];
    labels?: string[];
    assignee_user_id?: string | null;
  }) {
    if (!projectId || !issueId) return;
    setBusy(true);
    try {
      await api.patchIssue(projectId, issueId, patch);
      setIssue(await api.getIssue(projectId, issueId));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function act(status: 'active' | 'resolved' | 'ignored') {
    if (!projectId || !issueId) return;
    setBusy(true);
    try {
      await api.patchIssue(projectId, issueId, { status });
      const next = await api.getIssue(projectId, issueId);
      setIssue(next);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    if (!projectId || !issueId) return;
    Promise.all([
      api.getIssue(projectId, issueId),
      api.listEvents(projectId, { issue_id: issueId, limit: 50 }),
    ])
      .then(([i, e]) => {
        setIssue(i);
        setEvents(e);
      })
      .catch(e => setError(String(e)))
      .finally(() => setLoading(false));
  }, [projectId, issueId]);

  // Load the selected event — or the newest one — in full. The list
  // rows carry only identifiers; the payload with the stack,
  // breadcrumbs and context comes from the single-event endpoint.
  useEffect(() => {
    if (!projectId) return;
    const target = selectedEventId ?? events[0]?.id;
    if (!target) return;
    let cancelled = false;
    api
      .getEvent(projectId, target)
      .then(d => {
        if (!cancelled) setLatest(d);
      })
      .catch(e => {
        if (!cancelled) setError(String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [projectId, events, selectedEventId]);

  if (!projectId || !issueId) {
    return <ErrorBanner>{t('common.missingIds')}</ErrorBanner>;
  }
  if (loading) {
    return (
      <div className="py-16 text-center text-sm text-fg-subtle">Loading…</div>
    );
  }
  if (error) {
    return <ErrorBanner>{error}</ErrorBanner>;
  }
  if (!issue) {
    return <ErrorBanner>{t('crash.notFound')}</ErrorBanner>;
  }

  return (
    <div className="space-y-4">
      <PageHeader
        title={issue.error_type}
        subtitle={issue.message_sample}
        actions={
          <div className="flex items-center gap-2">
            {issue.status !== 'resolved' && (
              <Button
                size="sm"
                onClick={() => act('resolved')}
                disabled={busy}
              >
                {t('issues.resolve')}
              </Button>
            )}
            {issue.status !== 'ignored' && (
              <Button
                size="sm"
                variant="secondary"
                onClick={() => act('ignored')}
                disabled={busy}
              >
                {t('issues.ignore')}
              </Button>
            )}
            {issue.status !== 'active' && (
              <Button
                size="sm"
                variant="secondary"
                onClick={() => act('active')}
                disabled={busy}
              >
                {t('issues.reopen')}
              </Button>
            )}
            {myUserId && (
              <Button
                size="sm"
                variant={watching ? 'primary' : 'secondary'}
                onClick={toggleWatch}
              >
                {watching ? `★ ${t('crash.watching')} (${watchers.length})` : `☆ ${t('crash.watch')} (${watchers.length})`}
              </Button>
            )}
            <Button
              size="sm"
              variant="secondary"
              onClick={() => {
                navigator.clipboard?.writeText(issueId);
              }}
            >
              {t('crash.copyId')}
            </Button>
            <Button
              size="sm"
              variant="secondary"
              onClick={() => {
                navigator.clipboard?.writeText(window.location.href);
              }}
            >
              {t('crash.copyLink')}
            </Button>
            <LinkButton
              to={`/projects/${projectId}/issues`}
              size="sm"
              variant="ghost"
            >
              {t('crash.backToAll')}
            </LinkButton>
          </div>
        }
      />

      <Card>
        <CardHeader title={t('crash.meta')} />
        <CardBody>
          <div className="grid grid-cols-4 gap-4">
            <Cell label={t('crash.status')}>
              <Badge
                tone={
                  issue.status === 'resolved'
                    ? 'ok'
                    : issue.status === 'regressed'
                      ? 'warn'
                      : 'neutral'
                }
              >
                {t(`status.${issue.status}`)}
              </Badge>
            </Cell>
            {/* Priority and assignee are editable in place. They are
                the two fields an operator changes while reading the
                crash, and a round trip to a separate form to set one
                dropdown is the reason nobody triages. */}
            <Cell label={t('crash.priority')}>
              <Select
                value={issue.priority}
                disabled={busy}
                onChange={e => setField({ priority: e.target.value as Issue['priority'] })}
                className="w-full"
              >
                {(['p0', 'p1', 'p2', 'p3'] as const).map(p => (
                  <option key={p} value={p}>
                    {t(`priority.${p}`)}
                  </option>
                ))}
              </Select>
            </Cell>
            <Cell label={t('crash.assignee')}>
              <Select
                value={issue.assignee_user_id ?? ''}
                disabled={busy}
                onChange={e =>
                  setField({ assignee_user_id: e.target.value || null })
                }
                className="w-full"
              >
                <option value="">{t('crash.unassigned')}</option>
                {members.map(m => (
                  <option key={m.user_id} value={m.user_id}>
                    {m.email ?? m.user_id.slice(0, 8)}
                  </option>
                ))}
              </Select>
            </Cell>
            <Cell label={t('crash.kind')}>
              <span className="font-mono text-xs">{issue.kind}</span>
            </Cell>
            {/* Labels replace as a set, so removing one means sending
                the rest — the server takes what the issue should end up
                with rather than a delta. */}
            <Cell label={t('crash.labels')}>
              <div className="flex flex-wrap items-center gap-1">
                {issue.labels.length === 0 && (
                  <span className="text-xs text-fg-subtle">
                    {t('crash.noLabels')}
                  </span>
                )}
                {issue.labels.map(l => (
                  <button
                    key={l}
                    type="button"
                    disabled={busy}
                    title={t('action.remove')}
                    onClick={() =>
                      setField({ labels: issue.labels.filter(x => x !== l) })
                    }
                    className="inline-flex items-center gap-1 rounded bg-raised px-1.5 py-0.5 text-xs text-fg-muted hover:text-fg disabled:opacity-50"
                  >
                    {l} <span aria-hidden>×</span>
                  </button>
                ))}
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => {
                    const next = prompt(t('crash.addLabel'))?.trim();
                    if (next && !issue.labels.includes(next)) {
                      void setField({ labels: [...issue.labels, next] });
                    }
                  }}
                  className="rounded border border-dashed border-border-strong px-1.5 py-0.5 text-xs text-fg-subtle hover:text-fg disabled:opacity-50"
                >
                  +
                </button>
              </div>
            </Cell>
            <Cell label={t('issues.colEvents')}>{formatNumber(issue.event_count)}</Cell>
            <Cell label={t('crash.lastRelease')}>
              <span className="font-mono text-xs">
                {issue.last_release || '—'}
              </span>
            </Cell>
            <Cell label={t('crash.firstSeen')}>{formatRelative(issue.first_seen)}</Cell>
            <Cell label={t('crash.lastSeen')}>{formatRelative(issue.last_seen)}</Cell>
            <Cell label={t('crash.environment')}>
              <span className="font-mono text-xs">{issue.last_environment || '—'}</span>
            </Cell>
            <Cell label={t('crash.fingerprint')}>
              <span className="font-mono text-xs break-all">
                {issue.fingerprint.slice(0, 16)}…
              </span>
            </Cell>
            {issue.resolved_at && (
              <Cell label={t('crash.resolvedAt')}>
                {formatRelative(issue.resolved_at)}
              </Cell>
            )}
            {issue.regressed_at && (
              <Cell label={t('crash.regressedAt')}>
                {formatRelative(issue.regressed_at)}
                {issue.regressed_in_release && (
                  <span className="font-mono text-xs text-fg-subtle ml-1">
                    in {issue.regressed_in_release}
                  </span>
                )}
              </Cell>
            )}
          </div>
        </CardBody>
      </Card>

      {/* Evidence before discussion. The stack, the breadcrumbs and the
          replay are what the page is for — someone opening a crash is
          reading the error, not the thread about it. An empty comment
          box sitting between the summary and the stack trace pushed
          the first useful thing below the fold. */}
      {latest ? (
        <div className="space-y-8">
          {events.length > 1 && (
            <EventPicker
              events={events}
              selected={latest.id}
              onSelect={setSelectedEventId}
            />
          )}
          <EventEvidence event={latest} projectId={projectId} />
        </div>
      ) : (
        <p className="py-8 text-center text-sm text-fg-subtle">
          {t('crash.noEvent')}
        </p>
      )}

      {/* A stack trace says where; this says what they were trying to
          do. It is the only signal in the product that is not machine
          generated, so it sits with the evidence rather than with the
          team's own discussion below. */}
      {reports.length > 0 && (
        <Card>
          <CardHeader
            title={t('crash.userReports')}
            subtitle={t('crash.userReportsHint')}
          />
          <CardBody>
            <ul className="divide-y divide-border">
              {reports.map(r => (
                <li key={r.id} className="py-3">
                  <div className="flex items-baseline gap-2">
                    <span className="text-sm font-medium text-fg">
                      {r.name || r.email || '—'}
                    </span>
                    <span className="text-xs text-fg-subtle">
                      {formatRelative(r.received_at)}
                    </span>
                  </div>
                  {r.title && (
                    <p className="mt-1 text-sm text-fg">{r.title}</p>
                  )}
                  {r.body && (
                    <p className="mt-1 whitespace-pre-wrap text-sm text-fg-muted">
                      {r.body}
                    </p>
                  )}
                </li>
              ))}
            </ul>
          </CardBody>
        </Card>
      )}

      <Comments issueId={issueId} myUserId={myUserId} />
      <Activity issueId={issueId} />
    </div>
  );
}

function Cell({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <p className="text-xs uppercase tracking-wide text-fg-subtle">
        {label}
      </p>
      <div className="mt-1 text-sm">{children}</div>
    </div>
  );
}

function Comments({
  issueId,
  myUserId,
}: {
  issueId: string;
  myUserId: string | null;
}) {
  // Derived from the client rather than restated here. This was a
  // hand-copy of the response shape and it fell behind the moment the
  // server started sending the author's email.
  type Comment = Awaited<ReturnType<typeof api.listComments>>['comments'][number];
  const [rows, setRows] = useState<Comment[]>([]);
  const [text, setText] = useState('');
  const t = useT();
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api
      .listComments(issueId)
      .then(r => setRows(r.comments))
      .catch(() => {});
  }, [issueId]);

  async function post() {
    if (!text.trim()) return;
    setBusy(true);
    try {
      const c = await api.createComment(issueId, text.trim());
      setRows(rs => [...rs, c]);
      setText('');
    } catch {
      /* noop */
    } finally {
      setBusy(false);
    }
  }

  async function del(id: string) {
    if (!confirm(t('crash.deleteComment'))) return;
    try {
      await api.deleteComment(issueId, id);
      setRows(rs => rs.filter(r => r.id !== id));
    } catch {
      /* noop */
    }
  }

  return (
    <Card>
      <CardHeader title={`${t('crash.comments')} (${rows.length})`} />
      <CardBody>
        <div className="space-y-2">
          {rows.map(c => (
            <div
              key={c.id}
              className="rounded border border-border p-2 text-xs"
            >
              <div className="flex items-center justify-between">
                <span className="font-mono text-xs text-fg-subtle">
                  {c.author_email ?? `${c.author_user_id.slice(0, 8)}…`} ·{' '}
                  {formatRelative(c.created_at)}
                </span>
                {myUserId === c.author_user_id && (
                  <button
                    onClick={() => del(c.id)}
                    className="text-xs text-fg-subtle hover:text-danger"
                  >
                    {t('action.delete')}
                  </button>
                )}
              </div>
              <p className="mt-1 whitespace-pre-wrap text-fg">
                {c.body_md}
              </p>
            </div>
          ))}
          {myUserId && (
            <div className="space-y-2 pt-2">
              <textarea
                value={text}
                onChange={e => setText(e.target.value)}
                placeholder={t('crash.addComment')}
                className="w-full h-20 rounded border border-border p-2 text-xs"
              />
              <Button
                size="sm"
                onClick={post}
                disabled={busy || !text.trim()}
              >{t('action.post')}</Button>
            </div>
          )}
        </div>
      </CardBody>
    </Card>
  );
}

function Activity({ issueId }: { issueId: string }) {
  const t = useT();
  const [rows, setRows] = useState<
    {
      id: string;
      actor_user_id: string | null;
      kind: string;
      created_at: string;
    }[]
  >([]);
  useEffect(() => {
    api
      .listActivity(issueId)
      .then(r => setRows(r.activity))
      .catch(() => {});
  }, [issueId]);

  if (rows.length === 0) return null;
  return (
    <Card>
      <CardHeader title={`${t('crash.activity')} (${rows.length})`} />
      <CardBody>
        <ul className="space-y-1 text-xs">
          {rows.map(a => (
            <li
              key={a.id}
              className="flex items-center justify-between text-fg-muted"
            >
              <span>
                <Badge>{a.kind}</Badge>{' '}
                {a.actor_user_id
                  ? a.actor_user_id.slice(0, 8) + '…'
                  : 'system'}
              </span>
              <span className="font-mono text-xs">
                {formatRelative(a.created_at)}
              </span>
            </li>
          ))}
        </ul>
      </CardBody>
    </Card>
  );
}

/** Which occurrence of this issue you are looking at. Same crash,
 *  different device / build / moment — and those differences are
 *  often the whole diagnosis. */
function EventPicker({
  events,
  selected,
  onSelect,
}: {
  events: EventRow[];
  selected: string;
  onSelect: (id: string) => void;
}) {
  return (
    <div className="flex flex-wrap gap-1.5">
      {events.slice(0, 12).map(e => {
        const active = e.id === selected;
        return (
          <button
            key={e.id}
            type="button"
            onClick={() => onSelect(e.id)}
            aria-current={active}
            className={`inline-flex h-7 items-center rounded border px-2 font-mono text-xs transition focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent ${
              active
                ? 'border-accent text-fg'
                : 'border-border text-fg-subtle hover:text-fg-muted'
            }`}
          >
            {formatRelative(e.timestamp)}
            <span className="ml-1.5 text-fg-subtle">{e.platform}</span>
          </button>
        );
      })}
    </div>
  );
}
