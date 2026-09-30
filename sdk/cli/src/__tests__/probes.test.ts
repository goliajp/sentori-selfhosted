// The probe scanner: finds sentori.probe('REF') call sites, skips
// junk dirs, dedupes.

import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, beforeEach, describe, expect, test } from 'bun:test'

import { scanProbes } from '../probes.js'

let dir = ''
beforeEach(async () => {
  dir = await mkdtemp(join(tmpdir(), 'sentori-probes-'))
})
afterEach(async () => {
  await rm(dir, { force: true, recursive: true })
})

describe('scanProbes', () => {
  test('finds refs across quote styles and dedupes', async () => {
    await writeFile(
      join(dir, 'a.ts'),
      `sentori.probe('SENT-1'); probe("SENT-2", {x:1}); sentori.probe(\`SENT-1\`)`,
    )
    await writeFile(join(dir, 'b.tsx'), `if (bad) { sentori.probe('SENT-3') }`)
    expect(scanProbes(dir)).toEqual(['SENT-1', 'SENT-2', 'SENT-3'])
  })

  test('skips node_modules and non-source files', async () => {
    await mkdir(join(dir, 'node_modules'), { recursive: true })
    await writeFile(join(dir, 'node_modules', 'x.ts'), `probe('NOPE')`)
    await writeFile(join(dir, 'notes.md'), `probe('ALSO-NOPE')`)
    expect(scanProbes(dir)).toEqual([])
  })

  test('dynamic refs are invisible (documented limitation)', async () => {
    await writeFile(join(dir, 'c.ts'), 'sentori.probe(ref)')
    expect(scanProbes(dir)).toEqual([])
  })
})
