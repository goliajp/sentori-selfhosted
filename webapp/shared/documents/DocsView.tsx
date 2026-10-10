import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { ArrowRight, BookOpen, Braces, Compass, GitBranch, History, Rocket, Search, Server, ChevronDown, ChevronRight, FileCode, Link, Menu, List, X } from 'lucide-react';
import { SECTIONS } from './model';
import { docParams, docVersions as archivedVersions, getVersionPage, latestDocVersion as releasedVersion, type DocVersion } from './versions';
import { useSite } from '../site/runtime';
import { downloadText } from './download';
import { renderArticle, scrollToHeading } from './render';
import { DocArticle } from './DocArticle';
import type { ReactNode } from 'react';
function DocumentationLayout({navigation,article,outline}:{navigation:ReactNode;article:ReactNode;outline:ReactNode}) { return <div className="docs-grid"><nav className="docs-sidebar">{navigation}</nav><div className="min-w-0">{article}</div><aside className="docs-sidebar docs-outline">{outline}</aside></div>; }
function StatePanel({title,description,action}:{title:string;description?:string;action?:ReactNode}) { return <div className="state-panel" role="status"><div className="state-copy"><h1>{title}</h1>{description && <p>{description}</p>}</div><div className="state-actions">{action}</div></div>; }
import { Badge, Button } from '../design/controls';
import { Code } from './Code';
import { Dialog } from '../design/Dialog';

function VersionRegistry({ selected, versions }: { selected: DocVersion; versions: DocVersion[] }) {
  const { t, href } = useSite();
  return <section className="docs-version-registry">
    <span className="eyebrow"><GitBranch size={16}/>{t('docs.versions')}</span>
    <h1>{t('docs.versions')}</h1>
    <p>{t('docs.supportNote')}</p>
    <div className="docs-lts-note"><Badge>{t('docs.ltsUnassigned')}</Badge></div>
    <div className="data-wrap" tabIndex={0} role="region" aria-label={t('docs.versions')}>
      <table className="data-table docs-version-table"><thead><tr><th>{t('docs.version')}</th><th>{t('docs.releaseDate')}</th><th>{t('docs.maintenance')}</th><th>{t('docs.endOfSupport')}</th><th>{t('docs.changes')}</th></tr></thead>
        <tbody>{versions.map(version => <tr key={version.id} data-current={version.id === selected.id || undefined}><td><a className="text-link" href={href('docs',docParams(version.id))}>{version.id}</a><small>{t(`docs.${version.channel === 'current' ? 'workingCopy' : version.channel === 'latest' ? 'latest' : 'archive'}`)}</small></td><td>{version.releasedAt ? <time dateTime={version.releasedAt}>{version.releasedAt}</time> : t("docs.unreleased")}</td><td><Badge tone={version.support === 'eol' ? 'danger' : version.support === 'lts' ? 'success' : 'neutral'}>{t(`docs.support.${version.support}`)}</Badge></td><td>{version.endOfSupport ?? t('docs.notAnnounced')}</td><td><a className="text-link" href={href('docs',docParams(version.id,'_changes'))}>{t('docs.changes')}<ArrowRight size={13}/></a></td></tr>)}</tbody>
      </table>
    </div>
    <div className="docs-support-key">{(['active','lts','eol'] as const).map(status => <div key={status}><strong>{t(`docs.support.${status}`)}</strong><p>{t(`docs.policy.${status}`)}</p></div>)}</div>
  </section>;
}

type OutlineHeading = { id: string; text: string; depth: number };

