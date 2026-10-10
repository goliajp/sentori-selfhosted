import { useSite } from './runtime';
import type { ReactNode } from 'react';

/** Complete landscape artwork with a separate, accessible text layer. */
export function SiteBanner({ scene, children, note }: {
  scene: 'welcome' | 'demo'; children: ReactNode; note?: ReactNode;
}) {
  const { image } = useSite();
  return <section className={`site-banner site-banner-${scene}`}>
    <img className="site-banner-landscape" src={image(`site-scenes-v1/${scene}-panorama.png`)} alt="" fetchPriority="high" />
    <div className="site-banner-copy">{children}</div>
    {note && <div className="site-banner-note">{note}</div>}
  </section>;
}
