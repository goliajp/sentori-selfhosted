import { afterEach, beforeEach, describe, expect, test } from 'bun:test'
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { checkCompatibility, CLI_VERSION } from '../compatibility.js'
import { fetchArtifacts, type ArtifactsResponse } from '../artifacts.js'
import { runWorkflow } from '../workflow.js'

const legacy: ArtifactsResponse = { release: 'app@1+7', known: false, artifacts: [], kinds: {}, missing: [] }
const modern = { ...legacy, compatibility: { serverVersion: '5.1.12', artifactProtocol: 1, sourceBundleFormats: ['zip', 'json'], verification: ['sha256', 'format'] } }
let root: string
const originalFetch = globalThis.fetch
const oldToken = process.env.SENTORI_TEST_UPLOAD_TOKEN
beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), 'sentori-compatibility-'))
  process.env.SENTORI_TEST_UPLOAD_TOKEN = 'api-test'
})
afterEach(() => {
  globalThis.fetch = originalFetch
  rmSync(root, { recursive: true, force: true })
  if (oldToken === undefined) delete process.env.SENTORI_TEST_UPLOAD_TOKEN
  else process.env.SENTORI_TEST_UPLOAD_TOKEN = oldToken
})
function config(sources = true): string {
  const path = join(root, 'sentori.config.json')
  writeFileSync(path, JSON.stringify({ version: 1, apiUrl: 'https://upload.test', tokenEnv: 'SENTORI_TEST_UPLOAD_TOKEN', release: legacy.release,
    artifacts: sources ? {} : { mapping: ['mapping.txt'] }, ...(sources ? { sources: { include: ['src'] } } : {}) }))
  return path
}

describe('CLI/server compatibility', () => {
  test('installed package determines --version, independently of server and app releases', () => {
    expect(CLI_VERSION).toBe(JSON.parse(readFileSync(new URL('../../package.json', import.meta.url), 'utf8')).version)
    expect(checkCompatibility(modern, true)).toContain('server 5.1.12')
    expect(checkCompatibility(legacy, false)).toContain('Legacy artifact API')
    expect(() => checkCompatibility(legacy, true)).toThrow('does not advertise source ZIP')
    expect(() => checkCompatibility({ ...modern, compatibility: { ...modern.compatibility, artifactProtocol: 2 } }, false)).toThrow('Unsupported server artifact protocol')
    expect(() => checkCompatibility({ ...modern, compatibility: { ...modern.compatibility, sourceBundleFormats: ['json'] } }, true)).toThrow('does not support source ZIPs')
    expect(() => checkCompatibility({ ...modern, compatibility: { ...modern.compatibility, verification: ['sha256'] } }, false)).toThrow('verification capabilities')
  })
  test('doctor works before build files exist and never sends a write request', async () => {
    let requests = 0
    globalThis.fetch = (async (_url, init) => {
      expect(init?.method ?? 'GET').toBe('GET'); requests++
      return Response.json(modern)
    }) as typeof fetch
    const path = config()
    expect(await runWorkflow('doctor', ['--config', path])).toBe(0)
    expect(requests).toBe(1)
    expect(await runWorkflow('doctor', ['--config', path, '--dry-run'])).toBe(1)
    expect(await runWorkflow('verify', ['--config', path, '--dry-run'])).toBe(1)
    expect(requests).toBe(1)
  })
  test('old servers reject ZIP plans before uploading even the other artifact kinds', async () => {
    mkdirSync(join(root, 'src')); writeFileSync(join(root, 'src/main.ts'), 'export const x = 1')
    writeFileSync(join(root, 'mapping.txt'), 'A -> a:\n')
    const path = config()
    const value = JSON.parse(readFileSync(path, 'utf8')); value.artifacts.mapping = ['mapping.txt']; writeFileSync(path, JSON.stringify(value))
    const methods: string[] = []
    globalThis.fetch = (async (_url, init) => { methods.push(init?.method ?? 'GET'); return Response.json(legacy) }) as typeof fetch
    expect(await runWorkflow('doctor', ['--config', path])).toBe(1)
    expect(await runWorkflow('upload', ['--config', path, '--strict'])).toBe(1)
    expect(await runWorkflow('upload', ['--config', path])).toBe(0)
    expect(methods).toEqual(['GET', 'GET', 'GET'])
  })
  test('a legacy server remains usable for builds without source ZIPs', async () => {
    const path = config(false)
    globalThis.fetch = (async () => Response.json(legacy)) as typeof fetch
    expect(await runWorkflow('doctor', ['--config', path])).toBe(0)
  })
  test('wrong inventory shapes and unauthorised tokens never become compatibility success', async () => {
    const opts = { apiUrl: 'https://upload.test', token: 't', release: legacy.release }
    for (const body of [{}, { ...legacy, release: 'another-app' }, { ...legacy, artifacts: null }]) {
      globalThis.fetch = (async () => Response.json(body)) as typeof fetch
      await expect(fetchArtifacts(opts)).rejects.toThrow('Unsupported artifact inventory response')
    }
    globalThis.fetch = (async () => Response.json({ error: 'admin_token_required' }, { status: 403 })) as typeof fetch
    expect(await runWorkflow('doctor', ['--config', config()])).toBe(1)
  })
})
