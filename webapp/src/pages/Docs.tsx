// The documentation site, at /docs.
//
// Outside the authenticated shell: a stranger following a link from
// npm or GitHub has no account here, and before this page existed the
// SPA fallback answered 200 and the router sent them to someone
// else's issue list.
//
// The pages are the repository's own `docs/*.md`, read at build time.
// One copy, checked by the gates that already check it.

import { ArrowUpRight, Menu, X } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { Link, useLocation, useParams } from 'react-router-dom'

import { SentoriMark } from '../components/brand'
import { ThemeSwitch, clsx } from '../components/ui'
import { useT } from '../i18n'
import { SECTIONS, docCount, docPage, renderDoc, unlisted, type DocPage } from '../lib/docs'

function Sidebar({ current, onNavigate }: { current: string; onNavigate: () => void }) {
  const t = useT()
  const extra = unlisted()
  return (
    <nav className="flex flex-col gap-5 text-sm">
      {SECTIONS.map((section) => (
        <div key={section.key}>
          <div className="mb-1.5 px-2 text-xs font-semibold uppercase tracking-wide text-fg-subtle">
            {t(section.key as Parameters<typeof t>[0])}
          </div>
          {section.slugs.map((slug) => {
            const page = docPage(slug)
            if (!page) return null
            return (
              <Link
                key={slug}
                to={`/docs/${slug}`}
                onClick={onNavigate}
                className={clsx(
                  'block truncate rounded px-2 py-1 transition-colors',
                  slug === current
                    ? 'bg-raised font-medium text-fg'
                    : 'text-fg-muted hover:text-fg',
                )}
              >
                {page.title}
              </Link>
            )
          })}
        </div>
      ))}
      {extra.length > 0 && (
        <div>
          <div className="mb-1.5 px-2 text-xs font-semibold uppercase tracking-wide text-fg-subtle">
            {t('docs.section.more')}
          </div>
          {extra.map((page) => (
            <Link
              key={page.slug}
              to={`/docs/${page.slug}`}
              onClick={onNavigate}
              className={clsx(
                'block truncate rounded px-2 py-1 transition-colors',
                page.slug === current ? 'bg-raised font-medium text-fg' : 'text-fg-muted hover:text-fg',
              )}
            >
              {page.title}
            </Link>
          ))}
        </div>
      )}
    </nav>
  )
}

function NotFound({ slug }: { slug: string }) {
  const t = useT()
  return (
    <div className="py-16">
      <h1 className="text-xl font-semibold text-fg">{t('docs.missing')}</h1>
      <p className="mt-2 text-sm text-fg-muted">
        <code className="rounded bg-raised px-1 py-0.5 font-mono text-xs">/docs/{slug}</code>
      </p>
      <Link to="/docs" className="mt-4 inline-block text-sm text-accent hover:underline">
        {t('docs.backToIndex')}
      </Link>
    </div>
  )
}

function Article({ page }: { page: DocPage }) {
  const t = useT()
  const { html, headings } = useMemo(() => renderDoc(page), [page])
  const { hash } = useLocation()

  // A link with a fragment has to land on the heading, and the content
  // is injected rather than rendered as elements, so the browser's own
  // scroll-to-anchor has nothing to find on the first paint.
  useEffect(() => {
    if (!hash) {
      window.scrollTo(0, 0)
      return
    }
    const el = document.getElementById(hash.slice(1))
    if (el) el.scrollIntoView()
  }, [hash, page.slug])

  return (
    <div className="flex min-w-0 flex-1 gap-10">
      <article
        className="sn-doc min-w-0 flex-1 pb-24"
        // The pages are this repository's own files, compiled in at
        // build time. There is no user input on this path.
        dangerouslySetInnerHTML={{ __html: html }}
      />
      {headings.length > 2 && (
        <aside className="hidden w-52 shrink-0 xl:block">
          <div className="sticky top-8 border-l border-border pl-4 text-sm">
            <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-fg-subtle">
              {t('docs.onThisPage')}
            </div>
            {headings.map((h) => (
              <a
                key={h.id}
                href={`#${h.id}`}
                className={clsx(
                  'block truncate py-0.5 text-fg-muted transition-colors hover:text-fg',
                  h.depth === 3 && 'pl-3',
                )}
              >
                {h.text}
              </a>
            ))}
          </div>
        </aside>
      )}
    </div>
  )
}

export default function Docs() {
  const t = useT()
  const params = useParams()
  const slug = params['*'] ?? ''
  const page = docPage(slug)
  const [navOpen, setNavOpen] = useState(false)

  useEffect(() => {
    document.title = page ? `${page.title} · Sentori` : 'Sentori'
    // The frontmatter these pages carry names a description; it is
    // what a search result and a link preview show, and it was being
    // rendered into the body instead.
    const meta = document.querySelector('meta[name="description"]')
    if (meta && page?.description) meta.setAttribute('content', page.description)
  }, [page])

  return (
    <div className="min-h-screen bg-canvas">
      <header className="sticky top-0 z-20 border-b border-border bg-canvas/95 backdrop-blur">
        <div className="mx-auto flex max-w-[1400px] items-center gap-3 px-4 py-3">
          <button
            type="button"
            onClick={() => setNavOpen((v) => !v)}
            aria-label={t('docs.toggleNav')}
            className="rounded p-1 text-fg-muted hover:text-fg lg:hidden"
          >
            {navOpen ? <X className="h-4 w-4" /> : <Menu className="h-4 w-4" />}
          </button>
          <Link to="/docs" className="flex items-center gap-2 text-sm font-semibold text-fg">
            <SentoriMark className="h-4 w-4" />
            sentori
          </Link>
          <span className="text-xs text-fg-subtle">{t('docs.title')}</span>
          <div className="ml-auto flex items-center gap-3">
            <a
              href="https://github.com/goliajp/sentori-selfhosted"
              target="_blank"
              rel="noreferrer"
              className="hidden items-center gap-1 text-xs text-fg-muted hover:text-fg sm:flex"
            >
              GitHub
              <ArrowUpRight className="h-3 w-3" />
            </a>
            <Link to="/" className="text-xs text-fg-muted hover:text-fg">
              {t('docs.openDashboard')}
            </Link>
            <ThemeSwitch />
          </div>
        </div>
      </header>

      <div className="mx-auto flex max-w-[1400px] gap-10 px-4 py-8">
        <div
          className={clsx(
            'w-56 shrink-0 lg:block',
            navOpen ? 'block' : 'hidden',
          )}
        >
          <div className="sticky top-20">
            <Sidebar current={slug} onNavigate={() => setNavOpen(false)} />
          </div>
        </div>
        {page ? <Article page={page} /> : <NotFound slug={slug} />}
      </div>

      <footer className="border-t border-border px-4 py-6 text-xs text-fg-subtle">
        <div className="mx-auto max-w-[1400px]">
          {t('docs.pageCount', { count: String(docCount()) })}
        </div>
      </footer>
    </div>
  )
}
