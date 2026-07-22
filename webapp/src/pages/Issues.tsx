import { useEffect, useState } from 'react';
import { Link, useParams, useSearchParams } from 'react-router-dom';
import { api, ApiError, IngestRequest, Issue } from '../lib/api';
import { useI18n } from '../i18n';
import { useKeyHandlers } from '../lib/useShortcuts';
import { useProjectName } from '../lib/useProjectName';
import {
  Badge,
  Button,
  Card,
  DataTable,
  ErrorBanner,
  PageHeader,
  Tabs,
  formatNumber,
  formatRelative,
} from '../components/ui';

const STATUS_TONE: Record<Issue['status'], 'ok' | 'warn' | 'danger' | 'neutral'> = {
  active: 'danger',
  regressed: 'warn',
  resolved: 'ok',
  ignored: 'neutral',
};

export function IssuesPage() {
  const { id: projectId } = useParams<{ id: string }>();
  const projectName = useProjectName(projectId);
  const { t } = useI18n();
  const [search, setSearch] = useSearchParams();
  const statusFilter = search.get('status') ?? '';
  const [issues, setIssues] = useState<Issue[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState<Set<string>>(new Set());
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [cursor, setCursor] = useState(0);

  // Scroll cursor row into view after j/k navigation.
  useEffect(() => {
    if (!issues?.[cursor]) return;
    const el = document.querySelector(
      `[data-issue-id="${issues[cursor].id}"]`,
    );
    if (el && 'scrollIntoView' in el) {
      (el as HTMLElement).scrollIntoView({
        block: 'nearest',
        behavior: 'smooth',
      });
    }
  }, [cursor, issues]);

  useKeyHandlers({
    j: () => setCursor(c => Math.min((issues?.length ?? 1) - 1, c + 1)),
    k: () => setCursor(c => Math.max(0, c - 1)),
    x: () => {
      if (issues?.[cursor]) {
        const id = issues[cursor].id;
        setSelected(s => {
          const c = new Set(s);
          if (c.has(id)) c.delete(id);
          else c.add(id);
          return c;
        });
      }
    },
    e: () => {
      if (issues?.[cursor]) quickAction(issues[cursor].id, 'resolved');
    },
    i: () => {
      if (issues?.[cursor]) quickAction(issues[cursor].id, 'ignored');
    },
  });

  async function bulkApply(status: 'resolved' | 'ignored') {
    if (!projectId || selected.size === 0) return;
    try {
      await api.bulkPatchIssues(projectId, {
        ids: Array.from(selected),
        status,
      });
      setIssues(rows =>
        rows
          ? rows.map(r =>
              selected.has(r.id) ? { ...r, status } : r,
            )
          : rows,
      );
      setSelected(new Set());
    } catch (e) {
      setErr(String(e));
    }
  }

  useEffect(() => {
    if (!projectId) return;
    api
      .listIssues(projectId, { status: statusFilter || undefined })
      .then(setIssues)
      .catch((e: unknown) => {
        if (e instanceof ApiError) setErr(`${e.status}: ${e.body}`);
        else setErr(String(e));
      });
  }, [projectId, statusFilter]);

  async function quickAction(
    issueId: string,
    next: 'resolved' | 'ignored' | 'active',
  ) {
    if (!projectId) return;
    setBusy(b => new Set(b).add(issueId));
    try {
      await api.patchIssue(projectId, issueId, { status: next });
      setIssues(rows =>
        rows
          ? rows.map(r =>
              r.id === issueId ? { ...r, status: next } : r,
            )
          : rows,
      );
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(b => {
        const c = new Set(b);
        c.delete(issueId);
        return c;
      });
    }
  }

  if (!projectId) return <div>no project id</div>;

  return (
    <div>
      <PageHeader
        title={t('issues.title')}
        subtitle={projectName}
        action={
          <div className="flex gap-2">
            <SaveViewButton
              projectId={projectId}
              statusFilter={statusFilter}
            />
            <TestIngestButton projectId={projectId} />
          </div>
        }
      />

      <p className="mb-3 text-xs text-fg-subtle">
        {t('issues.shortcuts')}: <kbd className="rounded bg-raised px-1">j</kbd>/
        <kbd className="rounded bg-raised px-1">k</kbd> {t('issues.kbdNavigate')} ·{' '}
        <kbd className="rounded bg-raised px-1">x</kbd> {t('issues.kbdSelect')} ·{' '}
        <kbd className="rounded bg-raised px-1">e</kbd> {t('issues.resolve')} ·{' '}
        <kbd className="rounded bg-raised px-1">i</kbd> {t('issues.kbdIgnore')}
      </p>

      {selected.size > 0 && (
        <div className="mb-4 flex items-center gap-2 rounded border border-accent/40 bg-accent/20 px-3 py-2 text-xs">
          <span className="text-fg-muted">
            {selected.size} selected
          </span>
          <Button size="sm" onClick={() => bulkApply('resolved')}>{t('issues.resolveAll')}</Button>
          <Button
            size="sm"
            variant="secondary"
            onClick={() => bulkApply('ignored')}
          >{t('issues.ignoreAll')}</Button>
          <Button
            size="sm"
            variant="secondary"
            onClick={() => setSelected(new Set())}
          >{t('action.clear')}</Button>
        </div>
      )}

      <div className="mb-4">
        <Tabs
          value={statusFilter || 'all'}
          onChange={(v) => {
            if (v === 'all') {
              search.delete('status');
            } else {
              search.set('status', v);
            }
            setSearch(search, { replace: true });
          }}
          options={[
            { value: 'all', label: t('issues.tabAll') },
            { value: 'active', label: t('issues.tabActive') },
            { value: 'regressed', label: t('issues.tabRegressed') },
            { value: 'resolved', label: t('issues.tabResolved') },
            { value: 'ignored', label: t('issues.tabIgnored') },
          ]}
        />
      </div>

      {err && <ErrorBanner>{err}</ErrorBanner>}

      <Card>
        <DataTable
          rowKey={(r) => r.id}
          empty={t('issues.empty')}
          rows={issues ?? []}
          columns={[
            {
              key: 'select',
              label: '',
              width: '4%',
              render: (r) => {
                const idx = issues?.findIndex(x => x.id === r.id) ?? -1;
                const isCursor = idx === cursor;
                return (
                  <div className="flex items-center gap-1">
                    {isCursor && (
                      <span className="text-accent text-xs">▸</span>
                    )}
                    <input
                      type="checkbox"
                      checked={selected.has(r.id)}
                      onChange={e => {
                        setSelected(s => {
                          const c = new Set(s);
                          if (e.target.checked) c.add(r.id);
                          else c.delete(r.id);
                          return c;
                        });
                      }}
                      className="cursor-pointer"
                    />
                  </div>
                );
              },
            },
            {
              key: 'status',
              label: '',
              width: '5%',
              render: (r) => (
                <Badge tone={STATUS_TONE[r.status]}>{t(`status.${r.status}`)}</Badge>
              ),
            },
            {
              key: 'error_type',
              label: t('issues.colIssue'),
              render: (r) => (
                <Link
                  to={`/projects/${projectId}/issues/${r.id}`}
                  className="block hover:bg-surface/40 -m-2 p-2 rounded"
                >
                  <div className="font-medium text-fg">{r.error_type}</div>
                  <div className="font-mono text-xs text-fg-subtle">
                    {r.message_sample.slice(0, 80)}
                  </div>
                </Link>
              ),
            },
            {
              key: 'event_count',
              label: t('issues.colPriority'),
              width: '8%',
              // Sorting a list by urgency is the point of storing a
              // priority; showing it is the minimum that makes the
              // column worth setting.
              render: (r) => (
                <Badge tone={r.priority === 'p0' ? 'danger' : r.priority === 'p1' ? 'warn' : 'neutral'}>
                  {r.priority.toUpperCase()}
                </Badge>
              ),
            },
            {
              key: 'events',
              label: t('issues.colEvents'),
              width: '10%',
              render: (r) => (
                <span className="font-mono tabular-nums">
                  {formatNumber(r.event_count)}
                </span>
              ),
            },
            {
              key: 'last_release',
              label: t('issues.colRelease'),
              width: '15%',
              render: (r) => (
                <span className="font-mono text-xs text-fg-muted">
                  {r.last_release}
                </span>
              ),
            },
            {
              key: 'last_environment',
              label: t('issues.colEnv'),
              width: '10%',
              render: (r) => <Badge>{r.last_environment}</Badge>,
            },
            {
              key: 'last_seen',
              label: t('issues.colLastSeen'),
              width: '12%',
              render: (r) => (
                <span className="text-xs text-fg-subtle">
                  {formatRelative(r.last_seen)}
                </span>
              ),
            },
            {
              key: 'actions',
              label: '',
              width: '14%',
              render: (r) => (
                <div className="flex justify-end gap-1">
                  {r.status !== 'resolved' && (
                    <Button
                      size="sm"
                      variant="primary"
                      onClick={() => quickAction(r.id, 'resolved')}
                      disabled={busy.has(r.id)}
                      title={t('issues.resolve')}
                    >
                      {t('issues.resolve')}
                    </Button>
                  )}
                  {r.status !== 'ignored' && (
                    <Button
                      size="sm"
                      variant="ghost"
                      icon
                      onClick={() => quickAction(r.id, 'ignored')}
                      disabled={busy.has(r.id)}
                      title={t('issues.ignore')}
                    >
                      ⊘
                    </Button>
                  )}
                  {r.status !== 'active' && (
                    <Button
                      size="sm"
                      variant="ghost"
                      icon
                      onClick={() => quickAction(r.id, 'active')}
                      disabled={busy.has(r.id)}
                      title={t('issues.reopen')}
                    >
                      ↺
                    </Button>
                  )}
                </div>
              ),
            },
          ]}
        />
      </Card>
    </div>
  );
}

function TestIngestButton({ projectId }: { projectId: string }) {
  const [sending, setSending] = useState(false);
  const [out, setOut] = useState<string | null>(null);
  const { t } = useI18n();

  async function send() {
    setSending(true);
    setOut(null);
    const body: IngestRequest = {
      kind: 'error',
      error_type: 'TypeError',
      message: 'x is undefined (test ingest)',
      platform: 'javascript',
      release: 'webapp@0.1.0',
      environment: 'development',
    };
    try {
      const r = await api.ingestEvent(projectId, body);
      setOut(`${r.is_new ? 'new' : 'existing'}: ${r.issue_id.slice(0, 8)}`);
    } catch (e) {
      setOut(`error: ${String(e)}`);
    } finally {
      setSending(false);
    }
  }

  return (
    <div className="flex items-center gap-2">
      {out && (
        <span className="font-mono text-xs text-fg-subtle">{out}</span>
      )}
      <Button onClick={send} disabled={sending} variant="primary" size="sm">
        {sending ? t('common.sending') : t('issues.testIngest')}
      </Button>
    </div>
  );
}

function SaveViewButton({
  projectId,
  statusFilter,
}: {
  projectId: string;
  statusFilter: string;
}) {
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const { t } = useI18n();

  async function save() {
    const name = prompt(
      t('issues.savedViewName'),
      `Issues ${statusFilter || 'all'} – ${new Date().toLocaleDateString()}`,
    );
    if (!name) return;
    setSaving(true);
    setMsg(null);
    try {
      await api.createSavedView({
        name,
        target: 'issues',
        scope: 'workspace',
        project_id: projectId,
        payload: { status: statusFilter || 'all' },
      });
      setMsg('Saved');
      setTimeout(() => setMsg(null), 2000);
    } catch (e) {
      setMsg(String(e).slice(0, 40));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="flex items-center gap-2">
      {msg && (
        <span className="font-mono text-xs text-fg-subtle">{msg}</span>
      )}
      <Button onClick={save} disabled={saving} variant="secondary" size="sm">
        {saving ? t('common.saving') : t('issues.saveFilter')}
      </Button>
    </div>
  );
}
