import { useEffect, useState } from 'react';
import { Link, useParams, useSearchParams } from 'react-router-dom';
import { api, ApiError, IngestRequest, Issue } from '../lib/api';
import { useKeyHandlers } from '../lib/useShortcuts';
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

  if (!projectId) return <div className="p-8">no project id</div>;

  return (
    <div className="p-8">
      <PageHeader
        title="Issues"
        subtitle={`Project ${projectId.slice(0, 8)}…`}
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

      <p className="mb-3 text-[10px] text-zinc-500">
        Shortcuts: <kbd className="rounded bg-zinc-800 px-1">j</kbd>/
        <kbd className="rounded bg-zinc-800 px-1">k</kbd> navigate ·{' '}
        <kbd className="rounded bg-zinc-800 px-1">x</kbd> select ·{' '}
        <kbd className="rounded bg-zinc-800 px-1">e</kbd> resolve ·{' '}
        <kbd className="rounded bg-zinc-800 px-1">i</kbd> ignore
      </p>

      {selected.size > 0 && (
        <div className="mb-4 flex items-center gap-2 rounded border border-emerald-700/40 bg-emerald-900/20 px-3 py-2 text-xs">
          <span className="text-zinc-300">
            {selected.size} selected
          </span>
          <Button size="sm" onClick={() => bulkApply('resolved')}>
            Resolve all
          </Button>
          <Button
            size="sm"
            variant="secondary"
            onClick={() => bulkApply('ignored')}
          >
            Ignore all
          </Button>
          <Button
            size="sm"
            variant="secondary"
            onClick={() => setSelected(new Set())}
          >
            Clear
          </Button>
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
            { value: 'all', label: 'All' },
            { value: 'active', label: 'Active' },
            { value: 'regressed', label: 'Regressed' },
            { value: 'resolved', label: 'Resolved' },
            { value: 'ignored', label: 'Ignored' },
          ]}
        />
      </div>

      {err && <ErrorBanner>{err}</ErrorBanner>}

      <Card>
        <DataTable
          rowKey={(r) => r.id}
          empty="No issues. Send some events with the SDK to populate."
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
                      <span className="text-emerald-500 text-xs">▸</span>
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
              render: (r) => <Badge tone={STATUS_TONE[r.status]}>{r.status}</Badge>,
            },
            {
              key: 'error_type',
              label: 'Issue',
              render: (r) => (
                <Link
                  to={`/projects/${projectId}/issues/${r.id}`}
                  className="block hover:bg-zinc-900/40 -m-2 p-2 rounded"
                >
                  <div className="font-medium text-zinc-100">{r.error_type}</div>
                  <div className="font-mono text-[11px] text-zinc-500">
                    {r.message_sample.slice(0, 80)}
                  </div>
                </Link>
              ),
            },
            {
              key: 'event_count',
              label: 'Events',
              width: '10%',
              render: (r) => (
                <span className="font-mono tabular-nums">
                  {formatNumber(r.event_count)}
                </span>
              ),
            },
            {
              key: 'last_release',
              label: 'Release',
              width: '15%',
              render: (r) => (
                <span className="font-mono text-xs text-zinc-400">
                  {r.last_release}
                </span>
              ),
            },
            {
              key: 'last_environment',
              label: 'Env',
              width: '10%',
              render: (r) => <Badge>{r.last_environment}</Badge>,
            },
            {
              key: 'last_seen',
              label: 'Last seen',
              width: '12%',
              render: (r) => (
                <span className="text-xs text-zinc-500">
                  {formatRelative(r.last_seen)}
                </span>
              ),
            },
            {
              key: 'actions',
              label: '',
              width: '14%',
              render: (r) => (
                <div className="flex gap-1">
                  {r.status !== 'resolved' && (
                    <button
                      onClick={() => quickAction(r.id, 'resolved')}
                      disabled={busy.has(r.id)}
                      title="Resolve"
                      className="rounded bg-emerald-700/30 px-2 py-0.5 text-[11px] text-emerald-300 hover:bg-emerald-700/60 disabled:opacity-50"
                    >
                      ✓ Resolve
                    </button>
                  )}
                  {r.status !== 'ignored' && (
                    <button
                      onClick={() => quickAction(r.id, 'ignored')}
                      disabled={busy.has(r.id)}
                      title="Ignore"
                      className="rounded bg-zinc-700/40 px-2 py-0.5 text-[11px] text-zinc-300 hover:bg-zinc-600 disabled:opacity-50"
                    >
                      ⊘
                    </button>
                  )}
                  {r.status !== 'active' && (
                    <button
                      onClick={() => quickAction(r.id, 'active')}
                      disabled={busy.has(r.id)}
                      title="Reopen"
                      className="rounded bg-orange-700/30 px-2 py-0.5 text-[11px] text-orange-300 hover:bg-orange-700/60 disabled:opacity-50"
                    >
                      ↺
                    </button>
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
        <span className="font-mono text-xs text-zinc-500">{out}</span>
      )}
      <Button onClick={send} disabled={sending} variant="primary" size="sm">
        {sending ? 'Sending…' : 'Test ingest'}
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

  async function save() {
    const name = prompt(
      'Saved view name',
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
        <span className="font-mono text-xs text-zinc-500">{msg}</span>
      )}
      <Button onClick={save} disabled={saving} variant="secondary" size="sm">
        {saving ? 'Saving…' : 'Save filter'}
      </Button>
    </div>
  );
}