function DocsOutline({ headings, version, topic }: { headings: OutlineHeading[]; version: string; topic: string }) {
  const { t, href } = useSite();
  const [current, setCurrent] = useState('');
  useEffect(() => {
    let frame = 0;
    const update = () => {
      frame = 0;
      const visible = headings.map(heading => ({...heading, element:document.getElementById(heading.id)})).filter(item => item.element?.getClientRects().length);
      const passed = visible.filter(item => item.element!.getBoundingClientRect().top <= 130);
      setCurrent((passed.at(-1) ?? visible[0])?.id ?? '');
    };
    const onScroll = () => { if (!frame) frame = requestAnimationFrame(update); };
    update(); addEventListener('scroll', onScroll, true);
    return () => { removeEventListener('scroll', onScroll, true); cancelAnimationFrame(frame); };
  }, [headings]);
  if (!headings.length) return null;
  return <DocsDisclosure kind="outline" title={t('docs.outline')}><nav aria-label={t('docs.outline')}>{headings.map(heading => <a key={heading.id} data-depth={heading.depth} aria-current={current === heading.id ? 'location' : undefined} href={href('docs',docParams(version,topic,heading.id))} onClick={() => { if (location.hash === href('docs',docParams(version,topic,heading.id))) scrollToHeading(heading.id); }}>{heading.text}</a>)}</nav></DocsDisclosure>;
}

function DocsDisclosure({ kind, title, children }: { kind:'navigation'|'outline'; title:string; children:React.ReactNode }) {
  const [open, setOpen] = useState(() => !matchMedia('(max-width: 767px)').matches);
  useEffect(() => { const media = matchMedia('(max-width: 767px)'); const resize = () => setOpen(!media.matches); media.addEventListener('change',resize); return () => media.removeEventListener('change',resize); }, []);
  const Icon = kind === 'navigation' ? Menu : List;
  return <details className={`docs-disclosure docs-${kind}-disclosure`} open={open} onToggle={event => setOpen(event.currentTarget.open)}><summary onClick={event=>{if(!matchMedia('(max-width: 767px)').matches)event.preventDefault();}}><Icon size={15}/><span>{title}</span><ChevronDown size={14}/></summary><div className="docs-disclosure-body">{children}</div></details>;
}

