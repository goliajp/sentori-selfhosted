import { Action } from '../../shared/design/controls';
import { Checkbox, CheckboxInput } from "../../shared/design/Checkbox";
import { useDocumentSearchParams } from '../lib/search-params';
import { ArrowLeft, Keyboard } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { Link, useNavigate, useParams, useSearchParams } from 'react-router-dom';
import { Heading, Surface } from '../../shared/design/Surface';
import { Metric } from '../../shared/design/Metric';
import { Button, Badge, Field, Input } from '../../shared/design/controls';
import { Tabs } from '../../shared/design/Tabs';
import { useShell } from '../App';
import { CrashFreeCard } from '../components/CrashFreeCard';
import { KindBadge } from '../components/kind';
import { DataTable, ErrorBanner, Kbd, Select, TimeAgo } from '../components/ui';
import { useT, useLocale } from '../i18n';
import { api, type IssueSummary } from '../lib/api';
import { issueHeadline } from '../lib/issue-title';
import { formatApiError } from '../lib/useAsyncData';
import { useScopedAsyncData } from '../lib/useScopedAsyncData';
import { EventFilters } from '../components/EventFilters';
import { readEventFilters, type EventFilters as Filters } from '../lib/event-filters';
import { IssueDetailPane } from './IssueDetail';

const PAGE_SIZE = 50;
const KINDS = ['error', 'warn', 'assert', 'trace', 'probe'] as const;
export default function TriageView() {
  const { activeProject } = useShell();
  const { issueId } = useParams();
  const [params] = useSearchParams();
  const t = useT();
  const listParams=new URLSearchParams(params);
  for(const key of ['occurrence','occurrenceOffset','tab'])listParams.delete(key);
  if (issueId) return <div className="production-page issue-page">
    <Link className="text-link inline-flex items-center gap-2 mb-6" to={{pathname:'/issues',search:listParams.toString()}}><ArrowLeft size={16}/>{t('triage.back')}</Link>
    <IssueDetailPane key={issueId} issueId={issueId}/>
  </div>;
  if (!activeProject) return <div className="production-page"><Heading title={t('queue.title')}/><Surface><p>{t('inbox.emptyNoProject')}</p><Link className="text-link" to="/projects">{t('nav.projects')}</Link></Surface></div>;
  return <IssueQueue key={activeProject.id} projectId={activeProject.id}/>;
}

