import { useEffect, useId, useRef, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { Heading, Surface } from '../../shared/design/Surface';
import { Button, Field, Input, Textarea, Badge } from '../../shared/design/controls';
import { Checkbox } from '../../shared/design/Checkbox';
import { Dialog } from '../../shared/design/Dialog';
import { Metric } from '../../shared/design/Metric';
import { Tabs } from '../../shared/design/Tabs';
import { DataTable, ErrorBanner, Select, TimeAgo, formatBytes } from '../components/ui';
import { useShell } from '../App';
import { useT } from '../i18n';
import type { MessageKey } from '../i18n/en';
import { api, ApiError } from '../lib/api';
import { appDisplayLabel } from '../lib/app-name';
import { commandId } from '../lib/team-contract';
import { useDocumentSearchParams } from '../lib/search-params';
import { useScopedAsyncData } from '../lib/useScopedAsyncData';
import { useDebouncedValue } from '../lib/useDebouncedValue';
import { formatApiError } from '../lib/useAsyncData';
import type { WebCheck, WebCommand, WebConfiguration, WebPerformance, WebResult } from '../lib/web-contract';

const statuses:Record<string,MessageKey> = {queued:'web.queued',running:'web.running',passed:'web.passed',failed:'web.siteFailed',error:'web.monitorError',cancelled:'web.cancelled',healthy:'web.good',unknown:'web.unknown',degraded:'web.degraded',failing:'web.failing','monitor-error':'web.monitorError','insufficient-data':'web.insufficient','needs-attention':'web.attention',good:'web.good',observed:'web.observed'};
function StateLabel({value}:{value:string}) {const t=useT();return <Badge tone={value==='passed'||value==='good'||value==='healthy'?'success':value==='failed'||value==='failing'?'danger':'neutral'}>{statuses[value]?t(statuses[value]):value}</Badge>;}
function useCommand(project:string,saved:(id:string)=>void) {
  const pending=useRef<WebCommand|null>(null),lock=useRef(false);
  const [busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null);
  async function run(command:WebCommand) {
    if(lock.current)return;lock.current=true;setBusy(true);setError(null);pending.current??=command;
    try {const result=await api.webCommand(project,pending.current);pending.current=null;saved(result.id);}
    catch(failure){if(failure instanceof ApiError&&failure.status>=400&&failure.status<500)pending.current=null;setError(formatApiError(failure));}
    finally{lock.current=false;setBusy(false);}
  }
  return {busy,error,run};
}
export default function WebMonitoringPage() {
  const {activeProject,me}=useShell(),t=useT();
  return activeProject?<WebMonitoring key={activeProject.id} project={activeProject.id} owner={me.role==='superadmin'}/>:<div className="production-page"><Heading title={t('web.title')}/><Surface><Link className="text-link" to="/projects">{t('team.chooseProject')}</Link></Surface></div>;
}
function WebMonitoring({project,owner}:{project:string;owner:boolean}) {
  const t=useT(),[params,setParams]=useSearchParams(),current=useDocumentSearchParams();
  const apps=useScopedAsyncData(project,()=>api.listApps(project));
  const choices=apps.data?.apps.filter(a=>a.kind==='webapp'&&a.platform==='web')??[];
  const app=params.get('app')??choices[0]?.id??'',valid=choices.some(a=>a.id===app),tab=params.get('tab')??'performance';
  function filter(key:string,value:string){const next=current();if(value)next.set(key,value);else next.delete(key);if(key==='app')next.delete('run');setParams(next,{replace:true});}
  return <div className="production-page"><Heading title={t('web.title')} description={t('web.description')}/>
    {apps.error&&<ErrorBanner reason={apps.error}>{t('web.failed')}</ErrorBanner>}
    <Field label={t('web.chooseApp')}><Select value={app} onChange={e=>filter('app',e.target.value)}><option value="">{t('web.chooseApp')}</option>{choices.map(a=><option key={a.id} value={a.id}>{appDisplayLabel(a,t)}</option>)}</Select></Field>
    {!apps.loading&&!choices.length&&<Surface><p>{t('web.noApps')}</p><Link className="text-link" to={`/projects?project=${project}`}>{t('nav.projects')}</Link></Surface>}
    {valid&&<Tabs label={t('web.title')} items={[["performance",t('web.performance')],["errors",t('web.errors')],["checks",t('web.checks')]]} value={tab} onChange={value=>filter('tab',value)}>
      {tab==='checks'?<Checks key={app} project={project} app={app} owner={owner}/>:<Performance key={app} project={project} app={app} errors={tab==='errors'}/>}
    </Tabs>}
  </div>;
}
function Performance({project,app,errors}:{project:string;app:string;errors:boolean}) {
  const t=useT(),[params,setParams]=useSearchParams(),current=useDocumentSearchParams();
  const filters:Record<string,string>={app};for(const key of ['hours','environment','release','route','device']){const value=params.get(key);if(value)filters[key]=value;}
  const query=useDebouncedValue(JSON.stringify(filters),200);
  const data=useScopedAsyncData(`${project}:${query}`,signal=>api.webPerformance(project,JSON.parse(query),signal));
  function filter(key:string,value:string){const next=current();if(value)next.set(key,value);else next.delete(key);setParams(next,{replace:true});}
  return <div className="web-monitor-panels">
    <div className="control-bar">{(['environment','release','route'] as const).map(key=><Field key={key} label={t(`web.${key}`)}><Input value={params.get(key)??''} maxLength={key==='route'?240:key==='release'?200:100} onChange={e=>filter(key,e.target.value)}/></Field>)}
      <Field label={t('web.device')}><Select value={params.get('device')??''} onChange={e=>filter('device',e.target.value)}><option value="">{t('web.all')}</option><option value="mobile">{t('web.mobile')}</option><option value="desktop">{t('web.desktop')}</option></Select></Field>
      <Field label={t('web.hours')}><Input type="number" min={1} max={720} value={params.get('hours')??'24'} onChange={e=>filter('hours',e.target.value)}/></Field><Button onClick={data.reload}>{t('web.refresh')}</Button>
    </div>
    {data.error&&<ErrorBanner reason={data.error}>{t('web.failed')}</ErrorBanner>}
    {data.data&&<>{errors?<><p className="muted">{t('web.errorHint')}</p><DataTable rows={data.data.errors} rowKey={r=>r.issueId} empty={t('web.empty')} columns={[
      {key:'title',label:t('web.errors'),render:r=><Link className="text-link" to={`/issues/${r.issueId}?project=${project}&app=${app}`}>{r.title}</Link>},
      {key:'occurrences',label:t('web.occurrences'),render:r=>r.occurrences},{key:'visits',label:t('web.correlated'),render:r=>r.correlatedVisits},
      {key:'at',label:t('web.lastSeen'),render:r=><TimeAgo iso={r.lastSeen}/>},
    ]}/></>:<>
      <p className="muted">{t('web.rumHint')}</p><div className="metrics-grid"><Metric label={t('web.visits')} value={data.data.visits}/><Metric label={t('web.documents')} value={data.data.documentVisits}/><Metric label={t('web.errorRate')} value={data.data.estimatedErrorRate===null?'—':`${(data.data.estimatedErrorRate*100).toFixed(2)}%`}/></div>
      <p className="muted">{t('web.sampleRates')}: {data.data.sampleRates.map(v=>`${v*100}%`).join(', ')||'—'} · {t('web.retention',{days:String(data.data.retentionDays)})}</p>
      <DataTable rows={data.data.metrics} rowKey={r=>r.device+r.metric} empty={t('web.empty')} columns={[
        {key:'metric',label:t('web.metric'),render:r=>r.metric},{key:'device',label:t('web.device'),render:r=>t(r.device==='mobile'?'web.mobile':'web.desktop')},
        {key:'p75',label:'p75',render:r=>`${Number(r.p75.toFixed(3))} ${r.unit}`},{key:'samples',label:t('web.samples'),render:r=>`${r.samples} / ${r.minimumSamples}`},
        {key:'assessment',label:t('web.assessment'),render:r=><StateLabel value={r.assessment}/>},
      ]}/>
      <Surface title={t('web.coverage')}><div className="web-missing">{Object.entries(data.data.coverage).map(([metric,reasons])=><p key={metric}><strong>{metric}</strong> {Object.entries(reasons).map(([reason,n])=>`${reason}: ${n}`).join(' · ')}</p>)}</div></Surface>
      <Surface title={t('web.recent')}>{data.data.recent.map(row=><Visit key={row.id} row={row}/>)}</Surface>
    </>}</>}
  </div>;
}
function Visit({row}:{row:WebPerformance['recent'][number]}) {
  const t=useT();return <details className="team-monitor-history"><summary><TimeAgo iso={row.at}/> · {row.route} · {row.browser} · {row.navigationType}</summary>
    <p>{row.release} · {row.environment} · {row.device}</p><p>{Object.entries(row.metrics).map(([name,m])=>`${name}: ${Number(m.value.toFixed(3))} ${m.unit}`).join(' · ')||t('web.empty')}</p>
    <p>{t('web.diagnostics')}: {Object.entries(row.diagnostics.navigation??{}).map(([name,value])=>`${name}: ${value===null?'—':Number(value.toFixed(1))}`).join(' · ')}</p>
    <p>{Object.entries(row.diagnostics.longTasks).map(([name,value])=>`${name}: ${Number(value.toFixed(1))}`).join(' · ')}</p>
    {row.diagnostics.resources.map((resource,i)=><p className="break-all" key={i}><code>{resource.url}</code> · {resource.durationMs} ms · {resource.transferBytes===null?'—':formatBytes(resource.transferBytes)}</p>)}
  </details>;
}
function Checks({project,app,owner}:{project:string;app:string;owner:boolean}) {
  const t=useT(),[params,setParams]=useSearchParams(),current=useDocumentSearchParams();
  const list=useScopedAsyncData(`${project}:${app}`,signal=>api.webChecks(project,app,signal));
  const selected=params.get('run')??undefined;
  const runs=useScopedAsyncData(`${project}:${app}:${selected}`,signal=>api.webRuns(project,app,selected,signal));
  const [editor,setEditor]=useState<WebCheck|'new'|null>(null);
  const command=useCommand(project,id=>{const next=current();next.set('run',id);setParams(next,{replace:true});list.reload();runs.reload();});
  const pending=runs.data?.runs.some(r=>r.status==='queued'||r.status==='running');
  const reloadRuns=runs.reload,reloadChecks=list.reload;
  useEffect(()=>{if(!pending)return;const timer=setInterval(()=>{reloadRuns();reloadChecks();},5000);return()=>clearInterval(timer);},[pending,reloadRuns,reloadChecks]);
  return <div className="web-monitor-panels"><p className="muted">{t('web.labHint')}</p><div className="flex gap-2"><Button onClick={()=>{list.reload();runs.reload();}}>{t('web.refresh')}</Button>{owner&&<Button variant="primary" onClick={()=>setEditor('new')}>{t('web.create')}</Button>}</div>
    {list.data&&!list.data.runnerConfigured&&<p>{t('web.runnerMissing')}</p>}
    {[list.error,runs.error,command.error].filter(Boolean).map((error,i)=><ErrorBanner key={i} reason={error!}>{t('web.failed')}</ErrorBanner>)}
    {list.data?.checks.map(check=><Surface key={check.id} title={check.configuration.name} action={<div className="flex gap-2"><StateLabel value={check.health}/>{owner&&<><Button small onClick={()=>setEditor(check)}>{t('web.edit')}</Button><Button small busy={command.busy} disabled={!list.data?.runnerConfigured} onClick={()=>void command.run({commandId:commandId(),action:'run',id:check.id,trigger:'manual'})}>{t('web.run')}</Button></>}</div>}>
      <p className="break-all">{check.configuration.url}</p><p>{t(check.configuration.policy==='public'?'web.public':'web.private')} · {t(check.configuration.device==='mobile'?'web.mobile':'web.desktop')}</p>
      {check.latestRun&&<p><TimeAgo iso={check.latestRun.at}/> · <StateLabel value={check.latestRun.status}/></p>}
    </Surface>)}
    <Surface title={t('web.history')}>{runs.data?.runs.map(run=><details className="team-monitor-history" key={run.id} open={run.id===selected}><summary><TimeAgo iso={run.createdAt}/> · {run.configuration.name} · <StateLabel value={run.status}/></summary>{run.result&&<Result result={run.result}/>}</details>)}</Surface>
    {editor&&<Editor project={project} app={app} check={editor==='new'?null:editor} close={()=>setEditor(null)} saved={()=>{setEditor(null);list.reload();runs.reload();}}/>}
  </div>;
}
function Result({result}:{result:WebResult}) {
  const t=useT();return <div className="web-monitor-panels"><p>{result.errorCode}</p><p>{result.metrics?.map(m=>`${m.name}: ${Number(m.value.toFixed(3))} ${m.unit}`).join(' · ')}</p>
    <DataTable rows={result.findings??[]} rowKey={(r)=>r.rule+r.url+r.evidence} empty={t('web.noFindings')} columns={[
      {key:'severity',label:t('web.severity'),render:r=>r.severity},{key:'rule',label:t('web.rule'),render:r=><code>{r.rule}</code>},{key:'url',label:t('web.url'),render:r=><span className="break-all">{r.url}</span>},
      {key:'evidence',label:t('web.evidence'),render:r=>r.evidence},{key:'advice',label:t('web.advice'),render:r=>r.advice},
    ]}/><details><summary>{t('web.profile')}</summary>{[result.profile,result.versions,result.coverage].map((part,i)=><p key={i}>{Object.entries(part??{}).map(([key,value])=>`${key}: ${String(value)}`).join(' · ')}</p>)}</details>
  </div>;
}
function Editor({project,app,check,close,saved}:{project:string;app:string;check:WebCheck|null;close:()=>void;saved:()=>void}) {
  const t=useT(),form=useId();
  const [value,setValue]=useState<WebConfiguration>(check?.configuration??{appId:app,name:'',url:'',policy:'public',allowedOrigins:[],readySelector:null,authProfile:null,steps:[],intervalSeconds:0,environment:'production',enabled:false,lighthouse:true,device:'desktop',alerts:false,failureThreshold:2});
  const [origins,setOrigins]=useState(value.allowedOrigins.join('\n'));
  const [steps,setSteps]=useState(JSON.stringify(value.steps,null,2)),[localError,setLocalError]=useState<string|null>(null),[deleting,setDeleting]=useState(false);
  const command=useCommand(project,saved);
  const patch=(changes:Partial<WebConfiguration>)=>setValue(previous=>({...previous,...changes}));
  function save(){try {const configuration={...value,allowedOrigins:origins.split('\n').map(v=>v.trim()).filter(Boolean),steps:JSON.parse(steps)};setLocalError(null);void command.run(check?{commandId:commandId(),action:'update',id:check.id,expectedVersion:check.version,configuration}:{commandId:commandId(),action:'create',configuration});}catch(error){setLocalError(formatApiError(error));}}
  return <Dialog open title={t(check?'web.edit':'web.create')} onClose={close} closeLabel={t('web.cancel')} footer={<><Button disabled={command.busy} onClick={close}>{t('web.cancel')}</Button>{check&&<Button disabled={command.busy} onClick={()=>deleting?void command.run({commandId:commandId(),action:'delete',id:check.id,expectedVersion:check.version}):setDeleting(true)}>{t(deleting?'web.confirmDelete':'web.delete')}</Button>}<Button type="submit" form={form} variant="primary" busy={command.busy}>{t('web.save')}</Button></>}>
    <form id={form} onSubmit={e=>{e.preventDefault();save();}} className="web-monitor-panels">
      {(command.error||localError)&&<ErrorBanner reason={command.error||localError!}>{t('web.failed')}</ErrorBanner>}
      <Field label={t('web.name')}><Input required maxLength={100} value={value.name} onChange={e=>patch({name:e.target.value})}/></Field>
      <Field label={t('web.url')}><Input required type="url" value={value.url} onChange={e=>patch({url:e.target.value})}/></Field>
      <Field label={t('web.policy')}><Select value={value.policy} onChange={e=>patch({policy:e.target.value as 'public'|'private'})}><option value="public">{t('web.public')}</option><option value="private">{t('web.private')}</option></Select></Field>
      <Field label={t('web.device')}><Select value={value.device} onChange={e=>patch({device:e.target.value as 'mobile'|'desktop'})}><option value="desktop">{t('web.desktop')}</option><option value="mobile">{t('web.mobile')}</option></Select></Field>
      <Field label={t('web.environment')}><Input maxLength={100} value={value.environment} onChange={e=>patch({environment:e.target.value})}/></Field>
      <Field label={t('web.ready')}><Input maxLength={200} value={value.readySelector??''} onChange={e=>patch({readySelector:e.target.value||null})}/></Field>
      <Field label={t('web.auth')}><Input maxLength={64} value={value.authProfile??''} onChange={e=>patch({authProfile:e.target.value||null})}/></Field><p className="muted">{t('web.authHint')}</p>
      <Field label={t('web.origins')}><Textarea value={origins} onChange={e=>setOrigins(e.target.value)}/></Field>
      <Checkbox label={t('web.lighthouse')} checked={value.lighthouse} onChange={lighthouse=>patch({lighthouse})}/>
      {(['lcpMs','tbtMs','cls'] as const).map(key=><Field key={key} label={t(key==='lcpMs'?'web.budgetLcp':key==='tbtMs'?'web.budgetTbt':'web.budgetCls')}><Input type="number" min={0} max={key==='cls'?10:120000} step="any" value={value.budgets?.[key]??''} onChange={e=>patch({budgets:{...value.budgets,[key]:e.target.value===''?undefined:Number(e.target.value)}})}/></Field>)}
      <Checkbox label={t('web.schedule')} checked={value.enabled} onChange={enabled=>patch({enabled,intervalSeconds:enabled?Math.max(900,value.intervalSeconds):0})}/>
      {value.enabled&&<Field label={t('web.interval')}><Input type="number" min={900} max={604800} required value={value.intervalSeconds} onChange={e=>patch({intervalSeconds:Number(e.target.value)})}/></Field>}
      <Checkbox label={t('web.alerts')} checked={value.alerts} onChange={alerts=>patch({alerts})}/>
      {value.alerts&&<Field label={t('web.threshold')}><Input type="number" required min={2} max={10} value={value.failureThreshold} onChange={e=>patch({failureThreshold:Number(e.target.value)})}/></Field>}
      <details><summary>{t('web.advanced')}</summary><p>{t('web.stepsHint')}</p><Textarea aria-label={t('web.advanced')} value={steps} onChange={e=>setSteps(e.target.value)}/></details>
    </form>
  </Dialog>;
}
