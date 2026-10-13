import { Action } from "../design/controls";
import { useId, useState } from 'react';
import { Check, ArrowRight, Activity, Code2, GitBranch } from 'lucide-react';
import { useSite } from './runtime';
import type { ReactNode } from 'react';

import { Badge, Button } from '../design/controls';
import { Surface } from '../design/Surface';
/** All stages occupy the same grid cell; the tallest content sets their shared height. */
export function ProductStory({ code }: { code: ReactNode }) {
  const { t, number, navigate } = useSite(), id = useId();
  const [stage, setStage] = useState(0);
  const icons = [Code2, Activity, GitBranch];
  return <Surface title={t('source.story.example')} className="product-story-surface" action={<Badge>checkout · iOS</Badge>} footer={<div className="step-actions"><span className="text-xs subtle">{stage + 1} / 3</span><Button onClick={() => stage < 2 ? setStage(stage + 1) : navigate('demo', { journey: 'investigate' })}>{t(stage < 2 ? 'common.next' : 'public.demo')}<ArrowRight size={15}/></Button></div>}>
    <div className="product-story"><nav aria-label={t('source.story.steps')} role="tablist">{icons.map((Icon, step) => <Action type="button" role="tab" key={step} id={`${id}-tab-${step}`} aria-controls={`${id}-panel-${step}`} aria-selected={step === stage} tabIndex={step === stage ? 0 : -1} onClick={() => setStage(step)} onKeyDown={event => { if (['ArrowLeft','ArrowRight','ArrowUp','ArrowDown','Home','End'].includes(event.key)) { event.preventDefault(); const next = event.key === 'Home' ? 0 : event.key === 'End' ? 2 : (stage + (event.key === 'ArrowRight' || event.key === 'ArrowDown' ? 1 : 2)) % 3; setStage(next); document.getElementById(`${id}-tab-${next}`)?.focus(); } }}><Icon size={17}/><span>{t(`source.story.step.${step}`)}</span><small>0{step + 1}</small></Action>)}</nav>
      <div className="product-story-stage">{[0,1,2].map(step => <section className="product-story-panel" key={step} id={`${id}-panel-${step}`} role="tabpanel" aria-labelledby={`${id}-tab-${step}`} aria-hidden={step !== stage} inert={step !== stage} data-active={step === stage}>
        <header><h3>{t(`source.story.title.${step}`)}</h3><p>{t(`source.story.description.${step}`)}</p></header>
        {step === 0 ? <div className="story-code"><div className="flex gap-3 mb-5"><Badge>{t('source.story.impact')}</Badge><Badge>v1.4.2</Badge></div>{code}</div> : step === 1 ? <ol className="story-signals">{['cart','tap','network','error'].map((key,index) => <li key={key}><span className="tabular text-xs subtle">{['−12s','−3s','−1s','0s'][index]}</span><div><h4>{t(`source.story.event.${key}`)}</h4><p>{t(`source.story.eventHint.${key}`)}</p></div></li>)}</ol> : <div className="story-result"><p className="text-success flex items-center gap-2"><Check size={17}/>{t('source.story.release')}</p><p className="mt-5 text-3xl tabular">{number(2480)} <span className="text-sm muted">{t('source.story.sessions')}</span></p><p className="text-sm mt-3">{t('source.story.result')}</p><p className="text-xs subtle mt-5">{t('source.story.coverage')}</p></div>}
      </section>)}</div>
    </div>
  </Surface>;
}
