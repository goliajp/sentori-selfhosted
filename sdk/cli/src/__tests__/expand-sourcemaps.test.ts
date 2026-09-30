// A directory of maps keeps its shape on the way to the server.
//
// The server matches a frame to a map by how many trailing path
// segments they share, because a mini program names every page's entry
// `index.js` and a web build names every chunk after its route. The
// CLI sent only the basename, so that matching never saw more than a
// filename however carefully it was written — and passing a directory,
// which the web getting-started page tells people to do, threw EISDIR.

import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { describe, expect, it } from 'bun:test'

import { expandSourcemapPaths } from '../upload.js'

const tree = (): string => {
  const root = mkdtempSync(join(tmpdir(), 'sentori-maps-'))
  mkdirSync(join(root, 'pages/cart'), { recursive: true })
  mkdirSync(join(root, 'pages/home'), { recursive: true })
  writeFileSync(join(root, 'pages/cart/index.js.map'), '{}')
  writeFileSync(join(root, 'pages/home/index.js.map'), '{}')
  writeFileSync(join(root, 'pages/cart/index.js'), '//')
  writeFileSync(join(root, 'main.js.map'), '{}')
  return root
}

describe('expandSourcemapPaths', () => {
  it('keeps the path under the directory, so two index.js.map stay distinct', () => {
    const names = expandSourcemapPaths(tree())
      .map((t) => t.name)
      .sort()
    expect(names).toEqual(['main.js.map', 'pages/cart/index.js.map', 'pages/home/index.js.map'])
  })

  it('takes only maps, not the bundles beside them', () => {
    // A bundle uploaded as a sourcemap is stored, unparseable, and
    // lights the release row amber — which happened in production for
    // months on a real release.
    expect(expandSourcemapPaths(tree()).every((t) => t.name.endsWith('.map'))).toBe(true)
  })

  it('leaves a single file as its basename', () => {
    const root = tree()
    const out = expandSourcemapPaths(join(root, 'pages/cart/index.js.map'))
    expect(out).toHaveLength(1)
    expect(out[0]!.name).toBe('index.js.map')
  })

  it('does not throw on a path that does not exist', () => {
    // The caller's read produces the real error with the real path in
    // it; this must not pre-empt it with a worse one.
    expect(() => expandSourcemapPaths('/nope/missing.map')).not.toThrow()
  })
})
