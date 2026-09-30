// The documentation, read out of `docs/` at build time.
//
// One copy of these pages, not two. The repository's `docs/` is what
// the public mirror ships and what every gate already checks — link
// resolution, reachability from the index, that the APIs named exist,
// that the install lines install something. A second copy written for
// the site would drift from all of it within a release, and the
// drifting one is the one strangers read.
//
// `sentori.golia.jp/docs` answered 200 before this existed: the SPA
// fallback served the dashboard shell and the router sent the visitor
// to the issue list. A reader following a link from npm or GitHub
// landed in someone else's console.

import { marked } from 'marked'

import { highlightBlock, languageForPath } from './highlight'

const RAW = import.meta.glob('../../../docs/**/*.md', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>

/** `docs/getting-started/web.md` → `getting-started/web` */
function slugOf(path: string): string {
  return path.replace(/^.*\/docs\//, '').replace(/\.md$/, '')
}

export type DocPage = {
  /** Path under /docs, without extension. The index is ''. */
  slug: string
  title: string
  description?: string
  markdown: string
}

/** Seven pages still carry the YAML frontmatter of a static-site
 *  generator this repository removed in 2026-08. Nothing rendered
 *  them until now, so nothing minded; the first page served put
 *  `title: Getting started — browser description: …` on screen as a
 *  heading and in the table of contents. The keys are worth keeping —
 *  they are a better page title and a real meta description — so they
 *  are read here rather than deleted from the files. */
function splitFrontmatter(text: string): { meta: Record<string, string>; body: string } {
  if (!text.startsWith('---\n')) return { meta: {}, body: text }
  const end = text.indexOf('\n---', 4)
  if (end === -1) return { meta: {}, body: text }
  const meta: Record<string, string> = {}
  for (const line of text.slice(4, end).split('\n')) {
    const m = /^([a-zA-Z_]+):\s*(.*)$/.exec(line)
    if (m) meta[m[1]] = m[2].trim().replace(/^["']|["']$/g, '')
  }
  return { meta, body: text.slice(end + 4).replace(/^\n+/, '') }
}

const pages = new Map<string, DocPage>()
for (const [path, raw] of Object.entries(RAW)) {
  const slug = slugOf(path)
  const { meta, body } = splitFrontmatter(raw)
  const heading = /^#\s+(.+)$/m.exec(body)
  pages.set(slug === 'README' ? '' : slug, {
    slug: slug === 'README' ? '' : slug,
    title: meta.title ?? (heading ? heading[1].trim() : slug),
    description: meta.description,
    markdown: body,
  })
}

export function docPage(slug: string): DocPage | undefined {
  return pages.get(slug)
}

export function docCount(): number {
  return pages.size
}

/** The order the index presents them in, with its own section names.
 *
 *  Taken from `docs/README.md` rather than invented: that file is the
 *  index the project maintains, and `check-doc-reachable` already
 *  requires every page to be reachable from it. A page added to the
 *  repository and not to this list still opens by URL and still shows
 *  up in search; it is only missing from the sidebar, which the
 *  navigation check below reports. */
export const SECTIONS: { key: string; slugs: string[] }[] = [
  {
    key: 'docs.section.start',
    slugs: [
      'getting-started',
      'getting-started/react-native',
      'getting-started/web',
      'getting-started/weapp',
      'dashboard',
    ],
  },
  { key: 'docs.section.sdk', slugs: ['sdk-swift', 'sdk-kotlin'] },
  { key: 'docs.section.wire', slugs: ['protocol', 'errors', 'replay-encoding-v2'] },
  { key: 'docs.section.run', slugs: ['self-hosting', 'teams', 'runbook/scaling', 'runbook/cli-auth'] },
  { key: 'docs.section.recipes', slugs: ['recipes/sourcemap-upload', 'recipes/release-versioning'] },
  { key: 'docs.section.help', slugs: ['troubleshooting'] },
]

/** Pages that exist and no section lists. Rendered at the end of the
 *  sidebar rather than hidden: a page nobody can navigate to is the
 *  defect `check-doc-reachable` exists for, and hiding it here would
 *  reintroduce it one level down. */
export function unlisted(): DocPage[] {
  const listed = new Set(SECTIONS.flatMap((s) => s.slugs))
  return [...pages.values()].filter((p) => p.slug !== '' && !listed.has(p.slug))
}

/** A link between two docs pages has to keep working on the site.
 *
 *  In the repository `[errors](errors.md)` resolves relative to the
 *  file. On the site the same link has to become `/docs/errors`, and a
 *  link that leaves `docs/` — `../sdk/web/README.md` — has to go to
 *  GitHub, where a reader can actually open it. */
function rewriteLink(href: string, fromSlug: string): string {
  if (/^[a-z]+:/i.test(href) || href.startsWith('#')) return href
  const dir = fromSlug.includes('/') ? fromSlug.replace(/\/[^/]*$/, '') : ''
  const parts = (dir ? `${dir}/${href}` : href).split('/')
  const out: string[] = []
  for (const part of parts) {
    if (part === '.' || part === '') continue
    if (part === '..') out.pop()
    else out.push(part)
  }
  const joined = out.join('/')
  // Anything that climbed out of docs/ is not a docs page.
  if (parts.includes('..') && out.length > 0 && !pages.has(joined.replace(/\.md(#.*)?$/, ''))) {
    return `https://github.com/goliajp/sentori-selfhosted/blob/master/${joined}`
  }
  const [path, hash] = joined.split('#')
  const slug = path.replace(/\.md$/, '')
  return `/docs/${slug === 'README' ? '' : slug}${hash ? `#${hash}` : ''}`
}

/** Heading ids, so the in-page table of contents can link to them. */
export function headingId(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^\w\s-]/g, '')
    .trim()
    .replace(/\s+/g, '-')
}

export type Heading = { depth: number; text: string; id: string }

export function renderDoc(page: DocPage): { html: string; headings: Heading[] } {
  const headings: Heading[] = []
  const renderer = new marked.Renderer()

  renderer.heading = ({ text, depth }) => {
    const plain = text.replace(/<[^>]+>/g, '')
    const id = headingId(plain)
    if (depth >= 2 && depth <= 3) headings.push({ depth, text: plain, id })
    return `<h${depth} id="${id}">${text}</h${depth}>`
  }

  // The dashboard's own highlighter, so a snippet here and a stack
  // frame on an issue page are colored by one implementation.
  renderer.code = ({ text, lang }) => {
    const language = lang ? languageForPath(`x.${lang}`) ?? lang : undefined
    return `<pre><code>${highlightBlock(text, language)}</code></pre>`
  }

  renderer.link = function link({ href, title, tokens }) {
    const target = rewriteLink(href ?? '', page.slug)
    const external = /^https?:/.test(target)
    // The label's own tokens, parsed. Taking `raw` instead left the
    // backticks of an inline-code label on screen: three SDK links on
    // the front page read `` `@goliapkg/sentori-web` `` with the
    // backticks visible.
    const label = this.parser.parseInline(tokens)
    return `<a href="${target}"${title ? ` title="${title}"` : ''}${
      external ? ' target="_blank" rel="noreferrer"' : ''
    }>${label}</a>`
  }

  const html = marked.parse(page.markdown, { renderer, async: false }) as string
  return { html, headings }
}
