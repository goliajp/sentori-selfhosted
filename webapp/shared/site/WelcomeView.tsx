import type { ReactNode } from 'react';
import { ArrowRight, Check, Activity, Code2, GitBranch, ShieldCheck, Users } from 'lucide-react';
import { Button } from '../design/controls';
import { SiteBanner } from './SiteBanner';
import { ProductScene } from './ProductScene';
import { useSite } from './runtime';
import { SectionLayout } from './SectionLayout';
export function WelcomeView({ story, art }: { story: ReactNode; art: ReactNode }) {
  const { t, href, navigate } = useSite();
  return <>
    <SiteBanner scene="welcome" note={<><CircleEvidence /><span>checkout · iOS</span><code>OrderConfirmation.swift:84</code></>}><span className="eyebrow">{t("site.productEyebrow")}</span><h1>{t("public.title")}</h1><p>{t("public.subtitle")}</p><div className="hero-actions"><Button size="large" variant="primary" onClick={() => navigate("docs", {topic:"getting-started"})}>{t("public.start")}<ArrowRight size={16}/></Button><Button size="large" onClick={() => navigate("demo")}>{t("public.demo")}</Button></div></SiteBanner>
    <div className="platform-rail">{['React Native', 'Expo', 'Swift', 'Kotlin', 'Web', 'WeChat Mini Program'].map(platform => <span key={platform}>{platform}</span>)}</div>
    <SectionLayout title={t('site.evidence')} description={t('site.evidenceHint')}>{story}</SectionLayout>
    <section className="site-feature-stories" aria-label={t('public.features')}>
      {([{Icon: Activity, scene:'impact', topic:'dashboard'}, {Icon: Code2, scene:'evidence', topic:'getting-started'}, {Icon: Users, scene:'team', topic:'teams'}] as const).map(({Icon, scene, topic}, index) => <article className="site-feature-story" key={scene}><div className="site-feature-copy"><span className="site-feature-kicker"><Icon size={20} /><span>0{index + 1}</span></span><h2>{t(`public.feature${index + 1}`)}</h2><p>{t(`public.feature${index + 1}Text`)}</p><a href={href('docs', {topic})}>{t('common.details')}<ArrowRight size={16}/></a></div><div className="site-feature-art"><ProductScene kind={scene}/></div></article>)}
    </section>
    <section className="site-trust"><div><span className="eyebrow"><ShieldCheck size={16}/>{t('source.site.trustTitle')}</span><h2>{t('site.own')}</h2><p>{t('source.site.trustDescription')}</p><ul>{['privacy', 'failure', 'source'].map(key => <li key={key}><Check size={16}/><span>{t(`source.site.control.${key}`)}</span></li>)}</ul><Button size="large" onClick={() => navigate('docs', {topic:'self-hosting'})}>{t('source.site.deployment')}<ArrowRight size={16}/></Button></div>{art}</section>
    <section className="site-closing"><div><h2>{t('site.closing')}</h2><p>{t('site.closingHint')}</p></div><Button size="large" variant="primary" onClick={() => navigate('docs', {topic:'getting-started'})}>{t('public.start')}<ArrowRight size={16}/></Button></section>
  </>;
}
function CircleEvidence() { return <span className="hero-evidence-indicator" aria-hidden="true"><GitBranch size={14}/></span>; }
