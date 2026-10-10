import { AuthenticationMode } from '../components/AuthenticationMode';
import { canUse } from '../lib/system-contract';
import { Tabs } from "../../shared/design/Tabs";
import { Dialog } from "../../shared/design/Dialog";
import { Heading, Surface } from "../../shared/design/Surface";
import { Button as DesignButton } from "../../shared/design/controls";
// Settings — the admin surface. Owner sections (projects / admins /
// tokens / audit) plus the personal section (password, language).
//
// Push used to be a tab here and is now its own module. Four panels
// about whether last night's alert reached anyone do not belong
// behind the same door as "change my password".
// Jira posture throughout: visible labels on every control, real
// tables with headers for every list, full width put to work.

import { Check, Copy } from 'lucide-react';
import { useMemo, useState } from 'react';
import { useLocation, useNavigate, useParams, useSearchParams } from 'react-router-dom';

import { useShell } from '../App';
import { Button, DataTable, ErrorBanner, Field, Input, Panel, Select, TimeAgo, clsx, formatAbsolute, formatRelative } from '../components/ui';
import { useLocale, useSetLocale, useT } from '../i18n';
import {
  api,
  type AuditRow,
  type NotificationPref,
  type TokenRow,
  type UserRow,
} from '../lib/api';
import { formatApiError, useAsyncData } from '../lib/useAsyncData';
import InstanceSettings from './Instance';

type Tab = 'account' | 'audit' | 'instance' | 'notifications' | 'tokens' | 'users';

export default function SettingsPage() {
  const t = useT();
  const { me } = useShell();
  const owner = me.role === 'superadmin';
  const tabs: Tab[] = owner
    ? ['tokens', 'users', 'notifications', 'audit', 'instance', 'account']
    : ['tokens', 'notifications', 'instance', 'account'];
  // The tab lives in the URL, not in component state: a settings
  // section you cannot link to is a section nobody can point a
  // colleague at, and it is also one no screenshot sweep can reach.
  const [params, setParams] = useSearchParams();
  const { section } = useParams();
  const location = useLocation();
  const navigate = useNavigate();
  const asked = (section ?? params.get('tab')) as null | Tab;
  const tab: Tab = asked && tabs.includes(asked) ? asked : tabs[0] ?? 'account';
  const setTab = (x: Tab) => {
    const next = new URLSearchParams(params);
    next.delete('tab');
    if (section || x === 'instance') navigate({ pathname: `/settings/${x}`, search: next.toString(), hash: location.hash });
    else { next.set('tab', x); setParams(next, { replace: true }); }
  };

  return (
    <div className="production-page">
      <Heading title={t('nav.settings')} description={t('settings.description')} />
      <Tabs value={tab} onChange={value => setTab(value as Tab)} label={t('nav.settings')} items={tabs.map(value => [value, t(`settings.tab.${value}`)] as const)}>
      {tab === 'tokens' && <TokensTab />}
      {tab === 'users' && <UsersTab />}
      {tab === 'notifications' && <NotificationsTab />}
      {tab === 'audit' && <AuditTab />}
      {tab === 'account' && <AccountTab />}
      {tab === 'instance' && <InstanceSettings />}
      </Tabs>
    </div>
  );
}

function TokensTab() {
  const t = useT();
  const { activeProject, projects, setActiveProjectId } = useShell();
  return <div className="form-stack">
    <Field label={t('settings.fieldProject')}><Select value={activeProject?.id ?? ''} onChange={event => setActiveProjectId(event.target.value)}>
      {!activeProject && <option value="" disabled>{t('settings.selectProject')}</option>}
      {projects.map(project => <option key={project.id} value={project.id}>{project.name}</option>)}
    </Select></Field>
    {/* Remount on scope changes: rows, drafts and one-time secrets belong to one project. */}
    {activeProject ? <ProjectTokens key={activeProject.id} projectId={activeProject.id} projectName={activeProject.name} /> : <Surface><p>{t('settings.selectProject')}</p></Surface>}
  </div>;
}

