import { useDocumentSearchParams } from '../lib/search-params';
import { useId } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { RefreshCw } from 'lucide-react';
import { Heading, Surface } from '../../shared/design/Surface';
import { Metric } from '../../shared/design/Metric';
import { Button, Field, Input } from '../../shared/design/controls';
import { Tabs } from '../../shared/design/Tabs';
import { useShell } from '../App';
import { DataTable, ErrorBanner, Select, TimeAgo } from '../components/ui';
import { WorkbenchEmpty } from '../components/WorkbenchEmpty';
import { TelemetryChart } from '../components/TelemetryChart';
import { useT } from '../i18n';
import { api, type AssertRow, type LaunchRow, type ProbeRow, type TraceRow } from '../lib/api';
import { useScopedAsyncData } from '../lib/useScopedAsyncData';
import { useDebouncedValue } from '../lib/useDebouncedValue';

const tabs=['overview','launch','asserts','probes','traces'] as const;
const duration=(value:number|null)=>value===null?'—':value>=10000?`${(value/1000).toFixed(1)} s`:`${Math.round(value)} ms`;

export default function InstrumentsPage() {
  const {activeProject}=useShell(),t=useT();
  return activeProject?<Instruments key={activeProject.id} projectId={activeProject.id}/>:<div className="production-page"><Heading title={t('nav.instruments')}/><Surface><WorkbenchEmpty title={t('instruments.noProject')} action={<Link className="text-link" to="/projects">{t('nav.projects')}</Link>}/></Surface></div>;
}

