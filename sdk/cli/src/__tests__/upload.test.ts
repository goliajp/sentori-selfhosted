import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, beforeEach, describe, expect, test } from 'bun:test'

import { collectFiles, uploadSourcemaps } from '../upload.js'

let dir = ''
beforeEach(async () => {
  dir = await mkdtemp(join(tmpdir(), 'sentori-cli-test-'))
  await writeFile(join(dir, 'main.jsbundle'), '/*bundle*/')
  await writeFile(join(dir, 'main.jsbundle.map'), '{"version":3}')
  await writeFile(join(dir, 'app.js'), 'console.log(1)')
  await writeFile(join(dir, 'app.js.map'), '{"version":3}')
  await writeFile(join(dir, 'README.txt'), 'not a build artifact')
})
afterEach(async () => {
  await rm(dir, { force: true, recursive: true })
})

describe('collectFiles', () => {
  test('a directory yields its .map / .js / .bundle files, not others', async () => {
    const files = await collectFiles([dir])
    const names = files.map((f) => f.replace(`${dir}/`, '')).sort()
    expect(names).toEqual(['app.js', 'app.js.map', 'main.jsbundle', 'main.jsbundle.map'])
  })

  test('an explicit file is taken as-is regardless of extension', async () => {
    const files = await collectFiles([join(dir, 'README.txt')])
    expect(files).toEqual([join(dir, 'README.txt')])
  })

  test('dedupes when a file is named both directly and via its dir', async () => {
    const files = await collectFiles([dir, join(dir, 'app.js.map')])
    expect(files.filter((f) => f.endsWith('app.js.map'))).toHaveLength(1)
  })

  test('throws on a nonexistent path', async () => {
    await expect(collectFiles([join(dir, 'nope')])).rejects.toThrow('no such file or directory')
  })

  test('throws when a directory has no uploadable files', async () => {
    const empty = await mkdtemp(join(tmpdir(), 'sentori-cli-empty-'))
    try {
      await expect(collectFiles([empty])).rejects.toThrow('no .map')
    } finally {
      await rm(empty, { force: true, recursive: true })
    }
  })
})

describe('uploadSourcemaps', () => {
  test('dryRun returns the file list, makes no request', async () => {
    let called = false
    const orig = globalThis.fetch
    globalThis.fetch = (async () => {
      called = true
      return new Response()
    }) as typeof fetch
    try {
      const r = await uploadSourcemaps({
        apiUrl: 'https://api.example.com',
        dryRun: true,
        paths: [dir],
        release: 'app@1.0.0+1',
        token: 'st_pk_x',
      })
      expect(r.files).toHaveLength(4)
      expect(called).toBe(false)
    } finally {
      globalThis.fetch = orig
    }
  })

  test('POSTs multipart to /admin/api/releases/<release>/sourcemaps with the token', async () => {
    const calls: { headers: Headers; url: string }[] = []
    const orig = globalThis.fetch
    globalThis.fetch = (async (url: Request | string | URL, init?: RequestInit) => {
      calls.push({ headers: new Headers(init?.headers), url: String(url) })
      return new Response(JSON.stringify({ artifacts: [], uploaded: 4 }), {
        headers: { 'content-type': 'application/json' },
        status: 200,
      })
    }) as typeof fetch
    try {
      const r = await uploadSourcemaps({
        apiUrl: 'https://api.example.com/',
        paths: [dir],
        release: 'my app@1.0.0+1',
        token: 'st_pk_secret',
      })
      expect(calls).toHaveLength(1)
      expect(calls[0]?.url).toBe(
        'https://api.example.com/admin/api/releases/my%20app%401.0.0%2B1/sourcemaps',
      )
      expect(calls[0]?.headers.get('authorization')).toBe('Bearer st_pk_secret')
      expect(r.uploaded).toBe(4)
    } finally {
      globalThis.fetch = orig
    }
  })

  test('throws with the server detail on a non-2xx response', async () => {
    const orig = globalThis.fetch
    globalThis.fetch = (async () =>
      new Response('release frozen', { status: 403, statusText: 'Forbidden' })) as typeof fetch
    try {
      await expect(
        uploadSourcemaps({
          apiUrl: 'https://api.example.com',
          paths: [dir],
          release: 'app@1.0.0+1',
          token: 'st_pk_x',
        }),
      ).rejects.toThrow('403 Forbidden — release frozen')
    } finally {
      globalThis.fetch = orig
    }
  })
})
