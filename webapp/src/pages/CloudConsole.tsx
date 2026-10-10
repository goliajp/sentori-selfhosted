import { CloudWorkspaceDetail } from './CloudWorkspaceDetail';
import { useRef, useState } from 'react';
import { Link, useLocation, useNavigate, useSearchParams } from 'react-router-dom';
import { Heading, Surface } from '../../shared/design/Surface';
import { Button } from '../../shared/design/controls';
import { Dialog } from '../../shared/design/Dialog';
import { PublicLayout } from '../components/PublicLayout';
import { ErrorBanner, Field, Input, Select, TimeAgo } from '../components/ui';
import { useT } from '../i18n';
import { cloudApi, type CloudAccount, type CloudOrganization } from '../lib/cloud-api';
import { formatApiError, useAsyncData } from '../lib/useAsyncData';

export default function CloudConsole() {
  const t = useT(), path = useLocation().pathname, navigate = useNavigate();
  const account = useAsyncData(() => cloudApi.me(), []);
  const [failure, setFailure] = useState<string|null>(null), [busy, setBusy] = useState(false);
  async function logout() {
    setBusy(true); setFailure(null);
    try { await cloudApi.logout(); navigate('/cloud/login', {replace:true}); }
    catch (error) { setFailure(formatApiError(error)); }
    finally { setBusy(false); }
  }
  return <PublicLayout><div className="production-page cloud-console">
    <Heading title={t('cloud.console')} description={account.data?.email} action={account.data && <Button disabled={busy} onClick={() => void logout()}>{t('cloud.signOut')}</Button>}/>
    <nav className="flex flex-wrap gap-4 mb-6" aria-label={t('cloud.console')}><Link className="text-link" to="/cloud">{t('cloud.organizations')}</Link><Link className="text-link" to="/cloud/account">{t('settings.tab.account')}</Link></nav>
    {(failure || account.error) && <><ErrorBanner reason={failure || account.error}>{t('cloud.actionFailed')}</ErrorBanner><Button onClick={account.reload}>{t('common.retry')}</Button></>}
    {account.loading && !account.data && <p role="status">{t('shell.loading')}</p>}
    {account.data && (path === '/cloud/account' ? <CloudAccountSettings/> : path.startsWith('/cloud/workspaces/') ? <CloudWorkspaceDetail account={account.data}/> : <Organizations account={account.data}/>)}
  </div></PublicLayout>;
}

function CloudAccountSettings() {
  const t = useT(), navigate = useNavigate();
  const [current, setCurrent] = useState(''), [password, setPassword] = useState(''), [confirm, setConfirm] = useState('');
  const [busy, setBusy] = useState(false), [failure, setFailure] = useState<string|null>(null), locked = useRef(false);
  async function save(event: React.FormEvent) {
    event.preventDefault(); if (locked.current) return;
    if (password !== confirm) { setFailure(t('auth.passwordsDiffer')); return; }
    locked.current = true; setBusy(true); setFailure(null);
    try { await cloudApi.password(current,password); setCurrent(''); setPassword(''); setConfirm(''); navigate('/cloud/login', {replace:true}); }
    catch (error) { setFailure(formatApiError(error)); }
    finally { locked.current = false; setBusy(false); }
  }
  return <Surface title={t('settings.changePassword')}><form className="form-stack max-w-xl" onSubmit={save}>
    <p className="text-sm text-fg-muted">{t('cloud.passwordSessions')}</p>
    <Field label={t('settings.currentPassword')}><Input type="password" autoComplete="current-password" required value={current} disabled={busy} onChange={event => setCurrent(event.target.value)}/></Field>
    <Field label={t('auth.newPassword12')}><Input type="password" autoComplete="new-password" required minLength={12} maxLength={1024} value={password} disabled={busy} onChange={event => setPassword(event.target.value)}/></Field>
    <Field label={t('auth.confirm')}><Input type="password" autoComplete="new-password" required minLength={12} maxLength={1024} value={confirm} disabled={busy} onChange={event => setConfirm(event.target.value)}/></Field>
    {failure && <ErrorBanner reason={failure}>{t('cloud.actionFailed')}</ErrorBanner>}
    <Button variant="primary" type="submit" busy={busy}>{t('settings.changePassword')}</Button>
  </form></Surface>;
}