function ProjectTokens({ projectId, projectName }: { projectId: string; projectName: string }) {
  const t = useT();
  const { data, error, loading, reload } = useAsyncData(() => api.listTokens(projectId), [projectId]);
  const [creating, setCreating] = useState(false), [revoking, setRevoking] = useState<TokenRow | null>(null);
  const [name, setName] = useState(''), [scope, setScope] = useState<'api' | 'ingest'>('ingest');
  const [minted, setMinted] = useState<string | null>(null), [copied, setCopied] = useState(false);
  const [busy, setBusy] = useState(false), [failure, setFailure] = useState<string | null>(null), [copyFailure, setCopyFailure] = useState(false);
  async function create(event: React.FormEvent) {
    event.preventDefault(); if (busy || !name.trim()) return;
    setBusy(true); setFailure(null);
    try { const result = await api.createToken(projectId, name.trim(), scope); setMinted(result.token); setCopied(false); setCopyFailure(false); setCreating(false); reload(); }
    catch (error) { setFailure(formatApiError(error)); }
    finally { setBusy(false); }
  }
  async function revoke() {
    if (busy || !revoking) return;
    setBusy(true); setFailure(null);
    try { await api.revokeToken(revoking.id); setRevoking(null); reload(); }
    catch (error) { setFailure(formatApiError(error)); }
    finally { setBusy(false); }
  }
  const tokens = data?.tokens ?? [];
  return <div className="form-stack">
    <p className="muted">{t('settings.tokenRotation')}</p>
    <Surface title={`${projectName} · ${t('settings.tab.tokens')}`} body={false} action={<DesignButton variant="primary" onClick={() => { setName(''); setScope('ingest'); setFailure(null); setCreating(true); }}>{t('settings.mintToken')}</DesignButton>}>
      {error && <div className="surface-body"><ErrorBanner reason={error}>{t('settings.tokensLoadFailed')}</ErrorBanner><DesignButton onClick={reload}>{t('common.retry')}</DesignButton></div>}
      {loading && !data ? <p className="surface-body" role="status">{t('shell.loading')}</p> : <DataTable<TokenRow> appearance="design" rows={tokens} rowKey={row => row.id} columns={[
        {key:'name', label:t('settings.tokenName')},
        {key:'scope', label:t('settings.fieldScope'), render:row => <code>{row.scope}</code>},
        {key:'last4', label:t('settings.colToken'), render:row => <code>{row.last4 ? `••••${row.last4}` : '—'}</code>},
        {key:'createdAt', label:t('settings.colCreated'), render:row => <TimeAgo iso={row.createdAt} />},
        {key:'actions', label:t('settings.colAction'), align:'right', render:row => row.revokedAt ? <span className="subtle">{t('settings.revoked')}</span> : <DesignButton small onClick={() => { setFailure(null); setRevoking(row); }}>{t('settings.revoke')}</DesignButton>},
      ]} />}
    </Surface>
    <Dialog open={creating} title={t('settings.mintToken')} closeLabel={t('common.close')} onClose={() => { if (!busy) setCreating(false); }} footer={<><DesignButton disabled={busy} onClick={() => setCreating(false)}>{t('common.close')}</DesignButton><DesignButton form="create-project-token" type="submit" variant="primary" busy={busy} disabled={!name.trim()}>{t('settings.mintToken')}</DesignButton></>}>
      <form id="create-project-token" className="form-stack" onSubmit={create}>
        <Field label={t('settings.tokenName')}><Input required maxLength={200} value={name} disabled={busy} onChange={event => setName(event.target.value)} /></Field>
        <Field label={t('settings.fieldScope')} hint={t('settings.tokenScopeHint')}><Select value={scope} disabled={busy} onChange={event => setScope(event.target.value as 'api' | 'ingest')}><option value="ingest">ingest</option><option value="api">api</option></Select></Field>
        {failure && <ErrorBanner reason={failure}>{t('settings.tokenActionFailed')}</ErrorBanner>}
      </form>
    </Dialog>
    <Dialog open={minted !== null} title={t('settings.tokenOnce')} closeLabel={t('common.close')} onClose={() => setMinted(null)} footer={<><DesignButton onClick={() => setMinted(null)}>{t('common.close')}</DesignButton><DesignButton variant="primary" onClick={async () => { if (!minted) return; try { await navigator.clipboard.writeText(minted); setCopied(true); setCopyFailure(false); } catch { setCopyFailure(true); } }}>{copied ? <Check size={16} /> : <Copy size={16} />}{t(copied ? 'identity.copied' : 'identity.copyHint')}</DesignButton></>}>
      <code className="block break-all select-all rounded bg-raised p-4">{minted}</code>
      {copyFailure && <p role="alert" className="field-error mt-3">{t('setup.copyFailed')}</p>}
    </Dialog>
    <Dialog open={revoking !== null} title={t('settings.revoke')} closeLabel={t('common.close')} onClose={() => { if (!busy) setRevoking(null); }} footer={<><DesignButton disabled={busy} onClick={() => setRevoking(null)}>{t('common.close')}</DesignButton><DesignButton variant="danger" busy={busy} onClick={() => void revoke()}>{t('settings.revoke')}</DesignButton></>}>
      <p>{t('settings.revokeConfirm', {name:revoking?.name ?? ''})}</p>
      {failure && <ErrorBanner reason={failure}>{t('settings.tokenActionFailed')}</ErrorBanner>}
    </Dialog>
  </div>;
}

