import { useDocumentSearchParams } from '../lib/search-params';
import { useEffect, useRef, useState } from 'react';
import { ReleaseJobs } from '../components/ReleaseJobs';
import { Link, useSearchParams } from 'react-router-dom';
import { Heading, Surface } from '../../shared/design/Surface';
import { StatusLabel } from '../../shared/design/StatusLabel';
import { Tabs } from '../../shared/design/Tabs';
import { Metric } from '../../shared/design/Metric';
import { Button, Badge, Field, Input } from '../../shared/design/controls';
import { FileInput } from '../../shared/design/FileInput';
import { useSharedT } from '../i18n/shared';
import { Dialog } from '../../shared/design/Dialog';
import { useShell } from '../App';
import { DataTable, ErrorBanner, Select, TimeAgo, formatBytes } from '../components/ui';
import { useT, useLocale } from '../i18n';
import { api, type ArtifactRow, type ReleaseRow } from '../lib/api';
import { formatApiError } from '../lib/useAsyncData';
import { useScopedAsyncData } from '../lib/useScopedAsyncData';
import { useDebouncedValue } from '../lib/useDebouncedValue';
import { platformLabel } from '../lib/platform-label';
import { appDisplayName } from '../lib/app-name';
import { formatCrashFree } from '../lib/crash-free';
import { lightColour, lightState, type LightState } from '../lib/release-lights';

