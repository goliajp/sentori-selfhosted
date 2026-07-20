import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, beforeEach, describe, expect, test } from 'bun:test'

import { uploadSourceBundle } from '../source-bundle.js'

const ADMIN = {
  apiUrl: 'https://api.example.com/',
  projectId: 'proj-1',
  release: 'myapp@1.0.0',
  token: 'sk_test',
}

let dir = ''
const origFetch = globalThis.fetch
let calls: { body: Buffer | null; headers: Headers; url: string }[]

beforeEach(async () => {
  dir = await mkdtemp(join(tmpdir(), 'sentori-cli-srcbun-'))
  calls = []
})
afterEach(async () => {
  globalThis.fetch = origFetch
  await rm(dir, { force: true, recursive: true })
})

function mockFetch(status = 201, body: object = { contentHash: 'h', kind: 'source_bundle_ios', sizeBytes: 12 }): void {
  globalThis.fetch = (async (url: Request | string | URL, init?: RequestInit) => {
    const b = init?.body
    calls.push({
      body: b instanceof Uint8Array ? Buffer.from(b) : null,
      headers: new Headers(init?.headers),
      url: String(url),
    })
    return new Response(JSON.stringify(body), { status })
  }) as typeof fetch
}

describe('uploadSourceBundle', () => {
  test('rejects unknown platform', async () => {
    const path = join(dir, 'a.tar.gz')
    await writeFile(path, Buffer.from([0x1f, 0x8b, 0x08, 0x00]))
    mockFetch()
    await expect(
      uploadSourceBundle({ ...ADMIN, path, platform: 'windows' as unknown as 'ios' }),
    ).rejects.toThrow(/platform/)
  })

  test('rejects non-gzip body before hitting network', async () => {
    const path = join(dir, 'not-gzip.tar.gz')
    await writeFile(path, Buffer.from('plain text not a gzip stream'))
    mockFetch()
    await expect(
      uploadSourceBundle({ ...ADMIN, path, platform: 'ios' }),
    ).rejects.toThrow(/gzip|1f 8b/i)
    expect(calls.length).toBe(0)
  })

  test('rejects empty body', async () => {
    const path = join(dir, 'empty.tar.gz')
    await writeFile(path, Buffer.alloc(0))
    mockFetch()
    await expect(uploadSourceBundle({ ...ADMIN, path, platform: 'ios' })).rejects.toThrow(
      /empty/,
    )
  })

  test('posts to /admin/api/projects/<id>/source-bundles with platform + release query', async () => {
    const path = join(dir, 'src.tar.gz')
    // gzip magic + minimal payload — server-side validation passes.
    await writeFile(path, Buffer.from([0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00]))
    mockFetch(201, { contentHash: 'abc123', kind: 'source_bundle_ios', sizeBytes: 6 })

    const r = await uploadSourceBundle({ ...ADMIN, path, platform: 'ios' })

    expect(calls.length).toBe(1)
    expect(calls[0]!.url).toContain('/admin/api/projects/proj-1/source-bundles?')
    expect(calls[0]!.url).toContain('platform=ios')
    expect(calls[0]!.url).toContain('release=myapp%401.0.0')
    expect(calls[0]!.headers.get('authorization')).toBe('Bearer sk_test')
    expect(calls[0]!.headers.get('content-type')).toBe('application/gzip')
    expect(r.kind).toBe('source_bundle_ios')
    expect(r.contentHash).toBe('abc123')
  })

  test('android platform routes the same shape', async () => {
    const path = join(dir, 'src.tar.gz')
    await writeFile(path, Buffer.from([0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00]))
    mockFetch(201, { contentHash: 'def', kind: 'source_bundle_android', sizeBytes: 6 })
    const r = await uploadSourceBundle({ ...ADMIN, path, platform: 'android' })
    expect(calls[0]!.url).toContain('platform=android')
    expect(r.kind).toBe('source_bundle_android')
  })

  test('surfaces server 4xx/5xx as Error with snippet', async () => {
    const path = join(dir, 'src.tar.gz')
    await writeFile(path, Buffer.from([0x1f, 0x8b]))
    mockFetch(500, { error: { code: 'bad', message: 'boom on server' } })
    await expect(uploadSourceBundle({ ...ADMIN, path, platform: 'ios' })).rejects.toThrow(
      /500/,
    )
  })
})
