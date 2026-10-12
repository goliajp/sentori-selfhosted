import { useEffect, useLayoutEffect, useRef } from 'react';

import { tabCloseTargets } from './tab-actions';
const messages = {
  en: { menu:'Tab actions', close:'Close', others:'Close other tabs', left:'Close tabs to the left', right:'Close tabs to the right', all:'Close all tabs' },
  zh: { menu:'标签操作', close:'关闭', others:'关闭其他标签', left:'关闭左侧标签', right:'关闭右侧标签', all:'关闭所有标签' },
  ja: { menu:'タブ操作', close:'閉じる', others:'他のタブを閉じる', left:'左側のタブを閉じる', right:'右側のタブを閉じる', all:'すべてのタブを閉じる' },
};
export function TabMenu({ locale, target, keys, dismiss, run }: {
  locale:'en'|'zh'|'ja'; target:{key:string;x:number;y:number}; keys:string[];
  dismiss:(restoreFocus?:boolean)=>void; run:(keys:string[])=>void;
}) {
  const ref=useRef<HTMLDivElement>(null), labels=messages[locale];
  useLayoutEffect(()=>{
    const menu=ref.current!; menu.showPopover();
    const rect=menu.getBoundingClientRect();
    menu.style.left=`${Math.max(4,Math.min(target.x,innerWidth-rect.width-4))}px`;
    menu.style.top=`${Math.max(4,Math.min(target.y,innerHeight-rect.height-4))}px`;
    menu.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus();
    return ()=>menu.hidePopover();
  },[target]);
  // WebKit dispatches contextmenu before the opening mouse gesture finishes.
  // An auto popover treats that gesture's release as an outside dismissal.
  useEffect(()=>{
    const outside=(event:Event)=>{if(!ref.current?.contains(event.target as Node))dismiss(false);};
    const blur=()=>dismiss(false);
    document.addEventListener('pointerdown',outside,true);
    document.addEventListener('scroll',outside,true);
    window.addEventListener('blur',blur);
    return()=>{document.removeEventListener('pointerdown',outside,true);document.removeEventListener('scroll',outside,true);window.removeEventListener('blur',blur);};
  },[dismiss]);
  return <div ref={ref} popover="manual" role="menu" aria-label={labels.menu} className="workbench-tab-menu" onKeyDown={event=>{
    const buttons=Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>('button:not(:disabled)'));
    const index=buttons.indexOf(document.activeElement as HTMLButtonElement);
    if(['ArrowDown','ArrowUp','Home','End'].includes(event.key)) {
      event.preventDefault(); buttons[event.key==='Home'?0:event.key==='End'?buttons.length-1:(index+(event.key==='ArrowDown'?1:buttons.length-1))%buttons.length]?.focus();
    } else if(event.key==='Escape'||event.key==='Tab') {event.preventDefault();event.stopPropagation();dismiss();}
  }}>{(['close','others','left','right','all'] as const).map(action=>{
    const targets=tabCloseTargets(keys,target.key,action);
    return <button type="button" role="menuitem" key={action} disabled={!targets.length} onClick={()=>{dismiss(false);run(targets);}}><span className="control-label-text">{labels[action]}</span></button>;
  })}</div>;
}
