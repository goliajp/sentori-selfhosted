// Settings — the admin surface. Owner sections (projects / admins /
// tokens / audit) plus the personal section (password, language).
// Jira posture throughout: visible labels on every control, real
// tables with headers for every list, full width put to work.

import { useState } from 'react';

import { useShell } from '../App';
import {
  Button,
  DataTable,
  ErrorBanner,
  Field,
  Input,
  PageShell,
  Panel,
  Select,
  clsx,
  formatRelative,
} from '../components/ui';
import { useLocale, useSetLocale, useT } from '../i18n';
import {
  api,
  type AuditRow,
  type NotificationPref,
  type TokenRow,
  type UserRow,
} from '../lib/api';
import { useAsyncData } from '../lib/useAsyncData';

type Tab = 'account' | 'audit' | 'notifications' | 'tokens' | 'users';

export default function SettingsPage() {
  const t = useT();
  const { me } = useShell();
  const owner = me.role === 'superadmin';
  const tabs: Tab[] = owner
    ? ['tokens', 'users', 'notifications', 'audit', 'account']
    : ['tokens', 'notifications', 'account'];
  const [tab, setTab] = useState<Tab>(tabs[0] ?? 'account');

  return (
    <PageShell
      title={t('nav.settings')}
      toolbar={
        <div className="flex items-center gap-1">
          {tabs.map((x) => (
            <button
              key={x}
              type="button"
              onClick={() => setTab(x)}
              className={clsx(
                'rounded px-2 py-1 text-xs transition-colors',
                tab === x
                  ? 'bg-raised font-medium text-fg'
                  : 'text-fg-subtle hover:text-fg-muted',
              )}
            >
              {t(`settings.tab.${x}`)}
            </button>
          ))}
        </div>
      }
    >
      {tab === 'tokens' && <TokensTab />}
      {tab === 'users' && <UsersTab />}
      {tab === 'notifications' && <NotificationsTab />}
      {tab === 'audit' && <AuditTab />}
      {tab === 'account' && <AccountTab />}
    </PageShell>
  );
}

