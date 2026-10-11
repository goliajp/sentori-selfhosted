import { useEffect, useId, useRef, useState } from 'react';
import type { ECharts } from 'echarts';
import { Download, Table2 } from 'lucide-react';
import { Surface } from '../../shared/design/Surface';
import { Button } from '../../shared/design/controls';
import { useLocale, useT } from '../i18n';
import { ErrorBanner, formatAbsolute } from './ui';
import { WorkbenchEmpty } from './WorkbenchEmpty';

export type TelemetrySeries = {key:string;label:string;values:(number|null)[];color?:string};

/** The plot, accessible table and CSV all share the same real response values. */
export function TelemetryChart({title,description,labels,series,unit='',kind='line',empty=false,time=false}: {title:string;description:string;labels:string[];series:TelemetrySeries[];unit?:string;kind?:'line'|'bar';empty?:boolean;time?:boolean}) {
  const t=useT(),locale=useLocale(),id=useId(),host=useRef<HTMLDivElement>(null);
  const [table,setTable]=useState(false),[hidden,setHidden]=useState<string[]>([]),[failure,setFailure]=useState<string|null>(null),[attempt,setAttempt]=useState(0);
  const [ready,setReady]=useState(false);
  const signature=JSON.stringify({labels,series,hidden,kind,unit});
  const dataPresent=!empty&&series.some(s=>s.values.some(v=>v!==null&&Number.isFinite(v)));
  useEffect(()=>{
    const element=host.current;
    if(!element||!dataPresent||table)return;
    let disposed=false,chart:ECharts|undefined;
    const resize=new ResizeObserver(()=>chart?.resize());resize.observe(element);
    const render=()=>{
      if(!chart)return;
      const style=getComputedStyle(document.documentElement),token=(name:string)=>style.getPropertyValue(name).trim();
      const snapshot=JSON.parse(signature) as {labels:string[];series:TelemetrySeries[];hidden:string[];kind:'line'|'bar';unit:string};
      chart.setOption({animation:false,aria:{enabled:true,description:title},color:snapshot.series.map((s,i)=>token(s.color??`--ui-chart-${i+1}`)||token('--ui-action')),
        textStyle:{fontFamily:style.fontFamily,color:token('--ui-secondary')},
        grid:{left:12,right:20,top:30,bottom:labels.length>12?50:20,containLabel:true},
        tooltip:{trigger:'axis',renderMode:'richText',valueFormatter:(value:unknown)=>typeof value==='number'?`${value.toLocaleString(locale,{maximumFractionDigits:2})}${unit?' '+unit:''}`:'—'},
        xAxis:{type:'category',data:snapshot.labels,axisLine:{lineStyle:{color:token('--ui-line')}},axisTick:{show:false},axisLabel:{hideOverlap:true,width:110,overflow:'truncate',formatter:time?(value:string)=>new Date(value).toLocaleString(locale,{month:'short',day:'numeric',hour:'2-digit',minute:'2-digit'}):undefined}},
        yAxis:{type:'value',min:0,name:unit,axisLabel:{formatter:(value:number)=>value.toLocaleString(locale,{notation:'compact'})},splitLine:{lineStyle:{color:token('--ui-line'),type:'dashed'}}},
        dataZoom:labels.length>12?[{type:'slider',height:15,bottom:2,borderColor:token('--ui-line'),textStyle:{color:token('--ui-secondary')},start:0,end:100}]:[],
        series:snapshot.series.map((s,index)=>({id:s.key,name:s.label,type:snapshot.kind,data:snapshot.hidden.includes(s.key)?s.values.map(()=>null):s.values,connectNulls:false,showSymbol:labels.length<40,symbolSize:5,lineStyle:{width:2},barMaxWidth:28,itemStyle:{color:token(s.color??`--ui-chart-${index+1}`)||token('--ui-action')},emphasis:{focus:'series'}}))},{notMerge:true});
    };
    const theme=new MutationObserver(render);theme.observe(document.documentElement,{attributes:true,attributeFilter:['data-theme']});
    void import('../lib/chart-engine').then(engine=>{if(disposed)return;chart=engine.init(element,undefined,{renderer:'svg'});render();setReady(true);setFailure(null);}).catch(error=>{if(!disposed)setFailure(String(error));});
    return()=>{disposed=true;resize.disconnect();theme.disconnect();chart?.dispose();};
  },[signature,locale,title,unit,dataPresent,table,attempt,labels.length,time]);
  function download(){
    const quote=(value:unknown)=>{const raw=String(value??'');return '"'+(typeof value==='string'&&/^[=+@\-\t\r]/.test(raw)?"'"+raw:raw).replaceAll('"','""')+'"';};
    const csv=[[t('charts.dimension'),...series.map(s=>s.label+(unit?' ('+unit+')':''))],...labels.map((label,index)=>[label,...series.map(s=>s.values[index]??'')])].map(row=>row.map(quote).join(',')).join('\r\n');
    const url=URL.createObjectURL(new Blob(['\ufeff'+csv],{type:'text/csv;charset=utf-8'}));const anchor=document.createElement('a');anchor.href=url;anchor.download='sentori-metrics.csv';anchor.click();setTimeout(()=>URL.revokeObjectURL(url),1000);
  }
  return <Surface className="telemetry-chart" title={title} action={<div className="chart-actions"><Button small variant="ghost" aria-pressed={table} onClick={()=>setTable(value=>!value)}><Table2 size={14}/>{t(table?'charts.plot':'charts.table')}</Button><Button small variant="ghost" disabled={!dataPresent} onClick={download}><Download size={14}/>{t('charts.export')}</Button></div>}>
    <p className="chart-description" id={id}>{description}</p>
    {failure&&<div><ErrorBanner reason={failure}>{t('charts.failed')}</ErrorBanner><Button small onClick={()=>setAttempt(value=>value+1)}>{t('common.retry')}</Button></div>}
    {!dataPresent?<WorkbenchEmpty title={t('charts.empty')} description={t('charts.emptyHint')}/>:table?<div className="chart-data-table"><table><caption className="sr-only">{title}</caption><thead><tr><th>{t('charts.dimension')}</th>{series.map(s=><th key={s.key}>{s.label}{unit?' · '+unit:''}</th>)}</tr></thead><tbody>{labels.map((label,index)=><tr key={index}><th scope="row">{time?<time dateTime={label}>{formatAbsolute(label)}</time>:label}</th>{series.map(s=><td key={s.key}>{s.values[index]?.toLocaleString(locale,{maximumFractionDigits:2})??'—'}</td>)}</tr>)}</tbody></table></div>:<>
      <div className="chart-legend">{series.map((s,index)=><Button small variant="ghost" key={s.key} aria-pressed={!hidden.includes(s.key)} onClick={()=>setHidden(old=>old.includes(s.key)?old.filter(key=>key!==s.key):[...old,s.key])}><span className="chart-swatch" style={{background:`var(${s.color??`--ui-chart-${index+1}`})`}}/>{s.label}</Button>)}</div>
      <div ref={host} className="telemetry-plot" role="img" aria-label={title} aria-describedby={id} data-chart-ready={ready}/>
    </>}
  </Surface>;
}