type Row = ReleaseRow & {artifacts?:ArtifactRow[]|null};
export default function ReleasesPage(){
  const {activeProject}=useShell(),t=useT();
  return activeProject?<Releases key={activeProject.id} projectId={activeProject.id}/>:<div className="production-page"><Heading title={t('nav.releases')}/><Surface><p>{t('inbox.emptyNoProject')}</p><Link className="text-link" to="/projects">{t('nav.projects')}</Link></Surface></div>;
}
function Releases({projectId}:{projectId:string}){
  const currentSearchParams=useDocumentSearchParams();
  const listRef=useRef<HTMLDivElement>(null);
  const t=useT();const [params,setParams]=useSearchParams();
  const query=params.get('q')??'',selected=params.get('detail'),appId=params.get('app')??'';
  const apps=useScopedAsyncData(projectId,()=>api.listApps(projectId));
  const settledQuery=useDebouncedValue(query,150),pendingQuery=query!==settledQuery;
  const rawOffset=Number(params.get('offset')??0),offset=Number.isSafeInteger(rawOffset)&&rawOffset>=0?rawOffset:0;
  const state=useScopedAsyncData(JSON.stringify([projectId,appId,settledQuery,offset]),async signal=>{
    const result=await api.listReleases(projectId,{appId,q:settledQuery,offset,limit:50},signal);
    return {...result,rows:result.releases};
  });
  const health=useScopedAsyncData(JSON.stringify([projectId,appId]),()=>api.crashFree({projectId,appId,hours:720}));
  const byRelease=new Map<string,{sessions:number;crashed:number}>();
  for(const row of health.data?.releases??[]){const previous=byRelease.get(row.release)??{sessions:0,crashed:0};previous.sessions+=row.sessions??0;previous.crashed+=row.crashedSessions??0;byRelease.set(row.release,previous);}
  function update(changes:Record<string,string>){const next=currentSearchParams();for(const [key,value]of Object.entries(changes)){if(value)next.set(key,value);else next.delete(key);}setParams(next,{replace:true});}
  const listedRelease=state.data?.rows.find(row=>row.id===selected);
  const needsDetail=!!selected&&!listedRelease;
  const selectedState=useScopedAsyncData(JSON.stringify([projectId,selected,needsDetail]),()=>needsDetail?api.listReleases(projectId,{id:selected!,limit:1}).then(result=>result.releases[0]??null):Promise.resolve(null));
  const release=listedRelease??(needsDetail?selectedState.data:null);
  useEffect(()=>{
    const list=listRef.current;if(!selected||!list)return;
    const reveal=()=>{if(list.getClientRects().length)list.querySelector<HTMLElement>('[aria-current="true"]')?.scrollIntoView({block:'nearest'});};
    reveal();const observer=new ResizeObserver(reveal);observer.observe(list);return()=>observer.disconnect();
  },[selected,release?.id]);
  return <div className="production-page release-workspace" data-selected={!!selected}>
    <Heading title={t('nav.releases')} description={t('releaseView.description')}/>
    <div className="control-bar"><Field label={t('apps.title')}><Select value={appId} onChange={e=>update({app:e.target.value,offset:'',detail:''})}><option value="">{t('apps.all')}</option>{apps.data?.apps.map(app=><option key={app.id} value={app.id}>{appDisplayName(app,t)} · {platformLabel(app.platform,t)}</option>)}</Select></Field><Field label={t('releaseView.search')}><Input type="search" maxLength={200} value={query} onChange={e=>update({q:e.target.value,offset:'',detail:''})}/></Field></div>
    <div className="release-workspace-body">
    <div className="release-list" ref={listRef} aria-label={t('nav.releases')}>
    {!selected&&<Legend/>}
    {state.error&&<div><ErrorBanner reason={state.error}>{t('releases.loadFailed')}</ErrorBanner><Button onClick={state.reload}>{t('common.retry')}</Button></div>}
    {apps.error&&<ErrorBanner reason={apps.error}>{t('apps.loadFailed')}</ErrorBanner>}
    {health.error&&<ErrorBanner reason={health.error}>{t('releaseView.healthFailed')}</ErrorBanner>}
    <div className="surface" aria-busy={state.loading||pendingQuery}>{selected?<div className="release-item-list" onKeyDown={event=>{
      if(!['ArrowDown','ArrowUp','Home','End'].includes(event.key))return;
      const buttons=Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>('button'));
      const index=buttons.indexOf(document.activeElement as HTMLButtonElement);if(index<0)return;
      event.preventDefault();const next=buttons[event.key==='Home'?0:event.key==='End'?buttons.length-1:Math.max(0,Math.min(buttons.length-1,index+(event.key==='ArrowDown'?1:-1)))];next.focus();next.click();
    }}>
      {needsDetail&&release&&<><p className="workbench-message">{t('releaseView.outsideList')}</p><button type="button" aria-current="true" onClick={()=>update({detail:release.id})}><strong>{release.name}</strong><Coverage release={release}/></button></>}
      {(pendingQuery?[]:state.data?.rows??[]).map(row=><button type="button" key={row.id} aria-current={row.id===selected?'true':undefined} onClick={()=>update({detail:row.id})}><strong>{row.name}</strong><Coverage release={row}/><small><TimeAgo iso={(row.createdAt??row.created_at)!}/></small></button>)}
      {!state.loading&&!state.data?.rows.length&&<p className="workbench-message">{t('releases.emptyTitle')}</p>}
    </div>:<DataTable<Row> appearance="design" rowKey={row=>row.id} rows={pendingQuery?[]:state.data?.rows??[]} empty={t(state.loading||pendingQuery?'projects.loading':state.error?'releases.loadFailed':'releases.emptyTitle')} columns={[
      {key:'name',label:t('inbox.releaseFilter'),render:row=><button className="text-link font-semibold" onClick={()=>update({detail:row.id})}>{row.name}</button>},
      {key:'apps',label:t('apps.title'),render:row=>row.apps?.map(app=>`${appDisplayName(app,t)} · ${platformLabel(app.platform,t)}`).join(', ')||'—'},
      {key:'sessions',label:t('releaseView.sessions'),align:'right',render:row=>byRelease.get(row.name)?.sessions.toLocaleString()??'—'},
      {key:'crashFree',label:t('releases.crashFree'),align:'right',render:row=><CrashFreeCell health={byRelease.get(row.name)??null}/>},
      {key:'artifacts',label:t('releaseView.artifacts'),render:row=><Coverage release={row}/>},
      {key:'created',label:t('releaseView.created'),render:row=><TimeAgo iso={(row.createdAt??row.created_at)!}/>},
    ]}/>}<div className="table-pagination"><span>{state.data?t('queue.pageCount',{first:String(state.data.rows.length?offset+1:0),last:String(state.data.rows.length?offset+state.data.rows.length:0),total:String(state.data.total)}):'—'}</span><div className="flex gap-2"><Button small disabled={state.loading||pendingQuery||offset===0} onClick={()=>update({offset:String(Math.max(0,offset-50)),detail:''})}>{t('queue.previous')}</Button><Button small disabled={state.loading||pendingQuery||state.data?.nextOffset==null} onClick={()=>update({offset:String(state.data!.nextOffset),detail:''})}>{t('queue.next')}</Button></div></div></div>
    </div>
    {selected&&!release&&<Surface><p role="status">{t(state.loading||selectedState.loading?'shell.loading':'releaseView.notFound')}</p>{selectedState.error&&<><ErrorBanner reason={selectedState.error}>{t('releases.loadFailed')}</ErrorBanner><Button onClick={selectedState.reload}>{t('common.retry')}</Button></>}<Button onClick={()=>update({detail:''})}>{t('common.close')}</Button></Surface>}
    {release&&<ReleaseDetails key={release.id} projectId={projectId} release={release} health={byRelease.get(release.name)??null} onClose={()=>update({detail:''})} onChange={state.reload}/>}
    </div>
  </div>;
}
function Coverage({release}:{release:Row}){
  return <div className="release-coverage">{(['sourcemap','dsym','proguard','srcbundle'] as const).map((kind,index)=>{
    const artifacts=release.artifacts?.filter(row=>row.kind===kind);
    const summary=release.artifactSummary?.find(row=>row.kind===kind);
    // Unchecked historical artifacts are unknown, never green coverage.
    const on=artifacts!==undefined
      ?artifacts.some(row=>row.usable==null)?undefined:artifacts.some(row=>row.usable===true)
      :release.artifactSummary===undefined||summary?.unchecked?undefined:summary?.usable??false;
    return <Light key={kind} on={on} warn={artifacts?.some(row=>row.usable===false)??summary?.broken} label={['JavaScript','iOS','Android','Source'][index]} used={kind==='sourcemap'||kind==='dsym'&&!!release.platforms?.includes('ios')||kind==='proguard'&&!!release.platforms?.includes('android')}/>;
  })}</div>;
}
function ReleaseDetails({projectId,release,health,onClose,onChange}:{projectId:string;release:Row;health:{sessions:number;crashed:number}|null;onClose:()=>void;onChange:()=>void}){
  const t=useT(), sharedT=useSharedT(), locale=useLocale();const [fileEpoch,setFileEpoch]=useState(0);const [kind,setKind]=useState('sourcemap'),[file,setFile]=useState<File|null>(null);
  const [busy,setBusy]=useState(false),[failure,setFailure]=useState<string|null>(null),[receipt,setReceipt]=useState<boolean|null>(null);
  const [jobRevision,setJobRevision]=useState(0);
  const [section,setSection]=useState('files');
  const [deleting,setDeleting]=useState(false);const busyRef=useRef(false);
  const artifactState=useScopedAsyncData(JSON.stringify([projectId,release.id]),()=>api.listArtifacts(projectId,release.id));
  async function upload(event:React.FormEvent){event.preventDefault();if(!file||busyRef.current)return;busyRef.current=true;setBusy(true);setFailure(null);setReceipt(null);
    try{const result=await api.uploadArtifact(projectId,release.id,kind,file);setReceipt(result.usable);setFile(null);setFileEpoch(value=>value+1);setJobRevision(value=>value+1);artifactState.reload();onChange();}
    catch(error){setFailure(formatApiError(error));}finally{setBusy(false);busyRef.current=false;}
  }
  async function remove(){if(busyRef.current)return;busyRef.current=true;setBusy(true);setFailure(null);
    try{await api.deleteRelease(release.id);setDeleting(false);onClose();onChange();}catch(error){setFailure(formatApiError(error));}finally{setBusy(false);busyRef.current=false;}
  }
  return <>
    <section className="release-detail" aria-label={release.name}>
      <header className="release-detail-header"><div><span className="eyebrow">{t('releaseView.selected')}</span><h2>{release.name}</h2></div><Button small disabled={busy} onClick={onClose}>{t('releaseView.backToList')}</Button></header>
      <div className="release-detail-body">
      <div className="section-stack">
        <div className="metrics-grid"><Metric label={t('releaseView.sessions')} value={health?.sessions.toLocaleString()??'—'} note={t('releaseView.window')}/><Metric label={t('releases.crashFree')} value={<CrashFreeCell health={health}/>}/></div>
        <Coverage release={{...release,artifacts:artifactState.data?.artifacts}}/>
        <Tabs variant="line" label={t('releaseView.sections')} value={section} onChange={setSection} items={[["files",t('releaseView.artifacts')],["jobs",t('jobs.title')]]}>
        <div hidden={section!=='files'} className="section-stack">
        {artifactState.loading&&<p role="status">{t('shell.loading')}</p>}
        {artifactState.error&&<div><ErrorBanner reason={artifactState.error}>{t('releaseView.artifactFailed')}</ErrorBanner><Button onClick={artifactState.reload}>{t('common.retry')}</Button></div>}
        {artifactState.data?.artifacts.length===0&&<p className="muted">{t('releases.noArtifacts')}</p>}
        {artifactState.data?.artifacts.map(artifact=><Surface key={artifact.id}><div className="flex flex-wrap justify-between gap-3"><strong className="text-sm break-all">{artifact.name}</strong><Badge tone={artifact.usable===true?'success':artifact.usable===false?'danger':'neutral'}>{t(artifact.usable===true?'releaseView.usable':artifact.usable===false?'releaseView.unusable':'releaseView.unchecked')}</Badge></div><p className="muted text-xs mt-3">{artifact.kind} · {formatBytes(artifact.size_bytes??0)}</p><p className="subtle text-xs mt-2 break-all">{artifact.content_hash}</p></Surface>)}
        <Surface title={t('releaseView.upload')}><form className="form-stack" onSubmit={upload}>
          <Field label={t('queue.kind')}><Select disabled={busy} value={kind} onChange={e=>setKind(e.target.value)}>{['sourcemap','dsym','proguard','srcbundle'].map(value=><option key={value}>{value}</option>)}</Select></Field>
          <div><p className="field-label">{t('releaseView.file')}</p><p className="field-note">{t('releaseView.fileHint')}</p><FileInput key={fileEpoch} t={sharedT} number={(value,digits=0)=>value.toLocaleString(locale,{maximumFractionDigits:digits})} multiple={false} maxFiles={1} maxFileSize={255*1024*1024} preview={false} disabled={busy} onChange={files=>{setFile(files[0]??null);setReceipt(null);}}/></div>
          <Button type="submit" variant="primary" busy={busy} disabled={!file}>{t('releaseView.upload')}</Button>
          {failure&&<ErrorBanner reason={failure}>{t('releaseView.actionFailed')}</ErrorBanner>}
          {receipt!==null&&<p role="status" className={receipt?'text-ok':'text-danger'}>{t(receipt?'releaseView.uploaded':'releaseView.uploadedInvalid')}</p>}
        </form></Surface>
        </div><div hidden={section!=='jobs'}><ReleaseJobs projectId={projectId} releaseId={release.id} revision={jobRevision}/></div></Tabs>
      </div>
      </div>
      <footer className="release-detail-footer"><Button small variant="danger" disabled={busy} onClick={()=>{setFailure(null);setDeleting(true);}}>{t('releases.delete')}</Button></footer>
    </section>
    <Dialog open={deleting} title={t('releases.deleteConfirm',{name:release.name})} closeLabel={t('common.close')} onClose={()=>{if(!busy)setDeleting(false);}} footer={<Button variant="danger" busy={busy} onClick={remove}>{t('releases.delete')}</Button>}><p>{t('releaseView.deleteHint')}</p>{failure&&<ErrorBanner reason={failure}>{t('releases.deleteFailed')}</ErrorBanner>}</Dialog>
  </>;
}
function CrashFreeCell({ health }: { health: null | { sessions: number; crashed: number } }) {
  const t = useT();
  if (!health || health.sessions === 0) {
    return (
      <span
        className="w-20 text-right text-xs tabular-nums text-fg-subtle"
        title={t('releases.crashFreeNone')}
      >
        —
      </span>
    );
  }
  const pct = ((health.sessions - health.crashed) / health.sessions) * 100;
  const text = formatCrashFree(pct, health.sessions, health.crashed);
  return (
    <span
      className={`w-20 text-right text-xs tabular-nums ${
        health.crashed > 0 ? 'text-fg' : 'text-fg-muted'
      }`}
      title={t('releases.crashFreeTitle', { pct: text, sessions: String(health.sessions) })}
    >
      {text}%
    </span>
  );
}