function Organizations({account}: {account:CloudAccount}) {
  const t = useT(), [params, setParams] = useSearchParams(), [offset,setOffset] = useState(0);
  const selected = params.get('organization');
  const organizations = useAsyncData(() => cloudApi.organizations(offset), [offset]);
  const [inviteOffset,setInviteOffset] = useState(0);
  const invitations = useAsyncData(() => cloudApi.invitations(inviteOffset), [inviteOffset]);
  const [name,setName] = useState(''), [busy,setBusy] = useState(false), [failure,setFailure] = useState<string|null>(null), locked = useRef(false);
  const current = organizations.data?.items.find(row => row.id === selected);
  async function create(event:React.FormEvent) {
    event.preventDefault(); if (locked.current) return;
    locked.current = true; setBusy(true); setFailure(null);
    try { const result = await cloudApi.command<CloudOrganization>(account.id,'/organizations',{name:name.trim()}); setName(''); setOffset(0); organizations.reload(); setParams({organization:result.id}); }
    catch (error) { setFailure(formatApiError(error)); }
    finally { locked.current = false; setBusy(false); }
  }
  async function accept(id:string) {
    if (locked.current) return;
    locked.current = true; setBusy(true); setFailure(null);
    try { await cloudApi.acceptInvitation(id); setInviteOffset(0); invitations.reload(); organizations.reload(); }
    catch (error) { setFailure(formatApiError(error)); }
    finally { locked.current = false; setBusy(false); }
  }
  return <div className="form-stack">
    {(failure || organizations.error || invitations.error) && <><ErrorBanner reason={failure || organizations.error || invitations.error}>{t('cloud.actionFailed')}</ErrorBanner><Button onClick={() => {organizations.reload(); invitations.reload();}}>{t('common.retry')}</Button></>}
    {invitations.data && (invitations.data.items.length > 0 || inviteOffset > 0) && <Surface title={t('cloud.invitations')}><ul className="form-stack">{invitations.data.items.map(row => <li key={row.id} className="flex flex-wrap items-center justify-between gap-3"><span className="break-words">{row.organizationName}</span><Button disabled={busy} onClick={() => void accept(row.id)}>{t('cloud.acceptInvitation')}</Button></li>)}</ul><div className="flex gap-3 mt-4"><Button disabled={inviteOffset===0 || invitations.loading} onClick={() => setInviteOffset(Math.max(0,inviteOffset-100))}>{t('cloud.previous')}</Button><Button disabled={invitations.data.nextOffset==null || invitations.loading} onClick={() => setInviteOffset(invitations.data!.nextOffset!)}>{t('cloud.next')}</Button></div></Surface>}
    <Surface title={t('cloud.organizations')}>
      {organizations.loading && <p role="status">{t('shell.loading')}</p>}
      <div className="flex flex-wrap gap-3">{organizations.data?.items.map(row => <Button key={row.id} variant={selected === row.id ? 'primary':'secondary'} onClick={() => setParams({organization:row.id})}>{row.name}</Button>)}</div>
      {organizations.data?.items.length === 0 && <p>{t('table.empty')}</p>}
      <div className="flex gap-3 mt-4"><Button disabled={offset === 0 || organizations.loading} onClick={() => setOffset(Math.max(0,offset-100))}>{t('cloud.previous')}</Button><Button disabled={organizations.data?.nextOffset == null || organizations.loading} onClick={() => setOffset(organizations.data!.nextOffset!)}>{t('cloud.next')}</Button></div>
      <form onSubmit={create} className="form-stack mt-6 max-w-xl"><Field label={t('cloud.organizationName')}><Input required maxLength={80} value={name} onChange={event => setName(event.target.value)} disabled={busy}/></Field><Button type="submit" busy={busy} disabled={!name.trim()}>{t('cloud.createOrganization')}</Button></form>
    </Surface>
    {selected && <Organization key={selected} id={selected} account={account} fallbackRole={current?.role}/>}
  </div>;
}

