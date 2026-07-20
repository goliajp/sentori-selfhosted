import { useEffect, useState } from 'react';
import { api, ApiError, AuditEntry } from '../lib/api';
import {
  Button,
  Card,
  CardHeader,
  ErrorBanner,
  PageHeader,
  formatRelative,
} from '../components/ui';

export function AuditPage() {
  const [entries, setEntries] = useState<AuditEntry[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [projectId, setProjectId] = useState('');
  const [actor, setActor] = useState('');
  const [action, setAction] = useState('');
  const [ipFilter, setIpFilter] = useState('');
  const [limit, setLimit] = useState(200);

  async function load() {
    try {
      const r = await api.listAudit({
        project_id: projectId.trim() || undefined,
        actor_user_id: actor.trim() || undefined,
        action: action.trim() || undefined,
        ip: ipFilter.trim() || undefined,
        limit,
      });
      setEntries(r);
      setErr(null);
    } catch (e) {
      if (e instanceof ApiError) setErr(`${e.status}: ${e.body}`);
      else setErr(String(e));
    }
  }

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function clear() {
    setProjectId('');
    setActor('');
    setAction('');
    setIpFilter('');
    setLimit(200);
    setTimeout(load, 0);
  }

  // IP filter is now applied server-side (backend filters by
  // payload._ip substring match).
  const visibleEntries = entries;

  function exportCsv() {
    if (!entries || entries.length === 0) return;
    const headers = [
      'id',
      'created_at',
      'action',
      'actor_user_id',
      'project_id',
      'target_type',
      'target_id',
      'payload',
    ];
    const escape = (v: unknown): string => {
      if (v === null || v === undefined) return '';
      const s = typeof v === 'string' ? v : JSON.stringify(v);
      return `"${s.replace(/"/g, '""')}"`;
    };
    const csv = [
      headers.join(','),
      ...entries.map(e =>
        headers
          .map(h => escape((e as unknown as Record<string, unknown>)[h]))
          .join(','),
      ),
    ].join('\n');
    const blob = new Blob([csv], { type: 'text/csv;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `audit-${new Date().toISOString().slice(0, 10)}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  }

  return (
    <div className="p-8">
      <PageHeader
        title="Audit log"
        subtitle="Workspace-wide admin actions, append-only."
        action={
          entries && entries.length > 0 ? (
            <Button variant="secondary" size="sm" onClick={exportCsv}>
              Export CSV
            </Button>
          ) : null
        }
      />
      {err && <ErrorBanner>{err}</ErrorBanner>}

      <Card className="mb-4">
        <CardHeader title="Filter" />
        <div className="grid grid-cols-4 gap-2 p-4">
          <Field
            label="Project ID"
            value={projectId}
            onChange={setProjectId}
            placeholder="UUID (optional)"
          />
          <Field
            label="Actor user ID"
            value={actor}
            onChange={setActor}
            placeholder="UUID (optional)"
          />
          <Field
            label="Action"
            value={action}
            onChange={setAction}
            placeholder="e.g. project.create"
          />
          <Field
            label="Limit"
            value={String(limit)}
            onChange={v => setLimit(parseInt(v, 10) || 200)}
            placeholder="200"
          />
          <Field
            label="IP (substring match)"
            value={ipFilter}
            onChange={setIpFilter}
            placeholder="e.g. 198.51.100"
          />
          <div className="col-span-4 flex gap-2 text-xs">
            <span className="text-zinc-500">Quick:</span>
            <button
              onClick={() => {
                setAction('token.mint');
                setTimeout(load, 0);
              }}
              className="text-emerald-400 hover:underline"
            >
              token mints
            </button>
            <button
              onClick={() => {
                setAction('project.create');
                setTimeout(load, 0);
              }}
              className="text-emerald-400 hover:underline"
            >
              project creates
            </button>
            <button
              onClick={() => {
                setAction('issue.status');
                setTimeout(load, 0);
              }}
              className="text-emerald-400 hover:underline"
            >
              issue status
            </button>
            <button
              onClick={() => {
                setAction('push_credentials.upsert');
                setTimeout(load, 0);
              }}
              className="text-emerald-400 hover:underline"
            >
              push creds
            </button>
          </div>
          <div className="col-span-4 flex gap-2">
            <Button onClick={load}>Apply</Button>
            <Button variant="secondary" onClick={clear}>
              Clear
            </Button>
          </div>
        </div>
      </Card>

      <Card>
        {visibleEntries?.length === 0 ? (
          <div className="p-8 text-center text-sm text-zinc-500">
            {ipFilter ? `No entries match IP "${ipFilter}".` : 'No audit entries yet.'}
          </div>
        ) : (
          <>
            {ipFilter && (
              <div className="border-b border-zinc-800 px-4 py-2 text-[10px] text-zinc-500">
                Showing {visibleEntries?.length ?? 0} of {entries?.length ?? 0}
                {' '}entries matching IP "{ipFilter}".
              </div>
            )}
            <ul className="divide-y divide-zinc-800">
              {visibleEntries?.map(e => (
                <AuditRow key={e.id} entry={e} />
              ))}
            </ul>
          </>
        )}
      </Card>
    </div>
  );
}

function AuditRow({ entry: e }: { entry: AuditEntry }) {
  const [open, setOpen] = useState(false);
  const payloadObject =
    e.payload && typeof e.payload === 'object'
      ? (e.payload as Record<string, unknown>)
      : null;
  const hasPayload = payloadObject
    ? Object.keys(payloadObject).length > 0
    : Boolean(e.payload);
  const payloadIp = payloadObject?._ip;
  const payloadUa = payloadObject?._ua;

  return (
    <li>
      <button
        onClick={() => hasPayload && setOpen(!open)}
        className={`flex w-full items-center gap-4 px-5 py-3 text-left ${hasPayload ? 'hover:bg-zinc-900/40' : 'cursor-default'}`}
      >
        <span className="font-mono text-xs text-zinc-500 w-4">
          {hasPayload ? (open ? '▼' : '▶') : ''}
        </span>
        <div className="flex-1 min-w-0">
          <div className="font-mono text-sm text-zinc-100">{e.action}</div>
          {(e.target_type || e.target_id) && (
            <div className="font-mono text-[11px] text-zinc-500">
              {e.target_type ?? ''}{' '}
              {e.target_id ? `${e.target_id.slice(0, 16)}…` : ''}
            </div>
          )}
        </div>
        <span className="font-mono text-xs text-zinc-400 w-24 text-right">
          {e.actor_user_id
            ? e.actor_user_id.slice(0, 8) + '…'
            : 'system'}
        </span>
        <span className="font-mono text-xs text-zinc-400 w-20 text-right">
          {e.project_id ? e.project_id.slice(0, 8) + '…' : 'workspace'}
        </span>
        <span className="text-xs text-zinc-500 w-24 text-right">
          {formatRelative(e.created_at)}
        </span>
      </button>
      {open && hasPayload && (
        <div className="bg-zinc-950 px-12 py-3">
          {payloadIp != null && (
            <div className="font-mono text-[10px] text-zinc-500">
              IP: <span className="text-zinc-300">{String(payloadIp)}</span>
              {payloadUa != null && (
                <>
                  {' · UA: '}
                  <span className="text-zinc-300">
                    {String(payloadUa).slice(0, 80)}
                  </span>
                </>
              )}
            </div>
          )}
          <pre className="overflow-x-auto whitespace-pre-wrap break-all text-[11px] font-mono text-zinc-300">
            {JSON.stringify(e.payload, null, 2)}
          </pre>
        </div>
      )}
    </li>
  );
}

function Field({
  label,
  value,
  onChange,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
}) {
  return (
    <div>
      <p className="mb-1 text-[10px] uppercase tracking-wide text-zinc-500">
        {label}
      </p>
      <input
        type="text"
        value={value}
        onChange={e => onChange(e.target.value)}
        placeholder={placeholder}
        className="w-full rounded border border-zinc-700 bg-zinc-900 px-3 py-1.5 text-sm font-mono focus:border-brand-500 focus:outline-none"
      />
    </div>
  );
}
