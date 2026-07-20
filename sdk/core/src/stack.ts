import type { Frame } from './types.js'

/**
 * Cross-engine `Error.stack` parser.
 *
 * Handles two predominant formats:
 *   - V8 / Node / Bun / Hermes 0.71+:
 *       "    at fn (file:line:col)"
 *       "    at fn file:line:col"
 *       "    at file:line:col"
 *     File can be a URL (https://example.com/path) — the regex anchors
 *     on the trailing `:line:col` so the URL's leading colon doesn't
 *     confuse the file boundary.
 *   - SpiderMonkey / Safari / older Hermes:
 *       "fn@file:line:col"
 *
 * Hermes (React Native's default engine) emits bytecode frames as
 * `at fn (address at /path/index.android.bundle:1:289430)` — the
 * `address at ` marker is stripped so `file` is the clean bundle path
 * the server needs to look up a source map. `(native)` frames have no
 * location and don't match either regex (dropped).
 *
 * Frames marked `inApp = false` for paths that look like vendor / node
 * stdlib / remote scripts — `node_modules`, `node:` scheme, http(s) URLs.
 *
 * Returns `[]` for blank / non-string input. Lines that don't match
 * either format are silently dropped (typically the leading
 * "ErrorType: message" header on V8 stacks).
 */

const V8_RE = /^\s*at\s+(?:(?<fn>.+?)\s+)?\(?(?<file>.+?):(?<line>\d+):(?<col>\d+)\)?\s*$/
const SPIDER_RE = /^(?:(?<fn>[^@]*)@)?(?<file>.+?):(?<line>\d+):(?<col>\d+)\s*$/
const HERMES_ADDRESS_PREFIX = /^address at +/

export type ParseStackOptions = {
  /** Strip protocol + parent path so dashboard shows short filenames. */
  shortFilenames?: boolean
}

export function parseStack(
  stack: string | undefined,
  opts: ParseStackOptions = {},
): Frame[] {
  if (!stack || typeof stack !== 'string') return []
  const out: Frame[] = []
  for (const raw of stack.split('\n')) {
    const line = raw.trim()
    if (!line) continue
    const m = V8_RE.exec(line) ?? SPIDER_RE.exec(line)
    if (!m?.groups) continue
    const file = (m.groups.file ?? '<anonymous>').replace(HERMES_ADDRESS_PREFIX, '')
    out.push({
      absolutePath: file,
      column: Number(m.groups.col),
      file: opts.shortFilenames ? shortFile(file) : file,
      function: m.groups.fn?.trim() || undefined,
      inApp: isInApp(file),
      line: Number(m.groups.line),
    })
  }
  return out
}

function isInApp(file: string): boolean {
  if (!file || file === '<anonymous>') return false
  if (file.includes('node_modules/')) return false
  if (file.startsWith('node:')) return false
  if (/^https?:\/\//.test(file)) return false
  return true
}

function shortFile(absolute: string): string {
  const noProto = absolute.replace(/^https?:\/\/[^/]+\//, '')
  const tail = noProto.split('/').slice(-2).join('/')
  return tail || absolute
}
