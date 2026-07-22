import { spawnSync } from 'node:child_process'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, beforeEach, describe, expect, test } from 'bun:test'

import { buildSourceBundleFromDir } from '../source-bundle.js'

let dir = ''
beforeEach(async () => {
  dir = await mkdtemp(join(tmpdir(), 'sentori-cli-fromdir-'))
  await mkdir(join(dir, 'Sources', 'MyApp'), { recursive: true })
  await writeFile(join(dir, 'Sources', 'MyApp', 'View.swift'), '// swift\n')
  await writeFile(join(dir, 'Sources', 'MyApp', 'Util.m'), '// objc\n')
  await mkdir(join(dir, 'android', 'app', 'src'), { recursive: true })
  await writeFile(join(dir, 'android', 'app', 'src', 'MainActivity.kt'), '// kotlin\n')
  // Skips: hidden + skip-list + non-matching ext.
  await mkdir(join(dir, 'node_modules', 'foo'), { recursive: true })
  await writeFile(join(dir, 'node_modules', 'foo', 'index.swift'), '// should skip\n')
  await mkdir(join(dir, 'Pods'), { recursive: true })
  await writeFile(join(dir, 'Pods', 'a.swift'), '// should skip\n')
  await mkdir(join(dir, '.git'), { recursive: true })
  await writeFile(join(dir, '.git', 'config'), '\n')
  await writeFile(join(dir, 'README.md'), '# readme\n')
})
afterEach(async () => {
  await rm(dir, { force: true, recursive: true })
})

function listEntries(tarPath: string): string[] {
  const r = spawnSync('tar', ['-tzf', tarPath])
  if (r.status !== 0) throw new Error(`tar -t failed: ${r.stderr?.toString()}`)
  return r.stdout
    .toString()
    .split('\n')
    .map((s) => s.trim())
    .filter(Boolean)
}

describe('buildSourceBundleFromDir', () => {
  test('ios bundle includes .swift + .m + .h, excludes android + node_modules + Pods', async () => {
    const { cleanup, path } = await buildSourceBundleFromDir(dir, 'ios')
    try {
      const entries = listEntries(path)
      expect(entries).toContain('Sources/MyApp/View.swift')
      expect(entries).toContain('Sources/MyApp/Util.m')
      // No .kt
      expect(entries.find((e) => e.endsWith('.kt'))).toBeUndefined()
      // No node_modules
      expect(entries.find((e) => e.startsWith('node_modules/'))).toBeUndefined()
      // No Pods
      expect(entries.find((e) => e.startsWith('Pods/'))).toBeUndefined()
      // No .git
      expect(entries.find((e) => e.startsWith('.git/'))).toBeUndefined()
      // No README
      expect(entries.find((e) => e.endsWith('.md'))).toBeUndefined()
    } finally {
      await cleanup()
    }
  })

  test('android bundle picks .kt + .java only', async () => {
    const { cleanup, path } = await buildSourceBundleFromDir(dir, 'android')
    try {
      const entries = listEntries(path)
      expect(entries).toContain('android/app/src/MainActivity.kt')
      expect(entries.find((e) => e.endsWith('.swift'))).toBeUndefined()
      expect(entries.find((e) => e.endsWith('.m'))).toBeUndefined()
    } finally {
      await cleanup()
    }
  })

  test('empty directory of the right ext throws a clear error', async () => {
    const empty = await mkdtemp(join(tmpdir(), 'sentori-cli-empty-'))
    try {
      await expect(buildSourceBundleFromDir(empty, 'ios')).rejects.toThrow(
        /no ios source files/,
      )
    } finally {
      await rm(empty, { force: true, recursive: true })
    }
  })
})