function IssueQueue({projectId}: {projectId: string}) {
  const currentSearchParams=useDocumentSearchParams();
  const t = useT(), locale = useLocale(), navigate = useNavigate();
  const [params, setParams] = useSearchParams();
  const q = params.get('q') ?? '', status = params.get('status') ?? 'all';
  const group = params.get('group') ?? 'all', kind = params.get('kind') ?? '';
  let eventFilters: Filters = {}, filterError = false;
  try { eventFilters = readEventFilters(params); } catch { filterError = true; }
  const sort = params.get('sort') ?? 'newest';
  const rawOffset = Number(params.get('offset') ?? 0);
  const offset = Number.isSafeInteger(rawOffset) && rawOffset >= 0 ? rawOffset : 0;
  const scope = JSON.stringify([projectId,q,status,group,kind,eventFilters,filterError,sort,offset]);
  const {data,error,loading,reload} = useScopedAsyncData(scope, () => filterError ? Promise.reject(new Error(t('filters.invalid'))) : api.listIssues({projectId,q,status,group,kind:kind || undefined,...eventFilters,sort,offset,limit:PAGE_SIZE}));
  const health = useScopedAsyncData(projectId, () => api.projectHealth(projectId));
  const [selection,setSelection] = useState<{scope:string;ids:string[]}>({scope,ids:[]});
  const [busy,setBusy] = useState(false), [failure,setFailure] = useState<string|null>(null);
  const actionBusy = useRef(false);
  const rows = data?.issues ?? [], checked = selection.scope === scope ? selection.ids.filter(id=>rows.some(row=>row.id===id)) : [];
  function filter(changes: Record<string,string>, page = false) {
    const next = currentSearchParams();
    for (const [key,value] of Object.entries(changes)) { if (value) next.set(key,value); else next.delete(key); }
    if (!page) next.delete('offset');
    setParams(next,{replace:!page});
  }
  function sortable(ascending: string, descending: string) {
    return {
      onSort: () => filter({sort: sort === descending ? ascending : descending}),
      sortDirection: sort === ascending ? 'ascending' as const : sort === descending ? 'descending' as const : undefined,
    };
  }
  function toggle(id:string) { setSelection({scope,ids:checked.includes(id)?checked.filter(value=>value!==id):[...checked,id]}); }
  function open(id:string) { const next=currentSearchParams();for(const key of ['occurrence','occurrenceOffset','tab'])next.delete(key);navigate({pathname:'/issues/'+id,search:next.toString()}); }
  async function batch(action:'resolved'|'ignored', ids=checked) {
    if (!ids.length || actionBusy.current) return;
    actionBusy.current=true; setBusy(true); setFailure(null);
    const settled = await Promise.allSettled(ids.map(id=>action==='resolved'?api.resolveIssue(id):api.ignoreIssue(id)));
    const failed = ids.filter((_,i)=>settled[i].status==='rejected');
    const reasons = settled.flatMap(result=>result.status==='rejected'?[formatApiError(result.reason)]:[]);
    setSelection({scope,ids:failed});
    if (failed.length) setFailure(t('queue.partialFailure',{failed:String(failed.length),total:String(ids.length)})+' '+[...new Set(reasons)].join('; '));
    reload(); setBusy(false); actionBusy.current=false;
  }
  const keyboard = useRef({checked,toggle,batch});
  useEffect(()=>{keyboard.current={checked,toggle,batch};});
  useEffect(()=>{
    function handle(event:KeyboardEvent) {
      if(event.ctrlKey||event.metaKey||event.altKey||(event.target as HTMLElement).closest('input,textarea,select,[contenteditable="true"],dialog[open]'))return;
      const buttons=Array.from(document.querySelectorAll<HTMLButtonElement>('.workbench-document[data-active="true"] [data-issue-id]'));
      if(!buttons.length)return;
      const index=buttons.indexOf(document.activeElement as HTMLButtonElement), key=event.key.toLowerCase();
      if(key==='j'||key==='k') {event.preventDefault(); const next=index<0?(key==='j'?0:buttons.length-1):Math.max(0,Math.min(buttons.length-1,index+(key==='j'?1:-1)));buttons[next].focus();buttons[next].scrollIntoView({block:'nearest'});}
      else if(['x','r','i'].includes(key)) {event.preventDefault();const id=buttons[Math.max(0,index)].dataset.issueId!; if(key==='x')keyboard.current.toggle(id);else void keyboard.current.batch(key==='r'?'resolved':'ignored',keyboard.current.checked.length?keyboard.current.checked:[id]);}
    }
    addEventListener('keydown',handle);return()=>removeEventListener('keydown',handle);
  },[]);
  const number=(n:number|undefined)=>n===undefined?'—':n.toLocaleString(locale);
  function clear(){filter(Object.fromEntries(['q','status','group','kind','env','rel','release','ck','cv','dimension','context','sort','app','appType','platform','tags'].map(key=>[key,''])));}
  return <div className="production-page">
    <Heading title={t('queue.title')} description={t('queue.description')}/>
    <div className="metrics-grid mb-6" aria-busy={loading}>
      <Metric label={t('queue.events')} value={number(data?.summary.events)} note={t('queue.filteredTotals')}/>
      <Metric label={t('inbox.group.open')} value={number(data?.summary.open)}/>
      <Metric label={t('inbox.groupRegressed')} value={number(data?.summary.regressed)}/>
      <Metric label={t('queue.latestReleases')} value={number(data?.summary.releases)}/>
    </div>
    <Tabs label={t('queue.title')} value={group} onChange={value=>filter({group:value})} items={[["all",t('queue.all')],["open",t('inbox.group.open')],["regressed",t('inbox.groupRegressed')],["fresh",t('inbox.groupNew')]]}>
      <div className="surface result-filters">
        <div className="control-bar queue-filters-primary">
          <Field label={t('queue.search')}><Input type="search" maxLength={200} value={q} onChange={event=>filter({q:event.target.value})}/></Field>
          <Field label={t('queue.status')}><Select value={status} onChange={event=>filter({status:event.target.value})}><option value="all">{t('queue.all')}</option>{(['open','resolved','ignored'] as const).map(value=><option key={value} value={value}>{t(`inbox.group.${value}`)}</option>)}</Select></Field>
          <Field label={t('queue.kind')}><Select value={kind} onChange={event=>filter({kind:event.target.value})}><option value="">{t('queue.all')}</option>{KINDS.map(value=><option key={value} value={value}>{value}</option>)}</Select></Field>
          <Field label={t('queue.sort')}><Select value={sort} onChange={event=>filter({sort:event.target.value})}><option value="impact">{t('queue.impact')}</option><option value="newest">{t('queue.newest')}</option><option value="oldest">{t('queue.oldest')}</option>{(['title','kind','status','events','users'] as const).flatMap(column => (['asc','desc'] as const).map(direction => <option key={`${column}_${direction}`} value={`${column}_${direction}`}>{t(column === 'title' ? 'queue.issue' : `queue.${column}`)} · {t(direction === 'asc' ? 'queue.ascending' : 'queue.descending')}</option>))}</Select></Field>
          <Button small onClick={clear}>{t('projects.clearFilters')}</Button>
        </div>
        <EventFilters projectId={projectId} value={eventFilters} onChange={filter}/>

      </div>
      {failure&&<ErrorBanner reason={failure}>{t('queue.actionFailed')}</ErrorBanner>}
      {error&&<div><ErrorBanner reason={error}>{t('inbox.loadFailed')}</ErrorBanner><Button onClick={reload}>{t('common.retry')}</Button></div>}
      <div className="surface" aria-busy={loading}>
        <div className="control-bar selection-toolbar">
          <Checkbox aria-label={t('queue.selectPage')} checked={checked.length&&checked.length<rows.length?'mixed':!!rows.length&&checked.length===rows.length} disabled={busy||loading||!rows.length} onChange={()=>setSelection({scope,ids:checked.length===rows.length?[]:rows.map(row=>row.id)})} label={<><span aria-live="polite">{t('inbox.selectedCount',{n:String(checked.length)})}</span></>}/>
          <Button small disabled={busy||loading||!checked.length} onClick={()=>batch('resolved')}>{t('issue.resolve')}</Button>
          <Button small disabled={busy||loading||!checked.length} onClick={()=>batch('ignored')}>{t('issue.ignore')}</Button>
          <IssueShortcuts/>
        </div>
        <DataTable<IssueSummary> appearance="design" rowKey={row=>row.id} rows={rows} empty={loading?t('projects.loading'):error?t('inbox.loadFailed'):<div className="queue-empty">
          <p className="font-medium text-fg">{t('queue.noMatches')}</p>
          <p>{t(health.data?.lastEventAt===null?'inbox.awaitingFirstHint':'queue.noMatchesHint')}</p>
          {health.data?.lastEventAt===null?<Link className="text-link" to={'/setup?project='+projectId}>{t('setup.title')}</Link>:<Button onClick={clear}>{t('projects.clearFilters')}</Button>}
        </div>} columns={[
          {key:'select',label:t('queue.select'),render:row=><CheckboxInput aria-label={t('queue.selectIssue',{title:issueHeadline(row).headline})} checked={checked.includes(row.id)} disabled={busy||loading} onChange={()=>toggle(row.id)}/>},
          {key:'title',...sortable('title_asc','title_desc'),label:t('queue.issue'),render:row=><Action className="text-left issue-list-title" data-issue-id={row.id} onClick={()=>open(row.id)}><span className="text-action font-semibold">{issueHeadline(row).headline}</span><span className="block subtle text-xs mt-1">{row.id}</span></Action>},
          {key:'kind',...sortable('kind_asc','kind_desc'),label:t('queue.kind'),render:row=><KindBadge kind={row.kind}/>},
          {key:'status',...sortable('status_asc','status_desc'),label:t('queue.status'),render:row=><div className="flex flex-wrap gap-1"><Badge tone={row.status==='resolved'?'success':'neutral'}>{t(`inbox.group.${row.status}`)}</Badge>{row.status==='open'&&row.regressedAt&&<Badge tone="danger">{t('inbox.groupRegressed')}</Badge>}</div>},
          {key:'eventCount',...sortable('events_asc','events_desc'),label:t('queue.events'),align:'right',render:row=>number(row.eventCount)},
          {key:'usersCount',...sortable('users_asc','users_desc'),label:t('queue.users'),align:'right',render:row=>row.usersCount?number(row.usersCount):<span title={t('queue.usersUnknown')}>—</span>},
          {key:'lastSeen',...sortable('oldest','newest'),label:t('queue.lastSeen'),render:row=><TimeAgo iso={row.lastSeen}/>},
        ]}/>
        <div className="table-pagination"><span aria-live="polite">{data?t('queue.pageCount',{first:String(rows.length?offset+1:0),last:String(rows.length?offset+rows.length:0),total:number(data.total)}):'—'}</span><div className="flex gap-2"><Button small disabled={loading||offset===0} onClick={()=>filter({offset:String(Math.max(0,offset-PAGE_SIZE))},true)}>{t('queue.previous')}</Button><Button small disabled={loading||data?.nextOffset==null} onClick={()=>filter({offset:String(data!.nextOffset)},true)}>{t('queue.next')}</Button></div></div>
      </div>
    </Tabs>
    <div className="section-space"><CrashFreeCard projectId={projectId} appId={eventFilters.appId} environment={eventFilters.environment}/></div>
  </div>;
}