function Organization({id,account,fallbackRole}: {id:string;account:CloudAccount;fallbackRole?:CloudOrganization['role']}) {
  const t = useT(), [offset,setOffset] = useState(0);
  const info = useAsyncData(() => cloudApi.organization(id),[id]);
  const workspaces = useAsyncData(() => cloudApi.workspaces(id,offset),[id,offset]);
  const catalog = useAsyncData(() => cloudApi.catalog(),[]);
  const [orgName,setOrgName] = useState('');
  const [workspaceName,setWorkspaceName] = useState(''), [region,setRegion] = useState(''), [email,setEmail] = useState('');
  const [busy,setBusy] = useState(false), [failure,setFailure] = useState<string|null>(null), locked = useRef(false);
  const owner = (info.data?.members.find(row => row.id === account.id)?.role ?? fallbackRole) === 'owner';
  const [pending,setPending] = useState<{id:string;email:string;action:'setRole'|'removeMember';role?:'owner'|'member';version:number}|null>(null);
  const selectedRegion = region || catalog.data?.catalog?.regions[0]?.id || '';
  async function run(path:string,body:Record<string,unknown>) {
    if (locked.current) return;
    locked.current = true; setBusy(true); setFailure(null);
    try { await cloudApi.command(account.id,path,body); setPending(null); info.reload(); workspaces.reload(); setEmail(''); setWorkspaceName(''); setOrgName(''); }
    catch (error) { setFailure(formatApiError(error)); }
    finally { locked.current = false; setBusy(false); }
  }
  async function enter(workspace:string) {
    if (locked.current) return;
    locked.current = true; setBusy(true); setFailure(null);
    try { location.assign(await cloudApi.entry(workspace)); }
    catch (error) { setFailure(formatApiError(error)); locked.current = false; setBusy(false); }
  }
  return <div className="form-stack">
    {(failure || info.error || workspaces.error || catalog.error) && <><ErrorBanner reason={failure || info.error || workspaces.error || catalog.error}>{t('cloud.actionFailed')}</ErrorBanner><Button onClick={() => {info.reload();workspaces.reload();catalog.reload();}}>{t('common.retry')}</Button></>}
    <Surface title={info.data?.organization.name ?? t('cloud.workspaces')}>
      <ul className="form-stack">{workspaces.data?.items.map(row => <li key={row.id} className="rounded-lg border border-border p-4 flex flex-wrap items-center gap-4"><div className="min-w-0 flex-1"><h3 className="font-semibold break-words">{row.name}</h3><p className="text-sm text-fg-muted">{row.region} · {t(`cloud.state.${row.status}`)}</p></div>{row.status === 'ready' && <Button disabled={busy} onClick={() => void enter(row.id)}>{t('cloud.enter')}</Button>}<Link className="text-link" to={`/cloud/workspaces/${row.id}?organization=${encodeURIComponent(id)}`}>{t('cloud.manageWorkspace')}</Link></li>)}</ul>
      {workspaces.data?.items.length === 0 && <p>{t('table.empty')}</p>}
      <div className="flex gap-3 mt-4"><Button disabled={offset===0 || workspaces.loading} onClick={() => setOffset(Math.max(0,offset-100))}>{t('cloud.previous')}</Button><Button disabled={workspaces.data?.nextOffset==null || workspaces.loading} onClick={() => setOffset(workspaces.data!.nextOffset!)}>{t('cloud.next')}</Button></div>
      {owner && catalog.data?.catalog && <form className="form-stack mt-6 max-w-xl" onSubmit={event => {event.preventDefault(); void run('/organizations/'+id+'/workspaces',{name:workspaceName.trim(),region:selectedRegion});}}><Field label={t('cloud.workspaceName')}><Input required maxLength={80} value={workspaceName} disabled={busy} onChange={event => setWorkspaceName(event.target.value)}/></Field><Field label={t('cloud.region')}><Select value={selectedRegion} disabled={busy} onChange={event => setRegion(event.target.value)}>{catalog.data.catalog.regions.map(row => <option key={row.id} value={row.id}>{row.name}</option>)}</Select></Field><Button type="submit" busy={busy} disabled={!workspaceName.trim()}>{t('cloud.createWorkspace')}</Button></form>}
      {owner && catalog.data && !catalog.data.configured && <p className="mt-4">{t('cloud.catalogUnavailable')}</p>}
    </Surface>
    {owner && info.data && <Surface title={t('cloud.renameOrganization')}><form className="form-stack max-w-xl" onSubmit={event => {event.preventDefault(); void run('/organizations/'+id+'/commands',{expectedVersion:info.data!.organization.version,action:'rename',name:orgName.trim()});}}><Field label={t('cloud.renameOrganization')}><Input required maxLength={80} value={orgName} placeholder={info.data.organization.name} disabled={busy} onChange={event => setOrgName(event.target.value)}/></Field><Button type="submit" busy={busy} disabled={!orgName.trim()}>{t('cloud.renameOrganization')}</Button></form></Surface>}
    {info.data && <Surface title={t('cloud.members')}>
      <ul className="form-stack">{info.data.members.map(row => <li key={row.id} className="flex flex-wrap gap-3 items-center"><span className="min-w-0 flex-1 break-all">{row.email}</span><span>{t(row.role === 'owner' ? 'cloud.orgOwner':'cloud.orgMember')}</span>{owner && <><Button disabled={busy || (row.role === 'owner' && info.data!.members.filter(member => member.role === 'owner').length === 1)} onClick={() => setPending({id:row.id,email:row.email,action:'setRole',role:row.role === 'owner' ? 'member':'owner',version:info.data!.organization.version})}>{t(row.role === 'owner' ? 'cloud.makeMember':'cloud.makeOwner')}</Button><Button disabled={busy || row.id === account.id} onClick={() => setPending({id:row.id,email:row.email,action:'removeMember',version:info.data!.organization.version})}>{t('cloud.removeMember')}</Button></>}</li>)}</ul>
      {owner && <form className="form-stack mt-6 max-w-xl" onSubmit={event => {event.preventDefault(); void run('/organizations/'+id+'/commands',{expectedVersion:info.data!.organization.version,action:'invite',email:email.trim()});}}><p className="text-sm text-fg-muted">{t('cloud.inviteHint')}</p><Field label={t('auth.email')}><Input type="email" required value={email} disabled={busy} onChange={event => setEmail(event.target.value)}/></Field><Button busy={busy} type="submit">{t('cloud.invite')}</Button></form>}
      {owner && <ul className="form-stack mt-4">{info.data.invitations.map(row => <li key={row.id} className="flex flex-wrap gap-3 items-center"><span className="min-w-0 flex-1 break-all">{row.email}</span><TimeAgo iso={row.expiresAt}/><Button disabled={busy} onClick={() => void run('/organizations/'+id+'/commands',{expectedVersion:info.data!.organization.version,action:'revokeInvitation',id:row.id})}>{t('cloud.revokeInvitation')}</Button></li>)}</ul>}
    </Surface>}
    <Dialog open={pending!==null} title={t(pending?.action === 'removeMember' ? 'cloud.removeMember' : pending?.role === 'owner' ? 'cloud.makeOwner':'cloud.makeMember')} closeLabel={t('common.close')} onClose={() => {if (!busy) setPending(null);}} footer={<><Button disabled={busy} onClick={() => setPending(null)}>{t('common.close')}</Button><Button variant="primary" busy={busy} onClick={() => pending && void run('/organizations/'+id+'/commands',{expectedVersion:pending.version,action:pending.action,id:pending.id,...(pending.role ? {role:pending.role}:{})})}>{t(pending?.action === 'removeMember' ? 'cloud.removeMember' : pending?.role === 'owner' ? 'cloud.makeOwner':'cloud.makeMember')}</Button></>}><p>{t('cloud.memberChangeHint',{email:pending?.email ?? ''})}</p>{failure && <ErrorBanner reason={failure}>{t('cloud.actionFailed')}</ErrorBanner>}</Dialog>
  </div>;
}
