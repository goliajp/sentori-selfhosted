import { afterEach, beforeEach, describe, expect, test } from 'bun:test'
import { createHash } from 'node:crypto'
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { gunzipSync } from 'node:zlib'
import { loadUploadConfig, prepareUploadPlan, runWorkflow, verifyUploadPlan } from '../workflow.js'
import { inspectSourceArchive } from '../srcbundle.js'

let root: string
const realFetch = globalThis.fetch
const oldToken = process.env.SENTORI_TEST_UPLOAD_TOKEN
beforeEach(() => { root = mkdtempSync(join(tmpdir(), 'sentori-upload-test-')); process.env.SENTORI_TEST_UPLOAD_TOKEN = 'api-test' })
afterEach(() => { globalThis.fetch = realFetch; rmSync(root, { recursive: true, force: true }); if (oldToken === undefined) delete process.env.SENTORI_TEST_UPLOAD_TOKEN; else process.env.SENTORI_TEST_UPLOAD_TOKEN = oldToken })
function config(artifacts: object = {}, sources?: object) {
  const path = join(root, 'sentori.config.json')
  writeFileSync(path, JSON.stringify({ version: 1, apiUrl: 'https://upload.test', tokenEnv: 'SENTORI_TEST_UPLOAD_TOKEN', release: 'app@1+7', artifacts, ...(sources ? { sources } : {}) }))
  return path
}
function reply(body: unknown, status = 200) { return new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } }) }

