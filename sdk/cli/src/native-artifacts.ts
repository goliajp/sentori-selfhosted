// `sentori-cli upload dsym` (iOS) + `sentori-cli upload mapping` (Android).
// Both endpoints take the raw artifact bytes (NOT multipart) with a few
// headers / query params.

import { spawnSync } from 'node:child_process'
import { readFile } from 'node:fs/promises'
import { basename, extname, join } from 'node:path'
import { statSync, readdirSync } from 'node:fs'

export type AdminUpload = {
  apiUrl: string
  projectId: string
  release?: string
  token: string
}

async function postBytes(
  url: string,
  body: Buffer,
  token: string,
  headers: Record<string, string> = {},
): Promise<unknown> {
  const resp = await fetch(url, {
    body,
    headers: {
      Authorization: `Bearer ${token}`,
      'Content-Type': 'application/octet-stream',
      ...headers,
    },
    method: 'POST',
  })
  if (!resp.ok) {
    let detail = ''
    try {
      detail = await resp.text()
    } catch {
      // ignore
    }
    throw new Error(
      `${resp.status} ${resp.statusText}${detail ? ` — ${detail.slice(0, 300)}` : ''}`,
    )
  }
  const txt = await resp.text()
  return txt ? JSON.parse(txt) : null
}

// ── dSYM ──────────────────────────────────────────────────────────

export type DsymSlice = { arch: string; debugId: string; file: string }

const ARCHES = new Set([
  'arm64',
  'arm64_32',
  'arm64e',
  'armv7',
  'armv7k',
  'armv7s',
  'i386',
  'x86_64',
  'x86_64h',
])

/**
 * Use `dwarfdump --uuid <path>` to enumerate `(arch, debug_id, file)`
 * for each Mach-O slice. Returns [] if dwarfdump isn't installed or the
 * output couldn't be parsed; callers should fall back to explicit
 * `--debug-id` / `--arch` flags in that case.
 */
export function dsymSlicesFromDwarfdump(path: string): DsymSlice[] {
  const r = spawnSync('dwarfdump', ['--uuid', path])
  if (r.status !== 0 || !r.stdout) return []
  const out: DsymSlice[] = []
  // Output lines look like:
  //   UUID: 1234ABCD-... (arm64) /path/to/Foo.dSYM/Contents/Resources/DWARF/Foo
  const re = /^UUID:\s+([0-9A-Fa-f-]{32,36})\s+\(([^)]+)\)\s+(.+)\s*$/m
  for (const line of r.stdout.toString().split('\n')) {
    const m = re.exec(line)
    if (!m) continue
    const [, debugId, arch, file] = m as unknown as [string, string, string, string]
    if (!ARCHES.has(arch)) continue
    out.push({ arch, debugId: debugId.toUpperCase(), file: file.trim() })
  }
  return out
}

/** Walk a `.dSYM` bundle and return the DWARF binary files inside
 *  `Contents/Resources/DWARF/`. If `path` already points at a binary
 *  (not a bundle), returns `[path]`. */
export function dwarfBinariesIn(path: string): string[] {
  let st
  try {
    st = statSync(path)
  } catch {
    return []
  }
  if (st.isFile()) return [path]
  const dwarfDir = join(path, 'Contents/Resources/DWARF')
  try {
    return readdirSync(dwarfDir)
      .filter((n) => !n.startsWith('.'))
      .map((n) => join(dwarfDir, n))
  } catch {
    // not a .dSYM bundle layout — try the top-level path
    return [path]
  }
}

export type DsymUploadOptions = AdminUpload & {
  /** Explicit overrides when dwarfdump isn't available. */
  arch?: string
  debugId?: string
  /** A `Foo.dSYM` bundle or a raw DWARF binary. */
  path: string
  objectName?: string
}

export type DsymUploadResult = { slices: { arch: string; debugId: string }[] }

export async function uploadDsym(opts: DsymUploadOptions): Promise<DsymUploadResult> {
  const slices: DsymSlice[] = []
  if (opts.debugId && opts.arch) {
    // Explicit single-slice upload — no parsing needed.
    const binaries = dwarfBinariesIn(opts.path)
    if (binaries.length === 0) throw new Error(`no DWARF binary at: ${opts.path}`)
    slices.push({ arch: opts.arch, debugId: opts.debugId.toUpperCase(), file: binaries[0]! })
  } else {
    // Auto-discover via dwarfdump.
    const found = dsymSlicesFromDwarfdump(opts.path)
    if (found.length === 0) {
      throw new Error(
        'couldn’t enumerate dSYM slices — install Xcode command-line tools ' +
          '(for `dwarfdump`), or pass --debug-id and --arch explicitly',
      )
    }
    slices.push(...found)
  }

  const base = opts.apiUrl.replace(/\/+$/, '')
  const q = new URLSearchParams()
  if (opts.release) q.set('release', opts.release)
  if (opts.objectName ?? basename(opts.path).replace(/\.dSYM$/, ''))
    q.set('objectName', opts.objectName ?? basename(opts.path).replace(/\.dSYM$/, ''))
  const qs = q.toString()
  const url = `${base}/admin/api/projects/${encodeURIComponent(opts.projectId)}/dsyms${qs ? '?' + qs : ''}`

  const uploaded: { arch: string; debugId: string }[] = []
  for (const s of slices) {
    const body = await readFile(s.file)
    await postBytes(url, body, opts.token, {
      'x-sentori-arch': s.arch,
      'x-sentori-debug-id': s.debugId,
    })
    uploaded.push({ arch: s.arch, debugId: s.debugId })
  }
  return { slices: uploaded }
}

// ── ProGuard / R8 mapping ─────────────────────────────────────────

export type MappingUploadOptions = AdminUpload & {
  debugId?: string
  path: string
}

export async function uploadMapping(opts: MappingUploadOptions): Promise<void> {
  const ext = extname(opts.path).toLowerCase()
  if (ext && ext !== '.txt' && ext !== '.map') {
    // R8 emits `mapping.txt` by default; accept anything but warn.
    console.warn(`[sentori-cli] upload mapping: unexpected extension ${ext} — uploading anyway`)
  }
  const body = await readFile(opts.path)
  if (body.length === 0) throw new Error(`empty mapping file: ${opts.path}`)

  const base = opts.apiUrl.replace(/\/+$/, '')
  const q = new URLSearchParams()
  if (opts.release) q.set('release', opts.release)
  const qs = q.toString()
  const url = `${base}/admin/api/projects/${encodeURIComponent(opts.projectId)}/mappings${qs ? '?' + qs : ''}`
  const headers: Record<string, string> = {}
  if (opts.debugId) headers['x-sentori-debug-id'] = opts.debugId
  await postBytes(url, body, opts.token, headers)
}
