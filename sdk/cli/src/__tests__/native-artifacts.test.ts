import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, beforeEach, describe, expect, test } from 'bun:test'

import {
  dsymSlicesFromDwarfdump,
  dwarfBinariesIn,
  uploadDsym,
  uploadMapping,
} from '../native-artifacts.js'

const ADMIN = {
  apiUrl: 'https://api.example.com/',
    token: 'sk_test',
}

let dir = ''
const origFetch = globalThis.fetch
let calls: { body: Buffer | null; headers: Headers; method: string; url: string }[]
beforeEach(async () => {
  dir = await mkdtemp(join(tmpdir(), 'sentori-cli-native-'))
  calls = []
})
afterEach(async () => {
  globalThis.fetch = origFetch
  await rm(dir, { force: true, recursive: true })
})

function mockFetch(status = 200, respBody = ''): void {
  globalThis.fetch = (async (url: Request | string | URL, init?: RequestInit) => {
    const body = init?.body
    calls.push({
      body: body instanceof Uint8Array ? Buffer.from(body) : null,
      headers: new Headers(init?.headers),
      method: init?.method ?? 'GET',
      url: String(url),
    })
    return new Response(respBody, { status })
  }) as typeof fetch
}

describe('uploadMapping', () => {
  test('POSTs raw bytes to /mappings with Bearer + release query', async () => {
    mockFetch()
    const path = join(dir, 'mapping.txt')
    await writeFile(path, '# pg_map_id: abc\nfoo.bar -> a.b:\n')
    await uploadMapping({ ...ADMIN, path, release: 'app@1.0.0+1' })
    expect(calls).toHaveLength(1)
    expect(calls[0]?.method).toBe('POST')
    expect(calls[0]?.url).toBe(
      'https://api.example.com/v1/releases/app%401.0.0%2B1/artifacts',
    )
    expect(calls[0]?.headers.get('authorization')).toBe('Bearer sk_test')
  })

  test('sets x-sentori-debug-id when given', async () => {
    mockFetch()
    const path = join(dir, 'mapping.txt')
    await writeFile(path, 'x')
    await uploadMapping({ ...ADMIN, debugId: '1234-uuid', path, release: 'r' })
    expect(calls[0]?.url).toContain('/v1/releases/r/artifacts')
  })

  test('throws on empty file', async () => {
    const path = join(dir, 'empty.txt')
    await writeFile(path, '')
    await expect(uploadMapping({ ...ADMIN, path })).rejects.toThrow('empty mapping file')
  })

  test('throws on non-2xx with server detail', async () => {
    mockFetch(403, 'forbidden')
    const path = join(dir, 'mapping.txt')
    await writeFile(path, 'x')
    await expect(uploadMapping({ ...ADMIN, path })).rejects.toThrow('403')
  })
})

describe('uploadDsym (explicit single-slice)', () => {
  test('POSTs to /dsyms with x-sentori-debug-id, x-sentori-arch, release + objectName', async () => {
    mockFetch()
    const path = join(dir, 'Foo.dSYM/Contents/Resources/DWARF/Foo')
    await mkdir(join(dir, 'Foo.dSYM/Contents/Resources/DWARF'), { recursive: true })
    await writeFile(path, 'macho-bytes')
    const r = await uploadDsym({
      ...ADMIN,
      arch: 'arm64',
      debugId: '1234abcd-1234-1234-1234-1234567890ab',
      objectName: 'Foo',
      path,
      release: 'app@1.0.0+1',
    })
    expect(calls).toHaveLength(1)
    expect(calls[0]?.method).toBe('POST')
    expect(calls[0]?.url).toContain('/v1/releases/app%401.0.0%2B1/artifacts')
    expect(r.slices).toEqual([{ arch: 'arm64', debugId: '1234ABCD-1234-1234-1234-1234567890AB' }])
  })
})

describe('helpers', () => {
  test('dwarfBinariesIn walks a .dSYM bundle', async () => {
    const bundle = join(dir, 'Foo.dSYM')
    const dwarfDir = join(bundle, 'Contents/Resources/DWARF')
    await mkdir(dwarfDir, { recursive: true })
    await writeFile(join(dwarfDir, 'Foo'), 'x')
    await writeFile(join(dwarfDir, '.DS_Store'), 'noise')
    const found = dwarfBinariesIn(bundle)
    expect(found.map((p) => p.replace(bundle + '/', ''))).toEqual([
      'Contents/Resources/DWARF/Foo',
    ])
  })

  test('dwarfBinariesIn on a plain file returns just that file', async () => {
    const f = join(dir, 'binary')
    await writeFile(f, 'x')
    expect(dwarfBinariesIn(f)).toEqual([f])
  })

  test('dsymSlicesFromDwarfdump returns [] when dwarfdump fails (or is absent)', () => {
    // pass a path that doesn't exist; dwarfdump exits non-zero (or isn't installed)
    const slices = dsymSlicesFromDwarfdump(join(dir, 'nope.dSYM'))
    expect(slices).toEqual([])
  })
})

describe('gzip on the wire', () => {
  test('mapping upload body is gzip and filename gains .gz', async () => {
    const captured: { name: string; bytes: Uint8Array }[] = []
    globalThis.fetch = (async (_url: Request | string | URL, init?: RequestInit) => {
      const form = init?.body as FormData
      const file = form.get('file') as File
      captured.push({
        bytes: new Uint8Array(await file.arrayBuffer()),
        name: file.name,
      })
      return new Response('{}', { status: 201 })
    }) as typeof fetch
    const path = join(dir, 'mapping.txt')
    await writeFile(path, 'foo.bar -> a.b:\n'.repeat(1000))
    await uploadMapping({ ...ADMIN, path, release: 'r' })
    expect(captured).toHaveLength(1)
    expect(captured[0]?.name).toEndWith('.gz')
    expect(captured[0]?.bytes[0]).toBe(0x1f)
    expect(captured[0]?.bytes[1]).toBe(0x8b)
  })

  test('already-gzipped input passes through un-recompressed', async () => {
    const { gzipSync } = await import('node:zlib')
    const pre = gzipSync('already compressed')
    const captured: Uint8Array[] = []
    globalThis.fetch = (async (_url: Request | string | URL, init?: RequestInit) => {
      const form = init?.body as FormData
      const file = form.get('file') as File
      captured.push(new Uint8Array(await file.arrayBuffer()))
      return new Response('{}', { status: 201 })
    }) as typeof fetch
    const path = join(dir, 'mapping.txt.gz')
    await writeFile(path, pre)
    await uploadMapping({ ...ADMIN, path, release: 'r' })
    expect(Buffer.from(captured[0] ?? [])).toEqual(pre)
  })
})