function Instruments({projectId}:{projectId:string}) {
  const currentSearchParams=useDocumentSearchParams();
  const t=useT(),id=useId(),[params,setParams]=useSearchParams();
  const tab=tabs.find(tab=>tab===params.get('tab'))??'overview';
  const query=params.get('q')??'',release=params.get('rel')??params.get('release')??'';
  const requestedHours=Number(params.get('hours')??24),hours=[1,24,168,720].includes(requestedHours)?requestedHours:24;
  const settled=useDebouncedValue(query,200),pending=query!==settled;
  const state=useScopedAsyncData(JSON.stringify([projectId,settled,release,hours]),()=>api.instruments(projectId,{q:settled,release,hours}));
  const releases=useScopedAsyncData(projectId,()=>api.listReleases(projectId,{limit:500}));
  const data=pending?null:state.data;
  const activity=data?.activity??[],launch=data?.launch??[];
  const count=(key:'events'|'errors'|'warnings')=>data?activity.reduce((sum,row)=>sum+row[key],0):'—';
  const update=(changes:Record<string,string>)=>{const next=currentSearchParams();for(const [key,value]of Object.entries(changes)){if(value)next.set(key,value);else next.delete(key);}setParams(next,{replace:true});};
  const issueUrl=(name:string,kind:string)=>({pathname:'/issues',search:new URLSearchParams({project:projectId,q:name,kind,...release?{rel:release}:{}}).toString()});
  const title=(tab:typeof tabs[number])=>tab==='overview'?t('workbench.overview'):t(`instruments.${tab}`);
  const launchPlot=<TelemetryChart title={t('instruments.launch')} description={t('workbench.launchChartHint',{hours:String(hours)})} labels={launch.map(row=>row.release)} series={(['p50','p90','p95'] as const).map(key=>({key,label:key,values:launch.map(row=>row[key])}))} unit="ms" kind="bar"/>;
  const activityPlot=<TelemetryChart title={t('workbench.eventActivity')} description={t('workbench.receivedWindow',{hours:String(hours)})} time labels={activity.map(row=>row.at)} empty={count('events')===0} series={[
    {key:'errors',label:t('queue.kind')+' · error',values:activity.map(row=>row.errors),color:'--ui-danger'},
    {key:'warnings',label:t('queue.kind')+' · warn',values:activity.map(row=>row.warnings),color:'--ui-warning'},
    {key:'traces',label:t('instruments.traces'),values:activity.map(row=>row.traces),color:'--ui-info'},
    {key:'asserts',label:t('instruments.asserts'),values:activity.map(row=>row.assertions),color:'--ui-chart-3'},
    {key:'probes',label:t('instruments.probes'),values:activity.map(row=>row.probes),color:'--ui-success'},
  ]}/>;
  return <div className="production-page instruments-workbench">
    <Heading title={t('nav.instruments')} description={t('workbench.instrumentsDescription')} action={<Button disabled={state.loading||pending} onClick={state.reload}><RefreshCw size={15}/>{t('team.refresh')}</Button>}/>
    <div className="surface workbench-toolbar"><div className="control-bar">
      <Field label={t('workbench.searchInstruments')}><Input type="search" maxLength={200} value={query} onChange={e=>update({q:e.target.value})}/></Field>
      <Field label={t('inbox.releaseFilter')}><Input list={id+'-releases'} maxLength={200} value={release} placeholder={t('queue.all')} onChange={e=>update({rel:e.target.value,release:''})}/></Field><datalist id={id+'-releases'}>{releases.data?.releases.map(row=><option key={row.id} value={row.name}/>)}</datalist>
      <Field label={t('workbench.window')}><Select value={hours} onChange={e=>update({hours:e.target.value})}>{[1,24,168,720].map(value=><option key={value} value={value}>{t('workbench.lastHours',{hours:String(value)})}</option>)}</Select></Field>
      <Button onClick={()=>update({q:'',rel:'',release:'',hours:''})}>{t('projects.clearFilters')}</Button>
    </div>{releases.error&&<ErrorBanner reason={releases.error}>{t('releases.loadFailed')}</ErrorBanner>}</div>
    {state.error&&<ErrorBanner reason={state.error}>{t('instruments.loadFailed')}</ErrorBanner>}
    {(state.loading||pending)&&<p role="status" className="muted text-sm mb-4">{t('shell.loading')}</p>}
    <div className="metrics-grid mb-6" aria-busy={state.loading||pending}>
      <Metric label={t('queue.events')} value={count('events')} note={t('workbench.receivedWindow',{hours:String(hours)})}/>
      <Metric label={t('workbench.errorEvents')} value={count('errors')}/>
      <Metric label={t('workbench.warningEvents')} value={count('warnings')}/>
      <Metric label={t('queue.users')} value={data?(data.users||'—'):'—'} note={data&&data.users===0?t('queue.usersUnknown'):undefined}/>
    </div>
    <Tabs label={t('nav.instruments')} value={tab} onChange={value=>update({tab:value})} items={tabs.map(value=>[value,title(value)] as const)}>
      {data&&<div className="section-stack">
        {tab==='overview'&&<>{activityPlot}<div className="telemetry-grid">{launchPlot}<TelemetryChart title={t('instruments.traces')} description={t('workbench.topTraces')} labels={data.traces.slice(0,12).map(row=>row.name)} series={[{key:'events',label:t('queue.events'),values:data.traces.slice(0,12).map(row=>row.eventCount)}]} kind="bar"/></div></>}
        {tab==='launch'&&<>{launchPlot}<Surface title={t('instruments.launch')} body={false}><DataTable<LaunchRow> appearance="design" rows={launch} rowKey={row=>row.release} empty={t('instruments.launchEmpty')} columns={[
          {key:'release',label:t('instruments.colRelease')},
          {key:'samples',label:t('instruments.colSamples'),align:'right',render:row=>(row.samples-row.prewarmed).toLocaleString()},
          {key:'prewarmed',label:t('instruments.colPrewarmed'),align:'right',render:row=>row.prewarmed.toLocaleString()},
          ...(['p50','p90','p95'] as const).map(key=>({key,label:key,align:'right' as const,render:(row:LaunchRow)=>duration(row[key])})),
        ]}/><p className="workbench-note">{t('instruments.prewarmedTip')}</p></Surface></>}
        {tab==='asserts'&&<><TelemetryChart title={t('instruments.asserts')} description={t('workbench.cumulativeHint')} labels={data.asserts.slice(0,12).map(row=>row.name+' · '+row.release)} series={[{key:'pass',label:t('workbench.passed'),values:data.asserts.slice(0,12).map(row=>row.passCount),color:'--ui-success'},{key:'fail',label:t('workbench.failed'),values:data.asserts.slice(0,12).map(row=>row.failCount),color:'--ui-danger'}]} kind="bar"/>
          <Surface title={t('instruments.asserts')} body={false}><DataTable<AssertRow> appearance="design" rows={data.asserts} rowKey={row=>row.name+'\u0000'+row.release} empty={t('instruments.assertsEmpty')} columns={[
            {key:'name',label:t('instruments.colName'),render:row=><Link className="text-link" to={issueUrl(row.name,'assert')}>{row.name}</Link>},
            {key:'release',label:t('instruments.colRelease')},{key:'passCount',label:t('workbench.passed'),align:'right'},
            {key:'failCount',label:t('workbench.failed'),align:'right'},
            {key:'lastFailAt',label:t('instruments.colLastFail'),render:row=>row.lastFailAt?<TimeAgo iso={row.lastFailAt}/>:'—'},
          ]}/><p className="workbench-note">{t('workbench.cumulativeHint')}</p></Surface></>}
        {tab==='probes'&&<><TelemetryChart title={t('instruments.probes')} description={t('workbench.cumulativeHint')} labels={data.probes.slice(0,12).map(row=>row.ref)} series={[{key:'fired',label:t('workbench.probeFires'),values:data.probes.slice(0,12).map(row=>row.fireCount),color:'--ui-warning'}]} kind="bar"/>
          <Surface title={t('instruments.probes')} body={false}><DataTable<ProbeRow> appearance="design" rows={data.probes} rowKey={row=>row.ref} empty={t('instruments.probesEmpty')} columns={[
            {key:'ref',label:t('instruments.colName')},{key:'lastSeenRelease',label:t('instruments.colRelease')},
            {key:'fireCount',label:t('workbench.probeFires'),align:'right'},
            {key:'registeredAt',label:t('workbench.registered'),render:row=><TimeAgo iso={row.registeredAt}/>},
            {key:'lastFiredAt',label:t('instruments.colLastSeen'),render:row=>row.lastFiredAt?<TimeAgo iso={row.lastFiredAt}/>:'—'},
            {key:'issueId',label:t('instruments.guardedIssue'),render:row=>row.issueId?<Link className="text-link" to={`/issues/${row.issueId}?project=${projectId}`}>{t('instruments.guardedIssue')}</Link>:'—'},
          ]}/><p className="workbench-note">{t('workbench.cumulativeHint')}</p></Surface></>}
        {tab==='traces'&&<>{activityPlot}<Surface title={t('instruments.traces')} body={false}><DataTable<TraceRow> appearance="design" rows={data.traces} rowKey={row=>row.name} empty={t('instruments.tracesEmpty')} columns={[
          {key:'name',label:t('instruments.colName'),render:row=><Link className="text-link" to={issueUrl(row.name,'trace')}>{row.name}</Link>},
          {key:'eventCount',label:t('queue.events'),align:'right'},
          {key:'usersCount',label:t('queue.users'),align:'right',render:row=>row.usersCount||'—'},
          {key:'lastSeen',label:t('instruments.colLastSeen'),render:row=><TimeAgo iso={row.lastSeen}/>},
        ]}/><p className="workbench-note">{t('workbench.traceWindow',{hours:String(hours)})}</p></Surface></>}
      </div>}
    </Tabs>
  </div>;
}
