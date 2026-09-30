// `sentori-cli upload srcbundle` — native source context without
// repository access.
//
// Walks the given directories for native source files, packs them
// as one JSON object (project-relative path → content), and lands
// it on the artifacts endpoint as kind `srcbundle`. The server
// reads it when a dSYM/proguard frame resolves to file+line, so
// the dashboard shows the failing native code the same way a
// sourcemap's embedded sourcesContent covers JS. The build owns
// exactly what goes in — nothing is pulled from git, ever.

import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'

/** Native + shared-code extensions worth carrying. JS/TS ride the
 *  sourcemap already; including them anyway is harmless but bulky,
 *  so they stay out by default. */
const SOURCE_EXTS = new Set(['.swift', '.kt', '.kts', '.java', '.m', '.mm', '.h', '.c', '.cc', '.cpp', '.hpp'])

const SKIP_DIRS = new Set(['node_modules', '.git', 'build', 'Pods', 'DerivedData', '.gradle', 'dist'])

/** A single source file larger than this is generated, not written. */
const MAX_FILE_BYTES = 512 * 1024
/** Total budget before compression; the wire is gzip anyway. */
const MAX_TOTAL_BYTES = 64 * 1024 * 1024

export type SrcBundleStats = { files: number; bytes: number; skipped: number }

/** Collect `{ relPath: content }` from the roots. */
export function collectSources(roots: string[]): { bundle: Record<string, string>; stats: SrcBundleStats } {
  const bundle: Record<string, string> = {}
  const stats: SrcBundleStats = { files: 0, bytes: 0, skipped: 0 }

  const walk = (root: string, dir: string): void => {
    let entries
    try {
      entries = readdirSync(dir, { withFileTypes: true })
    } catch {
      return
    }
    for (const e of entries) {
      if (e.isDirectory()) {
        if (!SKIP_DIRS.has(e.name) && !e.name.startsWith('.')) walk(root, join(dir, e.name))
        continue
      }
      const dot = e.name.lastIndexOf('.')
      if (dot < 0 || !SOURCE_EXTS.has(e.name.slice(dot))) continue
      const full = join(dir, e.name)
      let size
      try {
        size = statSync(full).size
      } catch {
        continue
      }
      if (size > MAX_FILE_BYTES || stats.bytes + size > MAX_TOTAL_BYTES) {
        stats.skipped += 1
        continue
      }
      try {
        bundle[relative(root, full)] = readFileSync(full, 'utf8')
        stats.files += 1
        stats.bytes += size
      } catch {
        stats.skipped += 1
      }
    }
  }

  for (const root of roots) walk(root, root)
  return { bundle, stats }
}