function UsersTab() {
  const t = useT();
  const { projects } = useShell();
  const system = useAsyncData(() => api.systemInfo(), []);
  const manage = canUse(system.data, 'members.manage');
  const { data, error, loading, reload, setData } = useAsyncData(() => api.listUsers(), []);
  const [creating, setCreating] = useState(false), [managingId, setManagingId] = useState<string | null>(null), [deleting, setDeleting] = useState<UserRow | null>(null);
  const [email, setEmail] = useState(''), [password, setPassword] = useState('');
  const [busy, setBusy] = useState(false), [failure, setFailure] = useState<string | null>(null);
  const users = data?.users ?? [], managing = users.find(user => user.id === managingId);
  async function create(event: React.FormEvent) {
    event.preventDefault(); if (busy || !manage) return;
    setBusy(true); setFailure(null);
    try { await api.createUser(email.trim(), password); setPassword(''); setCreating(false); reload(); }
    catch (error) { setFailure(formatApiError(error)); }
    finally { setBusy(false); }
  }
  async function assign(projectId: string, checked: boolean) {
    if (!managing || busy) return;
    const userId = managing.id;
    setBusy(true); setFailure(null);
    try {
      if (checked) await api.assignProject(userId, projectId); else await api.unassignProject(userId, projectId);
      setData(current => current && ({users:current.users.map(user => user.id !== userId ? user : {...user, projects:checked ? [...new Set([...user.projects, projectId])] : user.projects.filter(id => id !== projectId)})}));
    } catch (error) { setFailure(formatApiError(error)); }
    finally { setBusy(false); }
  }
  async function remove() {
    if (!deleting || busy || !manage) return;
    setBusy(true); setFailure(null);
    try { await api.deleteUser(deleting.id); setDeleting(null); reload(); }
    catch (error) { setFailure(formatApiError(error)); }
    finally { setBusy(false); }
  }
  return <>
    {system.error && <ErrorBanner reason={system.error}>{t('instance.loadFailed')}</ErrorBanner>}
    {system.data?.deployment === 'cloud' && <p className="mb-4 text-sm text-fg-muted">{t('auth.cloudMembers')}</p>}
    <Surface title={t('settings.tab.users')} body={false} action={manage && <DesignButton variant="primary" onClick={() => {setEmail(''); setPassword(''); setFailure(null); setCreating(true);}}>{t('settings.createAdmin')}</DesignButton>}>
      {error && <div className="surface-body"><ErrorBanner reason={error}>{t('settings.usersLoadFailed')}</ErrorBanner><DesignButton onClick={reload}>{t('common.retry')}</DesignButton></div>}
      {loading && !data ? <p className="surface-body" role="status">{t('shell.loading')}</p> : <DataTable<UserRow> appearance="design" rows={users} rowKey={user => user.id} columns={[
        {key:'email', label:t('settings.adminEmail')},
        {key:'role', label:t('settings.colRole'), render:user => <span title={t(user.role === 'superadmin' ? 'settings.roleOwnerScope' : 'settings.roleAdminScope')}>{t(user.role === 'superadmin' ? 'shell.roleOwner' : 'shell.roleAdmin')}</span>},
        {key:'projects', label:t('settings.fieldProject'), render:user => user.role === 'superadmin' ? t('settings.allProjects') : String(user.projects.length)},
        {key:'lastLoginAt', label:t('settings.colLastLogin'), render:user => user.lastLoginAt ? <TimeAgo iso={user.lastLoginAt} /> : '—'},
        {key:'actions', label:t('settings.colAction'), align:'right', render:user => user.role === 'admin' && <div className="flex justify-end gap-2"><DesignButton small onClick={() => {setFailure(null); setManagingId(user.id);}}>{t('settings.manageAccess')}</DesignButton>{manage && <DesignButton small variant="danger" onClick={() => {setFailure(null); setDeleting(user);}}>{t('common.delete')}</DesignButton>}</div>},
      ]} />}
    </Surface>
    {manage && <Dialog open={creating} title={t('settings.createAdmin')} closeLabel={t('common.close')} onClose={() => {if (!busy) {setCreating(false); setPassword('');}}} footer={<><DesignButton disabled={busy} onClick={() => {setCreating(false); setPassword('');}}>{t('common.close')}</DesignButton><DesignButton type="submit" form="create-member" busy={busy} variant="primary">{t('settings.createAdmin')}</DesignButton></>}>
      <form id="create-member" className="form-stack" onSubmit={create}>
        <Field label={t('settings.adminEmail')}><Input type="email" autoComplete="off" required value={email} disabled={busy} onChange={event => setEmail(event.target.value)} /></Field>
        <Field label={t('settings.initialPassword')}><Input type="password" autoComplete="new-password" required minLength={8} value={password} disabled={busy} onChange={event => setPassword(event.target.value)} /></Field>
        {failure && <ErrorBanner reason={failure}>{t('settings.memberActionFailed')}</ErrorBanner>}
      </form>
    </Dialog>}
    <Dialog open={managing !== undefined} title={t('settings.manageAccess')} closeLabel={t('common.close')} onClose={() => {if (!busy) setManagingId(null);}} footer={<DesignButton disabled={busy} onClick={() => setManagingId(null)}>{t('common.close')}</DesignButton>}>
      <p className="break-all mb-4">{managing?.email}</p>
      <fieldset className="form-stack" disabled={busy}><legend className="field-label mb-3">{t('settings.fieldProject')}</legend>
        {projects.length === 0 && <p>{t('table.empty')}</p>}
        {projects.map(project => <label key={project.id} className="choice-control"><input type="checkbox" checked={managing?.projects.includes(project.id) ?? false} onChange={event => void assign(project.id, event.target.checked)} /><span className="break-all">{project.name}</span></label>)}
      </fieldset>
      {failure && <ErrorBanner reason={failure}>{t('settings.memberActionFailed')}</ErrorBanner>}
    </Dialog>
    {manage && <Dialog open={deleting !== null} title={t('common.delete')} closeLabel={t('common.close')} onClose={() => {if (!busy) setDeleting(null);}} footer={<><DesignButton disabled={busy} onClick={() => setDeleting(null)}>{t('common.close')}</DesignButton><DesignButton busy={busy} variant="danger" onClick={() => void remove()}>{t('common.delete')}</DesignButton></>}>
      <p className="break-words">{t('settings.deleteAdminConfirm', {email:deleting?.email ?? ''})}</p>
      {failure && <ErrorBanner reason={failure}>{t('settings.memberActionFailed')}</ErrorBanner>}
    </Dialog>}
  </>;
}

