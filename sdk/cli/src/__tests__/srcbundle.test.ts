import { afterEach, beforeEach, describe, expect, test } from 'bun:test'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { unzipSync, zipSync } from 'fflate'
import { collectSources, inspectSourceArchive, packSources, SOURCE_MANIFEST } from '../srcbundle.js'

let root: string
beforeEach(() => { root = mkdtempSync(join(tmpdir(), 'sentori-sources-test-')) })
afterEach(() => rmSync(root, { recursive: true, force: true }))
function file(path: string, body: string) { const full = join(root, path); mkdirSync(join(full, '..'), { recursive: true }); writeFileSync(full, body) }
function git(...args: string[]) { return execFileSync('git', ['-C', root, '-c', 'user.name=Test', '-c', 'user.email=test@example.test', '-c', 'commit.gpgsign=false', ...args], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim() }

describe('source archive collection', () => {
  test('works without Git, preserves selected roots and excludes dependencies and secrets', () => {
    file('ios/Pay.swift', 'ios code'); file('shared/Pay.swift', 'shared code')
    file('ios/.env', 'secret'); file('ios/Pods/P.swift', 'dependency')
    file('shared/tests/Test.swift', 'test code'); file('shared/index.ts', 'export {}')
    const result = packSources(['ios', 'shared'], 'app@1+7', { root, exclude: ['shared/tests'] })
    expect(result.manifest.files.map(x => x.path)).toEqual(['ios/Pay.swift', 'shared/Pay.swift', 'shared/index.ts'])
    expect(inspectSourceArchive(result.bytes, 'app@1+7').files).toEqual(result.manifest.files.map(x => x.path))
    expect(unzipSync(result.bytes)['shared/Pay.swift']).toEqual(Buffer.from('shared code'))
    expect(packSources(['shared', 'ios'], 'app@1+7', { root, exclude: ['shared/tests'] }).bytes).toEqual(result.bytes)
  })
  test('explicitly selected generated sources are included without sweeping build output', () => {
    file('src/a.kt', 'app'); file('build/generated/Bindings.kt', 'generated'); file('build/cache/Other.kt', 'cache')
    expect(Object.keys(collectSources(['.'], { root }).bundle)).toEqual(['src/a.kt'])
    expect(Object.keys(collectSources(['src', 'build/generated'], { root }).bundle)).toEqual(['src/a.kt', 'build/generated/Bindings.kt'])
  })
  test('an exact annotated tag ignores later commits and uncommitted files without checkout', () => {
    git('init'); file('src/a.kt', 'tagged code'); git('add', '.'); git('commit', '-m', 'tagged')
    git('-c', 'tag.gpgsign=false', 'tag', '-a', 'v1', '-m', 'release')
    const commit = git('rev-parse', 'HEAD')
    file('src/a.kt', 'later code'); git('add', '.'); git('commit', '-m', 'later')
    file('src/a.kt', 'uncommitted'); file('src/new.kt', 'untracked')
    const head = git('rev-parse', 'HEAD')
    const packed = packSources(['src'], 'app@1+7', { root, gitTag: 'v1' })
    expect(packed.manifest.source).toEqual({ type: 'git-tag', tag: 'v1', commit })
    expect(unzipSync(packed.bytes)['src/a.kt']).toEqual(Buffer.from('tagged code'))
    expect(packed.manifest.files).toHaveLength(1)
    expect(readFileSync(join(root, 'src/a.kt'), 'utf8')).toBe('uncommitted')
    expect(git('rev-parse', 'HEAD')).toBe(head)
    expect(() => packSources(['src'], 'r', { root, gitTag: 'master' })).toThrow()
  })
  test('skips symbolic links and rejects selections escaping the declared root', () => {
    file('src/a.kt', 'ok'); symlinkSync('/etc/passwd', join(root, 'src/secret.kt'))
    expect(Object.keys(collectSources(['src'], { root }).bundle)).toEqual(['src/a.kt'])
    expect(() => collectSources(['../'], { root })).toThrow('outside')
  })
  test('does not silently truncate oversized selected sources', () => {
    file('src/huge.kt', 'x'.repeat(512 * 1024 + 1))
    expect(() => packSources(['src'], 'r', { root })).toThrow('512 KiB')
  })
  test('validates external ZIP paths, release and manifest hashes', () => {
    file('src/a.kt', 'code')
    const packed = packSources(['src'], 'r1', { root })
    expect(() => inspectSourceArchive(packed.bytes, 'r2')).toThrow('does not match')
    const files = unzipSync(packed.bytes); files['src/a.kt'] = Buffer.from('tampered')
    expect(() => inspectSourceArchive(zipSync(files), 'r1')).toThrow('checksum')
    expect(() => inspectSourceArchive(zipSync({ '../bad.kt': Buffer.from('bad') }))).toThrow('unsafe')
    expect(() => inspectSourceArchive(zipSync({}))).toThrow('empty')
    expect(inspectSourceArchive(zipSync({ 'Sources/App.swift': Buffer.from('plain zip') })).files).toEqual(['Sources/App.swift'])
    expect(unzipSync(packed.bytes)[SOURCE_MANIFEST]).toBeDefined()
  })
})
