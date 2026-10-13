import { StatusLabel } from '../../shared/design/StatusLabel';
import { Activity, Suspense, useCallback, useContext, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { Link, UNSAFE_LocationContext, UNSAFE_NavigationContext, useLocation, useNavigate, type Location, type To } from 'react-router-dom';
import { Box, ChevronDown, CircleDot, X } from 'lucide-react';
import { WorkbenchLayout } from '../../shared/design/WorkbenchLayout';
import { DocumentTabs } from '../../shared/design/DocumentTabs';
import { Picker } from '../../shared/design/Picker';
import { Button } from '../../shared/design/controls';
import { useI18n } from '../i18n';
import type { MessageKey } from '../i18n/en';
import { DocumentInspectionContext, DocumentRuntimeContext } from '../lib/document-context';
import { WorkbenchEmpty } from './WorkbenchEmpty';
import { platformLabel } from '../lib/platform-label';
import { issueHeadline } from '../lib/issue-title';
import { api } from '../lib/api';
import { useScopedAsyncData } from '../lib/useScopedAsyncData';
import { DocumentProject, useShell } from '../lib/shell-context';

type OpenDocument = { key: string; projectId: string; location: Location; context: React.ContextType<typeof UNSAFE_LocationContext>; content: ReactNode; title?: string };
const routes: Record<string, MessageKey> = { '/':'nav.inbox', '/issues':'nav.inbox', '/projects':'nav.projects', '/tasks':'team.title', '/inbox':'team.inbox', '/status':'monitor.title', '/integrations':'integration.title', '/instruments':'nav.instruments', '/releases':'nav.releases', '/setup':'nav.setup', '/push':'nav.push', '/settings':'nav.settings', '/transfers':'transfer.title', '/self-hosted':'lifecycle.title' };
function documentKey(location: Pick<Location,'pathname'|'search'>, projectId: string) {
  const params = new URLSearchParams(location.search);
  return `${projectId}:${location.pathname === '/' ? '/issues' : location.pathname.startsWith('/settings') ? '/settings' : location.pathname}:${location.pathname === '/tasks' ? params.get('id') ?? '' : ''}`;
}

export function Workbench({ children }: { children: ReactNode }) {
  const { t, locale } = useI18n(), shell = useShell();
  const location = useLocation(), context = useContext(UNSAFE_LocationContext), navigate = useNavigate();
  const projectId = shell.activeProject?.id ?? '';
  const documentProjectId=projectId || new URLSearchParams(location.search).get('project') || '';
  const key = documentKey(location, documentProjectId);
  const [documents, setDocuments] = useState<OpenDocument[]>([]);
  const [closing, setClosing] = useState<Location | null>(null);
  const [inspectionHost,setInspectionHost] = useState<HTMLDivElement|null>(null);
  const [bottomTab, setBottomTab] = useState('issues');
  const current = documents.find(item => item.key === key);
  // A route snapshot includes the outlet's match context and location. Hidden
  // Activities pause effects/listeners while retaining forms and scroll DOM.
  if (closing && closing !== location) setClosing(null);
  if (closing !== location && (!current || current.location !== location)) {
    const next = { key, projectId:documentProjectId, location, context, content: children, title: current?.title };
    setDocuments(current ? documents.map(item => item.key === key ? next : item) : [...documents, next]);
  }
  const resources = useScopedAsyncData(projectId, async signal => {
    if (!projectId) return null;
    const [issues, releases] = await Promise.all([api.listIssues({ projectId, limit: 20, sort: 'newest' }, signal), api.listReleases(projectId, { limit: 20 }, signal)]);
    return { issues, releases };
  });
  const issueId = location.pathname.match(/^\/issues\/([^/]+)$/)?.[1];
  const issue = useScopedAsyncData(`${projectId}:${issueId ?? ''}`, async signal => {
    if (!issueId || !projectId) return null;
    const result = await api.getIssue(issueId, signal);
    return result.projectId === projectId ? result : null;
  });
  const params = new URLSearchParams(location.search);
  const releaseId = location.pathname === '/releases' ? params.get('detail') : null;
  const taskId = location.pathname === '/tasks' ? params.get('id') : null;
  const release = useScopedAsyncData(`${projectId}:${releaseId ?? ''}`, signal => projectId && releaseId ? api.listReleases(projectId, {id:releaseId,limit:1}, signal).then(result=>result.releases[0]??null) : Promise.resolve(null));
  const task = useScopedAsyncData(`${projectId}:${taskId ?? ''}`, () => projectId && taskId ? api.getTask(projectId, taskId) : Promise.resolve(null));
  const reloadResources=resources.reload, reloadIssue=issue.reload, reloadRelease=release.reload, reloadTask=task.reload;
  useEffect(() => api.onChange(path => {
    if (path.startsWith('/admin/api/issues/') || path.startsWith(`/admin/api/projects/${projectId}/`) || path.startsWith('/admin/api/releases/')) {
      reloadResources(); reloadIssue(); reloadRelease(); reloadTask();
    }
  }), [projectId, reloadResources, reloadIssue, reloadRelease, reloadTask]);
  const updateInactive = useCallback((sourceKey: string, to: To, state: unknown) => setDocuments(previous => {
    const source=previous.find(item=>item.key===sourceKey);
    if (!source) return previous;
    const destination=typeof to==='string' ? new URL(to,window.location.origin+source.location.pathname+source.location.search) : to;
    const nextLocation: Location={...source.location,pathname:destination.pathname??source.location.pathname,search:destination.search??'',hash:destination.hash??'',state,key:`${source.location.key}-updated`};
    const nextKey=documentKey(nextLocation,source.projectId);
    const next={...source,key:nextKey,location:nextLocation,context:{...source.context,location:nextLocation},content:null,title:undefined};
    return previous.some(item=>item.key===nextKey&&item.key!==sourceKey) ? previous.filter(item=>item.key!==sourceKey) : previous.map(item=>item.key===sourceKey?next:item);
  }), [setDocuments]);
  const entries = documents;
  function title(item: OpenDocument) {
    const project = shell.projects.find(project => project.id === item.projectId);
    const path = item.location.pathname;
    const params = new URLSearchParams(item.location.search);
    const name = item.title ?? (path.startsWith('/issues/') ? path.split('/').at(-1)! : (path === '/tasks' ? params.get('id') : null) ?? t(routes[path] ?? 'nav.settings'));
    return `${name}${project && project.id !== projectId ? ` · ${project.name}` : ''}`;
  }
  const select = (item: OpenDocument) => navigate({ pathname: item.location.pathname, search: item.location.search, hash: item.location.hash });
  function closeMany(targets: string[]) {
    const remaining = documents.filter(item => !targets.includes(item.key));
    setClosing(location); setDocuments(remaining);
    const next = remaining.find(item => item.key === key) ?? remaining[Math.max(0, documents.findIndex(item => item.key === key)-1)] ?? remaining.at(-1);
    if (next && targets.includes(key)) select(next);
    requestAnimationFrame(() => next ? document.getElementById(`tab-${next.key}`)?.focus({preventScroll:true}) : document.getElementById('workspace')?.focus());
  }
  const href = (path: string, params: Record<string, string> = {}) => ({ pathname: path, search: new URLSearchParams({ project: projectId, ...params }).toString() });
  const summary = (count: number, total: number) => t('bench.recent', { count: String(count), total: String(total) });
  const issues = resources.data?.issues, releases = resources.data?.releases;
  const resourceState = resources.error ? <div className="workbench-message" role="alert"><p>{t('bench.loadFailed')}</p><p>{resources.error}</p><Button small onClick={resources.reload}>{t('common.retry')}</Button></div> : resources.loading ? <p role="status" className="workbench-message">{t('shell.loading')}</p> : !projectId ? <p className="workbench-message">{t('settings.selectProject')}</p> : null;
  const details = issue.data;
  const inspectedRelease = release.data;
  const inspection = issueId ? issue : releaseId ? release : taskId ? task : null;
  const values: [MessageKey, string][] = details ? [['bench.project', shell.activeProject?.name ?? '—'], ['bench.status', t(`inbox.group.${details.status}`)], ['bench.release', details.lastRelease || t('bench.unknown')], ['bench.environment', details.environment || t('bench.unknown')], ['bench.events', details.eventCount.toLocaleString(locale)]] : [['bench.project', shell.activeProject?.name ?? '—'], ['bench.platform', platformLabel(shell.activeProject?.platform,t)], ...(issues ? [['bench.issues', issues.total.toLocaleString(locale)] as [MessageKey,string]] : []), ...(releases ? [['bench.releases', releases.total.toLocaleString(locale)] as [MessageKey,string]] : [])];
  const currentTitle = details ? issueHeadline(details).headline : task.data?.task.title;
  if (current && currentTitle && current.title !== currentTitle) setDocuments(previous => previous.map(item => item.key === key ? { ...item, title: currentTitle } : item));
  if (inspectedRelease) values.splice(1, values.length, ['bench.release',inspectedRelease.name], ['bench.platform',(inspectedRelease.platforms??[]).map(platform=>platformLabel(platform,t)).join(', ') || t('bench.unknown')]);
  if (task.data) values.splice(1, values.length, ['team.stage',t(`team.${task.data.task.stage}`)], ['team.priority',t(`team.${task.data.task.priority}`)], ['bench.release',task.data.task.fixRelease || t('bench.unknown')]);
  return <WorkbenchLayout className="production-workbench" storageKey="sentori-workbench-layout" routeKey={`${location.pathname}${location.search}`}
    labels={{ title:t('bench.title'), explorer:t('bench.explorer'), inspector:t('bench.inspector'), activity:t('bench.activity'), toggleExplorer:t('bench.toggleExplorer'), toggleInspector:t('bench.toggleInspector'), toggleActivity:t('bench.toggleActivity'), reset:t('bench.reset'), close:t('common.close'), resize:name=>t('bench.resize',{name}) }}
    toolbar={<><Picker className="workbench-project-picker" multiple={false} searchable locale={locale} aria-label={t('bench.projectSwitcher')} placeholder={t('settings.selectProject')} value={projectId} options={shell.projects.map(project=>({value:project.id,label:project.name}))} onChange={id=>shell.setActiveProjectId(id)}/><span className="workbench-toolbar-title">{t('bench.title')}</span></>}
    tabs={<DocumentTabs entries={entries.map(item=>({ key:item.key,title:title(item) }))} active={key} select={target=>select(entries.find(item=>item.key===target)!)} close={target=>closeMany([target])} closeMany={closeMany}
      reorder={(target,index)=>setDocuments(previous=>{const item=previous.find(item=>item.key===target)!;const next=previous.filter(item=>item.key!==target);next.splice(index,0,item);return next;})}
      locale={locale} labels={{navigation:t('bench.tabs'),hint:t('bench.tabHint'),close:name=>t('bench.closeTab',{name}),moved:(name,position,total)=>t('bench.moved',{name,position:String(position),total:String(total)})}}/>}
    explorer={<div className="workbench-tree">{resourceState}{resources.data && <>
      <details open><summary><ChevronDown size={13}/><CircleDot size={14}/><span className="control-label-text">{t('bench.issues')}</span><span>{issues?.total.toLocaleString(locale)}</span></summary><p className="workbench-resource-count">{summary(issues!.issues.length,issues!.total)}</p>{issues!.issues.map(item=><Link key={item.id} to={href(`/issues/${item.id}`)} title={issueHeadline(item).headline} aria-current={issueId===item.id?'page':undefined}><span className={`workbench-kind kind-${item.kind}`}/><span className="control-label-text">{issueHeadline(item).headline}</span><small>{item.eventCount.toLocaleString(locale)}</small></Link>)}{!issues!.issues.length && <p className="workbench-message">{t('bench.empty')}</p>}</details>
      <details open><summary><ChevronDown size={13}/><Box size={14}/><span className="control-label-text">{t('bench.releases')}</span><span>{releases?.total.toLocaleString(locale)}</span></summary><p className="workbench-resource-count">{summary(releases!.releases.length,releases!.total)}</p>{releases!.releases.map(item=><Link key={item.id} to={href('/releases',{detail:item.id})} title={item.name}><Box size={13}/><span className="control-label-text">{item.name}</span></Link>)}{!releases!.releases.length && <p className="workbench-message">{t('bench.empty')}</p>}</details>
    </>}</div>}
    inspector={<><div ref={setInspectionHost} className="workbench-inspection-host"/><div className="workbench-inspector-body workbench-inspection-fallback"><span className="eyebrow">{t('bench.context')}</span><h2>{(details ? issueHeadline(details).headline : undefined) ?? inspectedRelease?.name ?? task.data?.task.title ?? shell.activeProject?.name ?? t('nav.projects')}</h2>{inspection?.error ? <div role="alert"><p>{inspection.error}</p><Button small onClick={inspection.reload}>{t('common.retry')}</Button></div> : inspection?.loading ? <p role="status">{t('shell.loading')}</p> : <dl>{values.map(([name,value])=><div key={name}><dt>{t(name)}</dt><dd>{value}</dd></div>)}</dl>}{!inspection && <p>{t('bench.selection')}</p>}</div></>}
    activity={closeActivity=><><div className="workbench-bottom-tabs" role="tablist" aria-label={t('bench.activity')}>{['issues','activity'].map((tab,index)=><button type="button" key={tab} role="tab" id={`live-bottom-${tab}`} aria-controls="live-bottom-content" aria-selected={bottomTab===tab} tabIndex={bottomTab===tab?0:-1} onClick={()=>setBottomTab(tab)} onKeyDown={event=>{if(['ArrowLeft','ArrowRight','Home','End'].includes(event.key)){event.preventDefault();const next=event.key==='Home'?'issues':event.key==='End'?'activity':index===0?'activity':'issues';setBottomTab(next);document.getElementById(`live-bottom-${next}`)?.focus();}}}>{t(tab==='issues'?'bench.issues':'bench.activity')}</button>)}<Button small variant="ghost" className="button-icon" aria-label={t('bench.toggleActivity')} onClick={closeActivity}><X size={14}/></Button></div>
      <div id="live-bottom-content" className="workbench-bottom-content" role="tabpanel" aria-labelledby={`live-bottom-${bottomTab}`}>{bottomTab==='issues'?<>{resourceState}{issues && <><p className="workbench-resource-count">{summary(issues.issues.length,issues.total)}</p><table><tbody>{issues.issues.map(item=><tr key={item.id}><td><StatusLabel marker={<span className={`workbench-kind kind-${item.kind}`}/>}>{item.kind}</StatusLabel></td><td><Link to={href(`/issues/${item.id}`)}>{issueHeadline(item).headline}</Link></td><td>{item.lastRelease}</td><td>{item.eventCount.toLocaleString(locale)}</td></tr>)}</tbody></table></>}</>:!issueId?<p className="workbench-message">{t('bench.selectIssue')}</p>:issue.error?<p className="workbench-message" role="alert">{issue.error}</p>:issue.loading?<p className="workbench-message" role="status">{t('shell.loading')}</p>:details?.activity.length?<table><tbody>{details.activity.map(item=><tr key={item.id}><td>{new Date(item.at).toLocaleString(locale)}</td><td>{item.actorEmail ?? t('detail.system')}</td><td>{t(`detail.activity.${item.kind}`)}</td><td>{item.kind==='note'?String(item.body.text??''):JSON.stringify(item.body)}</td></tr>)}</tbody></table>:<p className="workbench-message">{t('bench.empty')}</p>}</div></>}
  >{!entries.length && <WorkbenchEmpty title={t('bench.noDocuments')} description={t('bench.openDocument')} action={<Button onClick={()=>setClosing(null)}>{t('bench.reopenDocument')}</Button>}/>}
  {entries.map(item=><WorkbenchDocument key={item.key} item={item} active={item.key===key} locationContext={item.key===key?context:item.context} updateInactive={updateInactive} inspectionHost={inspectionHost}>
    <Suspense fallback={<p className="workbench-message" role="status">{t('shell.loading')}</p>}>{item.key===key?children:item.content}</Suspense>
  </WorkbenchDocument>)}</WorkbenchLayout>;
}

function WorkbenchDocument({ item, active, locationContext, children, updateInactive, inspectionHost }: { inspectionHost: HTMLElement|null; item: OpenDocument; active: boolean; locationContext: OpenDocument['context']; children: ReactNode; updateInactive: (key:string,to:To,state:unknown)=>void }) {
  const navigation = useContext(UNSAFE_NavigationContext), activeRef = useRef(active), locationRef = useRef(locationContext.location);
  useLayoutEffect(() => { activeRef.current = active; locationRef.current=locationContext.location; }, [active,locationContext]);
  const runtime=useMemo(()=>{const isActive=()=>activeRef.current && documentKey(window.location,new URLSearchParams(window.location.search).get('project')??item.projectId)===item.key;return {isActive,search:()=>isActive()?window.location.search:locationRef.current.search};},[item.key,item.projectId]);
  // A request started in a document may finish after its tab is hidden. It may
  // update that document's saved URL, but must not navigate the current one.
  const scopedNavigation = useMemo(() => ({ ...navigation, navigator: {
    ...navigation.navigator,
    push: (...args: Parameters<typeof navigation.navigator.push>) => { if (runtime.isActive()) navigation.navigator.push(...args); else updateInactive(item.key,args[0],args[1]); },
    replace: (...args: Parameters<typeof navigation.navigator.replace>) => { if (runtime.isActive()) navigation.navigator.replace(...args); else updateInactive(item.key,args[0],args[1]); },
    go: (delta: number) => { if (runtime.isActive()) navigation.navigator.go(delta); },
  } }), [navigation,runtime,updateInactive,item.key]);
  const path=item.location.pathname;
  // Credential screens deliberately discard their in-memory form/secret state
  // on departure. Investigation documents keep their drafts.
  const ephemeral=path==='/setup'||path==='/integrations'||path==='/push'||path.startsWith('/settings')&&path!=='/settings/notifications';
  return <div className="workbench-document" hidden={!active} data-active={active} role="tabpanel" id={`document-${item.key}`} aria-labelledby={`tab-${item.key}`}>
    {(active || !ephemeral) && <Activity mode={active?'visible':'hidden'}><DocumentRuntimeContext.Provider value={runtime}><UNSAFE_LocationContext.Provider value={locationContext}><UNSAFE_NavigationContext.Provider value={scopedNavigation}><DocumentProject projectId={item.projectId}><DocumentInspectionContext.Provider value={active?inspectionHost:null}>{children}</DocumentInspectionContext.Provider></DocumentProject></UNSAFE_NavigationContext.Provider></UNSAFE_LocationContext.Provider></DocumentRuntimeContext.Provider></Activity>}
  </div>;
}
