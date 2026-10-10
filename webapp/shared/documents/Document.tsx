import { useId, type CSSProperties, type HTMLAttributes, type ReactNode } from 'react';

export function Prose({ as: Tag = 'div', className = '', ...props }: HTMLAttributes<HTMLElement> & { as?: 'div' | 'article' }) {
  return <Tag className={`document-prose ${className}`} {...props} />;
}

/** A page's heading, reading column, supporting rail and footer share one layout. */
export function ContentPage({ title, description, actions, aside, footer, children, titleAs = 'h1', className = '' }: {
  title: string; description?: string; actions?: ReactNode; aside?: ReactNode; footer?: ReactNode;
  children: ReactNode; titleAs?: 'h1' | 'h2' | 'h3'; className?: string;
}) {
  const id = useId(), Title = titleAs;
  return <article className={`document-page ${className}`} aria-labelledby={id}>
    <header className="document-page-header"><div><Title className="document-title" id={id}>{title}</Title>{description && <p className="document-lead">{description}</p>}</div>{actions && <div className="document-page-actions">{actions}</div>}</header>
    <div className={`document-page-body${aside ? ' has-aside' : ''}`}><div className="document-reading-column">{children}</div>{aside && <aside className="document-aside">{aside}</aside>}</div>
    {footer && <footer className="document-page-footer">{footer}</footer>}
  </article>;
}

export function ContentSection({ title, description, children, level = 2 }: { title: string; description?: string; children: ReactNode; level?: 2 | 3 | 4 }) {
  const id = useId(), Title = `h${level}` as 'h2' | 'h3' | 'h4';
  return <section className="document-section" aria-labelledby={id}><header><Title id={id}>{title}</Title>{description && <p>{description}</p>}</header>{children}</section>;
}

/** Caption and media are one unit; a media slot owns the aspect ratio before loading. */
export function ContentFigure({ children, caption, credit, ratio, className = '' }: {
  children: ReactNode; caption: string; credit?: string; ratio?: '1 / 1' | '3 / 2' | '16 / 9'; className?: string;
}) {
  const id = useId();
  return <figure className={`document-figure ${className}`} aria-labelledby={id}>
    <div className={`document-media${ratio ? ' has-ratio' : ''}`} style={ratio ? { '--figure-ratio': ratio } as CSSProperties : undefined}>{children}</div>
    <figcaption id={id}>{caption}{credit && <span className="document-credit">{credit}</span>}</figcaption>
  </figure>;
}

export type ContentListItem = { id: string; title: string; description?: string; leading?: ReactNode; trailing?: ReactNode };
export function ContentList({ items, label, divided = false }: { items: readonly ContentListItem[]; label: string; divided?: boolean }) {
  return <ul className="document-list" aria-label={label} data-divided={divided || undefined}>{items.map(item => <li key={item.id}>
    {item.leading && <span className="document-list-leading">{item.leading}</span>}
    <div className="document-list-copy"><strong>{item.title}</strong>{item.description && <p>{item.description}</p>}</div>
    {item.trailing && <div className="document-list-trailing">{item.trailing}</div>}
  </li>)}</ul>;
}