/**
 * What the four dots mean, once, above the list.
 *
 * They are the whole screen and they were four unlabelled colours: a
 * reader had to hover each one to learn that amber is not a milder
 * green. A legend costs one line and removes the guessing.
 */
function Legend() {
  const t = useT();
  const items: [LightState, string][] = [
    ['ok', t('releases.legendOk')],
    ['broken', t('releases.legendBroken')],
    ['missing', t('releases.legendMissing')],
    ['unused', t('releases.legendUnused')],
    ['unknown', t('releaseView.unchecked')],
  ];
  return (
    <div className="release-legend">
      <StatusLabel className="muted">{t('releases.legend')}</StatusLabel>
      {items.map(([state, label]) => (
        <StatusLabel key={state} marker={<i style={{ backgroundColor: lightColour(state) }}/>}>{label}</StatusLabel>
      ))}
    </div>
  );
}

function Light({
  on,
  label,
  used = false,
  warn = false,
}: {
  on: boolean | undefined;
  label: string;
  used?: boolean;
  /** This kind holds at least one artifact the reader could not
   *  parse. Green would be a lie even when a good one sits beside
   *  it — somebody uploaded a file believing it did something. */
  warn?: boolean;
}) {
  const t = useT();
  // The decision lives in `lib/release-lights` so a gate can see it:
  // this dot is the only thing most people read on this page, and it
  // rendered green over a broken artifact for months.
  const state = lightState({ broken: warn, on, used });
  const colour = lightColour(state);
  return (
    <StatusLabel className={used ? 'release-light-used' : 'release-light-unused'} marker={<i style={{ backgroundColor: colour }}/>}
      title={
        on === undefined ? t('releaseView.unchecked') : warn
          ? t('releases.artifactBroken', { platform: label })
          : on === false && used
            ? t('releases.artifactMissing', { platform: label })
            : on === false
              ? t('releases.artifactUnused', { platform: label })
              : undefined
      }>{label}</StatusLabel>
  );
}
