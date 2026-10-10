import { useState } from 'react';
import { ArrowRight, Check, HelpCircle, Plus, Server, ShieldCheck } from 'lucide-react';
import { useSite } from './runtime';


import { Button, Field, Input } from '../design/controls';
import { Dialog } from '../design/Dialog';
import { SegmentedControl } from '../design/SegmentedControl';
import { cloudPlans, recommendPlan, selfHostedServices, type BillingCycle } from './pricing-model';

export function PricingView({ active = true }: {active?: boolean}) {
  const { t, number, navigate } = useSite();
  const [cycle, setCycle] = useState<BillingCycle>('monthly');
  const [selected, setSelected] = useState<string | null>(null);
  const [usage, setUsage] = useState({ events: 100_000, replays: 1_000, ingestGB: 10 });
  const quote = recommendPlan(usage.events, usage.replays, usage.ingestGB, cycle);
  const oversized = usage.events > 10_000_000 || usage.replays > 100_000 || usage.ingestGB > 1_000;
  const selectedPlan = cloudPlans.find(plan => plan.id === selected);
  const selectedService = selfHostedServices.find(service => service.id === selected);
  const rows = [
    ['eventRow', (p: typeof cloudPlans[number]) => number(p.events)],
    ['replayRow', (p: typeof cloudPlans[number]) => number(p.replays)],
    ['dataRow', (p: typeof cloudPlans[number]) => `${p.ingestGB} GB`],
    ['artifactRow', (p: typeof cloudPlans[number]) => `${p.artifactsGB} GB`],
    ['eventDays', (p: typeof cloudPlans[number]) => t('plans.days', {count:p.eventDays})],
    ['replayDays', (p: typeof cloudPlans[number]) => t('plans.days', {count:p.replayDays})],
    ['projectRow', (p: typeof cloudPlans[number]) => number(p.projects)],
    ['rateRow', (p: typeof cloudPlans[number]) => number(p.rate)],
    ['supportRow', (p: typeof cloudPlans[number]) => t(`plans.${({free:'community',starter:'email',team:'priority',scale:'fast'} as const)[p.id]}`)],
  ] as const;
  return <>
    <header className="site-page-intro solutions-intro"><span className="eyebrow">{t('nav.pricing')}</span><h1>{t('plans.title')}</h1><p>{t('plans.intro')}</p></header>
    <div className="solutions-toolbar"><nav aria-label={t('nav.pricing')}><a href="#cloud-plans" onClick={e => {e.preventDefault(); document.getElementById('cloud-plans')?.scrollIntoView({behavior:'smooth'});}}>{t('plans.cloud')}</a><a href="#self-hosted-plans" onClick={e => {e.preventDefault(); document.getElementById('self-hosted-plans')?.scrollIntoView({behavior:'smooth'});}}>{t('plans.self')}</a></nav><SegmentedControl label={t('plans.cycle')} value={cycle} onChange={value=>setCycle(value as BillingCycle)} options={(['monthly','yearly'] as const).map(value=>[value,t(`plans.${value}`)])}/></div>
    <p className="solutions-preview">{t('plans.preview')}</p>
    <section id="cloud-plans" aria-label={t('plans.cloud')} className="solutions-grid">
      {cloudPlans.map(plan=><article className={`surface solution-card ${plan.id==='team'?'solution-featured':''}`} key={plan.id} data-plan={plan.id}>
        <header><h2>{plan.name}</h2>{plan.id==='team'&&<span className="solution-badge">{t('plans.recommended')}</span>}<p>{t(`plans.${plan.id}For`)}</p></header>
        <div className="solution-price"><strong>${number(cycle==='yearly'?plan.annual/12:plan.monthly,2)}</strong><span>{t('plans.month')}</span></div>
        <p className="solution-billing">{plan.id==='free'?t('plans.freeBill'):cycle==='yearly'?t('plans.annual',{amount:plan.annual}):t('plans.monthBill')}</p>
        <ul><li><Check/><span><strong>{number(plan.events)}</strong> {t('plans.events')}</span></li><li><Check/><span><strong>{number(plan.replays)}</strong> {t('plans.replays')}</span></li><li><Check/><span>{t('plans.retention',{days:plan.eventDays})}</span></li><li><Check/><span>{t('plans.projects',{count:plan.projects})}</span></li></ul>
        <Button size="large" variant={plan.id==='team'?'primary':'secondary'} onClick={()=>setSelected(plan.id)}>{t('plans.details')}<ArrowRight size={16}/></Button>
      </article>)}
    </section>
    <div className="solutions-footnotes"><p><ShieldCheck size={16}/>{t('plans.common')}</p><p>{t('plans.moneyNote')}</p></div>
    <section className="solutions-section"><header><h2>{t('plans.compare')}</h2><p>{t('plans.compareHint')}</p></header><div className="solution-table-scroll" role="region" aria-label={t('plans.compare')} tabIndex={0}><table className="solution-table"><colgroup><col className="solution-dimension-col" />{cloudPlans.map(plan=><col className="solution-plan-col" key={plan.id} />)}</colgroup><thead><tr><th scope="col">{t('plans.dimension')}</th>{cloudPlans.map(plan=><th scope="col" key={plan.id}>{plan.name}</th>)}</tr></thead><tbody>{rows.map(([key,format])=><tr key={key}><th scope="row">{t(`plans.${key}`)}</th>{cloudPlans.map(plan=><td key={plan.id}>{format(plan)}</td>)}</tr>)}</tbody></table></div><p className="solutions-caption">{t('plans.hours')}</p></section>
    <section className="solutions-budget"><div><ShieldCheck size={28}/><h2>{t('plans.budgetTitle')}</h2><p>{t('plans.budgetBody')}</p></div><div><p>{t('plans.packs')}</p><ul>{['eventPack','replayPack','dataPack'].map(key=><li key={key}><Check size={16}/><span>{t(`plans.${key}`)}</span></li>)}</ul></div></section>
    <section className="solutions-section solution-estimator"><header><h2>{t('plans.calcTitle')}</h2><p>{t('plans.calcHint')}</p></header><div className="solution-calculator"><div className="solution-usage">{(['events','replays','ingestGB'] as const).map((key,index)=><Field label={t(`plans.${['calcEvents','calcReplay','calcData'][index]}`)} key={key}><Input data-usage={key} type="number" min={0} step={key==='ingestGB'?1:100} value={usage[key]} onChange={e=>setUsage({...usage,[key]:Number.isFinite(e.target.valueAsNumber) ? Math.max(0,e.target.valueAsNumber) : 0})}/></Field>)}</div><output className="solution-estimate" aria-live="polite"><span>{t('plans.estimate')}</span>{oversized?<strong>{t('plans.customVolume')}</strong>:<><div><strong>${number(quote.total,2)}</strong><span>{quote.plan.name}</span></div><p>{quote.addons?t('plans.extra',{amount:quote.addons}):t('plans.noExtra')}</p>{cycle==='yearly'&&quote.plan.annual>0&&<p>{t('plans.annual',{amount:quote.plan.annual})}</p>}</>}</output></div></section>
    <section id="self-hosted-plans" className="solutions-section"><header><h2>{t('plans.self')}</h2><p>{t('plans.selfIntro')}</p></header><div className="solution-license"><Server size={28}/><div><h3>{t('plans.license')}</h3><p>{t('plans.selfBody')}</p></div><Button onClick={()=>navigate('docs',{topic:'self-hosting'})}>{t('site.deploy')}<ArrowRight size={16}/></Button></div><div className="solution-services">{selfHostedServices.map(service=><article className="surface solution-service" key={service.id}><h3>{t(`plans.${service.id}`)}</h3><div className="solution-price"><strong>${service.price}</strong><span>{t(service.id==='launch'?'plans.once':'plans.month')}</span></div><p>{t(`plans.${service.id}Body`)}</p><Button onClick={()=>setSelected(service.id)}>{t('plans.scope')}<ArrowRight size={16}/></Button></article>)}</div><p className="solutions-caption">{t('plans.supportTerms')}</p><div className="solution-enterprise"><h3>{t('plans.enterprise')}</h3><p>{t('plans.enterpriseBody')}</p></div></section>
    <section className="site-faq"><div className="site-faq-intro"><HelpCircle size={28}/><h2>{t('plans.rules')}</h2><p>{t('plans.moneyNote')}</p></div><div className="site-faq-list">{['events','data','excess','billing','migration'].map((key,index)=><details key={key} open={index===0}><summary><span>{t(`plans.q.${key}`)}</span><Plus size={18}/></summary><p>{t(`plans.a.${key}`)}</p></details>)}</div></section>
    <Dialog closeLabel={t("common.close")} active={active} open={selected!==null} onClose={()=>setSelected(null)} title={t('plans.dialog')} footer={<><Button onClick={()=>setSelected(null)}>{t('common.close')}</Button><Button variant="primary" onClick={()=>{setSelected(null);navigate('docs',{topic:selectedService?'self-hosting':'getting-started'});}}>{t('plans.next')}<ArrowRight size={16}/></Button></>}>
      <div className="solution-dialog"><h3>{selectedPlan?.name??(selectedService?t(`plans.${selectedService.id}`):'')}</h3>{selectedPlan&&<><div className="solution-price"><strong>${number(cycle==='yearly'?selectedPlan.annual/12:selectedPlan.monthly,2)}</strong><span>{t('plans.month')}</span></div><p>{selectedPlan.id==='free'?t('plans.freeBill'):cycle==='yearly'?t('plans.annual',{amount:selectedPlan.annual}):t('plans.monthBill')}</p><p>{number(selectedPlan.events)} {t('plans.events')} · {number(selectedPlan.replays)} {t('plans.replays')}</p><p>{t('plans.budgetBody')}</p></>}{selectedService&&<><div className="solution-price"><strong>${selectedService.price}</strong><span>{t(selectedService.id==='launch'?'plans.once':'plans.month')}</span></div><p>{t(`plans.${selectedService.id}Body`)}</p><p>{t('plans.supportTerms')}</p></>}<p className="solution-dialog-note">{t('plans.dialogNote')}</p></div>
    </Dialog>
  </>;
}