function TokensTab() {
  const t = useT();
  const { projects } = useShell();
  // `projects` loads async: a state initialised from projects[0] at
  // mount stays '' forever on a direct /settings load, and the list
  // never fetches. Derive the effective project instead.
  const [chosenId, setChosenId] = useState<null | string>(null);
  const projectId = chosenId ?? projects[0]?.id ?? '';
  const [name, setName] = useState('');
  const [scope, setScope] = useState<'api' | 'ingest'>('ingest');
  const [minted, setMinted] = useState<string | null>(null);
  const { data, reload } = useAsyncData(
    () => (projectId ? api.listTokens(projectId) : Promise.resolve({ tokens: [] })),
    [projectId],
  );
  const tokens = data?.tokens ?? [];

  return (
    <div className="space-y-4">
      <Panel title={t('settings.mintToken')}>
        {/* items-end so the button shares the controls' baseline */}
        <div className="flex flex-wrap items-end gap-3 p-3.5">
          {projects.length > 1 && (
            <Field label={t('settings.fieldProject')}>
              <Select value={projectId} onChange={(e) => setChosenId(e.target.value)}>
                {projects.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </Select>
            </Field>
          )}
          <Field label={t('settings.tokenName')} className="w-64">
            <Input value={name} onChange={(e) => setName(e.target.value)} />
          </Field>
          <Field label={t('settings.fieldScope')}>
            <Select
              value={scope}
              onChange={(e) => setScope(e.target.value as 'api' | 'ingest')}
            >
              <option value="ingest">ingest</option>
              <option value="api">api</option>
            </Select>
          </Field>
          <Button
            variant="primary"
            disabled={!projectId || name.trim().length === 0}
            onClick={() => {
              void api.createToken(projectId, name.trim(), scope).then((r) => {
                setMinted(r.token);
                setName('');
                reload();
              });
            }}
          >
            {t('settings.mintToken')}
          </Button>
        </div>
        {minted && (
          <div className="border-t border-border p-3.5 text-xs">
            <p className="mb-1.5 text-fg-muted">{t('settings.tokenOnce')}</p>
            <code className="block break-all rounded bg-bg p-2 font-mono text-fg">
              {minted}
            </code>
          </div>
        )}
      </Panel>

      <Panel title={`${t('settings.tab.tokens')} (${tokens.length})`}>
        <DataTable<TokenRow>
          rows={tokens}
          rowKey={(r) => r.id}
          columns={[
            { key: 'name', label: t('settings.tokenName') },
            {
              key: 'scope',
              label: t('settings.fieldScope'),
              width: '110px',
              render: (r) => (
                <span className="rounded bg-raised px-1.5 font-mono text-xs text-fg-muted">
                  {r.scope}
                </span>
              ),
            },
            {
              key: 'last4',
              label: t('settings.colToken'),
              width: '110px',
              render: (r) =>
                r.last4 ? (
                  <span className="font-mono text-xs text-fg-subtle">…{r.last4}</span>
                ) : (
                  '—'
                ),
            },
            {
              key: 'createdAt',
              label: t('settings.colCreated'),
              width: '120px',
              align: 'right',
              render: (r) => (
                <span className="text-xs tabular-nums text-fg-subtle">
                  {formatRelative(r.createdAt)}
                </span>
              ),
            },
            {
              key: 'actions',
              label: '',
              width: '90px',
              align: 'right',
              render: (r) =>
                r.revokedAt ? (
                  <span className="text-xs text-fg-subtle">{t('settings.revoked')}</span>
                ) : (
                  <button
                    type="button"
                    onClick={() => {
                      if (window.confirm(t('settings.revokeConfirm', { name: r.name }))) {
                        void api.revokeToken(r.id).then(reload);
                      }
                    }}
                    className="text-xs text-kind-error/70 hover:text-kind-error"
                  >
                    {t('settings.revoke')}
                  </button>
                ),
            },
          ]}
        />
      </Panel>
    </div>
  );
}

function UsersTab() {
  const t = useT();
  const { projects } = useShell();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const { data, error, reload } = useAsyncData(() => api.listUsers(), []);
  const users = data?.users ?? [];

  return (
    <div className="space-y-4">
      {error && <ErrorBanner>{t('settings.usersLoadFailed')}</ErrorBanner>}
      <Panel title={t('settings.createAdmin')}>
        <div className="flex flex-wrap items-end gap-3 p-3.5">
          <Field label={t('settings.adminEmail')} className="w-72">
            <Input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
            />
          </Field>
          {/* no hint line here: in an items-end inline form a hint
              would push the neighbouring button off the baseline */}
          <Field label={t('settings.initialPassword')} className="w-64">
            <Input
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              type="password"
            />
          </Field>
          <Button
            variant="primary"
            disabled={!email.includes('@') || password.length < 8}
            onClick={() => {
              void api.createUser(email.trim(), password).then(() => {
                setEmail('');
                setPassword('');
                reload();
              });
            }}
          >
            {t('settings.createAdmin')}
          </Button>
        </div>
      </Panel>

      <Panel title={`${t('settings.tab.users')} (${users.length})`}>
        <div className="divide-y divide-border/60">
          {/* header row — the assignment chips give each row a second
              line, so this stays a disciplined flex list rather than
              a <table>; the columns still line up via fixed widths */}
          <div className="flex items-center gap-3 bg-bg/50 px-4 py-2 text-xs font-medium text-fg-muted">
            <span className="flex-1">{t('settings.adminEmail')}</span>
            <span className="w-24">{t('settings.colRole')}</span>
            <span className="w-24 text-right">{t('settings.colLastLogin')}</span>
            <span className="w-14" />
          </div>
          {users.map((u: UserRow) => (
            <div key={u.id} className="px-4 py-2 text-sm">
              <div className="flex items-center gap-3">
                <span className="flex-1 text-fg">{u.email}</span>
                <span className="w-24 text-xs text-fg-subtle">
                  {u.role}
                </span>
                <span className="w-24 text-right text-xs tabular-nums text-fg-subtle">
                  {u.lastLoginAt ? formatRelative(u.lastLoginAt) : '—'}
                </span>
                <span className="w-14 text-right">
                  {u.role === 'admin' && (
                    <button
                      type="button"
                      onClick={() => {
                        if (
                          window.confirm(
                            t('settings.deleteAdminConfirm', { email: u.email }),
                          )
                        ) {
                          void api.deleteUser(u.id).then(reload);
                        }
                      }}
                      className="text-xs text-kind-error/70 hover:text-kind-error"
                    >
                      {t('common.delete')}
                    </button>
                  )}
                </span>
              </div>
              {u.role === 'admin' && (
                <div className="mt-1.5 flex flex-wrap gap-1.5">
                  {projects.map((p) => {
                    const has = u.projects.includes(p.id);
                    return (
                      <button
                        key={p.id}
                        type="button"
                        onClick={() => {
                          const op = has
                            ? api.unassignProject(u.id, p.id)
                            : api.assignProject(u.id, p.id);
                          void op.then(reload);
                        }}
                        className={clsx(
                          'rounded-full border px-2 py-0.5 text-xs transition-colors',
                          has
                            ? 'border-transparent bg-raised text-fg'
                            : 'border-border text-fg-subtle hover:text-fg-muted',
                        )}
                      >
                        {p.name}
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          ))}
        </div>
      </Panel>
    </div>
  );
}

function AuditTab() {
  const t = useT();
  const { data, error } = useAsyncData(() => api.listAudit(200), []);
  const entries = data?.entries ?? [];
  return (
    <div className="space-y-4">
      {error && <ErrorBanner>{t('settings.auditLoadFailed')}</ErrorBanner>}
      <Panel title={`${t('settings.tab.audit')} (${entries.length})`}>
        <DataTable<AuditRow>
          rows={entries}
          rowKey={(r) => r.id}
          columns={[
            {
              key: 'createdAt',
              label: t('settings.colWhen'),
              width: '110px',
              render: (r) => (
                <span className="text-xs tabular-nums text-fg-subtle">
                  {formatRelative(r.createdAt)}
                </span>
              ),
            },
            {
              key: 'actorEmail',
              label: t('settings.colActor'),
              width: '240px',
              render: (r) => (
                <span className="text-fg-muted">{r.actorEmail ?? '—'}</span>
              ),
            },
            {
              key: 'action',
              label: t('settings.colAction'),
              render: (r) => <span className="font-mono text-xs text-fg">{r.action}</span>,
            },
            {
              key: 'targetId',
              label: t('settings.colTarget'),
              width: '120px',
              align: 'right',
              render: (r) => (
                <span className="font-mono text-xs text-fg-subtle">
                  {r.targetId?.slice(0, 8) ?? '—'}
                </span>
              ),
            },
          ]}
        />
      </Panel>
    </div>
  );
}

function AccountTab() {
  const t = useT();
  const locale = useLocale();
  const setLocale = useSetLocale();
  const [current, setCurrent] = useState('');
  const [next, setNext] = useState('');
  const [saved, setSaved] = useState(false);

  return (
    <div className="grid max-w-4xl grid-cols-1 items-start gap-4 md:grid-cols-2">
      <Panel title={t('settings.changePassword')}>
        <div className="space-y-3 p-3.5">
          <Field label={t('settings.currentPassword')}>
            <Input
              type="password"
              value={current}
              onChange={(e) => setCurrent(e.target.value)}
            />
          </Field>
          <Field label={t('settings.newPassword')}>
            <Input
              type="password"
              value={next}
              onChange={(e) => setNext(e.target.value)}
            />
          </Field>
          <Button
            variant="primary"
            disabled={current.length === 0 || next.length < 8}
            onClick={() => {
              void api.changePassword(current, next).then(() => {
                setCurrent('');
                setNext('');
                setSaved(true);
                setTimeout(() => setSaved(false), 2000);
              });
            }}
          >
            {saved ? t('settings.saved') : t('settings.save')}
          </Button>
        </div>
      </Panel>
      <Panel title={t('settings.language')}>
        <div className="p-3.5">
          <Field label={t('settings.language')}>
            <Select
              value={locale}
              onChange={(e) => setLocale(e.target.value as typeof locale)}
            >
              <option value="en">English</option>
              <option value="ja">日本語</option>
              <option value="zh">简体中文</option>
            </Select>
          </Field>
        </div>
      </Panel>
    </div>
  );
}

function NotificationsTab() {
  const t = useT();
  const [testState, setTestState] = useState<'error' | 'idle' | 'sending' | 'sent'>(
    'idle',
  );
  const smtp = useAsyncData(() => api.smtpStatus(), []);
  const prefs = useAsyncData(() => api.listNotificationPrefs(), []);
  const [local, setLocal] = useState<Record<string, NotificationPref>>({});

  const rows = (prefs.data?.prefs ?? []).map((p) => local[p.projectId] ?? p);

  const flip = (p: NotificationPref, field: 'onNewIssue' | 'onRegression') => {
    const next = { ...p, [field]: !p[field] };
    setLocal((m) => ({ ...m, [p.projectId]: next }));
    void api
      .putNotificationPref({
        projectId: next.projectId,
        onNewIssue: next.onNewIssue,
        onRegression: next.onRegression,
      })
      .catch(() => {
        // roll back on failure so the UI never lies about state
        setLocal((m) => ({ ...m, [p.projectId]: p }));
      });
  };

  return (
    <div className="space-y-4">
      <Panel title={t('notify.smtpTitle')}>
        <div className="p-3.5">
          {smtp.data && smtp.data.configured && (
            <div className="flex items-center gap-3 text-sm">
              <span className="h-2 w-2 rounded-full bg-ok" />
              <span className="font-mono text-xs text-fg-muted">
                {smtp.data.host} · {smtp.data.from}
              </span>
              <Button
                size="sm"
                disabled={testState === 'sending'}
                onClick={() => {
                  setTestState('sending');
                  api.smtpTest().then(
                    () => setTestState('sent'),
                    () => setTestState('error'),
                  );
                }}
              >
                {testState === 'sending' ? t('notify.testSending') : t('notify.testButton')}
              </Button>
            </div>
          )}
          {smtp.data && !smtp.data.configured && (
            <div className="flex items-center gap-3 text-sm text-fg-muted">
              <span className="h-2 w-2 rounded-full bg-border-strong" />
              {t('notify.smtpUnconfigured')}
            </div>
          )}
          {testState === 'sent' && (
            <p className="mt-2 text-xs text-ok">{t('notify.testSent')}</p>
          )}
          {testState === 'error' && (
            <p className="mt-2 text-xs text-kind-error">{t('notify.testFailed')}</p>
          )}
        </div>
      </Panel>

      <Panel title={t('notify.prefsTitle')}>
        {prefs.error && (
          <div className="p-3.5">
            <ErrorBanner>{t('notify.loadFailed')}</ErrorBanner>
          </div>
        )}
        {rows.length === 0 && !prefs.loading && !prefs.error && (
          <p className="px-3.5 py-4 text-sm text-fg-subtle">{t('table.empty')}</p>
        )}
        {rows.length > 0 && (
          <>
            <p className="px-3.5 pt-2.5 text-xs text-fg-subtle">{t('notify.prefsHint')}</p>
            <div className="mt-1 divide-y divide-border/60">
              {rows.map((p) => (
                <div
                  key={p.projectId}
                  className="flex items-center gap-4 px-3.5 py-2 text-sm"
                >
                  <span className="min-w-0 flex-1 truncate font-medium text-fg">
                    {p.projectName}
                  </span>
                  <label className="flex items-center gap-1.5 text-xs text-fg-muted">
                    <input
                      type="checkbox"
                      checked={p.onNewIssue}
                      onChange={() => flip(p, 'onNewIssue')}
                      className="h-3.5 w-3.5 accent-accent"
                    />
                    {t('notify.onNewIssue')}
                  </label>
                  <label className="flex items-center gap-1.5 text-xs text-fg-muted">
                    <input
                      type="checkbox"
                      checked={p.onRegression}
                      onChange={() => flip(p, 'onRegression')}
                      className="h-3.5 w-3.5 accent-accent"
                    />
                    {t('notify.onRegression')}
                  </label>
                </div>
              ))}
            </div>
          </>
        )}
      </Panel>
    </div>
  );
}
