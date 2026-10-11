import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Dialog } from '../../shared/design/Dialog';
import { useShell } from '../App';
import { KindBadge } from './kind';
import { ErrorBanner } from './ui';
import { useT } from '../i18n';
import { api, type IssueSummary } from '../lib/api';
import { useScopedAsyncData } from '../lib/useScopedAsyncData';
import { useDebouncedValue } from '../lib/useDebouncedValue';

type Item = {type:'nav';label:string;to:string} | {type:'issue';issue:IssueSummary};
export const openPalette=():void=>{window.dispatchEvent(new CustomEvent('sentori:palette'));};

export function CommandPalette() {
  const t=useT(),navigate=useNavigate(),{activeProject}=useShell(),id=useId();
  const [open,setOpen]=useState(false),[query,setQuery]=useState(''),[cursor,setCursor]=useState(0);
  const input=useRef<HTMLInputElement>(null),project=activeProject?.id;
  const settled=useDebouncedValue(query,180),pending=query!==settled;
  const search=useScopedAsyncData<{issues:IssueSummary[]}>(JSON.stringify([open,project,settled]),()=>open&&settled.trim()?api.listIssues({projectId:project,status:'all',q:settled,limit:8}):Promise.resolve({issues:[]}));
  function toggle(next:boolean|((open:boolean)=>boolean)) {setOpen(next);setQuery('');setCursor(0);}
  useEffect(()=>{
    const key=(event:KeyboardEvent)=>{if((event.metaKey||event.ctrlKey)&&event.key.toLowerCase()==='k'){event.preventDefault();toggle(old=>!old);}};
    const show=()=>toggle(true);
    window.addEventListener('keydown',key);window.addEventListener('sentori:palette',show);
    return()=>{window.removeEventListener('keydown',key);window.removeEventListener('sentori:palette',show);};
  },[]);
  const items=useMemo<Item[]>(()=>{
    const q=query.trim().toLocaleLowerCase();
    const nav:Item[]=([
      [t('nav.projects'),'/projects'],[t('nav.inbox'),'/issues'],[t('team.title'),'/tasks'],
      [t('monitor.title'),'/status'],[t('nav.instruments'),'/instruments'],[t('nav.releases'),'/releases'],
      [t('nav.setup'),'/setup'],[t('nav.push'),'/push'],[t('integration.title'),'/integrations'],
      [t('team.inbox'),'/inbox'],[t('nav.settings'),'/settings'],
    ] as const).filter(([label])=>!q||label.toLocaleLowerCase().includes(q)).map(([label,to])=>({type:'nav',label,to}));
    return [...nav,...(!pending&&q?search.data?.issues??[]:[]).map(issue=>({type:'issue' as const,issue}))];
  },[query,pending,search.data,t]);
  const selected=Math.max(0,Math.min(cursor,items.length-1));
  function go(item:Item) {
    toggle(false);
    navigate(item.type==='nav'?{pathname:item.to,search:project?new URLSearchParams({project}).toString():''}:{pathname:'/issues/'+item.issue.id,search:new URLSearchParams({project:item.issue.projectId}).toString()});
  }
  return <Dialog open={open} title={t('palette.placeholder')} closeLabel={t('common.close')} onClose={()=>toggle(false)}>
    <input ref={input} data-autofocus value={query} maxLength={200} onChange={e=>{setQuery(e.target.value);setCursor(0);}}
      role="combobox" aria-expanded aria-autocomplete="list" aria-label={t('palette.placeholder')} aria-controls={id} aria-activedescendant={items.length?id+'-'+selected:undefined}
      onKeyDown={event=>{
        if(event.key==='ArrowDown'){event.preventDefault();setCursor(Math.min(selected+1,Math.max(0,items.length-1)));}
        else if(event.key==='ArrowUp'){event.preventDefault();setCursor(Math.max(selected-1,0));}
        else if(event.key==='Enter'&&items[selected]){event.preventDefault();go(items[selected]);}
      }} placeholder={t('palette.placeholder')} className="control w-full"/>
    {search.error&&<ErrorBanner reason={search.error}>{t('inbox.loadFailed')}</ErrorBanner>}
    {(pending||search.loading)&&query.trim()&&<p role="status" className="muted text-sm mt-3">{t('shell.loading')}</p>}
    <div id={id} role="listbox" aria-label={t('palette.placeholder')} className="palette-results">
      {!items.length&&!pending&&!search.loading&&!search.error&&<p className="muted p-4">{t('table.empty')}</p>}
      {items.map((item,index)=><button id={id+'-'+index} key={item.type==='nav'?item.to:item.issue.id} type="button" role="option" tabIndex={-1} aria-selected={index===selected} onClick={()=>go(item)} onMouseEnter={()=>setCursor(index)} className="palette-result">
        {item.type==='nav'?item.label:<><KindBadge kind={item.issue.kind}/><span className="min-w-0"><strong>{item.issue.title}</strong>{item.issue.messageSample&&<span className="block text-xs subtle">{item.issue.messageSample}</span>}</span></>}
      </button>)}
    </div>
  </Dialog>;
}
