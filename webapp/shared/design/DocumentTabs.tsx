import { useLayoutEffect, useRef, useState } from 'react';
import { TabMenu } from './TabMenu';
import { ChevronLeft, ChevronRight, CircleDot, X } from 'lucide-react';
const scrollLabels = { en: { before: 'Scroll left · {n} more tabs', after: 'Scroll right · {n} more tabs' }, zh: { before: '向左滚动，前面还有 {n} 页', after: '向右滚动，后面还有 {n} 页' }, ja: { before: '左へスクロール・前に {n} タブ', after: '右へスクロール・後ろに {n} タブ' } };
type Entry = { key: string; title: string };
export function DocumentTabs({ entries, active, select, close, closeMany, reorder, labels, locale }: { locale: 'en'|'zh'|'ja'; closeMany: (keys:string[])=>void; entries: Entry[]; active: string; select: (key:string)=>void; close:(key:string)=>void; reorder:(key:string,index:number)=>void; labels: { navigation: string; hint: string; close: (name: string) => string; moved: (name: string, position: number, total: number) => string } }) {
  const [menu,setMenu]=useState<{key:string;x:number;y:number}|null>(null);
  function dismissMenu(restoreFocus=true) { const key=menu?.key;setMenu(null);if(restoreFocus)requestAnimationFrame(()=>{if(key)document.getElementById(`tab-${key}`)?.focus({preventScroll:true});}); }
  const [announcement, announce] = useState('');
  const root=useRef<HTMLDivElement>(null),gesture=useRef<{key:string;start:number;moving:boolean;index:number}|null>(null),suppress=useRef(false);
  const entryLayout=entries.map(entry=>`${entry.key}:${entry.title}`).join('\0');
  useLayoutEffect(()=>{document.getElementById(`tab-${active}`)?.closest('.workbench-tab')?.scrollIntoView({block:'nearest',inline:'nearest'});},[active,entryLayout]);
  const [overflow,setOverflow] = useState({before:0,after:0});
  useLayoutEffect(()=>{
    const bar=root.current!;
    const measure=()=>{
      const rect=bar.getBoundingClientRect();
      const tabs=Array.from(bar.querySelectorAll<HTMLElement>('.workbench-tab'));
      const next={before:tabs.filter(tab=>tab.getBoundingClientRect().left<rect.left-1).length,after:tabs.filter(tab=>tab.getBoundingClientRect().right>rect.right+1).length};
      setOverflow(previous=>previous.before===next.before&&previous.after===next.after?previous:next);
    };
    const observer=new ResizeObserver(measure); observer.observe(bar);
    bar.querySelectorAll('.workbench-tab').forEach(tab=>observer.observe(tab));
    bar.addEventListener('scroll',measure,{passive:true}); measure();
    return()=>{observer.disconnect();bar.removeEventListener('scroll',measure);};
  },[entries]);
  function scroll(direction:-1|1) {
    const bar=root.current!;
    const rect=bar.getBoundingClientRect();
    const tabs=Array.from(bar.querySelectorAll<HTMLElement>('.workbench-tab'));
    const target=direction<0?tabs.filter(tab=>tab.getBoundingClientRect().left<rect.left-1).at(-1):tabs.find(tab=>tab.getBoundingClientRect().right>rect.right+1);
    if(target)bar.scrollBy({left:direction<0?target.getBoundingClientRect().left-rect.left:target.getBoundingClientRect().right-rect.right,behavior:matchMedia('(prefers-reduced-motion: reduce)').matches?'instant':'smooth'});
  }
  function scrollButton(direction:-1|1) {
    const count=direction<0?overflow.before:overflow.after;
    const label=scrollLabels[locale][direction<0?'before':'after'].replace('{n}',String(count));
    return <button type="button" className="workbench-tab-scroll" data-direction={direction<0?'before':'after'} disabled={!count} onClick={()=>scroll(direction)} aria-label={label} title={label}>{direction<0&&<ChevronLeft size={14}/>}<span aria-hidden>{count}</span>{direction>0&&<ChevronRight size={14}/>}</button>;
  }
  const [drop,setDrop]=useState<{key:string;index:number}|null>(null);
  function move(key:string,index:number) {const old=entries.findIndex(e=>e.key===key);if(index===old)return;reorder(key,index);announce(labels.moved(entries[old].title,index+1,entries.length));requestAnimationFrame(()=>document.getElementById(`tab-${key}`)?.focus());}
  function cancel(){if(gesture.current?.moving)suppress.current=true;gesture.current=null;setDrop(null);}
  const rest=drop?entries.filter(e=>e.key!==drop.key):[];
  return <><span className="sr-only" role="status" aria-live="polite">{announcement}</span><div className="workbench-tab-navigation">{scrollButton(-1)}<div ref={root} className="workbench-tabs" role="tablist" aria-label={labels.navigation} title={labels.hint} onKeyDown={e=>{if(e.key==='Escape'&&gesture.current){e.preventDefault();cancel();}}}>{entries.map(item=><div className="workbench-tab" key={item.key} onContextMenu={event=>{event.preventDefault();cancel();setMenu({key:item.key,x:event.clientX,y:event.clientY});}} onKeyDown={event=>{if(event.key==='ContextMenu'||event.shiftKey&&event.key==='F10'){event.preventDefault();const rect=event.currentTarget.getBoundingClientRect();setMenu({key:item.key,x:rect.left,y:rect.bottom});}}} onAuxClick={event=>{if(event.button===1){event.preventDefault();close(item.key);}}} data-key={item.key} data-active={item.key===active} data-dragging={drop?.key===item.key||undefined} data-drop={drop&&item.key!==drop.key?(rest[drop.index]?.key===item.key?'before':drop.index===rest.length&&rest.at(-1)?.key===item.key?'after':undefined):undefined}>
    <button type="button" role="tab" title={item.title} id={`tab-${item.key}`} aria-selected={item.key===active} tabIndex={item.key===active?0:-1} aria-controls={`document-${item.key}`} onPointerDown={e=>{if(e.button!==0||entries.length<2)return;suppress.current=false;gesture.current={key:item.key,start:e.clientX,moving:false,index:entries.findIndex(x=>x.key===item.key)};e.currentTarget.setPointerCapture(e.pointerId);}} onPointerMove={e=>{const g=gesture.current;if(!g||g.key!==item.key)return;if(!g.moving&&Math.abs(e.clientX-g.start)<6)return;g.moving=true;const bar=root.current!;const bounds=bar.getBoundingClientRect();if(e.clientX<bounds.left+30)bar.scrollLeft-=16;else if(e.clientX>bounds.right-30)bar.scrollLeft+=16;const others=Array.from(bar.querySelectorAll<HTMLElement>('.workbench-tab')).filter(el=>el.dataset.key!==g.key);g.index=others.filter(el=>{const r=el.getBoundingClientRect();return e.clientX>r.left+r.width/2;}).length;setDrop({key:g.key,index:g.index});}} onPointerUp={e=>{const g=gesture.current;if(g?.moving){suppress.current=true;move(g.key,g.index);}gesture.current=null;setDrop(null);if(e.currentTarget.hasPointerCapture(e.pointerId))e.currentTarget.releasePointerCapture(e.pointerId);}} onPointerCancel={cancel} onLostPointerCapture={()=>{if(gesture.current)cancel();}} onClick={()=>{if(suppress.current){suppress.current=false;return;}select(item.key);}} onKeyDown={e=>{const index=entries.findIndex(x=>x.key===item.key);if(e.altKey&&['ArrowLeft','ArrowRight'].includes(e.key)){e.preventDefault();move(item.key,Math.max(0,Math.min(entries.length-1,index+(e.key==='ArrowRight'?1:-1))));return;}if(['ArrowLeft','ArrowRight','Home','End'].includes(e.key)){e.preventDefault();const next=entries[e.key==='Home'?0:e.key==='End'?entries.length-1:(index+(e.key==='ArrowRight'?1:entries.length-1))%entries.length];select(next.key);requestAnimationFrame(()=>document.getElementById(`tab-${next.key}`)?.focus());}}}><CircleDot size={13}/><span className="control-label-text">{item.title}</span></button><button type="button" aria-label={labels.close(item.title)} onClick={()=>close(item.key)}><X size={12}/></button>
  </div>)}</div>{scrollButton(1)}</div>{menu && <TabMenu locale={locale} target={menu} keys={entries.map(item=>item.key)} dismiss={dismissMenu} run={closeMany}/>}</>;
}
