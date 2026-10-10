import { renderArticle, scrollToHeading } from "./render";
import { useMemo, type MouseEvent } from "react";
import { type DocPage } from "./model";

import { Prose } from "./Document";
import { Code } from "./Code";
import { useSite } from '../site/runtime';

export function DocArticle({ page, version, lang = 'en' }: { page: DocPage; version?: string; lang?: string }) {
  const { t, href } = useSite();
  const article = useMemo(() => renderArticle(page, version, { omitTitle: true, linkLabel: t('docs.sectionLink'), href }), [page, version, t, href]);
  function follow(event: MouseEvent) {
    const anchor = (event.target as HTMLElement).closest<HTMLAnchorElement>("a");
    if (!anchor) return;
    const target = anchor.getAttribute("href") ?? "";
    if (target.startsWith("#") && !target.startsWith("#/")) { event.preventDefault(); scrollToHeading(decodeURIComponent(target.slice(1))); }
  }
  return <Prose as="article" className="docs-prose" lang={lang} onClick={follow}>{article.html.split(/<!--VIS-CODE:(\d+)-->/).map((part, index) => index % 2 ? <Code t={t} key={index} {...article.codes[Number(part)]} /> : <div className="document-fragment" key={index} dangerouslySetInnerHTML={{ __html: part }} />)}</Prose>;
}
