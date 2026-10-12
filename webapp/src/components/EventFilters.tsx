import { useDocumentSearchParams } from '../lib/search-params';
import { useId, useState } from 'react';
import { Plus, X } from 'lucide-react';
import { Button, Field, Input } from '../../shared/design/controls';
import { ErrorBanner, Select } from './ui';
import { useT } from '../i18n';
import { api } from '../lib/api';
import { appDisplayName } from '../lib/app-name';
import { platformLabel } from '../lib/platform-label';
import { readEventFilters, type EventFilters as Filters, type EventTag } from '../lib/event-filters';
import { useScopedAsyncData } from '../lib/useScopedAsyncData';

/** Facets are actual telemetry; free entry keeps older/high-cardinality values reachable. */
export function EventFilters({projectId, value, onChange}: {projectId:string; value:Filters; onChange:(change:Record<string,string>)=>void}) {
  const currentSearchParams=useDocumentSearchParams();
  const t = useT(), id = useId();
  const [source,setSource] = useState<EventTag['source']>('context'), [key,setKey] = useState(''), [tagValue,setTagValue] = useState('');
  const apps = useScopedAsyncData(projectId, () => api.listApps(projectId));
  const appScope = {appId:value.appId,platform:value.platform};
  const scope = JSON.stringify([projectId,appScope]);
  const facets = useScopedAsyncData(scope, async () => {
    const [environments,releases] = await Promise.all([api.projectEnvironments(projectId,appScope),api.listReleases(projectId,{appId:value.appId,limit:500})]);
    return {environments:environments.environments,releases:releases.releases.map(r=>r.name)};
  });
  const tagScope = {...appScope,environment:value.environment,release:value.release};
  const keys = useScopedAsyncData(JSON.stringify([projectId,tagScope,source]), () => api.projectContextKeys(projectId,tagScope,source));
  const values = useScopedAsyncData(JSON.stringify([projectId,tagScope,source,key]), () => key ? api.projectContextValues(projectId,key,tagScope,source) : Promise.resolve({values:[]}));
  const appTypes = useScopedAsyncData(JSON.stringify([projectId,tagScope]), () => api.projectContextValues(projectId,'appType',tagScope,'context'));
  const tags = value.tags ?? [];
  const appType = tags.find(tag=>tag.source==='context'&&tag.key==='appType')?.value ?? '';
  const sameTag = (a:EventTag,b:EventTag) => a.source===b.source&&a.key===b.key&&a.value===b.value;
  function updateTags(update:(current:EventTag[])=>EventTag[]) {
    let current=tags;
    try { current=readEventFilters(currentSearchParams()).tags??[]; } catch { /* Editing replaces an invalid URL filter. */ }
    const next=update(current);
    if(next.length>12)return;
    onChange({tags:next.length?JSON.stringify(next):'',ck:'',cv:'',dimension:'',context:''});
  }
  const availableApps = (apps.data?.apps ?? []).filter(app=>!value.platform || app.platform===value.platform);
  const failure = apps.error || facets.error || keys.error || values.error || appTypes.error;
  return <div className="event-filters">
    <div className="control-bar">
      <Field label={t('apps.title')}><Select value={value.appId??''} onChange={e=>onChange({app:e.target.value,rel:'',release:''})}>
        <option value="">{t('apps.all')}</option>
        {value.appId&&!availableApps.some(app=>app.id===value.appId)&&<option value={value.appId}>{value.appId}</option>}
        {availableApps.map(app=><option key={app.id} value={app.id}>{appDisplayName(app,t)} · {platformLabel(app.platform,t)}</option>)}
      </Select></Field>
      <Field label={t('filters.platform')}><Select value={value.platform??''} onChange={e=>onChange({platform:e.target.value,appType:'',app:'',rel:'',release:''})}>
        <option value="">{t('queue.all')}</option>{['ios','android','web','weapp','javascript','unknown'].map(platform=><option key={platform} value={platform}>{platformLabel(platform,t)}</option>)}
      </Select></Field>
      <Field label={t('inbox.envFilter')}><Input list={id+'-env'} value={value.environment??''} placeholder={t('queue.all')} maxLength={200} onChange={e=>onChange({env:e.target.value})}/></Field><datalist id={id+'-env'}>{facets.data?.environments.map(env=><option key={env} value={env}/>)}</datalist>
      <Field label={t('inbox.releaseFilter')}><Input list={id+'-releases'} value={value.release??''} placeholder={t('queue.all')} maxLength={200} onChange={e=>onChange({rel:e.target.value,release:''})}/></Field><datalist id={id+'-releases'}>{facets.data?.releases.map(release=><option key={release} value={release}/>)}</datalist>
    </div>
    <details className="advanced-filters"><summary>{t('filters.advanced')}{tags.length>0 && <span>{tags.length}</span>}</summary><div className="advanced-filters-content"><div className="control-bar"><Field label={t('filters.appType')}><Input list={id+'-appTypes'} value={appType} placeholder={t('queue.all')} maxLength={1000} disabled={!tags.some(tag=>tag.source==='context'&&tag.key==='appType')&&tags.length>=12} onChange={e=>{const value=e.target.value;updateTags(current=>{const next=current.filter(tag=>tag.source!=='context'||tag.key!=='appType');if(value)next.push({source:'context',key:'appType',value});return next;});}}/></Field><datalist id={id+'-appTypes'}>{appTypes.data?.values.map(type=><option key={type} value={type}/>)}</datalist></div>
    <form className="tag-filter-form" onSubmit={e=>{e.preventDefault();if(!key||tags.length>=12)return;const tag={source,key,value:tagValue};updateTags(current=>current.some(old=>sameTag(old,tag))?current:[...current,tag]);setKey('');setTagValue('');}}>
      <Field label={t('filters.source')}><Select value={source} onChange={e=>{setSource(e.target.value as EventTag['source']);setKey('');setTagValue('');}}><option value="context">{t('filters.context')}</option><option value="tags">{t('filters.tags')}</option></Select></Field>
      <Field label={t('filters.key')}><Input list={id+'-keys'} value={key} maxLength={200} placeholder="appType" onChange={e=>{setKey(e.target.value);setTagValue('');}}/></Field><datalist id={id+'-keys'}>{keys.data?.keys.map(key=><option key={key} value={key}/>)}</datalist>
      <Field label={t('filters.value')}><Input list={id+'-values'} value={tagValue} maxLength={1000} disabled={!key} onChange={e=>setTagValue(e.target.value)}/></Field><datalist id={id+'-values'}>{values.data?.values.map(value=><option key={value} value={value}/>)}</datalist>
      <Button type="submit" disabled={!key||tags.length>=12}><Plus size={15}/>{t('filters.add')}</Button>
    </form>
    <p className="field-note">{t('filters.sameEvent')}</p></div></details>
    {tags.length>0&&<div className="filter-chips" aria-label={t('filters.active')}>{tags.map((tag,index)=><Button small key={index} onClick={()=>updateTags(current=>current.filter(item=>!sameTag(item,tag)))} aria-label={t('filters.remove',{key:tag.key})}><span>{tag.source}.{tag.key} = {tag.value||t('filters.emptyValue')}</span><X size={14}/></Button>)}</div>}
    {failure&&<div className="flex flex-wrap gap-3"><ErrorBanner reason={failure}>{t('queue.optionsFailed')}</ErrorBanner><Button small onClick={()=>{apps.reload();facets.reload();keys.reload();values.reload();appTypes.reload();}}>{t('common.retry')}</Button></div>}
  </div>;
}