function IssueShortcuts() {
  const t = useT();
  const details = useRef<HTMLDetailsElement>(null);
  useEffect(() => {
    const close = (event: PointerEvent) => {
      if (details.current && !details.current.contains(event.target as Node)) details.current.open = false;
    };
    addEventListener('pointerdown', close);
    return () => removeEventListener('pointerdown', close);
  }, []);
  const shortcuts = [
    {keys: ['J', 'K'], label: t('queue.shortcutMove')},
    {keys: ['Enter'], label: t('queue.shortcutOpen')},
    {keys: ['X'], label: t('queue.shortcutSelect')},
    {keys: ['R'], label: t('issue.resolve')},
    {keys: ['I'], label: t('issue.ignore')},
  ];
  return <details ref={details} className="table-shortcuts" onKeyDown={event => {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.currentTarget.open = false;
      event.currentTarget.querySelector('summary')?.focus();
    }
  }}>
    <summary className="button button-ghost button-small" aria-label={t('queue.shortcuts')}><Keyboard size={16}/>{t('queue.shortcutButton')}</summary>
    <div className="table-shortcuts-panel">
      <p className="table-shortcuts-title">{t('queue.shortcuts')}</p>
      <dl className="table-shortcuts-list">{shortcuts.map(shortcut => <div key={shortcut.keys[0]}>
        <dt>{shortcut.keys.map(key => <Kbd key={key}>{key}</Kbd>)}</dt><dd>{shortcut.label}</dd>
      </div>)}</dl>
    </div>
  </details>;
}
