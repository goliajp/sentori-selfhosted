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

import type { DocPage } from './types';
export type { DocPage } from './types';

const RAW = import.meta.glob('../../../docs/**/*.md', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>

/** `docs/getting-started/web.md` → `getting-started/web` */
function slugOf(path: string): string {
  return path.replace(/^.*\/docs\//, '').replace(/\.md$/, '')
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

export function allDocPages(): DocPage[] { return [...pages.values()] }

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

