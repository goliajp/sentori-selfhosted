import { useState } from 'react';
import { useT } from '../i18n';
import { api, AuditEntry } from '../lib/api';
import { useAsyncData } from '../lib/useAsyncData';
import {
  Button,
  Card,
  CardHeader,
  ErrorBanner,
  PageHeader,
  formatRelative,
} from '../components/ui';

export function AuditPage() {
  const t = useT();
  const [projectId, setProjectId] = useState('');
  const [actor, setActor] = useState('');
  const [action, setAction] = useState('');
  const [ipFilter, setIpFilter] = useState('');
  const [limit, setLimit] = useState(200);

  const { data: entries, error: err, reload: load } = useAsyncData<AuditEntry[]>(
    () =>
      api.listAudit({
        project_id: projectId.trim() || undefined,
        actor_user_id: actor.trim() || undefined,
        action: action.trim() || undefined,
        ip: ipFilter.trim() || undefined,
        limit,
      }),
    [],
  );

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
    <div>
      <PageHeader
        title={t('audit.title')}
        subtitle={t('audit.subtitle')}
        action={
          entries && entries.length > 0 ? (
            <Button variant="secondary" size="sm" onClick={exportCsv}>{t('action.exportCsv')}</Button>
          ) : null
        }
      />
      {err && <ErrorBanner>{err}</ErrorBanner>}

      <Card className="mb-4">
        <CardHeader title={t('common.filter')} />
        <div className="grid grid-cols-4 gap-2 px-5 py-4">
          <Field
            label={t('tokens.projectId')}
            value={projectId}
            onChange={setProjectId}
            placeholder={t('common.optional')}
          />
          <Field
            label={t('audit.actor')}
            value={actor}
            onChange={setActor}
            placeholder={t('common.optional')}
          />
          <Field
            label={t('audit.action')}
            value={action}
            onChange={setAction}
            placeholder={t('audit.actionPlaceholder')}
          />
          <Field
            label={t('audit.limit')}
            value={String(limit)}
            onChange={v => setLimit(parseInt(v, 10) || 200)}
            placeholder="200"
          />
          <Field
            label={t('audit.ip')}
            value={ipFilter}
            onChange={setIpFilter}
            placeholder={t('audit.ipPlaceholder')}
          />
          <div className="col-span-4 flex gap-2 text-xs">
            <span className="text-fg-subtle">{t('audit.quick')}:</span>
            <button
              onClick={() => {
                setAction('token.mint');
                setTimeout(load, 0);
              }}
              className="text-accent hover:underline"
            >
              {t('audit.quickTokens')}
            </button>
            <button
              onClick={() => {
                setAction('project.create');
                setTimeout(load, 0);
              }}
              className="text-accent hover:underline"
            >
              {t('audit.quickProjects')}
            </button>
            <button
              onClick={() => {
                setAction('issue.status');
                setTimeout(load, 0);
              }}
              className="text-accent hover:underline"
            >
              {t('audit.quickIssues')}
            </button>
            <button
              onClick={() => {
                setAction('push_credentials.upsert');
                setTimeout(load, 0);
              }}
              className="text-accent hover:underline"
            >
              {t('audit.quickPush')}
            </button>
          </div>
          <div className="col-span-4 flex gap-2">
            <Button onClick={load}>{t('action.apply')}</Button>
            <Button variant="secondary" onClick={clear}>{t('action.clear')}</Button>
          </div>
        </div>
      </Card>

      <Card>
        {visibleEntries?.length === 0 ? (
          <div className="p-8 text-center text-sm text-fg-subtle">
            {ipFilter ? `No entries match IP "${ipFilter}".` : t('audit.empty')}
          </div>
        ) : (
          <>
            {ipFilter && (
              <div className="border-b border-border px-4 py-2 text-xs text-fg-subtle">
                Showing {visibleEntries?.length ?? 0} of {entries?.length ?? 0}
                {' '}entries matching IP "{ipFilter}".
              </div>
            )}
            <ul className="divide-y divide-border">
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
  const t = useT();
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
        className={`flex w-full items-center gap-4 px-5 py-3 text-left ${hasPayload ? 'hover:bg-surface/40' : 'cursor-default'}`}
      >
        <span className="font-mono text-xs text-fg-subtle w-4">
          {hasPayload ? (open ? '▼' : '▶') : ''}
        </span>
        <div className="flex-1 min-w-0">
          <div className="font-mono text-sm text-fg">{e.action}</div>
          {(e.target_type || e.target_id) && (
            <div className="font-mono text-xs text-fg-subtle">
              {e.target_type ?? ''}{' '}
              {e.target_id ? `${e.target_id.slice(0, 16)}…` : ''}
            </div>
          )}
        </div>
        {/* Two bare uuid prefixes side by side told you nothing about
            which was which. The label costs a word and answers it. */}
        <span className="w-40 text-right text-xs text-fg-subtle">
          {t('audit.by')}{' '}
          <span className="font-mono text-fg-muted">
            {e.actor_user_id
              ? `${e.actor_user_id.slice(0, 8)}…`
              : t('audit.system')}
          </span>
        </span>
        <span className="w-40 text-right text-xs text-fg-subtle">
          {e.project_id ? (
            <>
              {t('audit.scope')}{' '}
              <span className="font-mono text-fg-muted">
                {`${e.project_id.slice(0, 8)}…`}
              </span>
            </>
          ) : (
            t('audit.workspaceScope')
          )}
        </span>
        <span className="text-xs text-fg-subtle w-24 text-right">
          {formatRelative(e.created_at)}
        </span>
      </button>
      {open && hasPayload && (
        <div className="bg-bg px-12 py-3">
          {payloadIp != null && (
            <div className="font-mono text-xs text-fg-subtle">
              IP: <span className="text-fg-muted">{String(payloadIp)}</span>
              {payloadUa != null && (
                <>
                  {' · UA: '}
                  <span className="text-fg-muted">
                    {String(payloadUa).slice(0, 80)}
                  </span>
                </>
              )}
            </div>
          )}
          <pre className="overflow-x-auto whitespace-pre-wrap break-all text-xs font-mono text-fg-muted">
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
      <p className="mb-1 text-xs uppercase tracking-wide text-fg-subtle">
        {label}
      </p>
      <input
        type="text"
        value={value}
        onChange={e => onChange(e.target.value)}
        placeholder={placeholder}
        className="w-full rounded border border-border-strong bg-surface px-3 py-1.5 text-sm font-mono focus:border-accent focus:outline-none"
      />
    </div>
  );
}
