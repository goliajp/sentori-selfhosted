// F5 — check that every error `code` referenced by the server has a
// corresponding markdown doc at docs/errors/<code>.md.
//
// Greps the server source for two patterns:
//   err_response(<status>, "<code>", ...)
//   err_response_with(<status>, "<code>", ...)
//
// Reports any code that lacks a doc page. Exit non-zero on miss so
// CI fails. Also lists unused docs (a stub that no source refers to).
//
// Run with: `bun scripts/check-error-docs.ts`

import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

const SERVER_SRC = 'server/src'
const DOCS = 'docs/errors'

// Each line under one of these prefixes is a candidate; we extract
// the literal that immediately follows the StatusCode.
const CALL_PATTERNS = [
  /err_response(?:_with)?\s*\(\s*StatusCode::[A-Z_]+\s*,\s*"([^"]+)"/g,
]

function walk(dir: string, out: string[]) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) walk(path, out)
    else if (entry.isFile() && (entry.name.endsWith('.rs') || entry.name.endsWith('.kt')))
      out.push(path)
  }
}

function scanCodes(): Set<string> {
  const files: string[] = []
  walk(SERVER_SRC, files)
  const codes = new Set<string>()
  for (const f of files) {
    const text = readFileSync(f, 'utf8')
    for (const pat of CALL_PATTERNS) {
      pat.lastIndex = 0
      let m: null | RegExpExecArray
      while ((m = pat.exec(text)) !== null) {
        codes.add(m[1]!)
      }
    }
  }
  return codes
}

function existingDocs(): Set<string> {
  if (!existsSync(DOCS)) return new Set()
  const out = new Set<string>()
  for (const f of readdirSync(DOCS)) {
    if (f.endsWith('.md')) out.add(f.slice(0, -3))
  }
  return out
}

const stubFor = (code: string) => `# \`${code}\`

> Auto-generated stub. Replace this body with a 200-word explainer of:
> what the error means, why a caller saw it, and how to fix it.

## What this means

TODO.

## Why you got it

TODO.

## How to fix it

TODO.

---

*Edit this file under \`docs/errors/${code}.md\` to update the docs surface.*
`

function main() {
  const codes = scanCodes()
  const docs = existingDocs()

  const missing: string[] = []
  for (const code of codes) {
    if (!docs.has(code)) missing.push(code)
  }
  const orphan: string[] = []
  for (const doc of docs) {
    if (!codes.has(doc)) orphan.push(doc)
  }

  console.log(`scanned ${codes.size} error code(s) across server source`)
  console.log(`found ${docs.size} doc page(s) under ${DOCS}/`)

  if (missing.length === 0 && orphan.length === 0) {
    console.log('✓ every code has a doc, every doc has a code')
    return 0
  }

  if (process.argv.includes('--write-stubs')) {
    for (const code of missing) {
      const path = join(DOCS, `${code}.md`)
      writeFileSync(path, stubFor(code))
      console.log(`wrote stub: ${path}`)
    }
    return 0
  }

  if (missing.length > 0) {
    console.error(`\n✗ MISSING DOCS for ${missing.length} code(s):`)
    for (const c of missing) console.error(`    ${c}`)
    console.error(`\n  fix by running: bun scripts/check-error-docs.ts --write-stubs`)
    console.error(`  then editing each stub under ${DOCS}/<code>.md`)
  }
  if (orphan.length > 0) {
    console.warn(`\n⚠ orphaned docs (referenced by no source):`)
    for (const c of orphan) console.warn(`    ${c}`)
  }
  return missing.length > 0 ? 1 : 0
}

process.exit(main())
