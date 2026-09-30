// The documentation site renders every page the repository has.
//
// `sentori.golia.jp/docs` answered 200 for a year and served the
// dashboard shell: the SPA had no /docs route and its catch-all sent
// the visitor to the issue list. Someone following a link from npm
// landed in a stranger's console.
//
// Now that it renders, two things can silently go wrong, and both did
// within an hour of it existing:
//
//   · a page exists and no section lists it, so it is in the
//     repository, linked from the index, and missing from the site's
//     navigation;
//   · a slug in a section names a page that was renamed or removed, so
//     the sidebar has a dead entry.
//
// It also refuses frontmatter leaking into the body. Seven pages carry
// the YAML of a static-site generator removed in 2026-08, and the
// first page served put `title: … description: …` on screen as a
// heading.
//
//   node devtools/check-docs-site.mjs

import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'

const DOCS = new URL('../../docs/', import.meta.url).pathname
const SRC = new URL('../src/lib/docs.ts', import.meta.url).pathname

if (!existsSync(DOCS)) {
  console.error('✗ no docs/ next to webapp. This checker is reading nothing.')
  process.exit(1)
}

const slugs = []
const walk = (d) => {
  for (const e of readdirSync(join(DOCS, d))) {
    const rel = d ? `${d}/${e}` : e
    if (statSync(join(DOCS, rel)).isDirectory()) walk(rel)
    else if (e.endsWith('.md')) slugs.push(rel.replace(/\.md$/, ''))
  }
}
walk('')

if (slugs.length < 10) {
  console.error(`✗ found ${slugs.length} docs pages. Broken checker, not a broken tree.`)
  process.exit(1)
}

const source = readFileSync(SRC, 'utf8')
const sectionBlock = /export const SECTIONS[\s\S]*?\n\]/.exec(source)
if (!sectionBlock) {
  console.error('✗ SECTIONS has moved in src/lib/docs.ts — this checker now reads nothing.')
  process.exit(1)
}
const listed = [...sectionBlock[0].matchAll(/'([\w/-]+)'/g)]
  .map((m) => m[1])
  .filter((s) => !s.startsWith('docs.section.'))

const problems = []

for (const slug of listed) {
  if (!slugs.includes(slug)) {
    problems.push(`the sidebar lists '${slug}' and docs/${slug}.md does not exist`)
  }
}
for (const slug of slugs) {
  if (slug === 'README') continue
  if (!listed.includes(slug)) {
    problems.push(`docs/${slug}.md is in no section — it opens by URL and is missing from the sidebar`)
  }
}

// Frontmatter is allowed in the file; it must not reach the body. The
// loader splits it, and this asserts the split still happens.
if (!/splitFrontmatter/.test(source)) {
  problems.push('src/lib/docs.ts no longer splits frontmatter — it will render as a heading')
}

if (problems.length > 0) {
  console.error(`✗ ${problems.length} docs-site problem(s):`)
  for (const p of problems) console.error(`    ${p}`)
  process.exit(1)
}

console.log(`✓ ${slugs.length - 1} docs pages, every one of them in the site's navigation`)
