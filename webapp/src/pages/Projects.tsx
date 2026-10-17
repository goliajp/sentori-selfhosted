import { StatusLabel } from '../../shared/design/StatusLabel';
import { Action } from '../../shared/design/controls';
import { useDocumentSearchParams } from '../lib/search-params';
import { ArrowUpRight, Plus } from 'lucide-react';
import { useId, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { Heading, Surface } from '../../shared/design/Surface';
import { Button, Badge, Field, Input } from '../../shared/design/controls';
import { Dialog } from '../../shared/design/Dialog';
import { useShell } from '../App';
import { ErrorBanner, Select, TimeAgo } from '../components/ui';
import { useT } from '../i18n';
import { api, ApiError, type Project, type ProjectApp, type ProjectHealth } from '../lib/api';
import { platformLabel } from '../lib/platform-label';
import { releaseDisplayName } from '../lib/release-name';
import { appDisplayName, isIdentifiedApp } from '../lib/app-name';
import { formatApiError, useAsyncData } from '../lib/useAsyncData';

type Row = Project & { health: ProjectHealth | null; failure: string | null };
export default function ProjectsPage() {
  const currentSearchParams=useDocumentSearchParams();
  const t = useT(), projectForm=useId();
  const { me, projects, activeProject, setActiveProjectId, reloadProjects } = useShell();
  const owner = me.role === 'superadmin';
  const [params, setParams] = useSearchParams();
  const query = params.get('q') ?? '', platformFilter = params.get('platform') ?? '';
  const [editing, setEditing] = useState<Project | 'new' | null>(null);
  const [deleting, setDeleting] = useState<Project | null>(null);
  const [name, setName] = useState('');
  const [creatingApp, setCreatingApp] = useState<Project | null>(null);
  const [editingApp, setEditingApp] = useState<ProjectApp | null>(null);
  const [confirmation, setConfirmation] = useState('');
  const [conflict, setConflict] = useState(false);
  const [busy, setBusy] = useState(false), [failure, setFailure] = useState<string | null>(null);
  const { data, loading, reload } = useAsyncData<Row[]>(() => Promise.all(projects.map(async project => {
    try { return {...project, health: await api.projectHealth(project.id), failure: null}; }
    catch (error) { return {...project, health: null, failure: formatApiError(error)}; }
  })), [projects]);
  // Retain known cards while fetching, including freshly created projects.
  const rows = projects.map(project => ({health: null, failure: null, ...data?.find(row => row.id === project.id), ...project}));
  const visible = rows.filter(row => row.name.toLocaleLowerCase().includes(query.toLocaleLowerCase()) && (!platformFilter || row.health?.apps?.some(app => app.platform === platformFilter)));
  function filter(key: string, value: string) { const next = currentSearchParams(); if (value) next.set(key, value); else next.delete(key); setParams(next, { replace: true }); }
  function edit(project: Project | 'new') { setEditing(project); setName(project === 'new' ? '' : project.name); setFailure(null); setConflict(false); }
  function open(project: Project, page: string) { setActiveProjectId(project.id,{pathname:'/'+page}); }
  async function save(event: React.FormEvent) {
    event.preventDefault(); if (!owner || !editing || busy || !name.trim()) return;
    setBusy(true); setFailure(null);
    try {
      if (editing === 'new') await api.createProject(name.trim());
      else await api.updateProject(editing.id, {name: name.trim(), expectedVersion: editing.version});
      await reloadProjects(); setEditing(null);
    } catch (error) { setFailure(formatApiError(error)); setConflict(error instanceof ApiError && error.status === 409); }
    finally { setBusy(false); }
  }
  async function remove() {
    if (!owner || !deleting || confirmation !== deleting.name || busy) return;
    setBusy(true); setFailure(null);
    try {
      await api.deleteProject(deleting.id, deleting.version);
      await reloadProjects();
      if (activeProject?.id === deleting.id) setActiveProjectId(projects.find(p => p.id !== deleting.id)?.id ?? '');
      setDeleting(null);
    } catch (error) { setFailure(formatApiError(error)); setConflict(error instanceof ApiError && error.status === 409); }
    finally { setBusy(false); }
  }
  async function refreshConflict() {
    const target = editing && editing !== 'new' ? editing : deleting;
    if (!target || busy) return;
    setBusy(true);
    try {
      const fresh = await api.getProject(target.id);
      if (deleting) { setDeleting(fresh); setConfirmation(''); setFailure(null); setConflict(false); }
      else edit(fresh);
      await reloadProjects();
    } catch (error) { setFailure(formatApiError(error)); }
    finally { setBusy(false); }
  }
  const problem = failure && <><ErrorBanner reason={failure}>{t(conflict ? 'projects.conflict' : 'projects.actionFailed')}</ErrorBanner>{conflict && <Button onClick={refreshConflict} disabled={busy}>{t('projects.reload')}</Button>}</>;
  return <div className="production-page">
    <Heading title={t('nav.projects')} description={t('projects.description')} action={owner && <Button variant="primary" onClick={() => edit('new')}><Plus size={16}/>{t('settings.createProject')}</Button>} />
    <div className="control-bar">
      <Input type="search" aria-label={t('projects.search')} placeholder={t('projects.search')} value={query} onChange={e => filter('q', e.target.value)} />
      <Select aria-label={t('setup.platform')} value={platformFilter} onChange={e => filter('platform', e.target.value)}><option value="">{t('projects.allPlatforms')}</option>{[...new Set(rows.flatMap(row => row.health?.apps?.map(app => app.platform) ?? []))].map(value => <option key={value} value={value}>{platformLabel(value, t)}</option>)}</Select>
    </div>
    <div className="grid gap-5 xl:grid-cols-2" aria-busy={loading}>
      {visible.map(row => <Surface key={row.id} title={row.name} className="project-card" action={owner && <div className="flex flex-wrap gap-2"><Button small variant="ghost" onClick={() => edit(row)}>{t('projects.edit')}</Button><Button small variant="ghost" onClick={() => { setDeleting(row); setConfirmation(''); setFailure(null); setConflict(false); }}>{t('common.delete')}</Button></div>} footer={<><Button disabled={!row.health && !row.failure} onClick={() => open(row, row.health && !row.health.lastEventAt ? 'setup' : 'issues')}>{t(row.health && !row.health.lastEventAt ? 'setup.title' : 'projects.open')}<ArrowUpRight size={15}/></Button><Button variant="ghost" onClick={() => open(row, 'releases')}>{t('nav.releases')}</Button></>}>
        <div className="flex justify-between gap-3">{owner && <Button small onClick={() => setCreatingApp(row)}><Plus size={14}/>{t('apps.create')}</Button>}</div>
        <PlatformConnections health={row.health} onSetup={platform => setActiveProjectId(row.id,{pathname:'/setup',params:{platform}})}/>
        <div className="grid grid-cols-2 gap-6 mt-5"><div><p className="text-xs subtle">{t('projects.errors24h')}</p><p className="text-3xl tabular mt-2">{row.health?.counts24h.error ?? '—'}</p></div><div><p className="text-xs subtle">{t('apps.title')}</p><p className="text-3xl tabular mt-2" data-app-count>{row.health?.apps?.filter(isIdentifiedApp).length ?? '—'}</p></div></div>
        <div className="flex flex-wrap gap-x-6 gap-y-3 mt-5 text-sm tabular muted"><span>{t('projects.warnings24h')} · {row.health?.counts24h.warn ?? '—'}</span><span title={t('health.usersTitle')}>{t('projects.users24h')} · {row.health?.users24h ?? '—'}</span>{row.health && row.health.replay24h.eligible > 0 && <span>{t('health.replay')} · {row.health.replay24h.withScreens}/{row.health.replay24h.eligible}</span>}</div>
        {row.health?.backend && <div className="flex flex-wrap items-center gap-3 mt-4 text-sm muted" title={row.health.backend.url}><StatusLabel marker={<span className="h-2 w-2 rounded-full" style={{background: row.health.backend.lastOk === null ? 'var(--ui-muted)' : row.health.backend.lastOk ? 'var(--ui-success)' : 'var(--ui-danger)'}}/>}>{t('health.backend')}</StatusLabel>{row.health.backend.checks24h > 0 && <span>{((row.health.backend.ok24h / row.health.backend.checks24h) * 100).toFixed(1)}%</span>}{row.health.backend.lastLatencyMs !== null && <span>{row.health.backend.lastLatencyMs}ms</span>}</div>}
        <div className="project-app-list">{row.health?.apps?.filter(isIdentifiedApp).map(app => <div key={app.id} className="project-app" data-app-id={app.id}>
          <div className="flex flex-wrap items-center justify-between gap-3"><strong className="break-all">{appDisplayName(app,t)}</strong><div className="project-app-actions"><Badge>{app.kind === 'webapi' ? 'Web API' : app.kind === 'webapp' ? 'Web App' : platformLabel(app.platform,t)}</Badge><Badge tone={app.lastEventAt || app.lastSessionAt ? 'success' : 'neutral'}>{t(app.lastEventAt || app.lastSessionAt ? 'projects.connected' : app.releaseCount ? 'apps.filesOnly' : 'projects.waiting')}</Badge>{owner && <Button small variant="ghost" onClick={() => setEditingApp(app)}>{t('apps.edit')}</Button>}</div></div>
          {app.identitySource==='release' && <p className="text-xs subtle">{t('apps.inferredName')}</p>}
          <p className="text-sm muted">{t('projects.latestRelease')} · {app.latestRelease ? <Action className="text-link" onClick={() => setActiveProjectId(row.id,{pathname:'/releases',params:{app:app.id}})}>{releaseDisplayName({name:app.latestRelease.name,apps:[app]},t)}</Action> : '—'}</p>
          {app.repositoryUrl && <a className="text-link text-sm break-all" href={app.repositoryUrl} target="_blank" rel="noreferrer">{app.repositoryUrl}<ArrowUpRight size={14}/></a>}
        </div>)}</div>
        {!!row.health?.apps?.some(app=>!isIdentifiedApp(app)) && <details className="unidentified-apps">
          <summary>{t('apps.unidentifiedData')}</summary>
          <div className="unidentified-apps-content"><p>{t('apps.unidentifiedHint')}</p>{row.health.apps.filter(app=>!isIdentifiedApp(app)).map(app=><div key={app.id} data-unidentified-app-id={app.id} className="flex flex-wrap justify-between gap-3">
            <span>{appDisplayName(app,t)}</span><span>{t('apps.events24h',{count:String(app.events24h)})}</span>
          </div>)}</div>
        </details>}
        {row.health && !row.health.apps?.length && <p className="text-sm muted mt-5">{t('apps.empty')}</p>}
        <p className="text-xs subtle mt-5">{t('projects.lastEvent')} · {row.health?.lastEventAt ? <TimeAgo iso={row.health.lastEventAt}/> : row.health ? t('health.silent') : '—'}</p>
        {row.failure && <div className="mt-4"><ErrorBanner reason={row.failure}>{t('projects.healthUnavailable')}</ErrorBanner><Button small onClick={reload}>{t('common.retry')}</Button></div>}
      </Surface>)}
    </div>
    {!visible.length && <Surface><p>{t(projects.length ? 'projects.noMatches' : 'projects.emptyTitle')}</p><p className="muted mt-3">{projects.length ? t('projects.filterHint') : t(owner ? 'projects.emptyHint' : 'setup.askOwner')}</p>{projects.length > 0 && <Button className="mt-5" onClick={() => { const next = currentSearchParams(); next.delete('q'); next.delete('platform'); setParams(next, {replace: true}); }}>{t('projects.clearFilters')}</Button>}</Surface>}
    <Dialog open={!!editing} onClose={() => { if (!busy) setEditing(null); }} closeLabel={t('common.close')} title={t(editing === 'new' ? 'settings.createProject' : 'projects.edit')} footer={<Button type="submit" form={projectForm} variant="primary" busy={busy}>{t('settings.save')}</Button>}>
      <form id={projectForm} className="form-stack" onSubmit={save}>
        <Field label={t('settings.projectName')}><Input required pattern={String.raw`.*\S.*`} maxLength={200} value={name} onChange={e => setName(e.target.value)} data-autofocus disabled={busy}/></Field>
        {problem}
      </form>
    </Dialog>
    {creatingApp && <CreateApp project={creatingApp} onClose={()=>setCreatingApp(null)} onSaved={()=>{setCreatingApp(null);reload();}}/>}
    {editingApp && <EditApp app={editingApp} onClose={() => setEditingApp(null)} onSaved={() => {setEditingApp(null); reload();}}/>}
    <Dialog open={!!deleting} onClose={() => { if (!busy) setDeleting(null); }} closeLabel={t('common.close')} title={t('settings.deleteProjectConfirm', {name: deleting?.name ?? ''})} footer={<Button variant="danger" busy={busy} disabled={confirmation !== deleting?.name} onClick={remove}>{t('common.delete')}</Button>}>
      <Field label={t('projects.confirmName')} hint={deleting?.name}><Input value={confirmation} onChange={e => setConfirmation(e.target.value)} autoComplete="off" disabled={busy} data-autofocus /></Field>
      {problem}
    </Dialog>
  </div>;
}

function PlatformConnections({health,onSetup}:{health:ProjectHealth|null;onSetup:(platform:string)=>void}) {
  const t=useT();
  return <div className="flex flex-wrap gap-2" aria-label={t('apps.platforms')}>
    {['javascript','ios','android','web','weapp'].map(platform=>{
      const connected=health?.apps?.some(app=>app.platform===platform && (app.lastEventAt || app.lastSessionAt));
      const kind=platform==='ios'?'dsym':platform==='android'?'proguard':platform==='javascript'?'sourcemap':null;
      const evidence=health?.artifactSummary?.find(row=>row.kind===kind);
      const state=connected?'projects.connected':evidence?.broken?'releaseView.unusable':evidence?.unchecked?'releaseView.unchecked':evidence?.usable?'apps.filesOnly':'projects.waiting';
      const color=connected?'var(--ui-success)':evidence?.broken?'var(--ui-danger)':evidence?.usable&&!evidence.unchecked?'var(--ui-success)':'var(--ui-muted)';
      return <Button small variant="ghost" key={platform} onClick={()=>onSetup(platform)} title={t(state)}><span aria-hidden className="h-2 w-2 rounded-full" style={{background:color}}/>{platformLabel(platform,t)}<span className="sr-only"> · {t(state)}</span></Button>;
    })}
  </div>;
}

function EditApp({app,onClose,onSaved}:{app:ProjectApp;onClose:()=>void;onSaved:()=>void}) {
  const t=useT(), appForm=useId();
  const [name,setName]=useState(app.name??''),[repositoryUrl,setRepositoryUrl]=useState(app.repositoryUrl??'');
  const [kind,setKind]=useState(app.kind ?? 'unknown');
  const [busy,setBusy]=useState(false),[failure,setFailure]=useState<string|null>(null);
  async function save(event:React.FormEvent) {
    event.preventDefault();if(busy)return;setBusy(true);setFailure(null);
    try {await api.updateApp(app.projectId,app.id,{name:name.trim(),repositoryUrl:repositoryUrl.trim(),kind,expectedVersion:app.version});onSaved();}
    catch(error){setFailure(error instanceof ApiError && error.status===409?t('apps.conflict'):formatApiError(error));}
    finally{setBusy(false);}
  }
  return <Dialog open onClose={()=>{if(!busy)onClose();}} title={t('apps.edit')} closeLabel={t('common.close')} footer={<Button type="submit" form={appForm} variant="primary" busy={busy}>{t('settings.save')}</Button>}>
    <form id={appForm} className="form-stack" onSubmit={save}>
      <Badge>{app.kind === 'webapi' ? 'Web API' : app.kind === 'webapp' ? 'Web App' : platformLabel(app.platform,t)}</Badge>
      <Field label={t('apps.kind')}><Select value={kind} onChange={event=>setKind(event.target.value)} disabled={busy}>{['ios','android','webapi','webapp','weapp','unknown'].map(value=><option key={value} value={value}>{value === 'webapi' ? 'Web API' : value === 'webapp' ? 'Web App' : platformLabel(value,t)}</option>)}</Select></Field>
      <Field label={t('apps.name')}><Input maxLength={200} value={name} onChange={event=>setName(event.target.value)} disabled={busy} data-autofocus/></Field>
      <dl className="app-identity">
        <div><dt>{t('apps.nameSource')}</dt><dd>{t(app.identitySource==='configured'?'apps.sourceConfigured':app.identitySource==='reported'?'apps.sourceReported':app.identitySource==='release'?'apps.sourceRelease':'apps.sourceUnknown')}</dd></div>
        <div><dt>{t('apps.identifier')}</dt><dd>{app.identifier || '—'}</dd></div>
      </dl>
      <Field label={t('apps.repository')} hint={t('apps.repositoryHint')}><Input type="url" maxLength={2048} value={repositoryUrl} onChange={event=>setRepositoryUrl(event.target.value)} disabled={busy}/></Field>
      {failure && <ErrorBanner reason={failure}>{t('projects.actionFailed')}</ErrorBanner>}
    </form>
  </Dialog>;
}

function CreateApp({project,onClose,onSaved}:{project:Project;onClose:()=>void;onSaved:()=>void}) {
  const t=useT(), formId=useId();
  const [name,setName]=useState(''), [identifier,setIdentifier]=useState(''), [kind,setKind]=useState('ios');
  const [busy,setBusy]=useState(false), [failure,setFailure]=useState<string|null>(null);
  async function save(event:React.FormEvent) {
    event.preventDefault(); if(busy)return; setBusy(true);setFailure(null);
    const platform=kind==='webapi'?'javascript':kind==='webapp'?'web':kind;
    try {await api.createApp(project.id,{name:name.trim(),identifier:identifier.trim(),kind,platform});onSaved();}
    catch(error){setFailure(formatApiError(error));}finally{setBusy(false);}
  }
  return <Dialog open title={t('apps.create')} closeLabel={t('common.close')} onClose={()=>{if(!busy)onClose();}} footer={<Button type="submit" form={formId} variant="primary" busy={busy}>{t('settings.save')}</Button>}>
    <form id={formId} className="form-stack" onSubmit={save}>
      <p>{project.name}</p>
      <Field label={t('apps.name')}><Input required maxLength={200} value={name} onChange={e=>setName(e.target.value)} disabled={busy} data-autofocus/></Field>
      <Field label={t('apps.kind')}><Select value={kind} onChange={e=>setKind(e.target.value)} disabled={busy}>{['ios','android','webapi','webapp','weapp'].map(value=><option key={value} value={value}>{value==='webapi'?'Web API':value==='webapp'?'Web App':platformLabel(value,t)}</option>)}</Select></Field>
      <Field label={t('apps.identifier')} hint={t('apps.identifierHint')}><Input required maxLength={200} value={identifier} onChange={e=>setIdentifier(e.target.value)} disabled={busy}/></Field>
      {failure && <ErrorBanner reason={failure}>{t('projects.actionFailed')}</ErrorBanner>}
    </form>
  </Dialog>;
}
