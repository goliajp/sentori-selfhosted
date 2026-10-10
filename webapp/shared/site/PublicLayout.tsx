import type { ReactNode } from "react";
import { ArrowRight, Menu, X } from "lucide-react";
import { useSite } from "./runtime";
export function PublicLayout({ children, footer, standalone = false }: { children: ReactNode; footer?: ReactNode; standalone?: boolean }) {
  const Content = standalone ? 'main' : 'div';
  const { t, path, href, image } = useSite();
  return <div className={`welcome-page site-${path} ${standalone ? 'site-standalone' : ''}`}>
    <nav className="product-nav" aria-label={t("nav.welcome")}>
      <a href={href("welcome")} className="product-identity" aria-label="Sentori">
        <img src={image("lockup-blue.svg")} alt="" className="identity-light" />
        <img src={image("lockup-white.svg")} alt="" className="identity-dark" />
      </a>
      <div className="product-nav-links">{["welcome", "demo", "pricing", "docs", "login"].map(route => <a key={route} href={href(route)} aria-current={path === route ? "page" : undefined}>{t(`nav.${route}`)}</a>)}</div>
      <details className="product-mobile-menu" key={path} onKeyDown={event => { if (event.key === 'Escape') { event.currentTarget.open = false; event.currentTarget.querySelector('summary')?.focus(); } }}>
        <summary aria-label={t('site.navigation')}><Menu size={20} className="site-menu-open"/><X size={20} className="site-menu-close"/><span>{t('site.menu')}</span></summary>
        <div className="product-mobile-links" onClick={event => { if ((event.target as HTMLElement).closest('a')) event.currentTarget.closest('details')?.removeAttribute('open'); }}>{["welcome", "demo", "pricing", "docs", "login"].map(route => <a key={route} href={href(route)} aria-current={path === route ? "page" : undefined}>{t(`nav.${route}`)}<ArrowRight size={16}/></a>)}</div>
      </details>
    </nav>
    <Content id="public-content" tabIndex={-1}>{children}</Content>
    <footer className="studio-footer"><span>Sentori</span><div className="flex flex-wrap gap-5">{["docs", "pricing", "login"].map(route => <a className="text-link" key={route} href={href(route)}>{t(`nav.${route}`)}</a>)}</div>{footer}</footer>
  </div>;
}
