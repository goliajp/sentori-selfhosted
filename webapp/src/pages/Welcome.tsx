import { ArrowRight } from 'lucide-react';
import { WelcomeView } from '../../shared/site/WelcomeView';
import { SiteBanner } from '../../shared/site/SiteBanner';
import { SectionLayout } from '../../shared/site/SectionLayout';
import { useSite } from '../../shared/site/runtime';
import { Button } from '../../shared/design/controls';
import { PublicLayout } from '../components/PublicLayout';
import { ProductStory } from '../components/ProductStory';
function WelcomeContent() {
  const { image } = useSite();
  return <WelcomeView story={<ProductStory />} art={<img className="ui-illustration ui-illustration-scene" src={image('observatory.png')} alt="" loading="lazy" />} />;
}
export default function Welcome() { return <PublicLayout><WelcomeContent /></PublicLayout>; }
function DemoContent() {
  const { t, navigate } = useSite();
  return <><SiteBanner scene="demo"><span className="eyebrow">{t('source.story.example')}</span><h1>{t('nav.demo')}</h1><p>{t('site.demoHint')}</p><div className="hero-actions"><Button size="large" variant="primary" onClick={() => navigate('docs', {topic: 'getting-started'})}>{t('public.start')}<ArrowRight size={16}/></Button></div></SiteBanner><SectionLayout title={t('source.story.steps')}><ProductStory /></SectionLayout></>;
}
export function Demo() { return <PublicLayout><DemoContent /></PublicLayout>; }