describe('unified release upload', () => {
  test('configuration paths are relative to config, and init never overwrites a config', async () => {
    writeFileSync(join(root, 'mapping.txt'), 'com.example.A -> a:\n    void run() -> a\n')
    const path = config({ mapping: ['mapping.txt'] })
    expect(prepareUploadPlan(path).files[0]?.path).toBe(join(root, 'mapping.txt'))
    expect(await runWorkflow('init', ['--config', path])).toBe(1)
    expect(loadUploadConfig(path).tokenEnv).toBe('SENTORI_TEST_UPLOAD_TOKEN')
  })
  test('checks every source map, DWARF binary, mapping and source archive after upload', async () => {
    writeFileSync(join(root, 'index.map'), JSON.stringify({ version: 3, sources: ['src/a.ts'], names: [], mappings: 'AAAA' }))
    writeFileSync(join(root, 'mapping.txt'), 'com.example.A -> a:\n    void run() -> a\n')
    writeFileSync(join(root, 'dwarf'), Buffer.from('cffaedfe00000000', 'hex'))
    mkdirSync(join(root, 'src')); writeFileSync(join(root, 'src/a.ts'), 'export const a = 1')
    const path = config({ sourcemap: ['index.map'], dsym: ['dwarf'], mapping: ['mapping.txt'] }, { include: ['src'] })
    const uploaded: any[] = []
    globalThis.fetch = (async (url, init) => {
      expect(String(url)).toContain('/v1/releases/app%401%2B7/artifacts')
      expect((init?.headers as any).Authorization).toBe('Bearer api-test')
      if (init?.method === 'POST') {
        const form = init.body as FormData; const file = form.get('file') as File
        const bytes = gunzipSync(Buffer.from(await file.arrayBuffer()))
        const kind = String(form.get('kind'))
        if (kind === 'srcbundle') expect(inspectSourceArchive(bytes, 'app@1+7').files).toEqual(['src/a.ts'])
        uploaded.push({ kind, name: file.name.replace(/\.gz$/, ''), usable: true, contentHash: createHash('sha256').update(bytes).digest('hex') })
        return reply({ id: String(uploaded.length), usable: true }, 201)
      }
      return reply({ release: 'app@1+7', known: uploaded.length > 0, artifacts: uploaded,
        compatibility: { serverVersion: '5.1.12', artifactProtocol: 1, sourceBundleFormats: ['zip', 'json'], verification: ['sha256', 'format'] } })
    }) as typeof fetch
    expect(await runWorkflow('upload', ['--config', path, '--strict'])).toBe(0)
    expect(uploaded.map(x => x.kind)).toEqual(['sourcemap', 'dsym', 'proguard', 'srcbundle'])
    expect(await runWorkflow('verify', ['--config', path])).toBe(0)
  })
  test('a readable old artifact of the same kind cannot satisfy verification', async () => {
    writeFileSync(join(root, 'mapping.txt'), 'A -> a:\n')
    const plan = prepareUploadPlan(config({ mapping: ['mapping.txt'] }))
    globalThis.fetch = (async () => reply({ known: true, release: plan.release, artifacts: [{ kind: 'proguard', usable: true, contentHash: 'old' }] })) as typeof fetch
    expect(await verifyUploadPlan(plan, 't')).toEqual(['missing exact bytes: proguard mapping.txt'])
    globalThis.fetch = (async () => reply({ known: true, release: plan.release, artifacts: [{ kind: 'proguard', name: 'mapping.txt', usable: null, contentHash: plan.files[0]!.hash }] })) as typeof fetch
    expect((await verifyUploadPlan(plan, 't'))[0]).toContain('not verified')
    globalThis.fetch = (async () => reply({ known: true, release: plan.release, artifacts: [{ kind: 'proguard', name: 'another.txt', usable: true, contentHash: plan.files[0]!.hash }] })) as typeof fetch
    expect((await verifyUploadPlan(plan, 't'))[0]).toContain('missing exact bytes')
  })
  test('dry-run needs no token or network; failures are lenient only for uploads', async () => {
    writeFileSync(join(root, 'mapping.txt'), 'A -> a:\n')
    const path = config({ mapping: ['mapping.txt'] })
    globalThis.fetch = (async () => { throw new Error('offline') }) as typeof fetch
    delete process.env.SENTORI_TEST_UPLOAD_TOKEN
    expect(await runWorkflow('upload', ['--config', path, '--dry-run'])).toBe(0)
    expect(await runWorkflow('upload', ['--config', path])).toBe(0)
    expect(await runWorkflow('upload', ['--config', path, '--strict'])).toBe(1)
    expect(await runWorkflow('verify', ['--config', path])).toBe(1)
  })
  test('invalid local files prevent any upload', async () => {
    writeFileSync(join(root, 'a.map'), 'not a map')
    const path = config({ sourcemap: ['a.map'] })
    globalThis.fetch = (async () => { throw new Error('network must not be called') }) as typeof fetch
    expect(await runWorkflow('validate', ['--config', path])).toBe(1)
  })
  test('partial remote failure still uploads remaining files and fails strict verification', async () => {
    writeFileSync(join(root, 'a.map'), JSON.stringify({ version: 3, sources: ['a.ts'], names: [], mappings: 'AAAA' }))
    writeFileSync(join(root, 'mapping.txt'), 'A -> a:\n')
    const path = config({ sourcemap: ['a.map'], mapping: ['mapping.txt'] })
    const attempted: string[] = [], uploaded: any[] = []
    globalThis.fetch = (async (_url, init) => {
      if (init?.method === 'POST') {
        const form = init.body as FormData, kind = String(form.get('kind'))
        attempted.push(kind)
        if (kind === 'sourcemap') return reply({ error: 'injected rejection' }, 400)
        const file = form.get('file') as File
        uploaded.push({ kind, usable: true, contentHash: createHash('sha256').update(gunzipSync(Buffer.from(await file.arrayBuffer()))).digest('hex') })
        return reply({ id: '1', usable: true }, 201)
      }
      return reply({ known: uploaded.length > 0, release: 'app@1+7', artifacts: uploaded })
    }) as typeof fetch
    expect(await runWorkflow('upload', ['--config', path, '--strict'])).toBe(1)
    expect(attempted).toEqual(['sourcemap', 'proguard'])
    expect(uploaded).toHaveLength(1)
  })

})
