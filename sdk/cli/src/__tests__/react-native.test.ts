import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, beforeEach, describe, expect, test } from 'bun:test'

import { composeSourceMaps, resolveComposeScript } from '../react-native.js'

let dir = ''
beforeEach(async () => {
  dir = await mkdtemp(join(tmpdir(), 'sentori-cli-rn-'))
})
afterEach(async () => {
  await rm(dir, { force: true, recursive: true })
})

describe('react-native helpers', () => {
  test('resolveComposeScript returns null when react-native is not installed', () => {
    // This package doesn't depend on react-native, so from sdk/cli's
    // own node_modules there's nothing to find.
    expect(resolveComposeScript()).toBeNull()
  })

  test('composeSourceMaps throws "no such file" for a missing input', () => {
    expect(() => composeSourceMaps(join(dir, 'nope.map'), join(dir, 'nope2.map'))).toThrow(
      'no such file',
    )
  })

  test('composeSourceMaps throws a helpful message when react-native is absent', async () => {
    await writeFile(join(dir, 'a.packager.map'), '{"version":3}')
    await writeFile(join(dir, 'a.hbc.map'), '{"version":3}')
    expect(() =>
      composeSourceMaps(join(dir, 'a.packager.map'), join(dir, 'a.hbc.map')),
    ).toThrow(/compose-source-maps\.js/)
  })
})
