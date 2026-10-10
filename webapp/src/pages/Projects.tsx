import { ArrowUpRight, Plus } from 'lucide-react';
import { useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { Heading, Surface } from '../../shared/design/Surface';
import { Button, Badge, Field, Input } from '../../shared/design/controls';
import { Dialog } from '../../shared/design/Dialog';
import { useShell } from '../App';
import { ErrorBanner, Select, TimeAgo, formatRelease } from '../components/ui';
import { useT } from '../i18n';
import { api, ApiError, type Project, type ProjectHealth } from '../lib/api';
import { platformLabel } from '../lib/platform-label';
import { SETUP_PLATFORMS } from '../lib/setup';
import { formatApiError, useAsyncData } from '../lib/useAsyncData';

type Row = Project & { health: ProjectHealth | null; failure: string | null };
export default function ProjectsPage() {
  const t = useT(), navigate = useNavigate();
  const { me, projects, activeProject, setActiveProjectId, reloadProjects } = useShell();
  const owner = me.role === 'superadmin';
  const [params, setParams] = useSearchParams();
  const query = params.get('q') ?? '', platformFilter = params.get('platform') ?? '';
  const [editing, setEditing] = useState<Project | 'new' | null>(null);
  const [deleting, setDeleting] = useState<Project | null>(null);
  const [name, setName] = useState(''), [platform, setPlatform] = useState('web');
  const [confirmation, setConfirmation] = useState('');
  const [conflict, setConflict] = useState(false);
  const [busy, setBusy] = useState(false), [failure, setFailure] = useState<string | null>(null);
  const { data, loading, reload } = useAsyncData<Row[]>(() => Promise.all(projects.map(async project => {
    try { return {...project, health: await api.projectHealth(project.id), failure: null}; }
    catch (error) { return {...project, health: null, failure: formatApiError(error)}; }
  })), [projects]);
  // Retain known cards while fetching, including freshly created projects.
  const rows = projects.map(project => ({health: null, failure: null, ...data?.find(row => row.id === project.id), ...project}));
  const visible = rows.filter(row => row.name.toLocaleLowerCase().includes(query.toLocaleLowerCase()) && (!platformFilter || row.platform === platformFilter));
  function filter(key: string, value: string) { const next = new URLSearchParams(params); if (value) next.set(key, value); else next.delete(key); setParams(next, { replace: true }); }
  function edit(project: Project | 'new') { setEditing(project); setName(project === 'new' ? '' : project.name); setPlatform(project === 'new' ? 'web' : project.platform || ''); setFailure(null); setConflict(false); }
  function open(project: Project, page: string) { setActiveProjectId(project.id); navigate({pathname: '/' + page, search: new URLSearchParams({project: project.id}).toString()}); }
  async function save(event: React.FormEvent) {
    event.preventDefault(); if (!owner || !editing || busy || !name.trim()) return;
    setBusy(true); setFailure(null);
    try {
      if (editing === 'new') await api.createProject(name.trim(), platform);
      else await api.updateProject(editing.id, {name: name.trim(), platform, expectedVersion: editing.version});
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
      <Select aria-label={t('setup.platform')} value={platformFilter} onChange={e => filter('platform', e.target.value)}><option value="">{t('projects.allPlatforms')}</option>{[...new Set(projects.map(p => p.platform).filter((value): value is string => !!value))].map(value => <option key={value} value={value}>{platformLabel(value, t)}</option>)}</Select>
    </div>
    <div className="grid gap-5 xl:grid-cols-2" aria-busy={loading}>
      {visible.map(row => <Surface key={row.id} title={row.name} className="project-card" action={owner && <div className="flex flex-wrap gap-2"><Button small variant="ghost" onClick={() => edit(row)}>{t('projects.edit')}</Button><Button small variant="ghost" onClick={() => { setDeleting(row); setConfirmation(''); setFailure(null); setConflict(false); }}>{t('common.delete')}</Button></div>} footer={<><Button disabled={!row.health && !row.failure} onClick={() => open(row, row.health && !row.health.lastEventAt ? 'setup' : 'issues')}>{t(row.health && !row.health.lastEventAt ? 'setup.title' : 'projects.open')}<ArrowUpRight size={15}/></Button><Button variant="ghost" onClick={() => open(row, 'releases')}>{t('nav.releases')}</Button></>}>
        <div className="flex flex-wrap gap-3 items-center mb-5"><Badge>{platformLabel(row.platform, t)}</Badge><Badge tone={row.health?.lastEventAt ? 'info' : 'neutral'}>{row.failure ? t('projects.healthUnavailable') : row.health?.lastEventAt ? t('projects.connected') : t(loading ? 'projects.loading' : 'projects.waiting')}</Badge></div>
        <div className="grid grid-cols-2 gap-6"><div><p className="text-xs subtle">{t('projects.errors24h')}</p><p className="text-3xl tabular mt-2">{row.health?.counts24h.error ?? '—'}</p></div><div><p className="text-xs subtle">{t('projects.latestRelease')}</p><p className="text-3xl tabular mt-2 break-all">{row.health?.latestRelease ? formatRelease(row.health.latestRelease) : '—'}</p></div></div>
        <div className="flex flex-wrap gap-x-6 gap-y-3 mt-5 text-sm tabular muted"><span>{t('projects.warnings24h')} · {row.health?.counts24h.warn ?? '—'}</span><span title={t('health.usersTitle')}>{t('projects.users24h')} · {row.health?.users24h ?? '—'}</span>{row.health && row.health.replay24h.eligible > 0 && <span>{t('health.replay')} · {row.health.replay24h.withScreens}/{row.health.replay24h.eligible}</span>}</div>
        {row.health?.backend && <div className="flex flex-wrap items-center gap-3 mt-4 text-sm muted" title={row.health.backend.url}><span className="h-2 w-2 rounded-full" aria-hidden style={{background: row.health.backend.lastOk === null ? 'var(--ui-muted)' : row.health.backend.lastOk ? 'var(--ui-success)' : 'var(--ui-danger)'}}/><span>{t('health.backend')}</span>{row.health.backend.checks24h > 0 && <span>{((row.health.backend.ok24h / row.health.backend.checks24h) * 100).toFixed(1)}%</span>}{row.health.backend.lastLatencyMs !== null && <span>{row.health.backend.lastLatencyMs}ms</span>}</div>}
        {row.health?.latestRelease && <div className="mt-4"><ArtifactLights kinds={row.health.latestReleaseArtifacts} used={row.health.platforms24h}/></div>}
        <p className="text-xs subtle mt-5">{t('projects.lastEvent')} · {row.health?.lastEventAt ? <TimeAgo iso={row.health.lastEventAt}/> : row.health ? t('health.silent') : '—'}</p>
        {row.failure && <div className="mt-4"><ErrorBanner reason={row.failure}>{t('projects.healthUnavailable')}</ErrorBanner><Button small onClick={reload}>{t('common.retry')}</Button></div>}
      </Surface>)}
    </div>
    {!visible.length && <Surface><p>{t(projects.length ? 'projects.noMatches' : 'projects.emptyTitle')}</p><p className="muted mt-3">{projects.length ? t('projects.filterHint') : t(owner ? 'projects.emptyHint' : 'setup.askOwner')}</p>{projects.length > 0 && <Button className="mt-5" onClick={() => { const next = new URLSearchParams(params); next.delete('q'); next.delete('platform'); setParams(next, {replace: true}); }}>{t('projects.clearFilters')}</Button>}</Surface>}
    <Dialog open={!!editing} onClose={() => { if (!busy) setEditing(null); }} closeLabel={t('common.close')} title={t(editing === 'new' ? 'settings.createProject' : 'projects.edit')} footer={<Button type="submit" form="project-edit" variant="primary" busy={busy}>{t('settings.save')}</Button>}>
      <form id="project-edit" className="form-stack" onSubmit={save}>
        <Field label={t('settings.projectName')}><Input required pattern={String.raw`.*\S.*`} maxLength={200} value={name} onChange={e => setName(e.target.value)} data-autofocus disabled={busy}/></Field>
        <Field label={t('setup.platform')}><Select value={platform} onChange={e => setPlatform(e.target.value)} disabled={busy}>{!SETUP_PLATFORMS.includes(platform as typeof SETUP_PLATFORMS[number]) && <option value={platform}>{platformLabel(platform, t)}</option>}{SETUP_PLATFORMS.map(value => <option key={value} value={value}>{platformLabel(value, t)}</option>)}</Select></Field>
        {problem}
      </form>
    </Dialog>
    <Dialog open={!!deleting} onClose={() => { if (!busy) setDeleting(null); }} closeLabel={t('common.close')} title={t('settings.deleteProjectConfirm', {name: deleting?.name ?? ''})} footer={<Button variant="danger" busy={busy} disabled={confirmation !== deleting?.name} onClick={remove}>{t('common.delete')}</Button>}>
      <Field label={t('projects.confirmName')} hint={deleting?.name}><Input value={confirmation} onChange={e => setConfirmation(e.target.value)} autoComplete="off" disabled={busy} data-autofocus /></Field>
      {problem}
    </Dialog>
  </div>;
}

function ArtifactLights({
  kinds,
  used,
}: {
  kinds: string[];
  used: Record<string, number>;
}) {
  const t = useT();
  // JS runs on every platform that reports at all, so a sourcemap is
  // owed whenever anything is coming in.
  const anyTraffic = Object.values(used).some((n) => n > 0);
  const lights: [string, string, boolean][] = [
    ['js', 'sourcemap', anyTraffic],
    ['ios', 'dsym', (used.ios ?? 0) > 0],
    ['android', 'proguard', (used.android ?? 0) > 0],
  ];
  return (
    <span className="flex shrink-0 gap-1.5 text-xs">
      {lights.map(([label, kind, live]) => {
        const have = kinds.includes(kind);
        return (
          <span
            key={label}
            title={
              have
                ? undefined
                : live
                  ? t('releases.artifactMissing', { platform: label })
                  : t('releases.artifactUnused', { platform: label })
            }
            style={{
              color: have
                ? 'var(--s-kind-probe)'
                : live
                  ? 'var(--s-kind-error)'
                  : 'var(--sn-fg-subtle)',
            }}
          >
            {label}
          </span>
        );
      })}
    </span>
  );
}
