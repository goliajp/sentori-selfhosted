#!/usr/bin/env node
/**
 * EAS post-build hook for Sentori source map upload.
 *
 * Wire it from app.json / eas.json:
 *
 *   {
 *     "build": {
 *       "production": {
 *         "ios": { "buildArtifactPaths": ["ios/build/**\/*.dSYM"] },
 *         "hooks": {
 *           "postPublish": [
 *             {
 *               "config": "@goliapkg/sentori-expo/eas-post-build",
 *               "options": { "release": "myapp@1.2.3+42" }
 *             }
 *           ]
 *         }
 *       }
 *     }
 *   }
 *
 * Or call this script directly from a custom build hook:
 *
 *   #!/bin/sh
 *   node ./node_modules/@goliapkg/sentori-expo/scripts/eas-post-build.mjs \
 *     --token $SENTORI_ADMIN_TOKEN --release "$EAS_BUILD_RELEASE"
 *
 * Shells out to `@goliapkg/sentori-cli upload sourcemap` for the actual
 * upload. Make sure `@goliapkg/sentori-cli` is installed (or reachable
 * via `npx`); if it can't be found this logs a warning and exits 0 so
 * it never fails a build.
 *
 * Note: for an EAS *Hermes* production build the bundle + maps are
 * platform-specific and must be composed first (Metro map + Hermes
 * map); see docs → Recipes → "Source map upload" → React Native. This
 * helper uploads whatever is under `./dist` (the default
 * `expo export --source-maps` output) — fine for managed JS-only
 * exports.
 */

import { spawnSync } from 'node:child_process'
import { existsSync } from 'node:fs'

const args = parseArgs(process.argv.slice(2))

const token = args.token ?? process.env.SENTORI_ADMIN_TOKEN
const release = args.release ?? process.env.EAS_BUILD_RELEASE
const apiUrl = args['api-url'] ?? process.env.SENTORI_API_URL ?? process.env.SENTORI_INGEST_URL

if (!token || !release) {
  console.error(
    '[sentori-expo:eas-post-build] missing --token or --release ' +
      '(env: SENTORI_ADMIN_TOKEN, EAS_BUILD_RELEASE)',
  )
  process.exit(1)
}

const cli = resolveCli()
if (!cli.length) {
  console.warn(
    '[sentori-expo:eas-post-build] @goliapkg/sentori-cli not found on PATH or in ' +
      'node_modules, and npx is unavailable. Skipping source-map upload — ' +
      'install @goliapkg/sentori-cli (or make npx available) to enable.',
  )
  process.exit(0)
}

const cmd = [
  ...cli.slice(1),
  'upload',
  'sourcemap',
  '--token',
  token,
  '--release',
  release,
  ...(apiUrl ? ['--api-url', apiUrl] : []),
  // Default Expo build output for the JS bundle + sourcemap.
  './dist',
]

console.log(`[sentori-expo:eas-post-build] running: ${cli[0]} ${cmd.join(' ')}`)
const r = spawnSync(cli[0], cmd, { stdio: 'inherit' })
process.exit(r.status ?? 0)

function parseArgs(argv) {
  const out = {}
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]
    if (a.startsWith('--')) {
      out[a.slice(2)] = argv[i + 1]
      i++
    }
  }
  return out
}

/** Returns `[command, ...prefixArgs]` to invoke the CLI, or `[]` if it
 *  can't be found anywhere. Prefers a locked node_modules copy over a
 *  global one; falls back to `npx`. */
function resolveCli() {
  // node_modules/.bin/sentori-cli — npm creates this from the package's
  // `bin` field; the locked version wins over a global install.
  if (existsSync('./node_modules/.bin/sentori-cli')) {
    return ['./node_modules/.bin/sentori-cli']
  }
  // The package's bin entry directly (in case .bin wasn't linked).
  const direct = './node_modules/@goliapkg/sentori-cli/lib/index.js'
  if (existsSync(direct)) return ['node', direct]
  // On PATH (global install).
  const which = spawnSync('which', ['sentori-cli'])
  const found = which.stdout?.toString().trim()
  if (found) return [found]
  // Last resort: npx (will fetch the package if not cached).
  const npx = spawnSync('which', ['npx'])
  if (npx.stdout?.toString().trim()) return ['npx', '--yes', '@goliapkg/sentori-cli@latest']
  return []
}
