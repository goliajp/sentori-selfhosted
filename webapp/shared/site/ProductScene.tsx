import { Activity, Code2, GitBranch, MessageSquare, ScanLine, Users } from 'lucide-react';
import { useSite } from './runtime';

export type ProductSceneKind = 'impact' | 'evidence' | 'team';

/** Real interface captures, arranged as a product illustration with readable callouts. */
export function ProductScene({ kind }: { kind: ProductSceneKind }) {
  const { t, locale, number, image } = useSite();
  const Icon = { impact: Activity, evidence: Code2, team: Users }[kind];
  return <figure className={`product-scene product-scene-${kind}`} aria-label={t(`scene.${kind}.alt`)}>
    <div className="product-scene-grid" aria-hidden="true" />
    <div className="product-scene-orbit" aria-hidden="true" />
    <div className="product-scene-window">
      <div className="product-scene-chrome"><span className="scene-window-dots" aria-hidden="true"><i/><i/><i/></span><span>Sentori</span><code>ISS-1001</code></div>
      {(['light', 'dark'] as const).map(theme => <img key={theme} className={`product-scene-capture scene-capture-${theme}`} src={image(`site-scenes-v1/${kind}-${locale}-${theme}.png`)} alt="" loading="lazy" decoding="async" />)}
    </div>
    <div className="scene-glass scene-glass-title"><span className="scene-glass-icon"><Icon size={18}/></span><div><small>{t('scene.example')}</small><strong>{t(`scene.${kind}.title`)}</strong></div></div>
    {kind === 'impact' ? <div className="scene-glass scene-glass-detail scene-metrics"><div><strong>{number(72)}</strong><span>{t('issue.events')}</span></div><div><strong>{number(17)}</strong><span>{t('issue.users')}</span></div><span className="scene-metric-release"><GitBranch size={14}/>3.8.0</span></div>
      : kind === 'evidence' ? <div className="scene-glass scene-glass-detail scene-evidence"><span><ScanLine size={16}/>{t('scene.event')}</span><i/><span><Code2 size={16}/>{t('scene.source')}</span><i/><span><Activity size={16}/>{t('scene.replay')}</span></div>
      : <div className="scene-glass scene-glass-detail scene-team"><span className="scene-avatar">AM</span><div><strong>Aiko Mori</strong><span>{t('scene.team.detail')}</span></div><MessageSquare size={18}/></div>}
  </figure>;
}
