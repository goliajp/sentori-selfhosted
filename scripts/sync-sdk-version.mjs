#!/usr/bin/env node
// Carry the version changesets just wrote in package.json into the
// one place the SDK repeats it.
//
// `transport.ts` hard-codes SDK_VERSION because it goes out in the
// `Sentori-Sdk` header on every request, and importing package.json
// to get it would bundle the whole file — devDependencies and all —
// into a client whose footprint is a stated product constraint.
//
// A test asserts the two agree, so a stale constant cannot ship. But
// that test only ran in CI, after the release commit was already
// pushed: `changeset version` does not know about the constant, so
// every release carried a manual step that was only ever caught by
// going red on master. This closes that: `bun run version-packages`
// runs `changeset version` and then this, and the test goes back to
// being a backstop rather than the only thing watching.

import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')

/** Each entry: the package whose version wins, and the file plus
 *  pattern that repeats it. Add a row when a package grows one. */
const MIRRORS = [
  {
    pkg: 'sdk/react-native/package.json',
    file: 'sdk/react-native/src/transport.ts',
    pattern: /^(const SDK_VERSION = ')([^']+)(';)$/m,
  },
  // The native SDKs have their own number, in `sdk/native/VERSION`.
  //
  // They took the React Native package's while the two shipped
  // together. They no longer do: an iOS or Android app has no opinion
  // about the RN SDK's major, and a version that moves for reasons the
  // consumer cannot see is worse than a separate one. Leaving this row
  // pointed at package.json would have had the native SDK report 6.2.1
  // in its `Sentori-Sdk` header while its published package said
  // 1.0.0 — precisely the lie this script exists to prevent.
  {
    version: 'sdk/native/VERSION',
    file: 'sdk/native/ios/Sources/Sentori/SentoriConfig.swift',
    pattern: /^(    public static let current = ")([^"]+)(")$/m,
  },
  {
    version: 'sdk/native/VERSION',
    file: 'sdk/native/android/src/main/java/com/sentori/SentoriConfig.kt',
    pattern: /^(    const val CURRENT = ")([^"]+)(")$/m,
  },
]

let changed = 0
for (const m of MIRRORS) {
  // Either a package.json's `version`, or a plain VERSION file.
  const version = m.pkg
    ? JSON.parse(readFileSync(join(root, m.pkg), 'utf8')).version
    : readFileSync(join(root, m.version), 'utf8').trim()
  const path = join(root, m.file)
  const src = readFileSync(path, 'utf8')
  const found = src.match(m.pattern)
  if (!found) {
    // The constant was renamed or removed and this script silently
    // stopped doing anything — the failure mode it exists to prevent,
    // wearing a different hat.
    console.error(`sync-sdk-version: no version constant in ${m.file}; pattern is stale`)
    process.exit(1)
  }
  if (found[2] === version) continue
  writeFileSync(path, src.replace(m.pattern, `$1${version}$3`))
  console.log(`sync-sdk-version: ${m.file} ${found[2]} → ${version}`)
  changed++
}
if (changed === 0) console.log('sync-sdk-version: all version constants already current')
