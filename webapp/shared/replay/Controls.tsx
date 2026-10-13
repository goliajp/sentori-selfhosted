import { NativeSelect } from "../design/controls";
import type { ReactNode } from 'react';
import { ChevronLeft, ChevronRight, Pause, Play } from 'lucide-react';
import { Button } from '../design/controls';
export function ReplayControls({index,last,playing,speed,time,onIndex,onToggle,onSpeed,labels}:{
  index:number;last:number;playing:boolean;speed:string;time:ReactNode;
  onIndex:(index:number)=>void;onToggle:()=>void;onSpeed:(speed:string)=>void;
  labels:{play:string;pause:string;back:string;forward:string;seek:string;speed:string};
}) {
  return <div className="replay-controls">
    <Button disabled={last===0} aria-label={playing?labels.pause:labels.play} onClick={onToggle}>{playing?<Pause size={16}/>:<Play size={16}/>}</Button>
    <Button aria-label={labels.back} disabled={index===0} onClick={()=>onIndex(index-1)}><ChevronLeft size={16}/></Button>
    <Button aria-label={labels.forward} disabled={index===last} onClick={()=>onIndex(index+1)}><ChevronRight size={16}/></Button>
    <label className="flex-1 min-w-24"><span className="sr-only">{labels.seek}</span><input type="range" min={0} max={last} value={index} onChange={event=>onIndex(Number(event.target.value))}/></label>
    <span className="tabular text-xs">{time}</span>
    <label className="field"><span className="field-label">{labels.speed}</span><NativeSelect className="input" value={speed} onChange={event=>onSpeed(event.target.value)}>{['0.5','1','2','4'].map(value=><option key={value} value={value}>{value}×</option>)}</NativeSelect></label>
  </div>;
}
