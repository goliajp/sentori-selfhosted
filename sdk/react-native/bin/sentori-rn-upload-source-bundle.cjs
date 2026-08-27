#!/usr/bin/env node
// v1.4 W28 — RN build-time source-bundle uploader.
//
// Operators add this to their build scripts (or a CI step) so a fresh
// source bundle is uploaded for each release without remembering to
// run sentori-cli by hand. The CLI is small on purpose:
//
//   sentori-rn-upload-source-bundle --platform ios|android \
//       [--release <r>] [--config <path>]
//
// Config resolution order (first hit wins):
//   1. --release flag
//   2. SENTORI_RELEASE env var
//   3. Auto-derived from the host app's package.json `version`
//      (falls back with a clear error if not readable).
//
// The token + project id + api url come from:
//   1. CLI flags (--token, --project, --api-url) — if any is set, use them
//   2. `sentori.config.json` in the cwd (the host app root) — keys:
//      { token, projectId, apiUrl, sources: { ios: "...", android: "..." } }
//   3. Env vars: SENTORI_TOKEN, SENTORI_PROJECT_ID, SENTORI_API_URL
//
// Source-tree path resolution:
//   - Default ios: `<cwd>/ios`
//   - Default android: `<cwd>/android/app/src`
//   - Overridable per-platform in sentori.config.json under
//     `sources.ios` / `sources.android`.
//
// This script shells out to `sentori-cli upload source-bundle
// --from-dir <path>` — sentori-cli already knows how to walk the
// directory, build a tarball, and upload. We just wire the flags.

'use strict'

const { spawnSync } = require('node:child_process')
const fs = require('node:fs')
const path = require('node:path')

const HELP = `\
sentori-rn-upload-source-bundle — upload native sources for inline source view

Usage:
  sentori-rn-upload-source-bundle --platform ios|android \\
      [--release <r>] [--module <label>] \\
      [--project <uuid>] [--token <t>] [--api-url <url>] \\
      [--config <path>]

Config (\`sentori.config.json\` in the app root):
  {
    "token":     "...",
    "projectId": "uuid",
    "apiUrl":    "https://sentori.golia.jp",
    "sources":   { "ios": "ios", "android": "android/app/src" }
  }

Example (in package.json):
  "scripts": {
    "build:ios":      "react-native bundle ... && sentori-rn-upload-source-bundle --platform ios",
    "build:android":  "react-native bundle ... && sentori-rn-upload-source-bundle --platform android"
  }
`

const args = parseArgs(process.argv.slice(2))
if (args.help) {
  console.log(HELP)
  process.exit(0)
}

const cwd = process.cwd()
const config = loadConfig(args.config, cwd)
const platform = args.platform
if (platform !== 'ios' && platform !== 'android') {
  fail(`--platform must be 'ios' or 'android' (got ${platform || '(missing)'})`)
}

const release =
  args.release ||
  process.env.SENTORI_RELEASE ||
  deriveReleaseFromPackageJson(cwd) ||
  fail('release not set: pass --release or set SENTORI_RELEASE, or add `version` to package.json')

const token = args.token || process.env.SENTORI_TOKEN || config.token
if (!token) fail('token not set: pass --token, set SENTORI_TOKEN, or add `token` to sentori.config.json')

const projectId = args.project || process.env.SENTORI_PROJECT_ID || config.projectId
if (!projectId)
  fail(
    'project not set: pass --project, set SENTORI_PROJECT_ID, or add `projectId` to sentori.config.json'
  )

const apiUrl =
  args['api-url'] || process.env.SENTORI_API_URL || config.apiUrl || 'https://sentori.golia.jp'

const sourceDir = resolveSourceDir(platform, cwd, config)
if (!fs.existsSync(sourceDir)) {
  fail(
    `source directory not found: ${sourceDir}\n` +
      `Set sources.${platform} in sentori.config.json if your tree lives elsewhere.`
  )
}

const cliBin = resolveCliBin(cwd)
const cliArgs = [
  'upload',
  'source-bundle',
  '--api-url',
  apiUrl,
  '--token',
  token,
  '--project',
  projectId,
  '--release',
  release,
  '--platform',
  platform,
]
if (args.module) {
  cliArgs.push('--module', args.module)
}
cliArgs.push(sourceDir)

console.log(
  `[sentori-rn-upload-source-bundle] ${platform} · release=${release} · src=${path.relative(cwd, sourceDir) || sourceDir}`
)
const result = spawnSync(cliBin, cliArgs, { stdio: 'inherit' })
if (result.error) {
  fail(`failed to spawn sentori-cli: ${result.error.message}`)
}
process.exit(result.status ?? 1)

// ── helpers ────────────────────────────────────────────────────────

function parseArgs(argv) {
  const out = {}
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]
    if (a === '-h' || a === '--help') {
      out.help = true
      continue
    }
    if (!a.startsWith('--')) continue
    const eq = a.indexOf('=')
    if (eq >= 0) {
      out[a.slice(2, eq)] = a.slice(eq + 1)
    } else {
      out[a.slice(2)] = argv[++i]
    }
  }
  return out
}

function loadConfig(customPath, cwd) {
  const file = customPath ? path.resolve(cwd, customPath) : path.join(cwd, 'sentori.config.json')
  if (!fs.existsSync(file)) return {}
  try {
    return JSON.parse(fs.readFileSync(file, 'utf8'))
  } catch (e) {
    fail(`failed to parse ${file}: ${e.message}`)
  }
}

function deriveReleaseFromPackageJson(cwd) {
  const file = path.join(cwd, 'package.json')
  if (!fs.existsSync(file)) return null
  try {
    const pkg = JSON.parse(fs.readFileSync(file, 'utf8'))
    if (pkg.name && pkg.version) return `${pkg.name}@${pkg.version}`
    return pkg.version ?? null
  } catch {
    return null
  }
}

function resolveSourceDir(platform, cwd, config) {
  const override = config.sources && config.sources[platform]
  if (override) return path.resolve(cwd, override)
  if (platform === 'ios') return path.join(cwd, 'ios')
  return path.join(cwd, 'android', 'app', 'src')
}

function resolveCliBin(cwd) {
  // Prefer the local install of `sentori-cli` so the operator's
  // pinned version of `@goliapkg/sentori-cli` is what runs. Falls
  // through to a global on PATH otherwise.
  const local = path.join(cwd, 'node_modules', '.bin', 'sentori-cli')
  if (fs.existsSync(local)) return local
  return 'sentori-cli'
}

function fail(msg) {
  console.error(`error: ${msg}\n${HELP}`)
  process.exit(2)
}