function AuditTab() {
  const t = useT();
  const { data, error } = useAsyncData(() => api.listAudit(200), []);
  const entries = data?.entries ?? [];
  return (
    <div className="space-y-4">
      {error && <ErrorBanner reason={error}>{t('settings.auditLoadFailed')}</ErrorBanner>}
      <Surface body={false} title={`${t('settings.tab.audit')} (${entries.length})`}>
        <DataTable<AuditRow> appearance="design"
          rows={entries}
          rowKey={(r) => r.id}
          columns={[
            {
              key: 'createdAt',
              label: t('settings.colWhen'),
              width: '190px',
              // Absolute, with the zone. An audit row that reads "2
              // minutes ago" cannot be lined up with a deploy, a
              // support ticket or another system's log — which is the
              // only thing an audit log is for. The relative form is
              // still the title, for the "how long ago" read.
              render: (r) => (
                <span
                  className="text-xs tabular-nums text-fg-subtle"
                  // bare-relative: inverted here, the absolute time is what is rendered
                  title={formatRelative(r.createdAt)}
                >
                  {formatAbsolute(r.createdAt)}
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
              // What was acted on, not just its id. The column showed
              // eight characters of a uuid, so a row read "someone
              // deleted 01a0ef95" — a project? a token? a person? The
              // type is the half that makes an audit log auditable,
              // and the server has been returning it all along.
              //
              // It is not the action's prefix either: `assignment.grant`
              // acts on a `user`, so the two columns say different
              // things.
              key: 'targetId',
              label: t('settings.colTarget'),
              width: '220px',
              render: (r) => (
                <span className="flex items-baseline gap-1.5 text-xs">
                  <span className="text-fg-muted">{r.targetType ?? '—'}</span>
                  {r.targetId && (
                    // The whole id in the title: eight characters
                    // cannot be pasted into a query, and two objects
                    // can share a prefix.
                    <span className="font-mono text-fg-subtle" title={r.targetId}>
                      {r.targetId.slice(0, 8)}
                    </span>
                  )}
                </span>
              ),
            },
          ]}
        />
      </Surface>
    </div>
  );
}

function AccountTab() {
  const t = useT(), locale = useLocale(), setLocale = useSetLocale();
  const { me } = useShell();
  const [current, setCurrent] = useState(''), [next, setNext] = useState(''), [repeat, setRepeat] = useState('');
  const [saved, setSaved] = useState(false), [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const [mismatch, setMismatch] = useState(false);
  async function save(event: React.FormEvent) {
    event.preventDefault(); if (busy) return;
    setSaved(false); setFailure(null); setMismatch(next !== repeat);
    if (next !== repeat) return;
    setBusy(true);
    try { await api.changePassword(current, next); setCurrent(''); setNext(''); setRepeat(''); setSaved(true); }
    catch (error) { setFailure(formatApiError(error)); }
    finally { setBusy(false); }
  }
  return <div className="grid grid-cols-1 items-start gap-5 lg:grid-cols-2">
    <div className="grid gap-5">
      <Surface title={t('settings.tab.account')}>
        <dl className="grid gap-4 text-sm"><div><dt className="muted">{t('auth.email')}</dt><dd className="mt-2 break-all">{me.email}</dd></div><div><dt className="muted">{t('settings.colRole')}</dt><dd className="mt-2">{t(me.role === 'superadmin' ? 'shell.roleOwner' : 'shell.roleAdmin')}</dd></div></dl>
      </Surface>
      <Surface title={t('settings.language')}><Field label={t('settings.language')}><Select value={locale} onChange={e => setLocale(e.target.value as typeof locale)}><option value="en">English</option><option value="ja">日本語</option><option value="zh">简体中文</option></Select></Field></Surface>
    </div>
    <Surface title={t('settings.changePassword')}><AuthenticationMode account>
      <form className="form-stack" onSubmit={save}>
        <Field label={t('settings.currentPassword')}><Input type="password" autoComplete="current-password" required value={current} disabled={busy} onChange={e => { setCurrent(e.target.value); setSaved(false); }} /></Field>
        <Field label={t('settings.newPassword')} hint={t('settings.passwordSessionHint')}><Input type="password" autoComplete="new-password" required minLength={12} value={next} disabled={busy} onChange={e => { setNext(e.target.value); setSaved(false); setMismatch(false); }} /></Field>
        <Field label={t('auth.confirm')} error={mismatch ? t('auth.passwordsDiffer') : undefined}><Input type="password" autoComplete="new-password" required minLength={12} value={repeat} disabled={busy} onChange={e => { setRepeat(e.target.value); setSaved(false); setMismatch(false); }} /></Field>
        {failure && <ErrorBanner reason={failure}>{t('settings.passwordFailed')}</ErrorBanner>}
        {saved && <p role="status" className="text-sm text-ok">{t('settings.saved')}</p>}
        <DesignButton type="submit" variant="primary" busy={busy}>{t('settings.changePassword')}</DesignButton>
      </form>
    </AuthenticationMode></Surface>
  </div>;
}

/**
 * The second alert channel.
 *
 * Issue notifications were email-only, and `spawn_issue_notification`
 * returned early with no SMTP — so a self-hosted instance without a
 * mail server got no alerts at all and nothing said so. The transport
 * had been in the notifier crate the whole time, with tests and a doc
 * comment teaching people to use it, and the server never registered
 * it.
 *
 * Per project because a webhook goes to a room. The list comes from
 * the notification preferences, which already enumerate the projects
 * this person can see.
 */
function WebhookPanel() {
  const t = useT();
  const { activeProject } = useShell();
  const projectId = activeProject?.id ?? null;
  const { data, reload } = useAsyncData(
    () => (projectId ? api.getProject(projectId) : Promise.resolve(null)),
    [projectId],
  );
  const [draft, setDraft] = useState<null | string>(null);
  const [state, setState] = useState<'error' | 'idle' | 'saved' | 'saving'>('idle');

  // `draft` is null until the field is touched, so a reload does not
  // fight what is being typed.
  const value = draft ?? data?.webhookUrl ?? '';

  return (
    <Panel title={t('notify.webhookTitle')}>
      <div className="space-y-2 p-3.5">
        <p className="text-xs text-fg-muted">{t('notify.webhookHint')}</p>
        <div className="flex items-center gap-2">
          <input
            type="url"
            value={value}
            placeholder={t('notify.webhookPlaceholder')}
            onChange={(e) => {
              setDraft(e.target.value);
              setState('idle');
            }}
            className="flex-1 rounded border border-border bg-bg px-2 py-1 font-mono text-xs text-fg"
          />
          <Button
            size="sm"
            disabled={!projectId || state === 'saving'}
            onClick={() => {
              if (!projectId) return;
              setState('saving');
              // The empty string is sent on purpose: it is how the
              // channel is turned off, and a field that cannot be
              // cleared is a channel that cannot be stopped.
              api.updateProject(projectId, { webhookUrl: value.trim() }).then(
                () => {
                  setState('saved');
                  setDraft(null);
                  reload();
                },
                () => setState('error'),
              );
            }}
          >
            {t('notify.webhookSave')}
          </Button>
        </div>
        {state === 'saved' && <p className="text-xs text-ok">{t('notify.webhookSaved')}</p>}
        {state === 'error' && (
          <p className="text-xs text-kind-error">{t('notify.webhookFailed')}</p>
        )}
        <p className="text-xs text-fg-subtle">{t('notify.webhookOwnerOnly')}</p>
      </div>
    </Panel>
  );
}

function NotificationsTab() {
  const locale = useLocale();
  const t = useT();
  const [testState, setTestState] = useState<'error' | 'idle' | 'sending' | 'sent'>(
    'idle',
  );
  const smtp = useAsyncData(() => api.smtpStatus(), []);
  const prefs = useAsyncData(() => api.listNotificationPrefs(), []);
  const [local, setLocal] = useState<Record<string, NotificationPref>>({});

  // Sorted here, in the viewer's language, rather than by the
  // database.
  //
  // `ORDER BY p.name` orders by the *database's* collation, which is
  // whichever the operator's image happens to have — and those differ:
  // our own compose ships postgres:18-alpine, which is musl, declares
  // en_US.utf8 and behaves as C. So the same Sentori listed these
  // projects in one order on our compose and another against a
  // customer's own Postgres, and neither order was the reader's.
  //
  // Locale-aware ordering belongs where the locale is known. This
  // console is trilingual; the database never knows which language is
  // on screen.
  const collator = useMemo(
    () => new Intl.Collator(locale, { numeric: true, sensitivity: 'base' }),
    [locale],
  );
  const rows = (prefs.data?.prefs ?? [])
    .map((p) => local[p.projectId] ?? p)
    // `projectId` breaks the tie, so equal names never reorder between
    // renders.
    .sort((a, b) =>
      collator.compare(a.projectName, b.projectName) ||
      a.projectId.localeCompare(b.projectId),
    );

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
          {smtp.data?.configured && (
            <div className="flex items-center gap-3 text-sm">
              {/* The dot follows the last delivery, not the config. It
                  was hard-coded green, so it stayed green through two
                  days of expired-certificate failures. */}
              <span
                className={clsx(
                  'h-2 w-2 shrink-0 rounded-full',
                  smtp.data.healthy ? 'bg-ok' : 'bg-kind-error',
                )}
              />
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
          {/* What the server said the last time it tried. Without this
              a run of failures is invisible here, and the delivery log
              is the only place it exists. */}
          {smtp.data?.lastStatus === 'failed' && (
            <div className="mt-3 rounded border border-kind-error/40 bg-kind-error/5 p-2.5 text-xs">
              <div className="font-medium text-kind-error">
                {t('notify.deliveryFailing', { count: String(smtp.data.failed7d) })}
              </div>
              {smtp.data.lastError && (
                <div className="mt-1 break-all font-mono text-fg-muted">
                  {smtp.data.lastError}
                </div>
              )}
              {smtp.data.lastDeliveredAt && (
                <div className="mt-1 text-fg-subtle">
                  {t('notify.lastDelivered')} <TimeAgo iso={smtp.data.lastDeliveredAt} />
                </div>
              )}
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

      <WebhookPanel />

      <Panel title={t('notify.prefsTitle')}>
        {prefs.error && (
          <div className="p-3.5">
            <ErrorBanner reason={prefs.error}>{t('notify.loadFailed')}</ErrorBanner>
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