export function DocsView({ params, versions = archivedVersions, defaultVersion = releasedVersion, active = true }: { params: URLSearchParams; versions?: DocVersion[]; defaultVersion?: DocVersion; active?: boolean }) {
  const { t, href, navigate } = useSite();
  const [notice, toast] = useState('');
  const search = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState('');
  const [sourceOpen, setSourceOpen] = useState(false);
  const version = params.has('version') ? versions.find(item => item.id === params.get('version')) : defaultVersion;
  const latestDocVersion = defaultVersion;
  const aliases: Record<string, string> = { gettingStarted: 'getting-started', investigation: 'dashboard', replay: 'replay-encoding-v2', releases: 'recipes/sourcemap-upload', permissions: 'teams', push: 'dashboard' };
  const requested = params.get('topic') ?? '', topic = aliases[requested] ?? requested;
  const registry = topic === '_versions', changes = topic === '_changes';
  const page = useMemo(() => version ? changes ? { slug: '_changes', description: undefined, title: `${version.id} · ${t('docs.changes')}`, markdown: version.changes } : getVersionPage(version, topic) : undefined, [version, topic, changes, t]);
  const headings = useMemo(() => page ? renderArticle(page, version?.id, {omitTitle:true,href}).headings : [], [page, version, href]);
  useEffect(() => {
    const previousTitle = document.title;
    const meta = document.querySelector('meta[name="description"]');
    const previousDescription = meta?.getAttribute('content');
    document.title = `${page?.title ?? t('nav.docs')} · Sentori`;
    if (meta && page?.description) meta.setAttribute('content',page.description);
    return () => { document.title = previousTitle; if (meta && previousDescription != null) meta.setAttribute('content',previousDescription); };
  }, [page, t]);
  const anchor = params.get('anchor');
  useEffect(() => { if (anchor) { const frame = requestAnimationFrame(() => scrollToHeading(anchor)); return () => cancelAnimationFrame(frame); } }, [anchor, topic, version]);
  const documentKey = `${topic}/${version?.id}`;
  const [previousDocument, setPreviousDocument] = useState(documentKey);
  if (previousDocument !== documentKey) { setPreviousDocument(documentKey); setQuery(''); setSourceOpen(false); toast(''); }
  // The shortcut must be available when the article first becomes visible.
  useLayoutEffect(() => {
    const onKey = (event:KeyboardEvent) => {
      if (event.key !== '/' || event.ctrlKey || event.metaKey || event.altKey || (event.target as HTMLElement)?.closest('input,textarea,select,[contenteditable=true]')) return;
      event.preventDefault(); search.current?.focus();
    };
    addEventListener('keydown',onKey); return () => removeEventListener('keydown',onKey);
  }, []);
  if (!version) return <><StatePanel title={t('docs.versionUnknown')} action={<Button onClick={() => navigate('docs',docParams(latestDocVersion.id,topic))}>{t('docs.switchLatest')}</Button>}/></>;

  const path = (slug = '', id = version.id) => href('docs',docParams(id,slug));
  const listed = SECTIONS.flatMap(section => section.slugs);
  const other = version.pages.filter(page => page.slug && !listed.includes(page.slug)).map(page => page.slug);
  const sections = [...SECTIONS,{key:'docs.section.more',slugs:other}].map(section => ({...section, pages:section.slugs.map(slug => getVersionPage(version,slug)).filter(page => !!page)})).filter(section => section.pages.length);
  const pages = [getVersionPage(version,''),...sections.flatMap(section => section.pages)].filter(page => !!page);
  const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  const matches = pages.filter(page => words.every(word => `${page.title} ${page.markdown}`.toLowerCase().includes(word)));
  const index = pages.findIndex(page => page.slug === topic), previous = pages[index - 1], next = pages[index + 1];
  const navLink = (item: NonNullable<ReturnType<typeof getVersionPage>>) => <a key={item.slug} href={path(item.slug)} aria-current={item.slug === topic ? 'page' : undefined} title={item.title}>{item.title}</a>;
  const sourcePath = changes ? 'CHANGELOG.md' : `docs/${topic || 'README'}.md`;
  return <>
    <div className="docs-topline"><span><BookOpen size={17}/>{t('nav.docs')}</span><label className="docs-version-select"><span>{t('docs.version')}</span><select className="control" aria-label={t('docs.version')} value={version.id} onChange={event => navigate('docs',docParams(event.target.value,topic))}>{versions.map(item => <option key={item.id} value={item.id}>{item.id} · {t(item.channel === 'current' ? 'docs.workingCopy' : item.channel === 'latest' ? 'docs.latest' : 'docs.archive')}</option>)}</select></label><div className="docs-search-area"><label className="docs-search"><Search size={15}/><input className="control" ref={search} type="search" aria-label={t('site.docsSearch')} placeholder={t('site.docsSearch')} value={query} onChange={event => setQuery(event.target.value)} onKeyDown={event => { if (event.key === 'Escape') { setQuery(''); search.current?.blur(); } if (event.key === 'Enter' && words.length && matches[0]) { setQuery(''); search.current?.blur(); navigate('docs',docParams(version.id,matches[0].slug)); } }}/><kbd aria-hidden="true">/</kbd></label>{words.length > 0 && <div className="docs-search-results" role="region" aria-label={t('docs.searchResults')}><div className="docs-search-count" role="status">{t('docs.resultCount',{count:matches.length})}</div>{matches.map(item => <a key={item.slug} href={path(item.slug)} onClick={() => setQuery('')}><BookOpen size={14}/><span><strong>{item.title}</strong><small>{item.slug || 'README'}</small></span><ChevronRight size={13}/></a>)}{!matches.length && <p>{t('common.noResults')}</p>}</div>}</div></div>
    {version.channel === 'archive' && <div className="docs-archive-notice"><History size={14}/><span>{t('docs.archiveNote',{version:version.id})}</span><a href={path(topic,latestDocVersion.id)}>{t('docs.switchLatest')}<ArrowRight size={13}/></a></div>}
    <DocumentationLayout
      navigation={<DocsDisclosure kind="navigation" title={t('docs.browse')}><a className="docs-home-link" href={path()} aria-current={!topic ? 'page' : undefined}><BookOpen size={14}/>{t('site.docsHome')}</a><div className="docs-version-links"><a href={path('_versions')} aria-current={registry ? 'page' : undefined}><GitBranch size={14}/>{t('docs.versions')}</a><a href={path('_changes')} aria-current={changes ? 'page' : undefined}><History size={14}/>{t('docs.changes')}</a></div>{sections.map(section => <div className="docs-nav-group" key={section.key}><h3>{t(section.key)}</h3>{section.pages.map(navLink)}</div>)}</DocsDisclosure>}
      article={registry ? <VersionRegistry selected={version} versions={versions}/> : page ? <>
        <nav className="docs-breadcrumb" aria-label={t('docs.breadcrumb')}><a href={path()}>{t('nav.docs')}</a><ChevronRight size={12}/><span>{version.id}</span><ChevronRight size={12}/><span>{topic ? page.title : t('site.docsHome')}</span></nav>
        <header className="docs-article-header"><h1>{topic ? page.title : t('docs.title')}</h1><div className="docs-article-meta"><Badge>{version.id}</Badge><span>{t('docs.releaseDate')}: {version.releasedAt ? <time dateTime={version.releasedAt}>{version.releasedAt}</time> : t("docs.unreleased")}</span><a href={path('_versions')}>{t(`docs.support.${version.support}`)}</a></div><div className="docs-article-actions"><Button small variant="ghost" onClick={async () => { try { await navigator.clipboard.writeText(new URL(href('docs',docParams(version.id,topic,anchor ?? undefined)),location.href).href); toast(t('common.copied')); } catch { toast(t('common.copyFailed')); } }}><Link size={13}/>{t('docs.copyLink')}</Button><Button small variant="ghost" onClick={() => setSourceOpen(true)}><FileCode size={13}/>{t('docs.viewSource')}</Button><Button small variant="ghost" onClick={() => downloadText(`${topic.replaceAll('/','-') || 'README'}-${version.id}.md`,page.markdown,'text/markdown')}><BookOpen size={13}/>{t('docs.downloadMarkdown')}</Button></div></header>
        {!topic && <section className="docs-welcome"><p>{t('docs.intro')}</p><div className="docs-start-grid">{([{Icon:Rocket,key:'Quick',topic:'getting-started'},{Icon:Compass,key:'Investigate',topic:'dashboard'},{Icon:Server,key:'Deploy',topic:'self-hosting'},{Icon:Braces,key:'Reference',topic:'protocol'}] as const).filter(item => getVersionPage(version,item.topic)).map(({Icon,key,topic}) => <a key={key} href={path(topic)}><Icon size={19}/><div><strong>{t(`site.docs${key}`)}</strong><span>{t(`site.docs${key}Hint`)}</span></div><ArrowRight size={13}/></a>)}</div></section>}
        {changes ? <p className="docs-language-note"><GitBranch size={13}/>{version.channel === 'current' ? t('docs.workingCopy') : <>{t('docs.sourceTag')}: <code>{version.id}</code> · <code>{version.commit.slice(0,8)}</code></>}</p> : <div className="docs-language-note"><BookOpen size={13}/><span>{t('site.docsLanguage')}</span></div>}
        <DocArticle page={page} version={version.id} lang={changes ? 'zh-CN' : 'en'}/>
        {!changes && <nav className="docs-page-pagination" aria-label={t('nav.docs')}>{previous && <a href={path(previous.slug)}><small>{t('site.docsPrevious')}</small><strong>{previous.title}</strong></a>}{next && <a href={path(next.slug)}><small>{t('site.docsNext')}</small><strong>{next.title}</strong></a>}</nav>}
      </> : <StatePanel title={t('docs.versionMissing',{version:version.id})} description={t('docs.versionMissingHint')} action={<Button onClick={() => navigate('docs',docParams(latestDocVersion.id,topic))}>{t('docs.switchLatest')}</Button>}/>}
      outline={<DocsOutline headings={headings} version={version.id} topic={topic}/>}
    />
    <div className="toast-stack" role="status" aria-live="polite">{notice && <div className="toast"><span className="flex-1">{notice}</span><Button small variant="ghost" className="button-icon" aria-label={t("common.close")} onClick={() => toast('')}><X size={16}/></Button></div>}</div>
    <Dialog closeLabel={t("common.close")} active={active} open={sourceOpen && !!page} onClose={() => setSourceOpen(false)} title={`${version.id} · ${sourcePath}`} footer={<Button onClick={() => setSourceOpen(false)}>{t("common.close")}</Button>}>{page && <Code t={t} language="markdown" fileName={sourcePath} code={page.markdown}/>}</Dialog>
  </>;
}
